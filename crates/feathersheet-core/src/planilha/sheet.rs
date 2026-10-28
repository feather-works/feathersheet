//! Armazenamento nativo em Rust da matriz de células da planilha.
//!
//! Substitui o `src/core/sheet.ts` e `data-table.ts` em TypeScript.
//! Toda a estrutura de células e valores reside aqui no motor nativo em Rust,
//! eliminando as milhões de alocações de objetos JS no Heap do V8.
//!
//! ## A decisão de arquitetura, resolvida (21/09/2026, à noite)
//!
//! Até aqui este arquivo guardava todo literal duas vezes: um `CellData`
//! completo (raw + valor + estilo) por célula em `cells` (`HashMap<u64,
//! CellData>`) **e**, quando dava para virar número, o mesmo valor de novo
//! em `data_table` (`HashMap<u64, f64>`) — sem texto, sem booleano, sem o
//! desenho colunar por bloco que [`crate::planilha::column_store`] existe para dar.
//! Era exatamente o conflito que o topo deste arquivo registrava em aberto
//! contra [`crate::planilha::data_table`] + [`crate::planilha::column_store`] + [`crate::celulas::pool`]
//! — duas tentativas independentes da mesma etapa, e nenhuma delas a
//! definitiva.
//!
//! Decisão do usuário: trocar para [`crate::planilha::data_table::DataTable`]. Literal
//! (número, texto **e** booleano, os três — o `HashMap<u64, f64>` antigo só
//! tinha o primeiro) agora mora só em [`Sheet::data`], colunar e sem cópia
//! dupla; **fórmula continua em `cells`**, como objeto no mapa, porque ainda
//! não existe recálculo nem grafo de dependência ligados a esta `Sheet` —
//! rotear fórmula por [`crate::formula::formula_table::FormulaTable`] agora seria
//! adiantar uma peça (o `check` que decide se um padrão pode virar coluna
//! depende de saber quais funções derramam matriz, o `DYNAMIC_FUNCTIONS` do
//! `functions.ts`, que ainda não tem par em Rust) sem ter como testar que o
//! encaixe está certo. Fica para quando o recálculo mesmo entrar.
//!
//! O estilo por célula, que morava dentro do `CellData`, ganhou um lugar
//! próprio (`styles`) — célula literal não tem mais `CellData` nenhum para
//! guardá-lo dentro.
//!
//! `ops.rs` e `pivot.rs` foram os dois arquivos que nasceram em cima do
//! `HashMap<u64, f64>` antigo (ver `docs/PLANO-BACKEND.md`). `pivot.rs`, na
//! prática, nunca tocou `Sheet` — opera sobre `Vec<Vec<Value>>` cru, quem
//! chama é que extrai os dados antes. Só `ops.rs::find_all` acessava
//! `sheet.cells`/`sheet.data_table` diretamente (para bater célula por
//! célula numa busca); a adaptação para `sheet.data` está lá, mesmo
//! arquivo, mesma função.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::formula::ast::Node;
use crate::planilha::data_table::{Cru, DataTable};
use crate::planilha::textos_crus::TextosCrus;
use crate::formula::evaluator::{EvalContext, Evaluator};
use crate::formula::formula_table::FormulaTable;
use crate::formula::parser::parse_formula;
use crate::celulas::pool::StringPool;
use crate::celulas::refs::{key, key_col, key_row, CellKey, Rect, MAX_COLS, MAX_ROWS};
use crate::planilha::sheet_deps::{is_volatile, DependencyGraph};
use crate::celulas::values::{CellError, ErrorCode, Value};
use crate::celulas::estilo_parcial::EstiloParcial;

pub mod edicao;
pub mod carga;
pub mod duplicar;
pub mod navegacao_macros;
pub mod derramamento;
pub mod auditoria;
pub mod nomes_selecao;
pub mod topicos;
pub mod protecao;
pub mod comentarios;
/// Sujar só as fórmulas que leem uma área de outro arquivo.
mod vinculo_externo;

/// Corrente funda de dependências (saldo acumulado de 1 milhão de linhas:
/// `K3=K2+I3`, `K4=K3+I4`…). O cálculo sob demanda é recursivo: pedir
/// K1000001 pediria K1000000, que pede K999999… e a pilha estoura, derrubando
/// o programa. Aqui a recursão tem teto: passando de [`corrente::TETO`]
/// níveis, a célula que faltou é anotada e o cálculo volta sem guardar nada
/// pela metade; na superfície ela é calculada primeiro e o pedido é refeito.
/// Cada rodada avança `TETO` níveis, e o total continua linear.
mod corrente {
    use std::cell::Cell;

    /// Níveis de fórmula aninhada por rodada. Em compilação de depuração cada
    /// nível ocupa vários KiB de pilha: 1.000 já estourava uma thread de
    /// 2 MiB (padrão do `spawn_blocking` do Tokio); 200 cabe com folga. Ver
    /// `tests/corrente_funda.rs`.
    pub(super) const TETO: u32 = 200;

    thread_local! {
        static PROFUNDIDADE: Cell<u32> = const { Cell::new(0) };
        /// A primeira célula que passou do teto nesta rodada:
        /// (endereço da aba, linha, coluna).
        static ADIADA: Cell<Option<(usize, u32, u32)>> = const { Cell::new(None) };
        /// Desliga o teto (célula adiada noutra aba, que daqui não se alcança).
        static SEM_TETO: Cell<bool> = const { Cell::new(false) };
        static SUPERFICIES: Cell<u32> = const { Cell::new(0) };
    }

    pub(super) fn profundidade() -> u32 { PROFUNDIDADE.with(Cell::get) }
    pub(super) fn entrar() { PROFUNDIDADE.with(|p| p.set(p.get() + 1)); }
    pub(super) fn sair() { PROFUNDIDADE.with(|p| p.set(p.get().saturating_sub(1))); }
    /// Há uma rodada sendo desfeita: nada calculado agora pode ir para o cache.
    pub(super) fn voltando() -> bool { ADIADA.with(Cell::get).is_some() }
    pub(super) fn adiar(aba: usize, row: u32, col: u32) {
        ADIADA.with(|a| if a.get().is_none() { a.set(Some((aba, row, col))) });
    }
    pub(super) fn tomar_adiada() -> Option<(usize, u32, u32)> { ADIADA.with(Cell::take) }
    pub(super) fn sem_teto() -> bool { SEM_TETO.with(Cell::get) }
    /// Superfícies de outras abas abertas umas dentro das outras.
    pub(super) fn superficies() -> u32 { SUPERFICIES.with(Cell::get) }
    pub(super) fn entrar_superficie() { SUPERFICIES.with(|p| p.set(p.get() + 1)); }
    pub(super) fn sair_superficie() { SUPERFICIES.with(|p| p.set(p.get().saturating_sub(1))); }
    pub(super) fn definir_sem_teto(v: bool) { SEM_TETO.with(|s| s.set(v)); }
}

/// Resposta de [`Sheet::cru_rapido`].
enum Rapido {
    /// O cru e se veio da coluna de literais (`true`) ou de fórmula.
    Cru(Cru, bool),
    Vazia,
    Lento,
}

#[derive(Debug, Clone)]
pub struct CellData {
    pub raw: String,
    pub formula: Option<Node>,
    pub value: Value,
    pub style_idx: Option<usize>,
}

/// Cache do valor **calculado** de cada célula com fórmula — recálculo
/// preguiçoso, sob demanda: `get_value` só reavalia o que está em `dirty`
/// (ou nunca foi calculado), e guarda o resultado aqui até a próxima
/// mudança. Fica separado de `CellData::value` (que continua guardando só o
/// erro de compilação, quando a fórmula nem chegou a virar árvore) para não
/// precisar mudar `cells` para `RefCell` — só isto aqui precisa de mutação
/// por trás de `&self`, porque `EvalContext::get_value` é `&self` (ver o
/// comentário no topo de `evaluator.rs`: "quem implementa guarda o que
/// precisa mudar atrás de um `RefCell`").
#[derive(Default)]
struct CalcCache {
    /// Último valor calculado de cada fórmula. Hash barato
    /// ([`crate::celulas::mapa_chave`]): a corrente de 1 milhão passa por
    /// estes três mapas várias vezes por célula.
    cache: crate::celulas::mapa_chave::MapaChave<Value>,
    /// Fórmulas que precisam recalcular antes do próximo `get_value` confiar
    /// no `cache`.
    dirty: crate::celulas::mapa_chave::ConjuntoChave,
    /// Pilha de quem está sendo avaliado agora, nesta cadeia de chamadas —
    /// achar `k` aqui de novo é referência circular.
    evaluating: crate::celulas::mapa_chave::ConjuntoChave,
}

// `Debug`/`Clone` saíram do derive: `DataTable` e `StringPool` não os têm
// (ChunkedColumn guarda `Box<[T]>` em blocos, e clonar isso célula por célula
// não é grátis) e nada no crate hoje pede `Sheet: Debug`/`Sheet: Clone` — os
// `.clone()` que aparecem perto de "sheet" em `evaluator.rs`/`fn_*.rs` são
// do campo `sheet: Option<String>` de `Rect`, não desta struct.
pub(crate) fn pattern_check(raw: &str) -> bool {
    if is_volatile(raw) {
        return false;
    }
    let stripped = raw.strip_prefix('=').unwrap_or(raw);
    let ast = match parse_formula(stripped) {
        Ok(ast) => ast,
        Err(_) => return false,
    };
    if crate::planilha::sheet_spill::maybe_matrix(&ast) {
        return false;
    }
    let mut precisa_do_grafo = false;
    let mut qualificadas = 0;
    ast.walk(&mut |n| match n {
        // Intervalo local (`AO:AO`, `A2:A10`) mora na coluna empacotada desde
        // 17/10/2026 (`FaixaPadrao`): `MÍNIMOSES(AO:AO;A:A;A2;…)` em 13,7
        // milhões de linhas não cabia no caminho de uma fórmula por célula.
        Node::Name(_) => precisa_do_grafo = true,
        // Outro arquivo (`[Vendas.fshz]Dados!A1`): fica avulsa, onde a
        // atualização que chega de fora acha quem lê cada célula
        // (`vinculo_externo.rs`) e onde o cálculo em paralelo não vai.
        Node::Ref { sheet: Some(s), .. } | Node::Range { sheet: Some(s), .. }
            if crate::planilha::vinculo::e_externa(s) => precisa_do_grafo = true,
        Node::Ref { sheet: Some(_), .. } => qualificadas += 1,
        Node::Range { sheet: Some(_), .. } => qualificadas += 2,
        _ => {},
    });
    if precisa_do_grafo { return false; }
    if qualificadas == 0 { return true; }
    // A:A / 1:1 ainda não têm marcadores no padrão textual. Continuam no
    // grafo avulso, para não perder a invalidação da aba consultada.
    use crate::formula::formula_pattern::{pattern_for, refs_of_pattern};
    pattern_for(raw, 0, 0).and_then(|p| refs_of_pattern(&p))
        .is_some_and(|refs| refs.iter().filter(|r| r.sheet.is_some()).count() == qualificadas)
}

#[derive(Default)]
pub struct Sheet {
    pub name: String,
    pub tab_color: Option<String>,
    /// Aba oculta no rodapé; os dados e referências continuam acessíveis.
    pub oculta: bool,
    /// Só fórmula mora aqui — célula literal vai para [`Self::data`].
    pub cells: HashMap<u64, CellData>,
    /// Literal (número, texto, booleano), colunar — ver a nota de topo do
    /// arquivo.
    pub data: DataTable,
    /// Armazenamento colunar de fórmulas que viram padrão.
    pub formulas: FormulaTable,
    /// O dicionário de texto que [`Self::data`] usa. Uma `Sheet` sozinha tem
    /// o seu; quando a `Workbook` existir (etapa de várias abas), o normal é
    /// todas as abas compartilharem um pool só, como no TypeScript
    /// (`globalStringPool`) — ver a nota de posse em `pool.rs`.
    pub pool: StringPool,
    /// Estilo de célula literal — não cabe mais dentro de [`CellData`]
    /// porque célula literal não tem `CellData` nenhum. Fórmula continua
    /// guardando o próprio `style_idx` no `CellData` de `cells`.
    pub styles: crate::celulas::estilos_celula::EstilosCelula,
    /// A tabela de estilos da aba. O `style_idx` das células aponta para cá —
    /// até 22/09/2026 ele era um índice para lugar nenhum, o que deixava o
    /// quadro binário sem como formatar moeda, data ou porcentagem.
    ///
    /// Repetido não entra: uma coluna inteira em negrito é UM estilo, não um
    /// por célula. É o mesmo desenho do `pool` de texto.
    pub style_table: Vec<crate::celulas::style::Style>,
    pub col_widths: HashMap<u32, u32>,
    /// Largura das colunas sem largura própria (Formatar ▾ → Largura padrão),
    /// em pixels; `None` = a da grade.
    pub largura_padrao: Option<u32>,
    pub row_heights: HashMap<u32, u32>,
    /// Linhas ocultas da `n` até o fim da planilha ("ocultar da 30 até a
    /// última", 16/10/2026): um número só, sem guardar 100 milhões de
    /// alturas. `None` = nenhuma. Vale junto com as alturas 0 do mapa.
    pub linhas_ocultas_desde: Option<u32>,
    /// O mesmo para as colunas.
    pub colunas_ocultas_desde: Option<u32>,
    /// Estrutura de tópicos (Dados → Agrupar). Ver [`topicos`].
    pub topicos: topicos::Topicos,
    /// Proteger planilha (Formatar ▾). Ver [`protecao`].
    pub protecao: Option<protecao::ProtecaoPlanilha>,
    /// Comentários das células (aba Revisão). Ver [`comentarios`].
    pub comentarios: comentarios::Comentarios,
    pub min_row: u32,
    pub max_row: u32,
    pub min_col: u32,
    pub max_col: u32,
    /// Blocos opacos de metadados preservados sem alteração.
    pub g: Option<String>,
    /// Intervalos protegidos por senha (só o hash; ver `ui/protecao/protecao.ts`),
    /// em JSON opaco: o Rust só os carrega de volta para o arquivo.
    pub pr: Option<String>,
    pub r: Option<String>,
    pub tb: Option<String>,
    pub oq: Option<String>,
    pub dn: Option<String>,
    pub mc: Option<String>,
    pub im: Option<String>,
    /// Regras de validação de célula (lista/número/data/texto/caixa), em
    /// JSON opaco — tradução de `Validacao`/`Regra` de `operacoes::validation`
    /// só acontece quando alguém checa um valor (`validacao_verificar`) ou
    /// reflui um inserir/excluir; o Rust guarda como veio e devolve igual.
    pub va: Option<String>,
    /// Botões de gatilho de macro (posição, rótulo, nome da macro) em JSON
    /// compatível com o TypeScript. O clique/arraste/edição fica na GUI Rust,
    /// e alterar_botoes registra seu histórico sem copiar células.
    pub bt: Option<String>,
    /// Tinta da aba Desenhar em texto opaco (âncoras nas células + traços no
    /// formato `FTINTA` do `feather-draw`). O motor só guarda e
    /// devolve; `alterar_tinta` registra o desfazer.
    pub tn: Option<String>,
    /// Objetos soltos sobre a grade (formas, ícones, SmartArt) em JSON com
    /// âncora de duas células (modelo e conversões em
    /// `feathersheet_io::objetos_aba`). O motor só guarda e devolve;
    /// `alterar_objetos` registra o desfazer.
    pub ob: Option<String>,
    /// Cenários do Gerenciador de cenários (nome, células variáveis e
    /// valores) em JSON opaco: o motor só os guarda e devolve ao arquivo.
    pub ce: Option<String>,
    /// Estilos de coluna inteira (`colStyles`), linha (`rowStyles`) e aba
    /// (`sheetStyle`). Parciais, como o TypeScript os guarda — ver
    /// [`crate::celulas::estilo_parcial`].
    pub col_styles: HashMap<u32, EstiloParcial>,
    pub row_styles: HashMap<u32, EstiloParcial>,
    pub sheet_style: Option<EstiloParcial>,
    /// O estilo parcial de cada entrada de [`Self::style_table`], no mesmo
    /// índice. O `style_table` é o resolvido (para formatar); este é o que
    /// volta para o TypeScript e para o arquivo sem perder o `bold: false`.
    pub style_parts: Vec<EstiloParcial>,
    /// O texto digitado de literal que não se reconstrói do valor: "007",
    /// "1.50", data digitada, moeda digitada. O valor mora em [`Self::data`]
    /// e o texto aqui — é o `roundTrips` do `sheet.ts`.
    pub textos: TextosCrus,
    /// Desfazer/refazer e a transação aberta. Ver `sheet/edicao.rs`.
    pub(crate) hist: edicao::Historico,
    /// As células tocadas desde a última vez que alguém perguntou — ver
    /// [`Rastro`]. `RefCell` porque a cascata de "sujo" roda com `&self`.
    pub rastro: RefCell<Rastro>,
    /// Escrita em lote ([`Self::adiar_cascata`]): as células escritas
    /// esperam aqui e sujam os dependentes numa cascata só, no fim.
    cascata_adiada: RefCell<Option<Vec<CellKey>>>,
    /// Algo saiu da borda do retângulo usado: recalcular antes de responder.
    pub(crate) bounds_dirty: bool,
    /// Fórmulas avulsas que leem outra aba ou um nome. As compactas guardam
    /// essas dependências nos padrões da `FormulaTable`. A pasta consulta
    /// ambas as fontes por `tem_formulas_externas`.
    pub(crate) externas: HashSet<CellKey>,
    /// Quem lê quem, entre fórmulas — usado para saber quem sujar quando uma
    /// célula muda. Ver a nota de [`CalcCache`].
    deps: DependencyGraph,
    calc: RefCell<CalcCache>,
    /// Sobe a cada mudança que pode alterar um valor da aba; as faixas
    /// guardadas de uma geração antiga são jogadas fora.
    geracao: std::cell::Cell<u64>,
    /// Faixas lidas inteiras pelas funções de critério. Ver
    /// [`crate::planilha::faixa_lida`].
    faixas: RefCell<crate::planilha::faixa_lida::Faixas>,
    /// Agregados por critério (`SOMASES` e família), uma passada por conjunto
    /// de faixas. Ver [`crate::planilha::agregado`].
    agregados: RefCell<crate::planilha::agregado::Agregados>,
    /// Árvore de cada padrão da coluna empacotada, analisada uma vez
    /// (`None`: o padrão não aceita modelo; analisa o texto). Ver
    /// [`crate::formula::modelo_padrao`].
    modelos: RefCell<HashMap<String, Option<crate::formula::modelo_padrao::ModeloPadrao>>>,
    /// As perguntas que respondem depois (`IMPORTARDADOS`, função de macro) e
    /// as respostas já chegadas. Ver [`crate::planilha::assincrono`].
    pub assincronos: RefCell<crate::planilha::assincrono::Assincronos>,
    /// Matrizes derramadas pelas âncoras de `deps.array_cells`. Ver
    /// [`derramamento`].
    pub(crate) derrame: RefCell<derramamento::Derrames>,
    /// Cálculo manual: a escrita não propaga o "sujo"; quem mudou espera o
    /// F9 em [`Self::adiadas`]. Ver [`crate::planilha::calculo_manual`].
    pub(crate) manual: std::cell::Cell<bool>,
    pub(crate) adiadas: RefCell<Vec<CellKey>>,
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tab_color: None,
            oculta: false,
            cells: HashMap::new(),
            data: DataTable::new(),
            formulas: FormulaTable::new(),
            pool: StringPool::new(),
            styles: Default::default(),
            style_table: Vec::new(),
            col_widths: HashMap::new(),
            largura_padrao: None,
            row_heights: HashMap::new(),
            linhas_ocultas_desde: None,
            colunas_ocultas_desde: None,
            topicos: Default::default(),
            protecao: None,
            comentarios: Default::default(),
            min_row: MAX_ROWS,
            max_row: 0,
            min_col: MAX_COLS,
            max_col: 0,
            g: None,
            pr: None,
            r: None,
            tb: None,
            oq: None,
            dn: None,
            mc: None,
            im: None,
            va: None,
            bt: None,
            tn: None,
            ob: None,
            ce: None,
            col_styles: HashMap::new(),
            row_styles: HashMap::new(),
            sheet_style: None,
            style_parts: Vec::new(),
            textos: TextosCrus::new(),
            hist: edicao::Historico::default(),
            rastro: RefCell::new(Rastro::default()),
            cascata_adiada: RefCell::new(None),
            bounds_dirty: false,
            externas: HashSet::new(),
            deps: DependencyGraph::new(),
            calc: RefCell::new(CalcCache::default()),
            geracao: std::cell::Cell::new(0),
            faixas: RefCell::new(Default::default()),
            agregados: RefCell::new(Default::default()),
            modelos: RefCell::new(HashMap::new()),
            assincronos: RefCell::new(Default::default()),
            derrame: RefCell::new(Default::default()),
            manual: std::cell::Cell::new(false),
            adiadas: RefCell::new(Vec::new()),
        }
    }

    #[inline]
    /// A linha está oculta (altura 0 ou depois de `linhas_ocultas_desde`).
    pub fn linha_oculta(&self, row: u32) -> bool {
        self.linhas_ocultas_desde.is_some_and(|n| row >= n) || self.row_heights.get(&row) == Some(&0)
    }

    /// A coluna está oculta (largura 0 ou depois de `colunas_ocultas_desde`).
    pub fn coluna_oculta(&self, col: u32) -> bool {
        self.colunas_ocultas_desde.is_some_and(|n| col >= n) || self.col_widths.get(&col) == Some(&0)
    }

    /// Há alguma linha oculta (para pular a consulta por célula quando não há).
    pub fn tem_linhas_ocultas(&self) -> bool {
        self.linhas_ocultas_desde.is_some() || self.row_heights.values().any(|&h| h == 0)
    }

    pub fn cell_key(row: u32, col: u32) -> u64 {
        key(row, col)
    }

    #[inline]
    pub fn key_to_coord(k: u64) -> (u32, u32) {
        (key_row(k), key_col(k))
    }

    pub fn set_cell_raw(&mut self, row: u32, col: u32, raw: String, style_idx: Option<usize>) {
        if row >= MAX_ROWS || col >= MAX_COLS {
            return;
        }
        self.rastro.borrow_mut().anotar(key(row, col));

        let k = Self::cell_key(row, col);
        let trimmed = raw.trim();

        // Desfaz o que este `k` dependia antes de decidir o que ele vira
        // agora — precisa vir antes de qualquer caminho que devolva cedo,
        // senão o grafo guarda precedente de uma fórmula que já foi embora
        // (célula apagada, ou virou literal). Se o conteúdo novo também for
        // fórmula, relinca mais abaixo.
        self.deps.unlink(k, &mut |_, _| {});
        self.formulas.clear_cell(row, col);
        self.textos.remove(&k);
        self.externas.remove(&k);

        if trimmed.is_empty() {
            self.cells.remove(&k);
            self.data.clear_cell(row, col);
            self.styles.remove(&k);
            self.mark_dirty_cascade(k);
            return;
        }

        // Atualiza limites de rect usado
        if row < self.min_row {
            self.min_row = row;
        }
        if row > self.max_row {
            self.max_row = row;
        }
        if col < self.min_col {
            self.min_col = col;
        }
        if col > self.max_col {
            self.max_col = col;
        }

        if trimmed.starts_with('=') {
            // Fórmula com estilo (moeda, %, data) também mora na coluna
            // empacotada: o estilo fica no `styles`, como o de um literal.
            // Recusar o estilo mandava quase toda planilha real para o
            // caminho de ~300 bytes por célula. A árvore vem do modelo do
            // padrão (`arvore_empacotada`), sem reanalisar o texto.
            if self.formulas.set(row, col, trimmed, pattern_check) {
                self.cells.remove(&k);
                self.data.clear_cell(row, col);
                match style_idx {
                    Some(idx) => { self.styles.insert(k, idx); }
                    None => { self.styles.remove(&k); }
                }
                self.mark_dirty_cascade(k);
                return;
            }

            // Fórmula: vai para `cells`, inteira — limpa o que a célula
            // tinha do lado literal, se tinha.
            self.data.clear_cell(row, col);
            self.styles.remove(&k);
            let stripped = &trimmed[1..];
            let (formula, value) = match parse_formula(stripped) {
                Ok(node) => {
                    // `sheet_name` copiado (não emprestado) de propósito:
                    // `is_self` precisa viajar dentro de `self.deps.link`,
                    // que já toma `&mut self.deps` — emprestar `self.name`
                    // no meio disso deixaria o borrow checker discutindo com
                    // o dono errado do empréstimo.
                    let sheet_name = self.name.to_lowercase();
                    let is_self = move |s: Option<&str>| s.map_or(true, |s| s.to_lowercase() == sheet_name);
                    self.deps.link(k, &node, &is_self, &mut |_, _| {});
                    if le_de_fora(&node, &self.name) {
                        self.externas.insert(k);
                    }
                    if is_volatile(stripped) {
                        self.deps.volatiles.insert(k);
                    } else {
                        self.deps.volatiles.remove(&k);
                    }
                    (Some(node), Value::Empty)
                }
                Err(err) => (None, Value::Error(CellError::com_motivo(ErrorCode::Name, err.message))),
            };
            self.cells.insert(k, CellData { raw, formula, value, style_idx });
            self.mark_dirty_cascade(k);
            return;
        }

        // Literal: sai de `cells` (podia ter fórmula antes) e vai para `data`.
        self.cells.remove(&k);
        let value = crate::celulas::input::parse_literal(trimmed).value;
        self.data.set_cell_value(row, col, &value, &mut self.pool);
        match style_idx {
            Some(idx) => {
                self.styles.insert(k, idx);
            }
            None => {
                self.styles.remove(&k);
            }
        }
        self.mark_dirty_cascade(k);
    }

    /// Valor da célula. Fórmula recalcula sob demanda, na hora — não a cada
    /// mudança, só quando alguém pede e o que está guardado está sujo (ver
    /// [`CalcCache`]).
    pub fn get_value(&self, row: u32, col: u32) -> Value {
        self.get_value_em(row, col, None)
    }

    /// Valor da célula, com a pasta em volta para resolver `Dados!A1` e
    /// intervalo nomeado. Sem ela ([`Self::get_value`]) essas referências dão
    /// `#REF!` — e o resultado **não** fica guardado, senão a próxima leitura
    /// com a pasta em mãos acharia o `#REF!` no cache.
    pub fn get_value_em(&self, row: u32, col: u32, host: Option<&dyn Hospedeiro>) -> Value {
        let profundidade = corrente::profundidade();
        if profundidade == 0 {
            if !self.deps.array_cells.is_empty() {
                self.garantir_derramamentos_frio(host);
            }
            return self.valor_na_superficie(row, col, host);
        }
        if profundidade >= corrente::TETO && !corrente::sem_teto() && self.precisa_calcular(row, col) {
            corrente::adiar(self as *const Self as usize, row, col);
            return Value::Empty;
        }
        corrente::entrar();
        let valor = self.valor_interno(row, col, host);
        corrente::sair();
        valor
    }

    /// Pedido de fora do avaliador: refaz o cálculo enquanto houver célula
    /// adiada pelo teto de profundidade, sempre a mais funda primeiro.
    fn valor_na_superficie(&self, row: u32, col: u32, host: Option<&dyn Hospedeiro>) -> Value {
        // Quase sempre nada fica adiado: sem a lista abaixo, que era uma
        // alocação por célula lida (a dinâmica lê milhões, 17/10/2026).
        corrente::entrar();
        let valor = self.valor_interno(row, col, host);
        corrente::sair();
        if !corrente::voltando() { return valor; }
        let mut ja_calculado = Some(valor);
        let eu = self as *const Self as usize;
        let mut pendentes = vec![(row, col)];
        loop {
            let (r, c) = *pendentes.last().expect("sempre há o pedido original");
            let valor = match ja_calculado.take() {
                Some(v) => v,
                None => {
                    corrente::entrar();
                    let v = self.valor_interno(r, c, host);
                    corrente::sair();
                    v
                }
            };
            match corrente::tomar_adiada() {
                None => {
                    pendentes.pop();
                    if pendentes.is_empty() { return valor; }
                }
                Some((aba, r2, c2)) if aba == eu && pendentes.contains(&(r2, c2)) => {
                    // A corrente voltou a uma célula que já esperava: é
                    // referência circular, só que longa demais para o
                    // conjunto `evaluating` de uma rodada enxergar.
                    let k = Self::cell_key(r2, c2);
                    let mut calc = self.calc.borrow_mut();
                    calc.cache.insert(k, Value::Error(CellError::new(ErrorCode::Circular)));
                    calc.dirty.remove(&k);
                }
                Some((aba, r2, c2)) if aba == eu => {
                    // Corrente numa coluna empacotada: em vez de descer 200
                    // níveis por rodada (e jogar fora o que calculou na
                    // volta), acha o começo sujo e calcula de cima para
                    // baixo, uma célula por vez.
                    match self.calcular_corrente(r2, c2, host) {
                        Ok(()) => {}
                        Err(adiada) => {
                            pendentes.push((r2, c2));
                            if let Some((aba3, r3, c3)) = adiada.filter(|a| a.0 == eu) {
                                let _ = aba3;
                                if !pendentes.contains(&(r3, c3)) {
                                    pendentes.push((r3, c3));
                                }
                            }
                        }
                    }
                }
                Some((aba, r2, c2)) if corrente::superficies() < 32
                    && host.and_then(|h| h.aba_no_endereco(aba)).is_some() => {
                    // Adiada noutra aba (`=Dados!K1000001` na Resumo): calcula
                    // a corrente de lá primeiro, pela superfície dela, e tenta
                    // de novo. Sem isto a pilha descia um milhão de níveis.
                    let outra = host.and_then(|h| h.aba_no_endereco(aba)).expect("conferida acima");
                    corrente::entrar_superficie();
                    let _ = outra.valor_na_superficie(r2, c2, host);
                    corrente::sair_superficie();
                }
                Some(_) => {
                    // Adiada noutra aba que daqui não se alcança (ou corrente
                    // indo e voltando entre abas): sem teto, como antes.
                    corrente::definir_sem_teto(true);
                    corrente::entrar();
                    let valor = self.valor_interno(r, c, host);
                    corrente::sair();
                    corrente::definir_sem_teto(false);
                    pendentes.pop();
                    if pendentes.is_empty() { return valor; }
                }
            }
        }
    }

    /// Calcula a corrente que termina em (`row`, `col`): o padrão lê a linha
    /// de cima na mesma coluna, e a coluna está suja dali para cima até
    /// `inicio`. Cada passo acha a célula acima já calculada, então nenhum
    /// desce fundo. `Err(adiada)`: não é corrente, ou algo no meio pediu
    /// outra célula funda (devolvida para quem chamou tratar); o que já foi
    /// calculado fica guardado.
    fn calcular_corrente(&self, row: u32, col: u32, host: Option<&dyn Hospedeiro>)
        -> Result<(), Option<(usize, u32, u32)>> {
        let Some((id, _)) = self.formulas.padrao_em(row, col) else { return Err(None) };
        if !self.formulas.padrao_le_linha_acima(id) {
            return Err(None);
        }
        let mut inicio = row;
        while inicio > 0
            && self.formulas.padrao_em(inicio - 1, col).is_some_and(|(i, _)| i == id)
            && self.precisa_calcular(inicio - 1, col) {
            inicio -= 1;
        }
        if inicio == row {
            return Err(None);
        }
        for r in inicio..=row {
            corrente::entrar();
            let _ = self.valor_interno(r, col, host);
            corrente::sair();
            if corrente::voltando() {
                return Err(corrente::tomar_adiada());
            }
            // O resultado não ficou guardado (fórmula que lê outra aba sem a
            // pasta em mãos): continuar seria calcular tudo de novo a cada
            // rodada, sem fim. O caminho antigo resolve.
            if self.precisa_calcular(r, col) {
                return Err(None);
            }
        }
        Ok(())
    }

    /// A célula espera um cálculo que pode demorar: fórmula sem valor
    /// confiável guardado. Volátil (`AGORA`, `ALEATÓRIO`) só fica suja num
    /// recálculo da pasta; a leitura devolve o valor guardado. A grade mostra "…" no lugar e
    /// pede o cálculo fora do desenho, para a janela nunca travar.
    pub fn calculo_pendente(&self, row: u32, col: u32) -> bool {
        self.precisa_calcular(row, col) && !self.deps.volatiles.contains(&Self::cell_key(row, col))
    }

    /// O último resultado guardado de uma fórmula avulsa, sem calcular nada
    /// (nem os derramamentos, que o `get_value` garante). Coluna empacotada:
    /// `formulas.cru_at`.
    pub fn valor_guardado(&self, row: u32, col: u32) -> Option<Value> {
        self.calc.borrow().cache.get(&Self::cell_key(row, col)).cloned()
    }

    /// A célula é fórmula que ainda não tem valor confiável guardado.
    fn precisa_calcular(&self, row: u32, col: u32) -> bool {
        let k = Self::cell_key(row, col);
        let calc = self.calc.borrow();
        if let Some(cell) = self.cells.get(&k) {
            return cell.formula.is_some()
                && (!calc.cache.contains_key(&k) || calc.dirty.contains(&k));
        }
        if !self.formulas.has(row, col) { return false; }
        if calc.dirty.contains(&k) { return true; }
        !(calc.cache.contains_key(&k) || self.formulas.is_clean(row, col))
    }

    fn valor_interno(&self, row: u32, col: u32, host: Option<&dyn Hospedeiro>) -> Value {
        let k = Self::cell_key(row, col);
        if let Some(cell) = self.cells.get(&k) {
            match &cell.formula {
                Some(node) => {
                    if let Some(avulso) = self.recalc_if_needed(k, node, row, col, host) {
                        return avulso;
                    }
                    self.calc.borrow().cache.get(&k).cloned().unwrap_or(Value::Empty)
                }
                // Fórmula que nem compilou: o erro já está pronto em
                // `cell.value` desde o `set_cell_raw`, não passa pelo
                // avaliador.
                None => cell.value.clone(),
            }
        } else if self.formulas.has(row, col) {
            self.recalc_formula_table_if_needed(row, col, host)
        } else {
            let valor = self.data.get_cell_value(row, col, &self.pool);
            if matches!(valor, Value::Empty) && !self.deps.array_cells.is_empty() {
                return self.vazia_ou_derramada(row, col, host);
            }
            valor
        }
    }

    /// Avalia uma árvore avulsa como se ela estivesse na célula — o `evalAt`
    /// do TypeScript. É o que faz a fórmula personalizada de uma regra de
    /// formatação condicional enxergar `LIN()` e as referências relativas do
    /// lugar certo. Fica fora do grafo de dependências: regra não é célula.
    pub fn avaliar_em(&self, row: u32, col: u32, node: &crate::formula::ast::Node, host: Option<&dyn Hospedeiro>) -> Value {
        let ctx = SheetCtx { sheet: self, host };
        Evaluator::na_celula(&ctx, row, col).eval(node)
    }

    fn recalc_formula_table_if_needed(&self, row: u32, col: u32, host: Option<&dyn Hospedeiro>) -> Value {
        let k = Self::cell_key(row, col);
        {
            let calc = self.calc.borrow();
            if !calc.dirty.contains(&k) {
                // O recalculado aqui é mais novo que o do arquivo: vale ele.
                if let Some(v) = calc.cache.get(&k) {
                    return v.clone();
                }
                // Sem recálculo e sem sujeira: o valor que veio do arquivo
                // (`FormulaTable::load_column`) ainda vale. Ignorá-lo obrigava
                // o Rust a recalcular toda fórmula na primeira leitura — e a
                // guardar cada resultado num `HashMap` à parte.
                if self.formulas.is_clean(row, col) {
                    return self.formulas.value_at(row, col, &self.pool);
                }
            }
        }
        {
            let mut calc = self.calc.borrow_mut();
            if calc.evaluating.contains(&k) {
                calc.cache.insert(k, Value::Error(CellError::new(ErrorCode::Circular)));
                calc.dirty.remove(&k);
                return Value::Error(CellError::new(ErrorCode::Circular));
            }
            calc.evaluating.insert(k);
        }

        let Some(arvore) = self.arvore_empacotada(row, col) else {
            self.calc.borrow_mut().evaluating.remove(&k);
            return Value::Empty;
        };
        let val = match arvore {
            Ok(node) => {
                let ctx = SheetCtx { sheet: self, host };
                let ev = Evaluator::na_celula(&ctx, row, col);
                ev.eval(&node)
            }
            Err(e) => Value::Error(CellError::com_motivo(ErrorCode::Name, e)),
        };

        let mut calc = self.calc.borrow_mut();
        calc.evaluating.remove(&k);
        if corrente::voltando() {
            // Resultado de uma rodada interrompida: pode estar errado.
            return Value::Empty;
        }
        // Uma consulta sem a pasta não pode guardar #REF! no lugar de um
        // resultado que será resolvido quando o hospedeiro estiver presente.
        if host.is_none() && self.formulas.le_de_fora_em(row, col) {
            return val;
        }
        calc.cache.insert(k, val.clone());
        calc.dirty.remove(&k);
        val
    }

    /// A árvore da fórmula empacotada em (`row`, `col`): do modelo do padrão
    /// quando dá, senão analisando o texto. `None`: não há fórmula ali.
    fn arvore_empacotada(&self, row: u32, col: u32) -> Option<Result<Node, String>> {
        use crate::formula::modelo_padrao::ModeloPadrao;
        let (_, padrao) = self.formulas.padrao_em(row, col)?;
        {
            let modelos = self.modelos.borrow();
            if let Some(modelo) = modelos.get(padrao) {
                if let Some(arvore) = modelo.as_ref().and_then(|m| m.arvore_em(row, col)) {
                    return Some(Ok(arvore));
                }
            } else {
                drop(modelos);
                let modelo = ModeloPadrao::montar(padrao, row, col);
                let pronta = modelo.as_ref().and_then(|m| m.arvore_em(row, col));
                self.modelos.borrow_mut().insert(padrao.to_string(), modelo);
                if let Some(arvore) = pronta {
                    return Some(Ok(arvore));
                }
            }
        }
        let raw = self.formulas.raw_at(row, col)?;
        Some(parse_formula(raw.strip_prefix('=').unwrap_or(&raw)).map_err(|e| e.message))
    }

    /// Invalida os valores calculados desta aba, como
    /// `recalculateAll()` de macros/api-aplicacao.ts.
    /// Não materializa fórmulas em memória nem altera o histórico de undo.
    /// O motor atual reavalia sob demanda quando a célula for consultada.
    /// Move até `limite` resultados de fórmulas empacotadas do mapa
    /// `calc.cache` (onde o cálculo, feito com `&self`, os deixa: ~60 bytes
    /// por célula) para dentro da própria coluna, que já tem o espaço
    /// reservado. Só o que está limpo; não muda versão nem desfazer — o
    /// valor é o mesmo, só muda de lugar. Devolve quantos moveu. A janela
    /// chama aos poucos, parada, para não travar.
    pub fn consolidar_calculos(&mut self, limite: usize) -> usize {
        let calc = self.calc.get_mut();
        if calc.cache.is_empty() || limite == 0 {
            return 0;
        }
        let (formulas, pool, dirty, evaluating) = (&mut self.formulas, &mut self.pool, &calc.dirty, &calc.evaluating);
        // `extract_if` para assim que a cota enche: o `retain` de antes
        // percorria o cache inteiro a cada chamada (13,7 milhões de entradas
        // para mover 500 mil — o colar de 13 milhões de fórmulas pesava a
        // cada segundo, 17/10/2026).
        let movidos = calc.cache.extract_if(|&k, v| {
            if dirty.contains(&k) || evaluating.contains(&k) {
                return false;
            }
            let (row, col) = Self::key_to_coord(k);
            if !formulas.has(row, col) {
                return false;
            }
            formulas.store(row, col, v, pool);
            true
        }).take(limite).count();
        if calc.cache.len() < calc.cache.capacity() / 4 {
            calc.cache.shrink_to_fit();
        }
        movidos
    }

    /// Deixa o estado do cálculo num lugar só antes do cálculo em paralelo:
    /// resultado limpo vai para a coluna, e a fórmula empacotada que está em
    /// `calc.dirty` passa a ser suja na própria coluna. Assim as threads, que
    /// não podem tocar o `RefCell`, decidem tudo olhando a `FormulaTable`.
    pub(crate) fn preparar_calculo_paralelo(&mut self) {
        while self.consolidar_calculos(usize::MAX) > 0 {}
        let calc = self.calc.get_mut();
        if calc.dirty.is_empty() {
            return;
        }
        let formulas = &mut self.formulas;
        let (dirty, cache) = (&mut calc.dirty, &mut calc.cache);
        dirty.retain(|&k| {
            let (r, c) = Self::key_to_coord(k);
            if !formulas.has(r, c) {
                return true;
            }
            formulas.mark_dirty(r, c);
            cache.remove(&k);
            false
        });
    }

    pub fn invalidar_todos_calculos(&mut self) {
        self.tocar();
        {
            let mut calc=self.calc.borrow_mut();
            calc.cache.clear();
            calc.dirty.clear();
            // `evaluating` é reservado ao avaliador ativo. A macro chama
            // este método entre avaliações, então não há pilha em curso.
        }
        // Além do HashMap legado, há fórmulas compactadas em uma
        // FormulaTable cujos valores podem ter sido carregados do FSHZ.
        self.formulas.marcar_todas_sujas();
        self.descartar_derramamentos();
    }

    /// Recalcula estas células (a resposta que elas pediram chegou) e quem
    /// depende delas.
    pub fn sujar_celulas(&self, chaves: &[u64]) {
        for &k in chaves {
            self.sujar_cascata(k);
        }
    }

    /// Recalcula as fórmulas que citam `nome` (uma função de macro acabou de
    /// ser criada ou apagada: o `#NAME?` guardado já não vale).
    pub fn sujar_as_que_citam(&self, nome: &str) {
        let nome = nome.to_uppercase();
        let cita = |raw: &str| raw.to_uppercase().contains(&nome);
        let mut chaves: Vec<u64> = self
            .cells
            .iter()
            .filter(|(_, c)| c.formula.is_some() && cita(&c.raw))
            .map(|(k, _)| *k)
            .collect();
        let usado = self.used_rect();
        self.formulas.for_each_in_range(usado.r0, usado.r1, usado.c0, usado.c1, |r, c| {
            if self.formulas.raw_at(r, c).is_some_and(|raw| cita(&raw)) {
                chaves.push(Self::cell_key(r, c));
            }
        });
        self.sujar_celulas(&chaves);
    }

    /// Marca `k` para recalcular e faz a cascata: toda fórmula que depende
    /// dele (direta ou por intervalo), e quem depende dessas, e assim por
    /// diante.
    fn mark_dirty_cascade(&self, k: CellKey) {
        if let Some(adiadas) = self.cascata_adiada.borrow_mut().as_mut() {
            adiadas.push(k);
            return;
        }
        self.sujar_volateis();
        self.sujar_cascata(k);
    }

    /// Começa uma escrita em lote: até [`Self::soltar_cascata`], cada célula
    /// escrita só é anotada. Regravar as fórmulas de uma coluna acumulada
    /// (`=K1+I2`, `=K2+I3`…) ao inserir uma coluna fazia, a cada célula, a
    /// cascata percorrer a cadeia inteira até o fim: quadrático (2 mil
    /// linhas = 3,3 s; 1 milhão, mais de 13 min).
    pub fn adiar_cascata(&self) {
        let mut adiadas = self.cascata_adiada.borrow_mut();
        if adiadas.is_none() {
            *adiadas = Some(Vec::new());
        }
    }

    /// Fim da escrita em lote: uma cascata só, com uma memória de visitadas
    /// para todas as células anotadas.
    pub fn soltar_cascata(&self) {
        let Some(adiadas) = self.cascata_adiada.borrow_mut().take() else { return };
        if adiadas.is_empty() {
            return;
        }
        self.sujar_volateis();
        self.sujar_cascata_de(adiadas);
    }

    /// Um recálculo da pasta (edição, F9, abrir) sorteia de novo as
    /// voláteis; leitura e desenho nunca. Marca cada volátil como suja, com
    /// os dependentes. Quem já está suja é pulada.
    pub(crate) fn sujar_volateis(&self) {
        if self.deps.volatiles.is_empty() {
            return;
        }
        let pendentes: Vec<CellKey> = {
            let calc = self.calc.borrow();
            self.deps.volatiles.iter().copied()
                .filter(|k| !calc.dirty.contains(k) && calc.cache.contains_key(k))
                .collect()
        };
        for v in pendentes {
            self.sujar_cascata(v);
        }
    }

    fn sujar_cascata(&self, k: CellKey) {
        self.sujar_cascata_de(vec![k]);
    }

    /// A cascata a partir de várias células, cada dependente visitado uma vez.
    fn sujar_cascata_de(&self, inicio: Vec<CellKey>) {
        self.tocar();
        // Não há fórmula na aba que possa depender desta escrita.
        // Anteriormente cada literal de uma importação/conversão era colocado
        // em calc.dirty mesmo sem fórmulas: milhões de entradas inúteis,
        // muita RAM e HashSet/hash por célula. Uma célula que deixou de ser
        // fórmula ainda precisa descartar seu cache individual.
        if self.deps.dependents.is_empty()
            && self.deps.range_deps.is_empty()
            && self.formulas.size() == 0
            && self.deps.array_cells.is_empty()
            && self.derrame.borrow().vazio()
        {
            let mut calc = self.calc.borrow_mut();
            for k in &inicio {
                calc.cache.remove(k);
                calc.dirty.remove(k);
            }
            drop(calc);
            let mut rastro = self.rastro.borrow_mut();
            for k in inicio {
                rastro.anotar(k);
            }
            return;
        }
        let mut queue = inicio;
        let mut visited = std::collections::HashSet::new();
        let mut colunas_inteiras = std::collections::HashSet::new();
        // Sem matriz dinâmica na aba, o passo 0 abaixo não custa nada.
        let tem_derrame = !self.deps.array_cells.is_empty() || !self.derrame.borrow().vazio();

        while let Some(curr) = queue.pop() {
            if !visited.insert(curr) {
                continue;
            }
            {
                // O cache só calcula fórmulas. Marcar cada literal como sujo
                // cria milhões de chaves mesmo quando há apenas UMA fórmula
                // em outro lugar da planilha. Dependentes continuam sendo
                // percorridos abaixo; só eles precisam entrar em calc.dirty.
                let (r, c) = Self::key_to_coord(curr);
                let formula = self.cells.get(&curr)
                    .is_some_and(|cel| cel.formula.is_some())
                    || self.formulas.has(r, c);
                let mut calc = self.calc.borrow_mut();
                if formula {
                    calc.dirty.insert(curr);
                } else {
                    calc.dirty.remove(&curr);
                }
                calc.cache.remove(&curr);
            }
            self.rastro.borrow_mut().anotar(curr);
            // Cálculo manual: a célula mudou, mas quem depende dela espera o F9.
            if self.manual.get() {
                self.adiadas.borrow_mut().push(curr);
                continue;
            }
            // 0) Escrever na área de uma matriz (derramada ou bloqueada) faz a
            // âncora recalcular: ela some para dar #SPILL!, ou volta a derramar.
            if tem_derrame {
                for ancora in self.ancoras_da_area(curr) {
                    if !visited.contains(&ancora) {
                        queue.push(ancora);
                    }
                }
                if self.deixou_de_ser_ancora(curr) {
                    self.descartar_ancora(curr);
                }
            }
            // 1) Dependentes diretos do grafo normal — um nível só; a fila
            // daqui faz a cascata com `visited`. `invalidate_dependents`
            // com um filtro que sempre aceita refazia a cascata inteira a
            // cada passo, sem memória: exponencial em cadeias de fórmulas.
            self.deps.step_dependents(curr, |dep| {
                if !visited.contains(&dep) {
                    queue.push(dep);
                }
            });

            // 2) Dependentes de padrões colunares em FormulaTable. O que
            // suja a coluna inteira (`A:A`, `$B$1`) vai por coluna, no fim:
            // enfileirar 13 milhões de células uma a uma era o travamento.
            self.step_pattern_dependents(curr, &mut |dep| {
                if !visited.contains(&dep) {
                    queue.push(dep);
                }
            }, Some(&mut colunas_inteiras));
        }
        if !colunas_inteiras.is_empty() {
            self.rastro.borrow_mut().desconhecer();
            let usos = self.formulas.columns_in_use();
            self.invalidar_colunas_e_dependentes(colunas_inteiras, &usos);
        }
    }

    /// As fórmulas empacotadas que leem `k`. Com `inteiras`, quando a coluna
    /// inteira do padrão lê `k` (referência ou intervalo de linhas fixas),
    /// a coluna vai para lá em vez de cada célula ir para `marcar`.
    fn step_pattern_dependents(&self, k: CellKey, marcar: &mut impl FnMut(CellKey),
        mut inteiras: Option<&mut std::collections::HashSet<u32>>) {
        if self.formulas.size() == 0 {
            return;
        }
        let (alvo_linha, alvo_col) = Self::key_to_coord(k);
        for col_use in self.formulas.columns_in_use() {
            let col = col_use.col;
            for uso in col_use.usos {
                for faixa in &uso.faixas {
                    let Some((c0, c1)) = faixa.colunas_em(col) else { continue };
                    if alvo_col < c0 || alvo_col > c1 {
                        continue;
                    }
                    if faixa.linhas_fixas() {
                        let Some((r0, _, r1, _)) = faixa.rect_em(0, col) else { continue };
                        if alvo_linha < r0 || alvo_linha > r1 {
                            continue;
                        }
                        match inteiras.as_deref_mut() {
                            Some(set) => { set.insert(col); }
                            None => self.formulas.for_each_with_pattern(col, uso.id, |r| marcar(Self::cell_key(r, col))),
                        }
                        continue;
                    }
                    // Janela que anda com a linha (`B1:B3` descendo): só as
                    // linhas cuja janela alcança a célula. Varrer a coluna a
                    // cada escrita era quadrático (CSV com `=SOMA(B1:B3)`).
                    if let Some((menor, maior)) = faixa.janela_relativa().filter(|(a, b)| b - a <= 4096) {
                        let de = (alvo_linha as i64 - maior).max(0);
                        let ate = alvo_linha as i64 - menor;
                        for r in de..=ate {
                            if self.formulas.uses_pattern(r as u32, col, uso.id) {
                                marcar(Self::cell_key(r as u32, col));
                            }
                        }
                        continue;
                    }
                    // Uma ponta fixa (`$B$1:B2`) ou janela enorme: a coluna toda.
                    match inteiras.as_deref_mut() {
                        Some(set) => { set.insert(col); }
                        None => self.formulas.for_each_with_pattern(col, uso.id, |r| {
                            if faixa.rect_em(r, col).is_some_and(|(r0, _, r1, _)| alvo_linha >= r0 && alvo_linha <= r1) {
                                marcar(Self::cell_key(r, col));
                            }
                        }),
                    }
                }
                for ref_ in &uso.refs {
                    // `Dados!A1` escrito dentro da própria Dados é local.
                    if ref_.sheet.as_deref().is_some_and(|s| !mesmo_nome(s, &self.name)) {
                        continue;
                    }
                    let ref_col = if ref_.col_lock { ref_.col } else { col as i32 + ref_.col };
                    if ref_col as u32 != alvo_col {
                        continue;
                    }
                    if ref_.row_lock {
                        if ref_.row as u32 != alvo_linha {
                            continue;
                        }
                        match inteiras.as_deref_mut() {
                            Some(set) => { set.insert(col); }
                            None => self.formulas.for_each_with_pattern(col, uso.id, |r| marcar(Self::cell_key(r, col))),
                        }
                        continue;
                    }
                    let linha = alvo_linha as i32 - ref_.row;
                    if linha < 0 {
                        continue;
                    }
                    if self.formulas.uses_pattern(linha as u32, col, uso.id) {
                        marcar(Self::cell_key(linha as u32, col));
                    }
                }
            }
        }
    }

    /// Recalcula `k` se estiver sujo (ou nunca calculado, ou volátil).
    ///
    /// Devolve `Some` quando o valor foi calculado sem a pasta em volta e a
    /// fórmula lê outra aba: esse valor serve a quem pediu e não entra no
    /// cache (ver [`Self::get_value_em`]).
    fn recalc_if_needed(
        &self,
        k: CellKey,
        node: &Node,
        row: u32,
        col: u32,
        host: Option<&dyn Hospedeiro>,
    ) -> Option<Value> {
        let externa = self.externas.contains(&k);
        let pronto = {
            let calc = self.calc.borrow();
            calc.cache.contains_key(&k) && !calc.dirty.contains(&k)
        };
        if pronto {
            return None;
        }
        {
            let mut calc = self.calc.borrow_mut();
            if calc.evaluating.contains(&k) {
                calc.cache.insert(k, Value::Error(CellError::new(ErrorCode::Circular)));
                calc.dirty.remove(&k);
                return None;
            }
            calc.evaluating.insert(k);
        }
        let ctx = SheetCtx { sheet: self, host };
        let ev = Evaluator::na_celula(&ctx, row, col);
        let valor = if let Some(erro) = externa.then(|| self.intervalo_externo_sem_valores(node, host)).flatten() {
            erro
        } else if !self.deps.array_cells.is_empty() && self.deps.array_cells.contains(&k) {
            self.avaliar_ancora(k, &ev, node, externa && host.is_none())
        } else {
            ev.eval(node)
        };
        let mut calc = self.calc.borrow_mut();
        calc.evaluating.remove(&k);
        if corrente::voltando() {
            return Some(Value::Empty);
        }
        if externa && host.is_none() {
            return Some(valor);
        }
        calc.cache.insert(k, valor);
        calc.dirty.remove(&k);
        None
    }

    /// Reconstrói o grafo de dependências do zero e joga fora todo valor


    /// Em CSVs grandes o texto de cada célula já existe no pool.
    /// Devolve uma fatia emprestada em vez de alocar um novo String durante
    /// validação e conversão de milhões de linhas. Números necessitam de
    /// formatação e retornam None (o chamador pode usar `get_raw`).
    /// Fórmulas são retornadas como raw, para que o chamador decida avaliá-las.
    pub fn texto_cru_divisao(&self, row: u32, col: u32) -> Option<&str> {
        let k = Self::cell_key(row, col);
        if let Some(raw) = self.textos.get(&k) { return Some(raw); }
        if let Some(celula) = self.cells.get(&k) { return Some(&celula.raw); }
        match self.data.cru_em(row, col) {
            Some(Cru::Texto(id)) => Some(crate::planilha::data_table::sem_marca(self.pool.get(id))),
            _ => None,
        }
    }

    pub fn get_raw(&self, row: u32, col: u32) -> String {
        let k = Self::cell_key(row, col);
        if let Some(t) = self.textos.get(&k) {
            return t.to_owned();
        }
        if let Some(cell) = self.cells.get(&k) {
            cell.raw.clone()
        } else if let Some(raw) = self.formulas.raw_at(row, col) {
            raw
        } else {
            self.data.get_raw(row, col, &self.pool)
        }
    }

    /// O estilo da célula, fórmula ou literal — os dois lugares onde ele
    /// pode morar hoje (ver a nota de topo do arquivo).
    /// Grava um valor literal já resolvido, sem passar pelo `input.rs`.
    ///
    /// ⚠ É o caminho de CARGA de arquivo, e só ele. Quem digita passa pelo
    /// [`Self::set_cell_raw`], que decide se `199,9` é número ou texto. Aqui o
    /// valor já veio decidido do `.fshz` — reinterpretá-lo seria repetir um
    /// trabalho caro dois milhões de vezes, e pior: um texto que *parece*
    /// número (o código de produto "0012") voltaria do arquivo como número.
    pub fn set_cell_valor(&mut self, row: u32, col: u32, valor: Value) {
        if row >= MAX_ROWS || col >= MAX_COLS {
            return;
        }
        let k = Self::cell_key(row, col);
        self.rastro.borrow_mut().anotar(k);
        self.cells.remove(&k);
        if row < self.min_row {
            self.min_row = row;
        }
        if row > self.max_row {
            self.max_row = row;
        }
        if col < self.min_col {
            self.min_col = col;
        }
        if col > self.max_col {
            self.max_col = col;
        }
        self.data.set_cell_value(row, col, &valor, &mut self.pool);
    }

    /// Aponta a célula para um estilo já registrado.
    ///
    /// Serve à carga do arquivo, onde o estilo da literal colunar vem numa
    /// lista separada das células (é assim que o `.fshz` grava).
    pub fn set_style_idx(&mut self, row: u32, col: u32, idx: usize) {
        let k = Self::cell_key(row, col);
        if let Some(cell) = self.cells.get_mut(&k) {
            cell.style_idx = Some(idx);
        } else {
            self.styles.insert(k, idx);
        }
    }

    /// Guarda um estilo e devolve o índice dele, reaproveitando se já existe.
    ///
    /// Estilo vazio não ocupa lugar: devolve `None`, e a célula fica sem
    /// estilo — é o que mantém o quadro pequeno numa planilha sem formatação.
    pub fn registrar_estilo(&mut self, estilo: crate::celulas::style::Style) -> Option<usize> {
        if estilo.is_empty() {
            return None;
        }
        self.registrar_parcial(crate::celulas::estilo_parcial::de_style(&estilo))
    }

    /// Guarda um estilo parcial e devolve o índice, reaproveitando se já
    /// existe. Vazio não ocupa lugar.
    pub fn registrar_parcial(&mut self, parcial: EstiloParcial) -> Option<usize> {
        if parcial.is_empty() {
            return None;
        }
        if let Some(i) = self.style_parts.iter().position(|e| *e == parcial) {
            return Some(i);
        }
        self.style_table.push(crate::celulas::estilo_parcial::para_style(&parcial));
        self.style_parts.push(parcial);
        Some(self.style_parts.len() - 1)
    }

    /// O estilo de uma célula, já resolvido.
    pub fn get_style(&self, row: u32, col: u32) -> Option<&crate::celulas::style::Style> {
        let idx = self.get_style_idx(row, col)?;
        self.style_table.get(idx)
    }

    pub fn get_style_idx(&self, row: u32, col: u32) -> Option<usize> {
        let k = Self::cell_key(row, col);
        if let Some(cell) = self.cells.get(&k) {
            return cell.style_idx;
        }
        self.styles.get(k)
    }

    /// Quantos itens há em cada estrutura da aba, para achar quem gasta
    /// memória (Monitor e medições).
    pub fn contagem_memoria(&self) -> String {
        let calc = self.calc.borrow();
        format!(
            "literais {} · fórmulas empacotadas {} · avulsas {} · estilos {} · textos {} · \
             resultados guardados {} · sujas {} · faixas lidas {} células",
            self.data.size(), self.formulas.size(), self.cells.len(), self.styles.len(),
            self.textos.len(), calc.cache.len(), calc.dirty.len(), self.faixas.borrow().celulas,
        )
    }

    /// Esquece as faixas lidas (ver [`Self::faixa_lida`]): servem enquanto
    /// muitas fórmulas leem as mesmas colunas; depois são só memória. Os
    /// agregados dos `*SES` ficam: são do tamanho dos grupos, não da coluna,
    /// e caem sozinhos quando a aba muda. Soltá-los a cada pulso do monitor
    /// (1 s) refazia o índice do `MÍNIMOSES` de 13,7 milhões de linhas
    /// (1,9 s) a cada segundo — o app "meio lerdo" de 17/10/2026.
    pub fn soltar_faixas(&self) {
        let mut faixas = self.faixas.borrow_mut();
        faixas.mapa = Default::default();
        faixas.celulas = 0;
    }

    /// O agregado por critério das faixas (todas desta aba, de mesmo
    /// tamanho), montado uma vez e guardado até a aba mudar. `None` quando
    /// não compensa ou não é seguro (faixa pequena, aba com volátil, cálculo
    /// sendo desfeito, `NaN`, grupos demais): quem chama varre célula por
    /// célula, como antes. Ver [`crate::planilha::agregado`].
    pub fn agregado(&self, soma: Option<&Rect>, criterios: &[Rect], host: Option<&dyn Hospedeiro>)
        -> Option<std::sync::Arc<crate::planilha::agregado::Agregado>> {
        use crate::planilha::agregado::{Agregado, Area, MINIMO};
        let primeiro = criterios.first()?;
        if !self.deps.volatiles.is_empty() || corrente::voltando() {
            return None;
        }
        let linhas = primeiro.r1.checked_sub(primeiro.r0)? as u64 + 1;
        let colunas = primeiro.c1.checked_sub(primeiro.c0)? as u64 + 1;
        if linhas * colunas < MINIMO {
            return None;
        }
        let mesmo_tamanho = |r: &Rect| {
            r.r1.checked_sub(r.r0).map(|x| x as u64 + 1) == Some(linhas)
                && r.c1.checked_sub(r.c0).map(|x| x as u64 + 1) == Some(colunas)
        };
        if !criterios.iter().all(mesmo_tamanho) || !soma.is_none_or(mesmo_tamanho) {
            return None;
        }
        let area = |r: &Rect| -> Area { (r.r0, r.c0, r.r1, r.c1) };
        let chave = (soma.map(area), criterios.iter().map(area).collect::<Vec<_>>());
        let geracao = self.geracao.get();
        {
            let mut agregados = self.agregados.borrow_mut();
            if agregados.geracao != geracao {
                agregados.mapa.clear();
                agregados.geracao = geracao;
            }
            if let Some(a) = agregados.mapa.get(&chave) {
                return Some(a.clone());
            }
            // Sem faixa agregada (`CONT.SES`), a contagem de linhas é a mesma
            // de qualquer agregado com estes critérios: reaproveita.
            if soma.is_none() {
                if let Some(a) = agregados.mapa.iter().find(|(k, _)| k.1 == chave.1).map(|(_, a)| a.clone()) {
                    return Some(a);
                }
            }
        }
        // Além da área usada tudo é vazio, e vazio não casa com critério
        // indexável: só o trecho da primeira faixa dentro da área usada.
        let usado = self.used_rect();
        if self.is_empty() || primeiro.r0 > usado.r1 || primeiro.c0 > usado.c1 {
            return Some(std::sync::Arc::new(Agregado::default()));
        }
        let linhas = (linhas as u32).min(usado.r1 - primeiro.r0 + 1);
        let colunas = (colunas as u32).min(usado.c1 - primeiro.c0 + 1);
        // O empréstimo do mapa fica solto aqui: ler pode calcular fórmulas.
        let texto = |id: i32| self.pool.get(id);
        let leitor = std::cell::RefCell::new(crate::planilha::agregado::LeitorChaves::novo(&texto));
        let agregado = Agregado::montar(criterios.len(), soma.is_some(), linhas, colunas,
            |i, r, c| {
                let (row, col) = (criterios[i].r0 + r, criterios[i].c0 + c);
                match self.cru_rapido(row, col) {
                    Rapido::Cru(cru, literal) => leitor.borrow_mut().do_cru(cru, literal),
                    Rapido::Vazia => Ok(None),
                    Rapido::Lento => {
                        let v = self.get_value_em(row, col, host);
                        leitor.borrow_mut().da_celula(&v)
                    }
                }
            },
            |r, c| {
                let rect = soma.expect("só é pedido quando há faixa agregada");
                let (row, col) = (rect.r0 + r, rect.c0 + c);
                match self.cru_rapido(row, col) {
                    Rapido::Cru(Cru::Numero(n), _) => Some(n),
                    Rapido::Cru(Cru::Texto(_), _) | Rapido::Vazia => None,
                    Rapido::Lento => match self.get_value_em(row, col, host) {
                        Value::Number(n) => Some(n),
                        _ => None,
                    },
                }
            },
            || std::mem::take(&mut leitor.borrow_mut().chaves),
        )?;
        if corrente::voltando() {
            return None;
        }
        let agregado = std::sync::Arc::new(agregado);
        if self.geracao.get() == geracao {
            self.agregados.borrow_mut().mapa.insert(chave, agregado.clone());
        }
        Some(agregado)
    }

    /// Algo na aba mudou (ou pode ter mudado): as faixas guardadas não
    /// valem mais.
    pub fn tocar(&self) {
        self.geracao.set(self.geracao.get().wrapping_add(1));
    }

    /// A geração atual (sobe em [`Self::tocar`]): quem guarda algo lido da
    /// aba fora dela (os dados de um gráfico, na interface) compara este
    /// número para saber se o guardado ainda vale.
    pub fn geracao(&self) -> u64 {
        self.geracao.get()
    }

    /// Os valores do retângulo lidos de uma vez, guardados até a aba mudar.
    /// `None` quando não compensa ou não é seguro guardar: faixa pequena ou
    /// enorme, aba com função volátil, ou cálculo sendo desfeito — quem
    /// chama lê célula por célula, como antes.
    pub fn faixa_lida(&self, rect: &Rect, host: Option<&dyn Hospedeiro>)
        -> Option<std::sync::Arc<crate::planilha::faixa_lida::FaixaLida>> {
        use crate::planilha::faixa_lida::{FaixaLida, MAXIMO, MINIMO, TOTAL};
        if !self.deps.volatiles.is_empty() || corrente::voltando() {
            return None;
        }
        let pedidas = (rect.r1 as u64 - rect.r0 as u64 + 1) * (rect.c1 as u64 - rect.c0 as u64 + 1);
        if pedidas < MINIMO {
            return None;
        }
        let chave = (rect.r0, rect.r1, rect.c0, rect.c1);
        let geracao = self.geracao.get();
        {
            let mut faixas = self.faixas.borrow_mut();
            if faixas.geracao != geracao {
                faixas.mapa.clear();
                faixas.celulas = 0;
                faixas.geracao = geracao;
            }
            if let Some(f) = faixas.mapa.get(&chave) {
                return Some(f.clone());
            }
        }
        // Além da última linha/coluna usada tudo é vazio: `A:A` não vira
        // um milhão de vazios guardados.
        let usado = self.used_rect();
        let (r1, c1) = (rect.r1.min(usado.r1), rect.c1.min(usado.c1));
        let (linhas, colunas) = if self.is_empty() || r1 < rect.r0 || c1 < rect.c0 {
            (0, 0)
        } else {
            (r1 - rect.r0 + 1, c1 - rect.c0 + 1)
        };
        let n = linhas as u64 * colunas as u64;
        if n > MAXIMO {
            return None;
        }
        // O empréstimo do mapa fica solto aqui: ler pode calcular fórmulas
        // que pedem outras faixas desta mesma aba.
        let mut valores = Vec::with_capacity(n as usize);
        for r in rect.r0..rect.r0 + linhas {
            for c in rect.c0..rect.c0 + colunas {
                valores.push(self.get_value_em(r, c, host));
            }
        }
        if corrente::voltando() {
            return None;
        }
        let faixa = std::sync::Arc::new(FaixaLida::nova(rect.r0, rect.c0, linhas, colunas, valores));
        if self.geracao.get() == geracao {
            let mut faixas = self.faixas.borrow_mut();
            if faixas.celulas + n as usize > TOTAL {
                faixas.mapa.clear();
                faixas.celulas = 0;
            }
            faixas.celulas += n as usize;
            faixas.mapa.insert(chave, faixa.clone());
        }
        Some(faixa)
    }

    /// O valor cru da célula quando ele está pronto no armazenamento colunar
    /// (literal, ou fórmula empacotada já calculada e sem resultado mais
    /// novo no `calc`), sem montar `Value`. `Lento`: ler pelo
    /// [`Self::get_value_em`] de sempre.
    fn cru_rapido(&self, row: u32, col: u32) -> Rapido {
        let k = Self::cell_key(row, col);
        if self.cells.contains_key(&k) {
            return Rapido::Lento;
        }
        if self.formulas.has(row, col) {
            {
                let calc = self.calc.borrow();
                if calc.dirty.contains(&k) || calc.cache.contains_key(&k) {
                    return Rapido::Lento;
                }
            }
            return match self.formulas.cru_at(row, col) {
                Some(cru) => Rapido::Cru(cru, false),
                None => Rapido::Lento,
            };
        }
        match self.data.cru_em(row, col) {
            Some(cru) => Rapido::Cru(cru, true),
            None if self.deps.array_cells.is_empty() => Rapido::Vazia,
            None => Rapido::Lento,
        }
    }

    pub fn used_rect(&self) -> Rect {
        let mut usado = if self.is_empty() {
            Rect::new(0, 0, 0, 0)
        } else {
            Rect::new(self.min_row, self.min_col, self.max_row, self.max_col)
        };
        // A área derramada conta como usada (Ctrl+End, SOMA(A:A), impressão).
        if self.derrame.borrow().vazio() {
            return usado;
        }
        for area in self.areas_derramadas() {
            usado = Rect::new(usado.r0.min(area.r0), usado.c0.min(area.c0),
                usado.r1.max(area.r1), usado.c1.max(area.c1));
        }
        usado
    }

    pub fn len(&self) -> usize {
        self.cells.len() + self.data.size() as usize + self.formulas.size() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.data.size() == 0 && self.formulas.size() == 0
    }

    /// Tira todas as células literais (`row`, `col`, `valor`) de [`Self::data`]
    /// para reconstruir a coluna deslocada — [`crate::planilha::column_store`] não tem
    /// (ainda) um jeito de mover linha/coluna dentro do bloco, então o
    /// caminho é o mesmo dos dois `HashMap` de antes: extrai tudo, limpa,
    /// recoloca já na posição nova. Mesma complexidade de sempre (O(células)),
    /// só que agora inclui texto e booleano, que o `HashMap<u64, f64>`
    /// antigo não guardava.
    fn literais_extraidos(&self) -> Vec<(u32, u32, Value)> {
        let mut out = Vec::new();
        self.data.for_each(&self.pool, &mut |row, col, val| {
            out.push((row, col, val.clone()));
        });
        out
    }

    /// `externas` anda junto com `cells` ao inserir/excluir linha ou coluna
    /// (`None` = a célula foi excluída); senão a fórmula que lê nome ou outra
    /// aba deixa de ser avisada quando a fonte muda.
    fn reposicionar_externas(&mut self, nova: impl Fn(u32, u32) -> Option<(u32, u32)>) {
        self.externas = std::mem::take(&mut self.externas).into_iter().filter_map(|k| {
            let (r, c) = Self::key_to_coord(k);
            nova(r, c).map(|(r, c)| Self::cell_key(r, c))
        }).collect();
    }

    pub fn insert_rows(&mut self, at: u32, count: u32) {
        self.rastro.borrow_mut().desconhecer();
        if count == 0 {
            return;
        }

        let mut next_cells = HashMap::with_capacity(self.cells.len());
        for (k, cell) in self.cells.drain() {
            let (r, c) = Self::key_to_coord(k);
            if r >= at {
                next_cells.insert(Self::cell_key(r + count, c), cell);
            } else {
                next_cells.insert(k, cell);
            }
        }
        self.cells = next_cells;
        self.reposicionar_externas(|r, c| Some((if r >= at { r + count } else { r }, c)));
        // As âncoras andaram com o grafo; as áreas derramadas recalculam.
        self.descartar_derramamentos();

        let literais = self.literais_extraidos();
        self.data.clear();
        for (r, c, val) in literais {
            let novo_r = if r >= at { r + count } else { r };
            self.data.set_cell_value(novo_r, c, &val, &mut self.pool);
        }

        self.styles.inserir_linhas(at, count);

        let mut next_rh = HashMap::new();
        for (r, h) in self.row_heights.drain() {
            if r >= at {
                next_rh.insert(r + count, h);
            } else {
                next_rh.insert(r, h);
            }
        }
        self.row_heights = next_rh;
        if let Some(n) = self.linhas_ocultas_desde.filter(|&n| n >= at) {
            self.linhas_ocultas_desde = Some(n.saturating_add(count).min(MAX_ROWS - 1));
        }

        if self.max_row >= at && self.max_row > 0 {
            self.max_row += count;
        }

        self.formulas.shift_rows(at, count, false);
        self.deps.shift_rows(at, count, false);
        let mut calc = self.calc.borrow_mut();
        calc.cache.clear();
        calc.dirty.clear();
        calc.evaluating.clear();
    }

    pub fn delete_rows(&mut self, at: u32, count: u32) {
        self.rastro.borrow_mut().desconhecer();
        if count == 0 {
            return;
        }
        let end = at + count;

        let mut next_cells = HashMap::with_capacity(self.cells.len());
        for (k, cell) in self.cells.drain() {
            let (r, c) = Self::key_to_coord(k);
            if r >= at && r < end {
                continue;
            } else if r >= end {
                next_cells.insert(Self::cell_key(r - count, c), cell);
            } else {
                next_cells.insert(k, cell);
            }
        }
        self.cells = next_cells;
        self.reposicionar_externas(|r, c| (r < at || r >= end).then(|| (if r >= end { r - count } else { r }, c)));
        // As âncoras andaram com o grafo; as áreas derramadas recalculam.
        self.descartar_derramamentos();

        let literais = self.literais_extraidos();
        self.data.clear();
        for (r, c, val) in literais {
            if r >= at && r < end {
                continue;
            }
            let novo_r = if r >= end { r - count } else { r };
            self.data.set_cell_value(novo_r, c, &val, &mut self.pool);
        }

        self.styles.excluir_linhas(at, count);

        let mut next_rh = HashMap::new();
        for (r, h) in self.row_heights.drain() {
            if r >= at && r < end {
                continue;
            } else if r >= end {
                next_rh.insert(r - count, h);
            } else {
                next_rh.insert(r, h);
            }
        }
        self.row_heights = next_rh;
        // Excluir dentro da faixa oculta não a muda; acima dela, ela sobe.
        if let Some(n) = self.linhas_ocultas_desde {
            self.linhas_ocultas_desde = Some(if n >= end { n - count } else if n > at { at } else { n });
        }

        self.formulas.shift_rows(at, count, true);
        self.deps.shift_rows(at, count, true);
        let mut calc = self.calc.borrow_mut();
        calc.cache.clear();
        calc.dirty.clear();
        calc.evaluating.clear();
    }

    pub fn insert_cols(&mut self, at: u32, count: u32) {
        self.rastro.borrow_mut().desconhecer();
        if count == 0 {
            return;
        }

        let mut next_cells = HashMap::with_capacity(self.cells.len());
        for (k, cell) in self.cells.drain() {
            let (r, c) = Self::key_to_coord(k);
            if c >= at {
                next_cells.insert(Self::cell_key(r, c + count), cell);
            } else {
                next_cells.insert(k, cell);
            }
        }
        self.cells = next_cells;
        self.reposicionar_externas(|r, c| Some((r, if c >= at { c + count } else { c })));
        // As âncoras andaram com o grafo; as áreas derramadas recalculam.
        self.descartar_derramamentos();

        // Colunar: deslocar é trocar a chave. Ver a nota em `delete_cols`.
        self.data.shift_columns(at, count, false);

        self.styles.inserir_colunas(at, count);

        let mut next_cw = HashMap::new();
        for (c, w) in self.col_widths.drain() {
            if c >= at {
                next_cw.insert(c + count, w);
            } else {
                next_cw.insert(c, w);
            }
        }
        self.col_widths = next_cw;
        if let Some(n) = self.colunas_ocultas_desde.filter(|&n| n >= at) {
            self.colunas_ocultas_desde = Some(n.saturating_add(count).min(MAX_COLS - 1));
        }

        if self.max_col >= at && self.max_col > 0 {
            self.max_col += count;
        }

        self.formulas.shift_columns(at, count, false);
        self.deps.shift_columns(at, count, false);
        let mut calc = self.calc.borrow_mut();
        calc.cache.clear();
        calc.dirty.clear();
        calc.evaluating.clear();
    }

    pub fn delete_cols(&mut self, at: u32, count: u32) {
        self.rastro.borrow_mut().desconhecer();
        if count == 0 {
            return;
        }
        let end = at + count;

        let mut next_cells = HashMap::with_capacity(self.cells.len());
        for (k, cell) in self.cells.drain() {
            let (r, c) = Self::key_to_coord(k);
            if c >= at && c < end {
                continue;
            } else if c >= end {
                next_cells.insert(Self::cell_key(r, c - count), cell);
            } else {
                next_cells.insert(k, cell);
            }
        }
        self.cells = next_cells;
        self.reposicionar_externas(|r, c| (c < at || c >= end).then(|| (r, if c >= end { c - count } else { c })));
        // As âncoras andaram com o grafo; as áreas derramadas recalculam.
        self.descartar_derramamentos();

        // ⚠ O `DataTable` é colunar — deslocar é trocar a chave.
        self.data.shift_columns(at, count, true);

        self.styles.excluir_colunas(at, count);

        let mut next_cw = HashMap::new();
        for (c, w) in self.col_widths.drain() {
            if c >= at && c < end {
                continue;
            } else if c >= end {
                next_cw.insert(c - count, w);
            } else {
                next_cw.insert(c, w);
            }
        }
        self.col_widths = next_cw;
        if let Some(n) = self.colunas_ocultas_desde {
            self.colunas_ocultas_desde = Some(if n >= end { n - count } else if n > at { at } else { n });
        }

        self.formulas.shift_columns(at, count, true);
        self.deps.shift_columns(at, count, true);
        let mut calc = self.calc.borrow_mut();
        calc.cache.clear();
        calc.dirty.clear();
        calc.evaluating.clear();
    }
}

/// Resumo da seleção para a barra de status: ignora vazios e linhas ocultas
/// (altura 0). Números alimentam soma, média, mínimo, máximo e desvio padrão;
/// erros só são contados à parte (não entram na contagem, como no Excel).
#[derive(Clone, Debug, PartialEq)]
pub struct ContagemSelecao {
    pub count: u64,
    pub numbers: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    /// Células com erro (#DIV/0!, #N/D…).
    pub erros: u64,
    /// Média corrente e soma dos quadrados dos desvios (Welford): o desvio
    /// padrão sai sem perder precisão em números grandes e parecidos.
    pub media_corrente: f64,
    pub m2: f64,
}
impl Default for ContagemSelecao {
    fn default() -> Self {
        Self {
            count: 0, numbers: 0, sum: 0., min: f64::INFINITY, max: f64::NEG_INFINITY,
            erros: 0, media_corrente: 0., m2: 0.,
        }
    }
}
impl ContagemSelecao {
    fn registrar(&mut self, valor: &Value) {
        match valor {
            Value::Empty => {}
            Value::Error(_) => self.erros += 1,
            Value::Number(v) => {
                self.count += 1;
                self.numbers += 1;
                self.sum += *v;
                self.min = self.min.min(*v);
                self.max = self.max.max(*v);
                let delta = *v - self.media_corrente;
                self.media_corrente += delta / self.numbers as f64;
                self.m2 += delta * (*v - self.media_corrente);
            }
            Value::Text(_) | Value::Bool(_) => self.count += 1,
        }
    }

    /// Reúne lotes sem perder os extremos (desvio pela fórmula de Chan).
    pub fn somar_lote(&mut self, outro: &Self) {
        let (na, nb) = (self.numbers as f64, outro.numbers as f64);
        if nb > 0. {
            let n = na + nb;
            let delta = outro.media_corrente - self.media_corrente;
            self.m2 += outro.m2 + delta * delta * na * nb / n;
            self.media_corrente += delta * nb / n;
        }
        self.count += outro.count;
        self.numbers += outro.numbers;
        self.sum += outro.sum;
        self.min = self.min.min(outro.min);
        self.max = self.max.max(outro.max);
        self.erros += outro.erros;
    }

    pub fn media(&self) -> Option<f64> {
        (self.numbers > 0).then(|| self.sum / self.numbers as f64)
    }

    /// Desvio padrão amostral (DESVPAD): precisa de dois números.
    pub fn desvio_padrao(&self) -> Option<f64> {
        (self.numbers > 1).then(|| (self.m2.max(0.) / (self.numbers - 1) as f64).sqrt())
    }
}
impl Sheet {
    /// O hospedeiro permite calcular fórmulas com referências a outras abas.
    /// Recorta seleções de coluna/linha inteira pela área efetivamente usada.
    pub fn contar_selecao(
        &self, selecao: &Rect, host: Option<&dyn Hospedeiro>,
    ) -> ContagemSelecao {
        let mut resultado = ContagemSelecao::default();
        if self.is_empty() { return resultado; }
        let usado = self.used_rect();
        let r0 = selecao.r0.max(usado.r0);
        let c0 = selecao.c0.max(usado.c0);
        let r1 = selecao.r1.min(usado.r1);
        let c1 = selecao.c1.min(usado.c1);
        if r1 < r0 || c1 < c0 { return resultado; }
        let intervalo = Rect::new(r0, c0, r1, c1);
        let contexto = SheetCtx { sheet: self, host };
        // Linha oculta (altura 0, filtro) não conta. Sem nenhuma oculta, pula
        // a consulta ao mapa a cada célula: era metade do custo em 13 milhões.
        let tem_ocultas = self.tem_linhas_ocultas();
        contexto.varrer(&intervalo, &mut |valor, linha, _coluna| {
            if tem_ocultas && self.linha_oculta(linha) { return; }
            resultado.registrar(valor);
        });
        resultado
    }
}

/// Empresta a `Sheet` para o [`Evaluator`] enxergar como [`EvalContext`].
/// Fica de fora do `impl Sheet` porque implementa uma trait de outro
/// arquivo, não um método da struct.
struct SheetCtx<'a> {
    sheet: &'a Sheet,
    host: Option<&'a dyn Hospedeiro>,
}

impl SheetCtx<'_> {
    /// Visita o retângulo direto no armazenamento (avulsas, fórmulas,
    /// colunas, derramados), sem montar nem guardar a faixa lida. É o que
    /// o rodapé usa: a faixa em cache serve às fórmulas, não a uma contagem
    /// feita uma vez (montá-la por lote custava 10× a conta).
    fn varrer(&self, rect: &Rect, visit: &mut dyn FnMut(&Value, u32, u32)) {
        for (&k, cell) in &self.sheet.cells {
            let (r, c) = Sheet::key_to_coord(k);
            if r < rect.r0 || r > rect.r1 || c < rect.c0 || c > rect.c1 {
                continue;
            }
            let valor = self.sheet.get_value_em(r, c, self.host);
            if !matches!(valor, Value::Empty) {
                visit(&valor, r, c);
            }
            let _ = cell;
        }
        self.sheet.formulas.for_each_in_range(rect.r0, rect.r1, rect.c0, rect.c1, |r, c| {
            let valor = self.sheet.get_value_em(r, c, self.host);
            if !matches!(valor, Value::Empty) {
                visit(&valor, r, c);
            }
        });
        self.sheet.data.for_each_in_range(rect, &self.sheet.pool, visit);
        self.sheet.para_cada_derramado(rect, self.host, visit);
    }
}

/// A pasta, vista de dentro de uma aba: é por aqui que `Dados!A1` e o
/// intervalo nomeado chegam ao avaliador. Quem implementa é a
/// [`crate::planilha::workbook::Workbook`].
pub trait Hospedeiro {
    /// A aba com esse nome (sem diferenciar maiúscula), se existir.
    fn aba(&self, nome: &str) -> Option<&Sheet>;
    /// O intervalo nomeado, já com a aba dele.
    fn nome(&self, nome: &str) -> Option<Rect>;
    /// Se o nome é de uma função criada por macro (a interface a executa).
    fn funcao_externa(&self, _nome: &str) -> bool {
        false
    }
    /// A aba neste endereço de memória, se for desta pasta — é como a
    /// corrente adiada lembra de que aba era a célula.
    fn aba_no_endereco(&self, _endereco: usize) -> Option<&Sheet> {
        None
    }
    /// Os nomes das abas, na ordem das guias.
    fn nomes_das_abas(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Se a fórmula lê algo de fora da própria aba: outra aba ou um nome.
pub(crate) fn le_de_fora(node: &Node, propria: &str) -> bool {
    let propria = propria.to_lowercase();
    let mut fora = false;
    node.walk(&mut |n| match n {
        Node::Name(_) => fora = true,
        Node::Ref { sheet: Some(s), .. } | Node::Range { sheet: Some(s), .. } if s.to_lowercase() != propria => {
            fora = true;
        }
        _ => {}
    });
    fora
}

/// Nome de aba igual sem ligar para maiúscula, sem alocar texto: é
/// perguntado a cada célula lida de um intervalo.
pub fn mesmo_nome(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

impl<'a> SheetCtx<'a> {
    /// A aba de que a referência fala: esta, outra da pasta, ou nenhuma.
    fn alvo(&self, nome: Option<&str>) -> Option<&'a Sheet> {
        match nome {
            None => Some(self.sheet),
            Some(n) if mesmo_nome(n, &self.sheet.name) => Some(self.sheet),
            Some(n) => self.host.and_then(|h| h.aba(n)),
        }
    }

    /// O `alvo`, com as matrizes da outra aba já derramadas: a leitura de
    /// fora (profundidade 0) só garante as âncoras da própria aba, e
    /// `Dados!A2` coberta por `SEQUÊNCIA` em `Dados!A1` saía vazia.
    fn alvo_derramado(&self, nome: Option<&str>) -> Option<&'a Sheet> {
        let aba = self.alvo(nome)?;
        if !std::ptr::eq(aba, self.sheet) && !aba.deps.array_cells.is_empty() {
            aba.garantir_derramamentos_frio(self.host);
        }
        Some(aba)
    }
}

impl<'a> EvalContext for SheetCtx<'a> {
    fn get_value(&self, row: u32, col: u32, sheet: Option<&str>) -> Value {
        match self.alvo_derramado(sheet) {
            Some(aba) => aba.get_value_em(row, col, self.host),
            None => Value::Error(CellError::com_motivo(
                ErrorCode::Ref,
                format!("Aba \"{}\" não existe", sheet.unwrap_or_default()),
            )),
        }
    }

    fn for_each_in_range(&self, rect: &Rect, visit: &mut dyn FnMut(&Value, u32, u32)) {
        let Some(aba) = self.alvo_derramado(rect.sheet.as_deref()) else {
            return;
        };
        if !std::ptr::eq(aba, self.sheet) {
            let ctx = SheetCtx { sheet: aba, host: self.host };
            let mut local = rect.clone();
            local.sheet = None;
            return ctx.for_each_in_range(&local, visit);
        }
        // Intervalo grande: a faixa lida de uma vez (e guardada) evita varrer
        // o mapa de células avulsas inteiro a cada `SOMA`.
        if let Some(faixa) = self.sheet.faixa_lida(rect, self.host) {
            return faixa.para_cada(visit);
        }
        self.varrer(rect, visit);
    }

    fn faixa_lida(&self, rect: &Rect) -> Option<std::sync::Arc<crate::planilha::faixa_lida::FaixaLida>> {
        self.alvo(rect.sheet.as_deref())?.faixa_lida(rect, self.host)
    }

    fn agregado(&self, soma: Option<&Rect>, criterios: &[Rect]) -> Option<std::sync::Arc<crate::planilha::agregado::Agregado>> {
        let aba = self.alvo(criterios.first()?.sheet.as_deref())?;
        // Todas as faixas na mesma aba: o agregado mora nela.
        let mesma = |r: &Rect| self.alvo(r.sheet.as_deref()).is_some_and(|a| std::ptr::eq(a, aba));
        if !criterios.iter().all(mesma) || !soma.is_none_or(mesma) {
            return None;
        }
        aba.agregado(soma, criterios, self.host)
    }

    fn range_size(&self, rect: &Rect) -> u64 {
        let largura = (rect.c1 - rect.c0 + 1) as u64;
        let altura = (rect.r1 - rect.r0 + 1) as u64;
        largura * altura
    }

    /// Intervalo nomeado: mora na `Workbook` de verdade (etapa 2, ainda não
    /// chegou lá) — por ora, todo nome vira `#NAME?`, o mesmo que o
    /// TypeScript mostra para um nome que não existe.
    fn resolve_name(&self, name: &str) -> Option<Rect> {
        self.host.and_then(|h| h.nome(name))
    }

    /// A pergunta fica anotada na aba da fórmula; a interface responde depois.
    fn request_async(&self, task_key: &str, cell: (u32, u32), _start: crate::formula::evaluator::AsyncTask) -> crate::celulas::values::Computed {
        self.sheet.assincronos.borrow_mut().pedir(task_key, Sheet::cell_key(cell.0, cell.1))
    }

    fn linha_no_topico(&self, row: u32, sheet: Option<&str>) -> bool {
        self.alvo(sheet).is_some_and(|a| a.topicos.linhas.nivel(row) > 0)
    }

    fn linha_oculta(&self, row: u32, sheet: Option<&str>) -> bool {
        self.alvo(sheet).is_some_and(|a| a.linha_oculta(row))
    }

    fn celula_com_subtotal(&self, row: u32, col: u32, sheet: Option<&str>) -> bool {
        let Some(a) = self.alvo(sheet) else { return false };
        let raw = a.get_raw(row, col);
        if !raw.starts_with('=') { return false; }
        let u = raw.to_uppercase();
        u.contains("SUBTOTAL(") || u.contains("AGREGAR(") || u.contains("AGGREGATE(")
    }

    fn funcao_externa(&self, nome: &str) -> bool {
        self.host.is_some_and(|h| h.funcao_externa(nome))
    }

    fn cor_celula(&self, row: u32, col: u32, sheet: Option<&str>, fonte: bool) -> Option<String> {
        self.alvo(sheet)?.cor_celula(row, col, fonte)
    }

    fn formula_de(&self, row: u32, col: u32, sheet: Option<&str>) -> Option<String> {
        Some(self.alvo(sheet)?.get_raw(row, col)).filter(|r| r.starts_with('='))
    }

    fn abas(&self) -> (Vec<String>, Option<String>) {
        let nomes = self.host.map(|h| h.nomes_das_abas()).filter(|n| !n.is_empty())
            .unwrap_or_else(|| vec![self.sheet.name.clone()]);
        (nomes, Some(self.sheet.name.clone()))
    }
}

#[cfg(test)]
mod testes;

/// As células que mudaram desde a última pergunta: as escritas e as fórmulas
/// que a cascata sujou. É o que deixa o gráfico manter o cache quando a
/// edição foi longe do intervalo dele (o `notarMudanca` do `ui/grafico/chart.ts`,
/// processo crítico). Quem mexe em massa por fora desses caminhos — ordenar,
/// inserir linha, estilo de coluna — declara o rastro **desconhecido**, e aí
/// quem perguntar relê tudo. Passando do teto, também.
///
/// Nasce desconhecido: numa aba recém-criada ou carregada, "nada mudou"
/// seria mentira.
#[derive(Debug)]
pub struct Rastro {
    chaves: Vec<CellKey>,
    desconhecido: bool,
}

impl Default for Rastro {
    fn default() -> Self {
        Rastro { chaves: Vec::new(), desconhecido: true }
    }
}

/// Acima disso a lista não ajuda ninguém: é "mudou muita coisa".
const TETO_DO_RASTRO: usize = 20_000;

impl Rastro {
    pub fn anotar(&mut self, k: CellKey) {
        if self.desconhecido {
            return;
        }
        if self.chaves.len() >= TETO_DO_RASTRO {
            self.desconhecer();
            return;
        }
        self.chaves.push(k);
    }

    pub fn desconhecer(&mut self) {
        self.desconhecido = true;
        self.chaves = Vec::new();
    }

    /// Um rastro vazio e sabido: para medir só o que uma ação toca.
    pub fn sabido() -> Self {
        Rastro { chaves: Vec::new(), desconhecido: false }
    }

    /// Soma ao rastro o que [`Self::tirar`] devolveu de outro.
    pub fn juntar(&mut self, tocadas: Option<&[CellKey]>) {
        match tocadas {
            Some(ks) => for &k in ks { self.anotar(k) },
            None => self.desconhecer(),
        }
    }

    /// As chaves tocadas (sem repetição), ou `None` quando não se sabe; e
    /// recomeça do zero, sabendo.
    pub fn tirar(&mut self) -> Option<Vec<CellKey>> {
        let desconhecido = std::mem::replace(&mut self.desconhecido, false);
        let mut chaves = std::mem::take(&mut self.chaves);
        if desconhecido {
            return None;
        }
        chaves.sort_unstable();
        chaves.dedup();
        Some(chaves)
    }
}
