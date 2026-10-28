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
use crate::celulas::refs::{key_col, Rect};
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
    /// Colunas com fórmula avulsa: o valor delas mora no `RefCell` da aba.
    colunas_avulsas: HashSet<u32>,
    /// O retângulo usado (`None`: aba vazia) — a faixa não passa dele.
    usado: Option<Rect>,
    /// Áreas derramadas por matriz dinâmica: o valor mora no `RefCell` da aba.
    derramadas: Vec<Rect>,
}

impl<'a> Vista<'a> {
    fn nova(sheet: &'a Sheet) -> Self {
        Vista {
            nome: &sheet.name,
            data: &sheet.data,
            formulas: &sheet.formulas,
            pool: &sheet.pool,
            cells: &sheet.cells,
            row_heights: &sheet.row_heights,
            linhas_ocultas_desde: sheet.linhas_ocultas_desde,
            topicos: &sheet.topicos.linhas,
            colunas_avulsas: colunas_avulsas_de(sheet),
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
        if vista.cells.contains_key(&Sheet::cell_key(row, col))
            || vista.derramadas.iter().any(|a| row >= a.r0 && row <= a.r1 && col >= a.c0 && col <= a.c1)
        {
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
        if vista.derramadas.iter().any(cruza)
            || (vista.colunas_avulsas.iter().any(|&c| c >= rect.c0 && c <= rect.c1)
                && vista.cells.keys().any(|&k| {
                    let (r, c) = Sheet::key_to_coord(k);
                    dentro(r, c)
                }))
        {
            self.falhar();
            return;
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
        let celula = self.faixas.agregados.lock().expect("agregados").entry(chave).or_default().clone();
        celula.get_or_init(|| {
            let lidas: Vec<Arc<FaixaLida>> = criterios.iter().map(|r| self.faixa_lida(r)).collect::<Option<_>>()?;
            let somada = match soma {
                Some(r) => Some((r, self.faixa_lida(r)?)),
                None => None,
            };
            let sem_pool = |_: i32| "";
            let leitor = std::cell::RefCell::new(crate::planilha::agregado::LeitorChaves::novo(&sem_pool));
            Agregado::montar(criterios.len(), soma.is_some(), linhas, colunas,
                |i, r, c| leitor.borrow_mut().da_celula(lidas[i].valor(criterios[i].r0 + r, criterios[i].c0 + c)),
                |r, c| match somada.as_ref().map(|(rect, f)| f.valor(rect.r0 + r, rect.c0 + c)) {
                    Some(Value::Number(n)) => Some(*n),
                    _ => None,
                },
                || std::mem::take(&mut leitor.borrow_mut().chaves),
            ).map(Arc::new)
        }).clone()
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
fn colunas_avulsas_de(sheet: &Sheet) -> HashSet<u32> {
    let mut colunas: HashSet<u32> = sheet.cells.keys().map(|&k| key_col(k)).collect();
    for a in sheet.areas_derramadas() {
        colunas.extend(a.c0..=a.c1);
    }
    colunas
}

fn planejar(abas: &[AbaViva]) -> (Vec<No>, Vec<Vec<usize>>) {
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
    let colunas_avulsas: Vec<HashSet<u32>> = abas.iter()
        .map(|a| colunas_avulsas_de(&a.sheet))
        .collect();

    // Dependências e quem não pode ir em paralelo.
    let mut deps: Vec<HashSet<usize>> = vec![HashSet::new(); nos.len()];
    let mut fora = vec![false; nos.len()];
    for (i, no) in nos.iter_mut().enumerate() {
        let sheet = &abas[no.aba].sheet;
        let usos = sheet.formulas.columns_in_use();
        let Some(uso) = usos.iter().find(|u| u.col == no.col) else { continue };
        // Pontas de faixa qualificada: a coluna depende de tudo entre elas.
        let mut faixas: HashMap<usize, (u32, u32)> = HashMap::new();
        for padrao in &uso.usos {
            // Intervalo local: a coluna depende de todas as colunas dele.
            // Passar pela própria coluna nem vira empacotada (`FormulaTable`).
            for f in &padrao.faixas {
                let Some((c0, c1)) = f.colunas_em(no.col) else { fora[i] = true; continue };
                if c0 <= no.col && no.col <= c1 {
                    fora[i] = true;
                    continue;
                }
                let e = faixas.entry(no.aba).or_insert((c0, c1));
                e.0 = e.0.min(c0);
                e.1 = e.1.max(c1);
            }
            for r in &padrao.refs {
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
                if alvo == no.aba && c == no.col {
                    if r.row_lock || r.row >= 0 {
                        fora[i] = true;
                    } else {
                        no.corrente = true;
                    }
                    continue;
                }
                if r.sheet.is_some() {
                    let f = faixas.entry(alvo).or_insert((c, c));
                    f.0 = f.0.min(c);
                    f.1 = f.1.max(c);
                } else {
                    if colunas_avulsas[alvo].contains(&c) {
                        fora[i] = true;
                    }
                    if let Some(&d) = indice.get(&(alvo, c)) {
                        deps[i].insert(d);
                    }
                }
            }
        }
        for (alvo, (c0, c1)) in faixas {
            if colunas_avulsas[alvo].iter().any(|&c| c >= c0 && c <= c1) {
                fora[i] = true;
            }
            for c in c0..=c1 {
                if let Some(&d) = indice.get(&(alvo, c)) {
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
    for &row in tarefa.linhas {
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
pub(crate) fn calcular(abas: &mut [AbaViva], funcoes: &HashSet<String>) -> usize {
    for aba in abas.iter_mut() {
        aba.sheet.preparar_calculo_paralelo();
    }
    let inicio = std::time::Instant::now();
    let (nos, niveis) = planejar(abas);
    let threads = crate::paralelismo::nucleos().max(1);
    let fora: usize = {
        let nos_planejados: HashSet<usize> = niveis.iter().flatten().copied().collect();
        (0..nos.len()).filter(|i| !nos_planejados.contains(i)).map(|i| nos[i].linhas.len()).sum()
    };
    let n_niveis = niveis.len();
    crate::paralelismo::andamento::comecar(niveis.iter().flatten().map(|&i| nos[i].linhas.len() as u64).sum());
    let mut total = 0;
    for nivel in niveis {
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
                let vistas: Vec<Vista<'_>> = abas.iter().map(|a| Vista::nova(&a.sheet)).collect();
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
                            let t = proxima.fetch_add(1, Ordering::Relaxed);
                            let Some(tarefa) = tarefas.get(t) else { break };
                            let t0 = std::time::Instant::now();
                            let valores = executar(tarefa, &vistas, funcoes, &faixas, &mut modelos);
                            crate::paralelismo::andamento::somar(tarefa.linhas.len() as u64);
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
