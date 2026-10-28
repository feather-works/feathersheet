//! Painel "Formatar gráfico": grupos em acordeão (um aberto por vez), todos
//! editando o `EstiloGrafico` do motor comum AO VIVO (cada edição grava no
//! gráfico e é um Ctrl+Z; letras seguidas num campo juntam num passo).
//!
//! Peças independentes do Sheet (aplicar uma [`Mudanca`], ler/gravar a cor
//! de um [`AlvoCor`]) ficam em funções puras, prontas para irem ao crate
//! comum quando o FeatherSlide/FeatherDocs usarem o mesmo painel.
use super::*;
use feather_charts::analitico::{DirecaoErro, QuantiaErro, TipoTendencia};
use feather_charts::estilo::{
    Alinhamento, CorEstilo, EstiloGrafico, FormaMarcador, FormatoNum, Linha, Margens,
    PosicaoLegenda, PosicaoRotulo, Preenchimento, Sombra, TemaGrafico, Traco,
};
use crate::compat::pick_list_vec as pick_list; use pelicano::svg; use pelicano::tooltip; use pelicano::TooltipPosition;
use serde_json::{Map, Value};

// ------------------------------------------------------------------ tipos

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    TipoEstilo,
    Dados,
    Titulo,
    Legenda,
    Eixos,
    Grades,
    Series,
    Pontos,
    Rotulos,
    Area,
    Margens,
    Fonte,
    Analiticos,
    Dica,
    Formas,
    Tamanho,
    TextoAlt,
}
impl Grupo {
    /// Os grupos da aba Personalizar (Dados virou a aba Configurar).
    pub const PERSONALIZAR: [Self; 16] = [
        Self::TipoEstilo, Self::Titulo, Self::Legenda, Self::Eixos, Self::Grades,
        Self::Series, Self::Pontos, Self::Rotulos, Self::Area, Self::Margens, Self::Fonte,
        Self::Analiticos, Self::Dica, Self::Formas, Self::Tamanho, Self::TextoAlt,
    ];
    pub const TODOS: [Self; 17] = [
        Self::TipoEstilo, Self::Dados, Self::Titulo, Self::Legenda, Self::Eixos, Self::Grades,
        Self::Series, Self::Pontos, Self::Rotulos, Self::Area, Self::Margens, Self::Fonte,
        Self::Analiticos, Self::Dica, Self::Formas, Self::Tamanho, Self::TextoAlt,
    ];
    fn nome(self) -> &'static str {
        match self {
            Self::TipoEstilo => tr!("Tipo e estilo"),
            Self::Dados => tr!("Dados"),
            Self::Titulo => tr!("Título"),
            Self::Legenda => tr!("Legenda"),
            Self::Eixos => tr!("Eixos"),
            Self::Grades => tr!("Linhas de grade"),
            Self::Series => tr!("Séries"),
            Self::Pontos => tr!("Pontos"),
            Self::Rotulos => tr!("Rótulos de dados"),
            Self::Area => tr!("Área e fundo"),
            Self::Margens => tr!("Margens e espaçamento"),
            Self::Fonte => tr!("Fonte"),
            Self::Analiticos => tr!("Elementos analíticos"),
            Self::Dica => tr!("Dica do mouse"),
            Self::Formas => tr!("Formas no gráfico"),
            Self::Tamanho => tr!("Tamanho e posição"),
            Self::TextoAlt => tr!("Texto alternativo"),
        }
    }
    fn icone(self) -> &'static str {
        match self {
            Self::TipoEstilo => "palette",
            Self::Dados => "table",
            Self::Titulo => "text-size",
            Self::Legenda => "list",
            Self::Eixos => "arrows-horizontal",
            Self::Grades => "layout-grid",
            Self::Series => "chart-line",
            Self::Pontos => "circle-dot",
            Self::Rotulos => "tag",
            Self::Area => "bucket",
            Self::Margens => "arrows-maximize",
            Self::Fonte => "bold",
            Self::Analiticos => "sparkline-line",
            Self::Dica => "pointer",
            Self::Formas => "shapes",
            Self::Tamanho => "arrow-autofit-width",
            Self::TextoAlt => "message-circle",
        }
    }
    /// O grupo vale para o tipo: o que o desenho do tipo não lê não aparece
    /// (antes abria e dizia "a pizza não tem eixos").
    pub(super) fn existe_em(self, t: TipoUi) -> bool {
        use TipoUi as T;
        match self {
            Self::Eixos => !matches!(t, T::Pizza | T::Rosca | T::MapaCalor | T::Linha3d),
            // Radar, Gantt, mapa de calor e 3D pintam a grade pelo tema.
            Self::Grades => !matches!(t, T::Pizza | T::Rosca | T::Radar | T::Gantt | T::MapaCalor | T::Linha3d),
            Self::Legenda => !matches!(t, T::Cascata | T::Candlestick | T::Gantt | T::MapaCalor | T::Histograma),
            // Cor por ponto: só onde o desenho pinta cada ponto.
            Self::Pontos => !matches!(t, T::Radar | T::Candlestick | T::Cascata | T::MapaCalor | T::Linha3d),
            // Tendência, erro e referência são do desenho cartesiano comum.
            Self::Analiticos => !matches!(t, T::Pizza | T::Rosca | T::Combinado | T::Histograma | T::Cascata | T::Radar
                | T::Bolhas | T::Candlestick | T::Gantt | T::MapaCalor | T::Linha3d),
            _ => true,
        }
    }
    /// Sem motor ainda: aparece desabilitado com a dica.
    fn em_breve(self) -> Option<&'static str> {
        match self {
            Self::Formas => Some(tr!("Em breve: círculo, seta e texto livre dentro do gráfico")),
            _ => None,
        }
    }
}

/// As duas abas do painel: o que entra no gráfico e como ele aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbaPainel {
    #[default]
    Configurar,
    Personalizar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EixoSel {
    X,
    #[default]
    Y,
    Y2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaSel {
    #[default]
    Grafico,
    Plotagem,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Superior,
    Inferior,
    Esquerda,
    Direita,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampoEixo {
    Minimo,
    Maximo,
    Principal,
    Secundaria,
    Cruzamento,
    Rotacao,
    Casas,
    EspessuraLinha,
    TamanhoRotulos,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoPreench {
    Automatico,
    Nenhum,
    Solido,
    Degrade,
}

/// O que recebe a cor escolhida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCor {
    Fundo(AreaSel),
    DegradeFim(AreaSel),
    Borda(AreaSel),
    FonteGeral,
    Titulo,
    Legenda,
    ItemLegenda(usize),
    RotulosEixo(EixoSel),
    LinhaEixo(EixoSel),
    Grade(EixoSel, bool),
    Serie(usize),
    Contorno(usize),
    Ponto(usize, usize),
    Rotulos,
    DicaFundo,
    DicaTexto,
}

/// Uma edição do estilo (o que o painel manda).
#[derive(Debug, Clone)]
pub enum Mudanca {
    Tema(Option<&'static str>),
    TituloVisivel(bool),
    TituloAlinhar(Alinhamento),
    TituloSobreposto(bool),
    TituloTamanho(String),
    TituloNegrito(bool),
    Subtitulo(String),
    LegendaPosicao(PosicaoLegenda),
    LegendaTamanho(String),
    ItemTexto(usize, String),
    ItemOculto(usize, bool),
    ItemMover(usize, i32, usize),
    EixoVisivel(EixoSel, bool),
    EixoNum(EixoSel, CampoEixo, String),
    EixoLog(EixoSel, bool),
    EixoInvertido(EixoSel, bool),
    EixoFormato(EixoSel, FormatoNum),
    EixoTitulo(EixoSel, String),
    EixoUnidade(String),
    EixoLinha(EixoSel, bool),
    GradeVisivel(EixoSel, bool, bool),
    GradeEspessura(EixoSel, bool, String),
    GradeTraco(EixoSel, bool, Traco),
    SerieEspessura(usize, String),
    SerieTraco(usize, Traco),
    SerieMarcador(usize, FormaMarcador),
    SerieTamanhoMarcador(usize, String),
    SerieSuavizar(usize, bool),
    SerieSecundario(usize, bool),
    SerieTransparencia(usize, String),
    SerieContornoEspessura(usize, String),
    PontoLimpar(usize, usize),
    PontoDestacar(usize, usize, String),
    RotulosVisivel(bool),
    RotulosPosicao(PosicaoRotulo),
    RotulosFormato(FormatoNum),
    RotulosCasas(String),
    RotulosValor(bool),
    RotulosSerie(bool),
    RotulosCategoria(bool),
    RotulosPercentual(bool),
    RotulosTamanho(String),
    AreaTipo(AreaSel, TipoPreench),
    AreaAngulo(AreaSel, String),
    AreaBorda(AreaSel, bool),
    AreaBordaEspessura(AreaSel, String),
    AreaBordaTraco(AreaSel, Traco),
    AreaSombra(AreaSel, bool),
    AreaRaio(AreaSel, String),
    Margem(AreaSel, Lado, String),
    BarrasEspaco(String),
    BarrasSobrepor(String),
    BarrasRaio(String),
    FonteFamilia(Option<&'static str>),
    FonteTamanho(String),
    FonteNegrito(bool),
    FonteItalico(bool),
    TextoAlt(String),
    /// `None` tira a linha de tendência.
    Tendencia(usize, Option<TipoTendencia>),
    TendenciaGrau(usize, String),
    TendenciaPeriodo(usize, String),
    TendenciaAvancar(usize, String),
    TendenciaRecuar(usize, String),
    TendenciaEquacao(usize, bool),
    TendenciaR2(usize, bool),
    /// Tipo de barra de erro (0 fixo, 1 porcentagem, 2 desvio-padrão, 3 erro-padrão); `None` tira.
    Erro(usize, Option<u8>),
    ErroValor(usize, String),
    ErroDirecao(usize, DirecaoErro),
    ErroSemPonta(usize, bool),
    /// Linha de referência (0 máximo, 1 mínimo, 2 média).
    Referencia(usize, u8, bool),
    DicaVisivel(bool),
    DicaCategoria(bool),
    DicaSerie(bool),
    DicaPercentual(bool),
    /// `None` = o formato dos rótulos.
    DicaFormato(Option<FormatoNum>),
    DicaPrefixo(String),
    DicaSufixo(String),
    DicaTamanho(String),
}

#[derive(Debug, Clone, Copy)]
pub enum Selecao {
    Eixo(EixoSel),
    Serie(usize),
    Ponto(usize),
    Area(AreaSel),
}

/// Estado só da interface (o estilo mora na `Aparencia`).
#[derive(Debug, Default)]
pub struct EstadoPainel {
    pub aba: AbaPainel,
    pub grupo: Option<Grupo>,
    pub eixo: EixoSel,
    pub serie: usize,
    pub ponto: usize,
    pub area: AreaSel,
    pub cor_menu: Option<AlvoCor>,
    /// Texto digitado nos campos numéricos (pode estar pela metade, "1,").
    pub rascunhos: std::collections::HashMap<String, String>,
    /// `_excel` do gráfico aberto: entra na conversão para o estilo novo.
    pub excel: Option<Map<String, Value>>,
    /// Cores das séries antes do estilo (as da planilha), para as amostras.
    pub cores_base: Vec<feather_draw::Cor>,
    /// Categorias dos dados (itens da legenda da pizza e lista de pontos).
    pub categorias: Vec<String>,
}

const FAMILIAS: [(&str, Option<&str>); 4] = [
    ("Padrão", None),
    ("Sem serifa", Some("Noto Sans, DejaVu Sans, Liberation Sans, Arial, sans-serif")),
    ("Com serifa", Some("Noto Serif, DejaVu Serif, Liberation Serif, Georgia, serif")),
    ("Monoespaçada", Some("DejaVu Sans Mono, Liberation Mono, Courier New, monospace")),
];
const PALETA_PADRAO: [u32; 14] = [
    0x000000, 0x404040, 0x7f7f7f, 0xbfbfbf, 0xffffff, 0x1f3a5f, 0x0b6e99,
    0xc62828, 0xe8710a, 0xfbbc04, 0x188038, 0x2aa9c8, 0x7b61ff, 0xd81b60,
];

/// Item de lista suspensa com valor e o nome que aparece.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Opcao<T> {
    valor: T,
    nome: String,
}
impl<T> std::fmt::Display for Opcao<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.nome)
    }
}

impl TipoUi {
    /// Desenha uma coluna de valores só: a aba Configurar oferece escolha
    /// única (pizza: o valor de cada fatia).
    pub(super) fn valor_unico(self) -> bool {
        matches!(self, Self::Pizza | Self::Histograma | Self::Cascata)
    }
    /// Os papéis das séries nos tipos que as leem pela posição, na ordem do
    /// desenho (ver `ordenar_papeis`).
    pub(super) fn papeis(self) -> &'static [&'static str] {
        match self {
            Self::Bolhas => &["Y", "Tamanho"],
            Self::Candlestick => &["Abertura", "Máxima", "Mínima", "Fechamento"],
            Self::Gantt => &["Início", "Fim"],
            _ => &[],
        }
    }
    /// O nome do campo das categorias na aba Configurar; `None` = o tipo
    /// não usa categorias. `bool`: aceita "nenhuma".
    fn campo_categoria(self) -> Option<(&'static str, bool)> {
        Some(match self {
            Self::Pizza | Self::Rosca => (tr!("Rótulos (fatias)"), false),
            Self::Dispersao => (tr!("X (número)"), true),
            Self::Bolhas => (tr!("X (número)"), false),
            Self::Cascata => (tr!("Categoria"), false),
            Self::Radar => (tr!("Eixos (categorias)"), true),
            Self::Candlestick => (tr!("Data"), false),
            Self::Gantt => (tr!("Tarefa"), false),
            Self::MapaCalor => (tr!("Linhas"), false),
            Self::Histograma | Self::Linha3d => return None,
            _ => (tr!("Eixo X (categorias)"), true),
        })
    }
    /// O nome da lista de séries (os tipos de várias séries).
    fn campo_series(self) -> &'static str {
        match self {
            Self::Dispersao => tr!("Y"),
            Self::MapaCalor => tr!("Colunas"),
            Self::Rosca => tr!("Séries (um anel cada)"),
            _ => tr!("Séries"),
        }
    }
}

/// Como cada série mostra as categorias iguais (Configurar > Séries).
const CONTAS: [(&str, Option<AggregateFn>); 8] = [
    ("Não resumir", None),
    ("Soma", Some(AggregateFn::Sum)),
    ("Média", Some(AggregateFn::Avg)),
    ("Contar", Some(AggregateFn::Count)),
    ("Mínimo", Some(AggregateFn::Min)),
    ("Máximo", Some(AggregateFn::Max)),
    ("Desvio padrão", Some(AggregateFn::Stddev)),
    ("Moda", Some(AggregateFn::Mode)),
];

/// Série de texto não tem soma nem média: só contar (pedido de 16/10).
const CONTAS_TEXTO: [(&str, Option<AggregateFn>); 3] = [
    ("Não resumir", None),
    ("Contar", Some(AggregateFn::Count)),
    ("Contar distintos", Some(AggregateFn::CountDistinct)),
];

/// As contas que a lista de uma série oferece: a de texto só conta; acima de
/// 200 mil células (`resumo_obrigatorio`) "Não resumir" sai.
pub(super) fn contas_da_serie(texto: bool, resumo_obrigatorio: bool) -> Vec<(&'static str, Option<AggregateFn>)> {
    let tabela: &[(&'static str, Option<AggregateFn>)] = if texto { &CONTAS_TEXTO } else { &CONTAS };
    tabela.iter().copied().filter(|(_, valor)| valor.is_some() || !resumo_obrigatorio).collect()
}

/// Quantas células de cada série olhar, no máximo, para saber se ela é de
/// texto quando a tabela passa de 200 mil células (espalhadas pela série).
const AMOSTRA_TIPO: u32 = 2048;

/// A célula tem letra: texto que não é número (`"12"` e `"1,5"` são números).
fn celula_de_texto(v: &feathersheet_core::celulas::values::Value) -> bool {
    match v {
        feathersheet_core::celulas::values::Value::Text(t) => {
            let t = t.trim();
            !t.is_empty() && t.parse::<f64>().is_err() && t.replacen(',', ".", 1).parse::<f64>().is_err()
        }
        _ => false,
    }
}

// ------------------------------------------------------------ funções puras

/// Passo das setas/roda e a faixa de cada campo, pela chave dele.
fn limites_do_campo(chave: &str) -> feather_ui::campo_numero::Limites {
    use feather_ui::campo_numero::Limites;
    let c = chave.to_lowercase();
    let l = |passo: f64, min: Option<f64>, max: Option<f64>| Limites { passo, min, max };
    if c.contains("transparencia") { l(5.0, Some(0.0), Some(100.0)) }
    else if c.contains("espessura") || c.contains("contorno") || c.contains("borda") || c.contains("grade") { l(0.5, Some(0.0), None) }
    else if c.contains("casas") { l(1.0, Some(0.0), Some(10.0)) }
    else if c.contains("tamanho") || c.contains("marcador") { l(1.0, Some(1.0), None) }
    else if c.contains("rotacao") || c.contains("angulo") { l(15.0, Some(-360.0), Some(360.0)) }
    else if c.contains("espaco") || c.contains("sobrepor") { l(10.0, Some(-100.0), Some(500.0)) }
    else if c.contains("margem") || c.contains("raio") || c.contains("destacar") { l(1.0, Some(0.0), None) }
    else { l(1.0, None, None) }
}

/// `""` = automático; vírgula ou ponto decimal.
fn numero(s: &str) -> Result<Option<f64>, ()> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some).ok_or(())
}
fn numero_f32(s: &str) -> Result<Option<f32>, ()> {
    numero(s).map(|v| v.map(|x| x as f32))
}

fn eixo_mut(e: &mut EstiloGrafico, s: EixoSel) -> &mut feather_charts::estilo::Eixo {
    match s {
        EixoSel::X => &mut e.eixo_x,
        EixoSel::Y => &mut e.eixo_y,
        EixoSel::Y2 => &mut e.eixo_y2,
    }
}
fn eixo(e: &EstiloGrafico, s: EixoSel) -> &feather_charts::estilo::Eixo {
    match s {
        EixoSel::X => &e.eixo_x,
        EixoSel::Y => &e.eixo_y,
        EixoSel::Y2 => &e.eixo_y2,
    }
}
fn area_mut(e: &mut EstiloGrafico, a: AreaSel) -> &mut feather_charts::estilo::Area {
    match a {
        AreaSel::Grafico => &mut e.area_grafico,
        AreaSel::Plotagem => &mut e.area_plotagem,
    }
}
fn area(e: &EstiloGrafico, a: AreaSel) -> &feather_charts::estilo::Area {
    match a {
        AreaSel::Grafico => &e.area_grafico,
        AreaSel::Plotagem => &e.area_plotagem,
    }
}
fn margens_mut(e: &mut EstiloGrafico, a: AreaSel) -> &mut Margens {
    match a {
        AreaSel::Grafico => &mut e.margens_grafico,
        AreaSel::Plotagem => &mut e.margens_plotagem,
    }
}
fn grade_mut(e: &mut EstiloGrafico, s: EixoSel, principal: bool) -> &mut Linha {
    let x = eixo_mut(e, s);
    if principal { &mut x.grade_principal } else { &mut x.grade_secundaria }
}

/// Chave do campo de texto (junta letras num passo de Ctrl+Z e guarda o rascunho).
pub fn chave(m: &Mudanca) -> Option<String> {
    Some(match m {
        Mudanca::TituloTamanho(_) => "titulo.tamanho".into(),
        Mudanca::Subtitulo(_) => "subtitulo".into(),
        Mudanca::LegendaTamanho(_) => "legenda.tamanho".into(),
        Mudanca::ItemTexto(i, _) => format!("legenda.item{i}"),
        Mudanca::EixoNum(s, c, _) => format!("eixo{s:?}.{c:?}"),
        Mudanca::EixoTitulo(s, _) => format!("eixo{s:?}.titulo"),
        Mudanca::EixoUnidade(_) => "eixo.unidade".into(),
        Mudanca::GradeEspessura(s, p, _) => format!("grade{s:?}{p}"),
        Mudanca::SerieEspessura(i, _) => format!("serie{i}.espessura"),
        Mudanca::SerieTamanhoMarcador(i, _) => format!("serie{i}.marcador"),
        Mudanca::SerieTransparencia(i, _) => format!("serie{i}.transparencia"),
        Mudanca::SerieContornoEspessura(i, _) => format!("serie{i}.contorno"),
        Mudanca::PontoDestacar(s, p, _) => format!("ponto{s}.{p}.destacar"),
        Mudanca::RotulosCasas(_) => "rotulos.casas".into(),
        Mudanca::RotulosTamanho(_) => "rotulos.tamanho".into(),
        Mudanca::AreaAngulo(a, _) => format!("area{a:?}.angulo"),
        Mudanca::AreaBordaEspessura(a, _) => format!("area{a:?}.borda"),
        Mudanca::AreaRaio(a, _) => format!("area{a:?}.raio"),
        Mudanca::Margem(a, l, _) => format!("margem{a:?}{l:?}"),
        Mudanca::BarrasEspaco(_) => "barras.espaco".into(),
        Mudanca::BarrasSobrepor(_) => "barras.sobrepor".into(),
        Mudanca::BarrasRaio(_) => "barras.raio".into(),
        Mudanca::FonteTamanho(_) => "fonte.tamanho".into(),
        Mudanca::TextoAlt(_) => "texto_alt".into(),
        Mudanca::TendenciaGrau(i, _) => format!("tendencia{i}.grau"),
        Mudanca::TendenciaPeriodo(i, _) => format!("tendencia{i}.periodo"),
        Mudanca::TendenciaAvancar(i, _) => format!("tendencia{i}.avancar"),
        Mudanca::TendenciaRecuar(i, _) => format!("tendencia{i}.recuar"),
        Mudanca::ErroValor(i, _) => format!("erro{i}.valor"),
        Mudanca::DicaPrefixo(_) => "dica.prefixo".into(),
        Mudanca::DicaSufixo(_) => "dica.sufixo".into(),
        Mudanca::DicaTamanho(_) => "dica.tamanho".into(),
        _ => return None,
    })
}
fn texto_da(m: &Mudanca) -> Option<&str> {
    match m {
        Mudanca::TituloTamanho(s) | Mudanca::Subtitulo(s) | Mudanca::LegendaTamanho(s) | Mudanca::ItemTexto(_, s)
        | Mudanca::EixoNum(_, _, s) | Mudanca::EixoTitulo(_, s) | Mudanca::EixoUnidade(s) | Mudanca::GradeEspessura(_, _, s)
        | Mudanca::SerieEspessura(_, s) | Mudanca::SerieTamanhoMarcador(_, s) | Mudanca::SerieTransparencia(_, s)
        | Mudanca::SerieContornoEspessura(_, s) | Mudanca::PontoDestacar(_, _, s) | Mudanca::RotulosCasas(s)
        | Mudanca::RotulosTamanho(s) | Mudanca::AreaAngulo(_, s) | Mudanca::AreaBordaEspessura(_, s) | Mudanca::AreaRaio(_, s)
        | Mudanca::Margem(_, _, s) | Mudanca::BarrasEspaco(s) | Mudanca::BarrasSobrepor(s) | Mudanca::BarrasRaio(s)
        | Mudanca::FonteTamanho(s) | Mudanca::TextoAlt(s)
        | Mudanca::TendenciaGrau(_, s) | Mudanca::TendenciaPeriodo(_, s) | Mudanca::TendenciaAvancar(_, s)
        | Mudanca::TendenciaRecuar(_, s) | Mudanca::ErroValor(_, s)
        | Mudanca::DicaPrefixo(s) | Mudanca::DicaSufixo(s) | Mudanca::DicaTamanho(s) => Some(s),
        _ => None,
    }
}

/// Aplica a mudança. `false` = texto ainda inválido (nada muda).
pub fn aplicar(e: &mut EstiloGrafico, m: &Mudanca) -> bool {
    let tam = |s: &str| numero_f32(s).map(|v| v.filter(|x| *x > 0.0).map(|x| x.clamp(4.0, 72.0)));
    let nao_neg = |s: &str| numero_f32(s).map(|v| v.map(|x| x.max(0.0)));
    macro_rules! ok {
        ($r:expr, $v:ident => $corpo:expr) => {
            match $r {
                Ok($v) => {
                    $corpo;
                    true
                }
                Err(()) => false,
            }
        };
    }
    match m.clone() {
        Mudanca::Tendencia(i, tipo) => {
            let a = &mut e.serie_mut(i).analise;
            match tipo {
                None => a.tendencia = None,
                Some(t) => a.tendencia.get_or_insert_with(Default::default).tipo = t,
            }
        }
        Mudanca::TendenciaGrau(i, s) => return ok!(numero(&s), v => {
            if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.grau = v.unwrap_or(2.0).round().clamp(2.0, 6.0) as u8; }
        }),
        Mudanca::TendenciaPeriodo(i, s) => return ok!(numero(&s), v => {
            if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.periodo = v.unwrap_or(2.0).round().clamp(2.0, 255.0) as u8; }
        }),
        Mudanca::TendenciaAvancar(i, s) => return ok!(numero(&s), v => {
            if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.avancar = v.unwrap_or(0.0).max(0.0); }
        }),
        Mudanca::TendenciaRecuar(i, s) => return ok!(numero(&s), v => {
            if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.recuar = v.unwrap_or(0.0).max(0.0); }
        }),
        Mudanca::TendenciaEquacao(i, v) => if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.mostrar_equacao = v },
        Mudanca::TendenciaR2(i, v) => if let Some(t) = e.serie_mut(i).analise.tendencia.as_mut() { t.mostrar_r2 = v },
        Mudanca::Erro(i, tipo) => {
            let a = &mut e.serie_mut(i).analise;
            match tipo {
                None => a.erro = None,
                Some(k) => a.erro.get_or_insert_with(Default::default).quantia = match k {
                    0 => QuantiaErro::Fixo(1.0),
                    1 => QuantiaErro::Percentual(5.0),
                    2 => QuantiaErro::DesvioPadrao(1.0),
                    _ => QuantiaErro::ErroPadrao,
                },
            }
        }
        Mudanca::ErroValor(i, s) => return ok!(numero(&s), v => {
            if let Some(b) = e.serie_mut(i).analise.erro.as_mut() {
                let v = v.unwrap_or(0.0).abs();
                b.quantia = match b.quantia {
                    QuantiaErro::Fixo(_) => QuantiaErro::Fixo(v),
                    QuantiaErro::Percentual(_) => QuantiaErro::Percentual(v),
                    QuantiaErro::DesvioPadrao(_) => QuantiaErro::DesvioPadrao(v),
                    QuantiaErro::ErroPadrao => QuantiaErro::ErroPadrao,
                };
            }
        }),
        Mudanca::ErroDirecao(i, d) => if let Some(b) = e.serie_mut(i).analise.erro.as_mut() { b.direcao = d },
        Mudanca::ErroSemPonta(i, v) => if let Some(b) = e.serie_mut(i).analise.erro.as_mut() { b.sem_ponta = v },
        Mudanca::Referencia(i, k, v) => {
            let a = &mut e.serie_mut(i).analise;
            match k { 0 => a.maximo = v, 1 => a.minimo = v, _ => a.media = v }
        }
        Mudanca::DicaVisivel(v) => e.dica.visivel = Some(v),
        Mudanca::DicaCategoria(v) => e.dica.categoria = Some(v),
        Mudanca::DicaSerie(v) => e.dica.serie = Some(v),
        Mudanca::DicaPercentual(v) => e.dica.percentual = v,
        Mudanca::DicaFormato(f) => e.dica.formato = f,
        Mudanca::DicaPrefixo(t) => e.dica.prefixo = t,
        Mudanca::DicaSufixo(t) => e.dica.sufixo = t,
        Mudanca::DicaTamanho(t) => return ok!(tam(&t), v => e.dica.tamanho = v),
        Mudanca::Tema(id) => e.aplicar_tema(id),
        Mudanca::TituloVisivel(v) => e.titulo.visivel = Some(v),
        Mudanca::TituloAlinhar(a) => e.titulo.alinhamento = Some(a),
        Mudanca::TituloSobreposto(v) => e.titulo.sobreposto = v,
        Mudanca::TituloTamanho(s) => return ok!(tam(&s), v => e.titulo.fonte.tamanho = v),
        Mudanca::TituloNegrito(v) => e.titulo.fonte.negrito = Some(v),
        Mudanca::Subtitulo(s) => e.subtitulo.texto = Some(s).filter(|t| !t.is_empty()),
        Mudanca::LegendaPosicao(p) => e.legenda.posicao = Some(p),
        Mudanca::LegendaTamanho(s) => return ok!(tam(&s), v => e.legenda.fonte.tamanho = v),
        Mudanca::ItemTexto(i, s) => e.legenda.itens.entry(i).or_default().texto = Some(s).filter(|t| !t.is_empty()),
        Mudanca::ItemOculto(i, v) => e.legenda.itens.entry(i).or_default().oculto = v,
        Mudanca::ItemMover(i, delta, total) => {
            let mut ordem = e.legenda.ordem_completa(total);
            if let Some(p) = ordem.iter().position(|x| *x == i) {
                let q = (p as i32 + delta).clamp(0, total as i32 - 1) as usize;
                ordem.swap(p, q);
                e.legenda.ordem = ordem;
            }
        }
        Mudanca::EixoVisivel(s, v) => eixo_mut(e, s).visivel = Some(v),
        Mudanca::EixoNum(s, c, t) => {
            let x = eixo_mut(e, s);
            return match c {
                CampoEixo::Minimo => ok!(numero(&t), v => x.minimo = v),
                CampoEixo::Maximo => ok!(numero(&t), v => x.maximo = v),
                CampoEixo::Principal => ok!(numero(&t), v => x.unidade_principal = v.filter(|u| *u > 0.0)),
                CampoEixo::Secundaria => ok!(numero(&t), v => x.unidade_secundaria = v.filter(|u| *u > 0.0)),
                CampoEixo::Cruzamento => ok!(numero(&t), v => x.cruzamento = v),
                CampoEixo::Rotacao => ok!(numero_f32(&t), v => x.rotacao = v.map(|g| g.clamp(-90.0, 90.0))),
                CampoEixo::Casas => ok!(numero(&t), v => {
                    let f = x.formato.unwrap_or(FormatoNum::Decimal { casas: 0 });
                    let f = if f == FormatoNum::Automatico { FormatoNum::Decimal { casas: 0 } } else { f };
                    x.formato = Some(f.com_casas(v.unwrap_or(0.0).clamp(0.0, 6.0) as u8));
                }),
                CampoEixo::EspessuraLinha => ok!(nao_neg(&t), v => x.linha.espessura = v),
                CampoEixo::TamanhoRotulos => ok!(tam(&t), v => x.rotulos.tamanho = v),
            };
        }
        Mudanca::EixoLog(s, v) => eixo_mut(e, s).logaritmica = v,
        Mudanca::EixoInvertido(s, v) => eixo_mut(e, s).invertido = v,
        Mudanca::EixoFormato(s, f) => {
            let x = eixo_mut(e, s);
            let casas = x.formato.map_or(if matches!(f, FormatoNum::Moeda { .. }) { 2 } else { 0 }, FormatoNum::casas);
            x.formato = Some(f.com_casas(casas));
        }
        Mudanca::EixoTitulo(s, t) => eixo_mut(e, s).titulo = Some(t).filter(|t| !t.is_empty()),
        Mudanca::EixoUnidade(t) => e.eixo_y.unidade = Some(t).filter(|t| !t.is_empty()),
        Mudanca::EixoLinha(s, v) => eixo_mut(e, s).linha.visivel = Some(v),
        Mudanca::GradeVisivel(s, p, v) => grade_mut(e, s, p).visivel = Some(v),
        Mudanca::GradeEspessura(s, p, t) => return ok!(nao_neg(&t), v => grade_mut(e, s, p).espessura = v),
        Mudanca::GradeTraco(s, p, t) => grade_mut(e, s, p).traco = Some(t),
        Mudanca::SerieEspessura(i, t) => return ok!(nao_neg(&t), v => e.serie_mut(i).espessura_linha = v),
        Mudanca::SerieTraco(i, t) => e.serie_mut(i).traco = Some(t),
        Mudanca::SerieMarcador(i, f) => e.serie_mut(i).marcador = Some(f),
        Mudanca::SerieTamanhoMarcador(i, t) => return ok!(numero_f32(&t), v => e.serie_mut(i).tamanho_marcador = v.map(|x| x.clamp(1.0, 40.0))),
        Mudanca::SerieSuavizar(i, v) => e.serie_mut(i).suavizar = Some(v),
        Mudanca::SerieSecundario(i, v) => e.serie_mut(i).eixo_secundario = v,
        Mudanca::SerieTransparencia(i, t) => return ok!(numero_f32(&t), v => e.serie_mut(i).opacidade = v.map(|p| 1.0 - p.clamp(0.0, 100.0) / 100.0)),
        Mudanca::SerieContornoEspessura(i, t) => return ok!(nao_neg(&t), v => e.serie_mut(i).espessura_contorno = v),
        Mudanca::PontoLimpar(s, p) => {
            e.serie_mut(s).pontos.remove(&p);
        }
        Mudanca::PontoDestacar(s, p, t) => return ok!(nao_neg(&t), v => e.serie_mut(s).pontos.entry(p).or_default().destacar = v),
        Mudanca::RotulosVisivel(v) => e.rotulos.visivel = Some(v),
        Mudanca::RotulosPosicao(p) => e.rotulos.posicao = Some(p),
        Mudanca::RotulosFormato(f) => {
            let casas = e.rotulos.formato.map_or(if matches!(f, FormatoNum::Moeda { .. }) { 2 } else { 0 }, FormatoNum::casas);
            e.rotulos.formato = Some(f.com_casas(casas));
        }
        Mudanca::RotulosCasas(t) => return ok!(numero(&t), v => {
            let f = e.rotulos.formato.filter(|f| *f != FormatoNum::Automatico).unwrap_or(FormatoNum::Decimal { casas: 0 });
            e.rotulos.formato = Some(f.com_casas(v.unwrap_or(0.0).clamp(0.0, 6.0) as u8));
        }),
        Mudanca::RotulosValor(v) => e.rotulos.valor = Some(v),
        Mudanca::RotulosSerie(v) => e.rotulos.nome_serie = Some(v),
        Mudanca::RotulosCategoria(v) => e.rotulos.categoria = Some(v),
        Mudanca::RotulosPercentual(v) => e.rotulos.percentual = Some(v),
        Mudanca::RotulosTamanho(t) => return ok!(tam(&t), v => e.rotulos.fonte.tamanho = v),
        Mudanca::AreaTipo(a, t) => {
            let atual = area(e, a).preenchimento.clone();
            let cor = match &atual {
                Some(Preenchimento::Solido { cor }) | Some(Preenchimento::Degrade { inicio: cor, .. }) => *cor,
                _ => e.tema().map_or(CorEstilo::hex(0xffffff), |t| if a == AreaSel::Grafico { t.fundo } else { t.plotagem.unwrap_or(t.fundo) }),
            };
            area_mut(e, a).preenchimento = match t {
                TipoPreench::Automatico => None,
                TipoPreench::Nenhum => Some(Preenchimento::Nenhum),
                TipoPreench::Solido => Some(Preenchimento::Solido { cor }),
                TipoPreench::Degrade => Some(Preenchimento::Degrade { inicio: cor, fim: CorEstilo::hex(0xdfe7f2), angulo: 90.0 }),
            };
        }
        Mudanca::AreaAngulo(a, t) => return ok!(numero_f32(&t), v => {
            if let Some(Preenchimento::Degrade { angulo, .. }) = &mut area_mut(e, a).preenchimento {
                *angulo = v.unwrap_or(90.0);
            }
        }),
        Mudanca::AreaBorda(a, v) => area_mut(e, a).borda.visivel = Some(v),
        Mudanca::AreaBordaEspessura(a, t) => return ok!(nao_neg(&t), v => area_mut(e, a).borda.espessura = v),
        Mudanca::AreaBordaTraco(a, t) => area_mut(e, a).borda.traco = Some(t),
        Mudanca::AreaSombra(a, v) => area_mut(e, a).sombra = v.then(Sombra::default),
        Mudanca::AreaRaio(a, t) => return ok!(nao_neg(&t), v => area_mut(e, a).raio = v),
        Mudanca::Margem(a, l, t) => return ok!(nao_neg(&t), v => {
            let m = margens_mut(e, a);
            *match l { Lado::Superior => &mut m.superior, Lado::Inferior => &mut m.inferior, Lado::Esquerda => &mut m.esquerda, Lado::Direita => &mut m.direita } = v.map(|x| x.min(400.0));
        }),
        Mudanca::BarrasEspaco(t) => return ok!(numero_f32(&t), v => e.barras.espacamento = v.map(|x| x.clamp(0.0, 500.0))),
        Mudanca::BarrasSobrepor(t) => return ok!(numero_f32(&t), v => e.barras.sobreposicao = v.map(|x| x.clamp(-100.0, 100.0))),
        Mudanca::BarrasRaio(t) => return ok!(nao_neg(&t), v => e.barras.raio = v),
        Mudanca::FonteFamilia(f) => e.fonte.familia = f.map(str::to_owned),
        Mudanca::FonteTamanho(t) => return ok!(tam(&t), v => e.fonte.tamanho = v),
        Mudanca::FonteNegrito(v) => e.fonte.negrito = Some(v),
        Mudanca::FonteItalico(v) => e.fonte.italico = Some(v),
        Mudanca::TextoAlt(t) => e.texto_alternativo = t,
    }
    true
}

/// A cor gravada para o alvo (`None` = automática).
pub fn cor_de(e: &EstiloGrafico, alvo: AlvoCor) -> Option<CorEstilo> {
    match alvo {
        AlvoCor::Fundo(a) => match &area(e, a).preenchimento {
            Some(Preenchimento::Solido { cor }) => Some(*cor),
            Some(Preenchimento::Degrade { inicio, .. }) => Some(*inicio),
            _ => None,
        },
        AlvoCor::DegradeFim(a) => match &area(e, a).preenchimento {
            Some(Preenchimento::Degrade { fim, .. }) => Some(*fim),
            _ => None,
        },
        AlvoCor::Borda(a) => area(e, a).borda.cor,
        AlvoCor::FonteGeral => e.fonte.cor,
        AlvoCor::Titulo => e.titulo.fonte.cor,
        AlvoCor::Legenda => e.legenda.fonte.cor,
        AlvoCor::ItemLegenda(i) => e.legenda.itens.get(&i).and_then(|it| it.cor),
        AlvoCor::RotulosEixo(s) => eixo(e, s).rotulos.cor,
        AlvoCor::LinhaEixo(s) => eixo(e, s).linha.cor,
        AlvoCor::Grade(s, p) => if p { eixo(e, s).grade_principal.cor } else { eixo(e, s).grade_secundaria.cor },
        AlvoCor::Serie(i) => e.serie(i).and_then(|s| s.cor),
        AlvoCor::Contorno(i) => e.serie(i).and_then(|s| s.contorno),
        AlvoCor::Ponto(s, p) => e.serie(s).and_then(|x| x.pontos.get(&p)).and_then(|q| q.cor),
        AlvoCor::Rotulos => e.rotulos.fonte.cor,
        AlvoCor::DicaFundo => e.dica.fundo,
        AlvoCor::DicaTexto => e.dica.texto,
    }
}

pub fn definir_cor(e: &mut EstiloGrafico, alvo: AlvoCor, cor: Option<CorEstilo>) {
    match alvo {
        AlvoCor::Fundo(a) => {
            let p = &mut area_mut(e, a).preenchimento;
            *p = match (p.take(), cor) {
                (Some(Preenchimento::Degrade { fim, angulo, .. }), Some(c)) => Some(Preenchimento::Degrade { inicio: c, fim, angulo }),
                (_, Some(c)) => Some(Preenchimento::Solido { cor: c }),
                (_, None) => None,
            };
        }
        AlvoCor::DegradeFim(a) => {
            if let (Some(Preenchimento::Degrade { fim, .. }), Some(c)) = (&mut area_mut(e, a).preenchimento, cor) {
                *fim = c;
            }
        }
        AlvoCor::Borda(a) => {
            let b = &mut area_mut(e, a).borda;
            b.cor = cor;
            if cor.is_some() {
                b.visivel = Some(true);
            }
        }
        AlvoCor::FonteGeral => e.fonte.cor = cor,
        AlvoCor::Titulo => e.titulo.fonte.cor = cor,
        AlvoCor::Legenda => e.legenda.fonte.cor = cor,
        AlvoCor::ItemLegenda(i) => e.legenda.itens.entry(i).or_default().cor = cor,
        AlvoCor::RotulosEixo(s) => eixo_mut(e, s).rotulos.cor = cor,
        AlvoCor::LinhaEixo(s) => eixo_mut(e, s).linha.cor = cor,
        AlvoCor::Grade(s, p) => {
            let g = grade_mut(e, s, p);
            g.cor = cor;
            if cor.is_some() {
                g.visivel = Some(true);
            }
        }
        AlvoCor::Serie(i) => e.serie_mut(i).cor = cor,
        AlvoCor::Contorno(i) => e.serie_mut(i).contorno = cor,
        AlvoCor::Ponto(s, p) => e.serie_mut(s).pontos.entry(p).or_default().cor = cor,
        AlvoCor::Rotulos => e.rotulos.fonte.cor = cor,
        AlvoCor::DicaFundo => e.dica.fundo = cor,
        AlvoCor::DicaTexto => e.dica.texto = cor,
    }
}

/// Estilo novo a partir dos campos antigos do gráfico (e do `_excel`).
pub fn migrar(a: &Aparencia, excel: Option<&Map<String, Value>>) -> EstiloGrafico {
    let mut e = EstiloGrafico::default();
    let cor = |s: &str| CorEstilo::de_texto(s);
    if let Ok(t) = a.fonte.trim().parse::<f32>() {
        if t.is_finite() && t > 0.0 {
            e.fonte.tamanho = Some(t.clamp(4.0, 72.0));
        }
    }
    if !a.titulo {
        e.titulo.visivel = Some(false);
    }
    e.subtitulo.texto = Some(a.subtitulo.clone()).filter(|s| !s.trim().is_empty());
    if !a.legenda {
        e.legenda.posicao = Some(PosicaoLegenda::Nenhuma);
    }
    if !a.eixos {
        e.eixo_x.visivel = Some(false);
        e.eixo_y.visivel = Some(false);
    }
    if a.valores {
        e.rotulos.visivel = Some(true);
    }
    e.eixo_x.titulo = Some(a.eixo_x.clone()).filter(|s| !s.trim().is_empty());
    e.eixo_y.titulo = Some(a.eixo_y.clone()).filter(|s| !s.trim().is_empty());
    e.eixo_y.unidade = Some(a.unidade.clone()).filter(|s| !s.trim().is_empty());
    if let Some(f) = super::formato_numero(&a.formato) {
        use feather_charts::FormatoNumero as F;
        let f = match f {
            F::Decimal(c) => FormatoNum::Decimal { casas: c },
            F::Moeda(c) => FormatoNum::Moeda { casas: c },
            F::Percentual(c) => FormatoNum::Percentual { casas: c },
        };
        e.eixo_y.formato = Some(f);
        e.rotulos.formato = Some(f);
    }
    if !a.fundo.eq_ignore_ascii_case("#ffffff") {
        e.area_grafico.preenchimento = cor(&a.fundo).map(|c| if c.0.a == 0 { Preenchimento::Nenhum } else { Preenchimento::Solido { cor: c } });
    }
    if let Some(x) = excel {
        let txt = |k: &str| x.get(k).and_then(Value::as_str).and_then(cor);
        if let Some(p) = x.get("fundoDegrade").and_then(Value::as_array).filter(|p| p.len() >= 2) {
            let mut paradas: Vec<&Value> = p.iter().collect();
            paradas.sort_by(|a, b| a[0].as_f64().unwrap_or(0.0).total_cmp(&b[0].as_f64().unwrap_or(0.0)));
            let ponta = |v: &Value| v[1].as_str().and_then(cor);
            if let (Some(i), Some(f)) = (ponta(paradas[0]), ponta(paradas[paradas.len() - 1])) {
                let angulo = x.get("fundoAngulo").and_then(Value::as_f64).unwrap_or(90.0) as f32;
                e.area_grafico.preenchimento = Some(Preenchimento::Degrade { inicio: i, fim: f, angulo });
            }
        } else if let Some(c) = txt("fundo") {
            e.area_grafico.preenchimento = Some(if c.0.a == 0 { Preenchimento::Nenhum } else { Preenchimento::Solido { cor: c } });
        } else if x.contains_key("fundo") {
            // `null` (importado por versão antiga): sem preenchimento.
            e.area_grafico.preenchimento = Some(Preenchimento::Nenhum);
        }
        if let Some(c) = txt("contorno") {
            e.area_grafico.borda = Linha { visivel: Some(true), cor: Some(c), espessura: Some(1.0), traco: None };
        }
        if let Some(c) = txt("corTexto") {
            e.fonte.cor = Some(c);
        }
        if let Some(c) = txt("corEixoValores") {
            e.eixo_y.rotulos.cor = Some(c);
        }
        if let Some(c) = txt("corRotulo") {
            e.rotulos.fonte.cor = Some(c);
        }
        if x.get("negritoRotulo").and_then(Value::as_bool) == Some(true) {
            e.rotulos.fonte.negrito = Some(true);
        }
        if x.get("eixoValores").and_then(Value::as_bool) == Some(false) {
            e.eixo_y.visivel = Some(false);
        }
        e.margens_grafico = Margens { superior: Some(6.0), inferior: Some(6.0), esquerda: Some(6.0), direita: Some(6.0) };
    }
    e
}

/// Os campos antigos refletindo o estilo (o gravador `.xlsx` lê esses).
pub fn sincronizar_legado(a: &mut Aparencia) {
    let Some(e) = a.estilo.clone() else { return };
    a.titulo = e.titulo.visivel.unwrap_or(true);
    a.subtitulo = e.subtitulo.texto.clone().unwrap_or_default();
    a.legenda = e.legenda.posicao != Some(PosicaoLegenda::Nenhuma);
    a.valores = e.rotulos.visivel.unwrap_or(false);
    a.eixos = e.eixo_y.visivel.unwrap_or(true) || e.eixo_x.visivel.unwrap_or(true);
    a.eixo_x = e.eixo_x.titulo.clone().unwrap_or_default();
    a.eixo_y = e.eixo_y.titulo.clone().unwrap_or_default();
    a.unidade = e.eixo_y.unidade.clone().unwrap_or_default();
    if let Some(t) = e.fonte.tamanho {
        a.fonte = format!("{}", t.round().clamp(4.0, 32.0));
    }
    a.fundo = match &e.area_grafico.preenchimento {
        Some(Preenchimento::Solido { cor }) | Some(Preenchimento::Degrade { inicio: cor, .. }) => cor.texto(),
        Some(Preenchimento::Nenhum) => "#00000000".into(),
        None => e.tema().map_or_else(|| "#ffffff".into(), |t| t.fundo.texto()),
    };
}

/// Cor resolvida da série `i` (estilo > tema > a da planilha).
pub fn cor_resolvida_serie(e: &EstiloGrafico, base: &[feather_draw::Cor], i: usize) -> CorEstilo {
    e.serie(i).and_then(|s| s.cor)
        .or_else(|| e.tema().map(|t| t.paleta[i % t.paleta.len()]))
        .unwrap_or_else(|| base.get(i).copied().map_or(CorEstilo::hex(0x4285f4), CorEstilo))
}

// ------------------------------------------------------------- miniaturas

fn miniaturas() -> &'static Vec<Option<pelicano::Svg>> {
    static CACHE: std::sync::OnceLock<Vec<Option<pelicano::Svg>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        use feather_charts::{Grafico, Serie, Tipo};
        let g = Grafico {
            titulo: String::new(),
            tipo: Tipo::Colunas,
            categorias: vec!["a".into(), "b".into(), "c".into()],
            series: vec![
                Serie { nome: "1".into(), valores: vec![3.0, 5.0, 4.0], cor: feather_draw::Cor::rgb(66, 133, 244) },
                Serie { nome: "2".into(), valores: vec![2.0, 4.0, 6.0], cor: feather_draw::Cor::rgb(234, 67, 53) },
                Serie { nome: "3".into(), valores: vec![1.0, 2.5, 3.0], cor: feather_draw::Cor::rgb(251, 188, 4) },
            ],
        };
        feather_charts::estilo::temas().iter().map(|t| {
            let mut e = EstiloGrafico { tema: Some(t.id.into()), ..Default::default() };
            e.titulo.visivel = Some(false);
            e.legenda.posicao = Some(PosicaoLegenda::Nenhuma);
            e.eixo_x.visivel = Some(false);
            e.eixo_y.visivel = Some(false);
            e.area_grafico.raio = Some(t.raio.min(6.0));
            e.margens_grafico = Margens { superior: Some(6.0), inferior: Some(6.0), esquerda: Some(6.0), direita: Some(6.0) };
            let s = g.exportar_svg_formatado(88.0, 54.0, &e).unwrap_or_default();
            pelicano::Svg::from_bytes(s.as_bytes())
        }).collect()
    })
}

// ------------------------------------------------------------------- vista

fn ev(e: Evento) -> Msg {
    Msg::Grafico(e)
}
fn mud(m: Mudanca) -> Msg {
    Msg::Grafico(Evento::Formatar(m))
}
fn rotulo<'a>(t: &str) -> pelicano::Text<'a> {
    text(t.to_owned()).size(12).color(COR_TEXTO_MUTED())
}
/// Título de seção com uma linha fina em cima: mostra onde a parte
/// anterior termina.
fn secao<'a>(t: &str) -> Element<'a, Msg> {
    let divisor = container(Space::new(Length::FILL, Length::Fixed(1.0))).style(|_| pelicano::Style {
        background: Some(pelicano::Background::Color(COR_BORDA())),
        ..Default::default()
    });
    pelicano::column![Space::new(0.0, 4), divisor, Space::new(0.0, 2), secao_inicial(t)].into()
}
/// Título da primeira seção (sem linha: nada termina antes dela).
fn secao_inicial<'a>(t: &str) -> Element<'a, Msg> {
    text(t.to_uppercase()).size(10.5).color(COR_TEXTO_MUTED())
        .font(pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }).into()
}
/// Rótulo à esquerda (encolhe), controle à direita com largura fixa.
fn linha<'a>(r: &str, controle: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    row![rotulo(r).width(Length::FILL), controle.into()].spacing(8).align(pelicano::Align::Center).into()
}
fn alternar<'a>(r: &str, v: bool, f: impl Fn(bool) -> Msg + 'a) -> Element<'a, Msg> {
    // O toggler do Iced com rótulo à esquerda e a chave na ponta direita.
    row![rotulo(r).color(COR_TEXTO()).width(Length::FILL), toggler("", v).on_toggle(f).color(COR_AZUL_ACENTO())]
        .spacing(8).align(pelicano::Align::Center).width(Length::FILL).into()
}
/// Botões de escolha (3 por linha), o ativo destacado.
fn escolhas<'a>(itens: Vec<(String, bool, Msg)>, por_linha: usize) -> Element<'a, Msg> {
    let mut col = pelicano::column(Vec::new()).spacing(4);
    let mut itens = itens.into_iter().peekable();
    while itens.peek().is_some() {
        let mut r = row(Vec::new()).spacing(4);
        for k in 0..por_linha {
            match itens.next() {
                Some((t, ativo, m)) => {
                    r = r.push(button(text(t).size(11.5).align_x(pelicano::Align::Center).width(Length::FILL))
                        .width(Length::FILL).padding([4, 4]).style(estilo_botao_pequeno(ativo)).on_press(m));
                }
                None => {
                    let _ = k;
                    r = r.push(Space::new(Length::FILL, 0.0));
                }
            }
        }
        col = col.push(r);
    }
    col.into()
}
fn icone_botao<'a>(nome: &str, dica: &str, m: Option<Msg>) -> Element<'a, Msg> {
    let b = button(container(feather_ui::icones::icone(nome, 14.0)).width(Length::Fixed(14.0)).height(Length::Fixed(14.0))
        .align_x(pelicano::Align::Center).align_y(pelicano::Align::Center))
        .width(Length::Fixed(26.0)).height(Length::Fixed(26.0)).padding(6)
        .style(estilo_botao_pequeno(false)).on_press_maybe(m);
    tooltip(b, dica.to_owned(), TooltipPosition::Top).padding([4, 8]).style(estilo_dica).into()
}
fn cor_iced(c: CorEstilo) -> pelicano::Color {
    pelicano::Color::rgb8(c.0.r, c.0.g, c.0.b).alpha(f32::from(c.0.a) / 255.0)
}
fn amostra<'a>(c: CorEstilo, ativo: bool, m: Msg) -> Element<'a, Msg> {
    let cor = cor_iced(c);
    button(Space::new(Length::Fixed(18.0), Length::Fixed(18.0)))
        .padding(0)
        .on_press(m)
        .style(move |_: &pelicano::Theme, status| pelicano::Style {
            background: Some(pelicano::Background::Color(cor)),
            border: pelicano::Border {
                color: if ativo || matches!(status, pelicano::Status::Hovered) { COR_AZUL_ACENTO() } else { COR_BORDA() },
                width: if ativo { 2.5 } else { 1.0 },
                radius: [4.0; 4],
            },
            ..Default::default()
        })
        .into()
}

impl App {
    pub(super) fn estilo_vista(&self) -> EstiloGrafico {
        let a = &self.graficos.aparencia;
        a.estilo.clone().unwrap_or_else(|| migrar(a, self.graficos.painel.excel.as_ref()))
    }

    fn campo_num<'a>(&'a self, chave_campo: String, atual: Option<f64>, dica: &'static str, f: impl Fn(String) -> Mudanca + 'a) -> Element<'a, Msg> {
        let valor = self.graficos.painel.rascunhos.get(&chave_campo).cloned()
            .unwrap_or_else(|| atual.map_or_else(String::new, |v| {
                let s = format!("{v:.4}");
                let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
                s.replace('.', ",")
            }));
        let invalido = numero(&valor).is_err();
        let f = std::rc::Rc::new(f);
        feather_ui::campo_numero::campo_numero(&valor, dica, 112.0, limites_do_campo(&chave_campo), invalido,
            move |s| mud(f(s)))
    }

    /// Amostra da cor + paleta (tema + padrão) + Automática + Mais cores…
    fn campo_cor<'a>(&'a self, r: &str, alvo: AlvoCor, e: &EstiloGrafico, padrao: CorEstilo) -> Element<'a, Msg> {
        let atual = cor_de(e, alvo);
        let mostra = atual.unwrap_or(padrao);
        let aberto = self.graficos.painel.cor_menu == Some(alvo);
        let botao = button(row![
            container(Space::new(Length::Fixed(22.0), Length::Fixed(14.0))).style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(cor_iced(mostra))),
                border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [3.0; 4] },
                ..Default::default()
            }),
            text(if atual.is_some() { "" } else { tr!("Auto") }).size(11).color(COR_TEXTO_MUTED()),
            feather_ui::icones::icone("chevron-down", 12.0),
        ].spacing(5).align(pelicano::Align::Center))
        .padding([4, 6])
        .width(Length::Fixed(96.0))
        .style(estilo_botao_pequeno(aberto))
        .on_press(ev(Evento::CorMenu(if aberto { None } else { Some(alvo) })));
        let mut col = pelicano::column![linha(r, botao)].spacing(6);
        if aberto {
            let tema: Vec<CorEstilo> = e.tema().map_or_else(
                || TemaGrafico::pelo_id("padrao").map_or_else(Vec::new, |t| t.paleta.to_vec()),
                |t| t.paleta.to_vec(),
            );
            let mut grade = pelicano::column![rotulo(tr!("Cores do tema")).size(11)].spacing(5);
            grade = grade.push(row(tema.iter().map(|c| amostra(*c, atual == Some(*c), ev(Evento::Cor(alvo, Some(c.texto())))))).spacing(5));
            grade = grade.push(rotulo(tr!("Cores padrão")).size(11));
            for fileira in PALETA_PADRAO.chunks(7) {
                grade = grade.push(row(fileira.iter().map(|v| {
                    let c = CorEstilo::hex(*v);
                    amostra(c, atual == Some(c), ev(Evento::Cor(alvo, Some(c.texto()))))
                })).spacing(5));
            }
            let mais = self.graficos.cor_ativa == Some(AlvoCorGrafico::Estilo(alvo));
            grade = grade.push(row![
                button(text(tr!("Automática")).size(12)).padding([4, 8]).style(estilo_botao_pequeno(atual.is_none()))
                    .on_press(ev(Evento::Cor(alvo, None))),
                button(row![feather_ui::icones::icone("palette", 13.0), text(tr!("Mais cores…")).size(12)].spacing(5).align(pelicano::Align::Center))
                    .padding([4, 8]).style(estilo_botao_pequeno(mais)).on_press(ev(Evento::MaisCores(alvo))),
            ].spacing(6));
            if mais {
                grade = grade.push(feather_ui::seletor_cor::painel(tr!("Mais cores"), &mostra.texto(), self.graficos.cor_hsva, &self.graficos.cor_hex)
                    .map(|evento| Msg::Grafico(Evento::SeletorCor(evento))));
            }
            col = col.push(container(grade).padding(8).width(Length::FILL).style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
                ..Default::default()
            }));
        }
        col.into()
    }

    fn seletor_eixo<'a>(&'a self, com_y2: bool) -> Element<'a, Msg> {
        let s = self.graficos.painel.eixo;
        let mut itens = vec![
            (tr!("Categorias (X)").to_owned(), s == EixoSel::X, ev(Evento::Selecionar(Selecao::Eixo(EixoSel::X)))),
            (tr!("Valores (Y)").to_owned(), s == EixoSel::Y, ev(Evento::Selecionar(Selecao::Eixo(EixoSel::Y)))),
        ];
        if com_y2 {
            itens.push((tr!("Secundário").to_owned(), s == EixoSel::Y2, ev(Evento::Selecionar(Selecao::Eixo(EixoSel::Y2)))));
        }
        escolhas(itens, 3)
    }

    fn seletor_serie<'a>(&'a self) -> Element<'a, Msg> {
        let g = &self.graficos;
        let itens = g.series.iter().enumerate().map(|(i, n)| {
            let nome = g.aparencia.nomes.get(&i).cloned().unwrap_or_else(|| n.split_once(" — ").map_or(n.clone(), |(_, x)| x.to_owned()));
            (cortar(&nome, 14), g.painel.serie == i, ev(Evento::Selecionar(Selecao::Serie(i))))
        }).collect();
        escolhas(itens, 3)
    }

    fn cabecalho_grupo<'a>(&self, grupo: Grupo) -> Element<'a, Msg> {
        let aberto = self.graficos.painel.grupo == Some(grupo);
        let breve = grupo.em_breve();
        let cor_txt = if breve.is_some() { COR_TEXTO_MUTED() } else { COR_TEXTO() };
        let conteudo = row![
            feather_ui::icones::icone(grupo.icone(), 16.0),
            text(grupo.nome()).size(13).color(cor_txt)
                .font(pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }).width(Length::FILL),
            if breve.is_some() {
                Element::from(text(tr!("Em breve")).size(11).color(COR_TEXTO_MUTED()))
            } else {
                feather_ui::icones::icone(if aberto { "chevron-down" } else { "chevron-right" }, 14.0).into()
            },
        ].spacing(8).align(pelicano::Align::Center);
        let b = button(container(conteudo).height(Length::Fixed(ALTURA_CABECALHO - 2.0 - 16.0)).align_y(pelicano::Align::Center))
            .width(Length::FILL)
            .padding([8, 10])
            .style(move |_: &pelicano::Theme, st| pelicano::Style {
                background: Some(pelicano::Background::Color(if aberto {
                    COR_AZUL_FUNDO()
                } else if breve.is_none() && matches!(st, pelicano::Status::Hovered) {
                    COR_HOVER()
                } else {
                    pelicano::Color::TRANSPARENT
                })),
                text_color: Some(cor_txt),
                border: pelicano::Border { radius: [8.0; 4], ..Default::default() },
                ..Default::default()
            })
            .on_press_maybe(breve.is_none().then(|| ev(Evento::Grupo(grupo))));
        match breve {
            Some(d) => tooltip(b, d, TooltipPosition::Bottom).padding([4, 8]).style(estilo_dica).into(),
            None => b.into(),
        }
    }

    /// Rótulos das colunas (ou das linhas, com séries em linhas) do
    /// intervalo: o cabeçalho da planilha, ou "Coluna B" sem cabeçalho.
    pub(super) fn rotulos_do_intervalo(&self) -> Vec<String> {
        let g = &self.graficos;
        let Some(r) = parse_range_ref(g.intervalo.trim()).map(|r| r.normalized()) else { return Vec::new() };
        let Some(aba) = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).find(|a| Some(a.id) == g.aba_id) else {
            return Vec::new();
        };
        let n = if g.colunas { r.c1 - r.c0 + 1 } else { r.r1 - r.r0 + 1 };
        // Lista para escolher à mão: tabela larga demais não cabe no painel.
        if n > 200 { return Vec::new(); }
        (0..n).map(|k| {
            let (lr, lc) = if g.colunas { (r.r0, r.c0 + k) } else { (r.r0 + k, r.c0) };
            let texto = if g.cabecalho {
                feathersheet_core::celulas::values::to_text(&aba.sheet.get_value(lr, lc))
            } else {
                String::new()
            };
            if !texto.trim().is_empty() { texto }
            else if g.colunas { trf!("Coluna {}", feathersheet_core::celulas::refs::col_name(r.c0 + k)) }
            else { trf!("Linha {}", r.r0 + k + 1) }
        }).collect()
    }

    /// As séries possíveis: (ordem de origem, rótulo), sem a coluna das
    /// categorias — a mesma ordem do `hiddenSeries`/`aggregateBySeries`.
    pub(super) fn series_da_origem(&self) -> Vec<(u32, String)> {
        let g = &self.graficos;
        self.rotulos_do_intervalo().into_iter().enumerate()
            .filter(|&(k, _)| !(g.categorias && k as u32 == g.coluna_categoria))
            .enumerate()
            .map(|(ordem, (_, nome))| (ordem as u32, nome))
            .collect()
    }

    /// As séries de texto, pela ordem de origem (a de [`Self::series_da_origem`]):
    /// as que têm alguma célula com letra. Vazio e o cabeçalho não contam.
    /// Até 200 mil células olha tudo; acima, uma amostra espalhada de cada
    /// série ([`AMOSTRA_TIPO`]), e só até a última linha/coluna usada.
    pub(super) fn series_de_texto(&self) -> std::collections::BTreeSet<u32> {
        let g = &self.graficos;
        let mut texto = std::collections::BTreeSet::new();
        let Some(r) = parse_range_ref(g.intervalo.trim()).map(|r| r.normalized()) else { return texto };
        let Some(aba) = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).find(|a| Some(a.id) == g.aba_id) else {
            return texto;
        };
        let s = &aba.sheet;
        let n = if g.colunas { r.c1 - r.c0 + 1 } else { r.r1 - r.r0 + 1 };
        // A mesma lista do painel: tabela larga demais não tem escolha por série.
        if n > 200 || s.is_empty() { return texto; }
        // Os valores de cada série: sem o cabeçalho (e, em linhas, sem a
        // coluna dos nomes), até a última célula usada da aba.
        let (inicio, fim) = if g.colunas {
            (r.r0 + u32::from(g.cabecalho), r.r1.min(s.max_row))
        } else {
            (r.c0 + u32::from(g.cabecalho || g.categorias), r.c1.min(s.max_col))
        };
        if fim < inicio { return texto; }
        let total = fim - inicio + 1;
        let passo = if u64::from(total) * u64::from(n) <= super::editor::MAX_CELULAS_GRAFICO {
            1
        } else {
            total.div_ceil(AMOSTRA_TIPO).max(1)
        };
        let mut ordem = 0;
        for k in 0..n {
            if g.categorias && k == g.coluna_categoria { continue; }
            let tem_letra = (inicio..=fim).step_by(passo as usize).any(|i| {
                let (lr, lc) = if g.colunas { (i, r.c0 + k) } else { (r.r0 + k, i) };
                celula_de_texto(&s.get_value(lr, lc))
            });
            if tem_letra { texto.insert(ordem); }
            ordem += 1;
        }
        texto
    }

    /// O valor desenhado nos tipos de valor único (pizza, histograma,
    /// cascata): a série visível que o desenho usa, pela ordem de origem.
    /// A pizza escolhe entre as visíveis pelo `_featherPieSeries` ("2 — Meta").
    pub(super) fn origem_do_valor(&self) -> Option<u32> {
        let g = &self.graficos;
        let visiveis: Vec<u32> = self.series_da_origem().into_iter().map(|s| s.0).filter(|o| !g.ocultas.contains(o)).collect();
        let i = if g.tipo == TipoUi::Pizza {
            g.serie.as_deref().and_then(|s| s.split(" — ").next()).and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1)
        } else {
            0
        };
        visiveis.get(i).or(visiveis.first()).copied()
    }

    /// As séries de origem de cada papel do tipo ([`TipoUi::papeis`]): as
    /// escolhidas no painel ou, sem escolha, as visíveis na ordem das colunas.
    pub(super) fn papeis_atuais(&self) -> Vec<u32> {
        let g = &self.graficos;
        let n = g.tipo.papeis().len();
        let todas: Vec<u32> = self.series_da_origem().into_iter().map(|s| s.0).collect();
        let salvos = &g.aparencia.papeis;
        if salvos.len() == n && salvos.iter().all(|o| todas.contains(o)) {
            return salvos.clone();
        }
        let mut p: Vec<u32> = todas.iter().copied().filter(|o| !g.ocultas.contains(o)).take(n).collect();
        for o in &todas {
            if p.len() >= n { break; }
            if !p.contains(o) { p.push(*o); }
        }
        p
    }

    /// Linha 3D sem escolha no painel: as colunas que o motor usaria.
    pub(super) fn eixos3d_padrao(&self) -> Option<feathersheet_core::grafico::chart::Eixos3d> {
        let g = &self.graficos;
        let r = parse_range_ref(g.intervalo.trim())?.normalized();
        let aba = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).find(|a| Some(a.id) == g.aba_id)?;
        let linha = r.r0 + u32::from(g.cabecalho && r.r1 > r.r0);
        Some(feathersheet_core::grafico::chart::eixos_3d_padrao(r.c1 - r.c0 + 1, |i| aba.sheet.get_value(linha, r.c0 + i)))
    }

    /// Aba Configurar, como a do Google Planilhas: tipo, intervalo e os
    /// campos de dados que o tipo usa, com os nomes dele (pizza: rótulos e
    /// valor; velas: data, abertura, máxima…; issue #108).
    fn view_configurar(&self) -> Element<'_, Msg> {
        let g = &self.graficos;
        let t = g.tipo;
        let mut col = pelicano::column(Vec::new()).spacing(8).padding([6, 4]);
        col = col.push(secao_inicial(tr!("Tipo de gráfico")));
        col = col.push(button(row![text(t.to_string()).size(12).width(Length::FILL), feather_ui::icones::icone("chevron-down", 14.0)]
            .align(pelicano::Align::Center)).padding([6, 8]).width(Length::FILL)
            .style(estilo_botao_secundario).on_press(ev(Evento::AlternarSeletor)));

        col = col.push(secao(tr!("Dados")));
        col = col.push(rotulo(tr!("Intervalo")));
        col = col.push(row![
            text_input("A1:C10", &g.intervalo).size(12).padding([5, 6]).on_input(|s| ev(Evento::Intervalo(s))).width(Length::FILL),
            tooltip(
                button(row![feather_ui::icones::icone("table", 14.0), text(tr!("Usar seleção")).size(12)].spacing(5).align(pelicano::Align::Center))
                    .padding([5, 8]).style(estilo_botao_secundario).on_press(ev(Evento::UsarSelecao)),
                tr!("Usa as células selecionadas na planilha"),
                TooltipPosition::Bottom,
            ).padding([4, 8]).style(estilo_dica),
        ].spacing(6).align(pelicano::Align::Center));
        col = col.push(alternar(tr!("Primeira linha é cabeçalho"), g.cabecalho, |v| ev(Evento::Cabecalho(v))));
        let rotulos = self.rotulos_do_intervalo();
        // A linha 3D lê X, Y e Z por coluna: não tem séries nas linhas.
        if t == TipoUi::Linha3d {
            return self.campos_3d(col, &rotulos).into();
        }
        col = col.push(alternar(tr!("Séries nas colunas"), g.colunas, |v| ev(Evento::Colunas(v))));

        if let Some((nome, com_nenhuma)) = t.campo_categoria() {
            let nome = if g.colunas { nome.to_owned() } else { trf!("{nome}, pela linha") };
            col = col.push(secao(&nome));
            let mut opcoes = Vec::new();
            if com_nenhuma {
                // Dispersão sem coluna de X: o X é a primeira série marcada.
                let vazio = if t == TipoUi::Dispersao { tr!("A primeira série marcada") } else { tr!("Nenhuma") };
                opcoes.push(Opcao { valor: None, nome: vazio.to_owned() });
            }
            opcoes.extend(rotulos.iter().enumerate().map(|(i, n)| Opcao { valor: Some(i as u32), nome: n.clone() }));
            let atual = opcoes.iter().find(|o| o.valor == g.categorias.then_some(g.coluna_categoria)).cloned();
            col = col.push(pick_list(opcoes, atual, |o: Opcao<Option<u32>>| ev(Evento::CategoriaColuna(o.valor)))
                .size(12).padding([5, 8]).width(Length::FILL));
        }

        // Acima de 200 mil células só resumido: "Não resumir" sai da lista.
        // Série de texto só conta (o tipo vem calculado de `atualizar_grafico`).
        let opcoes_de = |texto: bool| -> Vec<Opcao<Option<AggregateFn>>> {
            contas_da_serie(texto, g.resumo_obrigatorio).into_iter()
                .map(|(nome, valor)| Opcao { valor, nome: tr!(nome).to_owned() }).collect()
        };
        let contas_numero = opcoes_de(false);
        let contas_texto = opcoes_de(true);
        let conta = |n: u32| {
            let contas = if g.serie_texto(n) { &contas_texto } else { &contas_numero };
            let conta_atual = g.agrupar.then(|| g.conta_da_serie(n));
            let atual = contas.iter().find(|o| o.valor == conta_atual).cloned();
            pick_list(contas.clone(), atual, move |o: Opcao<Option<AggregateFn>>| ev(Evento::SerieConta(n, o.valor)))
                .size(12).padding([5, 6]).width(Length::Fixed(120.0))
        };
        let series = self.series_da_origem();
        if t.valor_unico() {
            // Escolha única (era caixa de marcar, e a pizza abria com duas marcadas).
            col = col.push(secao(if t == TipoUi::Histograma { tr!("Valores") } else { tr!("Valor") }));
            let atual = self.origem_do_valor();
            for (n, nome) in &series {
                let n = *n;
                let mut linha_valor = row![pelicano::radio(cortar(nome, 30), n, atual, |n| ev(Evento::SerieVisivel(n, true)))
                    .size(12.5), Space::new(Length::FILL, 0.0)].spacing(6).align(pelicano::Align::Center);
                if atual == Some(n) {
                    linha_valor = linha_valor.push(conta(n));
                }
                col = col.push(linha_valor);
            }
            if t == TipoUi::Histograma {
                col = col.push(text(tr!("As faixas saem dos próprios valores.")).size(11).color(COR_TEXTO_MUTED()));
            }
        } else if !t.papeis().is_empty() {
            let papeis = self.papeis_atuais();
            let opcoes: Vec<Opcao<u32>> = series.iter().map(|(n, nome)| Opcao { valor: *n, nome: nome.clone() }).collect();
            for (k, papel) in t.papeis().iter().enumerate() {
                let atual = papeis.get(k).and_then(|p| opcoes.iter().find(|o| o.valor == *p)).cloned();
                col = col.push(secao(tr!(*papel)));
                col = col.push(pick_list(opcoes.clone(), atual, move |o: Opcao<u32>| ev(Evento::Papel(k, o.valor)))
                    .size(12).padding([5, 8]).width(Length::FILL));
            }
        } else {
            col = col.push(secao(t.campo_series()));
            col = col.push(text(match t {
                TipoUi::Combinado => tr!("A primeira série marcada vira colunas; as outras, linhas."),
                TipoUi::MapaCalor => tr!("Cada quadro mostra o valor da célula. Marque as colunas do mapa."),
                _ => tr!("Marque as séries do gráfico. Nome em branco usa o cabeçalho."),
            }).size(11).color(COR_TEXTO_MUTED()));
            for (n, nome) in &series {
                let n = *n;
                let visivel = !g.ocultas.contains(&n);
                let nome_atual = g.aparencia.nomes_origem.get(&n).cloned().unwrap_or_default();
                col = col.push(row![
                    pelicano::checkbox("", visivel).on_toggle(move |v| ev(Evento::SerieVisivel(n, v))),
                    text_input(nome.clone(), nome_atual).size(12).padding([5, 6]).width(Length::FILL)
                        .on_input(move |s| ev(Evento::SerieNome(n, s))),
                    conta(n),
                ].spacing(6).align(pelicano::Align::Center));
            }
        }
        if series.is_empty() {
            col = col.push(text(tr!("Escolha um intervalo com dados.")).size(12).color(COR_TEXTO_MUTED()));
        }
        if t == TipoUi::Rosca {
            let furo = g.furo.unwrap_or(50);
            col = col.push(secao(tr!("Tamanho do furo")));
            col = col.push(row![
                pelicano::slider(10.0..=90.0, f32::from(furo), |v| ev(Evento::Furo(v.round() as u8))).width(Length::FILL),
                text(format!("{furo} %")).size(12).width(Length::Fixed(40.0)),
            ].spacing(8).align(pelicano::Align::Center));
        }
        if g.resumo_obrigatorio {
            col = col.push(text(tr!("Mais de 200.000 células: o gráfico sempre resume. Categorias iguais viram uma só, com a conta de cada série."))
                .size(11).color(COR_TEXTO_MUTED()));
        } else if g.agrupar {
            col = col.push(text(tr!("Categorias iguais viram uma só, com a conta de cada série. \"Não resumir\" vale para todas."))
                .size(11).color(COR_TEXTO_MUTED()));
        }
        col.into()
    }

    /// Linha 3D: a coluna de cada eixo e, se quiser, a da descrição (uma
    /// linha do gráfico para cada valor dela).
    fn campos_3d<'a>(&'a self, mut col: pelicano::Column<'a, Msg>, rotulos: &[String]) -> pelicano::Column<'a, Msg> {
        let Some(e) = self.graficos.eixos3d.or_else(|| self.eixos3d_padrao()) else { return col };
        let opcoes: Vec<Opcao<Option<u32>>> = rotulos.iter().enumerate().map(|(i, n)| Opcao { valor: Some(i as u32), nome: n.clone() }).collect();
        let nenhuma = Opcao { valor: None, nome: tr!("Nenhuma").to_owned() };
        for (k, nome, atual, opcional) in [
            (0, tr!("X"), Some(e.x), false),
            (1, tr!("Y"), Some(e.y), false),
            (2, tr!("Z (profundidade)"), e.z, false),
            (3, tr!("Descrição (uma linha por valor)"), e.grupo, true),
        ] {
            let mut lista = opcoes.clone();
            if opcional {
                lista.insert(0, nenhuma.clone());
            }
            let sel = lista.iter().find(|o| o.valor == atual).cloned();
            col = col.push(secao(nome)).push(pick_list(lista, sel, move |o: Opcao<Option<u32>>| ev(Evento::Eixo3d(k, o.valor)))
                .size(12).padding([5, 8]).width(Length::FILL));
        }
        col
    }

    pub(in crate::app) fn view_graficos(&self) -> Element<'_, Msg> {
        let g = &self.graficos;
        let e = self.estilo_vista();
        let cabecalho = row![
            feather_ui::icones::icone("chart-bar", 18.0),
            text(tr!("Gráfico")).size(17).font(pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }).width(Length::FILL),
            feather_ui::tema::botao_fechar(ev(Evento::Fechar)),
        ].spacing(8).align(pelicano::Align::Center);
        let mut lista = pelicano::column(Vec::new()).spacing(2);
        if g.painel.aba == AbaPainel::Configurar {
            lista = lista.push(self.view_configurar());
        } else {
            for grupo in Grupo::PERSONALIZAR.into_iter().filter(|x| x.existe_em(g.tipo)) {
                lista = lista.push(self.cabecalho_grupo(grupo));
                if g.painel.grupo == Some(grupo) {
                    let corpo = self.corpo_grupo(grupo, &e);
                    lista = lista.push(container(corpo).padding([8, 10]).width(Length::FILL));
                }
            }
        }
        let aba_botao = |nome: &'static str, icone: &'static str, aba: AbaPainel| {
            let ativo = g.painel.aba == aba;
            button(row![feather_ui::icones::icone(icone, 15.0), text(nome).size(13)].spacing(6).align(pelicano::Align::Center))
                .width(Length::FILL)
                .padding([7, 10])
                .style(estilo_botao_pequeno(ativo))
                .on_press(ev(Evento::AbaPainel(aba)))
        };
        let abas = row![
            aba_botao(tr!("Configurar"), "table", AbaPainel::Configurar),
            aba_botao(tr!("Personalizar"), "palette", AbaPainel::Personalizar),
        ].spacing(6);
        let mut exportar = button(row![feather_ui::icones::icone("file-export", 14.0), text(tr!("Exportar SVG")).size(12)].spacing(6).align(pelicano::Align::Center))
            .padding([6, 10]).style(estilo_botao_secundario);
        if g.svg.is_some() {
            exportar = exportar.on_press(ev(Evento::Exportar));
        }
        let mut acoes = row![exportar, Space::new(Length::FILL, 0.0)].spacing(8).align(pelicano::Align::Center);
        if let Some(id) = &g.id {
            acoes = acoes.push(button(text(tr!("Excluir gráfico")).size(12)).padding([6, 10]).style(estilo_botao_secundario)
                .on_press(ev(Evento::Remover(id.clone()))));
        } else {
            let mut inserir = button(text(tr!("Inserir na planilha")).size(12)).padding([6, 10]).style(estilo_botao_primario);
            if g.svg.is_some() {
                inserir = inserir.on_press(ev(Evento::Inserir));
            }
            acoes = acoes.push(inserir);
        }
        let mut corpo = pelicano::column![
            cabecalho,
            abas,
            feather_ui::tema::rolagem(lista).id(ID_ROLAGEM).height(Length::FILL),
        ].spacing(12);
        if !g.mensagem.is_empty() {
            corpo = corpo.push(row![feather_ui::icones::icone("info-circle", 14.0), text(&g.mensagem).size(11.5).color(COR_TEXTO_MUTED())]
                .spacing(6).align(pelicano::Align::Center));
        }
        corpo = corpo.push(acoes);
        container(corpo)
            .padding([14, 12])
            .width(Length::Fixed(LARGURA_PAINEL))
            .height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                text_color: Some(COR_TEXTO()),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1., radius: [0.0; 4] },
                ..Default::default()
            })
            .into()
    }

    #[allow(clippy::too_many_lines)]
    fn corpo_grupo<'a>(&'a self, grupo: Grupo, e: &EstiloGrafico) -> Element<'a, Msg> {
        let g = &self.graficos;
        let p = &g.painel;
        let col = pelicano::column(Vec::new()).spacing(8);
        // Pizza e rosca: legenda, cores e rótulos por fatia (categoria).
        let pizza = matches!(g.tipo, TipoUi::Pizza | TipoUi::Rosca);
        let barras = matches!(g.tipo, TipoUi::Colunas | TipoUi::Barras);
        let tema = e.tema();
        let texto_padrao = tema.map_or(CorEstilo::hex(0x202020), |t| t.texto);
        let eixo_padrao = tema.map_or(CorEstilo::rgb(95, 99, 104), |t| t.eixo);
        let grade_padrao = tema.map_or(CorEstilo::hex(0xe3e5e8), |t| t.grade);
        let n_series = g.series.len();
        match grupo {
            Grupo::TipoEstilo => {
                let mut col = col.push(linha(tr!("Tipo de gráfico"), button(text(g.tipo.to_string()).size(12)).padding([4, 8])
                    .width(Length::Fixed(150.0)).style(estilo_botao_secundario).on_press(ev(Evento::AlternarSeletor))));
                col = col.push(secao(tr!("Temas prontos")));
                let minis = miniaturas();
                let mut grade = pelicano::column(Vec::new()).spacing(6);
                for (k, fileira) in feather_charts::estilo::temas().chunks(3).enumerate() {
                    let mut r = row(Vec::new()).spacing(6);
                    for (j, t) in fileira.iter().enumerate() {
                        let ativo = e.tema.as_deref() == Some(t.id);
                        let mini = minis.get(k * 3 + j).cloned().flatten();
                        let cartao = pelicano::column![
                            mini.map_or_else(|| Element::from(Space::new(88, 54)), |h| svg(h).width(88).height(54).into()),
                            text(t.nome).size(11),
                        ].spacing(3).align(pelicano::Align::Center);
                        r = r.push(button(cartao).padding(4).width(Length::FILL).style(estilo_botao_pequeno(ativo))
                            .on_press(mud(Mudanca::Tema(Some(t.id)))));
                    }
                    for _ in fileira.len()..3 {
                        r = r.push(Space::new(Length::FILL, 0.0));
                    }
                    grade = grade.push(r);
                }
                col = col.push(grade);
                col = col.push(button(text(tr!("Sem tema (aparência padrão)")).size(12)).width(Length::FILL).padding([5, 8])
                    .style(estilo_botao_pequeno(e.tema.is_none())).on_press(mud(Mudanca::Tema(None))));
                col.into()
            }
            Grupo::Dados => {
                let mut col = col
                    .push(rotulo(tr!("Intervalo da tabela")))
                    .push(text_input("A1:C10", &g.intervalo).size(12).on_input(|s| ev(Evento::Intervalo(s))))
                    .push(alternar(tr!("Usar cabeçalhos"), g.cabecalho, |v| ev(Evento::Cabecalho(v))))
                    .push(alternar(tr!("Usar categorias"), g.categorias, |v| ev(Evento::Categorias(v))))
                    .push(alternar(tr!("Séries em colunas"), g.colunas, |v| ev(Evento::Colunas(v))))
                    .push(alternar(tr!("Resumir categorias iguais"), g.agrupar, |v| ev(Evento::Agrupar(v))));
                if g.tipo == TipoUi::Pizza {
                    col = col.push(rotulo(tr!("Série da pizza"))).push(escolhas(
                        g.series.iter().map(|s| (cortar(s, 14), g.serie.as_deref() == Some(s.as_str()), ev(Evento::Serie(s.clone())))).collect(), 2));
                }
                col.into()
            }
            Grupo::Titulo => col
                .push(alternar(tr!("Mostrar título"), e.titulo.visivel.unwrap_or(true), |v| mud(Mudanca::TituloVisivel(v))))
                .push(rotulo(tr!("Texto do título")))
                .push(text_input(tr!("Título do gráfico"), &g.titulo).size(12).on_input(|s| ev(Evento::Titulo(s))))
                .push(rotulo(tr!("Subtítulo")))
                .push(text_input(tr!("Subtítulo"), e.subtitulo.texto.clone().unwrap_or_default()).size(12).on_input(|s| mud(Mudanca::Subtitulo(s))))
                .push(rotulo(tr!("Alinhamento")))
                .push(escolhas([(tr!("Esquerda"), Alinhamento::Esquerda), (tr!("Centro"), Alinhamento::Centro), (tr!("Direita"), Alinhamento::Direita)].into_iter()
                    .map(|(t, a)| (t.to_owned(), e.titulo.alinhamento.unwrap_or_default() == a, mud(Mudanca::TituloAlinhar(a)))).collect(), 3))
                .push(alternar(tr!("Sobre a área do gráfico"), e.titulo.sobreposto, |v| mud(Mudanca::TituloSobreposto(v))))
                .push(linha(tr!("Tamanho da letra"), self.campo_num("titulo.tamanho".into(), e.titulo.fonte.tamanho.map(f64::from), tr!("Auto"), Mudanca::TituloTamanho)))
                .push(alternar(tr!("Negrito"), e.titulo.fonte.negrito.unwrap_or(false), |v| mud(Mudanca::TituloNegrito(v))))
                .push(self.campo_cor(tr!("Cor do título"), AlvoCor::Titulo, e, texto_padrao))
                .into(),
            Grupo::Legenda => {
                let mut col = col
                    .push(rotulo(tr!("Posição")))
                    .push(escolhas(PosicaoLegenda::TODAS.iter().map(|p| (p.nome().to_owned(), e.legenda.posicao.unwrap_or_default() == *p, mud(Mudanca::LegendaPosicao(*p)))).collect(), 3))
                    .push(linha(tr!("Tamanho da letra"), self.campo_num("legenda.tamanho".into(), e.legenda.fonte.tamanho.map(f64::from), tr!("Auto"), Mudanca::LegendaTamanho)))
                    .push(self.campo_cor(tr!("Cor do texto"), AlvoCor::Legenda, e, texto_padrao))
                    .push(secao(tr!("Itens")));
                let nomes: Vec<String> = if pizza {
                    p.categorias.clone()
                } else {
                    g.series.iter().enumerate().map(|(i, s)| g.aparencia.nomes.get(&i).cloned()
                        .unwrap_or_else(|| s.split_once(" — ").map_or(s.clone(), |(_, x)| x.to_owned()))).collect()
                };
                let total = nomes.len();
                for i in e.legenda.ordem_completa(total) {
                    let item = e.legenda.itens.get(&i).cloned().unwrap_or_default();
                    let padrao = if pizza { CorEstilo::hex(0x2b7a78) } else { cor_resolvida_serie(e, &p.cores_base, i) };
                    let alvo = AlvoCor::ItemLegenda(i);
                    let cor = item.cor.unwrap_or(padrao);
                    let aberto = p.cor_menu == Some(alvo);
                    let campo = text_input(nomes[i].clone(), item.texto.clone().unwrap_or_default()).size(12).padding([4, 6])
                        .on_input(move |s| mud(Mudanca::ItemTexto(i, s))).width(Length::FILL);
                    col = col.push(row![
                        amostra(cor, aberto, ev(Evento::CorMenu(if aberto { None } else { Some(alvo) }))),
                        campo,
                        icone_botao(if item.oculto { "eye-off" } else { "eye" }, if item.oculto { tr!("Mostrar na legenda") } else { tr!("Ocultar da legenda") },
                            Some(mud(Mudanca::ItemOculto(i, !item.oculto)))),
                        icone_botao("chevron-up", tr!("Subir na legenda"), Some(mud(Mudanca::ItemMover(i, -1, total)))),
                        icone_botao("chevron-down", tr!("Descer na legenda"), Some(mud(Mudanca::ItemMover(i, 1, total)))),
                    ].spacing(4).align(pelicano::Align::Center));
                    if aberto {
                        col = col.push(self.campo_cor(tr!("Cor do item"), alvo, e, padrao));
                    }
                }
                col.into()
            }
            Grupo::Eixos => {
                let s = p.eixo;
                let x = eixo(e, s);
                let valores = s != EixoSel::X || g.tipo == TipoUi::Dispersao;
                let k = |c: CampoEixo| format!("eixo{s:?}.{c:?}");
                let mut col = col.push(self.seletor_eixo(true))
                    .push(alternar(tr!("Mostrar eixo"), x.visivel.unwrap_or(true), move |v| mud(Mudanca::EixoVisivel(s, v))));
                if valores {
                    col = col.push(secao(tr!("Escala")))
                        .push(linha(tr!("Mínimo"), self.campo_num(k(CampoEixo::Minimo), x.minimo, tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Minimo, t))))
                        .push(linha(tr!("Máximo"), self.campo_num(k(CampoEixo::Maximo), x.maximo, tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Maximo, t))))
                        .push(linha(tr!("Unidade principal"), self.campo_num(k(CampoEixo::Principal), x.unidade_principal, tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Principal, t))))
                        .push(linha(tr!("Unidade secundária"), self.campo_num(k(CampoEixo::Secundaria), x.unidade_secundaria, tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Secundaria, t))))
                        .push(alternar(tr!("Escala logarítmica"), x.logaritmica, move |v| mud(Mudanca::EixoLog(s, v))));
                }
                col = col.push(alternar(if valores { tr!("Valores em ordem inversa") } else { tr!("Categorias em ordem inversa") }, x.invertido, move |v| mud(Mudanca::EixoInvertido(s, v))))
                    .push(linha(if valores { tr!("Outro eixo cruza em") } else { tr!("Outro eixo cruza na categoria") },
                        self.campo_num(k(CampoEixo::Cruzamento), x.cruzamento, tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Cruzamento, t))));
                if valores {
                    let f = x.formato.unwrap_or_default();
                    col = col.push(secao(tr!("Número")))
                        .push(escolhas([FormatoNum::Automatico, FormatoNum::Decimal { casas: 0 }, FormatoNum::Moeda { casas: 2 }, FormatoNum::Percentual { casas: 0 }]
                            .into_iter().map(|o| (o.nome().to_owned(), std::mem::discriminant(&o) == std::mem::discriminant(&f), mud(Mudanca::EixoFormato(s, o)))).collect(), 2))
                        .push(linha(tr!("Casas decimais"), self.campo_num(k(CampoEixo::Casas), x.formato.filter(|f| *f != FormatoNum::Automatico).map(|f| f64::from(f.casas())), tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::Casas, t))));
                }
                col = col.push(secao(tr!("Rótulos e título")))
                    .push(linha(tr!("Giro dos rótulos (graus)"), self.campo_num(k(CampoEixo::Rotacao), x.rotacao.map(f64::from), "0", move |t| Mudanca::EixoNum(s, CampoEixo::Rotacao, t))))
                    .push(linha(tr!("Tamanho da letra"), self.campo_num(k(CampoEixo::TamanhoRotulos), x.rotulos.tamanho.map(f64::from), tr!("Auto"), move |t| Mudanca::EixoNum(s, CampoEixo::TamanhoRotulos, t))))
                    .push(self.campo_cor(tr!("Cor dos rótulos"), AlvoCor::RotulosEixo(s), e, texto_padrao))
                    .push(rotulo(tr!("Título do eixo")))
                    .push(text_input(tr!("Sem título"), x.titulo.clone().unwrap_or_default()).size(12).on_input(move |t| mud(Mudanca::EixoTitulo(s, t))));
                if s == EixoSel::Y {
                    col = col.push(rotulo(tr!("Unidade (R$, kg…)")))
                        .push(text_input(tr!("Sem unidade"), e.eixo_y.unidade.clone().unwrap_or_default()).size(12).on_input(|t| mud(Mudanca::EixoUnidade(t))));
                }
                col.push(secao(tr!("Linha do eixo")))
                    .push(alternar(tr!("Mostrar linha"), x.linha.visivel.unwrap_or(true), move |v| mud(Mudanca::EixoLinha(s, v))))
                    .push(linha(tr!("Espessura (px)"), self.campo_num(k(CampoEixo::EspessuraLinha), x.linha.espessura.map(f64::from), "1", move |t| Mudanca::EixoNum(s, CampoEixo::EspessuraLinha, t))))
                    .push(self.campo_cor(tr!("Cor da linha"), AlvoCor::LinhaEixo(s), e, eixo_padrao))
                    .into()
            }
            Grupo::Grades => {
                let s = p.eixo;
                let mut col = col.push(self.seletor_eixo(true));
                for principal in [true, false] {
                    let l = if principal { &eixo(e, s).grade_principal } else { &eixo(e, s).grade_secundaria };
                    let padrao_vis = principal && s == EixoSel::Y && tema.is_some_and(|t| t.grade_visivel);
                    col = col.push(secao(if principal { tr!("Grade principal") } else { tr!("Grade secundária") }))
                        .push(alternar(tr!("Mostrar"), l.visivel.unwrap_or(padrao_vis), move |v| mud(Mudanca::GradeVisivel(s, principal, v))))
                        .push(linha(tr!("Espessura (px)"), self.campo_num(format!("grade{s:?}{principal}"), l.espessura.map(f64::from), if principal { "1" } else { "0,6" },
                            move |t| Mudanca::GradeEspessura(s, principal, t))))
                        .push(escolhas(Traco::TODOS.iter().map(|t| (t.nome().to_owned(), l.traco.unwrap_or_default() == *t, mud(Mudanca::GradeTraco(s, principal, *t)))).collect(), 2))
                        .push(self.campo_cor(tr!("Cor"), AlvoCor::Grade(s, principal), e, grade_padrao));
                }
                col.into()
            }
            Grupo::Series => {
                if n_series == 0 {
                    return col.push(rotulo(tr!("Sem séries nos dados."))).into();
                }
                let i = p.serie.min(n_series - 1);
                let se = e.serie(i).cloned().unwrap_or_default();
                let nome = g.aparencia.nomes.get(&i).cloned()
                    .unwrap_or_else(|| g.series[i].split_once(" — ").map_or(g.series[i].clone(), |(_, x)| x.to_owned()));
                let mut col = col.push(self.seletor_serie())
                    .push(rotulo(tr!("Nome da série")))
                    .push(text_input(tr!("Nome"), nome.clone()).size(12).on_input(move |s| ev(Evento::NomeSerie(i, s))))
                    .push(self.campo_cor(tr!("Cor"), AlvoCor::Serie(i), e, cor_resolvida_serie(e, &p.cores_base, i)))
                    .push(self.campo_cor(tr!("Contorno"), AlvoCor::Contorno(i), e, CorEstilo::hex(0x202020)))
                    .push(linha(tr!("Espessura do contorno"), self.campo_num(format!("serie{i}.contorno"), se.espessura_contorno.map(f64::from), "1", move |t| Mudanca::SerieContornoEspessura(i, t))))
                    .push(linha(tr!("Transparência (%)"), self.campo_num(format!("serie{i}.transparencia"), se.opacidade.map(|o| f64::from(((1.0 - o) * 100.0).round())), "0", move |t| Mudanca::SerieTransparencia(i, t))));
                if !barras && !pizza {
                    col = col.push(secao(tr!("Linha")))
                        .push(linha(tr!("Espessura (px)"), self.campo_num(format!("serie{i}.espessura"), se.espessura_linha.map(f64::from), "2", move |t| Mudanca::SerieEspessura(i, t))))
                        .push(escolhas(Traco::TODOS.iter().map(|t| (t.nome().to_owned(), se.traco.unwrap_or_default() == *t, mud(Mudanca::SerieTraco(i, *t)))).collect(), 2))
                        .push(alternar(tr!("Linha suavizada"), se.suavizar.unwrap_or(false), move |v| mud(Mudanca::SerieSuavizar(i, v))))
                        .push(secao(tr!("Marcador")))
                        .push(escolhas(FormaMarcador::TODAS.iter().map(|f| (f.nome().to_owned(),
                            se.marcador.unwrap_or(if g.tipo == TipoUi::Dispersao { FormaMarcador::Quadrado } else { FormaMarcador::Nenhum }) == *f,
                            mud(Mudanca::SerieMarcador(i, *f)))).collect(), 3))
                        .push(linha(tr!("Tamanho do marcador"), self.campo_num(format!("serie{i}.marcador"), se.tamanho_marcador.map(f64::from), "6", move |t| Mudanca::SerieTamanhoMarcador(i, t))));
                }
                if !pizza {
                    col = col.push(alternar(tr!("Eixo secundário (à direita)"), se.eixo_secundario, move |v| mud(Mudanca::SerieSecundario(i, v))));
                }
                col.into()
            }
            Grupo::Pontos => {
                if n_series == 0 {
                    return col.push(rotulo(tr!("Sem séries nos dados."))).into();
                }
                let s = if pizza { 0 } else { p.serie.min(n_series - 1) };
                let mut col = col.push(rotulo(tr!("Pinte só uma coluna, fatia, ponto ou trecho de linha.")));
                if !pizza {
                    col = col.push(self.seletor_serie());
                }
                let cats = &p.categorias;
                if cats.is_empty() {
                    return col.push(rotulo(tr!("Sem pontos nos dados."))).into();
                }
                let k = p.ponto.min(cats.len() - 1);
                let pintado = |j: usize| e.serie(s).is_some_and(|x| x.pontos.contains_key(&j));
                col = col.push(secao(tr!("Ponto"))).push(escolhas(cats.iter().enumerate().take(60).map(|(j, c)| {
                    let marca = if pintado(j) { format!("{} *", cortar(c, 10)) } else { cortar(c, 12) };
                    (marca, j == k, ev(Evento::Selecionar(Selecao::Ponto(j))))
                }).collect(), 3));
                let padrao = if pizza { CorEstilo::hex(0x2b7a78) } else { cor_resolvida_serie(e, &p.cores_base, s) };
                col = col.push(self.campo_cor(tr!("Cor do ponto"), AlvoCor::Ponto(s, k), e, padrao));
                if pizza {
                    let v = e.serie(s).and_then(|x| x.pontos.get(&k)).and_then(|q| q.destacar).map(f64::from);
                    col = col.push(linha(tr!("Afastar a fatia (px)"), self.campo_num(format!("ponto{s}.{k}.destacar"), v, "0", move |t| Mudanca::PontoDestacar(s, k, t))));
                }
                col.push(button(text(tr!("Voltar à cor da série")).size(12)).padding([4, 8]).style(estilo_botao_secundario)
                    .on_press_maybe(pintado(k).then(|| mud(Mudanca::PontoLimpar(s, k)))))
                    .into()
            }
            Grupo::Rotulos => {
                let r = &e.rotulos;
                let f = r.formato.unwrap_or_default();
                let mut col = col
                    .push(alternar(tr!("Mostrar rótulos de dados"), r.visivel.unwrap_or(pizza), |v| mud(Mudanca::RotulosVisivel(v))))
                    .push(secao(tr!("Conteúdo")))
                    .push(alternar(tr!("Valor"), r.valor.unwrap_or(!pizza), |v| mud(Mudanca::RotulosValor(v))))
                    .push(alternar(tr!("Nome da série"), r.nome_serie.unwrap_or(false), |v| mud(Mudanca::RotulosSerie(v))))
                    .push(alternar(tr!("Categoria"), r.categoria.unwrap_or(pizza && (g.tipo == TipoUi::Pizza || n_series == 1)), |v| mud(Mudanca::RotulosCategoria(v))));
                if pizza {
                    col = col.push(alternar(tr!("Porcentagem"), r.percentual.unwrap_or(false), |v| mud(Mudanca::RotulosPercentual(v))));
                }
                col.push(secao(tr!("Posição")))
                    .push(escolhas(PosicaoRotulo::TODAS.iter().map(|q| (q.nome().to_owned(), r.posicao.unwrap_or_default() == *q, mud(Mudanca::RotulosPosicao(*q)))).collect(), 3))
                    .push(secao(tr!("Número")))
                    .push(escolhas([FormatoNum::Automatico, FormatoNum::Decimal { casas: 0 }, FormatoNum::Moeda { casas: 2 }, FormatoNum::Percentual { casas: 0 }]
                        .into_iter().map(|o| (o.nome().to_owned(), std::mem::discriminant(&o) == std::mem::discriminant(&f), mud(Mudanca::RotulosFormato(o)))).collect(), 2))
                    .push(linha(tr!("Casas decimais"), self.campo_num("rotulos.casas".into(), r.formato.filter(|f| *f != FormatoNum::Automatico).map(|f| f64::from(f.casas())), tr!("Auto"), Mudanca::RotulosCasas)))
                    .push(linha(tr!("Tamanho da letra"), self.campo_num("rotulos.tamanho".into(), r.fonte.tamanho.map(f64::from), tr!("Auto"), Mudanca::RotulosTamanho)))
                    .push(self.campo_cor(tr!("Cor"), AlvoCor::Rotulos, e, texto_padrao))
                    .into()
            }
            Grupo::Area => {
                let a = p.area;
                let ar = area(e, a);
                let tipo = match &ar.preenchimento {
                    None => TipoPreench::Automatico,
                    Some(Preenchimento::Nenhum) => TipoPreench::Nenhum,
                    Some(Preenchimento::Solido { .. }) => TipoPreench::Solido,
                    Some(Preenchimento::Degrade { .. }) => TipoPreench::Degrade,
                };
                let fundo_padrao = tema.map_or(CorEstilo::hex(0xffffff), |t| if a == AreaSel::Grafico { t.fundo } else { t.plotagem.unwrap_or(t.fundo) });
                let mut col = col
                    .push(escolhas(vec![
                        (tr!("Área do gráfico").into(), a == AreaSel::Grafico, ev(Evento::Selecionar(Selecao::Area(AreaSel::Grafico)))),
                        (tr!("Área de plotagem").into(), a == AreaSel::Plotagem, ev(Evento::Selecionar(Selecao::Area(AreaSel::Plotagem)))),
                    ], 2))
                    .push(secao(tr!("Preenchimento")))
                    .push(escolhas([(tr!("Automático"), TipoPreench::Automatico), (tr!("Nenhum"), TipoPreench::Nenhum), (tr!("Sólido"), TipoPreench::Solido), (tr!("Degradê"), TipoPreench::Degrade)]
                        .into_iter().map(|(t, v)| (t.to_owned(), tipo == v, mud(Mudanca::AreaTipo(a, v)))).collect(), 2))
                    .push(self.campo_cor(if tipo == TipoPreench::Degrade { tr!("Cor inicial") } else { tr!("Cor") }, AlvoCor::Fundo(a), e, fundo_padrao));
                if let Some(Preenchimento::Degrade { angulo, .. }) = &ar.preenchimento {
                    col = col.push(self.campo_cor(tr!("Cor final"), AlvoCor::DegradeFim(a), e, CorEstilo::hex(0xdfe7f2)))
                        .push(linha(tr!("Ângulo (graus)"), self.campo_num(format!("area{a:?}.angulo"), Some(f64::from(*angulo)), "90", move |t| Mudanca::AreaAngulo(a, t))));
                }
                let borda_padrao = a == AreaSel::Grafico && tema.and_then(|t| t.borda).is_some();
                col.push(secao(tr!("Borda")))
                    .push(alternar(tr!("Mostrar borda"), ar.borda.visivel.unwrap_or(borda_padrao), move |v| mud(Mudanca::AreaBorda(a, v))))
                    .push(self.campo_cor(tr!("Cor da borda"), AlvoCor::Borda(a), e, tema.and_then(|t| t.borda).unwrap_or(eixo_padrao)))
                    .push(linha(tr!("Espessura (px)"), self.campo_num(format!("area{a:?}.borda"), ar.borda.espessura.map(f64::from), "1", move |t| Mudanca::AreaBordaEspessura(a, t))))
                    .push(escolhas(Traco::TODOS.iter().map(|t| (t.nome().to_owned(), ar.borda.traco.unwrap_or_default() == *t, mud(Mudanca::AreaBordaTraco(a, *t)))).collect(), 2))
                    .push(secao(tr!("Efeitos")))
                    .push(alternar(tr!("Sombra"), ar.sombra.is_some() || (a == AreaSel::Grafico && ar.sombra.is_none() && tema.is_some_and(|t| t.sombra)), move |v| mud(Mudanca::AreaSombra(a, v))))
                    .push(linha(tr!("Cantos arredondados (px)"), self.campo_num(format!("area{a:?}.raio"), ar.raio.map(f64::from), "0", move |t| Mudanca::AreaRaio(a, t))))
                    .into()
            }
            Grupo::Margens => {
                let mut col = col;
                for (titulo, a, m) in [(tr!("Margens do gráfico (px)"), AreaSel::Grafico, &e.margens_grafico), (tr!("Margens da área de plotagem (px)"), AreaSel::Plotagem, &e.margens_plotagem)] {
                    let padrao = if a == AreaSel::Grafico { "10" } else { "0" };
                    let campo = |l: Lado, v: Option<f32>| self.campo_num(format!("margem{a:?}{l:?}"), v.map(f64::from), padrao, move |t| Mudanca::Margem(a, l, t));
                    col = col.push(secao(titulo))
                        .push(linha(tr!("Em cima"), campo(Lado::Superior, m.superior)))
                        .push(linha(tr!("Embaixo"), campo(Lado::Inferior, m.inferior)))
                        .push(linha(tr!("À esquerda"), campo(Lado::Esquerda, m.esquerda)))
                        .push(linha(tr!("À direita"), campo(Lado::Direita, m.direita)));
                }
                if barras {
                    col = col.push(secao(tr!("Colunas e barras")))
                        .push(linha(tr!("Espaço entre grupos (%)"), self.campo_num("barras.espaco".into(), e.barras.espacamento.map(f64::from), tr!("Auto"), Mudanca::BarrasEspaco)))
                        .push(linha(tr!("Sobreposição (%)"), self.campo_num("barras.sobrepor".into(), e.barras.sobreposicao.map(f64::from), "0", Mudanca::BarrasSobrepor)))
                        .push(linha(tr!("Cantos das barras (px)"), self.campo_num("barras.raio".into(), e.barras.raio.map(f64::from), "0", Mudanca::BarrasRaio)));
                }
                col.into()
            }
            Grupo::Fonte => col
                .push(rotulo(tr!("Família")))
                .push(escolhas(FAMILIAS.iter().map(|(n, f)| (tr!(*n).to_owned(), e.fonte.familia.as_deref() == *f, mud(Mudanca::FonteFamilia(*f)))).collect(), 2))
                .push(linha(tr!("Tamanho base"), self.campo_num("fonte.tamanho".into(), e.fonte.tamanho.map(f64::from), "12", Mudanca::FonteTamanho)))
                .push(alternar(tr!("Negrito"), e.fonte.negrito.unwrap_or(false), |v| mud(Mudanca::FonteNegrito(v))))
                .push(alternar(tr!("Itálico"), e.fonte.italico.unwrap_or(false), |v| mud(Mudanca::FonteItalico(v))))
                .push(self.campo_cor(tr!("Cor do texto"), AlvoCor::FonteGeral, e, texto_padrao))
                .into(),
            Grupo::Analiticos => {
                if n_series == 0 {
                    return col.push(rotulo(tr!("Sem séries nos dados."))).into();
                }
                let i = p.serie.min(n_series - 1);
                let an = e.serie(i).map(|x| x.analise.clone()).unwrap_or_default();
                let mut col = col.push(self.seletor_serie()).push(secao(tr!("Linha de tendência")));
                let atual = an.tendencia.as_ref().map(|t| t.tipo);
                let mut opcoes = vec![(tr!("Nenhuma").to_owned(), atual.is_none(), mud(Mudanca::Tendencia(i, None)))];
                opcoes.extend(TipoTendencia::TODOS.iter().map(|t| (t.nome().to_owned(), atual == Some(*t), mud(Mudanca::Tendencia(i, Some(*t))))));
                col = col.push(escolhas(opcoes, 2));
                if let Some(t) = &an.tendencia {
                    match t.tipo {
                        TipoTendencia::Polinomial => col = col.push(linha(tr!("Grau (2 a 6)"), self.campo_num(format!("tendencia{i}.grau"), Some(f64::from(t.grau)), "2", move |s| Mudanca::TendenciaGrau(i, s)))),
                        TipoTendencia::MediaMovel => col = col.push(linha(tr!("Período"), self.campo_num(format!("tendencia{i}.periodo"), Some(f64::from(t.periodo)), "2", move |s| Mudanca::TendenciaPeriodo(i, s)))),
                        _ => {
                            col = col
                                .push(linha(tr!("Prever para a frente"), self.campo_num(format!("tendencia{i}.avancar"), Some(t.avancar).filter(|v| *v > 0.0), "0", move |s| Mudanca::TendenciaAvancar(i, s))))
                                .push(linha(tr!("Prever para trás"), self.campo_num(format!("tendencia{i}.recuar"), Some(t.recuar).filter(|v| *v > 0.0), "0", move |s| Mudanca::TendenciaRecuar(i, s))))
                                .push(alternar(tr!("Mostrar a equação"), t.mostrar_equacao, move |v| mud(Mudanca::TendenciaEquacao(i, v))))
                                .push(alternar(tr!("Mostrar o R²"), t.mostrar_r2, move |v| mud(Mudanca::TendenciaR2(i, v))));
                        }
                    }
                }
                col = col.push(secao(tr!("Barras de erro")));
                let tipo_erro = an.erro.as_ref().map(|b| match b.quantia {
                    QuantiaErro::Fixo(_) => 0u8,
                    QuantiaErro::Percentual(_) => 1,
                    QuantiaErro::DesvioPadrao(_) => 2,
                    QuantiaErro::ErroPadrao => 3,
                });
                let nomes = [tr!("Valor fixo"), tr!("Porcentagem"), tr!("Desvio-padrão"), tr!("Erro-padrão")];
                let mut opcoes = vec![(tr!("Nenhuma").to_owned(), tipo_erro.is_none(), mud(Mudanca::Erro(i, None)))];
                opcoes.extend(nomes.iter().enumerate().map(|(k, n)| (n.to_string(), tipo_erro == Some(k as u8), mud(Mudanca::Erro(i, Some(k as u8))))));
                col = col.push(escolhas(opcoes, 2));
                if let Some(b) = &an.erro {
                    let valor = match b.quantia {
                        QuantiaErro::Fixo(v) => Some((tr!("Valor"), v)),
                        QuantiaErro::Percentual(v) => Some((tr!("Porcentagem (%)"), v)),
                        QuantiaErro::DesvioPadrao(v) => Some((tr!("Quantos desvios"), v)),
                        QuantiaErro::ErroPadrao => None,
                    };
                    if let Some((r, v)) = valor {
                        col = col.push(linha(r, self.campo_num(format!("erro{i}.valor"), Some(v), "1", move |s| Mudanca::ErroValor(i, s))));
                    }
                    col = col
                        .push(escolhas(DirecaoErro::TODAS.iter().map(|d| (d.nome().to_owned(), b.direcao == *d, mud(Mudanca::ErroDirecao(i, *d)))).collect(), 3))
                        .push(alternar(tr!("Sem ponta"), b.sem_ponta, move |v| mud(Mudanca::ErroSemPonta(i, v))));
                }
                col.push(secao(tr!("Linhas de referência")))
                    .push(alternar(tr!("Máximo"), an.maximo, move |v| mud(Mudanca::Referencia(i, 0, v))))
                    .push(alternar(tr!("Mínimo"), an.minimo, move |v| mud(Mudanca::Referencia(i, 1, v))))
                    .push(alternar(tr!("Média"), an.media, move |v| mud(Mudanca::Referencia(i, 2, v))))
                    .into()
            }
            Grupo::Dica => {
                let d = &e.dica;
                let atual = d.formato;
                let mut formatos = vec![(tr!("Como os rótulos").to_owned(), atual.is_none(), mud(Mudanca::DicaFormato(None)))];
                formatos.extend([FormatoNum::Automatico, FormatoNum::Decimal { casas: 2 }, FormatoNum::Moeda { casas: 2 }, FormatoNum::Percentual { casas: 1 }]
                    .into_iter().map(|o| (o.nome().to_owned(), atual.is_some_and(|a| std::mem::discriminant(&a) == std::mem::discriminant(&o)), mud(Mudanca::DicaFormato(Some(o))))));
                col.push(rotulo(tr!("Aparece ao passar o mouse sobre um ponto do gráfico na planilha.")))
                    .push(alternar(tr!("Mostrar a dica"), d.visivel.unwrap_or(true), |v| mud(Mudanca::DicaVisivel(v))))
                    .push(secao(tr!("Conteúdo")))
                    .push(alternar(tr!("Categoria no título"), d.categoria.unwrap_or(true), |v| mud(Mudanca::DicaCategoria(v))))
                    .push(alternar(tr!("Nome da série"), d.serie.unwrap_or(true), |v| mud(Mudanca::DicaSerie(v))))
                    .push(alternar(if pizza { tr!("Porcentagem do total") } else { tr!("Porcentagem da categoria") }, d.percentual, |v| mud(Mudanca::DicaPercentual(v))))
                    .push(secao(tr!("Número")))
                    .push(escolhas(formatos, 2))
                    .push(rotulo(tr!("Antes do valor")))
                    .push(text_input(tr!("Ex.: R$ "), d.prefixo.clone()).size(12).on_input(|t| mud(Mudanca::DicaPrefixo(t))))
                    .push(rotulo(tr!("Depois do valor")))
                    .push(text_input(tr!("Ex.:  kg"), d.sufixo.clone()).size(12).on_input(|t| mud(Mudanca::DicaSufixo(t))))
                    .push(secao(tr!("Aparência")))
                    .push(linha(tr!("Tamanho da letra"), self.campo_num("dica.tamanho".into(), d.tamanho.map(f64::from), "12", Mudanca::DicaTamanho)))
                    .push(self.campo_cor(tr!("Fundo"), AlvoCor::DicaFundo, e, CorEstilo::hex(0xffffff)))
                    .push(self.campo_cor(tr!("Texto"), AlvoCor::DicaTexto, e, CorEstilo::hex(0x202020)))
                    .into()
            }
            Grupo::Formas => col.into(),
            Grupo::Tamanho => col
                .push(linha(tr!("Largura (px)"), text_input("480", &g.aparencia.largura).size(12).width(Length::Fixed(96.0))
                    .on_input(|s| ev(Evento::Estilo(CampoEstilo::Largura, s)))))
                .push(linha(tr!("Altura (px)"), text_input("300", &g.aparencia.altura).size(12).width(Length::Fixed(96.0))
                    .on_input(|s| ev(Evento::Estilo(CampoEstilo::Altura, s)))))
                .push(rotulo(tr!("Arraste o gráfico na planilha para mudar a posição.")))
                .into(),
            Grupo::TextoAlt => col
                .push(rotulo(tr!("Descrição para leitores de tela")))
                .push(text_input(tr!("Ex.: vendas por mês, 2026"), e.texto_alternativo.clone()).size(12).on_input(|s| mud(Mudanca::TextoAlt(s))))
                .into(),
        }
    }

    /// Rola a lista para o cabeçalho do grupo aberto ficar no alto.
    fn rolar_para_grupo(&self) -> Task<Msg> {
        let Some(g) = self.graficos.painel.grupo else { return Task::none() };
        let tipo = self.graficos.tipo;
        let i = Grupo::PERSONALIZAR.iter().filter(|x| x.existe_em(tipo)).position(|x| *x == g).unwrap_or(0);
        Task::scroll_to(ID_ROLAGEM, pelicano::Point::new(0.0, i as f32 * ALTURA_CABECALHO))
    }

    /// Eventos do painel novo.
    pub(super) fn evento_painel(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Grupo(grupo) => {
                let p = &mut self.graficos.painel;
                p.grupo = if p.grupo == Some(grupo) { None } else { Some(grupo) };
                p.cor_menu = None;
                self.graficos.cor_ativa = None;
                return self.rolar_para_grupo();
            }
            Evento::Selecionar(s) => {
                let p = &mut self.graficos.painel;
                match s {
                    Selecao::Eixo(x) => p.eixo = x,
                    Selecao::Serie(i) => p.serie = i,
                    Selecao::Ponto(i) => p.ponto = i,
                    Selecao::Area(a) => p.area = a,
                }
                p.cor_menu = None;
                self.graficos.cor_ativa = None;
            }
            Evento::CorMenu(alvo) => {
                self.graficos.painel.cor_menu = alvo;
                self.graficos.cor_ativa = None;
            }
            Evento::MaisCores(alvo) => {
                let ja = self.graficos.cor_ativa == Some(AlvoCorGrafico::Estilo(alvo));
                self.graficos.cor_ativa = None;
                if !ja {
                    let _ = self.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Estilo(alvo)));
                }
            }
            Evento::Cor(alvo, hex) => {
                let cor = hex.as_deref().and_then(CorEstilo::de_texto);
                self.mudar_estilo(|e| {
                    definir_cor(e, alvo, cor);
                    true
                }, None);
            }
            Evento::Formatar(m) => {
                let chave_campo = chave(&m);
                if let (Some(k), Some(t)) = (&chave_campo, texto_da(&m)) {
                    self.graficos.painel.rascunhos.insert(k.clone(), t.to_owned());
                }
                if matches!(m, Mudanca::Tema(_)) {
                    self.graficos.painel.rascunhos.clear();
                }
                let campo: Option<&'static str> = chave_campo.map(|k| &*Box::leak(k.into_boxed_str()));
                self.mudar_estilo(|e| aplicar(e, &m), campo);
            }
            _ => {}
        }
        Task::none()
    }

    /// Muda o estilo (convertendo o antigo na primeira vez) e grava ao vivo.
    pub(super) fn mudar_estilo(&mut self, f: impl FnOnce(&mut EstiloGrafico) -> bool, campo: Option<&'static str>) {
        let mut e = self.estilo_vista();
        if !f(&mut e) {
            return;
        }
        if !e.valido() {
            self.graficos.mensagem = tr!("Valor fora da faixa (mínimo deve ser menor que o máximo).").into();
            return;
        }
        self.graficos.aparencia.estilo = Some(e);
        sincronizar_legado(&mut self.graficos.aparencia);
        self.editar_ao_vivo(campo);
    }
}

impl App {
    /// `FEATHERSHEET_CAPTURAR_GRAFICO=<grupo>` (tipo, dados, titulo, legenda, eixos,
    /// grades, series, pontos, rotulos, area, margens, fonte, analiticos, formas,
    /// tamanho, alt, cor ou nenhum): tabela e gráfico de exemplo com tema, um
    /// ponto pintado e margens alteradas, e o painel aberto nesse grupo.
    pub(in crate::app) fn preparar_captura_grafico(&mut self, modo: &str) -> Task<Msg> {
        // `tipo:<nome>`: um tipo especial com dados próprios, sem o painel.
        if let Some(nome) = modo.strip_prefix("tipo:") {
            return self.preparar_captura_tipo(nome, None);
        }
        // `painel:<nome>` / `personalizar:<nome>`: o mesmo, com o painel
        // aberto na aba Configurar (ou Personalizar) desse tipo;
        // `painel-fim:<nome>` rola a aba Configurar até o fim.
        if let Some(nome) = modo.strip_prefix("painel:") {
            return self.preparar_captura_tipo(nome, Some(AbaPainel::Configurar));
        }
        if let Some(nome) = modo.strip_prefix("painel-fim:") {
            let _ = self.preparar_captura_tipo(nome, Some(AbaPainel::Configurar));
            self.config.faixa_recolhida = true;
            return Task::scroll_to(ID_ROLAGEM, pelicano::Point::new(0.0, 400.0));
        }
        if let Some(nome) = modo.strip_prefix("personalizar:") {
            return self.preparar_captura_tipo(nome, Some(AbaPainel::Personalizar));
        }
        // `vitrine`, `vitrine-painel`, `vitrine-legenda`: os prints do README.
        if let Some(variante) = modo.strip_prefix("vitrine") {
            return self.preparar_vitrine(variante.trim_start_matches('-'));
        }
        // `boletim`: o modelo Boletim de notas, A7:G22 em colunas empilhadas
        // com Prova 1, Prova 2 e Trabalho, e a aba Configurar aberta.
        if modo == "boletim" || modo == "boletim-pizza" {
            if let Some(wb) = feathersheet_core::modelos::criar("notas-escolares") {
                self.workbook = wb;
                self.folha = 0;
            }
            (self.linha, self.coluna, self.fim_selecao) = (6, 0, (21, 6));
            let _ = self.evento_grafico(Evento::Abrir);
            let pizza = modo == "boletim-pizza";
            let _ = self.evento_grafico(Evento::Tipo(if pizza { TipoUi::Pizza } else { TipoUi::ColunasEmpilhadas }));
            let _ = self.evento_grafico(Evento::Titulo(if pizza { tr!("Alunos por situação") } else { tr!("Notas por aluno") }.into()));
            if pizza {
                let _ = self.evento_grafico(Evento::CategoriaColuna(Some(6)));
                let _ = self.evento_grafico(Evento::SerieVisivel(1, true));
                let _ = self.evento_grafico(Evento::SerieConta(1, Some(AggregateFn::Count)));
            } else {
                for n in [0, 4, 5] {
                    let _ = self.evento_grafico(Evento::SerieVisivel(n, false));
                }
                let _ = self.evento_grafico(Evento::SerieNome(3, tr!("Trabalho em grupo").into()));
            }
            self.graficos.painel.aba = AbaPainel::Configurar;
            return Task::none();
        }
        let _ = self.update(Msg::Colar("Mês\tVendas\tMeta\nJan\t12\t10\nFev\t18\t14\nMar\t9\t15\nAbr\t22\t17\nMai\t16\t18".into()));
        self.linha = 0;
        self.coluna = 0;
        self.fim_selecao = (0, 0);
        let _ = self.evento_grafico(Evento::Abrir);
        let _ = self.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let _ = self.evento_grafico(Evento::Titulo(tr!("Vendas por mês").into()));
        for m in [
            Mudanca::Tema(Some("oceano")),
            Mudanca::Margem(AreaSel::Grafico, Lado::Superior, "22".into()),
            Mudanca::Margem(AreaSel::Grafico, Lado::Esquerda, "22".into()),
            Mudanca::Margem(AreaSel::Grafico, Lado::Direita, "22".into()),
            Mudanca::Margem(AreaSel::Plotagem, Lado::Esquerda, "10".into()),
            Mudanca::Subtitulo(tr!("1º semestre de 2026").into()),
            Mudanca::RotulosVisivel(true),
            Mudanca::EixoTitulo(EixoSel::Y, tr!("R$ mil").into()),
        ] {
            let _ = self.evento_grafico(Evento::Formatar(m));
        }
        let _ = self.evento_grafico(Evento::Cor(AlvoCor::Ponto(0, 3), Some("#d81b60".into())));
        self.graficos.painel.ponto = 3;
        let grupo = match modo {
            "tipo" => Some(Grupo::TipoEstilo),
            "dados" => Some(Grupo::Dados),
            "titulo" => Some(Grupo::Titulo),
            "legenda" => Some(Grupo::Legenda),
            "eixos" => Some(Grupo::Eixos),
            "grades" => Some(Grupo::Grades),
            "series" | "cor" => Some(Grupo::Series),
            "pontos" => Some(Grupo::Pontos),
            "rotulos" => Some(Grupo::Rotulos),
            "area" => Some(Grupo::Area),
            "margens" => Some(Grupo::Margens),
            "fonte" => Some(Grupo::Fonte),
            "tamanho" => Some(Grupo::Tamanho),
            "alt" => Some(Grupo::TextoAlt),
            "analiticos" => Some(Grupo::Analiticos),
            "dica" => Some(Grupo::Dica),
            _ => None,
        };
        if modo == "analiticos" {
            for m in [
                Mudanca::Tendencia(1, Some(TipoTendencia::Linear)),
                Mudanca::TendenciaEquacao(1, true),
                Mudanca::TendenciaR2(1, true),
                Mudanca::Erro(0, Some(1)),
                Mudanca::Referencia(0, 0, true),
                Mudanca::Referencia(0, 2, true),
            ] {
                let _ = self.evento_grafico(Evento::Formatar(m));
            }
            self.graficos.painel.serie = 1;
        }
        self.graficos.painel.grupo = grupo;
        if grupo.is_some() || modo == "nenhum" {
            self.graficos.painel.aba = AbaPainel::Personalizar;
        }
        if modo == "cor" {
            self.graficos.painel.grupo = Some(Grupo::Legenda);
            self.graficos.painel.cor_menu = Some(AlvoCor::Legenda);
        }
        self.rolar_para_grupo()
    }
}

pub(super) const LARGURA_PAINEL: f32 = 372.0;
const ID_ROLAGEM: &str = "painel-formatar-grafico";
/// Altura de um cabeçalho do acordeão (botão + espaço entre eles).
const ALTURA_CABECALHO: f32 = 35.0;

fn cortar(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aplicar_mudancas_numeros_pela_metade_e_cores() {
        let mut e = EstiloGrafico::default();
        assert!(aplicar(&mut e, &Mudanca::EixoNum(EixoSel::Y, CampoEixo::Minimo, "1,5".into())));
        assert_eq!(e.eixo_y.minimo, Some(1.5));
        assert!(!aplicar(&mut e, &Mudanca::EixoNum(EixoSel::Y, CampoEixo::Maximo, "1,".into())) || e.eixo_y.maximo == Some(1.0));
        assert!(!aplicar(&mut e, &Mudanca::EixoNum(EixoSel::Y, CampoEixo::Maximo, "abc".into())));
        assert!(aplicar(&mut e, &Mudanca::EixoNum(EixoSel::Y, CampoEixo::Minimo, "".into())));
        assert_eq!(e.eixo_y.minimo, None);
        aplicar(&mut e, &Mudanca::Margem(AreaSel::Plotagem, Lado::Esquerda, "20".into()));
        assert_eq!(e.margens_plotagem.esquerda, Some(20.0));
        aplicar(&mut e, &Mudanca::AreaTipo(AreaSel::Grafico, TipoPreench::Degrade));
        definir_cor(&mut e, AlvoCor::DegradeFim(AreaSel::Grafico), Some(CorEstilo::hex(0x112233)));
        assert!(matches!(e.area_grafico.preenchimento, Some(Preenchimento::Degrade { fim, .. }) if fim == CorEstilo::hex(0x112233)));
        definir_cor(&mut e, AlvoCor::Ponto(0, 2), Some(CorEstilo::hex(0xff0000)));
        assert_eq!(cor_de(&e, AlvoCor::Ponto(0, 2)), Some(CorEstilo::hex(0xff0000)));
        aplicar(&mut e, &Mudanca::PontoLimpar(0, 2));
        assert_eq!(cor_de(&e, AlvoCor::Ponto(0, 2)), None);
        aplicar(&mut e, &Mudanca::ItemMover(1, -1, 3));
        assert_eq!(e.legenda.ordem, vec![1, 0, 2]);
        aplicar(&mut e, &Mudanca::SerieTransparencia(0, "25".into()));
        assert_eq!(e.series[&0].opacidade, Some(0.75));
    }

    #[test]
    fn migrar_campos_antigos_e_excel() {
        let a = Aparencia { titulo: false, legenda: false, valores: true, subtitulo: "Sub".into(), eixo_y: "R$".into(),
            fundo: "#00000000".into(), formato: "moeda:2".into(), ..Default::default() };
        let excel: Map<String, Value> = serde_json::from_value(serde_json::json!({
            "fundoDegrade": [[1.0, "#2f5597"], [0.0, "#000000"]], "fundoAngulo": 270.0, "contorno": "#898989",
            "corTexto": "#bfbfbf", "eixoValores": false })).unwrap();
        let e = migrar(&a, Some(&excel));
        assert_eq!(e.titulo.visivel, Some(false));
        assert_eq!(e.legenda.posicao, Some(PosicaoLegenda::Nenhuma));
        assert_eq!(e.rotulos.visivel, Some(true));
        assert_eq!(e.eixo_y.titulo.as_deref(), Some("R$"));
        assert_eq!(e.eixo_y.formato, Some(FormatoNum::Moeda { casas: 2 }));
        assert!(matches!(e.area_grafico.preenchimento, Some(Preenchimento::Degrade { inicio, angulo, .. }) if inicio == CorEstilo::hex(0) && angulo == 270.0));
        assert_eq!(e.fonte.cor, Some(CorEstilo::hex(0xbfbfbf)));
        assert_eq!(e.eixo_y.visivel, Some(false));
        let mut b = Aparencia { estilo: Some(e), ..Default::default() };
        sincronizar_legado(&mut b);
        assert!(!b.titulo && !b.legenda && b.valores);
        assert_eq!(b.fundo, "#000000");
    }
}

impl App {
    /// Vitrine para os prints: o modelo Fluxo de caixa com o gráfico de
    /// entradas e saídas (tema, tendência e média) ao lado da tabela, a dica
    /// aberta numa coluna e, conforme a variante, o painel ou um item da
    /// legenda desligado.
    fn preparar_vitrine(&mut self, variante: &str) -> Task<Msg> {
        if let Some(wb) = feathersheet_core::modelos::criar("fluxo-caixa") {
            self.workbook = wb;
            self.folha = 0;
        }
        (self.linha, self.coluna, self.fim_selecao) = (6, 0, (18, 5));
        let _ = self.evento_grafico(Evento::Abrir);
        let _ = self.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let _ = self.evento_grafico(Evento::Titulo("Entradas e saídas em 2026".into()));
        // Séries pela ordem de origem: 0 Saldo inicial, 1 Entradas, 2 Saídas,
        // 3 Resultado do mês, 4 Saldo acumulado.
        for n in [0, 3, 4] {
            let _ = self.evento_grafico(Evento::SerieVisivel(n, false));
        }
        for m in [
            Mudanca::Tema(Some("corporativo")),
            Mudanca::Subtitulo("Fluxo de caixa mensal, em R$".into()),
            Mudanca::BarrasRaio("4".into()),
            Mudanca::GradeVisivel(EixoSel::Y, true, true),
            Mudanca::Tendencia(0, Some(TipoTendencia::Linear)),
            Mudanca::TendenciaEquacao(0, true),
            Mudanca::TendenciaR2(0, true),
            Mudanca::Referencia(1, 2, true),
            Mudanca::DicaPercentual(true),
            Mudanca::DicaPrefixo("R$ ".into()),
        ] {
            let _ = self.evento_grafico(Evento::Formatar(m));
        }
        let Some(id) = self.graficos.id.clone() else { return Task::none() };
        // Ao lado da tabela (colunas A:F), alto o bastante para os 12 meses.
        if let Some(aba) = self.workbook.aba_mut(self.folha) {
            let _ = feathersheet_io::graficos_salvos::salvar(&mut aba.sheet, &id,
                serde_json::json!({"position": {"x": 880.0, "y": 96.0, "width": 620.0, "height": 380.0}}));
        }
        self.graficos.aparencia.largura = "620".into();
        self.graficos.aparencia.altura = "380".into();
        if variante == "painel" {
            self.graficos.painel.aba = AbaPainel::Personalizar;
            self.graficos.painel.grupo = Some(Grupo::Analiticos);
            self.graficos.painel.serie = 0;
            (self.linha, self.coluna, self.fim_selecao) = (0, 3, (0, 3));
            self.origem = Some("Fluxo de caixa 2026.fshz".into());
            // O painel ocupa a direita: a planilha começa na coluna D para o
            // gráfico aparecer inteiro ao lado dele.
            self.coluna_inicio = 3;
            return self.rolar_para_grupo();
        }
        let _ = self.evento_grafico(Evento::Fechar);
        self.graficos.selecionado = None;
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
        let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) else { return Task::none() };
        let Some(v) = lista.into_iter().find(|v| v["id"].as_str() == Some(&id)) else { return Task::none() };
        let Some(mapa) = super::editor::desenhar_grafico_salvo(&aba.sheet, &v, None).and_then(|d| d.mapa) else { return Task::none() };
        let zoom = (self.config.zoom as f32 / 100.).max(0.01);
        if variante == "legenda" {
            if let Some(item) = mapa.legenda.iter().find(|i| i.indice == 1) {
                let p = pelicano::Point::new((item.x + item.largura / 2.0) * zoom, (item.y + item.altura / 2.0) * zoom);
                let _ = self.evento_grafico(Evento::Pairar(id.clone(), p));
                let _ = self.evento_grafico(Evento::Arrastar(id.clone()));
                self.graficos.interacao.transicao = None;
                self.graficos.selecionado = None;
            }
        }
        // Dica sobre a coluna de junho da primeira série.
        if let Some(alvo) = mapa.pontos.iter().find(|p| p.serie == 0 && p.ponto == 5) {
            if let feather_charts::formatado::FormaAlvo::Retangulo { x, y, largura, .. } = alvo.forma {
                let p = pelicano::Point::new((x + largura / 2.0) * zoom, (y + 12.0) * zoom);
                let _ = self.evento_grafico(Evento::Pairar(id, p));
            }
        }
        self.graficos.interacao.fixa = true;
        // Sem a seleção azul em cima da tabela; título com o nome do modelo.
        (self.linha, self.coluna, self.fim_selecao) = (0, 0, (0, 0));
        self.origem = Some("Fluxo de caixa 2026.fshz".into());
        Task::none()
    }
}

impl App {
    /// Foto de um tipo especial (radar, cascata, Gantt…) com dados que
    /// fazem sentido para ele, desenhado na planilha com o tema Oceano.
    fn preparar_captura_tipo(&mut self, nome: &str, painel: Option<AbaPainel>) -> Task<Msg> {
        let (tipo, dados) = match nome {
            "pizza" => (TipoUi::Pizza, "Situação\tAlunos\tMédia\nAprovado\t18\t7,8\nRecuperação\t6\t5,4\nReprovado\t3\t3,9"),
            "rosca" => (TipoUi::Rosca, "Situação\tAlunos\nAprovado\t18\nRecuperação\t6\nReprovado\t3"),
            "rosca2" => (TipoUi::Rosca, "Região\tVendas 2025\tVendas 2026\nNorte\t18\t22\nSul\t12\t9\nLeste\t7\t11\nOeste\t5\t8"),
            "colunas" => (TipoUi::Colunas, "Mês\tVendas\tMeta\nJan\t12\t10\nFev\t18\t14\nMar\t9\t15\nAbr\t22\t17"),
            "dispersao" => (TipoUi::Dispersao, "Preço\tVendas\tLucro\n10\t120\t30\n15\t95\t45\n22\t80\t60\n30\t52\t40\n38\t40\t72"),
            "radar" => (TipoUi::Radar, "Critério\tProduto A\tProduto B\nPreço\t8\t6\nQualidade\t7\t9\nPrazo\t9\t6\nSuporte\t6\t8\nDesign\t7\t8\nGarantia\t5\t7"),
            "cascata" => (TipoUi::Cascata, "Item\tValor\nReceita\t120\nCustos\t-45\nImpostos\t-18\nServiços\t30\nDespesas\t-22\nOutros\t9"),
            "histograma" => (TipoUi::Histograma, "Aluno\tNota\nA\t4,5\nB\t6\nC\t7,5\nD\t8\nE\t5,5\nF\t9\nG\t6,5\nH\t7\nI\t7,2\nJ\t8,8\nK\t3,9\nL\t6,1\nM\t7,8\nN\t5,2\nO\t9,4\nP\t6,9"),
            "combinado" => (TipoUi::Combinado, "Mês\tVendas\tMeta\tMédia do setor\nJan\t120\t110\t100\nFev\t135\t115\t104\nMar\t128\t120\t110\nAbr\t150\t125\t112\nMai\t162\t130\t118\nJun\t158\t135\t121"),
            "bolhas" => (TipoUi::Bolhas, "Preço\tVendas\tLucro\n10\t120\t30\n15\t95\t45\n22\t80\t60\n30\t52\t40\n38\t40\t72\n45\t25\t35"),
            "velas" => (TipoUi::Candlestick, "Dia\tAbertura\tMáxima\tMínima\tFechamento\nSeg\t20\t24\t18\t23\nTer\t23\t26\t21\t22\nQua\t22\t25\t19\t24\nQui\t24\t29\t23\t28\nSex\t28\t30\t25\t26"),
            "gantt" => (TipoUi::Gantt, "Tarefa\tInício\tFim\nLevantamento\t1\t5\nProjeto\t4\t12\nCompras\t10\t16\nObra\t14\t28\nEntrega\t27\t30"),
            "calor" => (TipoUi::MapaCalor, "Hora\tSeg\tTer\tQua\tQui\tSex\n8h\t12\t15\t9\t14\t20\n10h\t30\t28\t35\t31\t26\n12h\t45\t50\t41\t47\t55\n14h\t25\t22\t30\t27\t33\n16h\t18\t20\t16\t21\t40"),
            "linha3d" => (TipoUi::Linha3d, "X\tY\tZ\n1\t2\t1\n2\t4\t3\n3\t3\t5\n4\t6\t4\n5\t5\t7\n6\t8\t6"),
            "area-empilhada" => (TipoUi::AreaDegrausEmpilhada, "Mês\tNorte\tSul\tLeste\nJan\t12\t8\t5\nFev\t15\t9\t7\nMar\t11\t12\t8\nAbr\t18\t10\t9\nMai\t16\t14\t11\nJun\t20\t13\t12"),
            _ => (TipoUi::Colunas, "A\tB\nx\t1"),
        };
        let _ = self.update(Msg::Colar(dados.into()));
        let linhas = dados.lines().count() as u32;
        let colunas = dados.lines().next().map_or(1, |l| l.split('\t').count()) as u32;
        (self.linha, self.coluna, self.fim_selecao) = (0, 0, (linhas - 1, colunas - 1));
        let _ = self.evento_grafico(Evento::Abrir);
        let _ = self.evento_grafico(Evento::Tipo(tipo));
        let _ = self.evento_grafico(Evento::Titulo(tipo.to_string()));
        let _ = self.evento_grafico(Evento::Formatar(Mudanca::Tema(Some("oceano"))));
        if let Some(aba) = painel {
            self.graficos.painel.aba = aba;
            return Task::none();
        }
        let Some(id) = self.graficos.id.clone() else { return Task::none() };
        if let Some(aba) = self.workbook.aba_mut(self.folha) {
            let _ = feathersheet_io::graficos_salvos::salvar(&mut aba.sheet, &id,
                serde_json::json!({"position": {"x": 560.0, "y": 30.0, "width": 600.0, "height": 380.0}}));
        }
        let _ = self.evento_grafico(Evento::Fechar);
        self.graficos.selecionado = None;
        (self.linha, self.coluna, self.fim_selecao) = (0, 0, (0, 0));
        Task::none()
    }
}
