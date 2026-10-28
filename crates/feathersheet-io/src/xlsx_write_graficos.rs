//! Gráficos da pasta no `.xlsx`, como o Excel grava: por aba com gráfico,
//! um `xl/drawings/drawingN.xml` com um `twoCellAnchor` por gráfico (e os
//! traços de tinta da aba, quando houver), as relações do desenho e um
//! `xl/charts/chartN.xml` (`c:chartSpace`) por gráfico. Cascata e histograma
//! vão no formato do Office 2016 (`cx:chartSpace`, `chartExN.xml`), dentro de
//! um `mc:AlternateContent` cuja reserva é um gráfico clássico parecido
//! (`reservaN.xml`) para quem não lê o formato novo.
//!
//! As séries apontam para as células (`'Aba'!$B$4:$B$9`) e levam o cache dos
//! valores, então o arquivo abre com o gráfico desenhado sem recalcular.
//! Quando o gráfico agrupa (soma por categoria, hierarquia de datas) ou foi
//! amostrado, não existe intervalo que dê aqueles números: aí os valores vão
//! literais (`numLit`/`strLit`).
//!
//! Tipos do motor sem equivalente no Excel — o mais próximo:
//! linhas/áreas em degraus → linhas/áreas comuns (mesmo empilhamento);
//! mapa de calor → colunas agrupadas; Gantt → barras empilhadas com a
//! primeira série (início) invisível e a duração calculada; linha 3D →
//! dispersão com linhas (X e Y; o Z fica de fora).

use std::fmt::Write as _;

use feathersheet_core::celulas::refs::col_name;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::grafico::chart::{
    extract_chart_data, get_series_color, get_stack_mode, ChartConfig, ChartData, ChartType, DateHierarchy, StackMode,
};
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::planilha::workbook::Workbook;

/// Escapa texto de elemento e de atributo entre aspas duplas; a aspa simples
/// fica como está, como o Excel grava (`'Aba'!$A$1`).
fn escape_xml(texto: &str) -> String {
    crate::xlsx_write::escape_xml(texto).replace("&apos;", "'")
}

const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
const NS_CX1: &str = "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex";
const NS_XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_GRAFICO: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const REL_GRAFICO_EX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
pub(crate) const TIPO_GRAFICO: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
pub(crate) const TIPO_GRAFICO_EX: &str = "application/vnd.ms-office.chartex+xml";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

const EMU_POR_PX: f64 = 9525.0;
/// Coluna sem largura própria: a da grade do FeatherSheet (a mesma que o
/// leitor usa em `graficos_ancora`).
const LARGURA_GRADE: f64 = 122.0;
/// Linha sem altura própria: a padrão que o `.xlsx` declara
/// (`defaultRowHeight="15"` pt = 20 px). É a altura com que o Excel desenha a
/// linha, e a que o leitor dá às linhas de uma aba com desenho
/// (`desenho_objetos::alturas_das_abas_com_desenho`): o gráfico sai com o
/// tamanho em pixels que tinha.
const ALTURA_ARQUIVO: f64 = 20.0;

/// Uma parte do pacote (`xl/charts/chart1.xml`) e o seu tipo de conteúdo.
pub(crate) struct ParteGrafico {
    pub caminho: String,
    pub xml: String,
    pub tipo: &'static str,
}

/// O desenho de uma aba: o XML, as relações dele e os gráficos.
pub(crate) struct DesenhoAba {
    pub xml: String,
    pub rels: Option<String>,
    pub partes: Vec<ParteGrafico>,
}

/// As linhas `<Override>` do `[Content_Types].xml` para os gráficos.
pub(crate) fn tipos_de_conteudo(partes: &[ParteGrafico]) -> String {
    partes
        .iter()
        .map(|p| format!("<Override PartName=\"/{}\" ContentType=\"{}\"/>", p.caminho, p.tipo))
        .collect()
}

// ------------------------------------------------------------ dados

#[derive(Clone, Debug, Default)]
struct Textos {
    f: Option<String>,
    v: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Numeros {
    f: Option<String>,
    /// `NaN` é célula vazia: não entra no cache.
    v: Vec<f64>,
}

#[derive(Clone, Debug)]
struct Serie {
    nome: Textos,
    val: Numeros,
    cor: String,
    invisivel: bool,
}

/// O que todas as séries compartilham: categorias e (dispersão) o X.
#[derive(Clone, Debug, Default)]
struct Extraido {
    cats: Option<Textos>,
    x: Option<Numeros>,
    series: Vec<Serie>,
}

/// `'Aba'!$A$4:$A$9` (ou `'Aba'!$A$3` para uma célula).
fn area_ref(aba: &str, r0: u32, c0: u32, r1: u32, c1: u32) -> String {
    let aba = format!("'{}'", aba.replace('\'', "''"));
    if r0 == r1 && c0 == c1 {
        format!("{aba}!${}${}", col_name(c0), r0 + 1)
    } else {
        format!("{aba}!${}${}:${}${}", col_name(c0), r0 + 1, col_name(c1), r1 + 1)
    }
}

/// Se a célula conta como número para virar série (o `parece_numero` do motor).
fn parece_numero(v: &Value) -> bool {
    match v {
        Value::Number(_) => true,
        Value::Text(t) => {
            let t = t.trim().replacen(',', ".", 1);
            let digitos: String = t
                .chars()
                .enumerate()
                .take_while(|(i, c)| c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))
                .map(|(_, c)| c)
                .collect();
            digitos.chars().any(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

fn texto_da_celula(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => feathersheet_core::celulas::number::to_js_string(*n),
        Value::Text(t) => t.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Error(e) => e.code.as_str().to_string(),
    }
}

/// Tudo literal, direto do que o motor extraiu.
fn literal(cfg: &ChartConfig, dados: &ChartData) -> Extraido {
    Extraido {
        cats: (!dados.categories.is_empty()).then(|| Textos { f: None, v: dados.categories.clone() }),
        x: dados.x_values.clone().map(|v| Numeros { f: None, v }),
        series: dados
            .series
            .iter()
            .enumerate()
            .map(|(i, s)| Serie {
                nome: Textos { f: None, v: vec![s.name.clone()] },
                val: Numeros { f: None, v: s.values.clone() },
                cor: get_series_color(&cfg.style, i as u32),
                invisivel: false,
            })
            .collect(),
    }
}

/// Uma série candidata: a área dos valores e a célula do nome, se houver.
struct Candidata {
    valores: (u32, u32, u32, u32),
    nome: Option<(u32, u32)>,
}

/// As séries com referência às células, refazendo a escolha de colunas (ou
/// linhas) do `extract_chart_data`. Se o resultado não bater com o que o
/// motor extraiu, cai no literal — nunca grava referência errada.
fn extrair(sheet: &Sheet, aba: &str, cfg: &ChartConfig, dados: &ChartData) -> Extraido {
    let lit = literal(cfg, dados);
    let r = cfg.range;
    let agrupa = cfg.aggregate || cfg.style.date_hierarchy.unwrap_or_default() != DateHierarchy::None;
    if agrupa || dados.amostrado.is_some() || cfg.tipo == ChartType::Line3d || r.r1 < r.r0 || r.c1 < r.c0 {
        return lit;
    }
    let num_rows = r.r1 - r.r0 + 1;
    let num_cols = r.c1 - r.c0 + 1;
    let cab = u32::from(cfg.has_headers && num_rows > 1);
    let inicio = r.r0 + cab;
    if inicio > r.r1 {
        return lit;
    }
    let cel = |l: u32, c: u32| sheet.get_value(l, c);
    let use_cat = cfg.has_category_labels;
    let mut cats = None;
    let mut candidatas: Vec<Candidata> = Vec::new();
    if cfg.series_in_columns {
        let cat_col = r.c0 + cfg.category_column_index.unwrap_or(0).min(num_cols - 1);
        if use_cat {
            cats = Some(Textos { f: Some(area_ref(aba, inicio, cat_col, r.r1, cat_col)), v: dados.categories.clone() });
        }
        for c in r.c0..=r.c1 {
            if (use_cat && c == cat_col) || (!cfg.series_columns.is_empty() && !cfg.series_columns.contains(&c)) {
                continue;
            }
            if (inicio..=r.r1).any(|l| parece_numero(&cel(l, c))) {
                let nome = (cab > 0 && !texto_da_celula(&cel(r.r0, c)).is_empty()).then_some((r.r0, c));
                candidatas.push(Candidata { valores: (inicio, c, r.r1, c), nome });
            }
        }
    } else {
        let cat_row = r.r0 + cfg.category_column_index.unwrap_or(0).min(num_rows - 1);
        let mut col_ini = r.c0 + u32::from(use_cat || cfg.has_headers);
        if cfg.has_headers && num_cols > 2 {
            let primeira = col_ini;
            for c in primeira..r.c1 {
                if (inicio..=r.r1).any(|l| parece_numero(&cel(l, c))) {
                    break;
                }
                col_ini = c + 1;
            }
        }
        if col_ini > r.c1 {
            return lit;
        }
        if use_cat {
            cats = Some(Textos { f: Some(area_ref(aba, cat_row, col_ini, cat_row, r.c1)), v: dados.categories.clone() });
        }
        for l in inicio..=r.r1 {
            if use_cat && l == cat_row {
                continue;
            }
            if (col_ini..=r.c1).any(|c| parece_numero(&cel(l, c))) {
                // Nome de várias colunas vira texto literal.
                let nome = (cfg.has_headers && col_ini == r.c0 + 1).then_some((l, r.c0));
                candidatas.push(Candidata { valores: (l, col_ini, l, r.c1), nome });
            }
        }
    }
    let mut x = None;
    if cfg.tipo == ChartType::Scatter && candidatas.len() > 1 {
        let c = candidatas.remove(0);
        let (a, b, cc, d) = c.valores;
        x = Some(Numeros { f: Some(area_ref(aba, a, b, cc, d)), v: dados.x_values.clone().unwrap_or_default() });
    }
    let escondidas = &cfg.style.hidden_series;
    let candidatas: Vec<Candidata> =
        candidatas.into_iter().enumerate().filter(|(k, _)| !escondidas.contains(&(*k as u32))).map(|(_, c)| c).collect();
    if candidatas.len() != dados.series.len() {
        return lit;
    }
    let series = candidatas
        .iter()
        .zip(&dados.series)
        .enumerate()
        .map(|(i, (c, s))| {
            let (a, b, cc, d) = c.valores;
            Serie {
                nome: Textos { f: c.nome.map(|(l, col)| area_ref(aba, l, col, l, col)), v: vec![s.name.clone()] },
                val: Numeros { f: Some(area_ref(aba, a, b, cc, d)), v: s.values.clone() },
                cor: get_series_color(&cfg.style, i as u32),
                invisivel: false,
            }
        })
        .collect();
    Extraido { cats, x, series }
}

// ------------------------------------------------------------ escrita c:

fn num(v: f64) -> String {
    feathersheet_core::celulas::number::to_js_string(v)
}

/// `#2563eb` → `2563EB`.
fn rgb(css: &str) -> String {
    crate::xlsx_write::para_argb_pub(css).map(|a| a[2..].to_string()).unwrap_or_else(|| "4472C4".into())
}

fn str_cache(o: &mut String, t: &Textos) {
    let _ = write!(o, "<c:ptCount val=\"{}\"/>", t.v.len());
    for (i, v) in t.v.iter().enumerate() {
        let _ = write!(o, "<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", escape_xml(v));
    }
}

fn num_cache(o: &mut String, n: &Numeros, formato: &str) {
    let _ = write!(o, "<c:formatCode>{}</c:formatCode><c:ptCount val=\"{}\"/>", escape_xml(formato), n.v.len());
    for (i, v) in n.v.iter().enumerate() {
        if v.is_finite() {
            let _ = write!(o, "<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", num(*v));
        }
    }
}

/// Fonte de texto (`cat`): `strRef` com cache, ou `strLit`.
fn fonte_texto(o: &mut String, t: &Textos) {
    match &t.f {
        Some(f) => {
            let _ = write!(o, "<c:strRef><c:f>{}</c:f><c:strCache>", escape_xml(f));
            str_cache(o, t);
            o.push_str("</c:strCache></c:strRef>");
        }
        None => {
            o.push_str("<c:strLit>");
            str_cache(o, t);
            o.push_str("</c:strLit>");
        }
    }
}

/// Fonte numérica (`val`, `xVal`, `yVal`, `bubbleSize`).
fn fonte_numero(o: &mut String, n: &Numeros) {
    match &n.f {
        Some(f) => {
            let _ = write!(o, "<c:numRef><c:f>{}</c:f><c:numCache>", escape_xml(f));
            num_cache(o, n, "General");
            o.push_str("</c:numCache></c:numRef>");
        }
        None => {
            o.push_str("<c:numLit>");
            num_cache(o, n, "General");
            o.push_str("</c:numLit>");
        }
    }
}

fn tx_serie(o: &mut String, t: &Textos) {
    let nome = t.v.first().cloned().unwrap_or_default();
    match &t.f {
        Some(f) => {
            let _ = write!(
                o,
                "<c:tx><c:strRef><c:f>{}</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>",
                escape_xml(f),
                escape_xml(&nome)
            );
        }
        None => {
            let _ = write!(o, "<c:tx><c:v>{}</c:v></c:tx>", escape_xml(&nome));
        }
    }
}

fn preenchimento(cor: &str) -> String {
    format!("<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>", rgb(cor))
}

/// `spPr` de barra/área/fatia.
fn sp_cheio(s: &Serie) -> String {
    if s.invisivel {
        "<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>".into()
    } else {
        format!("<c:spPr>{}</c:spPr>", preenchimento(&s.cor))
    }
}

/// `spPr` de linha (sem linha: só marcadores).
fn sp_linha(s: &Serie, com_linha: bool) -> String {
    if com_linha {
        format!("<c:spPr><a:ln w=\"28575\" cap=\"rnd\">{}<a:round/></a:ln></c:spPr>", preenchimento(&s.cor))
    } else {
        "<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr>".into()
    }
}

fn marcador(s: &Serie, simbolo: &str) -> String {
    if simbolo == "none" {
        return "<c:marker><c:symbol val=\"none\"/></c:marker>".into();
    }
    format!(
        "<c:marker><c:symbol val=\"{simbolo}\"/><c:size val=\"6\"/><c:spPr>{}<a:ln><a:noFill/></a:ln></c:spPr></c:marker>",
        preenchimento(&s.cor)
    )
}

fn simbolo_excel(forma: &str) -> &'static str {
    match forma {
        "square" => "square",
        "triangle" => "triangle",
        "diamond" => "diamond",
        "star" => "star",
        "x" => "x",
        "none" => "none",
        _ => "circle",
    }
}

fn rotulos(mostrar: bool) -> String {
    if !mostrar {
        return String::new();
    }
    "<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>".into()
}

fn rich(texto: &str, tamanho: u32, negrito: bool, segundo: Option<&str>) -> String {
    let b = if negrito { "1" } else { "0" };
    let mut o = format!(
        "<c:rich><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"{tamanho}\" b=\"{b}\"/></a:pPr><a:r><a:rPr lang=\"pt-BR\" sz=\"{tamanho}\" b=\"{b}\"/><a:t>{}</a:t></a:r></a:p>",
        escape_xml(texto)
    );
    if let Some(s) = segundo.filter(|s| !s.trim().is_empty()) {
        let _ = write!(
            o,
            "<a:p><a:pPr><a:defRPr sz=\"1000\" b=\"0\"/></a:pPr><a:r><a:rPr lang=\"pt-BR\" sz=\"1000\" b=\"0\"/><a:t>{}</a:t></a:r></a:p>",
            escape_xml(s)
        );
    }
    o.push_str("</c:rich>");
    o
}

fn titulo_eixo(texto: &str) -> String {
    if texto.trim().is_empty() {
        return String::new();
    }
    format!("<c:title><c:tx>{}</c:tx><c:overlay val=\"0\"/></c:title>", rich(texto, 1000, false, None))
}

const EIXO_A: u32 = 500_000_001;
const EIXO_B: u32 = 500_000_002;

/// Opções de aparência que valem para o gráfico todo.
struct Aparencia {
    titulo: Option<String>,
    subtitulo: String,
    legenda: Option<&'static str>,
    rotulos: bool,
    eixo_x: String,
    eixo_y: String,
    eixos: bool,
    fundo: Option<String>,
}

fn aparencia(v: &serde_json::Value, cfg: &ChartConfig) -> Aparencia {
    let estilo = &v["style"];
    let nativo = &v["_featherNative"];
    let mostrar_titulo = estilo["showTitle"].as_bool().unwrap_or(true);
    let titulo = (mostrar_titulo && !cfg.title.trim().is_empty()).then(|| cfg.title.clone());
    let legenda = match estilo["legendPosition"].as_str().unwrap_or("bottom") {
        "none" | "hidden" => None,
        "top" => Some("t"),
        "left" => Some("l"),
        "right" => Some("r"),
        _ => Some("b"),
    };
    let txt = |k: &str| nativo[k].as_str().unwrap_or("").to_string();
    Aparencia {
        titulo,
        subtitulo: estilo["subtitle"].as_str().map(str::to_string).unwrap_or_else(|| txt("subtitulo")),
        legenda,
        rotulos: estilo["dataLabels"].as_bool().unwrap_or(false),
        eixo_x: txt("eixo_x"),
        eixo_y: txt("eixo_y"),
        eixos: nativo["eixos"].as_bool().unwrap_or(true),
        fundo: nativo["fundo"].as_str().filter(|s| crate::xlsx_write::para_argb_pub(s).is_some()).map(str::to_string),
    }
}

/// Eixo de categorias + eixo de valores (ou dois de valores na dispersão).
fn eixos(ap: &Aparencia, horizontal: bool, dispersao: bool, percentual: bool, invertido: bool) -> String {
    let apagado = if ap.eixos { "0" } else { "1" };
    let (pos_cat, pos_val) = if horizontal { ("l", "b") } else { ("b", "l") };
    let formato_val = if percentual {
        "<c:numFmt formatCode=\"0%\" sourceLinked=\"0\"/>"
    } else {
        "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>"
    };
    let comum = "<c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>";
    let orientacao = if invertido { "maxMin" } else { "minMax" };
    let primeiro = if dispersao {
        format!(
            "<c:valAx><c:axId val=\"{EIXO_A}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"{apagado}\"/><c:axPos val=\"b\"/>{}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>{comum}<c:crossAx val=\"{EIXO_B}\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"midCat\"/></c:valAx>",
            titulo_eixo(&ap.eixo_x)
        )
    } else {
        format!(
            "<c:catAx><c:axId val=\"{EIXO_A}\"/><c:scaling><c:orientation val=\"{orientacao}\"/></c:scaling><c:delete val=\"{apagado}\"/><c:axPos val=\"{pos_cat}\"/>{}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>{comum}<c:crossAx val=\"{EIXO_B}\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx>",
            titulo_eixo(&ap.eixo_x)
        )
    };
    let entre = if dispersao { "midCat" } else { "between" };
    let segundo = format!(
        "<c:valAx><c:axId val=\"{EIXO_B}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"{apagado}\"/><c:axPos val=\"{pos_val}\"/><c:majorGridlines><c:spPr><a:ln w=\"6350\"><a:solidFill><a:srgbClr val=\"D9D9D9\"/></a:solidFill></a:ln></c:spPr></c:majorGridlines>{}{formato_val}{comum}<c:crossAx val=\"{EIXO_A}\"/><c:crosses val=\"{}\"/><c:crossBetween val=\"{entre}\"/></c:valAx>",
        titulo_eixo(&ap.eixo_y),
        if invertido { "max" } else { "autoZero" }
    );
    format!("{primeiro}{segundo}")
}

fn ids_eixos() -> String {
    format!("<c:axId val=\"{EIXO_A}\"/><c:axId val=\"{EIXO_B}\"/>")
}

/// Séries de barra/coluna.
fn series_barra(o: &mut String, ex: &Extraido, de: usize, series: &[Serie]) {
    for (k, s) in series.iter().enumerate() {
        let i = de + k;
        let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
        tx_serie(o, &s.nome);
        o.push_str(&sp_cheio(s));
        o.push_str("<c:invertIfNegative val=\"0\"/>");
        if let Some(c) = &ex.cats {
            o.push_str("<c:cat>");
            fonte_texto(o, c);
            o.push_str("</c:cat>");
        }
        o.push_str("<c:val>");
        fonte_numero(o, &s.val);
        o.push_str("</c:val></c:ser>");
    }
}

fn series_linha(o: &mut String, ex: &Extraido, de: usize, series: &[Serie], cfg: &ChartConfig, com_linha: bool) {
    for (k, s) in series.iter().enumerate() {
        let i = de + k;
        let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
        tx_serie(o, &s.nome);
        o.push_str(&sp_linha(s, com_linha));
        let forma = feathersheet_core::grafico::chart::get_series_point_shape(&cfg.style, i as u32);
        o.push_str(&marcador(s, simbolo_excel(forma)));
        if let Some(c) = &ex.cats {
            o.push_str("<c:cat>");
            fonte_texto(o, c);
            o.push_str("</c:cat>");
        }
        o.push_str("<c:val>");
        fonte_numero(o, &s.val);
        o.push_str("</c:val><c:smooth val=\"0\"/></c:ser>");
    }
}

fn agrupamento(modo: StackMode, barra: bool) -> &'static str {
    match modo {
        StackMode::None => {
            if barra {
                "clustered"
            } else {
                "standard"
            }
        }
        StackMode::Stacked => "stacked",
        StackMode::Percent => "percentStacked",
    }
}

/// O empilhamento pelo tipo (degraus incluídos), ou o escolhido no estilo.
fn modo_empilhar(cfg: &ChartConfig) -> StackMode {
    if cfg.style.stack_mode.is_some() {
        return get_stack_mode(cfg);
    }
    match cfg.tipo {
        ChartType::StepLineStacked | ChartType::StepAreaStacked => StackMode::Stacked,
        ChartType::StepLineStacked100 | ChartType::StepAreaStacked100 => StackMode::Percent,
        _ => get_stack_mode(cfg),
    }
}

/// O `plotArea` (gráficos + eixos) de um gráfico clássico.
fn area_de_plotagem(cfg: &ChartConfig, ex: &Extraido, ap: &Aparencia, pizza_serie: usize) -> String {
    let mut o = String::from("<c:plotArea><c:layout/>");
    let dl = rotulos(ap.rotulos);
    let modo = modo_empilhar(cfg);
    let pct = modo == StackMode::Percent;
    let barra = |o: &mut String, dir: &str, grupo: &str, series: &[Serie], ex: &Extraido| {
        let _ = write!(o, "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{grupo}\"/><c:varyColors val=\"0\"/>");
        series_barra(o, ex, 0, series);
        o.push_str(&dl);
        o.push_str("<c:gapWidth val=\"150\"/>");
        if grupo != "clustered" {
            o.push_str("<c:overlap val=\"100\"/>");
        }
        o.push_str(&ids_eixos());
        o.push_str("</c:barChart>");
    };
    match cfg.tipo {
        ChartType::Column | ChartType::ColumnStacked | ChartType::ColumnStacked100 | ChartType::Heatmap | ChartType::Outro => {
            barra(&mut o, "col", agrupamento(modo, true), &ex.series, ex);
            o.push_str(&eixos(ap, false, false, pct, false));
        }
        ChartType::Bar | ChartType::BarStacked | ChartType::BarStacked100 => {
            barra(&mut o, "bar", agrupamento(modo, true), &ex.series, ex);
            o.push_str(&eixos(ap, true, false, pct, false));
        }
        ChartType::Gantt => {
            // Início invisível + duração (término − início), empilhados.
            let mut series = Vec::new();
            if let [inicio, fim, ..] = ex.series.as_slice() {
                let mut base = inicio.clone();
                base.invisivel = true;
                let duracao = Serie {
                    nome: Textos { f: None, v: vec!["Duração".into()] },
                    val: Numeros {
                        f: None,
                        v: inicio.val.v.iter().zip(&fim.val.v).map(|(a, b)| b - a).collect(),
                    },
                    cor: inicio.cor.clone(),
                    invisivel: false,
                };
                series.push(base);
                series.push(duracao);
            } else {
                series = ex.series.clone();
            }
            barra(&mut o, "bar", "stacked", &series, ex);
            o.push_str(&eixos(ap, true, false, false, true));
        }
        ChartType::Line | ChartType::StepLine | ChartType::StepLineStacked | ChartType::StepLineStacked100 => {
            let _ = write!(o, "<c:lineChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>", agrupamento(modo, false));
            series_linha(&mut o, ex, 0, &ex.series, cfg, true);
            o.push_str(&dl);
            o.push_str("<c:marker val=\"1\"/>");
            o.push_str(&ids_eixos());
            o.push_str("</c:lineChart>");
            o.push_str(&eixos(ap, false, false, pct, false));
        }
        ChartType::Area | ChartType::StepArea | ChartType::StepAreaStacked | ChartType::StepAreaStacked100 => {
            let _ = write!(o, "<c:areaChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>", agrupamento(modo, false));
            for (i, s) in ex.series.iter().enumerate() {
                let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
                tx_serie(&mut o, &s.nome);
                o.push_str(&sp_cheio(s));
                if let Some(c) = &ex.cats {
                    o.push_str("<c:cat>");
                    fonte_texto(&mut o, c);
                    o.push_str("</c:cat>");
                }
                o.push_str("<c:val>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:val></c:ser>");
            }
            o.push_str(&dl);
            o.push_str(&ids_eixos());
            o.push_str("</c:areaChart>");
            o.push_str(&eixos(ap, false, false, pct, false));
        }
        ChartType::Pie | ChartType::Doughnut => {
            let rosca = cfg.tipo == ChartType::Doughnut;
            let paleta = cfg.style.palette.unwrap_or(feathersheet_core::grafico::chart::PaletteName::Vibrant);
            let cores = feathersheet_core::grafico::chart::palette(paleta);
            // Pizza: a série escolhida; rosca: todas, um anel cada (a cor é da fatia, igual em todos).
            let series: Vec<&Serie> = if rosca {
                ex.series.iter().collect()
            } else {
                ex.series.get(pizza_serie).or(ex.series.first()).into_iter().collect()
            };
            o.push_str(if rosca { "<c:doughnutChart>" } else { "<c:pieChart>" });
            o.push_str("<c:varyColors val=\"1\"/>");
            for (i, s) in series.into_iter().enumerate() {
                let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
                tx_serie(&mut o, &s.nome);
                for p in 0..s.val.v.len() {
                    let cor = cfg.style.series_colors.get(&(p as u32)).cloned().unwrap_or_else(|| cores[p % cores.len()].to_string());
                    let _ = write!(
                        o,
                        "<c:dPt><c:idx val=\"{p}\"/><c:bubble3D val=\"0\"/><c:spPr>{}<a:ln w=\"12700\"><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></a:ln></c:spPr></c:dPt>",
                        preenchimento(&cor)
                    );
                }
                if let Some(c) = &ex.cats {
                    o.push_str("<c:cat>");
                    fonte_texto(&mut o, c);
                    o.push_str("</c:cat>");
                }
                o.push_str("<c:val>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:val></c:ser>");
            }
            o.push_str(&dl);
            o.push_str("<c:firstSliceAng val=\"0\"/>");
            if rosca {
                let _ = write!(o, "<c:holeSize val=\"{}\"/></c:doughnutChart>", cfg.hole_size.unwrap_or(50).clamp(10, 90));
            } else {
                o.push_str("</c:pieChart>");
            }
        }
        ChartType::Scatter | ChartType::Line3d => {
            let com_linha = cfg.tipo == ChartType::Line3d;
            o.push_str("<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>");
            for (i, s) in ex.series.iter().enumerate() {
                let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
                tx_serie(&mut o, &s.nome);
                o.push_str(&sp_linha(s, com_linha));
                let forma = feathersheet_core::grafico::chart::get_series_point_shape(&cfg.style, i as u32);
                o.push_str(&marcador(s, simbolo_excel(forma)));
                if let Some(x) = &ex.x {
                    o.push_str("<c:xVal>");
                    fonte_numero(&mut o, x);
                    o.push_str("</c:xVal>");
                } else if let Some(c) = &ex.cats {
                    o.push_str("<c:xVal>");
                    fonte_texto(&mut o, c);
                    o.push_str("</c:xVal>");
                }
                o.push_str("<c:yVal>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:yVal><c:smooth val=\"0\"/></c:ser>");
            }
            o.push_str(&dl);
            o.push_str(&ids_eixos());
            o.push_str("</c:scatterChart>");
            o.push_str(&eixos(ap, false, true, false, false));
        }
        ChartType::Bubble => {
            // X nas categorias (números), Y na 1ª série, tamanho na 2ª.
            o.push_str("<c:bubbleChart><c:varyColors val=\"0\"/>");
            if let Some(s) = ex.series.first() {
                o.push_str("<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>");
                tx_serie(&mut o, &s.nome);
                let _ = write!(o, "<c:spPr><a:solidFill><a:srgbClr val=\"{}\"><a:alpha val=\"75000\"/></a:srgbClr></a:solidFill></c:spPr>", rgb(&s.cor));
                o.push_str("<c:invertIfNegative val=\"0\"/>");
                if let Some(c) = &ex.cats {
                    let x = Numeros { f: c.f.clone(), v: c.v.iter().map(|t| t.replacen(',', ".", 1).trim().parse().unwrap_or(f64::NAN)).collect() };
                    o.push_str("<c:xVal>");
                    fonte_numero(&mut o, &x);
                    o.push_str("</c:xVal>");
                }
                o.push_str("<c:yVal>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:yVal><c:bubbleSize>");
                match ex.series.get(1) {
                    Some(t) => fonte_numero(&mut o, &t.val),
                    None => fonte_numero(&mut o, &Numeros { f: None, v: vec![1.0; s.val.v.len()] }),
                }
                o.push_str("</c:bubbleSize><c:bubble3D val=\"0\"/></c:ser>");
            }
            o.push_str(&dl);
            o.push_str("<c:bubbleScale val=\"100\"/><c:showNegBubbles val=\"0\"/>");
            o.push_str(&ids_eixos());
            o.push_str("</c:bubbleChart>");
            o.push_str(&eixos(ap, false, true, false, false));
        }
        ChartType::Radar => {
            o.push_str("<c:radarChart><c:radarStyle val=\"marker\"/><c:varyColors val=\"0\"/>");
            for (i, s) in ex.series.iter().enumerate() {
                let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
                tx_serie(&mut o, &s.nome);
                o.push_str(&sp_linha(s, true));
                o.push_str(&marcador(s, "circle"));
                if let Some(c) = &ex.cats {
                    o.push_str("<c:cat>");
                    fonte_texto(&mut o, c);
                    o.push_str("</c:cat>");
                }
                o.push_str("<c:val>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:val></c:ser>");
            }
            o.push_str(&dl);
            o.push_str(&ids_eixos());
            o.push_str("</c:radarChart>");
            o.push_str(&eixos(ap, false, false, false, false));
        }
        ChartType::Candlestick => {
            // Abertura, máxima, mínima, fechamento: as quatro séries do Excel.
            o.push_str("<c:stockChart>");
            for (i, s) in ex.series.iter().take(4).enumerate() {
                let _ = write!(o, "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
                tx_serie(&mut o, &s.nome);
                o.push_str("<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>");
                if let Some(c) = &ex.cats {
                    o.push_str("<c:cat>");
                    fonte_texto(&mut o, c);
                    o.push_str("</c:cat>");
                }
                o.push_str("<c:val>");
                fonte_numero(&mut o, &s.val);
                o.push_str("</c:val><c:smooth val=\"0\"/></c:ser>");
            }
            o.push_str(&dl);
            o.push_str("<c:hiLowLines><c:spPr><a:ln w=\"9525\"><a:solidFill><a:srgbClr val=\"404040\"/></a:solidFill></a:ln></c:spPr></c:hiLowLines><c:upDownBars><c:gapWidth val=\"150\"/><c:upBars><c:spPr><a:solidFill><a:srgbClr val=\"20A06A\"/></a:solidFill></c:spPr></c:upBars><c:downBars><c:spPr><a:solidFill><a:srgbClr val=\"D74146\"/></a:solidFill></c:spPr></c:downBars></c:upDownBars>");
            o.push_str(&ids_eixos());
            o.push_str("</c:stockChart>");
            o.push_str(&eixos(ap, false, false, false, false));
        }
        ChartType::Combo => {
            // Colunas da primeira série e linhas das demais, nos mesmos eixos.
            let (primeira, resto) = ex.series.split_at(ex.series.len().min(1));
            barra(&mut o, "col", "clustered", primeira, ex);
            if !resto.is_empty() {
                o.push_str("<c:lineChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>");
                series_linha(&mut o, ex, 1, resto, cfg, true);
                o.push_str(&dl);
                o.push_str("<c:marker val=\"1\"/>");
                o.push_str(&ids_eixos());
                o.push_str("</c:lineChart>");
            }
            o.push_str(&eixos(ap, false, false, false, false));
        }
        ChartType::Waterfall | ChartType::Histogram => {
            // Só a reserva clássica passa por aqui (ver `reserva_classica`).
            barra(&mut o, "col", "stacked", &ex.series, ex);
            o.push_str(&eixos(ap, false, false, false, false));
        }
    }
    o.push_str("</c:plotArea>");
    o
}

fn chart_space(plot: &str, ap: &Aparencia, legenda_sem: &[usize], so_visiveis: bool) -> String {
    let mut o = format!(
        "{XML_DECL}<c:chartSpace xmlns:c=\"{NS_C}\" xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\"><c:date1904 val=\"0\"/><c:lang val=\"pt-BR\"/><c:roundedCorners val=\"0\"/><c:chart>"
    );
    match &ap.titulo {
        Some(t) => {
            let _ = write!(
                o,
                "<c:title><c:tx>{}</c:tx><c:overlay val=\"0\"/></c:title><c:autoTitleDeleted val=\"0\"/>",
                rich(t, 1400, true, Some(&ap.subtitulo))
            );
        }
        None => o.push_str("<c:autoTitleDeleted val=\"1\"/>"),
    }
    o.push_str(plot);
    if let Some(pos) = ap.legenda {
        let _ = write!(o, "<c:legend><c:legendPos val=\"{pos}\"/>");
        for i in legenda_sem {
            let _ = write!(o, "<c:legendEntry><c:idx val=\"{i}\"/><c:delete val=\"1\"/></c:legendEntry>");
        }
        o.push_str("<c:overlay val=\"0\"/></c:legend>");
    }
    // O gráfico feito aqui desenha a linha oculta; só o que veio do Excel assim a esconde.
    o.push_str(&format!("<c:plotVisOnly val=\"{}\"/><c:dispBlanksAs val=\"gap\"/></c:chart>", u8::from(so_visiveis)));
    let fundo = ap.fundo.as_deref().map(preenchimento).unwrap_or_else(|| "<a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill>".into());
    let _ = write!(o, "<c:spPr>{fundo}<a:ln w=\"9525\"><a:solidFill><a:srgbClr val=\"D9D9D9\"/></a:solidFill></a:ln></c:spPr>");
    o.push_str("<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"1000\"/></a:pPr><a:endParaRPr lang=\"pt-BR\"/></a:p></c:txPr></c:chartSpace>");
    o
}

/// O `c:chartSpace` de um gráfico clássico.
fn grafico_classico(cfg: &ChartConfig, ex: &Extraido, ap: &Aparencia, pizza_serie: usize) -> String {
    let plot = area_de_plotagem(cfg, ex, ap, pizza_serie);
    let sem = if cfg.tipo == ChartType::Gantt && ex.series.len() >= 2 { vec![0] } else { Vec::new() };
    chart_space(&plot, ap, &sem, cfg.plot_vis_only)
}

// ------------------------------------------------------------ escrita cx:

/// Os intervalos de um histograma como o motor desenha: √n faixas iguais.
fn faixas_histograma(valores: &[f64]) -> (Vec<String>, Vec<f64>) {
    let v: Vec<f64> = valores.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let min = v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let n = ((v.len() as f64).sqrt() as usize).clamp(1, 32);
    let mut freq = vec![0.0; n];
    for x in &v {
        let p = if max > min { ((x - min) / (max - min) * n as f64) as usize } else { 0 };
        freq[p.min(n - 1)] += 1.0;
    }
    let passo = (max - min) / n as f64;
    let rotulos = (0..n)
        .map(|i| format!("[{}; {}]", num(min + passo * i as f64), num(min + passo * (i + 1) as f64)))
        .collect();
    (rotulos, freq)
}

/// A reserva clássica de cascata/histograma, para quem não lê `chartEx`.
fn reserva_classica(cfg: &ChartConfig, ex: &Extraido, ap: &Aparencia) -> String {
    let Some(s) = ex.series.first() else {
        return grafico_classico(&ChartConfig { tipo: ChartType::Column, ..cfg.clone() }, ex, ap, 0);
    };
    if cfg.tipo == ChartType::Histogram {
        let (rotulos, freq) = faixas_histograma(&s.val.v);
        let ex2 = Extraido {
            cats: Some(Textos { f: None, v: rotulos }),
            x: None,
            series: vec![Serie { nome: s.nome.clone(), val: Numeros { f: None, v: freq }, cor: s.cor.clone(), invisivel: false }],
        };
        let cfg2 = ChartConfig { tipo: ChartType::Column, ..cfg.clone() };
        return grafico_classico(&cfg2, &ex2, ap, 0);
    }
    // Cascata: base invisível + variação, empilhadas.
    let (mut base, mut barra) = (Vec::new(), Vec::new());
    let mut saldo = 0.0;
    for v in &s.val.v {
        let v = if v.is_finite() { *v } else { 0.0 };
        let novo = saldo + v;
        base.push(f64::min(saldo, novo));
        barra.push(v.abs());
        saldo = novo;
    }
    let ex2 = Extraido {
        cats: ex.cats.clone(),
        x: None,
        series: vec![
            Serie { nome: Textos { f: None, v: vec!["Base".into()] }, val: Numeros { f: None, v: base }, cor: s.cor.clone(), invisivel: true },
            Serie { nome: s.nome.clone(), val: Numeros { f: None, v: barra }, cor: s.cor.clone(), invisivel: false },
        ],
    };
    let plot = area_de_plotagem(&ChartConfig { tipo: ChartType::Waterfall, ..cfg.clone() }, &ex2, ap, 0);
    chart_space(&plot, ap, &[0], cfg.plot_vis_only)
}

fn cx_texto(t: &Textos) -> String {
    let nome = escape_xml(t.v.first().map(String::as_str).unwrap_or(""));
    match &t.f {
        Some(f) => format!("<cx:tx><cx:txData><cx:f>{}</cx:f><cx:v>{nome}</cx:v></cx:txData></cx:tx>", escape_xml(f)),
        None => format!("<cx:tx><cx:txData><cx:v>{nome}</cx:v></cx:txData></cx:tx>"),
    }
}

/// O `cx:chartSpace` (Office 2016) da cascata ou do histograma.
fn grafico_ex(cfg: &ChartConfig, ex: &Extraido, ap: &Aparencia) -> String {
    let mut o = format!("{XML_DECL}<cx:chartSpace xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:cx=\"{NS_CX}\"><cx:chartData><cx:data id=\"0\">");
    let vazia = Serie { nome: Textos::default(), val: Numeros::default(), cor: "#2563eb".into(), invisivel: false };
    let s = ex.series.first().unwrap_or(&vazia);
    let histograma = cfg.tipo == ChartType::Histogram;
    if !histograma {
        if let Some(c) = &ex.cats {
            o.push_str("<cx:strDim type=\"cat\">");
            if let Some(f) = &c.f {
                let _ = write!(o, "<cx:f>{}</cx:f>", escape_xml(f));
            }
            let _ = write!(o, "<cx:lvl ptCount=\"{}\">", c.v.len());
            for (i, v) in c.v.iter().enumerate() {
                let _ = write!(o, "<cx:pt idx=\"{i}\">{}</cx:pt>", escape_xml(v));
            }
            o.push_str("</cx:lvl></cx:strDim>");
        }
    }
    o.push_str("<cx:numDim type=\"val\">");
    if let Some(f) = &s.val.f {
        let _ = write!(o, "<cx:f>{}</cx:f>", escape_xml(f));
    }
    let _ = write!(o, "<cx:lvl ptCount=\"{}\" formatCode=\"General\">", s.val.v.len());
    for (i, v) in s.val.v.iter().enumerate() {
        if v.is_finite() {
            let _ = write!(o, "<cx:pt idx=\"{i}\">{}</cx:pt>", num(*v));
        }
    }
    o.push_str("</cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart>");
    if let Some(t) = &ap.titulo {
        let _ = write!(
            o,
            "<cx:title pos=\"t\" align=\"ctr\" overlay=\"0\"><cx:tx><cx:txData><cx:v>{}</cx:v></cx:txData></cx:tx></cx:title>",
            escape_xml(t)
        );
    }
    let layout = if histograma { "clusteredColumn" } else { "waterfall" };
    let _ = write!(o, "<cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"{layout}\" uniqueId=\"{{00000000-0000-4000-8000-000000000001}}\">");
    o.push_str(&cx_texto(&s.nome));
    let _ = write!(o, "<cx:spPr>{}</cx:spPr>", preenchimento(&s.cor));
    if ap.rotulos {
        o.push_str("<cx:dataLabels pos=\"outEnd\"><cx:visibility seriesName=\"0\" categoryName=\"0\" value=\"1\"/></cx:dataLabels>");
    }
    o.push_str("<cx:dataId val=\"0\"/>");
    if histograma {
        let n = ((s.val.v.iter().filter(|v| v.is_finite()).count() as f64).sqrt() as usize).clamp(1, 32);
        let _ = write!(o, "<cx:layoutPr><cx:binning intervalClosed=\"r\"><cx:binCount val=\"{n}\"/></cx:binning></cx:layoutPr>");
    } else {
        o.push_str("<cx:layoutPr><cx:visibility connectorLines=\"1\"/></cx:layoutPr>");
    }
    o.push_str("</cx:series></cx:plotAreaRegion>");
    let gap = if histograma { "0" } else { "0.5" };
    let oculto = if ap.eixos { "0" } else { "1" };
    let _ = write!(
        o,
        "<cx:axis id=\"0\" hidden=\"{oculto}\"><cx:catScaling gapWidth=\"{gap}\"/><cx:tickLabels/></cx:axis><cx:axis id=\"1\" hidden=\"{oculto}\"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis></cx:plotArea>"
    );
    if let Some(pos) = ap.legenda {
        let _ = write!(o, "<cx:legend pos=\"{pos}\" align=\"ctr\" overlay=\"0\"/>");
    }
    o.push_str("</cx:chart></cx:chartSpace>");
    o
}

// ------------------------------------------------------------ desenho

/// px → (índice da coluna/linha, deslocamento em EMU).
fn canto(px: f64, tamanho: impl Fn(u32) -> f64, limite: u32) -> (u32, i64) {
    let px = px.max(0.0);
    let (mut i, mut acumulado) = (0u32, 0.0);
    while i < limite {
        let t = tamanho(i).max(0.0);
        if acumulado + t > px {
            break;
        }
        acumulado += t;
        i += 1;
    }
    (i, ((px - acumulado) * EMU_POR_PX).round() as i64)
}

fn ancora_xml(sheet: &Sheet, pos: &serde_json::Value) -> (String, String) {
    let n = |k: &str, padrao: f64| pos[k].as_f64().filter(|v| v.is_finite()).unwrap_or(padrao);
    let (x, y) = (n("x", 50.0), n("y", 50.0));
    let (w, h) = (n("width", 480.0).max(20.0), n("height", 300.0).max(20.0));
    let largura_padrao = sheet.largura_padrao.filter(|w| *w > 0).map(f64::from).unwrap_or(LARGURA_GRADE);
    let largura = |c: u32| sheet.col_widths.get(&c).map(|&v| f64::from(v)).unwrap_or(largura_padrao);
    let altura = |r: u32| sheet.row_heights.get(&r).map(|&v| f64::from(v)).unwrap_or(ALTURA_ARQUIVO);
    let ponto = |px: f64, py: f64| {
        let (c, dx) = canto(px, largura, 16_383);
        let (r, dy) = canto(py, altura, 1_048_575);
        format!("<xdr:col>{c}</xdr:col><xdr:colOff>{dx}</xdr:colOff><xdr:row>{r}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff>")
    };
    (ponto(x, y), ponto(x + w, y + h))
}

fn quadro(id: usize, nome: &str, uri: &str, elemento: &str, rid: &str) -> String {
    format!(
        "<xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{uri}\">{elemento} r:id=\"{rid}\"/></a:graphicData></a:graphic></xdr:graphicFrame>",
        escape_xml(nome)
    )
}

/// O que fica entre `<xdr:wsDr …>` e `</xdr:wsDr>` no desenho da tinta.
fn miolo_do_desenho(xml: &str) -> &str {
    let Some(i) = xml.find("<xdr:wsDr") else { return "" };
    let Some(j) = xml[i..].find('>') else { return "" };
    let Some(k) = xml.rfind("</xdr:wsDr>") else { return "" };
    xml.get(i + j + 1..k).unwrap_or("")
}

/// A lista de gráficos da aba (o JSON opaco `g`).
fn graficos_da_aba(sheet: &Sheet) -> Vec<serde_json::Value> {
    sheet
        .g
        .as_deref()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

/// O desenho da aba `aba` (0-based): os gráficos dela mais a tinta (já em
/// `tinta`, o `drawingN.xml` pronto). `contador` numera as partes dos
/// gráficos na pasta inteira. `None` quando a aba não tem nada a desenhar.
pub(crate) fn desenho_da_aba(pasta: &Workbook, aba: usize, tinta: Option<String>, contador: &mut usize) -> Option<DesenhoAba> {
    let viva = pasta.aba(aba)?;
    let sheet = &viva.sheet;
    let lista = graficos_da_aba(sheet);
    if lista.is_empty() {
        return tinta.map(|xml| DesenhoAba { xml, rels: None, partes: Vec::new() });
    }
    let mut ancoras = String::new();
    let mut rels = String::new();
    let mut partes = Vec::new();
    let mut usa_mc = false;
    for (k, v) in lista.iter().enumerate() {
        let Ok(cfg) = serde_json::from_value::<ChartConfig>(v.clone()) else { continue };
        // Os dados podem morar em outra aba (`sourceSheet`).
        let origem = cfg
            .source_sheet
            .as_deref()
            .and_then(|n| pasta.indice_por_nome(n))
            .and_then(|i| pasta.aba(i))
            .unwrap_or(viva);
        let dados = extract_chart_data(&origem.sheet, &cfg);
        let mut ex = extrair(&origem.sheet, &origem.sheet.name, &cfg, &dados);
        // Bolhas, velas e Gantt: as séries na ordem dos campos escolhidos no painel (como o app desenha).
        let papeis: Vec<u32> = serde_json::from_value(v["_featherNative"]["papeis"].clone()).unwrap_or_default();
        let ordem = feathersheet_core::grafico::chart::ordem_dos_papeis(cfg.tipo, &cfg.style.hidden_series, &papeis, ex.series.len());
        ex.series = ordem.into_iter().map(|i| ex.series[i].clone()).collect();
        let ap = aparencia(v, &cfg);
        // O app grava "2 — Meta" (posição a partir de 1), não um número.
        let pizza = v["_featherPieSeries"].as_str()
            .and_then(|s| s.split(" — ").next())
            .and_then(|s| s.trim().parse::<usize>().ok())
            .or_else(|| v["_featherPieSeries"].as_u64().map(|n| n as usize + 1))
            .unwrap_or(1)
            .saturating_sub(1);
        *contador += 1;
        let n = *contador;
        let rid = format!("rIdGrafico{}", k + 1);
        let (de, ate) = ancora_xml(sheet, &v["position"]);
        let id_forma = 100_000 + k * 2; // longe dos ids da tinta (1, 2, …)
        let nome = if cfg.title.trim().is_empty() { format!("Gráfico {}", k + 1) } else { cfg.title.clone() };
        let _ = write!(ancoras, "<xdr:twoCellAnchor><xdr:from>{de}</xdr:from><xdr:to>{ate}</xdr:to>");
        if matches!(cfg.tipo, ChartType::Waterfall | ChartType::Histogram) {
            usa_mc = true;
            let rid_reserva = format!("rIdReserva{}", k + 1);
            let principal = quadro(id_forma, &nome, NS_CX, "<cx:chart", &rid);
            let reserva = quadro(id_forma + 1, &nome, NS_C, "<c:chart", &rid_reserva);
            let _ = write!(
                ancoras,
                "<mc:AlternateContent><mc:Choice Requires=\"cx1\">{principal}</mc:Choice><mc:Fallback>{reserva}</mc:Fallback></mc:AlternateContent>"
            );
            let _ = write!(
                rels,
                "<Relationship Id=\"{rid}\" Type=\"{REL_GRAFICO_EX}\" Target=\"../charts/chartEx{n}.xml\"/><Relationship Id=\"{rid_reserva}\" Type=\"{REL_GRAFICO}\" Target=\"../charts/reserva{n}.xml\"/>"
            );
            partes.push(ParteGrafico { caminho: format!("xl/charts/chartEx{n}.xml"), xml: grafico_ex(&cfg, &ex, &ap), tipo: TIPO_GRAFICO_EX });
            partes.push(ParteGrafico { caminho: format!("xl/charts/reserva{n}.xml"), xml: reserva_classica(&cfg, &ex, &ap), tipo: TIPO_GRAFICO });
        } else {
            ancoras.push_str(&quadro(id_forma, &nome, NS_C, "<c:chart", &rid));
            let _ = write!(rels, "<Relationship Id=\"{rid}\" Type=\"{REL_GRAFICO}\" Target=\"../charts/chart{n}.xml\"/>");
            partes.push(ParteGrafico { caminho: format!("xl/charts/chart{n}.xml"), xml: grafico_classico(&cfg, &ex, &ap, pizza), tipo: TIPO_GRAFICO });
        }
        ancoras.push_str("<xdr:clientData/></xdr:twoCellAnchor>");
    }
    if partes.is_empty() {
        return tinta.map(|xml| DesenhoAba { xml, rels: None, partes: Vec::new() });
    }
    let tinta_miolo = tinta.as_deref().map(miolo_do_desenho).unwrap_or("");
    let mc = if usa_mc { format!(" xmlns:mc=\"{NS_MC}\" xmlns:cx1=\"{NS_CX1}\" xmlns:cx=\"{NS_CX}\"") } else { String::new() };
    let xml = format!(
        "{XML_DECL}<xdr:wsDr xmlns:xdr=\"{NS_XDR}\" xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:c=\"{NS_C}\"{mc}>{tinta_miolo}{ancoras}</xdr:wsDr>"
    );
    let rels = format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{rels}</Relationships>");
    Some(DesenhoAba { xml, rels: Some(rels), partes })
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn pasta_com_graficos(tipos: &[&str]) -> Workbook {
        let mut wb = Workbook::new();
        wb.renomear_aba(0, "Dados e Gráficos");
        let linhas = [
            ["Mês", "Vendas", "Meta", "Custo", "Fim"],
            ["Jan", "120", "100", "80", "130"],
            ["Fev", "135", "110", "85", "140"],
            ["Mar", "150", "120", "90", "160"],
            ["Abr", "110", "130", "95", "150"],
        ];
        let lista: Vec<serde_json::Value> = tipos
            .iter()
            .enumerate()
            .map(|(i, t)| {
                json!({"id": format!("g{i}"), "type": t, "title": format!("Gráfico {t}"),
                  "range": {"r0": 0, "c0": 0, "r1": 4, "c1": 3},
                  "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
                  "style": {"showTitle": true, "legendPosition": "bottom", "dataLabels": i % 2 == 0},
                  "position": {"x": 20.0 + i as f64 * 10.0, "y": 200.0, "width": 480.0, "height": 300.0}})
            })
            .collect();
        wb.editar(0, |s| {
            for (r, l) in linhas.iter().enumerate() {
                for (c, v) in l.iter().enumerate() {
                    s.set_cell(r as u32, c as u32, v);
                }
            }
            s.g = Some(serde_json::to_string(&lista).unwrap());
        });
        wb
    }

    fn bem_formado(xml: &str) {
        let mut leitor = quick_xml::Reader::from_str(xml);
        loop {
            match leitor.read_event() {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("XML mal formado: {e}\n{xml}"),
            }
        }
    }

    #[test]
    fn ancora_cobre_as_celulas_pela_grade() {
        let mut wb = Workbook::new();
        wb.editar(0, |s| {
            s.col_widths.insert(0, 50);
            s.row_heights.insert(1, 40);
        });
        let s = &wb.aba(0).unwrap().sheet;
        // x = 60 → coluna 1 (50 px) + 10; y = 65 → linha 2 (20+40) + 5.
        let (de, ate) = ancora_xml(s, &json!({"x": 60, "y": 65, "width": 234, "height": 100}));
        assert_eq!(de, "<xdr:col>1</xdr:col><xdr:colOff>95250</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>47625</xdr:rowOff>");
        // 60 + 234 = 294 = 50 + 122 + 122 → começo da coluna 3.
        assert!(ate.starts_with("<xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff>"), "{ate}");
    }

    #[test]
    fn referencias_e_cache_das_series() {
        let wb = pasta_com_graficos(&["column"]);
        let mut n = 0;
        let d = desenho_da_aba(&wb, 0, None, &mut n).unwrap();
        let g = &d.partes[0].xml;
        assert!(g.contains("<c:f>'Dados e Gráficos'!$A$2:$A$5</c:f>"), "{g}");
        assert!(g.contains("<c:f>'Dados e Gráficos'!$B$1</c:f>"));
        assert!(g.contains("<c:f>'Dados e Gráficos'!$B$2:$B$5</c:f>"));
        assert!(g.contains("<c:pt idx=\"0\"><c:v>120</c:v></c:pt>"));
        assert!(g.contains("<c:pt idx=\"3\"><c:v>Abr</c:v></c:pt>"));
        assert!(g.contains("<c:barDir val=\"col\"/><c:grouping val=\"clustered\"/>"));
        assert!(g.contains("2563EB"), "cor da primeira série");
        assert_eq!(g.matches("<c:ser>").count(), 3);
    }

    #[test]
    fn todos_os_tipos_geram_xml_bem_formado() {
        let tipos = [
            "column", "column-stacked", "column-stacked-100", "bar", "bar-stacked", "bar-stacked-100", "line", "line-3d",
            "step-line", "step-line-stacked", "step-line-stacked-100", "area", "step-area", "step-area-stacked",
            "step-area-stacked-100", "pie", "scatter", "bubble", "combo", "heatmap", "waterfall", "histogram", "radar",
            "candlestick", "gantt", "desconhecido",
        ];
        let wb = pasta_com_graficos(&tipos);
        let mut n = 0;
        let d = desenho_da_aba(&wb, 0, None, &mut n).unwrap();
        bem_formado(&d.xml);
        bem_formado(d.rels.as_deref().unwrap());
        // 26 gráficos; cascata e histograma levam chartEx + reserva.
        assert_eq!(n, tipos.len());
        assert_eq!(d.partes.len(), tipos.len() + 2);
        for p in &d.partes {
            bem_formado(&p.xml);
        }
        let achar = |c: &str| d.partes.iter().find(|p| p.caminho == c).map(|p| p.xml.as_str()).unwrap();
        assert!(achar("xl/charts/chart2.xml").contains("<c:grouping val=\"stacked\"/>"));
        assert!(achar("xl/charts/chart3.xml").contains("<c:grouping val=\"percentStacked\"/>"));
        assert!(achar("xl/charts/chart4.xml").contains("<c:barDir val=\"bar\"/>"));
        assert!(achar("xl/charts/chart10.xml").contains("<c:lineChart><c:grouping val=\"stacked\"/>"));
        assert!(achar("xl/charts/chart16.xml").contains("<c:pieChart>"));
        assert!(achar("xl/charts/chart17.xml").contains("<c:scatterChart>"));
        assert!(achar("xl/charts/chart18.xml").contains("<c:bubbleSize>"));
        let combo = achar("xl/charts/chart19.xml");
        assert!(combo.contains("<c:barChart>") && combo.contains("<c:lineChart>"));
        assert!(achar("xl/charts/chartEx21.xml").contains("layoutId=\"waterfall\""));
        assert!(achar("xl/charts/chartEx22.xml").contains("layoutId=\"clusteredColumn\""));
        assert!(achar("xl/charts/chart23.xml").contains("<c:radarChart>"));
        assert!(achar("xl/charts/chart24.xml").contains("<c:stockChart>"));
        assert!(achar("xl/charts/chart25.xml").contains("<a:noFill/>"), "início invisível do Gantt");
        assert!(d.xml.contains("<mc:Choice Requires=\"cx1\">"));
    }

    #[test]
    fn rosca_e_campos_de_bolhas_e_velas_vao_e_voltam() {
        let mut wb = pasta_com_graficos(&["doughnut", "bubble", "candlestick"]);
        wb.editar(0, |s| {
            let mut lista: Vec<serde_json::Value> = serde_json::from_str(s.g.as_deref().unwrap()).unwrap();
            lista[0]["holeSize"] = json!(65);
            // Bolhas: Y = Meta, Tamanho = Vendas (o contrário das colunas).
            lista[1]["_featherNative"] = json!({"papeis": [1, 0]});
            // Velas: Abertura = Meta, Máxima = Vendas, Mínima = Custo, Fechamento = Fim.
            lista[2]["range"]["c1"] = json!(4);
            lista[2]["_featherNative"] = json!({"papeis": [1, 0, 2, 3]});
            s.g = Some(serde_json::to_string(&lista).unwrap());
        });
        let mut n = 0;
        let d = desenho_da_aba(&wb, 0, None, &mut n).unwrap();
        let rosca = &d.partes[0].xml;
        bem_formado(rosca);
        assert!(rosca.contains("<c:doughnutChart><c:varyColors val=\"1\"/>"), "{rosca}");
        assert_eq!(rosca.matches("<c:ser>").count(), 3, "um anel por série");
        assert!(rosca.contains("<c:firstSliceAng val=\"0\"/><c:holeSize val=\"65\"/></c:doughnutChart>"), "{rosca}");
        let bolhas = &d.partes[1].xml;
        assert!(bolhas.contains("<c:yVal><c:numRef><c:f>'Dados e Gráficos'!$C$2:$C$5</c:f>"), "{bolhas}");
        assert!(bolhas.contains("<c:bubbleSize><c:numRef><c:f>'Dados e Gráficos'!$B$2:$B$5</c:f>"), "{bolhas}");
        let velas = &d.partes[2].xml;
        let ordem: Vec<usize> = ["$C$2", "$B$2", "$D$2", "$E$2"].iter().map(|c| velas.find(c).unwrap()).collect();
        assert!(ordem.windows(2).all(|w| w[0] < w[1]), "abertura, máxima, mínima, fechamento: {velas}");

        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        // Para abrir no Excel/LibreOffice e conferir à mão.
        if let Ok(caminho) = std::env::var("FEATHERSHEET_GRAFICOS_XLSX") {
            std::fs::write(caminho, &bytes).unwrap();
        }
        let (volta, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let lista: Vec<serde_json::Value> = serde_json::from_str(&volta.aba(0).unwrap().sheet.g.clone().unwrap()).unwrap();
        let tipo = |t: &str| lista.iter().find(|v| v["type"] == t).unwrap_or_else(|| panic!("{t}: {lista:?}"));
        assert_eq!(tipo("doughnut")["holeSize"], json!(65));
        assert_eq!(tipo("bubble")["_featherNative"]["papeis"], json!([1, 0]));
        assert_eq!(tipo("candlestick")["_featherNative"]["papeis"], json!([1, 0, 2, 3]));
        assert_eq!(tipo("candlestick")["range"], json!({"r0": 0, "c0": 0, "r1": 4, "c1": 4}));
    }

    #[test]
    fn xlsx_completo_e_leitor_devolve_os_graficos() {
        let tipos = ["column", "bar", "line", "area", "pie", "scatter", "radar", "waterfall", "histogram"];
        let wb = pasta_com_graficos(&tipos);
        let partes = crate::xlsx_write::montar_partes(&wb);
        let nome = |c: &str| partes.iter().find(|(n, _)| n == c).map(|(_, b)| String::from_utf8(b.clone()).unwrap());
        let tipos_conteudo = nome("[Content_Types].xml").unwrap();
        assert!(tipos_conteudo.contains("/xl/charts/chart1.xml"));
        assert!(tipos_conteudo.contains("/xl/charts/chartEx8.xml"));
        assert!(tipos_conteudo.contains("/xl/drawings/drawing1.xml"));
        assert!(nome("xl/worksheets/sheet1.xml").unwrap().contains("<drawing r:id=\"rIdTinta\"/>"));
        assert!(nome("xl/worksheets/_rels/sheet1.xml.rels").unwrap().contains("../drawings/drawing1.xml"));
        assert!(nome("xl/drawings/_rels/drawing1.xml.rels").unwrap().contains("../charts/chart1.xml"));

        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (volta, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let g = volta.aba(0).unwrap().sheet.g.clone().expect("gráficos na volta");
        let lista: Vec<serde_json::Value> = serde_json::from_str(&g).unwrap();
        assert_eq!(lista.len(), tipos.len(), "{g}");
        let mut lidos: Vec<String> = lista.iter().map(|v| v["type"].as_str().unwrap_or("").to_string()).collect();
        lidos.sort();
        let mut esperados: Vec<String> = tipos.iter().map(|t| t.to_string()).collect();
        esperados.sort();
        assert_eq!(lidos, esperados);
        let coluna = lista.iter().find(|v| v["type"] == "column").unwrap();
        assert_eq!(coluna["range"], json!({"r0": 0, "c0": 0, "r1": 4, "c1": 3}));
        assert_eq!(coluna["position"]["width"].as_f64(), Some(480.0));
        assert_eq!(coluna["position"]["height"].as_f64(), Some(300.0));
        assert_eq!((coluna["position"]["x"].as_f64(), coluna["position"]["y"].as_f64()), (Some(20.0), Some(200.0)));
    }
}
