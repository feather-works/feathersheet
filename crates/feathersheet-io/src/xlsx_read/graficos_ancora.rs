//! Onde fica cada gráfico do `.xlsx`: a aba vem da relação aba → desenho →
//! gráfico (não do intervalo dos dados, que pode estar em outra aba), e o
//! retângulo vem do `twoCellAnchor`/`oneCellAnchor` do desenho. Também lê o
//! gráfico do Office 2016 (`chartEx`: cascata, funil, histograma…), que não
//! tem `plotArea/barChart` e cujos dados apontam para nomes ocultos
//! (`_xlchart.v5.N`) do `workbook.xml`.

use super::formas::caminho_relativo;
use super::sheet::{read_rels, SheetJson};
use super::xml::{parse_xml, XmlNode};
use super::{entry_text, refs};
use std::collections::HashMap;
use std::io::Cursor;
use zip::ZipArchive;

const EMU_POR_PX: f64 = 9525.0;
/// Largura e altura das colunas/linhas sem tamanho próprio na grade
/// (`feathersheet-gui/src/grade.rs`: `LARGURA_PADRAO`, `ALTURA_PADRAO`).
const LARGURA_GRADE: f64 = 122.0;
const ALTURA_GRADE: f64 = 28.0;

/// Canto da âncora: célula (base 0) e deslocamento em pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Canto {
    pub coluna: u32,
    pub dx: f64,
    pub linha: u32,
    pub dy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LugarGrafico {
    pub aba: usize,
    pub de: Canto,
    /// `None` no `oneCellAnchor`: aí vale `tamanho`.
    pub ate: Option<Canto>,
    pub tamanho: (f64, f64),
    /// Posição na ordem de empilhamento do desenho (a das formas, `z` do `ob`).
    pub z: usize,
}

fn numero(no: Option<&XmlNode>) -> f64 {
    no.and_then(|n| n.text.trim().parse::<f64>().ok()).unwrap_or(0.0)
}

fn canto(no: &XmlNode) -> Canto {
    Canto {
        coluna: numero(no.child("col")).max(0.0) as u32,
        dx: numero(no.child("colOff")) / EMU_POR_PX,
        linha: numero(no.child("row")).max(0.0) as u32,
        dy: numero(no.child("rowOff")) / EMU_POR_PX,
    }
}

/// O `r:id` do gráfico dentro de um `graphicFrame` (`c:chart` ou `cx:chart`).
fn id_do_grafico(no: &XmlNode) -> Option<&str> {
    if no.name == "chart" {
        if let Some(id) = no.attr("r:id") {
            return Some(id);
        }
    }
    no.children.iter().find_map(id_do_grafico)
}

/// Caminho do gráfico no zip (`xl/charts/chart1.xml`) → aba e âncora.
pub(super) fn lugares(
    archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>,
    abas: &[(String, String, bool)],
) -> HashMap<String, LugarGrafico> {
    let mut saida = HashMap::new();
    for (i, (_, folha, _)) in abas.iter().enumerate() {
        let (dir, arq) = folha.rsplit_once('/').unwrap_or(("", folha));
        let Ok(rels) = entry_text(archive, &format!("{dir}/_rels/{arq}.rels")) else { continue };
        let Ok(rels_xml) = parse_xml(&rels) else { continue };
        let desenhos: Vec<String> = rels_xml
            .children_named("Relationship")
            .filter(|r| r.attr("Type").is_some_and(|t| t.ends_with("/drawing")))
            .filter_map(|r| r.attr("Target"))
            .map(|t| caminho_relativo(folha, t))
            .collect();
        for d in desenhos {
            let Ok(x) = entry_text(archive, &d).and_then(|t| parse_xml(&t)) else { continue };
            let (ddir, darq) = d.rsplit_once('/').unwrap_or(("", &d));
            let rels_d = entry_text(archive, &format!("{ddir}/_rels/{darq}.rels")).map(|t| read_rels(&t)).unwrap_or_default();
            for (z, a) in x.children.iter().filter(|c| c.name.ends_with("Anchor")).enumerate() {
                if !a.name.ends_with("CellAnchor") { continue; }
                // O gráfico do Office 2016 vem dentro de `mc:AlternateContent`.
                let Some(id) = id_do_grafico(a) else { continue };
                let Some(alvo) = rels_d.get(id) else { continue };
                let Some(de) = a.child("from").map(canto) else { continue };
                let ate = a.child("to").map(canto);
                let ext = a.child("ext");
                let medida = |n: &str| ext.and_then(|e| e.attr(n)).and_then(|v| v.parse::<f64>().ok()).map(|v| v / EMU_POR_PX).unwrap_or(0.0);
                saida.insert(
                    caminho_relativo(&d, alvo),
                    LugarGrafico { aba: i, de, ate, tamanho: (medida("cx"), medida("cy")), z },
                );
            }
        }
    }
    saida
}

fn inicio(tamanhos: &HashMap<i64, i64>, padrao: f64, indice: u32) -> f64 {
    let mut total = indice as f64 * padrao;
    for (&i, &t) in tamanhos {
        if i >= 0 && (i as u32) < indice {
            total += t as f64 - padrao;
        }
    }
    total
}

/// Retângulo absoluto na folha (px de zoom 100 %), como o `position` dos
/// gráficos guardados: `{x, y, width, height}`.
pub(super) fn posicao(lugar: &LugarGrafico, aba: &SheetJson) -> serde_json::Value {
    let larguras: HashMap<i64, i64> = aba.col_widths.iter().copied().collect();
    let alturas: HashMap<i64, i64> = aba.row_heights.iter().copied().collect();
    let largura_padrao = aba.largura_padrao.filter(|w| *w > 0).map(|w| w as f64).unwrap_or(LARGURA_GRADE);
    let ponto = |c: &Canto| {
        (
            inicio(&larguras, largura_padrao, c.coluna) + c.dx,
            inicio(&alturas, ALTURA_GRADE, c.linha) + c.dy,
        )
    };
    let (x, y) = ponto(&lugar.de);
    let (w, h) = match &lugar.ate {
        Some(ate) => {
            let (x1, y1) = ponto(ate);
            (x1 - x, y1 - y)
        }
        None => lugar.tamanho,
    };
    serde_json::json!({
        "x": x.round(), "y": y.round(),
        "width": w.max(120.0).round(), "height": h.max(100.0).round(),
    })
}

/// Intervalo com a aba (sem aba = a do gráfico): `(aba, (r0, c0, r1, c1))`.
pub(super) type AreaNomeada = (Option<String>, (i64, i64, i64, i64));

/// Dados de um `chartEx`: tipo, título, intervalo (categorias + valores),
/// aba de origem e as colunas do meio que não são série.
pub(super) struct GraficoEx {
    pub tipo: &'static str,
    pub titulo: String,
    pub aba: Option<String>,
    pub area: (i64, i64, i64, i64),
    pub ocultas: Vec<u32>,
}

fn texto_de(no: &XmlNode, saida: &mut String) {
    if no.name == "t" || (no.name == "v" && saida.is_empty()) {
        saida.push_str(&no.text);
    }
    for c in &no.children {
        texto_de(c, saida);
    }
}

/// `cx:chartSpace` → [`GraficoEx`]. `nomes` resolve `_xlchart.v5.N`.
pub(super) fn ler_chartex(raiz: &XmlNode, nomes: &HashMap<String, AreaNomeada>) -> Option<GraficoEx> {
    let dados = raiz.child("chartData")?;
    let grafico = raiz.child("chart")?;
    let serie = grafico.child("plotArea")?.child("plotAreaRegion")?.child("series")?;
    let tipo = match serie.attr("layoutId").unwrap_or("") {
        "waterfall" => "waterfall",
        "clusteredColumn" | "paretoLine" => "histogram",
        // Funil, mapa de árvore, explosão solar e caixa: sem tipo próprio no
        // motor ainda; abrem como barras/colunas com os mesmos dados.
        "funnel" => "bar",
        _ => "column",
    };
    let mut titulo = String::new();
    if let Some(t) = grafico.child("title") {
        texto_de(t, &mut titulo);
    }
    let resolver = |f: &str| -> Option<AreaNomeada> {
        let f = f.trim();
        if let Some(r) = nomes.get(f) {
            return Some(r.clone());
        }
        let limpo = f.replace('$', "");
        let (aba, ref_) = match limpo.rsplit_once('!') {
            Some((a, r)) => (Some(a.trim_matches('\'').to_string()), r.to_string()),
            None => (None, limpo.clone()),
        };
        let mut p = ref_.split(':');
        let a = p.next().and_then(refs::parse_cell_ref)?;
        let b = p.next().and_then(refs::parse_cell_ref).unwrap_or(a);
        Some((aba, (a.row, a.col, b.row, b.col)))
    };
    let mut categorias = None;
    let mut valores = None;
    for d in dados.children_named("data") {
        for dim in &d.children {
            let Some(f) = dim.child("f") else { continue };
            let Some(r) = resolver(&f.text) else { continue };
            match (dim.name.as_str(), dim.attr("type")) {
                ("strDim", _) | (_, Some("cat")) => categorias = categorias.or(Some(r)),
                (_, Some("val")) | ("numDim", _) => valores = valores.or(Some(r)),
                _ => {}
            }
        }
    }
    // O Excel às vezes guarda as categorias da cascata no nome da série
    // (`series/tx/txData/f`), um intervalo na vertical do mesmo tamanho.
    if categorias.is_none() {
        if let Some(r) = serie.child("tx").and_then(|t| t.child("txData")).and_then(|t| t.child("f")).and_then(|f| resolver(&f.text)) {
            categorias = Some(r);
        }
    }
    let (aba_v, v) = valores?;
    let (aba, area, ocultas) = match categorias {
        Some((aba_c, c)) if aba_c == aba_v && c.0 == v.0 && c.2 == v.2 && c.1 < v.1 => {
            // Categoria à esquerda dos valores: as colunas no meio não são série.
            let ocultas = (c.1 + 1..v.1).map(|col| (col - c.1 - 1) as u32).collect();
            (aba_v, (v.0, c.1, v.2, v.3), ocultas)
        }
        _ => (aba_v, v, Vec::new()),
    };
    Some(GraficoEx { tipo, titulo: titulo.trim().to_string(), aba, area, ocultas })
}

/// Cor de preenchimento de um `spPr`: sólida, ou a primeira parada do
/// degradê. `Some(None)` = `noFill`.
fn cor_do_sppr(pr: Option<&XmlNode>) -> Option<Option<String>> {
    let pr = pr?;
    if pr.child("noFill").is_some() { return Some(None); }
    if let Some(s) = pr.child("solidFill") { return Some(super::desenho_objetos::cor_alfa(s)); }
    let g = pr.child("gradFill")?.child("gsLst")?;
    Some(g.children_named("gs").next().and_then(super::desenho_objetos::cor_alfa))
}

/// Cor do traço (`ln`) de um `spPr`.
fn cor_da_linha(pr: Option<&XmlNode>) -> Option<String> {
    let ln = pr?.child("ln")?;
    if ln.child("noFill").is_some() { return None; }
    super::desenho_objetos::cor_alfa(ln.child("solidFill")?)
}

/// Cor de um preenchimento de texto: sólido, ou o padrão `pctN` (N % de
/// pontos da cor da frente sobre a de fundo) misturado numa cor só.
fn cor_do_texto(r: &XmlNode) -> Option<String> {
    if let Some(s) = r.child("solidFill") { return super::desenho_objetos::cor_alfa(s); }
    let p = r.child("pattFill")?;
    let pct = p.attr("prst").and_then(|v| v.strip_prefix("pct")).and_then(|v| v.parse::<f64>().ok())? / 100.0;
    let rgb = |n: &str| -> Option<[f64; 3]> {
        let h = super::desenho_objetos::cor_alfa(p.child(n)?)?;
        let v = u32::from_str_radix(h.trim_start_matches('#').get(..6)?, 16).ok()?;
        Some([((v >> 16) & 255) as f64, ((v >> 8) & 255) as f64, (v & 255) as f64])
    };
    let (f, b) = (rgb("fgClr")?, rgb("bgClr").unwrap_or([255.0; 3]));
    let m: Vec<u8> = (0..3).map(|i| (f[i] * pct + b[i] * (1.0 - pct)).round() as u8).collect();
    Some(format!("#{:02x}{:02x}{:02x}", m[0], m[1], m[2]))
}

/// Cor e tamanho (pt) do texto de um `txPr` (`a:defRPr`).
fn texto_do_txpr(tx: Option<&XmlNode>) -> (Option<String>, Option<f64>, bool) {
    let r = tx.and_then(|t| t.child("p")).and_then(|p| p.child("pPr")).and_then(|p| p.child("defRPr"));
    let cor = r.and_then(cor_do_texto);
    let sz = r.and_then(|r| r.attr("sz")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100.0);
    (cor, sz, matches!(r.and_then(|r| r.attr("b")), Some("1") | Some("true")))
}

/// Formato do rótulo do Excel (`R$ #,##0.00`, contábil, `0.0`) no
/// vocabulário do gráfico: `moeda:N`, `percentual:N`, `decimal:N`.
pub(super) fn formato_do_excel(codigo: &str) -> Option<String> {
    let c = codigo.split(';').next().unwrap_or(codigo);
    if c.eq_ignore_ascii_case("general") || c.is_empty() { return None; }
    let casas = c.split_once('.').map_or(0, |(_, f)| f.chars().take_while(|ch| *ch == '0').count());
    let tipo = if c.contains("R$") || c.contains('$') || c.contains('€') { "moeda" }
        else if c.contains('%') { "percentual" } else { "decimal" };
    Some(format!("{tipo}:{casas}"))
}

/// Aparência de um gráfico do Excel (`c:chartSpace`): o `_featherNative`
/// (o que o editor de gráficos entende: cores das séries, legenda, eixos,
/// rótulos) e o `_excel` (fundo em degradê/transparente, contorno, cor e
/// tamanho do texto, formato dos rótulos, eixo de valores apagado, grade).
pub(super) fn estilo_excel(raiz: &XmlNode) -> Option<(serde_json::Value, serde_json::Value)> {
    let grafico = raiz.child("chart")?;
    let area = grafico.child("plotArea")?;
    let tipo = area.children.iter().find(|c| c.name.ends_with("Chart"))?;
    let mut cores = serde_json::Map::new();
    let mut primeira = None;
    for (i, ser) in tipo.children_named("ser").enumerate() {
        let pr = ser.child("spPr");
        let linha = tipo.name.starts_with("line") || tipo.name.starts_with("scatter") || tipo.name.starts_with("radar");
        let cor = if linha { cor_da_linha(pr) } else { cor_do_sppr(pr).flatten() };
        if let Some(c) = cor {
            let c = c.chars().take(7).collect::<String>();
            if i == 0 { primeira = Some(c.clone()); }
            cores.insert(i.to_string(), serde_json::Value::String(c));
        }
    }
    // Os rótulos da série valem mais que os do tipo (o Excel grava os dois).
    let rotulos = tipo.child("ser").and_then(|s| s.child("dLbls")).or_else(|| tipo.child("dLbls"));
    let valores = rotulos.and_then(|d| d.child("showVal")).is_some_and(|v| matches!(v.attr("val"), Some("1") | Some("true")));
    let legenda = grafico.child("legend");
    let eixo_valor = area.child("valAx");
    let eixo_cat = area.child("catAx").or_else(|| area.child("dateAx"));
    let apagado = |e: Option<&XmlNode>| e.and_then(|e| e.child("delete")).is_some_and(|d| matches!(d.attr("val"), Some("1") | Some("true")));
    let titulo = grafico.child("title").is_some()
        && !grafico.child("autoTitleDeleted").is_some_and(|d| matches!(d.attr("val"), Some("1") | Some("true")));
    // Texto: o do eixo de categorias, senão o do gráfico inteiro.
    let (cor_eixo, tam_eixo, _) = texto_do_txpr(eixo_cat.and_then(|e| e.child("txPr")));
    let (cor_eixo_valor, _, _) = texto_do_txpr(eixo_valor.and_then(|e| e.child("txPr")));
    let (cor_geral, tam_geral, _) = texto_do_txpr(raiz.child("txPr"));
    let (cor_rotulo, tam_rotulo, negrito_rotulo) = texto_do_txpr(rotulos.and_then(|d| d.child("txPr")));
    let formato = rotulos.and_then(|d| d.child("numFmt")).and_then(|n| n.attr("formatCode")).and_then(formato_do_excel)
        .or_else(|| {
            // Sem formato próprio no rótulo: o da origem (cache dos valores).
            tipo.child("ser").and_then(|s| s.child("val")).and_then(|v| v.child("numRef"))
                .and_then(|n| n.child("numCache")).and_then(|c| c.child("formatCode")).and_then(|f| formato_do_excel(&f.text))
        });
    let fundo_pr = raiz.child("spPr");
    let mut fundo_degrade = Vec::new();
    if let Some(g) = fundo_pr.and_then(|p| p.child("gradFill")) {
        for gs in g.child("gsLst").into_iter().flat_map(|l| l.children_named("gs")) {
            let pos = gs.attr("pos").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100_000.0;
            if let Some(c) = super::desenho_objetos::cor_alfa(gs) { fundo_degrade.push(serde_json::json!([pos, c])); }
        }
    }
    let angulo = fundo_pr.and_then(|p| p.child("gradFill")).and_then(|g| g.child("lin")).and_then(|l| l.attr("ang"))
        .and_then(|v| v.parse::<f64>().ok()).map_or(90.0, |a| a / 60_000.0);
    // Só `noFill` é transparente. Sem `spPr`, sem preenchimento dentro dele
    // (só a borda) ou com cor que não se resolve: o automático do Excel, branco.
    let transparente = fundo_pr.is_some_and(|p| p.child("noFill").is_some());
    let fundo = if transparente {
        "#00000000".to_string()
    } else {
        cor_do_sppr(fundo_pr).flatten().unwrap_or_else(|| "#ffffff".to_string())
    };
    let tamanho = tam_eixo.or(tam_geral).unwrap_or(10.0);
    let nativa = serde_json::json!({
        "cor": primeira.unwrap_or_else(|| "#4472c4".into()),
        "cores": cores,
        "fundo": "#00000000",
        "fonte": format!("{}", tamanho.clamp(4.0, 32.0)),
        "titulo": titulo,
        "legenda": legenda.is_some(),
        "eixos": !(apagado(eixo_valor) && apagado(eixo_cat)),
        "valores": valores,
        "formato": formato.clone().unwrap_or_default(),
    });
    let excel = serde_json::json!({
        "fundo": fundo,
        "fundoDegrade": fundo_degrade,
        "fundoAngulo": angulo,
        "contorno": cor_da_linha(fundo_pr),
        "corTexto": cor_eixo.or(cor_geral),
        "corEixoValores": cor_eixo_valor,
        "corRotulo": cor_rotulo,
        "tamanhoRotulo": tam_rotulo,
        "negritoRotulo": negrito_rotulo,
        "eixoValores": !apagado(eixo_valor),
        "eixoCategorias": !apagado(eixo_cat),
        "grade": eixo_valor.is_some_and(|e| e.child("majorGridlines").is_some()),
        "legendaPos": legenda.and_then(|l| l.child("legendPos")).and_then(|p| p.attr("val")).unwrap_or("r"),
        "tresD": tipo.name.contains("3D"),
    });
    Some((nativa, excel))
}

/// Séries que pulam colunas no `.xlsx`: a coluna da categoria (dentro do
/// retângulo) e a coluna de cada `ser`, na ordem do arquivo.
pub(super) struct Saltos {
    pub categoria: u32,
    pub colunas: Vec<u32>,
}

/// O `parece_numero` do gráfico, calculando só a fórmula que abriu sem
/// valor guardado, e com a pasta em volta: `get_value` sem ela refaz os
/// derramamentos a cada leitura e não guarda nada (travava a abertura).
// shortcut: texto guardado na coluna empacotada conta como não número (o pool não está à mão);
// texto que parece número numa fórmula empacotada erra o índice das ocultas por um.
fn numerica(wb: &feathersheet_core::planilha::workbook::Workbook, aba: usize, l: u32, c: u32) -> bool {
    use feathersheet_core::celulas::values::Value;
    use feathersheet_core::planilha::data_table::Cru;
    let parece = crate::xlsx_write_graficos::parece_numero;
    let Some(sheet) = wb.aba(aba).map(|a| &a.sheet) else { return false };
    if !sheet.is_formula(l, c) {
        let raw = sheet.get_raw(l, c);
        return !raw.is_empty() && parece(&Value::Text(raw));
    }
    if let Some(v) = sheet.valor_guardado(l, c) {
        return parece(&v);
    }
    match sheet.formulas.cru_at(l, c) {
        Some(Cru::Numero(_)) => true,
        Some(Cru::Texto(_)) => false,
        None => parece(&wb.valor(aba, l, c)),
    }
}

/// Chave provisória do JSON do gráfico: as colunas das `ser`, até a aba ser lida.
pub(super) const COLUNAS_SER: &str = "_xlsxColunasSer";

/// O `hiddenSeries` conta só as colunas que têm número (como o editor e o
/// gravador do `.xlsx`), então só sai com as abas já lidas. As cores e os
/// pontos do Excel vêm pela ordem das `ser` e passam para esse índice.
pub(super) fn ocultar_colunas_puladas(wb: &mut feathersheet_core::planilha::workbook::Workbook) {
    for i in 0..wb.len() {
        let Some(texto) = wb.aba(i).and_then(|a| a.sheet.g.clone()).filter(|g| g.contains(COLUNAS_SER)) else { continue };
        let Ok(mut lista) = serde_json::from_str::<Vec<serde_json::Value>>(&texto) else { continue };
        for g in &mut lista {
            let Some(cols) = g.as_object_mut().and_then(|o| o.remove(COLUNAS_SER)) else { continue };
            let cols: Vec<u32> = serde_json::from_value(cols).unwrap_or_default();
            let Ok(cfg) = serde_json::from_value::<feathersheet_core::grafico::chart::ChartConfig>(g.clone()) else { continue };
            let origem = cfg.source_sheet.as_deref()
                .and_then(|n| (0..wb.len()).find(|&j| wb.aba(j).is_some_and(|a| a.sheet.name.eq_ignore_ascii_case(n))))
                .unwrap_or(i);
            let r = cfg.range;
            let inicio = r.r0 + u32::from(cfg.has_headers && r.r1 > r.r0);
            let cat = r.c0 + cfg.category_column_index.unwrap_or(0);
            let numericas: Vec<u32> = (r.c0..=r.c1)
                .filter(|&c| !(cfg.has_category_labels && c == cat))
                .filter(|&c| (inicio..=r.r1).any(|l| numerica(wb, origem, l, c)))
                .collect();
            let ordem = |c: u32| numericas.iter().position(|&n| n == c);
            let ocultas: Vec<usize> = (0..numericas.len()).filter(|&k| !cols.contains(&numericas[k])).collect();
            if !g["style"].is_object() { g["style"] = serde_json::json!({}); }
            g["style"]["hiddenSeries"] = serde_json::json!(ocultas);
            for caminho in ["/_featherNative/cores", "/style/seriesPointShapes", "/style/seriesPointSizes"] {
                if let Some(m) = g.pointer_mut(caminho).and_then(|v| v.as_object_mut()) {
                    *m = std::mem::take(m).into_iter()
                        .filter_map(|(k, v)| Some((ordem(*cols.get(k.parse::<usize>().ok()?)?)?.to_string(), v)))
                        .collect();
                }
            }
        }
        if let Some(a) = wb.aba_mut(i) {
            a.sheet.g = Some(serde_json::Value::Array(lista).to_string());
        }
    }
}

/// O Excel guarda cada série com o próprio intervalo (`B` categoria, `L`, `M`,
/// `N` valores); o modelo tem um retângulo só (`B:N`). Sem isto as colunas
/// `C`–`K` viravam séries. `None` quando o retângulo já é o gráfico.
// shortcut: só séries em colunas; em linhas o retângulo continua inteiro, até aparecer um arquivo assim.
pub(super) fn series_salteadas(
    em_colunas: bool,
    (_, c0, _, c1): (i64, i64, i64, i64),
    cat: Option<(refs::CellRef, refs::CellRef)>,
    vals: &[(refs::CellRef, refs::CellRef)],
) -> Option<Saltos> {
    if !em_colunas || vals.iter().any(|(a, b)| a.col != b.col) {
        return None;
    }
    let cat = cat.filter(|(a, b)| a.col == b.col).map(|(a, _)| a.col);
    let usadas: Vec<i64> = vals.iter().map(|(a, _)| a.col).collect();
    let pulou = (c0..=c1).any(|c| Some(c) != cat && !usadas.contains(&c));
    let categoria = cat.map_or(0, |k| (k - c0) as u32);
    if !pulou && categoria == 0 {
        return None;
    }
    Some(Saltos { categoria, colunas: usadas.into_iter().map(|c| c as u32).collect() })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn series_que_pulam_colunas_escondem_as_do_meio() {
        let r = |t: &str| refs::parse_cell_ref(t).unwrap();
        let faixa = |a: &str, b: &str| (r(a), r(b));
        // Categoria B, valores L, M, N (o "Gráfico Status x Objeto" do usuário).
        let vals = [faixa("L3", "L60"), faixa("M3", "M60"), faixa("N3", "N60")];
        let s = series_salteadas(true, (1, 1, 59, 13), Some(faixa("B3", "B60")), &vals).unwrap();
        assert_eq!((s.categoria, s.colunas), (0, vec![11, 12, 13]));
        // Retângulo contíguo: nada muda.
        let vals = [faixa("C3", "C9"), faixa("D3", "D9")];
        assert!(series_salteadas(true, (1, 1, 8, 3), Some(faixa("B3", "B9")), &vals).is_none());
        // Categoria no meio: o índice dela muda e as séries pulam a coluna dela.
        let vals = [faixa("B3", "B9"), faixa("D3", "D9")];
        let s = series_salteadas(true, (1, 1, 8, 3), Some(faixa("C3", "C9")), &vals).unwrap();
        assert_eq!((s.categoria, s.colunas), (1, vec![1, 3]));
    }

    #[test]
    fn ocultas_contam_so_as_colunas_com_numero() {
        use feathersheet_core::planilha::workbook::Workbook;
        // B categoria, C texto (não vira série), D número pulado, E a série do Excel.
        let mut wb = Workbook::new();
        for (c, t) in [(1, "Cat"), (2, "Desc"), (3, "Pulada"), (4, "Valor")] {
            wb.escrever_celula(0, 0, c, t.into());
        }
        for (c, t) in [(1, "a"), (2, "texto"), (3, "5"), (4, "7")] {
            wb.escrever_celula(0, 1, c, t.into());
        }
        let g = serde_json::json!([{ "type": "column", "range": {"r0":0,"c0":1,"r1":1,"c1":4},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true, "categoryColumnIndex": 0,
            "_featherNative": {"cores": {"0": "#ff0000"}}, COLUNAS_SER: [4] }]);
        wb.aba_mut(0).unwrap().sheet.g = Some(g.to_string());
        ocultar_colunas_puladas(&mut wb);
        let g: serde_json::Value = serde_json::from_str(wb.aba(0).unwrap().sheet.g.as_deref().unwrap()).unwrap();
        // Numéricas: D (0) e E (1); esconde D, a cor da `ser` 0 vai para E.
        assert_eq!(g[0]["style"]["hiddenSeries"], serde_json::json!([0]));
        assert_eq!(g[0]["_featherNative"]["cores"], serde_json::json!({"1": "#ff0000"}));
        assert!(g[0].get(COLUNAS_SER).is_none());
    }

    #[test]
    fn posicao_pela_ancora_com_larguras_proprias() {
        let aba = SheetJson { col_widths: vec![(0, 50)], row_heights: vec![(1, 40)], ..Default::default() };
        let lugar = LugarGrafico {
            aba: 0,
            de: Canto { coluna: 1, dx: 10.0, linha: 2, dy: 5.0 },
            ate: Some(Canto { coluna: 3, dx: 0.0, linha: 10, dy: 0.0 }),
            tamanho: (0.0, 0.0),
            z: 0,
        };
        let p = posicao(&lugar, &aba);
        // x = 50 + 10; y = 28 + 40 + 5; largura = 50+122+122 − 60.
        assert_eq!((p["x"].as_f64(), p["y"].as_f64()), (Some(60.0), Some(73.0)));
        assert_eq!(p["width"].as_f64(), Some(234.0));
        assert_eq!(p["height"].as_f64(), Some(40.0 + 28.0 * 9.0 - 73.0));
    }

    #[test]
    fn estilo_do_grafico_do_excel() {
        let x = parse_xml(r#"<c:chartSpace xmlns:c="c" xmlns:a="a"><c:chart><c:autoTitleDeleted val="1"/>
            <c:plotArea><c:barChart><c:barDir val="col"/>
              <c:ser><c:idx val="0"/><c:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></c:spPr>
                <c:dLbls><c:txPr><a:bodyPr/><a:p><a:pPr><a:defRPr sz="900" b="1"><a:solidFill><a:schemeClr val="lt1"/></a:solidFill></a:defRPr></a:pPr></a:p></c:txPr><c:showVal val="1"/></c:dLbls>
                <c:val><c:numRef><c:f>D!$E$2:$E$10</c:f><c:numCache><c:formatCode>_("R$"* #,##0.00_)</c:formatCode></c:numCache></c:numRef></c:val></c:ser>
            </c:barChart><c:catAx><c:delete val="0"/></c:catAx><c:valAx><c:delete val="1"/><c:majorGridlines/></c:valAx></c:plotArea></c:chart>
            <c:spPr><a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="tx1"/></a:gs><a:gs pos="100000"><a:srgbClr val="2F5597"><a:alpha val="0"/></a:srgbClr></a:gs></a:gsLst><a:lin ang="16200000"/></a:gradFill>
            <a:ln w="6350"><a:solidFill><a:srgbClr val="898989"/></a:solidFill></a:ln></c:spPr></c:chartSpace>"#).unwrap();
        let (n, e) = estilo_excel(&x).unwrap();
        assert_eq!(n["cor"], "#ff0000");
        assert_eq!((n["titulo"].as_bool(), n["legenda"].as_bool(), n["valores"].as_bool()), (Some(false), Some(false), Some(true)));
        assert_eq!(n["formato"], "moeda:2");
        assert_eq!(e["fundoDegrade"], serde_json::json!([[0.0, "#000000"], [1.0, "#2f559700"]]));
        assert_eq!(e["fundoAngulo"], 270.0);
        assert_eq!(e["contorno"], "#898989");
        assert_eq!((e["corRotulo"].as_str(), e["negritoRotulo"].as_bool()), (Some("#ffffff"), Some(true)));
        assert_eq!((e["eixoValores"].as_bool(), e["grade"].as_bool()), (Some(false), Some(true)));
        assert_eq!(formato_do_excel("General"), None);
        let tx = parse_xml(r#"<c:txPr xmlns:c="c" xmlns:a="a"><a:p><a:pPr><a:defRPr sz="900"><a:pattFill prst="pct5"><a:fgClr><a:srgbClr val="000000"/></a:fgClr><a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr></a:pattFill></a:defRPr></a:pPr></a:p></c:txPr>"#).unwrap();
        assert_eq!(texto_do_txpr(Some(&tx)), (Some("#f2f2f2".into()), Some(9.0), false));
        assert_eq!(formato_do_excel("0.0"), Some("decimal:1".into()));
    }

    #[test]
    fn chartex_de_cascata_por_nome_oculto() {
        let x = parse_xml(r#"<cx:chartSpace xmlns:cx="cx" xmlns:a="a"><cx:chartData><cx:data id="0">
            <cx:strDim type="cat"><cx:f>_xlchart.v5.0</cx:f></cx:strDim>
            <cx:numDim type="val"><cx:f>_xlchart.v5.1</cx:f></cx:numDim></cx:data></cx:chartData>
            <cx:chart><cx:title><cx:tx><cx:txData><cx:v>TOTAL</cx:v></cx:txData></cx:tx></cx:title>
            <cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"/></cx:plotAreaRegion></cx:plotArea></cx:chart>
            <cx:fmtOvrs><cx:fmtOvr idx="1"><cx:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></cx:spPr></cx:fmtOvr></cx:fmtOvrs>
            </cx:chartSpace>"#).unwrap();
        let mut nomes = HashMap::new();
        nomes.insert("_xlchart.v5.0".to_string(), (Some("R".to_string()), (3, 1, 8, 1)));
        nomes.insert("_xlchart.v5.1".to_string(), (Some("R".to_string()), (3, 4, 8, 4)));
        let g = ler_chartex(&x, &nomes).unwrap();
        assert_eq!((g.tipo, g.titulo.as_str(), g.aba.as_deref()), ("waterfall", "TOTAL", Some("R")));
        assert_eq!(g.area, (3, 1, 8, 4));
        assert_eq!(g.ocultas, vec![0, 1]);
    }
}
