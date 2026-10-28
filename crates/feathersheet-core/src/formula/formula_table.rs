//! Armazenamento colunar de fórmulas — o `data_table.rs` das fórmulas.
//!
//! Tradução de `src/core/formula-table.ts`.
//!
//! Uma coluna preenchida para baixo não guarda um milhão de fórmulas; guarda
//! o **padrão** uma vez (ver [`crate::formula::formula_pattern`]) e, por célula, dois
//! números:
//!
//!   - `slot` (`i32`) = número do padrão nos 24 bits altos, tipo do valor
//!     nos 8 baixos. É o `form`.
//!   - `payload` (`f64`) = o valor já calculado. É o `value`.
//!
//! São **12 bytes por fórmula**, contra os ~400 de um objeto num `Map`
//! (objeto, entrada do mapa, texto da fórmula e as entradas do grafo de
//! dependências). Em 6 milhões de fórmulas é a diferença entre 72 MB e 2,4 GB.
//!
//! O texto da fórmula não é guardado: é reconstruído do padrão quando alguém
//! pede ([`FormulaTable::raw_at`]), e o resultado bate caractere a caractere
//! com o que foi digitado — é o que [`crate::formula::formula_pattern::pattern_for`]
//! confere antes de aceitar um padrão.
//!
//! Fórmula que não vira padrão (ou que derrama matriz) não entra aqui: volta
//! para o `Map` do `sheet.ts`, como sempre foi.
//!
//! ## Diferença de posse, não de comportamento
//!
//! O TypeScript lê e escreve o `globalStringPool` (variável de módulo). Rust
//! não tem estado global mutável de graça — o mesmo motivo já documentado em
//! `pool.rs` e seguido por `data_table.rs` — então aqui o [`crate::celulas::pool::StringPool`]
//! entra como parâmetro explícito de quem chama, em vez de import implícito.
//! O valor gravado é idêntico; só a posse do pool muda de forma.
//!
//! ## Diferença de forma, não de resultado
//!
//! `columnsInUse()` e `entries()` eram geradores (`function*`) no TypeScript,
//! percorridos preguiçosamente. Rust não tem gerador pronto sem um crate a
//! mais (que `sheet-core` não pode ganhar — regra 11 do `CLAUDE.md`), então
//! aqui os dois devolvem `Vec` já montado. O conteúdo e a ordem por coluna
//! são os mesmos; só a preguiça do TypeScript não tem par direto.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::planilha::column_store::{ChunkedFloatColumn, ChunkedInt32Column};
use crate::formula::formula_pattern::{expand_pattern, pattern_for, refs_of_pattern, PatternRef};
use crate::formula::modelo_padrao::{faixas_do_padrao, FaixaPadrao};
use crate::celulas::pool::StringPool;
use crate::celulas::values::{CellError, ErrorCode, Value};

/// O que os 8 bits baixos do `slot` dizem sobre o valor guardado.
const KIND_DIRTY: i32 = 0;
const KIND_NUMBER: i32 = 1;
/// `payload` é um id do pool de textos.
const KIND_TEXT: i32 = 2;
/// `payload` é 0 ou 1.
const KIND_BOOL: i32 = 3;
/// `payload` é um índice em `errors`.
const KIND_ERROR: i32 = 4;
const KIND_EMPTY: i32 = 5;

pub const KIND_BITS: i32 = 8;
const KIND_MASK: i32 = 0xff;
/// Teto de padrões distintos: o id ocupa os 24 bits altos do `slot`. Uma aba
/// que passe disso não tem padrão nenhum de verdade (seria uma fórmula
/// diferente por célula), e aí o caminho antigo serve melhor mesmo.
const MAX_PATTERNS: i32 = 1 << 23;

#[derive(Default)]
struct Column {
    slot: ChunkedInt32Column,
    payload: ChunkedFloatColumn,
    /// Invalidação de toda a coluna em O(1), materializada apenas quando
    /// um resultado novo for consolidado. Não aloca estado por fórmula.
    invalidada: AtomicBool,
}

/// As três fatias de uma coluna salva, emprestadas (ver
/// [`FormulaTable::load_column_partes`]).
struct PartesColuna<'a> {
    slots: &'a [i32],
    payload: &'a [f64],
    bitmap: &'a [u8],
}

/// Uma coluna de fórmula em forma de bytes, pronta para gravar.
pub struct ColunaSalva {
    /// Número do padrão nos 24 bits altos, tipo do valor nos 8 baixos.
    pub slots: Vec<i32>,
    /// O valor: número cru, ou índice em `dict`/`erros`.
    pub payload: Vec<f64>,
    pub bitmap: Vec<u8>,
    pub dict: Vec<String>,
    pub erros: Vec<ErroSalvo>,
}

pub struct ErroSalvo {
    pub code: String,
    pub detail: Option<String>,
}

/// Um padrão em uso numa coluna, com o que se precisa saber dele.
pub struct PatternUse {
    pub id: i32,
    /// Referências de célula e pontas de faixas qualificadas, como
    /// deslocamento. Padrões constantes podem ter a lista vazia.
    pub refs: Vec<PatternRef>,
    /// Intervalos locais que o padrão lê (`AO:AO`, `A2:A10`).
    pub faixas: Vec<FaixaPadrao>,
}

/// As colunas que têm fórmula, com os padrões que cada uma usa.
pub struct ColumnUse {
    pub col: u32,
    pub usos: Vec<PatternUse>,
}

/// Uma coluna pronta para gravar (só as coordenadas, sem os bytes ainda).
pub struct ColumnForSave {
    pub col: u32,
    pub max_row: i64,
}

/// Uma fórmula percorrida por [`FormulaTable::entries`].
pub struct FormulaEntry {
    pub row: u32,
    pub col: u32,
    pub raw: String,
}

/// A maior linha e coluna ocupadas, ou `-1` quando não há nada — mesmo
/// sentinela do [`ChunkedFloatColumn`] de que esta tabela é feita.
pub struct Bounds {
    pub max_row: i64,
    pub max_col: i64,
}

/// Quem decide se uma fórmula pode morar aqui. É chamado **uma vez por
/// padrão**, não por célula: `=B2+C2` e `=B3+C3` são o mesmo padrão, então a
/// análise (que precisa compilar a fórmula) acontece três vezes numa
/// planilha de seis milhões de fórmulas, não seis milhões.
///
/// Devolve `false` para recusar o padrão — fórmula que derrama matriz ou que
/// é volátil precisa do objeto no `Map`, com estado próprio.
pub trait PatternCheck: FnMut(&str) -> bool {}
impl<F: FnMut(&str) -> bool> PatternCheck for F {}

#[derive(Default)]
pub struct FormulaTable {
    patterns: Vec<String>,
    pattern_ids: HashMap<String, i32>,
    /// As referências de cada padrão, calculadas uma vez.
    pattern_refs: Vec<Vec<PatternRef>>,
    /// Paralelo a `pattern_refs`: os intervalos locais de cada padrão.
    pattern_faixas: Vec<Vec<FaixaPadrao>>,
    /// Padrões já reprovados: recusa na hora, sem compilar de novo.
    rejeitados: HashSet<String>,
    columns: HashMap<u32, Column>,
    /// Padrões em uso por coluna — é o que o grafo deduzido percorre.
    used_by_col: HashMap<u32, HashSet<i32>>,
    /// Algum padrão em uso lê outra aba. Guardado porque é perguntado a cada
    /// célula escrita: percorrer os padrões toda vez pesava na importação.
    externas_em_uso: bool,
    errors: Vec<CellError>,
    error_ids: HashMap<String, i32>,
    count: u32,
}

impl FormulaTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Quantas fórmulas moram aqui.
    pub fn size(&self) -> u32 {
        self.count
    }

    /// Quantos padrões distintos existem (o tamanho real do "texto" guardado).
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    fn column(&mut self, col: u32) -> &mut Column {
        self.columns.entry(col).or_default()
    }

    /// Guarda a fórmula da célula. Devolve `false` quando ela não pode morar
    /// aqui — aí quem chamou manda pelo caminho antigo, o objeto no `Map`.
    ///
    /// Recusa quando a fórmula não vira padrão, quando lê intervalo local
    /// (o grafo de blocos cuida desses) ou quando o
    /// `check` reprova o padrão. Cada uma dessas decisões é tomada **uma vez
    /// por padrão** e lembrada.
    pub fn set(&mut self, row: u32, col: u32, raw: &str, check: impl PatternCheck) -> bool {
        let pattern = pattern_for(raw, row as i32, col as i32);
        self.set_com_padrao(row, col, raw, pattern.as_deref(), check)
    }

    /// [`Self::set`] com o padrão já calculado (`pattern_for`) — a abertura
    /// de arquivo calcula em várias threads e grava aqui, numa só.
    /// Referências locais são deduzidas por subtração; as qualificadas são
    /// conservadas por padrão para invalidação e edição estrutural.
    fn refs_aceitas(pattern: &str) -> Option<Vec<PatternRef>> {
        refs_of_pattern(pattern)
    }

    pub fn tem_externas(&self) -> bool {
        self.count > 0 && self.externas_em_uso
    }

    fn padrao_le_de_fora(&self, id: i32) -> bool {
        self.pattern_refs.get(id as usize).is_some_and(|refs| refs.iter().any(|r| r.sheet.is_some()))
    }

    /// Refaz [`Self::externas_em_uso`] depois de `used_by_col` encolher.
    fn recontar_externas(&mut self) {
        self.externas_em_uso = self.used_by_col.values()
            .any(|ids| ids.iter().any(|&id| self.padrao_le_de_fora(id)));
    }

    /// As linhas da coluna cuja fórmula ainda não tem resultado válido, em
    /// ordem crescente. É a lista de trabalho do cálculo em paralelo.
    pub(crate) fn linhas_sujas(&self, col: u32) -> Vec<u32> {
        let Some(entry) = self.columns.get(&col) else { return Vec::new() };
        if entry.slot.max_row < 0 {
            return Vec::new();
        }
        let toda = entry.invalidada.load(Ordering::Relaxed);
        (entry.slot.min_row..=entry.slot.max_row as u32)
            .filter(|&r| entry.slot.has(r)
                && (toda || entry.slot.get(r).unwrap_or(0) & KIND_MASK == KIND_DIRTY))
            .collect()
    }

    /// Se o padrão lê a célula imediatamente acima na mesma coluna
    /// (`K3=K2+I3`): é uma corrente, calculada de cima para baixo.
    pub(crate) fn padrao_le_linha_acima(&self, id: usize) -> bool {
        self.pattern_refs.get(id).is_some_and(|refs| refs.iter().any(|r|
            r.row == -1 && r.col == 0 && !r.row_lock && !r.col_lock && r.sheet.is_none()))
    }

    pub fn le_de_fora_em(&self, row: u32, col: u32) -> bool {
        self.padrao_em(row, col).is_some_and(|(id, _)| {
            self.pattern_refs[id].iter().any(|r| r.sheet.is_some())
        })
    }

    /// Se a fórmula `raw`, de padrão `pattern`, caberia aqui (fora o teto de
    /// padrões). Sem estado: a abertura de arquivo pergunta em várias
    /// threads, uma vez por padrão, para nem montar a árvore de quem cabe.
    pub fn aceitaria(pattern: &str, raw: &str, mut check: impl PatternCheck) -> bool {
        Self::refs_aceitas(pattern).is_some() && faixas_do_padrao(pattern).is_some() && check(raw)
    }

    /// O intervalo do padrão `id` passa pela própria coluna `col`: a
    /// fórmula leria a si mesma ou a vizinha de cima/baixo (`=SOMA(A:A)` em
    /// A). Fica avulsa, onde o cálculo célula a célula acha o circular.
    fn le_a_propria_coluna(&self, id: i32, col: u32) -> bool {
        self.pattern_faixas.get(id as usize).is_some_and(|faixas| faixas.iter()
            .any(|f| f.colunas_em(col).is_none_or(|(c0, c1)| col >= c0 && col <= c1)))
    }

    /// O [`Self::set_com_padrao`] de um padrão já registrado, sem o texto da
    /// célula; `None` quando o padrão ainda não existe (aí o texto é preciso).
    pub fn set_padrao_conhecido(&mut self, row: u32, col: u32, pattern: &str) -> Option<bool> {
        let id = *self.pattern_ids.get(pattern)?;
        Some(self.set_por_id(row, col, id))
    }

    pub fn set_com_padrao(&mut self, row: u32, col: u32, raw: &str, pattern: Option<&str>,
        mut check: impl PatternCheck) -> bool {
        let Some(pattern) = pattern else {
            return false;
        };

        let id = match self.pattern_ids.get(pattern) {
            Some(&id) => id,
            None => {
                if self.rejeitados.contains(pattern) {
                    return false;
                }
                if self.patterns.len() as i32 >= MAX_PATTERNS {
                    return false;
                }
                let Some((refs, faixas)) = Self::refs_aceitas(pattern)
                    .zip(faixas_do_padrao(pattern))
                    .filter(|_| check(raw)) else {
                    self.rejeitados.insert(pattern.to_string());
                    return false;
                };
                let id = self.patterns.len() as i32;
                self.patterns.push(pattern.to_string());
                self.pattern_refs.push(refs);
                self.pattern_faixas.push(faixas);
                self.pattern_ids.insert(pattern.to_string(), id);
                id
            }
        };
        self.set_por_id(row, col, id)
    }

    fn set_por_id(&mut self, row: u32, col: u32, id: i32) -> bool {
        if self.le_a_propria_coluna(id, col) {
            return false;
        }
        let ja_tinha = self.columns.get(&col).is_some_and(|e| e.slot.has(row));
        if !ja_tinha {
            self.count += 1;
        }
        let entry = self.column(col);
        entry.slot.set(row, (id << KIND_BITS) | KIND_DIRTY);
        self.used_by_col.entry(col).or_default().insert(id);
        if !self.externas_em_uso && self.padrao_le_de_fora(id) {
            self.externas_em_uso = true;
        }
        true
    }

    /// Se um padrão (já registrado ou não) poderia morar na coluna `col`:
    /// os intervalos dele não passam por ela.
    pub fn caberia_na_coluna(&self, padrao: &str, col: u32) -> bool {
        match self.pattern_ids.get(padrao) {
            Some(&id) => !self.le_a_propria_coluna(id, col),
            None => faixas_do_padrao(padrao).is_some_and(|faixas| faixas.iter()
                .all(|f| f.colunas_em(col).is_some_and(|(c0, c1)| col < c0 || col > c1))),
        }
    }

    /// O padrão que a fórmula `raw` tem na linha `r0` vai para `r0..=r1`
    /// da coluna `col`, que tem de estar vazia aqui, em blocos: preencher
    /// 13,7 milhões de linhas com a mesma fórmula relativa é gravar um número
    /// por linha, sem texto nem árvore (17/10/2026). `false` quando o padrão
    /// não cabe na coluna empacotada — nada é gravado.
    pub fn preencher_padrao(&mut self, col: u32, r0: u32, r1: u32, raw: &str, check: impl PatternCheck) -> bool {
        let Some(padrao) = pattern_for(raw, r0 as i32, col as i32) else { return false };
        if r0 > r1 || !self.set_com_padrao(r0, col, raw, Some(&padrao), check) {
            return false;
        }
        if r1 > r0 {
            let id = self.pattern_ids[&padrao];
            self.column(col).slot.preencher_faixa(r0 + 1, r1, (id << KIND_BITS) | KIND_DIRTY);
            self.count += r1 - r0;
        }
        true
    }

    /// Esvazia `r0..=r1` da coluna, em blocos (o desfazer do
    /// [`Self::preencher_padrao`]).
    pub fn limpar_faixa(&mut self, col: u32, r0: u32, r1: u32) {
        let Some(entry) = self.columns.get_mut(&col) else { return };
        let mut tinha = 0u32;
        if entry.slot.max_row >= r0 as i64 {
            for r in r0.max(entry.slot.min_row)..=r1.min(entry.slot.max_row as u32) {
                tinha += u32::from(entry.slot.has(r));
            }
        }
        entry.slot.apagar_faixa(r0, r1);
        entry.payload.apagar_faixa(r0, r1);
        self.count = self.count.saturating_sub(tinha);
    }

    pub fn has(&self, row: u32, col: u32) -> bool {
        self.columns.get(&col).is_some_and(|e| e.slot.has(row))
    }

    /// A máscara de presença das fórmulas do bloco `idx` da coluna.
    pub fn mascara_do_bloco(&self, col: u32, idx: usize) -> Option<&[u8]> {
        self.columns.get(&col)?.slot.mascara_do_bloco(idx)
    }

    /// O texto da fórmula, reconstruído do padrão.
    pub fn raw_at(&self, row: u32, col: u32) -> Option<String> {
        let entry = self.columns.get(&col)?;
        if !entry.slot.has(row) {
            return None;
        }
        let slot = entry.slot.get(row)?;
        let pattern = self.patterns.get((slot >> KIND_BITS) as usize)?;
        Some(expand_pattern(pattern, row as i32, col as i32))
    }

    /// O padrão da célula: o número dele e o texto com os marcadores (o que
    /// o [`crate::formula::formula_pattern::expand_pattern`] expande). Serve quem
    /// trabalha uma vez por padrão em vez de uma vez por célula — o
    /// exportador de `.xlsx` traduz a fórmula para o inglês assim.
    pub fn padrao_em(&self, row: u32, col: u32) -> Option<(usize, &str)> {
        let entry = self.columns.get(&col)?;
        if !entry.slot.has(row) {
            return None;
        }
        let id = (entry.slot.get(row)? >> KIND_BITS) as usize;
        Some((id, self.patterns.get(id)?.as_str()))
    }

    /// Se a célula já tem valor calculado.
    pub fn is_clean(&self, row: u32, col: u32) -> bool {
        let Some(entry) = self.columns.get(&col) else {
            return false;
        };
        if entry.invalidada.load(Ordering::Relaxed) || !entry.slot.has(row) {
            return false;
        }
        (entry.slot.get(row).unwrap_or(0) & KIND_MASK) != KIND_DIRTY
    }

    /// O valor guardado. Só faz sentido quando [`Self::is_clean`] — antes
    /// disso a célula nunca foi calculada e quem pergunta precisa calcular.
    pub fn value_at(&self, row: u32, col: u32, pool: &StringPool) -> Value {
        let Some(entry) = self.columns.get(&col) else {
            return Value::Empty;
        };
        if entry.invalidada.load(Ordering::Relaxed) || !entry.slot.has(row) {
            return Value::Empty;
        }
        let kind = entry.slot.get(row).unwrap_or(0) & KIND_MASK;
        let payload = entry.payload.get(row).unwrap_or(0.0);
        match kind {
            KIND_NUMBER => Value::Number(payload),
            KIND_TEXT => Value::Text(pool.get(payload as i32).to_string()),
            KIND_BOOL => Value::Bool(payload != 0.0),
            KIND_ERROR => self
                .errors
                .get(payload as usize)
                .cloned()
                .map(Value::Error)
                .unwrap_or_else(|| Value::erro(ErrorCode::Error)),
            _ => Value::Empty,
        }
    }

    /// O resultado já calculado, cru, quando é número ou texto: o número, ou o
    /// id do texto no pool. `None` quando a célula está suja ou o resultado é
    /// outra coisa (lógico, erro, vazio) — aí quem pergunta usa o
    /// [`Self::value_at`] de sempre. É o que a busca usa para não montar um
    /// `Value` por célula.
    pub fn cru_at(&self, row: u32, col: u32) -> Option<crate::planilha::data_table::Cru> {
        let entry = self.columns.get(&col)?;
        if entry.invalidada.load(Ordering::Relaxed) || !entry.slot.has(row) {
            return None;
        }
        let payload = entry.payload.get(row).unwrap_or(0.0);
        match entry.slot.get(row).unwrap_or(0) & KIND_MASK {
            KIND_NUMBER => Some(crate::planilha::data_table::Cru::Numero(payload)),
            KIND_TEXT => Some(crate::planilha::data_table::Cru::Texto(payload as i32)),
            _ => None,
        }
    }

    /// Guarda o resultado do cálculo.
    pub fn store(&mut self, row: u32, col: u32, value: &Value, pool: &mut StringPool) {
        let id = {
            let Some(entry) = self.columns.get_mut(&col) else {
                return;
            };
            if !entry.slot.has(row) {
                return;
            }
            // O acesso é exclusivo (&mut self): só uma leitura no caminho
            // comum, sem RMW atômico nem uma busca extra por resultado.
            if entry.invalidada.load(Ordering::Relaxed) {
                entry.invalidada.store(false, Ordering::Relaxed);
                for r in entry.slot.min_row..=entry.slot.max_row as u32 {
                    if let Some(slot) = entry.slot.get(r) {
                        entry.slot.set(r, (slot >> KIND_BITS) << KIND_BITS);
                    }
                }
            }
            entry.slot.get(row).unwrap_or(0) >> KIND_BITS
        };
        let (kind, payload) = match value {
            Value::Number(n) => (KIND_NUMBER, *n),
            Value::Text(t) => (KIND_TEXT, pool.get_or_create(t) as f64),
            Value::Bool(b) => (KIND_BOOL, if *b { 1.0 } else { 0.0 }),
            Value::Error(e) => (KIND_ERROR, self.error_id(e) as f64),
            Value::Empty => (KIND_EMPTY, 0.0),
        };
        let entry = self.column(col);
        entry.slot.set(row, (id << KIND_BITS) | kind);
        entry.payload.set(row, payload);
    }

    fn error_id(&mut self, err: &CellError) -> i32 {
        let chave = format!("{}\u{0}{}", err.code.as_str(), err.detail.as_deref().unwrap_or(""));
        if let Some(&id) = self.error_ids.get(&chave) {
            return id;
        }
        let id = self.errors.len() as i32;
        self.errors.push(err.clone());
        self.error_ids.insert(chave, id);
        id
    }

    /// Marca para recalcular. Devolve `false` se já estava suja ou não existe.
    pub fn mark_dirty(&mut self, row: u32, col: u32) -> bool {
        let Some(entry) = self.columns.get_mut(&col) else {
            return false;
        };
        if !entry.slot.has(row) {
            return false;
        }
        let slot = entry.slot.get(row).unwrap_or(0);
        if (slot & KIND_MASK) == KIND_DIRTY {
            return false;
        }
        entry.slot.set(row, (slot >> KIND_BITS) << KIND_BITS);
        true
    }

    /// Solicitação explícita de recálculo: invalida todos os resultados da
    /// tabela colunar em memória constante, preservando seus padrões.
    /// O valor será recalculado na próxima leitura (avaliação preguiçosa).
    pub fn marcar_todas_sujas(&self) {
        for entry in self.columns.values() {
            entry.invalidada.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn marcar_colunas_sujas(&self, colunas: &HashSet<u32>) {
        for col in colunas {
            if let Some(entry) = self.columns.get(col) {
                entry.invalidada.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn clear_cell(&mut self, row: u32, col: u32) {
        let Some(entry) = self.columns.get_mut(&col) else {
            return;
        };
        if !entry.slot.has(row) {
            return;
        }
        self.count -= 1;
        entry.slot.clear(row);
        entry.payload.clear(row);
    }

    pub fn clear(&mut self) {
        self.patterns.clear();
        self.pattern_ids.clear();
        self.pattern_refs.clear();
        self.pattern_faixas.clear();
        self.rejeitados.clear();
        self.columns.clear();
        self.used_by_col.clear();
        self.externas_em_uso = false;
        self.errors.clear();
        self.error_ids.clear();
        self.count = 0;
    }

    /// As colunas que têm fórmula, com os padrões que cada uma usa.
    pub fn columns_in_use(&self) -> Vec<ColumnUse> {
        self.used_by_col
            .iter()
            .map(|(&col, ids)| ColumnUse {
                col,
                usos: ids
                    .iter()
                    .map(|&id| PatternUse {
                        id,
                        refs: self.pattern_refs[id as usize].clone(),
                        faixas: self.pattern_faixas.get(id as usize).cloned().unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Reprova o padrão desta célula: nenhuma fórmula nova entra por ele
    /// daqui em diante. Serve para o caso que só aparece na hora de
    /// calcular — uma função que devolve matriz sem estar na lista de
    /// funções dinâmicas, que nenhuma análise do texto pega antes.
    pub fn reject_pattern_at(&mut self, row: u32, col: u32) {
        let Some(entry) = self.columns.get(&col) else {
            return;
        };
        if !entry.slot.has(row) {
            return;
        }
        let id = (entry.slot.get(row).unwrap_or(0) >> KIND_BITS) as usize;
        if let Some(pattern) = self.patterns.get(id) {
            self.rejeitados.insert(pattern.clone());
        }
    }

    /// Percorre as linhas da coluna que usam exatamente este padrão.
    pub fn for_each_with_pattern(&self, col: u32, id: i32, mut visit: impl FnMut(u32)) {
        let Some(entry) = self.columns.get(&col) else {
            return;
        };
        if entry.slot.max_row < 0 {
            return;
        }
        for r in entry.slot.min_row..=(entry.slot.max_row as u32) {
            if entry.slot.has(r) && (entry.slot.get(r).unwrap_or(0) >> KIND_BITS) == id {
                visit(r);
            }
        }
    }

    /// Se a célula usa exatamente este padrão.
    pub fn uses_pattern(&self, row: u32, col: u32, id: i32) -> bool {
        let Some(entry) = self.columns.get(&col) else {
            return false;
        };
        entry.slot.has(row) && (entry.slot.get(row).unwrap_or(0) >> KIND_BITS) == id
    }

    /// Percorre as fórmulas dentro do retângulo. Só toca as colunas que têm
    /// fórmula e só as linhas pedidas — é o que faz um `=SOMA(A1:A10)` no
    /// meio de uma planilha de dois milhões de linhas custar dez leituras.
    /// As colunas que têm fórmula.
    pub fn colunas(&self) -> Vec<u32> {
        self.columns.iter().filter(|(_, e)| e.slot.max_row >= 0).map(|(&c, _)| c).collect()
    }

    /// Última linha com fórmula na coluna sem expandir padrões em milhões
    /// de objetos. O limite já foi atualizado pela ChunkedColumn.
    pub fn ultima_linha_coluna(&self, col: u32) -> Option<u32> {
        let maximo = self.columns.get(&col)?.slot.max_row;
        u32::try_from(maximo).ok()
    }

    pub fn for_each_in_range(&self, r0: u32, r1: u32, c0: u32, c1: u32, mut visit: impl FnMut(u32, u32)) {
        for (&col, entry) in &self.columns {
            if col < c0 || col > c1 || entry.slot.max_row < 0 {
                continue;
            }
            let inicio = r0.max(entry.slot.min_row);
            let fim = r1.min(entry.slot.max_row as u32);
            if inicio > fim {
                continue;
            }
            for r in inicio..=fim {
                if entry.slot.has(r) {
                    visit(r, col);
                }
            }
        }
    }

    pub fn shift_columns(&mut self, at: u32, count: u32, removing: bool) {
        if count == 0 {
            return;
        }

        let mut new_columns: HashMap<u32, Column> = HashMap::with_capacity(self.columns.len());
        for (col, entry) in self.columns.drain() {
            if removing {
                if col >= at && col < at + count {
                    if entry.slot.max_row >= 0 {
                        for r in entry.slot.min_row..=(entry.slot.max_row as u32) {
                            if entry.slot.has(r) {
                                self.count = self.count.saturating_sub(1);
                            }
                        }
                    }
                    continue;
                }
                let target = if col >= at + count { col - count } else { col };
                new_columns.insert(target, entry);
            } else {
                let target = if col >= at { col + count } else { col };
                new_columns.insert(target, entry);
            }
        }
        self.columns = new_columns;

        let mut new_used: HashMap<u32, HashSet<i32>> = HashMap::with_capacity(self.used_by_col.len());
        for (col, used) in self.used_by_col.drain() {
            if removing {
                if col >= at && col < at + count {
                    continue;
                }
                let target = if col >= at + count { col - count } else { col };
                new_used.insert(target, used);
            } else {
                let target = if col >= at { col + count } else { col };
                new_used.insert(target, used);
            }
        }
        self.used_by_col = new_used;
    }

    /// Permuta as linhas `first..first + ordem.len()` da coluna `col` (o
    /// ordenar): a linha `ordem[i]` vai para `first + i`.
    ///
    /// O que se move é o número do padrão, não o texto: uma fórmula relativa
    /// levada de uma linha para outra continua sendo o mesmo padrão
    /// (`=B2*2` na linha 2 e `=B9*2` na linha 9 são um padrão só). A exceção
    /// é a referência que sairia da planilha no lugar novo (`=D1+C2` levada
    /// para a linha 1 apontaria para a linha 0): essa volta na lista, como
    /// `(destino, origem, texto na origem)`, para quem chamou regravar pelo
    /// texto — que vira `#REF!`, como no `shiftFormula`.
    ///
    /// O valor calculado vai junto e continua valendo quando tudo o que a
    /// fórmula lê é da mesma linha e de dentro de `c0..=c1` — o que andou
    /// com ela. Senão, a célula fica suja.
    pub fn permutar_linhas(&mut self, col: u32, first: u32, ordem: &[u32], c0: u32, c1: u32) -> Vec<(u32, u32, String)> {
        let Some(entry) = self.columns.get(&col) else {
            return Vec::new();
        };
        let last = first as u64 + ordem.len() as u64 - 1;
        if entry.slot.max_row < first as i64 || (entry.slot.min_row as u64) > last {
            return Vec::new();
        }
        // Por padrão: cabe em qualquer linha? anda com a linha?
        let mut limites: HashMap<i32, (i64, i64, bool)> = HashMap::new();
        let mut fora = Vec::new();
        for (i, &origem) in ordem.iter().enumerate() {
            let destino = first + i as u32;
            let Some(slot) = entry.slot.get(origem) else { continue };
            let id = slot >> KIND_BITS;
            let (menor, maior, _) = *limites.entry(id).or_insert_with(|| {
                let refs = self.pattern_refs.get(id as usize).map(Vec::as_slice).unwrap_or(&[]);
                let faixas = self.pattern_faixas.get(id as usize).map(Vec::as_slice).unwrap_or(&[]);
                let desloc: Vec<i64> = refs.iter().filter(|r| !r.row_lock).map(|r| r.row as i64)
                    .chain(faixas.iter().flat_map(FaixaPadrao::deslocamentos_de_linha)).collect();
                let menor = desloc.iter().copied().min().unwrap_or(0);
                let maior = desloc.iter().copied().max().unwrap_or(0);
                // Quem lê intervalo não leva o valor junto: o intervalo pode
                // ter mudado de ordem.
                let anda = faixas.is_empty() && refs.iter().all(|r| {
                    let alvo = if r.col_lock { r.col as i64 } else { col as i64 + r.col as i64 };
                    r.sheet.is_none() && !r.row_lock && r.row == 0 && alvo >= c0 as i64 && alvo <= c1 as i64
                });
                (menor, maior, anda)
            });
            if destino as i64 + menor < 0 || destino as i64 + maior >= crate::celulas::refs::MAX_ROWS as i64 {
                fora.push((destino, origem, self.raw_at(origem, col).unwrap_or_default()));
            }
        }
        let anda_por_id: HashMap<i32, bool> = limites.iter().map(|(&id, &(_, _, anda))| (id, anda)).collect();
        let entry = self.columns.get_mut(&col).expect("conferida acima");
        entry.slot.permutar(first, ordem);
        entry.payload.permutar(first, ordem);
        // O que não anda com a linha fica sujo.
        for i in 0..ordem.len() {
            let row = first + i as u32;
            let Some(slot) = entry.slot.get(row) else { continue };
            if !anda_por_id.get(&(slot >> KIND_BITS)).copied().unwrap_or(false) {
                entry.slot.set(row, (slot >> KIND_BITS) << KIND_BITS);
            }
        }
        fora
    }

    /// Suja toda fórmula fora do retângulo `first..=last` × `c0..=c1` — o que
    /// pode ler o que acabou de ser permutado.
    pub fn sujar_fora_de(&mut self, first: u32, last: u32, c0: u32, c1: u32) {
        for (&col, entry) in &mut self.columns {
            if entry.slot.max_row < 0 {
                continue;
            }
            for row in entry.slot.min_row..=(entry.slot.max_row as u32) {
                if col >= c0 && col <= c1 && row >= first && row <= last {
                    continue;
                }
                if let Some(slot) = entry.slot.get(row) {
                    if slot & KIND_MASK != KIND_DIRTY {
                        entry.slot.set(row, (slot >> KIND_BITS) << KIND_BITS);
                    }
                }
            }
        }
    }

    pub fn shift_rows(&mut self, at: u32, count: u32, removing: bool) {
        if count == 0 {
            return;
        }

        for (&_col, entry) in &mut self.columns {
            if entry.slot.max_row < at as i64 {
                continue;
            }

            let min_r = entry.slot.min_row;
            let max_r = entry.slot.max_row as u32;

            let mut old_entries = Vec::new();
            for r in min_r..=max_r {
                if entry.slot.has(r) {
                    let slot = entry.slot.get(r).unwrap_or(0);
                    let payload = entry.payload.get(r).unwrap_or(0.0);
                    old_entries.push((r, slot, payload));
                }
            }

            let mut new_slot = ChunkedInt32Column::new();
            let mut new_payload = ChunkedFloatColumn::new();

            for (r, slot, payload) in old_entries {
                if removing {
                    if r >= at && r < at + count {
                        self.count = self.count.saturating_sub(1);
                        continue;
                    }
                    let target_r = if r >= at + count { r - count } else { r };
                    new_slot.set(target_r, slot);
                    new_payload.set(target_r, payload);
                } else {
                    let target_r = if r >= at { r + count } else { r };
                    new_slot.set(target_r, slot);
                    new_payload.set(target_r, payload);
                }
            }

            entry.slot = new_slot;
            entry.payload = new_payload;
        }

        let mut cols_to_remove = Vec::new();
        for (&col, used) in &mut self.used_by_col {
            let mut new_used = HashSet::new();
            if let Some(entry) = self.columns.get(&col) {
                if entry.slot.max_row >= 0 {
                    for r in entry.slot.min_row..=(entry.slot.max_row as u32) {
                        if entry.slot.has(r) {
                            let slot = entry.slot.get(r).unwrap_or(0);
                            new_used.insert(slot >> KIND_BITS);
                        }
                    }
                }
            }
            if new_used.is_empty() {
                cols_to_remove.push(col);
            } else {
                *used = new_used;
            }
        }
        for col in cols_to_remove {
            self.used_by_col.remove(&col);
        }
        self.recontar_externas();
    }

    // ------------------------------------------------- gravar e carregar

    /// A tabela de padrões, na ordem dos ids.
    pub fn pattern_list(&self) -> &[String] {
        &self.patterns
    }

    /// As colunas que têm fórmula, para gravar.
    pub fn columns_for_save(&self) -> Vec<ColumnForSave> {
        self.columns
            .iter()
            .filter(|(_, entry)| entry.slot.max_row >= 0)
            .map(|(&col, entry)| ColumnForSave { col, max_row: entry.slot.max_row })
            .collect()
    }

    /// Uma coluna de fórmula pronta para o Arrow — **com o valor calculado**.
    ///
    /// Guardar o valor é o que evita a planilha jogar fora, ao ser gravada,
    /// um trabalho que ela já tinha feito. Sem isso, toda fórmula nasce suja
    /// ao abrir e o primeiro clique que a tocar paga a conta — é o que o
    /// Excel também faz.
    ///
    /// Dois valores não podem ir crus, porque apontam para tabelas que só
    /// existem nesta execução: texto (o `payload` é um id do pool) e erro (é
    /// um índice na tabela de erros da aba). Os dois viram índice num
    /// dicionário local, que vai junto.
    pub fn save_column(&self, col: u32, length: u32, pool: &StringPool) -> Option<ColunaSalva> {
        let entry = self.columns.get(&col)?;
        let length = length as usize;
        let (mut slots, bitmap) = entry.slot.flatten(length);
        let (brutos, _) = entry.payload.flatten(length);
        let mut payload = vec![0.0f64; length];
        let mut dict: Vec<String> = Vec::new();
        let mut por_pool: HashMap<i32, usize> = HashMap::new();
        let mut erros: Vec<ErroSalvo> = Vec::new();
        let mut por_erro: HashMap<usize, usize> = HashMap::new();
        // Erros vêm em sequência (a coluna inteira com o mesmo #REF!).
        let mut ultimo_erro: Option<(usize, usize)> = None;
        let invalidada = entry.invalidada.load(Ordering::Relaxed);

        for r in 0..length {
            if bitmap[r >> 3] & (1 << (r & 7)) == 0 {
                continue;
            }
            if invalidada {
                slots[r] = (slots[r] >> KIND_BITS) << KIND_BITS;
                continue;
            }
            let kind = slots[r] & KIND_MASK;
            let bruto = brutos[r];
            if kind == KIND_TEXT {
                let id = bruto as i32;
                let local = *por_pool.entry(id).or_insert_with(|| {
                    dict.push(pool.get(id).to_string());
                    dict.len() - 1
                });
                payload[r] = local as f64;
            } else if kind == KIND_ERROR {
                // O mesmo erro repetido vira uma entrada só: 1 milhão de #REF!
                // eram 2 milhões de Strings e 75 ms por coluna ao salvar.
                let local = match ultimo_erro {
                    Some((id, local)) if id == bruto as usize => local,
                    _ => {
                        let local = *por_erro.entry(bruto as usize).or_insert_with(|| {
                            let err = self.errors.get(bruto as usize);
                            erros.push(ErroSalvo {
                                code: err.map(|e| e.code.as_str().to_string()).unwrap_or_else(|| "#ERROR!".to_string()),
                                detail: err.and_then(|e| e.detail.clone()),
                            });
                            erros.len() - 1
                        });
                        ultimo_erro = Some((bruto as usize, local));
                        local
                    }
                };
                payload[r] = local as f64;
            } else {
                payload[r] = bruto;
            }
        }
        Some(ColunaSalva { slots, payload, bitmap, dict, erros })
    }

    /// Devolve a tabela de padrões lida do arquivo. Os padrões já passaram
    /// pelo `check` quando foram gravados, então aqui não se reanalisa — e
    /// se algum deles derramar matriz numa versão futura do motor, quem
    /// calcula faz a célula migrar para o `Map` sozinha.
    pub fn load_patterns(&mut self, patterns: &[String]) {
        for pattern in patterns {
            if self.pattern_ids.contains_key(pattern) {
                continue;
            }
            let refs = refs_of_pattern(pattern);
            self.pattern_ids.insert(pattern.clone(), self.patterns.len() as i32);
            self.patterns.push(pattern.clone());
            // Um padrão que o arquivo trouxe mas que hoje não se deduz
            // (intervalo) não deveria existir; sem referências, ninguém o
            // suja — melhor um vetor vazio do que faltar a entrada e
            // desalinhar os ids.
            self.pattern_refs.push(refs.unwrap_or_default());
            self.pattern_faixas.push(faixas_do_padrao(pattern).unwrap_or_default());
        }
    }

    /// Carrega uma coluna inteira, **já calculada**. O texto e o erro voltam
    /// do dicionário local para as tabelas desta execução.
    pub fn load_column(&mut self, col: u32, salva: &ColunaSalva, length: u32, pool: &mut StringPool) {
        let erros: Vec<(&str, Option<&str>)> = salva.erros.iter().map(|e| (e.code.as_str(), e.detail.as_deref())).collect();
        self.load_column_partes(col, &salva.slots, &salva.payload, &salva.bitmap, &salva.dict, &erros, length, pool);
    }

    /// [`Self::load_column`] com as partes emprestadas de quem leu o arquivo,
    /// sem copiar os vetores de 1 milhão de linhas para um `ColunaSalva`.
    #[allow(clippy::too_many_arguments)]
    pub fn load_column_partes(&mut self, col: u32, slots: &[i32], payload: &[f64], bitmap: &[u8],
        dict: &[String], erros: &[(&str, Option<&str>)], length: u32, pool: &mut StringPool) {
        let length = (length as usize).min(slots.len()).min(payload.len()).min(bitmap.len() * 8);
        let pool_ids: Vec<i32> = dict.iter().map(|t| pool.get_or_create(t)).collect();
        let erros_locais: Vec<i32> = erros
            .iter()
            .map(|(code, detail)| {
                let code = ErrorCode::from_code(code).unwrap_or(ErrorCode::Error);
                let err = match detail {
                    Some(d) => CellError::com_motivo(code, d.to_string()),
                    None => CellError::new(code),
                };
                self.error_id(&err)
            })
            .collect();
        let salva = PartesColuna { slots, payload, bitmap };
        let mut novos_slots = vec![0i32; length];
        let mut novo_payload = vec![0.0f64; length];
        let mut bitmap = salva.bitmap.to_vec();
        let padroes = self.patterns.len();
        let usados = self.used_by_col.entry(col).or_default();

        for r in 0..length {
            if bitmap[r >> 3] & (1 << (r & 7)) == 0 {
                continue;
            }
            let slot = salva.slots[r];
            // Arquivo corrompido: padrão que não existe derrubaria o app na primeira edição.
            if slot < 0 || (slot >> KIND_BITS) as usize >= padroes {
                bitmap[r >> 3] &= !(1 << (r & 7));
                continue;
            }
            let kind = slot & KIND_MASK;
            novos_slots[r] = slot;
            usados.insert(slot >> KIND_BITS);
            self.count += 1;
            let bruto = salva.payload[r];
            novo_payload[r] = if kind == KIND_TEXT {
                pool_ids.get(bruto as usize).copied().unwrap_or(0) as f64
            } else if kind == KIND_ERROR {
                erros_locais.get(bruto as usize).copied().unwrap_or(0) as f64
            } else {
                bruto
            };
        }
        let entry = self.column(col);
        entry.slot.load_flat(&novos_slots, &bitmap, length);
        entry.payload.load_flat(&novo_payload, &bitmap, length);
        entry.invalidada.store(false, Ordering::Relaxed);
        self.recontar_externas();
    }

    /// Percorre as fórmulas do retângulo entregando **o valor já calculado**.
    ///
    /// É o caminho rápido de leitura: resolve a coluna uma vez e lê do array
    /// empacotado, como na coluna de números, em vez de voltar por cinco
    /// buscas em mapa por célula pelo caminho geral.
    ///
    /// Célula ainda sem valor é entregue a `nao_calculada`, para quem chama
    /// decidir (normalmente, calcular).
    // Os quatro limites do retângulo continuam soltos, como no TypeScript
    // (`r0, r1, c0, c1`) — 8 parâmetros com `self` e `pool`, um a mais do
    // que o clippy pede por padrão. `data_table.rs` embrulha isso num
    // `Rect`, mas mudar só este método deixaria os dois estilos
    // convivendo aqui dentro; ficar igual ao `.ts` pesou mais.
    #[allow(clippy::too_many_arguments)]
    pub fn for_each_value_in_range(
        &self,
        r0: u32,
        r1: u32,
        c0: u32,
        c1: u32,
        pool: &StringPool,
        mut visit: impl FnMut(&Value, u32, u32),
        mut nao_calculada: impl FnMut(u32, u32),
    ) {
        for (&col, entry) in &self.columns {
            if col < c0 || col > c1 || entry.slot.max_row < 0 {
                continue;
            }
            let inicio = r0.max(entry.slot.min_row);
            let fim = r1.min(entry.slot.max_row as u32);
            if inicio > fim {
                continue;
            }
            for r in inicio..=fim {
                if !entry.slot.has(r) {
                    continue;
                }
                let kind = entry.slot.get(r).unwrap_or(0) & KIND_MASK;
                if entry.invalidada.load(Ordering::Relaxed) || kind == KIND_DIRTY {
                    nao_calculada(r, col);
                    continue;
                }
                let payload = entry.payload.get(r).unwrap_or(0.0);
                match kind {
                    KIND_NUMBER => visit(&Value::Number(payload), r, col),
                    KIND_TEXT => visit(&Value::Text(pool.get(payload as i32).to_string()), r, col),
                    KIND_BOOL => visit(&Value::Bool(payload != 0.0), r, col),
                    KIND_ERROR => {
                        let val = self
                            .errors
                            .get(payload as usize)
                            .cloned()
                            .map(Value::Error)
                            .unwrap_or_else(|| Value::erro(ErrorCode::Error));
                        visit(&val, r, col);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Percorre toda fórmula guardada. A ordem é por coluna.
    pub fn entries(&self) -> Vec<FormulaEntry> {
        let mut out = Vec::new();
        for (&col, entry) in &self.columns {
            if entry.slot.max_row < 0 {
                continue;
            }
            for r in entry.slot.min_row..=(entry.slot.max_row as u32) {
                if !entry.slot.has(r) {
                    continue;
                }
                let slot = entry.slot.get(r).unwrap_or(0);
                let pattern = &self.patterns[(slot >> KIND_BITS) as usize];
                out.push(FormulaEntry { row: r, col, raw: expand_pattern(pattern, r as i32, col as i32) });
            }
        }
        out
    }

    /// A maior linha e coluna ocupadas, ou `-1` quando não há nada.
    pub fn bounds(&self) -> Bounds {
        let mut max_row: i64 = -1;
        let mut max_col: i64 = -1;
        for (&col, entry) in &self.columns {
            if entry.slot.max_row < 0 {
                continue;
            }
            if entry.slot.max_row > max_row {
                max_row = entry.slot.max_row;
            }
            if col as i64 > max_col {
                max_col = col as i64;
            }
        }
        Bounds { max_row, max_col }
    }
}

#[cfg(test)]
mod tests;
