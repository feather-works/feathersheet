//! Cálculo das fórmulas empacotadas em várias threads.
//!
//! O cálculo normal é preguiçoso e recursivo: pedir uma célula calcula quem
//! ela lê, guardando tudo em `RefCell` — a `Sheet` não atravessa thread.
//! Aqui o trabalho é feito por coluna, na ordem das dependências:
//!
//! 1. cada coluna da [`FormulaTable`] com linha suja vira um nó; as
//!    referências dos padrões dizem de quais colunas (desta ou de outra aba)
//!    ela depende;
//! 2. os nós são separados em níveis (ordem topológica). Colunas do mesmo
//!    nível não se leem, então são calculadas ao mesmo tempo, em fatias de
//!    linhas, por `paralelismo::nucleos()` threads (todas menos uma);
//! 3. entre um nível e o próximo, os resultados são gravados na coluna, em
//!    série — o próximo nível os lê já calculados.
//!
//! As threads só leem estruturas sem mutação interna (`DataTable`,
//! `FormulaTable`, `StringPool`, o mapa das avulsas). A coluna que lê a si
//! mesma em linhas anteriores (o saldo acumulado `K3=K2+I3`) é calculada por
//! uma thread só, de cima para baixo, guardando os resultados ao lado.
//!
//! O que não cabe nesse desenho fica como estava — sujo — e o cálculo
//! preguiçoso de sempre resolve quando a célula for lida: coluna que lê
//! fórmula avulsa, ciclo entre colunas, aba que não existe, função de macro.
//! O resultado é o mesmo; só a velocidade muda.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::celulas::pool::StringPool;
use crate::celulas::mapa_chave::MapaChave;
use crate::celulas::refs::Rect;
use crate::celulas::values::{CellError, ErrorCode, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::{EvalContext, Evaluator};
use crate::formula::formula_table::FormulaTable;
use crate::formula::modelo_padrao::ModeloPadrao;
use crate::formula::parser::parse_formula;
use crate::planilha::data_table::DataTable;
use crate::planilha::agregado::Agregado;
use crate::planilha::faixa_lida::{FaixaLida, MAXIMO, MINIMO, TOTAL};
use crate::planilha::sheet::{mesmo_nome, CellData, Sheet};
use crate::planilha::workbook::AbaViva;

/// Linhas gravadas por lote: os resultados de um lote ficam num `Vec` até
/// serem guardados na coluna (~32 B por linha), então o lote limita o pico.
const LINHAS_POR_LOTE: usize = 2_000_000;
/// Tarefas por thread em cada lote. Mais de uma equilibra a carga quando
/// umas linhas custam muito mais que outras (um `SOMASES` sobre um milhão de
/// linhas ao lado de um `=B2*2`); uma fatia pode ter uma linha só.
const TAREFAS_POR_THREAD: usize = 8;

/// Uma aba vista só para leitura: tudo aqui é `Sync`.
struct Vista<'a> {
    nome: &'a str,
    data: &'a DataTable,
    formulas: &'a FormulaTable,
    pool: &'a StringPool,
    cells: &'a crate::celulas::mapa_chave::MapaChave<CellData>,
    row_heights: &'a HashMap<u32, u32>,
    /// `Sheet::linhas_ocultas_desde`: ocultas até o fim da aba.
    linhas_ocultas_desde: Option<u32>,
    topicos: &'a crate::planilha::sheet::topicos::EixoTopicos,
    /// Linhas (em ordem) das fórmulas avulsas por coluna: o valor delas mora
    /// no `RefCell` da aba, e só as de `prontas` podem ser lidas daqui.
    avulsas: HashMap<u32, Vec<u32>>,
    prontas: &'a MapaChave<Value>,
    /// O retângulo usado (`None`: aba vazia) — a faixa não passa dele.
    usado: Option<Rect>,
    /// Áreas derramadas por matriz dinâmica: o valor mora no `RefCell` da aba.
    derramadas: Vec<Rect>,
}

impl<'a> Vista<'a> {
    fn nova(sheet: &'a Sheet, prontas: &'a MapaChave<Value>) -> Self {
        Self::com_nome(sheet, &sheet.name, prontas)
    }

    /// Aba de outro arquivo (`[Vendas.xlsx]Dados`), achada pelo nome como a fórmula escreveu.
    fn com_nome(sheet: &'a Sheet, nome: &'a str, prontas: &'a MapaChave<Value>) -> Self {
        Vista {
            nome,
            data: &sheet.data,
            formulas: &sheet.formulas,
            pool: &sheet.pool,
            cells: &sheet.cells,
            row_heights: &sheet.row_heights,
            linhas_ocultas_desde: sheet.linhas_ocultas_desde,
            topicos: &sheet.topicos.linhas,
            avulsas: avulsas_por_coluna(sheet),
            prontas,
            derramadas: sheet.areas_derramadas(),
            usado: (!sheet.is_empty()).then(|| sheet.used_rect()),
        }
    }
}

type ChaveFaixa = (usize, u32, u32, u32, u32);

/// Faixas lidas de uma vez, divididas entre as threads de um lote: o
/// `SOMASES` sobre um milhão de linhas lê a coluna uma vez, não uma por
/// fórmula. É o [`crate::planilha::faixa_lida`] do cálculo preguiçoso, com
/// um `OnceLock` por faixa para duas threads não lerem a mesma.
#[derive(Default)]
struct Faixas {
    mapa: Mutex<HashMap<ChaveFaixa, Arc<OnceLock<Option<Arc<FaixaLida>>>>>>,
    celulas: AtomicUsize,
    /// Agregados por critério (SOMASES, CONT.SES…), montados das faixas
    /// acima uma vez por conjunto de faixas e usados por todas as threads.
    agregados: Mutex<HashMap<ChaveAgregado, Arc<OnceLock<Option<Arc<Agregado>>>>>>,
    /// Grupos guardados nos agregados acima; passou do teto, para de agregar (conta direto).
    grupos: AtomicUsize,
}

type AreaFaixa = (u32, u32, u32, u32);
/// Aba, faixa somada (se houver) e faixas de critério.
type ChaveAgregado = (usize, Option<AreaFaixa>, Vec<AreaFaixa>);

/// Uma coluna em cálculo pela própria thread, de cima para baixo.
struct Corrente {
    aba: usize,
    col: u32,
    primeira: u32,
    valores: RefCell<Vec<Option<Value>>>,
}

/// O contexto do avaliador numa thread. Quando algo não pode ser lido daqui
/// (fórmula avulsa, célula ainda suja), marca `falhou`: a linha não é
/// gravada e fica para o cálculo preguiçoso.
struct Contexto<'a, 'v> {
    abas: &'v [Vista<'a>],
    atual: usize,
    corrente: Option<&'v Corrente>,
    funcoes_externas: &'v HashSet<String>,
    faixas: &'v Faixas,
    /// As faixas já pedidas por esta tarefa: o `Mutex` de [`Faixas`] é
    /// tomado uma vez por faixa, não a cada `PROCX` (1 milhão deles na mesma
    /// lista disputavam o cadeado entre as threads: ~2 s de 2,1 s).
    lidas: RefCell<HashMap<ChaveFaixa, Option<Arc<FaixaLida>>>>,
    falhou: Cell<bool>,
}

impl Contexto<'_, '_> {
    fn indice(&self, nome: Option<&str>) -> Option<usize> {
        match nome {
            None => Some(self.atual),
            Some(n) => self.abas.iter().position(|a| mesmo_nome(a.nome, n)),
        }
    }

    fn falhar(&self) -> Value {
        self.falhou.set(true);
        Value::Empty
    }

    /// Lê a faixa inteira (até a borda usada da aba). `None` se alguma
    /// célula não pôde ser lida daqui, ou se não compensa/cabe guardar.
    fn montar_faixa(&self, aba: usize, rect: &Rect) -> Option<Arc<FaixaLida>> {
        let usado = self.abas[aba].usado.as_ref();
        let (r1, c1) = usado.map_or((0, 0), |u| (rect.r1.min(u.r1), rect.c1.min(u.c1)));
        let (linhas, colunas) = match usado {
            Some(_) if r1 >= rect.r0 && c1 >= rect.c0 => (r1 - rect.r0 + 1, c1 - rect.c0 + 1),
            _ => (0, 0),
        };
        let n = linhas as u64 * colunas as u64;
        if n > MAXIMO || self.faixas.celulas.fetch_add(n as usize, Ordering::Relaxed) + n as usize > TOTAL {
            return None;
        }
        let antes = self.falhou.replace(false);
        let mut valores = Vec::with_capacity(n as usize);
        for r in rect.r0..rect.r0 + linhas {
            for c in rect.c0..rect.c0 + colunas {
                valores.push(self.ler(aba, r, c));
            }
        }
        if self.falhou.get() {
            return None;
        }
        self.falhou.set(antes);
        Some(Arc::new(FaixaLida::nova(rect.r0, rect.c0, linhas, colunas, valores)))
    }

    fn ler(&self, aba: usize, row: u32, col: u32) -> Value {
        let vista = &self.abas[aba];
        if let Some(c) = self.corrente.filter(|c| c.aba == aba && c.col == col && row >= c.primeira) {
            if let Some(Some(v)) = c.valores.borrow().get((row - c.primeira) as usize) {
                return v.clone();
            }
        }
        if vista.derramadas.iter().any(|a| row >= a.r0 && row <= a.r1 && col >= a.c0 && col <= a.c1) {
            return self.falhar();
        }
        let chave = Sheet::cell_key(row, col);
        if let Some(v) = vista.prontas.get(&chave) {
            return v.clone();
        }
        if vista.cells.contains_key(&chave) {
            return self.falhar();
        }
        if vista.formulas.has(row, col) {
            if !vista.formulas.is_clean(row, col) {
                return self.falhar();
            }
            return vista.formulas.value_at(row, col, vista.pool);
        }
        vista.data.get_cell_value(row, col, vista.pool)
    }
}

impl EvalContext for Contexto<'_, '_> {
    fn get_value(&self, row: u32, col: u32, sheet: Option<&str>) -> Value {
        match self.indice(sheet) {
            Some(aba) => self.ler(aba, row, col),
            None => self.falhar(),
        }
    }

    fn for_each_in_range(&self, rect: &Rect, visit: &mut dyn FnMut(&Value, u32, u32)) {
        let Some(aba) = self.indice(rect.sheet.as_deref()) else {
            self.falhar();
            return;
        };
        let vista = &self.abas[aba];
        let dentro = |r: u32, c: u32| r >= rect.r0 && r <= rect.r1 && c >= rect.c0 && c <= rect.c1;
        let cruza = |a: &Rect| a.r0 <= rect.r1 && a.r1 >= rect.r0 && a.c0 <= rect.c1 && a.c1 >= rect.c0;
        if vista.derramadas.iter().any(cruza) {
            self.falhar();
            return;
        }
        // Avulsa ainda por calcular: falha. Já calculada: só a leitura célula
        // a célula (`ler`) a enxerga, o caminho em bloco do fim, não.
        let mut com_avulsa = false;
        for (&c, linhas) in &vista.avulsas {
            if c < rect.c0 || c > rect.c1 {
                continue;
            }
            let i = linhas.partition_point(|&r| r < rect.r0);
            for &r in linhas[i..].iter().take_while(|&&r| r <= rect.r1) {
                if !vista.prontas.contains_key(&Sheet::cell_key(r, c)) {
                    self.falhar();
                    return;
                }
                com_avulsa = true;
            }
        }
        if self.corrente.is_some_and(|c| c.aba == aba && dentro(c.primeira.max(rect.r0), c.col)) {
            self.falhar();
            return;
        }
        // Linha a linha, como o cálculo serial (faixa lida ou, pequena,
        // célula a célula): a soma de ponto flutuante depende da ordem, e a
        // faixa pode faltar aqui conforme as threads gastam o orçamento (#116).
        if let Some(faixa) = self.faixa_lida(rect) {
            return faixa.para_cada(visit);
        }
        let usado = vista.usado.as_ref();
        let (r1, c1) = usado.map_or((0, 0), |u| (rect.r1.min(u.r1), rect.c1.min(u.c1)));
        if usado.is_some() && r1 >= rect.r0 && c1 >= rect.c0
            && (r1 - rect.r0 + 1) as u64 * (c1 - rect.c0 + 1) as u64 <= MAXIMO {
            for r in rect.r0..=r1 {
                for c in rect.c0..=c1 {
                    let v = self.ler(aba, r, c);
                    if self.falhou.get() {
                        return;
                    }
                    if !matches!(v, Value::Empty) {
                        visit(&v, r, c);
                    }
                }
            }
            return;
        }
        if com_avulsa {
            self.falhar();
            return;
        }
        let mut suja = false;
        vista.formulas.for_each_value_in_range(
            rect.r0, rect.r1, rect.c0, rect.c1, vista.pool,
            |v, r, c| visit(v, r, c),
            |_, _| suja = true,
        );
        if suja {
            self.falhar();
            return;
        }
        vista.data.for_each_in_range(rect, vista.pool, visit);
    }

    fn faixa_lida(&self, rect: &Rect) -> Option<Arc<FaixaLida>> {
        let aba = self.indice(rect.sheet.as_deref())?;
        let pedidas = (rect.r1 as u64 - rect.r0 as u64 + 1) * (rect.c1 as u64 - rect.c0 as u64 + 1);
        if pedidas < MINIMO {
            return None;
        }
        // A coluna em cálculo nesta thread muda a cada linha: não se guarda.
        if self.corrente.is_some_and(|c| c.aba == aba && c.col >= rect.c0 && c.col <= rect.c1) {
            return None;
        }
        let chave = (aba, rect.r0, rect.r1, rect.c0, rect.c1);
        if let Some(f) = self.lidas.borrow().get(&chave) {
            return f.clone();
        }
        let celula = self.faixas.mapa.lock().expect("faixas").entry(chave).or_default().clone();
        let faixa = celula.get_or_init(|| self.montar_faixa(aba, rect)).clone();
        self.lidas.borrow_mut().insert(chave, faixa.clone());
        if faixa.is_none() {
            // Quem chama lê célula por célula; se a faixa tinha algo
            // ilegível daqui, `for_each_in_range` marca a falha.
            return None;
        }
        faixa
    }

    fn range_size(&self, rect: &Rect) -> u64 {
        (rect.c1 - rect.c0 + 1) as u64 * (rect.r1 - rect.r0 + 1) as u64
    }

    /// O `usado` da vista, tirado antes do cálculo: as fórmulas sujas já
    /// estão na `FormulaTable` (contam no retângulo), e depois dele `ler`
    /// só acharia vazio. Aba que não existe: `None` (a leitura marca a falha).
    fn ultima_linha_usada(&self, sheet: Option<&str>) -> Option<u32> {
        Some(self.abas[self.indice(sheet)?].usado.as_ref().map_or(0, |u| u.r1))
    }

    /// O agregado por critério também no cálculo paralelo: montado das
    /// faixas lidas (compartilhadas entre as threads), uma vez por conjunto
    /// de faixas. Sem isto cada `SOMASES` da Resumo varria 1 milhão de
    /// linhas (~0,7 s por coluna). `None`: varrer célula por célula.
    fn agregado(&self, soma: Option<&Rect>, criterios: &[Rect]) -> Option<Arc<Agregado>> {
        let primeiro = criterios.first()?;
        let aba = self.indice(primeiro.sheet.as_deref())?;
        let mesma = |r: &Rect| self.indice(r.sheet.as_deref()) == Some(aba);
        if !criterios.iter().all(mesma) || !soma.is_none_or(mesma) {
            return None;
        }
        let linhas = primeiro.r1.checked_sub(primeiro.r0)? + 1;
        let colunas = primeiro.c1.checked_sub(primeiro.c0)? + 1;
        if (linhas as u64) * (colunas as u64) < crate::planilha::agregado::MINIMO {
            return None;
        }
        let mesmo_tamanho = |r: &Rect| r.r1.checked_sub(r.r0).map(|x| x + 1) == Some(linhas)
            && r.c1.checked_sub(r.c0).map(|x| x + 1) == Some(colunas);
        if !criterios.iter().all(mesmo_tamanho) || !soma.is_none_or(mesmo_tamanho) {
            return None;
        }
        // A coluna em cálculo nesta thread muda a cada linha: não se agrega.
        if self.corrente.is_some_and(|c| c.aba == aba
            && criterios.iter().chain(soma).any(|r| c.col >= r.c0 && c.col <= r.c1)) {
            return None;
        }
        let area = |r: &Rect| (r.r0, r.c0, r.r1, r.c1);
        let chave = (aba, soma.map(area), criterios.iter().map(area).collect::<Vec<_>>());
        // `CONT.SE($A$2:A2;A2)` arrastado: uma faixa nova por linha, e sem teto a memória acabava.
        let cheio = self.faixas.grupos.load(std::sync::atomic::Ordering::Relaxed) > crate::planilha::agregado::LIMITE_GRUPOS;
        let existente = self.faixas.agregados.lock().expect("agregados").get(&chave).cloned();
        if cheio && existente.is_none() { return None; }
        let celula = existente.unwrap_or_else(|| self.faixas.agregados.lock().expect("agregados").entry(chave).or_default().clone());
        let contar = |a: Option<Arc<Agregado>>| {
            if let Some(a) = &a { self.faixas.grupos.fetch_add(a.len(), std::sync::atomic::Ordering::Relaxed); }
            a
        };
        celula.get_or_init(|| contar((|| {
            let sem_pool = |_: i32| "";
            let leitor = std::cell::RefCell::new(crate::planilha::agregado::LeitorChaves::novo(&sem_pool));
            let lidas: Option<Vec<Arc<FaixaLida>>> = criterios.iter().map(|r| self.faixa_lida(r)).collect();
            // `Some(None)`: não há faixa somada; `None`: ela não coube na cópia.
            let somada: Option<Option<Arc<FaixaLida>>> = match soma {
                Some(r) => self.faixa_lida(r).map(Some),
                None => Some(None),
            };
            if let (Some(lidas), Some(somada)) = (lidas, somada) {
                return Agregado::montar(criterios.len(), soma.is_some(), linhas, colunas,
                    |i, r, c| leitor.borrow_mut().da_celula(lidas[i].valor(criterios[i].r0 + r, criterios[i].c0 + c)),
                    |r, c| match (soma, somada.as_ref()) {
                        (Some(rect), Some(f)) => match f.valor(rect.r0 + r, rect.c0 + c) { Value::Number(n) => Some(*n), _ => None },
                        _ => None,
                    },
                    || std::mem::take(&mut leitor.borrow_mut().chaves),
                ).map(Arc::new);
            }
            // A faixa não coube na cópia (mais de 4 milhões de células): lê direto
            // das colunas, até a última linha usada. Sem isto o MÍNIMOSES de 13,7
            // milhões de linhas varria a coluna inteira por linha (28/10/2026).
            let ultima = self.abas[aba].usado.as_ref().map_or(0, |u| u.r1);
            let linhas_lidas = if ultima < primeiro.r0 { 0 } else { linhas.min(ultima - primeiro.r0 + 1) };
            let antes = self.falhou.replace(false);
            let agregado = Agregado::montar(criterios.len(), soma.is_some(), linhas_lidas, colunas,
                |i, r, c| leitor.borrow_mut().da_celula(&self.ler(aba, criterios[i].r0 + r, criterios[i].c0 + c)),
                |r, c| match soma.map(|rect| self.ler(aba, rect.r0 + r, rect.c0 + c)) {
                    Some(Value::Number(n)) => Some(n),
                    _ => None,
                },
                || std::mem::take(&mut leitor.borrow_mut().chaves),
            );
            // Alguma célula ilegível daqui (avulsa, ainda suja): varre célula por célula.
            if self.falhou.replace(antes) { return None; }
            agregado.map(|a| Arc::new(a.com_total(linhas as u64 * colunas as u64, linhas_lidas as u64 * colunas as u64)))
        })())).clone()
    }

    fn resolve_name(&self, _name: &str) -> Option<Rect> {
        self.falhar();
        None
    }

    fn formula_de(&self, _row: u32, _col: u32, _sheet: Option<&str>) -> Option<String> {
        self.falhar();
        None
    }

    fn abas(&self) -> (Vec<String>, Option<String>) {
        self.falhar();
        (Vec::new(), None)
    }

    fn funcao_externa(&self, nome: &str) -> bool {
        if self.funcoes_externas.contains(&nome.to_uppercase()) {
            self.falhar();
        }
        false
    }

    fn linha_no_topico(&self, row: u32, sheet: Option<&str>) -> bool {
        self.indice(sheet).is_some_and(|a| self.abas[a].topicos.nivel(row) > 0)
    }

    fn linha_oculta(&self, row: u32, sheet: Option<&str>) -> bool {
        self.indice(sheet).is_some_and(|a| {
            let v = &self.abas[a];
            v.linhas_ocultas_desde.is_some_and(|n| row >= n) || v.row_heights.get(&row) == Some(&0)
        })
    }

    fn celula_com_subtotal(&self, row: u32, col: u32, sheet: Option<&str>) -> bool {
        let Some(aba) = self.indice(sheet) else { return false };
        let vista = &self.abas[aba];
        let raw = match vista.cells.get(&Sheet::cell_key(row, col)) {
            Some(c) => c.raw.clone(),
            None => vista.formulas.raw_at(row, col).unwrap_or_default(),
        };
        let u = raw.to_uppercase();
        raw.starts_with('=')
            && (u.contains("SUBTOTAL(") || u.contains("AGREGAR(") || u.contains("AGGREGATE("))
    }

    fn request_async(&self, _: &str, _: (u32, u32), _: crate::formula::evaluator::AsyncTask)
        -> crate::celulas::values::Computed {
        self.falhar();
        crate::celulas::values::Computed::Scalar(Value::Error(CellError::new(ErrorCode::Value)))
    }

    /// A vista paralela não carrega estilos: a célula volta ao cálculo comum.
    fn cor_celula(&self, _: u32, _: u32, _: Option<&str>, _: bool) -> Option<String> {
        self.falhar();
        None
    }
}

/// Uma coluna com trabalho a fazer.
struct No {
    aba: usize,
    col: u32,
    linhas: Vec<u32>,
    /// Lê a si mesma em linhas anteriores: uma thread, em ordem.
    corrente: bool,
}

/// Uma fatia de trabalho para uma thread.
struct Tarefa<'n> {
    no: &'n No,
    linhas: &'n [u32],
}

/// Monta os nós e a ordem. Devolve os níveis (índices em `nos`).
/// Colunas cujo valor mora no `RefCell` da aba: as das fórmulas avulsas e as
/// cobertas por matriz derramada.
fn avulsas_por_coluna(sheet: &Sheet) -> HashMap<u32, Vec<u32>> {
    let mut m: HashMap<u32, Vec<u32>> = HashMap::new();
    for &k in sheet.cells.keys() {
        let (r, c) = Sheet::key_to_coord(k);
        m.entry(c).or_default().push(r);
    }
    for v in m.values_mut() {
        v.sort_unstable();
    }
    m
}

fn planejar(abas: &[AbaViva], externas: &[(String, &Sheet)], prontas: &[MapaChave<Value>]) -> (Vec<No>, Vec<Vec<usize>>) {
    let mut nos = Vec::new();
    let mut indice: HashMap<(usize, u32), usize> = HashMap::new();
    for (a, aba) in abas.iter().enumerate() {
        for col in aba.sheet.formulas.colunas() {
            let linhas = aba.sheet.formulas.linhas_sujas(col);
            if !linhas.is_empty() {
                indice.insert((a, col), nos.len());
                nos.push(No { aba: a, col, linhas, corrente: false });
            }
        }
    }
    let achar_aba = |nome: &str| abas.iter().position(|x| mesmo_nome(&x.sheet.name, nome));
    // Aba de outro arquivo com valores carregados: só valores, nenhuma coluna para esperar.
    let externa = |nome: &str| externas.iter().any(|(n, _)| mesmo_nome(n, nome));
    // Linhas por coluna onde o valor mora no `RefCell` (avulsas e derramadas).
    // Por linha, não por coluna: um cabeçalho avulso na linha 8 não tira do
    // paralelo a coluna de 50 mil linhas abaixo dele.
    let mut avulsas: Vec<HashMap<u32, Vec<(u32, u32)>>> = vec![HashMap::new(); abas.len()];
    for (a, aba) in abas.iter().enumerate() {
        for &k in aba.sheet.cells.keys().filter(|k| !prontas[a].contains_key(k)) {
            let (r, c) = Sheet::key_to_coord(k);
            avulsas[a].entry(c).or_default().push((r, r));
        }
        for d in aba.sheet.areas_derramadas() {
            for c in d.c0..=d.c1 {
                avulsas[a].entry(c).or_default().push((d.r0, d.r1));
            }
        }
    }
    let avulsa_em = |aba: usize, c0: u32, c1: u32, r0: u32, r1: u32| {
        avulsas[aba].iter().any(|(&c, rs)| c >= c0 && c <= c1 && rs.iter().any(|&(a, b)| a <= r1 && b >= r0))
    };
    // Linhas que a referência lê, para as linhas `lo..=hi` da coluna.
    let linhas_lidas = |lock: bool, v: i64, lo: u32, hi: u32| -> (u32, u32) {
        let f = |x: i64| x.clamp(0, u32::MAX as i64) as u32;
        if lock { (f(v), f(v)) } else { (f(lo as i64 + v), f(hi as i64 + v)) }
    };

    // Só espera outra coluna quem lê alguma linha suja dela: os totais da linha 2
    // leem os dados, e os dados leem a linha deles nas colunas dos totais.
    let sujas: Vec<Vec<u32>> = nos.iter().map(|n| n.linhas.clone()).collect();
    let toca = |d: usize, r0: u32, r1: u32| {
        let l = &sujas[d];
        l.get(l.partition_point(|&r| r < r0)).is_some_and(|&r| r <= r1)
    };
    // Dependências e quem não pode ir em paralelo.
    let mut deps: Vec<HashSet<usize>> = vec![HashSet::new(); nos.len()];
    let mut fora = vec![false; nos.len()];
    for (i, no) in nos.iter_mut().enumerate() {
        let sheet = &abas[no.aba].sheet;
        let usos = sheet.formulas.columns_in_use();
        let Some(uso) = usos.iter().find(|u| u.col == no.col) else { continue };
        let (lo, hi) = (no.linhas[0], no.linhas[no.linhas.len() - 1]);
        // Pontas de faixa qualificada: a coluna depende de tudo entre elas.
        // (c0, c1, r0, r1): colunas e linhas lidas.
        let mut faixas: HashMap<usize, (u32, u32, u32, u32)> = HashMap::new();
        for padrao in &uso.usos {
            // Intervalo local: a coluna depende de todas as colunas dele.
            // Passar pela própria coluna nem vira empacotada (`FormulaTable`).
            for f in &padrao.faixas {
                let Some((c0, c1)) = f.colunas_em(no.col) else { fora[i] = true; continue };
                if c0 <= no.col && no.col <= c1 {
                    fora[i] = true;
                    continue;
                }
                // Faixa que sai por cima na primeira linha lê desde a linha 1.
                let (a, b) = (f.rect_em(lo, no.col), f.rect_em(hi, no.col));
                let r0 = a.map_or(0, |x| x.0.min(b.map_or(x.0, |y| y.0)));
                let r1 = b.map_or(u32::MAX, |x| x.2).max(a.map_or(0, |x| x.2));
                let e = faixas.entry(no.aba).or_insert((c0, c1, r0, r1));
                e.0 = e.0.min(c0);
                e.1 = e.1.max(c1);
                e.2 = e.2.min(r0);
                e.3 = e.3.max(r1);
            }
            for r in &padrao.refs {
                if r.sheet.as_deref().is_some_and(|n| achar_aba(n).is_none() && externa(n)) {
                    continue;
                }
                let alvo = match &r.sheet {
                    None => Some(no.aba),
                    Some(n) => achar_aba(n),
                };
                let Some(alvo) = alvo else { fora[i] = true; continue };
                let c = if r.col_lock { r.col } else { no.col as i32 + r.col };
                if c < 0 {
                    fora[i] = true;
                    continue;
                }
                let c = c as u32;
                let (r0, r1) = linhas_lidas(r.row_lock, r.row as i64, lo, hi);
                if alvo == no.aba && c == no.col {
                    // Linha fixa fora das que esta coluna recalcula: já está limpa.
                    if r.row_lock && no.linhas.binary_search(&r0).is_err() {
                        continue;
                    }
                    if r.row_lock || r.row >= 0 {
                        fora[i] = true;
                    } else {
                        no.corrente = true;
                    }
                    continue;
                }
                if r.sheet.is_some() {
                    let f = faixas.entry(alvo).or_insert((c, c, r0, r1));
                    f.0 = f.0.min(c);
                    f.1 = f.1.max(c);
                    f.2 = f.2.min(r0);
                    f.3 = f.3.max(r1);
                } else {
                    if avulsa_em(alvo, c, c, r0, r1) {
                        fora[i] = true;
                    }
                    if let Some(&d) = indice.get(&(alvo, c)).filter(|&&d| toca(d, r0, r1)) {
                        deps[i].insert(d);
                    }
                }
            }
        }
        for (alvo, (c0, c1, r0, r1)) in faixas {
            if avulsa_em(alvo, c0, c1, r0, r1) {
                fora[i] = true;
            }
            for c in c0..=c1 {
                if let Some(&d) = indice.get(&(alvo, c)).filter(|&&d| toca(d, r0, r1)) {
                    if d != i {
                        deps[i].insert(d);
                    }
                }
            }
        }
    }

    // Kahn: nível a nível. Quem depende de um nó de fora, fica de fora.
    let mut nivel_de = vec![usize::MAX; nos.len()];
    let mut niveis: Vec<Vec<usize>> = Vec::new();
    let mut feitos = 0;
    loop {
        let mut agora = Vec::new();
        for i in 0..nos.len() {
            if nivel_de[i] != usize::MAX || fora[i] {
                continue;
            }
            if deps[i].iter().any(|&d| fora[d]) {
                fora[i] = true;
                continue;
            }
            if deps[i].iter().all(|&d| nivel_de[d] < niveis.len()) {
                agora.push(i);
            }
        }
        if agora.is_empty() {
            break;
        }
        for &i in &agora {
            nivel_de[i] = niveis.len();
        }
        feitos += agora.len();
        niveis.push(agora);
        if feitos == nos.len() {
            break;
        }
    }
    (nos, niveis)
}

/// Calcula numa thread as linhas de uma tarefa.
fn executar(
    tarefa: &Tarefa<'_>,
    vistas: &[Vista<'_>],
    funcoes: &HashSet<String>,
    faixas: &Faixas,
    modelos: &mut HashMap<(usize, String), Option<ModeloPadrao>>,
) -> Vec<(u32, Value)> {
    let no = tarefa.no;
    let vista = &vistas[no.aba];
    let corrente = no.corrente.then(|| Corrente {
        aba: no.aba,
        col: no.col,
        primeira: tarefa.linhas[0],
        valores: RefCell::new(vec![None; (tarefa.linhas[tarefa.linhas.len() - 1] - tarefa.linhas[0] + 1) as usize]),
    });
    let ctx = Contexto {
        abas: vistas,
        atual: no.aba,
        corrente: corrente.as_ref(),
        funcoes_externas: funcoes,
        faixas,
        lidas: RefCell::default(),
        falhou: Cell::new(false),
    };
    let mut saida = Vec::with_capacity(tarefa.linhas.len());
    // O Parar e o andamento valem por linha: uma fatia de fórmula lenta (o
    // MÍNIMOSES de critério vazio varre a coluna inteira) não terminava nunca,
    // e o aviso ficava em "0 de 13,8 milhões" sem conseguir parar (28/10/2026).
    let mut desde_o_aviso = 0u64;
    for &row in tarefa.linhas {
        desde_o_aviso += 1;
        if desde_o_aviso == 256 {
            crate::paralelismo::andamento::somar(desde_o_aviso);
            desde_o_aviso = 0;
            if crate::paralelismo::andamento::parado() { break; }
        }
        let Some(arvore) = arvore(vista.formulas, no.aba, row, no.col, modelos) else { continue };
        ctx.falhou.set(false);
        let valor = match arvore {
            Ok(node) => Evaluator::na_celula(&ctx, row, no.col).eval(&node),
            Err(e) => Value::Error(CellError::com_motivo(ErrorCode::Name, e)),
        };
        if ctx.falhou.get() {
            continue;
        }
        if let Some(c) = &corrente {
            c.valores.borrow_mut()[(row - c.primeira) as usize] = Some(valor.clone());
        }
        saida.push((row, valor));
    }
    crate::paralelismo::andamento::somar(desde_o_aviso);
    saida
}

/// A árvore da fórmula da célula, pelo modelo do padrão (montado uma vez por
/// thread) ou analisando o texto — o mesmo caminho de `arvore_empacotada`.
fn arvore(
    formulas: &FormulaTable,
    aba: usize,
    row: u32,
    col: u32,
    modelos: &mut HashMap<(usize, String), Option<ModeloPadrao>>,
) -> Option<Result<Node, String>> {
    let (_, padrao) = formulas.padrao_em(row, col)?;
    let modelo = modelos
        .entry((aba, padrao.to_string()))
        .or_insert_with(|| ModeloPadrao::montar(padrao, row, col));
    if let Some(a) = modelo.as_ref().and_then(|m| m.arvore_em(row, col)) {
        return Some(Ok(a));
    }
    let raw = formulas.raw_at(row, col)?;
    Some(parse_formula(raw.strip_prefix('=').unwrap_or(&raw)).map_err(|e| e.message))
}

/// Calcula em paralelo o que dá das fórmulas empacotadas da pasta. Devolve
/// quantas células ficaram calculadas.
pub(crate) fn calcular(abas: &mut [AbaViva], funcoes: &HashSet<String>, externas: &[(String, &Sheet)]) -> usize {
    for aba in abas.iter_mut() {
        aba.sheet.preparar_calculo_paralelo();
    }
    let inicio = std::time::Instant::now();
    let prontas: Vec<MapaChave<Value>> = abas.iter().map(|a| a.sheet.avulsas_prontas()).collect();
    let prontas_externas: Vec<MapaChave<Value>> = externas.iter().map(|(_, s)| s.avulsas_prontas()).collect();
    let (nos, niveis) = planejar(abas, externas, &prontas);
    let threads = crate::paralelismo::nucleos().max(1);
    let fora: usize = {
        let nos_planejados: HashSet<usize> = niveis.iter().flatten().copied().collect();
        (0..nos.len()).filter(|i| !nos_planejados.contains(i)).map(|i| nos[i].linhas.len()).sum()
    };
    let n_niveis = niveis.len();
    crate::paralelismo::andamento::comecar(niveis.iter().flatten().map(|&i| nos[i].linhas.len() as u64).sum());
    let mut total = 0;
    for nivel in niveis {
        if crate::paralelismo::andamento::parado() { break; }
        let inicio_nivel = std::time::Instant::now();
        let colunas = nivel.len();
        let mut tempo_por_coluna: HashMap<(usize, u32), std::time::Duration> = HashMap::new();
        // Lotes dentro do nível, para o pico de resultados ter teto.
        let mut lote: Vec<usize> = Vec::new();
        let mut linhas_lote = 0;
        let mut lotes = Vec::new();
        for i in nivel {
            linhas_lote += nos[i].linhas.len();
            lote.push(i);
            if linhas_lote >= LINHAS_POR_LOTE {
                lotes.push(std::mem::take(&mut lote));
                linhas_lote = 0;
            }
        }
        if !lote.is_empty() {
            lotes.push(lote);
        }
        for lote in lotes {
            let resultados = {
                // As abas de outro arquivo vêm depois das da pasta: os índices das da pasta não mudam.
                let vistas: Vec<Vista<'_>> = abas.iter().zip(&prontas).map(|(a, p)| Vista::nova(&a.sheet, p))
                    .chain(externas.iter().zip(&prontas_externas).map(|((nome, s), p)| Vista::com_nome(s, nome, p))).collect();
                let linhas: usize = lote.iter().map(|&i| nos[i].linhas.len()).sum();
                let fatia = (linhas / (threads * TAREFAS_POR_THREAD)).max(1);
                let mut tarefas = Vec::new();
                for &i in &lote {
                    let no = &nos[i];
                    if no.corrente {
                        tarefas.push(Tarefa { no, linhas: &no.linhas });
                    } else {
                        tarefas.extend(no.linhas.chunks(fatia).map(|linhas| Tarefa { no, linhas }));
                    }
                }
                // A corrente é a mais longa: começa primeiro.
                tarefas.sort_by_key(|t| std::cmp::Reverse(t.no.corrente));
                let proxima = AtomicUsize::new(0);
                let faixas = Faixas::default();
                let n = threads.min(tarefas.len());
                std::thread::scope(|escopo| {
                    let alcas: Vec<_> = (0..n).map(|_| escopo.spawn(|| {
                        let mut modelos = HashMap::new();
                        let mut feitas = Vec::new();
                        loop {
                            if crate::paralelismo::andamento::parado() { break; }
                            let t = proxima.fetch_add(1, Ordering::Relaxed);
                            let Some(tarefa) = tarefas.get(t) else { break };
                            let t0 = std::time::Instant::now();
                            let valores = executar(tarefa, &vistas, funcoes, &faixas, &mut modelos);
                            feitas.push((tarefa.no.aba, tarefa.no.col, valores, t0.elapsed()));
                        }
                        feitas
                    })).collect();
                    alcas.into_iter().flat_map(|a| a.join().expect("thread de cálculo")).collect::<Vec<_>>()
                })
            };
            for (aba, col, valores, gasto) in resultados {
                *tempo_por_coluna.entry((aba, col)).or_default() += gasto;
                let sheet = &mut abas[aba].sheet;
                total += valores.len();
                for (row, valor) in valores {
                    sheet.formulas.store(row, col, &valor, &mut sheet.pool);
                }
            }
        }
        crate::anotar!(
            "calcular em paralelo: nível com {colunas} coluna(s) em {} ms ({})",
            inicio_nivel.elapsed().as_millis(),
            tempo_por_coluna.iter()
                .map(|((a, c), d)| format!("{}!{}: {} ms",
                    abas[*a].sheet.name, crate::celulas::refs::col_name(*c), d.as_millis()))
                .collect::<Vec<_>>().join(", "));
    }
    let antes_de_devolver = std::time::Instant::now();
    crate::paralelismo::devolver_memoria_ao_sistema();
    crate::anotar!("calcular em paralelo: devolver memória ao sistema em {} ms",
        antes_de_devolver.elapsed().as_millis());
    crate::anotar!(
        "calcular em paralelo: {total} célula(s), {n_niveis} nível(is), {threads} thread(s), \
         {fora} ficaram para o cálculo sob demanda · {} ms",
        inicio.elapsed().as_millis());
    total
}
