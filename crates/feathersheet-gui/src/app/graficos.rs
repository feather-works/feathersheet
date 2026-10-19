//! Painel Iced: extração no motor Sheet, desenho no Charts compartilhado.
use super::*;
mod editor;
mod interacao;
mod painel;
mod posicionamento;
mod seletor_tipo;
use posicionamento::{Alca, Quadro};
use feather_charts::{Grafico, Tipo};
use feathersheet_core::celulas::refs::{format_ref, parse_range_ref};
use feathersheet_core::grafico::chart::{AggregateFn, ChartConfig, ChartRange, ChartType};
use feather_ui::seletor_cor::{self, Evento as EventoCor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoUi {
    #[default]
    Colunas,
    ColunasEmpilhadas,
    Colunas100,
    Barras,
    BarrasEmpilhadas,
    Barras100,
    Linhas,
    Linha3d,
    Degraus,
    DegrausEmpilhados,
    Degraus100,
    Area,
    AreaDegraus,
    AreaDegrausEmpilhada,
    AreaDegraus100,
    Pizza,
    Dispersao,
    Bolhas,
    Combinado,
    MapaCalor,
    Cascata,
    Histograma,
    Radar,
    Candlestick,
    Gantt,
}
impl TipoUi {
    #[cfg(test)]
    const TODOS: [Self; 25] = [
        Self::Colunas,
        Self::ColunasEmpilhadas,
        Self::Colunas100,
        Self::Barras,
        Self::BarrasEmpilhadas,
        Self::Barras100,
        Self::Linhas,
        Self::Linha3d,
        Self::Degraus,
        Self::DegrausEmpilhados,
        Self::Degraus100,
        Self::Area,
        Self::AreaDegraus,
        Self::AreaDegrausEmpilhada,
        Self::AreaDegraus100,
        Self::Pizza,
        Self::Dispersao,
        Self::Bolhas,
        Self::Combinado,
        Self::MapaCalor,
        Self::Cascata,
        Self::Histograma,
        Self::Radar,
        Self::Candlestick,
        Self::Gantt,
    ];
    fn motor(self) -> ChartType {
        match self {
            Self::Colunas => ChartType::Column,
            Self::ColunasEmpilhadas => ChartType::ColumnStacked,
            Self::Colunas100 => ChartType::ColumnStacked100,
            Self::Barras => ChartType::Bar,
            Self::BarrasEmpilhadas => ChartType::BarStacked,
            Self::Barras100 => ChartType::BarStacked100,
            Self::Linhas => ChartType::Line,
            Self::Linha3d => ChartType::Line3d,
            Self::Degraus => ChartType::StepLine,
            Self::DegrausEmpilhados => ChartType::StepLineStacked,
            Self::Degraus100 => ChartType::StepLineStacked100,
            Self::Area => ChartType::Area,
            Self::AreaDegraus => ChartType::StepArea,
            Self::AreaDegrausEmpilhada => ChartType::StepAreaStacked,
            Self::AreaDegraus100 => ChartType::StepAreaStacked100,
            Self::Pizza => ChartType::Column,
            Self::Dispersao => ChartType::Scatter,
            Self::Bolhas => ChartType::Bubble,
            Self::Combinado => ChartType::Combo,
            Self::MapaCalor => ChartType::Heatmap,
            Self::Cascata => ChartType::Waterfall,
            Self::Histograma => ChartType::Histogram,
            Self::Radar => ChartType::Radar,
            Self::Candlestick => ChartType::Candlestick,
            Self::Gantt => ChartType::Gantt,
        }
    }
    /// Os sete tipos anteriores continuam usando o caminho vetorial validado.
    /// Os demais são despachados por featheroffice-charts::motor.
    fn avancado(self) -> bool {
        !matches!(self, Self::Colunas | Self::Barras | Self::Linhas | Self::Area | Self::Degraus | Self::Pizza | Self::Dispersao
            | Self::ColunasEmpilhadas | Self::Colunas100 | Self::BarrasEmpilhadas | Self::Barras100)
    }
}
impl std::fmt::Display for TipoUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Colunas => tr!("Colunas"),
            Self::ColunasEmpilhadas => tr!("Colunas empilhadas"),
            Self::Colunas100 => tr!("Colunas 100%"),
            Self::Barras => tr!("Barras"),
            Self::BarrasEmpilhadas => tr!("Barras empilhadas"),
            Self::Barras100 => tr!("Barras 100%"),
            Self::Linhas => tr!("Linhas"),
            Self::Linha3d => tr!("Linha 3D"),
            Self::Degraus => tr!("Degraus"),
            Self::DegrausEmpilhados => tr!("Degraus empilhados"),
            Self::Degraus100 => tr!("Degraus 100%"),
            Self::Area => tr!("Áreas"),
            Self::AreaDegraus => tr!("Área degraus"),
            Self::AreaDegrausEmpilhada => tr!("Área empilhada"),
            Self::AreaDegraus100 => tr!("Área 100%"),
            Self::Pizza => tr!("Pizza"),
            Self::Dispersao => tr!("Dispersão"),
            Self::Bolhas => tr!("Bolhas"),
            Self::Combinado => tr!("Combinado"),
            Self::MapaCalor => tr!("Mapa de calor"),
            Self::Cascata => tr!("Cascata"),
            Self::Histograma => tr!("Histograma"),
            Self::Radar => tr!("Radar"),
            Self::Candlestick => tr!("Candlestick"),
            Self::Gantt => tr!("Gantt"),
        })
    }
}
#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    FecharSeletor,
    Cursor(iced::Point),
    /// Mouse sobre o gráfico parado (posição relativa ao canto dele).
    Pairar(String, iced::Point),
    SairDoGrafico(String),
    /// Um quadro da transição da legenda.
    QuadroTransicao,
    Arrastar(String),
    Redimensionar(String, Alca),
    Soltar,
    Remover(String),
    Editar(String),
    /// Botão direito no gráfico: menu com Editar e Excluir.
    Contexto(String),
    /// O mouse passou por um item do menu (é a mensagem de abrir submenu;
    /// o menu do gráfico não tem): nada muda. Era `FecharContexto`, e o
    /// menu sumia antes do clique (17/10/2026).
    PassouNoMenu,
    FecharContexto,
    Inserir,
    /// Ícone de tipo da faixa: cria direto o gráfico da seleção nesse tipo
    /// (`None` = Recomendados, o tipo escolhido pelos dados).
    Rapido(Option<TipoUi>),
    Estilo(CampoEstilo, String),
    AbrirSeletorCor(AlvoCorGrafico),
    SeletorCor(EventoCor),
    Visibilidade(CampoVisivel, bool),
    CorSerie(usize, String),
    NomeSerie(usize, String),
    Intervalo(String),
    Titulo(String),
    Tipo(TipoUi),
    AlternarSeletor,
    Cabecalho(bool),
    Categorias(bool),
    Colunas(bool),
    Agrupar(bool),
    Serie(String),
    /// Aba Configurar: coluna das categorias (`None` = sem categorias).
    CategoriaColuna(Option<u32>),
    /// Mostrar/ocultar a série de origem `n`.
    SerieVisivel(u32, bool),
    /// Conta da série `n` (`None` = não resumir: vale para todas).
    SerieConta(u32, Option<AggregateFn>),
    /// Nome na legenda da série de origem `n`.
    SerieNome(u32, String),
    /// Intervalo = a seleção atual da planilha.
    UsarSelecao,
    /// Copia o gráfico (menu do botão direito ou Ctrl+C).
    Copiar(String),
    /// Cola o gráfico copiado na aba ativa.
    Colar,
    /// Copia e cola na hora, deslocado (Ctrl+D).
    Duplicar(String),
    /// Troca entre as abas Configurar e Personalizar do painel.
    AbaPainel(painel::AbaPainel),
    Atualizar,
    Exportar,
    Exportado(Result<bool, String>),
    /// Painel Formatar gráfico: abre/fecha um grupo do acordeão.
    Grupo(painel::Grupo),
    Selecionar(painel::Selecao),
    /// Abre (ou fecha, com `None`) a paleta de uma amostra de cor.
    CorMenu(Option<painel::AlvoCor>),
    MaisCores(painel::AlvoCor),
    /// Cor escolhida na paleta (`None` = automática).
    Cor(painel::AlvoCor, Option<String>),
    Formatar(painel::Mudanca),
}
/// Alvo do seletor HSV compartilhado; independente das cores das células.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCorGrafico {
    Principal,
    Fundo,
    Serie(usize),
    /// Qualquer cor do estilo novo (painel Formatar gráfico).
    Estilo(painel::AlvoCor),
}

#[derive(Debug, Clone, Copy)]
pub enum CampoEstilo {
    Subtitulo,
    Unidade,
    Cor,
    Fonte,
    EixoX,
    EixoY,
    Fundo,
    Largura,
    Altura,
}
impl CampoEstilo {
    /// Chave do campo para juntar letras seguidas num passo de Ctrl+Z.
    fn chave(self) -> &'static str {
        match self {
            Self::Subtitulo => "subtitulo",
            Self::Unidade => "unidade",
            Self::Cor => "cor",
            Self::Fonte => "fonte",
            Self::EixoX => "eixo_x",
            Self::EixoY => "eixo_y",
            Self::Fundo => "fundo",
            Self::Largura => "largura",
            Self::Altura => "altura",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum CampoVisivel {
    Titulo,
    Legenda,
    Eixos,
    Valores,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Aparencia {
    eixo_x: String,
    eixo_y: String,
    fundo: String,
    largura: String,
    altura: String,
    cores: std::collections::BTreeMap<usize, String>,
    nomes: std::collections::BTreeMap<usize, String>,
    /// Nome na legenda escolhido na aba Configurar, pela ordem da coluna
    /// (ou linha) de origem: não muda quando outra série é ocultada.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    nomes_origem: std::collections::BTreeMap<u32, String>,
    subtitulo: String,
    unidade: String,
    cor: String,
    fonte: String,
    titulo: bool,
    legenda: bool,
    eixos: bool,
    valores: bool,
    /// Formato dos rótulos de valor (`moeda:2`, `decimal:0`, `percentual:1`);
    /// vazio = o padrão do gráfico. Vem dos gráficos do `.xlsx`.
    formato: String,
    /// Estilo completo do painel Formatar gráfico; `None` = gráfico antigo,
    /// desenhado como antes até a primeira edição no painel novo.
    #[serde(skip_serializing_if = "Option::is_none")]
    estilo: Option<feather_charts::EstiloGrafico>,
}
impl Default for Aparencia {
    fn default() -> Self {
        Self {
            eixo_x: String::new(),
            eixo_y: String::new(),
            fundo: "#ffffff".into(),
            largura: "480".into(),
            altura: "300".into(),
            cores: Default::default(),
            nomes: Default::default(),
            nomes_origem: Default::default(),
            subtitulo: String::new(),
            unidade: String::new(),
            cor: "#4285f4".into(),
            fonte: "12".into(),
            titulo: true,
            legenda: true,
            eixos: true,
            valores: false,
            formato: String::new(),
            estilo: None,
        }
    }
}
#[derive(Default)]
pub(super) struct Estado {
    /// Id da tabela dinâmica de um gráfico dinâmico que está nascendo; vai
    /// para o JSON como `pivotSource` e o gráfico segue a dinâmica.
    pub(super) dinamica: Option<String>,
    pub(super) aberto: bool,
    pub(super) cursor: iced::Point,
    /// X do botão Gráfico no instante da abertura; não acompanha o cursor depois.
    pub(super) ancora_seletor_x: f32,
    arraste: Option<(String, u32, iced::Point, f64, f64)>,
    /// Menu do botão direito aberto: (gráfico, x, y na janela).
    pub(super) contexto: Option<(String, f32, f32)>,
    /// Último clique num gráfico (o Iced 0.13 não tem duplo clique no `mouse_area`).
    ultimo_clique: Option<(String, std::time::Instant)>,
    redimensionamento: Option<(String, u32, iced::Point, Quadro, Alca)>,
    /// Os gráficos da aba antes de arrastar/redimensionar: ao soltar, o gesto
    /// inteiro vira um passo de Ctrl+Z.
    antes_do_gesto: Option<(u32, Option<String>)>,
    selecionado: Option<String>,
    /// O SVG de cada gráfico da camada, pela aba dos dados, a versão dela,
    /// o tema e o JSON do gráfico: rolar a planilha não redesenha nada.
    pub(super) svgs: std::cell::RefCell<std::collections::HashMap<String, String>>,
    id: Option<String>,
    aparencia: Aparencia,
    /// Um único seletor de cor aberto por vez, dentro do painel lateral.
    pub(super) cor_ativa: Option<AlvoCorGrafico>,
    cor_hsva: [f32; 4],
    cor_hex: String,
    intervalo: String,
    titulo: String,
    tipo: TipoUi,
    pub(super) seletor_aberto: bool,
    cabecalho: bool,
    categorias: bool,
    colunas: bool,
    agrupar: bool,
    /// Mais de 200 mil células: só resumido (o "Não resumir" some).
    pub(super) resumo_obrigatorio: bool,
    /// Coluna (ou linha) das categorias, relativa ao começo do intervalo.
    coluna_categoria: u32,
    /// Séries ocultas, pela ordem de origem (sem a coluna de categoria).
    ocultas: Vec<u32>,
    /// Conta de cada série quando as categorias iguais são juntadas.
    contas: std::collections::BTreeMap<u32, AggregateFn>,
    /// Séries de texto (pela ordem de origem), com a chave do que foi olhado
    /// (aba, versão, intervalo, orientação, categoria): só se recalcula
    /// quando isso muda, nunca a cada desenho do painel.
    tipos: Option<(String, std::collections::BTreeSet<u32>)>,
    /// Os números já extraídos de cada gráfico (ver [`CacheDados`]).
    pub(super) dados: CacheDados,
    /// Aba de onde vêm os dados (`sourceSheet` no gráfico do `.xlsx`).
    aba_id: Option<u32>,
    /// Aba onde o gráfico está desenhado e gravado (pode não ser a dos dados).
    aba_grafico: Option<u32>,
    /// Último campo de texto editado: letras seguidas no mesmo campo viram
    /// um passo só de Ctrl+Z.
    ultimo_campo: Option<&'static str>,
    serie: Option<String>,
    series: Vec<String>,
    /// Gráfico copiado (Ctrl+C ou menu), com `sourceSheet` preenchido para
    /// a cópia colada em outra aba continuar lendo os mesmos dados.
    copiado: Option<serde_json::Value>,
    /// Interface do painel Formatar gráfico (grupo aberto, seleções).
    pub(super) painel: painel::EstadoPainel,
    /// Dica do mouse e clique na legenda do gráfico parado.
    interacao: interacao::Estado,
    svg: Option<String>,
    mensagem: String,
}
impl Estado {
    /// A série `n` (ordem de origem) tem texto: só pode ser contada.
    pub(super) fn serie_texto(&self, n: u32) -> bool {
        self.tipos.as_ref().is_some_and(|(_, texto)| texto.contains(&n))
    }

    /// A conta que a série `n` usa ao resumir: a escolhida, se serve ao tipo
    /// dela; senão Contar (série de texto) ou Soma (série de números).
    pub(super) fn conta_da_serie(&self, n: u32) -> AggregateFn {
        let texto = self.serie_texto(n);
        self.contas.get(&n).copied()
            .filter(|c| !texto || c.serve_para_texto())
            .unwrap_or(if texto { AggregateFn::Count } else { AggregateFn::Sum })
    }
}

/// Os números extraídos de cada gráfico (`ChartData`), pela aba dos dados,
/// a versão e a geração dela e só a parte da configuração que muda os
/// números. Agrupar lê TODAS as linhas (até 1 milhão), e uma troca no painel
/// (a categoria, por exemplo) lia a tabela três vezes: a prévia, o desenho do
/// gráfico gravado e a camada da planilha. Com isto, lê uma. Mudou a aba,
/// a chave muda sozinha; o teto evita juntar memória sem fim.
#[derive(Default)]
pub(super) struct CacheDados(
    std::cell::RefCell<std::collections::HashMap<ChaveDados, std::sync::Arc<feathersheet_core::grafico::chart::ChartData>>>,
);

/// (id da aba, versão, geração da aba, configuração que muda os números).
type ChaveDados = (u32, u32, u64, String);

impl CacheDados {
    /// A aba `aba` passou da geração `antes` para `depois` só porque o JSON
    /// dos gráficos dela foi regravado (nenhuma célula mudou): o que foi
    /// lido continua valendo. Sem isto, gravar o gráfico (cada troca no
    /// painel, cada passo do arraste) fazia reler a tabela inteira.
    pub(super) fn renovar(&self, aba: u32, antes: u64, depois: u64) {
        if antes == depois {
            return;
        }
        let mut mapa = self.0.borrow_mut();
        let velhas: Vec<ChaveDados> = mapa.keys().filter(|k| k.0 == aba && k.2 == antes).cloned().collect();
        for k in velhas {
            if let Some(dados) = mapa.remove(&k) {
                mapa.insert((k.0, k.1, depois, k.3), dados);
            }
        }
    }
}

/// Quantos conjuntos de dados guardar; passando, recomeça do zero.
const MAX_DADOS_GUARDADOS: usize = 32;

/// A aba de onde vêm os dados de um gráfico, para guardar o que foi lido.
#[derive(Clone, Copy)]
pub(super) struct Fonte<'a> {
    pub cache: &'a CacheDados,
    /// Id da aba (não o índice, que muda ao mover/excluir abas).
    pub aba: u32,
    pub versao: u32,
}

/// Só o que muda os números do gráfico, em texto: a prévia e o gráfico
/// gravado chegam aqui por caminhos diferentes (id, `sourceSheet`, cores,
/// conta padrão escrita ou não) e precisam dar a mesma chave.
fn chave_dos_dados(config: &ChartConfig, lacuna_zero: bool) -> String {
    use feathersheet_core::grafico::chart::DateHierarchy;
    let mut c = config.clone();
    c.id.clear();
    c.source_sheet = None;
    c.aggregate_fn = Some(c.aggregate_fn.unwrap_or_default());
    c.category_column_index = Some(c.category_column_index.unwrap_or(0));
    let estilo = std::mem::take(&mut c.style);
    c.style.hidden_series = estilo.hidden_series;
    c.style.date_hierarchy = estilo.date_hierarchy.filter(|h| *h != DateHierarchy::None);
    format!("{}|{}", u8::from(lacuna_zero), serde_json::to_string(&c).unwrap_or_default())
}

/// Os números do gráfico: lidos da aba uma vez e reaproveitados enquanto ela
/// não muda (`fonte`; sem ela, lê sempre). `lacuna_zero`: célula vazia na
/// série vale 0 — o gráfico do `.xlsx`, ver `editor::desenhar_grafico_salvo`.
pub(super) fn dados_do_grafico(
    sheet: &feathersheet_core::Sheet,
    config: &ChartConfig,
    lacuna_zero: bool,
    fonte: Option<Fonte<'_>>,
) -> std::sync::Arc<feathersheet_core::grafico::chart::ChartData> {
    use feathersheet_core::celulas::values::Value as V;
    use feathersheet_core::grafico::chart::{extract_chart_data, extract_chart_data_with};
    let extrair = || {
        if lacuna_zero {
            let (r0, c0) = (config.range.r0, config.range.c0);
            extract_chart_data_with(&|r, c| match sheet.get_value(r, c) {
                V::Empty if r > r0 && c > c0 => V::Number(0.0),
                v => v,
            }, config)
        } else {
            extract_chart_data(sheet, config)
        }
    };
    let Some(f) = fonte else { return std::sync::Arc::new(extrair()) };
    let base = chave_dos_dados(config, lacuna_zero);
    let chave = |geracao: u64| (f.aba, f.versao, geracao, base.clone());
    let guardado = f.cache.0.borrow().get(&chave(sheet.geracao())).cloned();
    if let Some(dados) = guardado {
        return dados;
    }
    let dados = std::sync::Arc::new(extrair());
    // A geração DEPOIS de ler: se a leitura mexeu na aba, a chave já é a nova.
    let mut mapa = f.cache.0.borrow_mut();
    if mapa.len() >= MAX_DADOS_GUARDADOS {
        mapa.clear();
    }
    mapa.insert(chave(sheet.geracao()), dados.clone());
    dados
}

/// Uma única política visual para a prévia e o gráfico embutido.
/// Toda a extração/agregação permanece no Sheet; Charts recebe apenas dados,
/// cor e rótulo, sem recontar valores ou alterar os arquivos .fshz.
fn aplicar_aparencia_grafico(g: &mut Grafico, a: &Aparencia) -> Result<(), String> {
    let primaria = cor_rgb(&a.cor)
        .ok_or(tr!("Use uma cor hexadecimal, como #4285f4."))?;
    // Para pizza, o editor já reduziu para uma série; os índices de
    // aparência referem-se à série VISÍVEL, tal como na prévia legada.
    for (i, serie) in g.series.iter_mut().enumerate() {
        if i == 0 {
            serie.cor = primaria;
        }
        if let Some(cor) = a.cores.get(&i) {
            serie.cor = cor_rgb(cor)
                .ok_or(tr!("Use cores hexadecimais, como #4285f4."))?;
        }
        if let Some(nome) = a.nomes.get(&i) {
            serie.nome = nome.clone();
        }
    }
    Ok(())
}

/// Nomes da legenda escolhidos na aba Configurar (pela série de origem).
fn aplicar_nomes_origem(c: &mut feather_charts::ContratoGrafico, a: &Aparencia) {
    for (serie, meta) in c.grafico.series.iter_mut().zip(&c.metadados_series) {
        if let Some(nome) = a.nomes_origem.get(&meta.indice_origem).filter(|n| !n.trim().is_empty()) {
            serie.nome = nome.clone();
        }
    }
}

/// O mesmo dos nomes de origem para os tipos avançados (empilhados,
/// cascata…), que desenham direto do `ChartData`.
fn renomear_series_dados(d: &mut feathersheet_core::grafico::chart::ChartData, ocultas: &[u32], a: &Aparencia) {
    let origens = (0u32..).filter(|o| !ocultas.contains(o));
    for (serie, o) in d.series.iter_mut().zip(origens) {
        if let Some(nome) = a.nomes_origem.get(&o).filter(|n| !n.trim().is_empty()) {
            serie.name = nome.clone();
        }
    }
}

/// Colunas/barras empilhadas: o tipo base e como empilhar. Desenhadas pelo
/// desenho com estilo (fundo, título, eixos), não pelo motor dos avançados.
fn empilhamento(t: ChartType) -> Option<(ChartType, feather_charts::estilo::Empilhar)> {
    use feather_charts::estilo::Empilhar as E;
    Some(match t {
        ChartType::ColumnStacked => (ChartType::Column, E::Empilhado),
        ChartType::ColumnStacked100 => (ChartType::Column, E::Cem),
        ChartType::BarStacked => (ChartType::Bar, E::Empilhado),
        ChartType::BarStacked100 => (ChartType::Bar, E::Cem),
        _ => return None,
    })
}

/// Como os tipos fora dos sete básicos chegam ao desenhista novo: os de
/// linha/área em degraus e empilhados viram o tipo básico com empilhamento
/// e degraus; os demais têm desenho próprio (`TipoAvancado`).
enum Rota {
    Basico(Tipo, feather_charts::estilo::Empilhar, bool),
    Avancado(feather_charts::formatado::TipoAvancado),
}
fn rota_especial(t: ChartType) -> Option<Rota> {
    use feather_charts::estilo::Empilhar as E;
    use feather_charts::formatado::TipoAvancado as A;
    Some(match t {
        ChartType::StepLineStacked => Rota::Basico(Tipo::Degraus, E::Empilhado, true),
        ChartType::StepLineStacked100 => Rota::Basico(Tipo::Degraus, E::Cem, true),
        ChartType::StepArea => Rota::Basico(Tipo::Area, E::Nenhum, true),
        ChartType::StepAreaStacked => Rota::Basico(Tipo::Area, E::Empilhado, true),
        ChartType::StepAreaStacked100 => Rota::Basico(Tipo::Area, E::Cem, true),
        ChartType::Combo => Rota::Avancado(A::Combinado),
        ChartType::Histogram => Rota::Avancado(A::Histograma),
        ChartType::Waterfall => Rota::Avancado(A::Cascata),
        ChartType::Radar => Rota::Avancado(A::Radar),
        ChartType::Bubble => Rota::Avancado(A::Bolhas),
        ChartType::Candlestick => Rota::Avancado(A::Velas),
        ChartType::Gantt => Rota::Avancado(A::Gantt),
        ChartType::Heatmap => Rota::Avancado(A::MapaCalor),
        ChartType::Line3d => Rota::Avancado(A::Linha3d),
        _ => return None,
    })
}

/// Desenho de um tipo especial (ver [`Rota`]) com a moldura e o estilo do
/// desenhista novo. Devolve também o mapa (dica, legenda), os dados e o estilo.
pub(super) fn desenhar_especial(
    config: &ChartConfig,
    dados: &feathersheet_core::grafico::chart::ChartData,
    aparencia: &Aparencia,
    excel: Option<&serde_json::Map<String, serde_json::Value>>,
    largura: f32,
    altura: f32,
) -> Result<(String, feather_charts::formatado::MapaGrafico, Grafico, feather_charts::estilo::EstiloGrafico), String> {
    let rota = rota_especial(config.tipo).ok_or(tr!("Tipo de gráfico não reconhecido."))?;
    let series = dados.series.iter().enumerate().map(|(i, s)| feather_charts::Serie {
        nome: s.name.clone(),
        valores: s.values.iter().map(|v| if v.is_finite() { *v } else { 0.0 }).collect(),
        cor: cor_rgb(&feathersheet_core::grafico::chart::get_series_color(&config.style, i as u32))
            .unwrap_or(feather_draw::Cor::rgba(66, 133, 244, 255)),
    }).collect();
    let mut g = Grafico { titulo: dados.title.clone(), tipo: Tipo::Colunas, categorias: dados.categories.clone(), series };
    if !aparencia.nomes_origem.is_empty() || !aparencia.cores.is_empty() || !aparencia.nomes.is_empty() {
        aplicar_aparencia_grafico(&mut g, aparencia)?;
    }
    let mut e = aparencia.estilo.clone().unwrap_or_else(|| painel::migrar(aparencia, excel));
    let falha = |_| tr!("Não foi possível desenhar o gráfico com esses dados. Confira se a tabela tem as colunas que este tipo pede.").to_string();
    let (svg, mapa) = match rota {
        Rota::Basico(tipo, emp, degraus) => {
            g.tipo = tipo;
            e.empilhar = emp;
            e.degraus = degraus;
            g.exportar_svg_formatado_com_mapa(largura, altura, &e).map_err(falha)?
        }
        Rota::Avancado(tipo) => {
            let pedido = feather_charts::formatado::PedidoAvancado {
                tipo, x: dados.x_values.clone(), z: dados.z_values.clone(),
            };
            g.exportar_svg_avancado_com_mapa(largura, altura, &e, &pedido).map_err(falha)?
        }
    };
    Ok((svg, mapa, g, e))
}

/// A aparência com o empilhamento do tipo (cria o estilo se ainda não há).
fn aparencia_empilhada(a: &Aparencia, emp: feather_charts::estilo::Empilhar) -> Aparencia {
    let mut a = a.clone();
    a.estilo.get_or_insert_with(Default::default).empilhar = emp;
    a
}

/// A prévia do editor no tamanho em que o gráfico fica na planilha: assim
/// o gráfico inserido sai idêntico ao que se viu no painel.
fn preparar_svg(g: &Grafico, a: &Aparencia) -> Result<String, String> {
    let medida = |s: &str, padrao: f32| s.trim().replace(',', ".").parse::<f32>().ok()
        .filter(|v| v.is_finite()).unwrap_or(padrao).clamp(120., 3200.);
    preparar_svg_em(g, a, medida(&a.largura, 480.), medida(&a.altura, 300.))
}

/// `moeda:2` → `FormatoNumero::Moeda(2)`; vazio ou inválido = o padrão.
fn formato_numero(f: &str) -> Option<feather_charts::FormatoNumero> {
    use feather_charts::FormatoNumero as F;
    let (tipo, casas) = f.split_once(':')?;
    let casas = casas.parse::<u8>().ok()?.min(6);
    Some(match tipo { "moeda" => F::Moeda(casas), "percentual" => F::Percentual(casas), "decimal" => F::Decimal(casas), _ => return None })
}

/// O SVG do gráfico em `largura` x `altura` (unidades da cena), sempre pelo
/// desenhista novo: com o estilo do painel ou o convertido da aparência antiga.
fn preparar_svg_em(g: &Grafico, a: &Aparencia, largura: f32, altura: f32) -> Result<String, String> {
    if a.estilo.is_none() {
        a.fonte.parse::<f32>().ok().filter(|v| (4.0..=32.0).contains(v))
            .ok_or(tr!("O tamanho da fonte deve estar entre 4 e 32."))?;
        cor_rgb(&a.fundo).ok_or(tr!("Use uma cor de fundo hexadecimal, como #ffffff."))?;
    }
    let e = a.estilo.clone().unwrap_or_else(|| painel::migrar(a, None));
    g.exportar_svg_formatado(largura, altura, &e)
        .map_err(|_| tr!("Não foi possível desenhar o gráfico com essas opções.").to_string())
}
/// Aceita a mesma codificação #RRGGBB/#RRGGBBAA do seletor compartilhado.
/// Alfa segue até a Cena vetorial, sem converter RGB em outra paleta.
fn cor_rgb(s: &str) -> Option<feather_draw::Cor> {
    if !seletor_cor::hex_valido(s) || !s.trim().starts_with('#') {
        return None;
    }
    let [r, g, b, a] = seletor_cor::rgba_de(s);
    Some(feather_draw::Cor::rgba(r, g, b, a))
}
impl App {
    /// Aplica um clique/arrasto do colorpicker sem afetar o colorpicker das
    /// células. A prévia é recalculada pelo mesmo motor Charts de sempre.
    /// Sincroniza HSV quando o usuário digita hexadecimal com a paleta aberta.
    fn sincronizar_campo_hex_grafico(&mut self, alvo: AlvoCorGrafico, hex: &str) {
        if self.graficos.cor_ativa != Some(alvo) { return; }
        self.graficos.cor_hex = hex.to_owned();
        if seletor_cor::hex_valido(hex) {
            self.graficos.cor_hsva = seletor_cor::rgba_para_hsva(
                seletor_cor::rgba_de(hex), self.graficos.cor_hsva[0],
            );
        }
    }

    fn aplicar_cor_seletor_grafico(&mut self, nova: String) {
        let Some(alvo) = self.graficos.cor_ativa else { return; };
        // O picker compartilhado possui a opção Sem cor; em gráficos,
        // Fundo/Principal usam seu padrão e Série volta à cor herdada.
        if let AlvoCorGrafico::Estilo(a) = alvo {
            let cor = feather_charts::estilo::CorEstilo::de_texto(&nova);
            if cor.is_none() && !nova.trim().is_empty() { return; }
            let atual = cor.map_or_else(String::new, |c| c.texto());
            self.mudar_estilo(|e| { painel::definir_cor(e, a, cor); true }, Some("seletor_cor"));
            if !atual.is_empty() {
                self.graficos.cor_hex = atual.clone();
                self.graficos.cor_hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&atual), self.graficos.cor_hsva[0]);
            }
            return;
        }
        if nova.trim().is_empty() {
            match alvo {
                AlvoCorGrafico::Principal => self.graficos.aparencia.cor = "#4285f4".into(),
                AlvoCorGrafico::Fundo => self.graficos.aparencia.fundo = "#ffffff".into(),
                AlvoCorGrafico::Serie(i) => { self.graficos.aparencia.cores.remove(&i); }
                AlvoCorGrafico::Estilo(_) => {}
            }
        } else {
            if !seletor_cor::hex_valido(&nova) { return; }
            let hex = format!("#{}", nova.trim().trim_start_matches('#').to_ascii_lowercase());
            match alvo {
                AlvoCorGrafico::Principal => self.graficos.aparencia.cor = hex,
                AlvoCorGrafico::Fundo => self.graficos.aparencia.fundo = hex,
                AlvoCorGrafico::Serie(i) => { self.graficos.aparencia.cores.insert(i, hex); }
                AlvoCorGrafico::Estilo(_) => {}
            }
        }
        let atual = match alvo {
            AlvoCorGrafico::Principal => self.graficos.aparencia.cor.clone(),
            AlvoCorGrafico::Fundo => self.graficos.aparencia.fundo.clone(),
            AlvoCorGrafico::Serie(i) => self.graficos.aparencia.cores.get(&i)
                .cloned().unwrap_or_else(|| self.graficos.aparencia.cor.clone()),
            AlvoCorGrafico::Estilo(_) => return,
        };
        self.graficos.cor_hex = atual.clone();
        self.graficos.cor_hsva = seletor_cor::rgba_para_hsva(
            seletor_cor::rgba_de(&atual), self.graficos.cor_hsva[0],
        );
        self.editar_ao_vivo(Some("seletor_cor"));
    }

    fn evento_seletor_cor_grafico(&mut self, evento: EventoCor) {
        if self.graficos.cor_ativa.is_none() { return; }
        match evento {
            EventoCor::Fechar => self.graficos.cor_ativa = None,
            EventoCor::Escolhida(hex) => self.aplicar_cor_seletor_grafico(hex),
            EventoCor::SaturacaoBrilho(s, v) => {
                self.graficos.cor_hsva[1] = s.clamp(0., 1.);
                self.graficos.cor_hsva[2] = v.clamp(0., 1.);
                self.aplicar_cor_seletor_grafico(self.hex_do_seletor_grafico());
            }
            EventoCor::Matiz(h) => {
                self.graficos.cor_hsva[0] = h.rem_euclid(360.);
                self.aplicar_cor_seletor_grafico(self.hex_do_seletor_grafico());
            }
            EventoCor::Alfa(a) => {
                self.graficos.cor_hsva[3] = a.clamp(0., 1.);
                self.aplicar_cor_seletor_grafico(self.hex_do_seletor_grafico());
            }
            EventoCor::TextoHex(hex) => {
                self.graficos.cor_hex = hex.clone();
                if seletor_cor::hex_valido(&hex) {
                    self.aplicar_cor_seletor_grafico(hex);
                }
            }
            EventoCor::Canal(indice, valor) => {
                if indice < 4 {
                    if let Ok(v) = valor.parse::<u8>() {
                        let mut rgba = seletor_cor::rgba_de(&self.hex_do_seletor_grafico());
                        rgba[indice] = v;
                        self.aplicar_cor_seletor_grafico(seletor_cor::hex_de(rgba));
                    }
                }
            }
        }
    }

    fn hex_do_seletor_grafico(&self) -> String {
        let [h, s, v, a] = self.graficos.cor_hsva;
        let [r, g, b] = seletor_cor::hsv_para_rgb(h, s, v);
        seletor_cor::hex_de([r, g, b, (a * 255.).round() as u8])
    }

    /// Uma edição do painel: grava no gráfico (Ctrl+Z) e redesenha.
    fn editar_ao_vivo(&mut self, campo: Option<&'static str>) {
        // A conta das séries de texto entra no mesmo passo de Ctrl+Z.
        self.normalizar_contas();
        self.salvar_grafico_ao_vivo(campo);
        self.atualizar_grafico();
    }

    /// A geração da aba `indice` agora (para [`CacheDados::renovar`]).
    pub(super) fn geracao_da_aba(&self, indice: usize) -> Option<(u32, u64)> {
        self.workbook.aba(indice).map(|a| (a.id, a.sheet.geracao()))
    }

    /// Depois de regravar só o JSON dos gráficos da aba `indice` (que estava
    /// na geração `antes`): os números guardados dela continuam valendo.
    pub(super) fn renovar_dados_da_aba(&self, indice: usize, antes: Option<(u32, u64)>) {
        if let (Some((id, antes)), Some((id_agora, depois))) = (antes, self.geracao_da_aba(indice)) {
            if id == id_agora {
                self.graficos.dados.renovar(id, antes, depois);
            }
        }
    }

    /// A aba de índice `indice` como fonte de dados guardáveis.
    pub(super) fn fonte_dados(&self, indice: usize) -> Option<Fonte<'_>> {
        self.workbook.aba(indice).map(|a| Fonte {
            cache: &self.graficos.dados,
            aba: a.id,
            versao: self.workbook.versao(indice),
        })
    }

    /// Recalcula quais séries são de texto, só se o intervalo, a orientação,
    /// a categoria ou os dados mudaram desde a última vez.
    fn atualizar_tipos_series(&mut self) {
        let g = &self.graficos;
        let versao = (0..self.workbook.len())
            .find(|&i| self.workbook.aba(i).is_some_and(|a| Some(a.id) == g.aba_id))
            .map(|i| self.workbook.versao(i));
        let chave = format!("{:?}|{versao:?}|{}|{}|{}|{}|{}", g.aba_id, g.intervalo.trim(), g.colunas, g.cabecalho, g.categorias, g.coluna_categoria);
        if g.tipos.as_ref().is_some_and(|(c, _)| *c == chave) {
            return;
        }
        let texto = self.series_de_texto();
        self.graficos.tipos = Some((chave, texto));
    }

    /// Série de texto não soma: com o resumo ligado, a conta guardada que
    /// não serve (Soma, Média…) ou a que falta vira Contar. Devolve se mudou.
    fn normalizar_contas(&mut self) -> bool {
        self.atualizar_tipos_series();
        let g = &mut self.graficos;
        if !g.agrupar {
            return false;
        }
        let Some((_, texto)) = &g.tipos else { return false };
        let mut mudou = false;
        for &n in texto {
            if !g.contas.get(&n).is_some_and(|c| c.serve_para_texto()) {
                g.contas.insert(n, AggregateFn::Count);
                mudou = true;
            }
        }
        mudou
    }

    /// Acima de 200 mil células o gráfico só existe resumido: liga o resumo
    /// sozinho (soma em cada série, como no menu) e grava, se já está na
    /// planilha. Devolve se ligou agora.
    fn exigir_resumo_se_grande(&mut self) -> bool {
        let Some(r) = parse_range_ref(self.graficos.intervalo.trim()).map(|r| r.normalized()) else {
            self.graficos.resumo_obrigatorio = false;
            return false;
        };
        let Some(aba) = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i))
            .find(|a| Some(a.id) == self.graficos.aba_id) else { return false };
        let mut faixa = ChartRange { r0: r.r0, c0: r.c0, r1: r.r1, c1: r.c1 };
        self.graficos.resumo_obrigatorio = editor::cortar_intervalo(&aba.sheet, &mut faixa, false).is_none();
        if !self.graficos.resumo_obrigatorio || self.graficos.agrupar {
            return false;
        }
        self.graficos.agrupar = true;
        if self.graficos.id.is_some() {
            self.salvar_grafico_ao_vivo(None);
        }
        true
    }

    /// Recomendados: o tipo que os dados pedem. Categorias numéricas →
    /// dispersão; datas → linhas; uma série curta e sem negativos → pizza;
    /// o resto → colunas.
    // shortcut: olha só as 200 primeiras linhas; basta para escolher o tipo.
    fn tipo_recomendado(&self) -> TipoUi {
        let Some(r) = parse_range_ref(self.graficos.intervalo.trim()).map(|r| r.normalized()) else { return TipoUi::Colunas };
        let Some(aba) = self.workbook.aba(self.folha) else { return TipoUi::Colunas };
        let numero = |t: &str| t.trim().replace(',', ".").parse::<f64>().ok();
        let r0 = r.r0 + u32::from(self.graficos.cabecalho);
        let linhas: Vec<u32> = (r0..=r.r1.min(r0 + 199)).collect();
        let categorias: Vec<String> = linhas.iter().map(|&l| aba.sheet.get_raw(l, r.c0)).filter(|t| !t.trim().is_empty()).collect();
        if categorias.is_empty() || r.c1 <= r.c0 {
            return TipoUi::Colunas;
        }
        if categorias.iter().all(|t| numero(t).is_some()) {
            return TipoUi::Dispersao;
        }
        let parece_data = |t: &str| t.chars().filter(|c| *c == '/' || *c == '-').count() == 2
            && t.chars().any(|c| c.is_ascii_digit());
        if categorias.iter().all(|t| parece_data(t)) {
            return TipoUi::Linhas;
        }
        let sem_negativos = linhas.iter().all(|&l| numero(&aba.sheet.get_raw(l, r.c0 + 1)).is_none_or(|v| v >= 0.0));
        if r.c1 == r.c0 + 1 && categorias.len() <= 6 && sem_negativos {
            return TipoUi::Pizza;
        }
        TipoUi::Colunas
    }

    fn atualizar_grafico(&mut self) {
        let ligou_resumo = self.exigir_resumo_se_grande();
        if self.normalizar_contas() && self.graficos.id.is_some() {
            self.salvar_grafico_ao_vivo(None);
        }
        self.graficos.svg = None;
        // Gráfico já gravado: a prévia é o desenho dele na planilha (o mesmo
        // caminho da camada, com o fundo e as cores do Excel quando houver).
        // Desenhado ANTES, para a prévia só refazer listas e avisos (os
        // números saem do mesmo cache) e não desenhar um SVG jogado fora.
        let salvo = self.svg_do_grafico_em_edicao();
        let desenhar_previa = salvo.is_none();
        let resultado = (|| {
            let r = parse_range_ref(self.graficos.intervalo.trim())
                .ok_or(tr!("Intervalo inválido. Use A1:C10."))?
                .normalized();
            if r.sheet.is_some() {
                return Err(tr!("Informe apenas o intervalo da aba de origem.").into());
            }
            let indice = (0..self.workbook.len())
                .find(|&i| self.workbook.aba(i).is_some_and(|a| Some(a.id) == self.graficos.aba_id))
                .ok_or(tr!("A aba de origem foi removida. Abra um novo gráfico."))?;
            let aba = self.workbook.aba(indice)
                .ok_or(tr!("A aba de origem foi removida. Abra um novo gráfico."))?;
            let fonte = Fonte { cache: &self.graficos.dados, aba: aba.id, versao: self.workbook.versao(indice) };
            // Mais de 200 mil células só resumindo (soma, média, contagem,
            // desvio padrão…): aí o gráfico lê todas as linhas.
            let mut cortado = ChartRange { r0: r.r0, c0: r.c0, r1: r.r1, c1: r.c1 };
            if editor::cortar_intervalo(&aba.sheet, &mut cortado, self.graficos.agrupar).is_none() {
                return Err(if self.graficos.agrupar {
                    tr!("Mesmo resumindo, o gráfico lê até 100 milhões de células. Selecione menos colunas.")
                } else {
                    tr!("Mais de 200.000 células: ligue \"Resumir categorias iguais\" (em Dados) para usar todas as linhas com soma, média, contagem ou desvio padrão, ou selecione menos células.")
                }.into());
            }
            let pilha = empilhamento(self.graficos.tipo.motor());
            let config = ChartConfig {
                tipo: pilha.map_or(self.graficos.tipo.motor(), |p| p.0),
                title: self.graficos.titulo.clone(),
                range: cortado,
                series_in_columns: self.graficos.colunas,
                has_headers: self.graficos.cabecalho,
                has_category_labels: self.graficos.categorias,
                category_column_index: Some(self.graficos.coluna_categoria),
                aggregate: self.graficos.agrupar,
                aggregate_fn: Some(AggregateFn::Sum),
                aggregate_by_series: self.graficos.contas.clone(),
                style: feathersheet_core::grafico::chart::ChartStyle {
                    hidden_series: self.graficos.ocultas.clone(),
                    ..Default::default()
                },
                ..Default::default()
            };
            // Novo motor Rust compartilhado: tipos especializados do chart.ts.
            // Mantém os sete tipos já consolidados no Charts legado, bem como
            // todo o processamento de células, agrupamento e FSHZ no Sheet.
            if self.graficos.tipo.avancado() {
                let mut dados = (*dados_do_grafico(&aba.sheet, &config, false, Some(fonte))).clone();
                renomear_series_dados(&mut dados, &config.style.hidden_series, &self.graficos.aparencia);
                self.graficos.series = dados.series.iter().enumerate()
                    .map(|(i,s)| format!("{} — {}",i+1,s.name)).collect();
                self.graficos.painel.categorias = dados.categories.clone();
                let medida = |s: &str, padrao: f32| s.trim().replace(',', ".").parse::<f32>().ok()
                    .filter(|v| v.is_finite()).unwrap_or(padrao).clamp(120., 3200.);
                let (w, h) = (medida(&self.graficos.aparencia.largura, 480.), medida(&self.graficos.aparencia.altura, 300.));
                let (svg, _, g, _) = desenhar_especial(&config, &dados, &self.graficos.aparencia, None, w, h)?;
                self.graficos.painel.cores_base = g.series.iter().map(|s| s.cor).collect();
                self.graficos.mensagem = dados.aviso.unwrap_or_else(|| trf!(
                    "{} categorias, {} séries. Tipo {} em Rust.",
                    dados.categories.len(),dados.series.len(),self.graficos.tipo
                ));
                return Ok(svg);
            }
            // Gráfico do `.xlsx`: lacuna vale 0, como no desenho do gravado.
            let lacuna_zero = self.graficos.painel.excel.is_some();
            let dados = dados_do_grafico(&aba.sheet, &config, lacuna_zero, Some(fonte));
            let mut contrato =
                feathersheet_io::grafico_compartilhado::preparar_grafico(&config, (*dados).clone())
                    .map_err(|_| tr!("Selecione uma tabela com categorias e valores numéricos. Se a tabela não tem cabeçalhos, desative essa opção.").to_string())?;
            aplicar_nomes_origem(&mut contrato, &self.graficos.aparencia);
            self.graficos.series = contrato
                .grafico
                .series
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{} — {}", i + 1, s.nome))
                .collect();
            if self.graficos.tipo == TipoUi::Pizza {
                let i = self
                    .graficos
                    .serie
                    .as_ref()
                    .and_then(|s| self.graficos.series.iter().position(|v| v == s))
                    .unwrap_or(0);
                self.graficos.serie = self.graficos.series.get(i).cloned();
                contrato.grafico.series = vec![contrato.grafico.series[i].clone()];
                contrato.grafico.tipo = Tipo::Pizza;
            }
            aplicar_aparencia_grafico(
                &mut contrato.grafico, &self.graficos.aparencia
            )?;
            self.graficos.painel.cores_base = contrato.grafico.series.iter().map(|s| s.cor).collect();
            self.graficos.painel.categorias = contrato.grafico.categorias.clone();
            let mut avisos = contrato.avisos;
            if let Some(a) = contrato.amostra {
                avisos.push(trf!(
                    "Visualização amostrada: {} de {} pontos.",
                    a.usados, a.disponiveis
                ));
            }
            self.graficos.mensagem = if avisos.is_empty() {
                trf!(
                    "{} categorias, {} séries.",
                    contrato.grafico.categorias.len(),
                    contrato.grafico.series.len()
                )
            } else {
                avisos.join("; ")
            };
            if !desenhar_previa {
                return Ok(String::new());
            }
            match pilha {
                Some((_, emp)) => preparar_svg(&contrato.grafico, &aparencia_empilhada(&self.graficos.aparencia, emp)),
                None => preparar_svg(&contrato.grafico, &self.graficos.aparencia),
            }
        })();
        match resultado {
            Ok(svg) => self.graficos.svg = Some(svg),
            Err(e) => self.graficos.mensagem = e,
        }
        if ligou_resumo {
            self.graficos.mensagem = trf!("Mais de 200.000 células: o gráfico resume as categorias iguais. Escolha a conta de cada série (soma, média, contagem, desvio padrão…). {}",
                self.graficos.mensagem);
        }
        if let Some(svg) = salvo {
            if self.graficos.svg.is_none() {
                self.graficos.mensagem.clear();
            }
            self.graficos.svg = Some(svg);
        }
    }

    fn svg_do_grafico_em_edicao(&self) -> Option<String> {
        let id = self.graficos.id.as_deref()?;
        let indice_grafico = self.indice_aba_grafico()?;
        let aba = self.workbook.aba(indice_grafico)?;
        let v = feathersheet_io::graficos_salvos::listar(&aba.sheet).ok()?
            .into_iter().find(|v| v["id"].as_str() == Some(id))?;
        let indice = v["sourceSheet"].as_str()
            .and_then(|n| self.workbook.indice_por_nome(n))
            .filter(|&i| self.workbook.aba(i).is_some())
            .unwrap_or(indice_grafico);
        let dados = self.workbook.aba(indice).map_or(&aba.sheet, |a| &a.sheet);
        let svg = editor::svg_do_grafico_salvo_em(dados, &v, self.fonte_dados(indice))?;
        // A camada desenha este mesmo gráfico com a mesma chave: já fica pronto.
        self.guardar_svg_camada(indice, &v, &svg);
        Some(svg)
    }
    /// Menu do botão direito do gráfico aberto: (id, x, y na janela).
    pub(in crate::app) fn graficos_contexto(&self) -> Option<(String, f32, f32)> {
        self.graficos.contexto.clone()
    }

    /// Tira a seleção do gráfico (clique numa célula ou numa forma).
    pub(in crate::app) fn desselecionar_grafico(&mut self) {
        self.graficos.selecionado = None;
        self.graficos.contexto = None;
    }

    /// O id do gráfico selecionado na grade (para o foco do teclado).
    pub(in crate::app) fn grafico_selecionado_id(&self) -> Option<&str> {
        self.graficos.selecionado.as_deref()
    }

    /// Delete com um gráfico selecionado (e o editor fechado): exclui.
    pub(in crate::app) fn apagar_grafico_selecionado(&mut self) -> bool {
        match self.graficos.selecionado.clone().filter(|_| !self.graficos.aberto) {
            Some(id) => {
                let _ = self.evento_grafico(Evento::Remover(id));
                true
            }
            None => false,
        }
    }

    /// Guarda o gráfico `id` da aba ativa para colar depois.
    fn copiar_grafico(&mut self, id: &str) -> bool {
        let Some(aba) = self.workbook.aba(self.folha) else { return false };
        let Some(mut v) = feathersheet_io::graficos_salvos::listar(&aba.sheet).ok()
            .and_then(|l| l.into_iter().find(|v| v["id"].as_str() == Some(id))) else { return false };
        if v["sourceSheet"].as_str().is_none() {
            v["sourceSheet"] = serde_json::json!(aba.sheet.name);
        }
        self.graficos.copiado = Some(v);
        self.status = tr!("Gráfico copiado. Ctrl+V cola.").into();
        true
    }

    /// Cola o gráfico copiado na aba ativa, 24 px abaixo e à direita do
    /// original (ou da última cópia), com id novo; um Ctrl+Z.
    fn colar_grafico(&mut self) -> bool {
        let Some(mut v) = self.graficos.copiado.clone() else { return false };
        let id = format!("native-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        v["id"] = serde_json::json!(id);
        for k in ["x", "y"] {
            let n = v["position"][k].as_f64().unwrap_or(80.0) + 24.0;
            v["position"][k] = serde_json::json!(n);
        }
        // A próxima colagem sai mais deslocada, como no Excel.
        self.graficos.copiado = Some(v.clone());
        // Dados na própria aba: sem sourceSheet, como os criados aqui.
        if let Some(aba) = self.workbook.aba(self.folha) {
            if v["sourceSheet"].as_str() == Some(aba.sheet.name.as_str()) {
                if let Some(o) = v.as_object_mut() { o.remove("sourceSheet"); }
            }
        }
        let Some(aba) = self.workbook.aba_mut(self.folha) else { return false };
        match feathersheet_io::graficos_salvos::mesclado(&aba.sheet, &id, v) {
            Ok(novo) => {
                aba.sheet.alterar_graficos(Some(novo), false);
                self.alterado = true;
                self.graficos.selecionado = Some(id);
                self.invalidar_cache_objetos();
                self.status = tr!("Gráfico colado (Ctrl+Z desfaz).").into();
                true
            }
            Err(e) => {
                self.status = e;
                false
            }
        }
    }

    /// Outra coisa foi copiada (forma): Ctrl+V não cola mais o gráfico.
    pub(in crate::app) fn graficos_esquecer_copia(&mut self) {
        self.graficos.copiado = None;
    }

    /// Há gráfico copiado para o "Colar" do menu.
    pub(in crate::app) fn grafico_copiado(&self) -> bool {
        self.graficos.copiado.is_some()
    }

    /// Ctrl+C/V/D com gráfico selecionado (painel fechado). Devolve se usou.
    pub(in crate::app) fn atalho_grafico(&mut self, letra: &str) -> bool {
        let sel = self.graficos.selecionado.clone().filter(|_| !self.graficos.aberto);
        match (letra, sel) {
            ("c", Some(id)) => self.copiar_grafico(&id),
            ("d", Some(id)) => self.copiar_grafico(&id) && self.colar_grafico(),
            ("v", _) if self.graficos.copiado.is_some() => self.colar_grafico(),
            // Copiar células (ou forma) depois de um gráfico: Ctrl+V volta a colar aquilo.
            ("c" | "x", None) => {
                self.graficos.copiado = None;
                false
            }
            _ => false,
        }
    }

    pub(super) fn evento_grafico(&mut self, event: Evento) -> Task<Msg> {
        match event {
            Evento::Abrir => {
                if self.graficos.seletor_aberto {
                    self.graficos.seletor_aberto = false;
                    return Task::none();
                }
                self.confirmar();
                self.menu_aberto = false;
                let mut area = self.area_selecionada();
                if area.r0 == area.r1 && area.c0 == area.c1 {
                    if let Some(aba) = self.workbook.aba_mut(self.folha) {
                        area = aba.sheet.regiao_da_tabela(area.r0, area.c0);
                    }
                }
                // Captura a posição do clique ANTES de recriar o estado.
                // A assinatura global do mouse usa coordenadas da janela.
                let cursor_abertura = self.graficos.cursor;
                let copiado = self.graficos.copiado.take();
                // O que já foi lido e desenhado continua valendo.
                let dados = std::mem::take(&mut self.graficos.dados);
                let svgs = std::mem::take(&mut self.graficos.svgs);
                self.graficos = Estado {
                    copiado,
                    dados,
                    svgs,
                    aberto: false,
                    cursor: cursor_abertura,
                    ancora_seletor_x: cursor_abertura.x,
                    seletor_aberto: true,
                    intervalo: format_ref(&area),
                    titulo: tr!("Gráfico").into(),
                    cabecalho: self.workbook.aba(self.folha).is_some_and(|a| {
                        // Se a primeira linha já contém números, não é um
                        // cabeçalho. Ex.: "a 1" / "b 2" são duas categorias.
                        if area.c1 <= area.c0 { return false; }
                        let inicial = a.sheet.get_raw(area.r0, area.c0 + 1);
                        let texto = inicial.trim();
                        !texto.is_empty()
                            && texto.parse::<f64>().is_err()
                            && texto.replace(',', ".").parse::<f64>().is_err()
                    }),
                    categorias: true,
                    colunas: true,
                    aba_id: self.workbook.aba(self.folha).map(|a| a.id),
                    ..Default::default()
                };
                self.atualizar_grafico();
                return Task::none();
            }
            Evento::Cursor(p) => {
                self.graficos.cursor = p;
                let zoom = (self.config.zoom as f64 / 100.).max(0.01);
                // Mover/redimensionar só regrava a posição no JSON: os números
                // do gráfico não precisam ser lidos de novo a cada passo.
                let geracao_antes = self.geracao_da_aba(self.folha);
                if let Some((id, aba_id, origem, x, y)) = self.graficos.arraste.clone() {
                    if let Some(aba) = self.workbook.aba_mut(self.folha).filter(|a| a.id == aba_id)
                    {
                        if let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) {
                            if let Some(v) = lista.iter().find(|v| v["id"].as_str() == Some(&id)) {
                                let mut pos = v["position"].clone();
                                pos["x"] =
                                    serde_json::json!((x + f64::from(p.x - origem.x) / zoom).max(0.));
                                pos["y"] =
                                    serde_json::json!((y + f64::from(p.y - origem.y) / zoom).max(0.));
                                if feathersheet_io::graficos_salvos::salvar(
                                    &mut aba.sheet,
                                    &id,
                                    serde_json::json!({"position":pos}),
                                )
                                .is_ok()
                                {
                                    self.alterado = true;
                                }
                            }
                        }
                    }
                }
                if let Some((id, aba_id, origem, quadro, alca)) =
                    self.graficos.redimensionamento.clone()
                {
                    if let Some(aba) = self.workbook.aba_mut(self.folha).filter(|a| a.id == aba_id) {
                        let novo = quadro.redimensionar(
                            alca,
                            f64::from(p.x - origem.x) / zoom,
                            f64::from(p.y - origem.y) / zoom,
                        );
                        // Preservar x/y, campos e metadados existentes.
                        // Ler ANTES de pedir &mut aba.sheet para o salvamento.
                        let mut pos = feathersheet_io::graficos_salvos::listar(&aba.sheet)
                            .ok()
                            .and_then(|lista| lista.into_iter()
                                .find(|v| v["id"].as_str() == Some(id.as_str()))
                                .map(|v| v["position"].clone()))
                            .unwrap_or_else(|| serde_json::json!({"x":80.,"y":80.}));
                        pos["x"] = serde_json::json!(novo.x);
                        pos["y"] = serde_json::json!(novo.y);
                        pos["width"] = serde_json::json!(novo.largura);
                        pos["height"] = serde_json::json!(novo.altura);
                        if feathersheet_io::graficos_salvos::salvar(
                            &mut aba.sheet, &id, serde_json::json!({"position": pos}),
                        ).is_ok() {
                            self.alterado = true;
                            // O editor lateral mantém as dimensões da alça,
                            // evitando restaurar o tamanho antigo ao salvar.
                            if self.graficos.id.as_deref() == Some(id.as_str()) {
                                self.graficos.aparencia.largura = format!("{:.0}", novo.largura);
                                self.graficos.aparencia.altura = format!("{:.0}", novo.altura);
                            }
                        }
                    }
                }
                self.renovar_dados_da_aba(self.folha, geracao_antes);
            }
            Evento::Pairar(id, p) => {
                if self.graficos.arraste.is_none() && self.graficos.redimensionamento.is_none() {
                    self.pairar_grafico(id, p);
                }
            }
            Evento::SairDoGrafico(id) => self.sair_do_grafico(&id),
            Evento::QuadroTransicao => self.quadro_transicao_grafico(),
            Evento::Arrastar(id) => {
                self.graficos.contexto = None;
                if self.clique_na_legenda(&id) {
                    self.graficos.selecionado = Some(id);
                    return Task::none();
                }
                // Segundo clique no mesmo gráfico logo depois: abre o editor.
                let duplo = self.graficos.ultimo_clique.as_ref()
                    .is_some_and(|(u, t)| *u == id && t.elapsed().as_millis() < 450);
                self.graficos.ultimo_clique = (!duplo).then(|| (id.clone(), std::time::Instant::now()));
                if duplo {
                    return self.evento_grafico(Evento::Editar(id));
                }
                self.graficos.selecionado = Some(id.clone());
                self.graficos.redimensionamento = None;
                if let Some(aba) = self.workbook.aba(self.folha) {
                    if let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) {
                        if let Some(v) = lista.iter().find(|v| v["id"].as_str() == Some(&id)) {
                            self.graficos.antes_do_gesto = Some((aba.id, aba.sheet.g.clone()));
                            self.graficos.arraste = Some((
                                id,
                                aba.id,
                                self.graficos.cursor,
                                v["position"]["x"].as_f64().unwrap_or(80.),
                                v["position"]["y"].as_f64().unwrap_or(80.),
                            ));
                        }
                    }
                }
            }
            Evento::Redimensionar(id, alca) => {
                self.graficos.selecionado = Some(id.clone());
                self.graficos.arraste = None;
                if let Some(aba) = self.workbook.aba(self.folha) {
                    if let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) {
                        if let Some(v) = lista.iter().find(|v| v["id"].as_str() == Some(&id)) {
                            if let Some(quadro) = Quadro::da_posicao(&v["position"]) {
                                self.graficos.antes_do_gesto = Some((aba.id, aba.sheet.g.clone()));
                                self.graficos.redimensionamento = Some((
                                    id, aba.id, self.graficos.cursor, quadro, alca,
                                ));
                            }
                        }
                    }
                }
            }
            Evento::Contexto(id) => {
                self.graficos.selecionado = Some(id.clone());
                self.graficos.arraste = None;
                self.graficos.redimensionamento = None;
                self.graficos.contexto = Some((id, self.ponteiro_x, self.ponteiro_y));
            }
            Evento::PassouNoMenu => {}
            Evento::FecharContexto => self.graficos.contexto = None,
            Evento::Remover(id) => {
                self.graficos.contexto = None;
                if let Some(aba) = self.workbook.aba_mut(self.folha) {
                    let removido = feathersheet_io::graficos_salvos::sem(&aba.sheet, &id)
                        .map(|novo| novo.is_some_and(|n| aba.sheet.alterar_graficos(Some(n), false)));
                    match removido {
                        Ok(true) => {
                            self.alterado = true;
                            self.graficos.aberto = false;
                            self.graficos.id = None;
                            self.graficos.selecionado = None;
                            self.status = tr!("Gráfico excluído.").into();
                        }
                        Ok(false) => {}
                        Err(e) => self.graficos.mensagem = e,
                    }
                }
            }
            Evento::Soltar => {
                self.graficos.arraste = None;
                self.graficos.redimensionamento = None;
                if let Some((aba_id, antes)) = self.graficos.antes_do_gesto.take() {
                    if let Some(aba) = self.workbook.aba_mut(self.folha).filter(|a| a.id == aba_id) {
                        if aba.sheet.g != antes {
                            let depois = std::mem::replace(&mut aba.sheet.g, antes);
                            aba.sheet.alterar_graficos(depois, false);
                        }
                    }
                }
            }
            Evento::Fechar => {
                self.graficos.aberto = false;
                self.graficos.seletor_aberto = false;
                self.graficos.cor_ativa = None;
            }
            Evento::FecharSeletor => {
                self.graficos.seletor_aberto = false;
            }
            Evento::Editar(id) => {
                self.graficos.contexto = None;
                self.graficos.arraste = None;
                self.graficos.selecionado = Some(id.clone());
                return self.editar_grafico_salvo(&id);
            }
            Evento::Inserir => {
                return self.inserir_grafico();
            }
            Evento::AbrirSeletorCor(alvo) => {
                if self.graficos.cor_ativa == Some(alvo) {
                    self.graficos.cor_ativa = None;
                } else {
                    let cor = match alvo {
                        AlvoCorGrafico::Principal => self.graficos.aparencia.cor.clone(),
                        AlvoCorGrafico::Fundo => self.graficos.aparencia.fundo.clone(),
                        AlvoCorGrafico::Serie(i) => self.graficos.aparencia.cores.get(&i)
                            .cloned().unwrap_or_else(|| self.graficos.aparencia.cor.clone()),
                        AlvoCorGrafico::Estilo(a) => {
                            let e = self.graficos.aparencia.estilo.clone().unwrap_or_default();
                            painel::cor_de(&e, a).map_or_else(|| "#4285f4".to_owned(), |c| c.texto())
                        }
                    };
                    self.graficos.cor_hsva = seletor_cor::rgba_para_hsva(
                        seletor_cor::rgba_de(&cor), self.graficos.cor_hsva[0],
                    );
                    self.graficos.cor_hex = cor;
                    self.graficos.cor_ativa = Some(alvo);
                }
            }
            Evento::SeletorCor(evento) => self.evento_seletor_cor_grafico(evento),
            e @ (Evento::Grupo(_) | Evento::Selecionar(_) | Evento::CorMenu(_) | Evento::MaisCores(_)
                | Evento::Cor(..) | Evento::Formatar(_)) => return self.evento_painel(e),
            Evento::CorSerie(i, valor) => {
                self.sincronizar_campo_hex_grafico(AlvoCorGrafico::Serie(i), &valor);
                self.graficos.aparencia.cores.insert(i, valor);
                self.editar_ao_vivo(Some("cor_serie"));
            }
            Evento::NomeSerie(i, valor) => {
                self.graficos.aparencia.nomes.insert(i, valor);
                self.editar_ao_vivo(Some("nome_serie"));
            }
            Evento::Estilo(campo, valor) => {
                match campo {
                    CampoEstilo::Cor =>
                        self.sincronizar_campo_hex_grafico(AlvoCorGrafico::Principal, &valor),
                    CampoEstilo::Fundo =>
                        self.sincronizar_campo_hex_grafico(AlvoCorGrafico::Fundo, &valor),
                    _ => {}
                }
                match campo {
                    CampoEstilo::Subtitulo => self.graficos.aparencia.subtitulo = valor,
                    CampoEstilo::Unidade => self.graficos.aparencia.unidade = valor,
                    CampoEstilo::Cor => self.graficos.aparencia.cor = valor,
                    CampoEstilo::Fonte => self.graficos.aparencia.fonte = valor,
                    CampoEstilo::EixoX => self.graficos.aparencia.eixo_x = valor,
                    CampoEstilo::EixoY => self.graficos.aparencia.eixo_y = valor,
                    CampoEstilo::Fundo => self.graficos.aparencia.fundo = valor,
                    CampoEstilo::Largura => self.graficos.aparencia.largura = valor,
                    CampoEstilo::Altura => self.graficos.aparencia.altura = valor,
                }
                self.editar_ao_vivo(Some(campo.chave()));
            }
            Evento::Visibilidade(campo, valor) => {
                match campo {
                    CampoVisivel::Titulo => self.graficos.aparencia.titulo = valor,
                    CampoVisivel::Legenda => self.graficos.aparencia.legenda = valor,
                    CampoVisivel::Eixos => self.graficos.aparencia.eixos = valor,
                    CampoVisivel::Valores => self.graficos.aparencia.valores = valor,
                }
                self.editar_ao_vivo(None);
            }
            Evento::Intervalo(s) => {
                self.graficos.intervalo = s;
                self.editar_ao_vivo(Some("intervalo"));
            }
            Evento::Titulo(s) => {
                self.graficos.titulo = s;
                self.editar_ao_vivo(Some("titulo"));
            }
            Evento::Rapido(tipo) => {
                self.graficos.seletor_aberto = false;
                let _ = self.evento_grafico(Evento::Abrir);
                let tipo = tipo.unwrap_or_else(|| self.tipo_recomendado());
                return self.evento_grafico(Evento::Tipo(tipo));
            }
            Evento::Tipo(t) => {
                self.graficos.cor_ativa = None;
                self.graficos.tipo = t;
                self.graficos.aberto = true;
                self.graficos.seletor_aberto = false;
                self.editar_ao_vivo(None);
                if self.graficos.id.is_none() && self.graficos.svg.is_some() {
                    return self.inserir_grafico();
                }
            }
            Evento::AlternarSeletor => {
                // Galeria de tipos e picker de cor não competem por foco.
                self.graficos.cor_ativa = None;
                self.graficos.seletor_aberto = !self.graficos.seletor_aberto;
            }
            Evento::Cabecalho(v) => {
                self.graficos.cabecalho = v;
                self.editar_ao_vivo(None);
            }
            Evento::Categorias(v) => {
                self.graficos.categorias = v;
                self.editar_ao_vivo(None);
            }
            Evento::Colunas(v) => {
                self.graficos.colunas = v;
                self.editar_ao_vivo(None);
            }
            Evento::Agrupar(v) => {
                if !v && self.graficos.resumo_obrigatorio {
                    self.graficos.mensagem = tr!("Com mais de 200.000 células o gráfico precisa resumir. Para ver ponto a ponto, selecione menos células.").into();
                    return Task::none();
                }
                self.graficos.agrupar = v;
                self.editar_ao_vivo(None);
            }
            Evento::Serie(s) => {
                self.graficos.serie = Some(s);
                self.editar_ao_vivo(None);
            }
            Evento::CategoriaColuna(c) => {
                let g = &mut self.graficos;
                g.categorias = c.is_some();
                if let Some(c) = c {
                    // As séries contam sem a coluna de categoria: trocar a
                    // categoria muda a ordem, então mostrar e contas recomeçam.
                    if g.coluna_categoria != c {
                        g.ocultas.clear();
                        g.contas.clear();
                        g.aparencia.nomes_origem.clear();
                    }
                    g.coluna_categoria = c;
                }
                self.editar_ao_vivo(None);
            }
            Evento::SerieVisivel(n, v) => {
                let total = self.series_da_origem().len() as u32;
                let g = &mut self.graficos;
                if g.tipo == TipoUi::Pizza {
                    // Pizza desenha uma série: escolher uma oculta as outras.
                    g.ocultas = (0..total).filter(|&i| i != n).collect();
                    g.serie = None;
                } else if v {
                    g.ocultas.retain(|&i| i != n);
                } else if total.saturating_sub(g.ocultas.len() as u32) > 1 {
                    g.ocultas.push(n);
                    g.ocultas.sort_unstable();
                } else {
                    g.mensagem = tr!("O gráfico precisa de pelo menos uma série.").into();
                    return Task::none();
                }
                self.editar_ao_vivo(None);
            }
            Evento::SerieConta(n, conta) => {
                let g = &mut self.graficos;
                match conta {
                    Some(c) => {
                        g.agrupar = true;
                        g.contas.insert(n, c);
                    }
                    None if g.resumo_obrigatorio => return Task::none(),
                    None => {
                        g.agrupar = false;
                        g.contas.clear();
                    }
                }
                self.editar_ao_vivo(None);
            }
            Evento::SerieNome(n, nome) => {
                if nome.is_empty() {
                    self.graficos.aparencia.nomes_origem.remove(&n);
                } else {
                    self.graficos.aparencia.nomes_origem.insert(n, nome);
                }
                self.editar_ao_vivo(Some("serie_nome"));
            }
            Evento::Copiar(id) => {
                self.graficos.contexto = None;
                self.copiar_grafico(&id);
            }
            Evento::Colar => {
                self.graficos.contexto = None;
                self.colar_grafico();
            }
            Evento::Duplicar(id) => {
                self.graficos.contexto = None;
                if self.copiar_grafico(&id) {
                    self.colar_grafico();
                }
            }
            Evento::AbaPainel(aba) => {
                self.graficos.painel.aba = aba;
                self.graficos.painel.cor_menu = None;
                self.graficos.cor_ativa = None;
            }
            Evento::UsarSelecao => {
                let area = self.area_selecionada();
                self.graficos.intervalo = format_ref(&area);
                self.graficos.aba_id = self.workbook.aba(self.folha).map(|a| a.id);
                self.graficos.ocultas.clear();
                self.graficos.contas.clear();
                self.graficos.aparencia.nomes_origem.clear();
                self.editar_ao_vivo(None);
            }
            Evento::Atualizar => {
                self.confirmar();
                self.atualizar_grafico();
            }
            Evento::Exportar => {
                if let Some(conteudo) = self.graficos.svg.clone() {
                    return Task::perform(
                        async move {
                            let Some(p) = rfd::AsyncFileDialog::new()
                                .add_filter("SVG", &["svg"])
                                .set_file_name("grafico.svg")
                                .save_file()
                                .await
                            else {
                                return Ok(false);
                            };
                            std::fs::write(p.path(), conteudo).map_err(|e| e.to_string())?;
                            Ok(true)
                        },
                        |r| Msg::Grafico(Evento::Exportado(r)),
                    );
                }
            }
            Evento::Exportado(r) => {
                self.graficos.mensagem = match r {
                    Ok(true) => tr!("Gráfico exportado em SVG").into(),
                    Ok(false) => tr!("Exportação cancelada").into(),
                    Err(e) => trf!("Erro ao exportar: {e}"),
                }
            }
        }
        Task::none()
    }
}
#[cfg(test)]
mod testes;
