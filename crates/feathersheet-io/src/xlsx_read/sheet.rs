//! Leitura de uma planilha (`xl/worksheets/sheetN.xml`), relações, strings
//! compartilhadas e nomes definidos. Espelha `readRels`/`readSharedStrings`/
//! `readSheet`/`cellRaw`/`readNames` de `src/io/xlsx.ts:819-930`.

use super::formula::safe_formula;
use super::refs::{parse_cell_ref, shift_formula, MAX_COLS};
use super::styles::{Style, XfStyle};
use super::xml::{parse_xml, parse_xml_linhas, XmlNode};
use feathersheet_core::celulas::values::{CellError, ErrorCode, Value};

// Protege importações pequenas com milhões de células somente formatadas.
const LIMITE_CELULAS_IMPORTACAO: usize = 250_000;
use serde::Serialize;
use std::collections::HashMap;

const PX_TO_PT: f64 = 0.75;
const EPOCH_MS: i64 = -2_209_161_600_000; // Date.UTC(1899, 11, 30)
const DAY_MS: i64 = 86_400_000;

/// Largura gravada no arquivo (caracteres) para pixel: `w * 7` (MDW 7). O
/// `w * 7 + 5` é a conversão da largura exibida pelo Excel, não da gravada;
/// `defaultColWidth="9.140625"` do Excel = 64 px. Inverso de `px_para_largura`.
fn widths_to_px(w: f64) -> i64 {
    (w * 7.0).round() as i64
}

/// `serialToDate` + formatação dd/mm/aaaa [hh:mm:ss] sem `Intl` — igual o
/// motor TypeScript faz hoje (`core/style.ts`, corrigido nesta sessão por
/// causa do custo do `Intl` por célula).
fn format_date_serial(n: f64, incluir_hora: bool, so_hora: bool) -> String {
    let ms = EPOCH_MS + (n * DAY_MS as f64).round() as i64;
    let dias_desde_epoca = ms.div_euclid(DAY_MS);
    let ms_no_dia = ms.rem_euclid(DAY_MS);

    let (ano, mes, dia) = dias_para_data(dias_desde_epoca);
    let h = ms_no_dia / 3_600_000;
    let m = (ms_no_dia % 3_600_000) / 60_000;
    let s = (ms_no_dia % 60_000) / 1000;

    let data = format!("{:02}/{:02}/{:04}", dia, mes, ano);
    let hora = format!("{h:02}:{m:02}:{s:02}");
    if so_hora {
        hora
    } else if incluir_hora {
        format!("{data} {hora}")
    } else {
        data
    }
}

/// Dias desde 1970-01-01 (proléptico gregoriano) -> (ano, mês 1-12, dia).
pub(super) fn dias_para_data(dias_desde_epoca: i64) -> (i64, i64, i64) {
    // Algoritmo de Howard Hinnant (civil_from_days), domínio público.
    let z = dias_desde_epoca + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn round2(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

#[derive(Debug, Default, Serialize)]
pub struct CellSnapshot {
    pub raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
}

#[derive(Debug, Default, Serialize)]
pub struct SheetJson {
    pub name: String,
    pub cells: HashMap<String, CellSnapshot>,
    #[serde(rename = "colWidths")]
    pub col_widths: Vec<(i64, i64)>,
    #[serde(rename = "rowHeights")]
    pub row_heights: Vec<(i64, i64)>,
    pub charts: Vec<serde_json::Value>,
    pub rules: Vec<serde_json::Value>,
    /// Células mescladas, `{ r0, c0, r1, c1 }` — o `mesclas` do `loadJSON`.
    pub mesclas: Vec<serde_json::Value>,
    pub tabelas: Vec<serde_json::Value>,
    pub dinamicas: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub oculta: bool,
    /// `sheetFormatPr/@defaultColWidth`, já em pixels.
    #[serde(skip)]
    pub largura_padrao: Option<i64>,
    /// `sheetFormatPr zeroHeight="1"`: linhas não listadas no arquivo ficam
    /// ocultas; vira "ocultas da linha seguinte à última visível até o fim".
    #[serde(skip)]
    pub linhas_ocultas_desde: Option<u32>,
    /// `<col min=".." max="16384" hidden="1">`: ocultas até a última coluna.
    #[serde(skip)]
    pub colunas_ocultas_desde: Option<u32>,
    /// Estrutura de tópicos: `outlineLevel` e `collapsed` das linhas e colunas.
    #[serde(skip)]
    pub topicos: feathersheet_core::planilha::sheet::topicos::Topicos,
    /// `<sheetProtection sheet="1">`.
    #[serde(skip)]
    pub protecao: Option<feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha>,
    /// Validação de dados (`<dataValidations>` e `x14`), já no formato do motor.
    #[serde(skip)]
    pub validacoes: Vec<feathersheet_core::operacoes::validation::Validacao>,
}

/// Uma `<row>` com algo além das células: índice, `ht`, `hidden`,
/// `customHeight`, `outlineLevel` e `collapsed`.
pub(super) type AlturaLida = (i64, Option<f64>, bool, bool, u8, bool);

/// `outlineLevel` e `collapsed` de uma `<row>` ou `<col>`.
pub(super) fn topico_do_no(no: &XmlNode) -> (u8, bool) {
    let nivel = no.attr("outlineLevel").and_then(|s| s.parse::<u8>().ok()).unwrap_or(0).min(7);
    (nivel, matches!(no.attr("collapsed"), Some("1") | Some("true")))
}

/// `<sheetProtection>`: só vale com `sheet="1"`. Atributo ausente segue o
/// padrão do formato: selecionar, objetos e cenários livres; o resto proibido.
fn ler_protecao(no: &XmlNode) -> Option<feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha> {
    use feathersheet_core::planilha::sheet::protecao::{Permissao, ProtecaoPlanilha, SenhaPlanilha};
    let sim = |v: Option<&str>| matches!(v, Some("1") | Some("true"));
    if !sim(no.attr("sheet")) { return None; }
    let livre_por_padrao = |p: Permissao| matches!(p, Permissao::SelecionarBloqueadas | Permissao::SelecionarDesbloqueadas
        | Permissao::EditarObjetos | Permissao::EditarCenarios);
    let permitidas = Permissao::TODAS.iter()
        .filter(|(p, _, atributo)| match no.attr(atributo) {
            Some(v) => !sim(Some(v)),
            None => livre_por_padrao(*p),
        })
        .map(|(p, ..)| *p).collect();
    // O `password="CBEB"` antigo (16 bits) não é guardado: a aba abre protegida sem senha.
    let senha = (|| Some(SenhaPlanilha {
        algoritmo: no.attr("algorithmName")?.to_string(),
        hash: no.attr("hashValue")?.to_string(),
        sal: no.attr("saltValue")?.to_string(),
        giros: no.attr("spinCount")?.parse().ok()?,
    }))();
    Some(ProtecaoPlanilha { senha, permitidas })
}

pub fn read_rels(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if xml.is_empty() {
        return out;
    }
    if let Ok(root) = parse_xml(xml) {
        for rel in root.children_named("Relationship") {
            if let (Some(id), Some(target)) = (rel.attr("Id"), rel.attr("Target")) {
                out.insert(id.to_string(), target.to_string());
            }
        }
    }
    out
}

pub fn read_shared_strings(xml: &str) -> Vec<String> {
    if xml.is_empty() {
        return Vec::new();
    }
    let root = match parse_xml(xml) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    root.children_named("si")
        .map(|si| {
            if let Some(direct) = si.child("t") {
                direct.text.clone()
            } else {
                si.children_named("r").map(|r| r.child("t").map(|t| t.text.clone()).unwrap_or_default()).collect()
            }
        })
        .collect()
}

pub struct DefinedName {
    pub name: String,
    pub r0: i64,
    pub c0: i64,
    pub r1: i64,
    pub c1: i64,
    /// Aba do intervalo (`inversores!$Q$2:$Q$12`); sem aba, a da fórmula.
    pub sheet: Option<String>,
}

/// `'Minha aba'!A1` → `Minha aba` (aspas dobradas viram uma).
fn aba_do_nome(prefixo: &str) -> Option<String> {
    let p = prefixo.trim();
    let p = match p.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')) {
        Some(dentro) => dentro.replace("''", "'"),
        None => p.to_string(),
    };
    (!p.is_empty()).then_some(p)
}

pub fn read_names(book: &XmlNode) -> Vec<DefinedName> {
    let mut out = Vec::new();
    let Some(defined) = book.child("definedNames") else { return out };
    for node in defined.children_named("definedName") {
        if node.attr("function") == Some("1") {
            continue;
        }
        let Some(name) = node.attr("name") else { continue };
        let text = node.text.trim();
        let (sheet, area) = match text.rfind('!') {
            Some(i) => (aba_do_nome(&text[..i]), &text[i + 1..]),
            None => (None, text),
        };
        let area = area.replace('$', "");
        let mut partes = area.splitn(2, ':');
        let a = partes.next().unwrap_or("");
        let b = partes.next();
        let Some(start) = parse_cell_ref(a) else { continue };
        let end = match b {
            Some(bb) => match parse_cell_ref(bb) {
                Some(e) => e,
                None => continue,
            },
            None => super::refs::CellRef { row: start.row, col: start.col },
        };
        out.push(DefinedName { name: name.to_string(), r0: start.row, c0: start.col, r1: end.row, c1: end.col, sheet });
    }
    out
}

pub(super) struct SharedFormula {
    raw: String,
    row: i64,
    col: i64,
}

/// Uma `<c>` lida: da árvore ([`XmlNode`]) ou do leitor rápido de linhas
/// ([`super::xml::CelulaXml`]). As regras da célula são as mesmas nos dois.
pub(super) trait FonteCelula {
    fn atributo(&self, nome: &str) -> Option<&str>;
    /// Texto da `<f>` (sem aparar), `t` e `si`; `None` sem `<f>`.
    fn formula(&self) -> Option<(&str, Option<&str>, Option<&str>)>;
    /// Texto do `<v>` (sem aparar); `None` sem `<v>`.
    fn valor(&self) -> Option<&str>;
    /// Texto de `<is>`; `None` sem `<is>`.
    fn inline(&self) -> Option<String>;
}

impl FonteCelula for XmlNode {
    fn atributo(&self, nome: &str) -> Option<&str> { self.attr(nome) }
    fn formula(&self) -> Option<(&str, Option<&str>, Option<&str>)> {
        self.child("f").map(|f| (f.text.as_str(), f.attr("t"), f.attr("si")))
    }
    fn valor(&self) -> Option<&str> { self.child("v").map(|v| v.text.as_str()) }
    fn inline(&self) -> Option<String> {
        self.child("is").map(|is| {
            if let Some(direct) = is.child("t") {
                direct.text.clone()
            } else {
                is.children_named("r").map(|r| r.child("t").map(|t| t.text.clone()).unwrap_or_default()).collect()
            }
        })
    }
}

impl FonteCelula for super::xml::CelulaXml {
    fn atributo(&self, nome: &str) -> Option<&str> { self.attr(nome) }
    fn formula(&self) -> Option<(&str, Option<&str>, Option<&str>)> {
        self.f.get().map(|t| (t, self.f_t.get(), self.f_si.get()))
    }
    fn valor(&self) -> Option<&str> { self.v.get() }
    fn inline(&self) -> Option<String> { self.inline.get().map(str::to_owned) }
}

pub(super) fn cell_raw<C: FonteCelula + ?Sized>(c: &C, strings: &[String], xf: Option<&XfStyle>, shared: &mut HashMap<String, SharedFormula>, row: i64, col: i64) -> String {
    if let Some((f_texto, f_t, si)) = c.formula() {
        let text = f_texto.trim();
        if !text.is_empty() {
            let formula = safe_formula(text);
            if let Some(si) = si {
                if f_t == Some("shared") {
                    shared.insert(si.to_string(), SharedFormula { raw: formula.clone(), row, col });
                }
            }
            return formula;
        }
        if let Some(si) = si {
            if let Some(master) = shared.get(si) {
                return shift_formula(&master.raw, row - master.row, col - master.col);
            }
        }
    }

    let tipo = c.atributo("t").unwrap_or("n");
    if tipo == "inlineStr" {
        return c.inline().unwrap_or_default();
    }
    let Some(v) = c.valor() else { return String::new() };
    let text = v.trim();
    match tipo {
        "s" => text.parse::<usize>().ok().and_then(|i| strings.get(i)).cloned().unwrap_or_default(),
        "str" => text.to_string(),
        "b" => if text == "1" { "VERDADEIRO" } else { "FALSO" }.to_string(),
        "e" => text.to_string(),
        _ => numero_da_celula(text, xf),
    }
}

/// O `<v>` de uma célula numérica como o motor recebe (data, hora e
/// porcentagem pelo formato do estilo). Sem estado: roda em qualquer thread.
pub(super) fn numero_da_celula(text: &str, xf: Option<&XfStyle>) -> String {
    let Ok(n) = text.parse::<f64>() else { return text.to_string() };
    match xf.map(|x| x.format) {
        Some("date") => format_date_serial(n, false, false),
        Some("datetime") => format_date_serial(n, true, false),
        Some("time") => format_date_serial(n, false, true),
        Some("percent") => format!("{}%", format_num(round2(n * 100.0))).replace('.', ","),
        _ => format_num(n),
    }
}

/// Texto de uma `<f>` comum (não compartilhada) como o motor recebe.
pub(super) fn formula_da_celula(text: &str) -> String {
    safe_formula(text.trim())
}

/// O resultado que o Excel gravou de uma fórmula (`<c><f>…</f><v>…</v></c>`).
/// `None` em célula sem fórmula ou sem `<v>` (arquivo de outro programa).
pub(super) fn valor_guardado<C: FonteCelula + ?Sized>(c: &C, strings: &[String]) -> Option<Value> {
    c.formula()?;
    let v = c.valor()?;
    let texto = v.trim();
    Some(match c.atributo("t").unwrap_or("n") {
        "str" | "inlineStr" => Value::Text(v.to_owned()),
        "s" => Value::Text(texto.parse::<usize>().ok().and_then(|i| strings.get(i)).cloned()?),
        "b" => Value::Bool(texto == "1"),
        "e" => Value::Error(CellError::new(ErrorCode::from_code(texto)?)),
        _ => Value::Number(texto.parse::<f64>().ok()?),
    })
}

/// `String(n)` do JS: inteiro sem `.0`, senão a representação padrão.
fn format_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Sem as relações da aba (sem hiperlink): o atalho dos testes.
#[cfg(test)]
pub fn read_sheet(name: &str, xml: &str, strings: &[String], xfs: &[XfStyle]) -> SheetJson {
    read_sheet_com_links(name, xml, strings, xfs, &HashMap::new()).unwrap()
}

/// O `read_sheet` com as relações da aba (`xl/worksheets/_rels/sheetN.xml.rels`),
/// que é onde mora o endereço de cada hiperlink. Espelha `readSheet` com
/// `relsDaAba` em `io/xlsx.ts`.
pub fn read_sheet_com_links(
    name: &str,
    xml: &str,
    strings: &[String],
    xfs: &[XfStyle],
    rels_da_aba: &HashMap<String, String>,
) -> Result<SheetJson, String> {
    read_sheet_com_links_limite(name, xml, strings, xfs, rels_da_aba, LIMITE_CELULAS_IMPORTACAO)
}

fn read_sheet_com_links_limite(
    name: &str,
    xml: &str,
    strings: &[String],
    xfs: &[XfStyle],
    rels_da_aba: &HashMap<String, String>,
    limite: usize,
) -> Result<SheetJson, String> {
    let mut sheet = SheetJson { name: name.to_string(), ..Default::default() };
    if xml.is_empty() { return Ok(sheet); }
    // O callback processa e descarta cada linha, sem acumular sheetData
    // como arvore de XML (o maior pico de memoria no XLSX comprimido).
    let mut shared = HashMap::<String, SharedFormula>::new();
    let mut alturas = Vec::<AlturaLida>::new();
    let mut ultima_visivel = -1_i64;
    let mut excedeu = false;
    let root = parse_xml_linhas(xml, &mut |row| {
        if excedeu { return; }
        let Some(row_index) = row.attr("r").and_then(|s| s.parse::<i64>().ok())
            .map(|r| r - 1) else { return };
        let hidden = matches!(row.attr("hidden"), Some("1") | Some("true"));
        if !hidden { ultima_visivel = ultima_visivel.max(row_index); }
        let ht = row.attr("ht").and_then(|s| s.parse::<f64>().ok());
        let propria = matches!(row.attr("customHeight"), Some("1") | Some("true"));
        let (nivel, recolhida) = topico_do_no(row);
        if row_index >= 0 && (hidden || ht.is_some() || nivel > 0 || recolhida) {
            alturas.push((row_index, ht, hidden, propria, nivel, recolhida));
        }
        for celula in row.children_named("c") {
            let Some(at) = parse_cell_ref(celula.attr("r").unwrap_or("")) else { continue };
            let xf_index = celula.attr("s").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let xf = xfs.get(xf_index);
            let raw = cell_raw(celula, strings, xf, &mut shared, at.row, at.col);
            let has_style = xf.map(|x| x.style.has_any()).unwrap_or(false);
            if raw.is_empty() && !has_style { continue; }
            let key = (at.row * MAX_COLS + at.col).to_string();
            if sheet.cells.len() >= limite && !sheet.cells.contains_key(&key) {
                excedeu = true;
                return;
            }
            sheet.cells.insert(key, CellSnapshot {
                raw, style: if has_style { xf.map(|x| x.style.clone()) } else { None },
            });
        }
    })?;
    if excedeu {
        return Err(format!(
            "XLSX excede {} celulas (inclusive celulas so formatadas). Remova              formatacao de linhas/colunas inteiras no Excel antes de importar.",
            limite,
        ));
    }
    Ok(aplicar_metadados(&root, rels_da_aba, alturas, ultima_visivel, sheet, xfs))
}

/// Finaliza os metadados sem exigir que todas as células fiquem num HashMap.
/// O leitor nativo aplica as linhas direto no armazenamento colunar.
pub(super) fn ler_metadados(
    nome: &str,
    root: &XmlNode,
    rels_da_aba: &HashMap<String, String>,
    alturas: Vec<AlturaLida>,
    ultima_visivel: i64,
    xfs: &[XfStyle],
) -> SheetJson {
    aplicar_metadados(
        root, rels_da_aba, alturas, ultima_visivel,
        SheetJson { name: nome.to_string(), ..Default::default() },
        xfs,
    )
}

fn aplicar_metadados(
    root: &XmlNode,
    rels_da_aba: &HashMap<String, String>,
    alturas: Vec<AlturaLida>,
    ultima_visivel: i64,
    mut sheet: SheetJson,
    xfs: &[XfStyle],
) -> SheetJson {
    // Verdadeiro no XML é "1" ou "true": o Excel escreve o primeiro, o
    // LibreOffice o segundo — e aceitar só o "1" deixava a planilha do Calc
    // sem largura nenhuma (bug de 23/09/2026). Espelha `io/xlsx.ts`.
    let verdade = |v: Option<&str>| matches!(v, Some("1") | Some("true"));
    // Linha com altura diferente da padrão vale mesmo sem `customHeight`: é a
    // que o programa ajustou ao texto quebrado.
    let altura_padrao = root
        .child("sheetFormatPr")
        .and_then(|f| f.attr("defaultRowHeight"))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(15.0);

    sheet.protecao = root.child("sheetProtection").and_then(ler_protecao);
    let (validacoes, fora) = super::validacoes::ler(root);
    if fora > 0 {
        feathersheet_core::anotar!("[abrir] {fora} validação(ões) de tipo ainda sem regra no FeatherSheet (tamanho do texto, hora ou personalizada) ficaram de fora");
    }
    sheet.validacoes = validacoes;

    sheet.largura_padrao = root
        .child("sheetFormatPr")
        .and_then(|f| f.attr("defaultColWidth"))
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|w| *w > 0.0)
        .map(widths_to_px);

    // `zeroHeight`: o Excel lista toda linha visível; as demais, até o fim,
    // estão ocultas ("ocultar da linha 30 até o fim").
    if verdade(root.child("sheetFormatPr").and_then(|f| f.attr("zeroHeight"))) {
        sheet.linhas_ocultas_desde = Some((ultima_visivel + 1).clamp(0, i64::from(feathersheet_core::MAX_ROWS) - 1) as u32);
    }

    if let Some(cols) = root.child("cols") {
        // O Excel grava os `<col>` em ordem e sem sobreposição: o que já foi
        // coberto é pulado, e mil cópias de `min=1 max=16383` num arquivo
        // malicioso custam uma volta só, não bilhões de larguras (#53).
        let mut proximo_topico = 0u32;
        let mut proxima_largura = 0i64;
        for col in cols.children_named("col") {
            let (nivel, recolhida) = topico_do_no(col);
            if nivel > 0 || recolhida {
                let min = (col.attr("min").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1).max(1) - 1).max(proximo_topico);
                let max = (col.attr("max").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1).max(1) - 1).min(MAX_COLS as u32 - 1);
                proximo_topico = proximo_topico.max(max.saturating_add(1));
                for c in min..=max {
                    if nivel > 0 { sheet.topicos.colunas.niveis.insert(c, nivel); }
                    if recolhida { sheet.topicos.colunas.recolhidas.insert(c); }
                }
            }
            let min = (col.attr("min").and_then(|s| s.parse::<i64>().ok()).unwrap_or(1) - 1).max(proxima_largura);
            let max = (col.attr("max").and_then(|s| s.parse::<i64>().ok()).unwrap_or(1) - 1).min(MAX_COLS - 1);
            if min > max { continue; }
            proxima_largura = max + 1;
            // Coluna oculta = largura 0 (como a linha oculta). O Excel oculta
            // de J até XFD numa linha só: aqui não há o teto de 1000.
            if verdade(col.attr("hidden")) {
                // Até a última coluna: um número só, sem 16 mil larguras.
                if max == MAX_COLS - 1 && min > 0 {
                    let desde = min as u32;
                    sheet.colunas_ocultas_desde = Some(sheet.colunas_ocultas_desde.map_or(desde, |d| d.min(desde)));
                    continue;
                }
                for c in min.max(0)..=max {
                    sheet.col_widths.push((c, 0));
                }
                continue;
            }
            let Some(width) = col.attr("width").and_then(|s| s.parse::<f64>().ok()) else { continue };
            if !verdade(col.attr("customWidth")) {
                continue;
            }
            let mut c = min;
            while c <= max && c >= 0 && c - min < 1000 {
                sheet.col_widths.push((c, widths_to_px(width)));
                c += 1;
            }
        }
    }

    // Metadados de altura se resolvem apos ler sheetFormatPr, sem reter rows.
    for (row_index, ht, hidden, propria, nivel, recolhida) in alturas {
        if row_index >= 0 && nivel > 0 { sheet.topicos.linhas.niveis.insert(row_index as u32, nivel); }
        if row_index >= 0 && recolhida { sheet.topicos.linhas.recolhidas.insert(row_index as u32); }
        if hidden { sheet.row_heights.push((row_index, 0)); }
        else if let Some(ht) = ht {
            if propria || (ht - altura_padrao).abs() > 0.01 {
                sheet.row_heights.push((row_index, (ht / PX_TO_PT).round() as i64));
            }
        }
    }

    // Hiperlinks: `<hyperlink ref="B2" r:id="rId1"/>`, endereço nas relações
    // da aba. O `ref` pode ser um intervalo (a coluna de links inteira). Só
    // `location` é link para dentro da pasta: ainda não. Espelha `io/xlsx.ts`.
    if let Some(hyperlinks) = root.child("hyperlinks") {
        for h in hyperlinks.children_named("hyperlink") {
            let Some(alvo) = h.attr("r:id").and_then(|id| rels_da_aba.get(id)) else { continue };
            let texto = h.attr("ref").unwrap_or("").replace('$', "");
            let mut partes = texto.split(':');
            let Some(inicio) = partes.next().and_then(parse_cell_ref) else { continue };
            let (r1, c1) = partes.next().and_then(parse_cell_ref).map_or((inicio.row, inicio.col), |f| (f.row, f.col));
            if r1 < inicio.row || c1 < inicio.col { continue; }
            // Hiperlink A1:XFD1048576: só percorrer as células importadas.
            // Evita loops de bilhões de coordenadas temporárias vazias.
            for (chave, cel) in sheet.cells.iter_mut() {
                let Ok(indice) = chave.parse::<i64>() else { continue };
                let (r, c) = (indice / MAX_COLS, indice % MAX_COLS);
                if r < inicio.row || r > r1 || c < inicio.col || c > c1 || cel.raw.is_empty() {
                    continue;
                }
                cel.style.get_or_insert_with(Style::default).link = Some(alvo.clone());
            }
        }
    }

    // Células mescladas: `<mergeCell ref="A1:F1"/>`. Espelha `io/xlsx.ts`.
    if let Some(merges) = root.child("mergeCells") {
        for m in merges.children_named("mergeCell") {
            let texto = m.attr("ref").unwrap_or("").replace('$', "");
            let mut partes = texto.split(':');
            let (Some(a), Some(b)) = (partes.next().and_then(parse_cell_ref), partes.next().and_then(parse_cell_ref)) else {
                continue;
            };
            sheet.mesclas.push(serde_json::json!({ "r0": a.row, "c0": a.col, "r1": b.row, "c1": b.col }));
        }
    }

    // Formatação condicional: `<conditionalFormatting>` e o x14 do `<extLst>`.
    let condicional = super::condicional::ler(root, xfs);
    if !condicional.fora.is_empty() {
        feathersheet_core::anotar!(
            "[abrir] {} de {} regra(s) de formatação condicional ficaram de fora: {}",
            condicional.fora.len(), condicional.total, condicional.fora.join("; ")
        );
    }
    sheet.rules = condicional.regras;

    sheet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xlsx_read::styles::read_styles;

    #[test]
    fn pequenas_planilhas_com_milhares_de_linhas_vazias_nao_geram_celulas() {
        // O XLSX compactado pode conter milhares de rows sem dados.
        let mut xml = String::from("<worksheet><sheetData>");
        for i in 1..=8_000 {
            xml.push_str(&format!("<row r=\"{i}\"/>"));
        }
        xml.push_str("<row r=\"8001\"><c r=\"C8001\" t=\"str\"><v>ok</v></c></row>");
        xml.push_str("</sheetData></worksheet>");
        let sheet = read_sheet("Dados", &xml, &[], &[]);
        assert_eq!(sheet.cells.len(), 1);
        assert_eq!(sheet.cells.get(&(8_000i64 * MAX_COLS + 2).to_string()).unwrap().raw, "ok");
        assert!(sheet.row_heights.is_empty());
    }

    #[test]
    fn hiperlink_da_planilha_inteira_so_altera_celulas_existentes() {
        // Antes: loop por 1.048.576 x 16.384 posicoes (potencial travamento).
        let xml = r#"<worksheet xmlns:r="urn:rel"><sheetData><row r="1"><c r="A1" t="str"><v>link</v></c></row><row r="2"><c r="D2" t="str"><v>texto</v></c></row></sheetData><hyperlinks><hyperlink ref="A1:XFD1048576" r:id="rId1"/></hyperlinks></worksheet>"#;
        let rels = HashMap::from([("rId1".to_string(), "https://exemplo.org".to_string())]);
        let sheet = read_sheet_com_links("Links", xml, &[], &[], &rels).unwrap();
        assert_eq!(sheet.cells.len(), 2);
        assert_eq!(sheet.cells["0"].style.as_ref().unwrap().link.as_deref(), Some("https://exemplo.org"));
        assert_eq!(sheet.cells[&(MAX_COLS + 3).to_string()].style.as_ref().unwrap().link.as_deref(), Some("https://exemplo.org"));
    }

    #[test]
    fn limite_de_celulas_formatadas_rejeita_sem_abrir_aba_parcial() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>A</v></c><c r="B1" t="str"><v>B</v></c><c r="C1" t="str"><v>C</v></c></row></sheetData></worksheet>"#;
        let erro = read_sheet_com_links_limite("Dados", xml, &[], &[], &HashMap::new(), 2)
            .err().expect("deve rejeitar dados acima do limite");
        assert!(erro.contains("2 celulas"), "{erro}");
        let todas = read_sheet_com_links_limite("Dados", xml, &[], &[], &HashMap::new(), 3).unwrap();
        assert_eq!(todas.cells.len(), 3);
    }

    #[test]
    fn celula_simples_com_string_compartilhada() {
        let strings = vec!["Olá".to_string()];
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c></row></sheetData></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &strings, &[]);
        assert_eq!(sheet.cells.get("0").unwrap().raw, "Olá");
    }

    #[test]
    fn data_guardada_como_serial_volta_formatada() {
        let styles_xml = r#"<styleSheet><cellXfs><xf numFmtId="14"/></cellXfs></styleSheet>"#;
        let xfs = read_styles(styles_xml);
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" s="0"><v>46284</v></c></row></sheetData></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &[], &xfs);
        assert_eq!(sheet.cells.get("0").unwrap().raw, "19/09/2026");
    }

    #[test]
    fn formula_compartilhada_desloca() {
        let xml = r#"<worksheet><sheetData>
          <row r="1"><c r="B1"><f t="shared" si="0">A1*10</f><v>10</v></c></row>
          <row r="2"><c r="B2"><f t="shared" si="0"/><v>20</v></c></row>
          <row r="3"><c r="B3"><f t="shared" si="0"/><v>30</v></c></row>
        </sheetData></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &[], &[]);
        let key_b2 = 1i64 * MAX_COLS + 1;
        let key_b3 = 2i64 * MAX_COLS + 1;
        assert_eq!(sheet.cells.get(&key_b2.to_string()).unwrap().raw, "=A2*10");
        assert_eq!(sheet.cells.get(&key_b3.to_string()).unwrap().raw, "=A3*10");
    }

    #[test]
    fn texto_solto_inlinestr() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>solto</t></is></c></row></sheetData></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &[], &[]);
        assert_eq!(sheet.cells.get("0").unwrap().raw, "solto");
    }
    #[test]
    fn larguras_e_alturas_do_libreoffice() {
        let xfs = read_styles("");
        let xml = r#"<worksheet><sheetFormatPr defaultRowHeight="15"/><cols><col min="1" max="1" width="40" customWidth="true"/><col min="4" max="5" width="14" customWidth="true"/><col min="7" max="7" width="30" customWidth="false"/></cols><sheetData><row r="1" ht="21.75" customHeight="true"/><row r="2" ht="15" customHeight="false"/><row r="3" ht="23.85" customHeight="false"/><row r="4" ht="15" hidden="true"/></sheetData></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &[], &xfs);
        assert_eq!(sheet.col_widths, vec![(0, 280), (3, 98), (4, 98)]);
        assert_eq!(sheet.row_heights, vec![(0, 29), (2, 32), (3, 0)]);
    }
    #[test]
    fn celulas_mescladas() {
        let xfs = read_styles("");
        let xml = r#"<worksheet><sheetData/><mergeCells count="2"><mergeCell ref="A1:F1"/><mergeCell ref="$A$16:$D$17"/><mergeCell ref="B2"/></mergeCells></worksheet>"#;
        let sheet = read_sheet("Plan1", xml, &[], &xfs);
        assert_eq!(
            sheet.mesclas,
            vec![
                serde_json::json!({ "r0": 0, "c0": 0, "r1": 0, "c1": 5 }),
                serde_json::json!({ "r0": 15, "c0": 0, "r1": 16, "c1": 3 }),
            ]
        );
    }
}

#[cfg(test)]
mod testes_nomes {
    use super::*;

    #[test]
    fn nome_guarda_a_aba_do_intervalo() {
        let book = super::super::xml::parse_xml(r#"<workbook><definedNames>
            <definedName name="Inversores">inversores!$Q$2:$Q$12</definedName>
            <definedName name="Eixo">'2. Eixo - Ações'!$C$6</definedName>
            <definedName name="Velho">#REF!</definedName>
            <definedName name="Solto">$B$3</definedName>
        </definedNames></workbook>"#).unwrap();
        let n = read_names(&book);
        assert_eq!(n.len(), 3);
        assert_eq!((n[0].r0, n[0].c0, n[0].r1, n[0].sheet.as_deref()), (1, 16, 11, Some("inversores")));
        assert_eq!(n[1].sheet.as_deref(), Some("2. Eixo - Ações"));
        assert_eq!(n[2].sheet, None);
    }
}
