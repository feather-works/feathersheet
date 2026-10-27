//! Exportar a pasta para `.xlsx` e uma aba para CSV/TSV, lendo direto da
//! `Workbook` do Rust.
//!
//! Tradução do `exportXlsx` do `io/xlsx.ts` (e do laço de CSV do
//! `App.exportar`). Lá a pasta do Rust era lida pelo `toJSON`: cada célula
//! atravessava a ponte como objeto, com o texto e o estilo, só para ser
//! escrita em XML e voltar ao disco — o contrário da regra 13 do `CLAUDE.md`.
//!
//! **Portar é traduzir, não melhorar.** O arquivo sai igual ao do
//! TypeScript, com os mesmos limites de lá:
//!
//! - só o estilo **direto** da célula vai (o de coluna, linha e aba não);
//! - o link sai do estilo e vai para `<hyperlinks>`, nas relações da aba;
//! - só a primeira tabela da aba vira `<autoFilter>` (com cor, 10 primeiros,
//!   período de data, lista e a ordenação: ver `xlsx_write_autofiltro`);
//! - todo nome definido aponta para a **primeira** aba (o nome guarda só o
//!   retângulo, não a aba);
//! - gráficos e painel congelado não vão (a
//!   validação de dados vai: ver [`validacoes_xml`]).
//!   A formatação condicional vai: ver `xlsx_write_condicional`.
//! - a tabela dinâmica vai como definição de verdade (cache + tabela, o
//!   Excel atualiza ao abrir), além do resultado nas células: ver
//!   `xlsx_write_dinamica`.
//!
//! Duas diferenças, as duas para não perder o arquivo inteiro por uma
//! célula: fórmula que o tokenizador recusa vai como está (no TypeScript o
//! `tokenize` lançava e a exportação toda falhava), e dois estilos iguais com
//! campos em ordem diferente viram um `xf` só (lá a chave era o
//! `JSON.stringify`, que depende da ordem).
//!
//! O trabalho é dividido em dois: [`montar_partes`] lê a pasta e escreve os
//! XML (precisa dela, e portanto da ponte); [`crate::zip_write::zipar`]
//! comprime, e pode rodar depois de soltá-la.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::formula::formula_pattern::{expand_pattern, pattern_of, pedacos, Pedaco};
use feathersheet_core::formula::formula_text::quote_sheet;
use feathersheet_core::funcoes::functions::canonical;
use feathersheet_core::celulas::input::parse_literal;
use feathersheet_core::celulas::number::to_js_string;
use feathersheet_core::celulas::refs::{cell_name, col_name, key, key_col, key_row, CellKey};
use feathersheet_core::planilha::data_table::Cru;
use feathersheet_core::planilha::sheet::edicao::{formato_do_literal, implied_format};
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::celulas::style::{format_number, NumberFormat};
use feathersheet_core::formula::tokenizer::{tokenize, TokenType};
use feathersheet_core::celulas::values::{to_text, Value};
use feathersheet_core::planilha::workbook::Workbook;

use crate::zip_write::zipar;

// ============================================================== XML

/// `escapeXml` do `io/xlsx.ts`, com o mesmo descarte de caractere de
/// controle (inválido em XML 1.0).
pub fn escape_xml(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    escrever_escapado(&mut out, texto);
    out
}

fn escrever_escapado(out: &mut String, texto: &str) {
    for ch in texto.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c < ' ' && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
}

/// A referência A1 escrita direto em `out`, sem montar `String` — é o
/// mesmo que o [`cell_name`], uma vez por célula de milhões.
fn escrever_ref(out: &mut String, row: u32, col: u32) {
    escrever_coluna(out, col);
    let _ = write!(out, "{}", row + 1);
}

/// A letra da coluna (`col_name`) escrita direto em `out`.
fn escrever_coluna(out: &mut String, col: u32) {
    let mut letras = [0u8; 8];
    let mut i = letras.len();
    let mut c = col + 1;
    while c > 0 {
        c -= 1;
        i -= 1;
        letras[i] = b'A' + (c % 26) as u8;
        c /= 26;
    }
    out.push_str(std::str::from_utf8(&letras[i..]).expect("só letras ASCII"));
}

// ============================================================== fórmulas

/// `formulaToExcel`: `=SOMA(A1:A3;2,5)` -> `SUM(A1:A3,2.5)`, sem o `=`.
///
/// Dentro de `{}` a pontuação já é a mesma dos dois lados (`;` muda de
/// linha), e a mesma linha é `\` aqui e `,` lá.
pub fn formula_to_excel(raw: &str) -> String {
    let src = raw.strip_prefix('=').unwrap_or(raw);
    let Ok(tokens) = tokenize(src) else {
        // Ver a nota de topo: o TypeScript derrubava a exportação inteira.
        return src.to_string();
    };
    // A posição do token é em caracteres, não em bytes.
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len() + 8);
    let mut chaves = 0i32;

    let mut params = crate::xlsx_write_futuras::Parametros::default();
    let mut ultimo_ident: Option<String> = None;
    for (ti, t) in tokens.iter().enumerate() {
        let seguinte = tokens.get(ti + 1).map(|x| x.kind);
        let anterior_ident = ultimo_ident.take();
        // O tokenizador engole o espaço em branco; sem devolvê-lo,
        // `=SOMA(A1; B1)` voltaria do Excel grudado.
        let pos = t.pos.min(chars.len());
        let mut folga = 0;
        while folga < pos && chars[pos - 1 - folga].is_whitespace() {
            folga += 1;
        }
        out.extend(&chars[pos - folga..pos]);

        match t.kind {
            TokenType::Num => {
                let n = t.value.or_else(|| t.text.replacen(',', ".", 1).parse::<f64>().ok());
                out.push_str(&n.map(to_js_string).unwrap_or_else(|| "NaN".to_string()));
            }
            TokenType::Str => {
                out.push('"');
                out.push_str(&t.text.replace('"', "\"\""));
                out.push('"');
            }
            TokenType::Ident => {
                let nome = canonical(&t.text).or_else(|| fora_do_motor(&t.text)).unwrap_or(&t.text);
                if seguinte == Some(TokenType::LParen) {
                    FUNCOES_USADAS.with(|f| if let Some(lista) = f.borrow_mut().as_mut() {
                        lista.insert(nome.to_uppercase());
                    });
                    if let Some(p) = crate::xlsx_write_futuras::prefixo(nome) {
                        out.push_str(p);
                    }
                    ultimo_ident = Some(nome.to_string());
                    out.push_str(nome);
                } else {
                    if params.declara(&t.text, seguinte == Some(TokenType::Comma)) || params.conhecido(&t.text) {
                        out.push_str("_xlpm.");
                    }
                    out.push_str(nome);
                }
            }
            TokenType::LParen => {
                params.abre(anterior_ident.as_deref());
                out.push_str(&t.text);
            }
            TokenType::RParen => {
                params.fecha();
                out.push_str(&t.text);
            }
            TokenType::Ref => out.push_str(&ref_to_excel(&t.text)),
            TokenType::Comma => {
                params.virgula();
                out.push(if chaves == 0 { ',' } else if t.text == ";" { ';' } else { ',' });
            }
            TokenType::LBrace => {
                chaves += 1;
                out.push('{');
            }
            TokenType::RBrace => {
                chaves -= 1;
                out.push('}');
            }
            TokenType::Eof => {}
            _ => out.push_str(&t.text),
        }
    }
    out
}

/// Funções que o motor não tem, mas que aparecem em fórmula guardada fora da
/// célula: o `INDIRETO` da lista dependente (`Regra::Formula`) é resolvido à
/// parte pelo `planilha::lista_validacao` e não está no `canonical`.
fn fora_do_motor(nome: &str) -> Option<&'static str> {
    match nome.to_uppercase().as_str() {
        "INDIRETO" => Some("INDIRECT"),
        _ => None,
    }
}

/// O tokenizador tira as aspas do nome da aba; aqui elas voltam se precisam.
fn ref_to_excel(texto: &str) -> String {
    match texto.rfind('!') {
        Some(i) => {
            let aba = &texto[..i];
            // Outro arquivo: o Excel quer `[N]Aba`, com N o livro em `externalReferences`.
            let externa = feathersheet_core::planilha::vinculo::separar(aba)
                .and_then(|(arquivo, nome)| crate::xlsx_write_vinculos::numero(&arquivo).map(|n| format!("[{n}]{nome}")));
            format!("{}!{}", quote_sheet(externa.as_deref().unwrap_or(aba)), &texto[i + 1..])
        }
        None => texto.to_string(),
    }
}

// ============================================================== estilo

/// O campo de estilo como o JavaScript o veria num `if (s.campo)`.
fn verdadeiro(c: Option<&Campo>) -> bool {
    match c {
        Some(Campo::Logico(b)) => *b,
        Some(Campo::Numero(n)) => *n != 0.0 && !n.is_nan(),
        Some(Campo::Texto(t)) => !t.is_empty(),
        Some(Campo::Objeto(_)) => true,
        Some(Campo::Nulo) | None => false,
    }
}

fn texto(e: &BTreeMap<String, Campo>, k: &str) -> Option<String> {
    match e.get(k) {
        Some(Campo::Texto(t)) => Some(t.clone()),
        _ => None,
    }
}

fn numero(e: &BTreeMap<String, Campo>, k: &str) -> Option<f64> {
    match e.get(k) {
        Some(Campo::Numero(n)) => Some(*n),
        _ => None,
    }
}

/// `Math.round(n * 100) / 100`, com o arredondamento do JavaScript (metade
/// para cima, rumo a +∞).
fn arredondar(n: f64) -> f64 {
    (n * 100.0 + 0.5).floor() / 100.0
}

fn codigo_do_formato(f: &str) -> Option<&'static str> {
    Some(match f {
        "number" => "#,##0.00",
        "integer" => "#,##0",
        "currency" => "\"R$\" #,##0.00",
        "scientific" => "0.00E+00",
        "accounting" => "_(\"R$\"* #,##0.00_);_(\"R$\"* \\(#,##0.00\\);_(\"R$\"* \"-\"??_)",
        "duration" => "[h]:mm:ss",
        "percent" => "0.00%",
        "date" => "dd/mm/yyyy",
        "datetime" => "dd/mm/yyyy hh:mm:ss",
        "time" => "hh:mm:ss",
        "text" => "@",
        _ => return None,
    })
}

fn zeros(casas: f64) -> String {
    "0".repeat(casas.max(0.0) as usize)
}

/// `formatCodeFor`: o código do Excel com as casas decimais que a célula pede.
fn codigo_para(estilo: &EstiloParcial, implicito: Option<&str>) -> Option<String> {
    // Formato personalizado: volta ao arquivo como veio, só com os nomes de
    // cor em inglês (`[Vermelho]` → `[Red]`), que é o que o Excel lê.
    if let Some(Campo::Texto(codigo)) = estilo.get("numFmt") {
        if !codigo.is_empty() {
            return Some(feathersheet_core::celulas::formato_personalizado::para_xlsx(codigo));
        }
    }
    let formato = match estilo.get("format") {
        Some(Campo::Texto(f)) => Some(f.as_str()),
        _ => implicito,
    };
    let casas = numero(estilo, "decimals");
    let Some(formato) = formato.filter(|f| *f != "auto") else {
        return casas.map(|d| if d <= 0.0 { "#,##0".to_string() } else { format!("#,##0.{}", zeros(d)) });
    };
    let base = codigo_do_formato(formato);
    let Some(d) = casas else { return base.map(str::to_string) };
    Some(match formato {
        "number" => if d == 0.0 { "#,##0".to_string() } else { format!("#,##0.{}", zeros(d)) },
        "percent" => if d == 0.0 { "0%".to_string() } else { format!("0.{}%", zeros(d)) },
        "currency" => if d == 0.0 { "\"R$\" #,##0".to_string() } else { format!("\"R$\" #,##0.{}", zeros(d)) },
        _ => return base.map(str::to_string),
    })
}

/// `'#ff8800'` -> `'FFFF8800'`.
fn para_argb(css: Option<String>) -> Option<String> {
    let css = css?;
    let hex = css.trim().replacen('#', "", 1);
    let so_hex = !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
    if !so_hex {
        return None;
    }
    match hex.len() {
        6 => Some(format!("FF{}", hex.to_uppercase())),
        3 => Some(format!("FF{}", hex.chars().flat_map(|c| [c, c]).collect::<String>().to_uppercase())),
        8 => Some(hex.to_uppercase()),
        _ => None,
    }
}

/// `'#ff8800'` -> `'FFFF8800'`, para os outros módulos de gravação.
pub(crate) fn para_argb_pub(css: &str) -> Option<String> {
    para_argb(Some(css.to_string()))
}

struct Slot {
    estilo: EstiloParcial,
    num_fmt: Option<String>,
}

/// Junta os estilos iguais numa tabela só. O 0 é sempre o estilo vazio.
struct TabelaDeEstilos {
    indice: HashMap<String, usize>,
    slots: Vec<Slot>,
    /// `<dxf>` da formatação condicional, sem repetir.
    dxfs: Vec<String>,
}

impl TabelaDeEstilos {
    fn nova() -> Self {
        let mut t = TabelaDeEstilos { indice: HashMap::new(), slots: Vec::new(), dxfs: Vec::new() };
        t.indice.insert(format!("{:?}|", EstiloParcial::new()), 0);
        t.slots.push(Slot { estilo: EstiloParcial::new(), num_fmt: None });
        t
    }

    /// O `dxfId` do estilo de uma regra condicional (`None` se não pinta nada).
    fn dxf(&mut self, estilo: &feathersheet_core::operacoes::rules::CondStyle) -> Option<usize> {
        let xml = crate::xlsx_write_condicional::dxf_xml(estilo)?;
        if let Some(i) = self.dxfs.iter().position(|d| *d == xml) {
            return Some(i);
        }
        self.dxfs.push(xml);
        Some(self.dxfs.len() - 1)
    }

    fn add(&mut self, estilo: EstiloParcial, implicito: Option<&str>) -> usize {
        let num_fmt = codigo_para(&estilo, implicito);
        let chave = format!("{:?}|{}", estilo, num_fmt.as_deref().unwrap_or(""));
        if let Some(&i) = self.indice.get(&chave) {
            return i;
        }
        self.slots.push(Slot { estilo, num_fmt });
        let i = self.slots.len() - 1;
        self.indice.insert(chave, i);
        i
    }
}

fn internar(lista: &mut Vec<String>, visto: &mut HashMap<String, usize>, xml: String) -> usize {
    if let Some(&i) = visto.get(&xml) {
        return i;
    }
    lista.push(xml.clone());
    visto.insert(xml, lista.len() - 1);
    lista.len() - 1
}

fn borda_xml(b: &BTreeMap<String, Campo>, estilo_linha: Option<&str>) -> String {
    let cor = para_argb(texto(b, "color"));
    // `borderLineStyle` do estilo (Formatar células, Desenhar borda).
    let traco = match estilo_linha {
        Some("Média") => "medium",
        Some("Grossa") => "thick",
        Some("Tracejada") => "dashed",
        _ => "thin",
    };
    let lado = |nome: &str| {
        if verdadeiro(b.get(nome)) {
            match &cor {
                Some(c) => format!("<{nome} style=\"{traco}\"><color rgb=\"{c}\"/></{nome}>"),
                None => format!("<{nome} style=\"{traco}\"></{nome}>"),
            }
        } else {
            format!("<{nome}/>")
        }
    };
    format!("<border>{}{}{}{}<diagonal/></border>", lado("left"), lado("right"), lado("top"), lado("bottom"))
}

/// `stylesXml`: o `xl/styles.xml` a partir da tabela de estilos.
fn estilos_xml(tabela: &TabelaDeEstilos) -> String {
    let mut num_fmts: Vec<String> = Vec::new();
    let mut num_fmt_id: HashMap<String, usize> = HashMap::new();
    let (mut fontes, mut fonte_id) = (Vec::new(), HashMap::new());
    let mut fills = vec![
        "<fill><patternFill patternType=\"none\"/></fill>".to_string(),
        "<fill><patternFill patternType=\"gray125\"/></fill>".to_string(),
    ];
    let mut fill_id = HashMap::new();
    let mut bordas = vec!["<border><left/><right/><top/><bottom/><diagonal/></border>".to_string()];
    let mut borda_id = HashMap::new();
    let mut xfs: Vec<String> = Vec::new();

    for slot in &tabela.slots {
        let s = &slot.estilo;

        let mut partes = String::new();
        for (campo, tag) in [("bold", "<b/>"), ("italic", "<i/>"), ("underline", "<u/>"), ("strike", "<strike/>")] {
            if verdadeiro(s.get(campo)) {
                // Sublinhado duplo: `<u val="double"/>`, como o Excel grava.
                let duplo = campo == "underline" && verdadeiro(s.get("doubleUnderline"));
                partes.push_str(if duplo { "<u val=\"double\"/>" } else { tag });
            }
        }
        let tamanho = match numero(s, "fontSize") {
            Some(n) if n != 0.0 && !n.is_nan() => to_js_string(arredondar(n * 0.75)),
            _ => "11".to_string(),
        };
        partes.push_str(&format!("<sz val=\"{tamanho}\"/>"));
        if let Some(c) = para_argb(texto(s, "color")) {
            partes.push_str(&format!("<color rgb=\"{c}\"/>"));
        }
        let familia = texto(s, "fontFamily").unwrap_or_else(|| "Calibri".to_string());
        partes.push_str(&format!("<name val=\"{}\"/>", escape_xml(&familia)));
        let fonte = internar(&mut fontes, &mut fonte_id, format!("<font>{partes}</font>"));

        // Preenchimento: 0 e 1 são reservados pelo formato.
        let fill = match para_argb(texto(s, "bg")) {
            Some(bg) => {
                let xml = format!(
                    "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{bg}\"/><bgColor indexed=\"64\"/></patternFill></fill>"
                );
                match fill_id.get(&xml) {
                    Some(&i) => i,
                    None => {
                        fills.push(xml.clone());
                        fill_id.insert(xml, fills.len() - 1);
                        fills.len() - 1
                    }
                }
            }
            None => 0,
        };

        let borda = match s.get("borders") {
            Some(Campo::Objeto(b)) => {
                let xml = borda_xml(b, texto(s, "borderLineStyle").as_deref());
                match borda_id.get(&xml) {
                    Some(&i) => i,
                    None => {
                        bordas.push(xml.clone());
                        borda_id.insert(xml, bordas.len() - 1);
                        bordas.len() - 1
                    }
                }
            }
            _ => 0,
        };

        let mut fmt_id = 0;
        if let Some(codigo) = &slot.num_fmt {
            fmt_id = match num_fmt_id.get(codigo) {
                Some(&i) => i,
                None => {
                    let id = 164 + num_fmts.len();
                    num_fmts.push(format!("<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>", escape_xml(codigo)));
                    num_fmt_id.insert(codigo.clone(), id);
                    id
                }
            };
        }

        let mut alinhar: Vec<String> = Vec::new();
        let recuo = numero(s, "indent").filter(|n| *n >= 1.0 && !n.is_nan()).map(|n| n.round().min(15.0) as u32);
        if verdadeiro(s.get("align")) {
            alinhar.push(format!("horizontal=\"{}\"", texto(s, "align").unwrap_or_default()));
        } else if recuo.is_some() {
            // O Excel só aplica recuo a alinhamento à esquerda/direita/distribuído.
            alinhar.push("horizontal=\"left\"".to_string());
        }
        if verdadeiro(s.get("vAlign")) {
            let v = texto(s, "vAlign").unwrap_or_default();
            alinhar.push(format!("vertical=\"{}\"", if v == "middle" { "center" } else { &v }));
        }
        if verdadeiro(s.get("wrap")) {
            alinhar.push("wrapText=\"1\"".to_string());
        }
        if let Some(n) = recuo { alinhar.push(format!("indent=\"{n}\"")); }
        if let Some(r) = numero(s, "rotation").filter(|r| *r != 0.0 && !r.is_nan()) {
            alinhar.push(format!("textRotation=\"{}\"", to_js_string(if r < 0.0 { 90.0 - r } else { r })));
        }

        // Proteção da célula (vale com a aba protegida): só o que foge do padrão.
        let mut proteger: Vec<&str> = Vec::new();
        if matches!(s.get("locked"), Some(Campo::Logico(false))) { proteger.push("locked=\"0\""); }
        if matches!(s.get("hidden"), Some(Campo::Logico(true))) { proteger.push("hidden=\"1\""); }

        let bandeiras: Vec<&str> = [
            (fmt_id != 0, "applyNumberFormat=\"1\""),
            (fonte != 0, "applyFont=\"1\""),
            (fill != 0, "applyFill=\"1\""),
            (borda != 0, "applyBorder=\"1\""),
            (!alinhar.is_empty(), "applyAlignment=\"1\""),
            (!proteger.is_empty(), "applyProtection=\"1\""),
        ]
        .into_iter()
        .filter_map(|(sim, b)| sim.then_some(b))
        .collect();
        let bandeiras = if bandeiras.is_empty() { String::new() } else { format!(" {}", bandeiras.join(" ")) };
        let mut filhos = String::new();
        if !alinhar.is_empty() { filhos.push_str(&format!("<alignment {}/>", alinhar.join(" "))); }
        if !proteger.is_empty() { filhos.push_str(&format!("<protection {}/>", proteger.join(" "))); }
        let corpo = if filhos.is_empty() { "/>".to_string() } else { format!(">{filhos}</xf>") };
        xfs.push(format!(
            "<xf numFmtId=\"{fmt_id}\" fontId=\"{fonte}\" fillId=\"{fill}\" borderId=\"{borda}\" xfId=\"0\"{bandeiras}{corpo}"
        ));
    }

    let num_fmts = if num_fmts.is_empty() {
        String::new()
    } else {
        format!("<numFmts count=\"{}\">{}</numFmts>", num_fmts.len(), num_fmts.concat())
    };
    let dxfs = if tabela.dxfs.is_empty() {
        String::new()
    } else {
        format!("<dxfs count=\"{}\">{}</dxfs>", tabela.dxfs.len(), tabela.dxfs.concat())
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"{NS}\">{num_fmts}<fonts count=\"{}\">{}</fonts><fills count=\"{}\">{}</fills><borders count=\"{}\">{}</borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"{}\">{}</cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>{dxfs}</styleSheet>",
        fontes.len(),
        fontes.concat(),
        fills.len(),
        fills.concat(),
        bordas.len(),
        bordas.concat(),
        xfs.len(),
        xfs.concat(),
    )
}

// ============================================================== textos

/// Cada texto uma vez só; é o que faz o arquivo não inchar.
#[derive(Default)]
struct TextosCompartilhados {
    indice: HashMap<String, usize>,
    lista: Vec<String>,
}

impl TextosCompartilhados {
    fn add(&mut self, t: &str) -> usize {
        if let Some(&i) = self.indice.get(t) {
            return i;
        }
        self.lista.push(t.to_string());
        self.indice.insert(t.to_string(), self.lista.len() - 1);
        self.lista.len() - 1
    }

    fn xml(&self) -> String {
        let mut itens = String::new();
        for s in &self.lista {
            itens.push_str("<si><t xml:space=\"preserve\">");
            escrever_escapado(&mut itens, s);
            itens.push_str("</t></si>");
        }
        let n = self.lista.len();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"{NS}\" count=\"{n}\" uniqueCount=\"{n}\">{itens}</sst>"
        )
    }
}

// ============================================================== exportar

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
/// Última coluna (XFD) e última linha do Excel, contadas a partir de 1.
const MAX_COLS_XLSX: u32 = 16_384;
const MAX_ROWS_XLSX: u32 = 1_048_576;
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// O Excel mede em ponto e em "caracteres"; aqui é pixel (96 dpi), e a
/// largura de coluna gravada no arquivo é `pixels / 7` (MDW 7 da Calibri 11, já
/// com o recuo de 5 px dentro): a coluna padrão do Excel, 64 px, é
/// `defaultColWidth="9.140625"`. O `(px - 5) / 7` é a largura *exibida* na
/// interface do Excel, não a gravada. O leitor (`widths_to_px`) faz o inverso.
const PX_PARA_PT: f64 = 0.75;

fn px_para_largura(px: u32) -> f64 {
    (px as f64 / 7.0).max(0.0)
}

/// Todas as células da aba, nas quatro moradas, em ordem de linha e coluna
/// (a chave é `linha * MAX_COLS + coluna`). Só a chave: o texto e o estilo
/// são lidos um de cada vez na hora de escrever, em vez de uma cópia da aba
/// inteira na memória.
fn chaves_ocupadas(sheet: &Sheet) -> Vec<CellKey> {
    let mut chaves: Vec<CellKey> = Vec::with_capacity(sheet.cells.len() + sheet.styles.len());
    chaves.extend(sheet.cells.keys().copied());
    chaves.extend(sheet.styles.keys());
    sheet.data.for_each(&sheet.pool, &mut |row, col, _| chaves.push(key(row, col)));
    sheet.formulas.for_each_in_range(0, u32::MAX, 0, u32::MAX, |row, col| chaves.push(key(row, col)));
    chaves.sort_unstable();
    chaves.dedup();
    chaves
}

/// Um retângulo `{r0, c0, r1, c1}` do JSON opaco das tabelas e mescladas.
fn retangulo(v: &serde_json::Value) -> Option<(u32, u32, u32, u32)> {
    let n = |k: &str| v.get(k).and_then(serde_json::Value::as_f64).map(|x| x.max(0.0) as u32);
    Some((n("r0")?, n("c0")?, n("r1")?, n("c1")?))
}

fn lista_json(opaco: &Option<String>) -> Vec<serde_json::Value> {
    opaco
        .as_deref()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

/// Um pedaço do padrão em inglês, pronto para o XML: o texto já escapado,
/// e a referência com o prefixo da aba já citado e escapado.
enum PedacoXml {
    Texto(String),
    Ref { prefixo: String, col_lock: bool, row_lock: bool, col: i32, row: i32 },
}

/// A tradução de um padrão de fórmula, feita uma vez para a coluna inteira.
struct Padrao {
    /// O padrão já em inglês e em pedaços; `None` quando a conferência
    /// falhou, e aí cada célula é traduzida sozinha.
    ingles: Option<Vec<PedacoXml>>,
    /// Quantas células já conferiram o atalho contra a tradução célula a
    /// célula. Duas bastam: uma referência que o `map_references` do texto
    /// em inglês não reconhecesse já apareceria na primeira.
    conferidas: u8,
    /// O `xf`: fórmula empacotada não tem estilo direto, só o formato
    /// implícito, que é o mesmo para o padrão inteiro (vem do nome da função).
    xf: usize,
}

/// A fórmula de um `<c>`: nenhuma, um texto a escapar, ou um padrão em
/// pedaços expandido na célula.
enum Formula<'a> {
    Nenhuma,
    Texto(&'a str),
    Padrao(&'a [PedacoXml], u32, u32),
}

impl Formula<'_> {
    fn existe(&self) -> bool {
        !matches!(self, Formula::Nenhuma)
    }

    /// Escreve a fórmula já escapada. O `Padrao` dá o mesmo que
    /// `escape_xml(expand_pattern(...))`: o `format_ref` do padrão, com a
    /// aba citada pelo mesmo `quote_sheet`.
    fn escrever(&self, out: &mut String) {
        match self {
            Formula::Nenhuma => {}
            Formula::Texto(t) => escrever_escapado(out, t),
            Formula::Padrao(pedacos, row, col) => {
                for p in pedacos.iter() {
                    match p {
                        PedacoXml::Texto(t) => out.push_str(t),
                        PedacoXml::Ref { prefixo, col_lock, row_lock, col: c, row: r } => {
                            let c = if *col_lock { *c } else { *col as i32 + c };
                            let r = if *row_lock { *r } else { *row as i32 + r };
                            if c < 0 || r < 0 {
                                out.push_str("#REF!");
                                continue;
                            }
                            out.push_str(prefixo);
                            if *col_lock {
                                out.push('$');
                            }
                            escrever_coluna(out, c as u32);
                            if *row_lock {
                                out.push('$');
                            }
                            let _ = write!(out, "{}", r + 1);
                        }
                    }
                }
            }
        }
    }
}

/// Escreve as células de uma aba. Guarda o que se repete de célula para
/// célula — a tradução de cada padrão de fórmula e o `xf` de cada estilo —
/// para não refazer por célula: na planilha de 2 milhões de linhas eram
/// 6 milhões de fórmulas tokenizadas uma a uma.
/// (estilo direto, formato implícito): o que decide o `xf` de uma célula.
type ChaveXf = (Option<usize>, Option<&'static str>);

/// O que um texto do pool vira no `.xlsx` quando mora numa célula colunar
/// sem estilo direto e sem texto digitado guardado: aí o `<c>` depende só do
/// texto, e é calculado uma vez por texto distinto em vez de uma vez por
/// célula (no `spotify_full`, 97 milhões de células).
#[derive(Clone, Copy)]
enum MemoTexto {
    /// O texto vai para a tabela compartilhada.
    Compartilhado { indice: u32, xf: u32 },
    /// O texto vira número ou lógico (`R$ 5`, `VERDADEIRO`).
    Numero { n: f64, xf: u32 },
    Logico { b: bool, xf: u32 },
    /// Caso raro (vazio, começa com `=`): vai pelo caminho de sempre.
    Geral,
}

struct Escritor<'a> {
    pasta: &'a Workbook,
    indice: usize,
    sheet: &'a Sheet,
    estilos: &'a mut TabelaDeEstilos,
    textos: &'a mut TextosCompartilhados,
    padroes: HashMap<usize, Padrao>,
    /// (estilo direto, formato implícito) -> (`xf`, link).
    xfs: HashMap<ChaveXf, (usize, Option<String>)>,
    /// Por id do pool da aba; ver [`MemoTexto`].
    memo_textos: Vec<Option<MemoTexto>>,
    /// A aba tem matriz derramada (SEQUÊNCIA, FILTRO...)?
    ha_derrame: bool,
}

/// O padrão da fórmula em inglês, em pedaços, se ele reconstrói a tradução
/// da célula (`ingles`, sem o `=`).
fn padrao_em_ingles(ingles: &str, row: u32, col: u32) -> Option<Vec<PedacoXml>> {
    let com_igual = format!("={ingles}");
    let padrao = pattern_of(&com_igual, row as i32, col as i32);
    if expand_pattern(&padrao, row as i32, col as i32) != com_igual {
        return None;
    }
    let mut saida = Vec::new();
    for (i, p) in pedacos(&padrao).into_iter().enumerate() {
        saida.push(match p {
            Pedaco::Texto(t) => {
                let t = if i == 0 { t.strip_prefix('=').unwrap_or(&t).to_string() } else { t };
                PedacoXml::Texto(escape_xml(&t))
            }
            // O `=` é sempre o primeiro caractere, e referência não começa
            // antes dele.
            Pedaco::Ref(_) if i == 0 => return None,
            Pedaco::Ref(r) => PedacoXml::Ref {
                prefixo: r.sheet.as_deref().map(|s| escape_xml(&format!("{}!", quote_sheet(s)))).unwrap_or_default(),
                col_lock: r.col_lock,
                row_lock: r.row_lock,
                col: r.col,
                row: r.row,
            },
        });
    }
    Some(saida)
}

impl Escritor<'_> {
    /// Escreve o `<c>` de uma célula em `out`; `None` quando ela não tem
    /// nada a dizer ao Excel. Quando tem, devolve o link dela (referência e
    /// endereço).
    fn celula(&mut self, row: u32, col: u32, out: &mut String) -> Option<Option<(String, String)>> {
        let k = key(row, col);
        let sheet = self.sheet;
        // Matriz dinâmica: a âncora leva `<f t="array" ref>` e `cm="1"`; as
        // células derramadas vão só com o valor, como o Excel 365 grava.
        let mut matriz: Option<feathersheet_core::celulas::refs::Rect> = None;
        if self.ha_derrame {
            if let Some(a) = sheet.area_derramada_em(row, col).filter(|a| a.r0 != a.r1 || a.c0 != a.c1) {
                if (a.r0, a.c0) == (row, col) {
                    matriz = Some(a);
                } else {
                    let valor = self.pasta.valor(self.indice, row, col);
                    if matches!(valor, Value::Empty) {
                        return None;
                    }
                    let (xf, _) = self.xf(sheet.get_style_idx(row, col), None, None);
                    escrever_c(out, row, col, xf, &Formula::Nenhuma, &valor, self.textos);
                    return Some(None);
                }
            }
        }
        // A mesma ordem do `snapshot`: `Map`, literal colunar, fórmula
        // empacotada. O literal sem estilo e a fórmula empacotada vão por
        // atalho; o resto, pelo `snapshot`.
        if matriz.is_none() && !sheet.cells.contains_key(&k) {
            match sheet.data.cru_em(row, col) {
                Some(cru) if !sheet.styles.contains_key(&k) && !sheet.textos.contains_key(&k) => {
                    if self.literal_sem_estilo(row, col, cru, out) {
                        return Some(None);
                    }
                }
                Some(_) => {}
                // A fórmula empacotada com estilo direto vai pelo caminho de
                // sempre: o atalho escreve o `xf` do padrão e perdia o estilo
                // (fórmula em negrito saía sem negrito no .xlsx).
                None if !sheet.data.has_cell(row, col) && !sheet.styles.contains_key(&k) => {
                    if let Some((id, padrao)) = sheet.formulas.padrao_em(row, col) {
                        self.formula_empacotada(row, col, id, padrao, out);
                        return Some(None);
                    }
                }
                None => {}
            }
        }

        let snap = sheet.snapshot(row, col)?;
        let raw = snap.raw.as_str();
        let formula = raw.starts_with('=');
        let valor = if formula { self.pasta.valor(self.indice, row, col) } else { parse_literal(raw).value };
        let implicito = implied_format(raw);
        let (xf, link) = self.xf(sheet.get_style_idx(row, col), implicito, snap.style.as_ref());

        if matches!(valor, Value::Empty) && !formula && xf == 0 {
            return None;
        }
        let ingles = formula.then(|| formula_to_excel(raw));
        let f = ingles.as_deref().map_or(Formula::Nenhuma, Formula::Texto);
        let inicio = out.len();
        escrever_c(out, row, col, xf, &f, &valor, self.textos);
        if let Some(a) = matriz {
            // `<c r="A1" ...>` -> com `cm="1"`; `<f>` -> `<f t="array" ref="A1:A5">`.
            let fim_ref = out[inicio..].find("\"").and_then(|i| out[inicio + i + 1..].find('"').map(|j| inicio + i + 1 + j + 1));
            let trecho = out[inicio..].replacen("<f>", &format!("<f t=\"array\" ref=\"{}:{}\">", cell_name(a.r0, a.c0), cell_name(a.r1, a.c1)), 1);
            out.truncate(inicio);
            out.push_str(&trecho);
            if let Some(f) = fim_ref {
                out.insert_str(f, " cm=\"1\"");
            }
        }
        Some(link.map(|l| (cell_name(row, col), l)))
    }

    /// O `xf` (e o link) de um estilo direto com um formato implícito.
    fn xf(
        &mut self,
        idx: Option<usize>,
        implicito: Option<&'static str>,
        estilo: Option<&EstiloParcial>,
    ) -> (usize, Option<String>) {
        if let Some(achado) = self.xfs.get(&(idx, implicito)) {
            return achado.clone();
        }
        // O link não é formatação no Excel: vai para `<hyperlinks>`, e fora
        // do estilo para não criar um `xf` por endereço.
        let mut estilo = estilo.cloned().unwrap_or_default();
        let link = match estilo.remove("link") {
            Some(Campo::Texto(t)) if !t.is_empty() => Some(t),
            _ => None,
        };
        let par = (self.estilos.add(estilo, implicito), link);
        self.xfs.insert((idx, implicito), par.clone());
        par
    }

    /// O atalho do literal colunar sem estilo direto e sem texto digitado
    /// guardado. Dá o mesmo que o caminho do `snapshot` — lá o texto digitado
    /// seria o valor escrito (`display_raw` sem estilo), reanalisado pelo
    /// `parse_literal` —, sem montar o texto nem clonar estilo. `false`
    /// quando a célula precisa do caminho de sempre.
    fn literal_sem_estilo(&mut self, row: u32, col: u32, cru: Cru, out: &mut String) -> bool {
        match cru {
            Cru::Numero(n) => {
                let raw = to_js_string(n);
                let lit = parse_literal(&raw);
                let (xf, _) = self.xf(None, formato_do_literal(lit.format), None);
                match lit.value {
                    // O de quase sempre: o texto volta ao mesmo número, e o
                    // `<v>` é ele — sem escrever o número uma segunda vez.
                    Value::Number(m) if m.to_bits() == n.to_bits() => {
                        out.push_str("<c r=\"");
                        escrever_ref(out, row, col);
                        out.push('"');
                        if xf != 0 {
                            let _ = write!(out, " s=\"{xf}\"");
                        }
                        let _ = write!(out, "><v>{raw}</v></c>");
                    }
                    valor => escrever_c(out, row, col, xf, &Formula::Nenhuma, &valor, self.textos),
                }
                true
            }
            Cru::Texto(id) => {
                let i = id as usize;
                if self.memo_textos.len() <= i {
                    self.memo_textos.resize(i + 1, None);
                }
                let memo = match self.memo_textos[i] {
                    Some(m) => m,
                    None => {
                        let m = self.memo_do_texto(self.sheet.pool.get(id));
                        self.memo_textos[i] = Some(m);
                        m
                    }
                };
                let simples = |out: &mut String, xf: u32, atributo: &str| {
                    out.push_str("<c r=\"");
                    escrever_ref(out, row, col);
                    out.push('"');
                    if xf != 0 {
                        let _ = write!(out, " s=\"{xf}\"");
                    }
                    out.push_str(atributo);
                };
                match memo {
                    MemoTexto::Compartilhado { indice, xf } => {
                        simples(out, xf, " t=\"s\"><v>");
                        let _ = write!(out, "{indice}</v></c>");
                    }
                    MemoTexto::Numero { n, xf } => {
                        escrever_c(out, row, col, xf as usize, &Formula::Nenhuma, &Value::Number(n), self.textos);
                    }
                    MemoTexto::Logico { b, xf } => {
                        simples(out, xf, if b { " t=\"b\"><v>1</v></c>" } else { " t=\"b\"><v>0</v></c>" });
                    }
                    MemoTexto::Geral => return false,
                }
                true
            }
        }
    }

    /// O [`MemoTexto`] de um texto do pool. O texto digitado seria ele mesmo:
    /// o `display_raw` de um texto é o texto, e o de `VERDADEIRO`/`FALSO`
    /// (que o pool guarda como texto e a célula lê como lógico) é a palavra.
    fn memo_do_texto(&mut self, raw: &str) -> MemoTexto {
        if raw.is_empty() || raw.starts_with('=') {
            return MemoTexto::Geral;
        }
        // Texto "VERDADEIRO" (#66): sai como texto, não pelo analisador.
        if let Some(t) = raw.strip_prefix(feathersheet_core::planilha::data_table::MARCA_TEXTO) {
            let (xf, _) = self.xf(None, formato_do_literal(None), None);
            return MemoTexto::Compartilhado { indice: self.textos.add(t) as u32, xf: xf as u32 };
        }
        let lit = parse_literal(raw);
        let (xf, _) = self.xf(None, formato_do_literal(lit.format), None);
        let xf = xf as u32;
        match lit.value {
            Value::Text(t) => MemoTexto::Compartilhado { indice: self.textos.add(&t) as u32, xf },
            Value::Number(n) => MemoTexto::Numero { n, xf },
            Value::Bool(b) => MemoTexto::Logico { b, xf },
            _ => MemoTexto::Geral,
        }
    }

    fn formula_empacotada(&mut self, row: u32, col: u32, id: usize, padrao: &str, out: &mut String) {
        let valor = self.pasta.valor(self.indice, row, col);
        let celula_a_celula = || formula_to_excel(&expand_pattern(padrao, row as i32, col as i32));
        let mut texto: Option<String> = None;
        match self.padroes.get_mut(&id) {
            None => {
                let raw = expand_pattern(padrao, row as i32, col as i32);
                let ingles = formula_to_excel(&raw);
                let xf = self.estilos.add(EstiloParcial::new(), implied_format(&raw));
                let atalho = padrao_em_ingles(&ingles, row, col);
                self.padroes.insert(id, Padrao { ingles: atalho, conferidas: 1, xf });
                texto = Some(ingles);
            }
            Some(p) => {
                if p.conferidas < 2 {
                    p.conferidas += 1;
                    let certo = celula_a_celula();
                    if let Some(pedacos) = &p.ingles {
                        let mut atalho = String::new();
                        Formula::Padrao(pedacos, row, col).escrever(&mut atalho);
                        if atalho != escape_xml(&certo) {
                            p.ingles = None;
                        }
                    }
                    texto = Some(certo);
                } else if p.ingles.is_none() {
                    texto = Some(celula_a_celula());
                }
            }
        }
        let p = &self.padroes[&id];
        let f = match (&texto, &p.ingles) {
            (Some(t), _) => Formula::Texto(t),
            (None, Some(pedacos)) => Formula::Padrao(pedacos, row, col),
            (None, None) => unreachable!("sem atalho, o texto foi traduzido acima"),
        };
        if cfg!(debug_assertions) {
            let mut atalho = String::new();
            f.escrever(&mut atalho);
            debug_assert_eq!(atalho, escape_xml(&celula_a_celula()), "o atalho do padrão {id} divergiu");
        }
        escrever_c(out, row, col, p.xf, &f, &valor, self.textos);
    }
}

/// O `<c>` de uma célula; `formula` já em inglês.
fn escrever_c(
    out: &mut String,
    row: u32,
    col: u32,
    xf: usize,
    formula: &Formula,
    valor: &Value,
    textos: &mut TextosCompartilhados,
) {
    out.push_str("<c r=\"");
    escrever_ref(out, row, col);
    out.push('"');
    if xf != 0 {
        let _ = write!(out, " s=\"{xf}\"");
    }
    let fx = |out: &mut String| {
        if formula.existe() {
            out.push_str("<f>");
            formula.escrever(out);
            out.push_str("</f>");
        }
    };
    match valor {
        Value::Error(e) => {
            out.push_str(" t=\"e\">");
            fx(out);
            out.push_str("<v>");
            escrever_escapado(out, e.code.as_str());
            out.push_str("</v></c>");
        }
        Value::Number(n) => {
            out.push('>');
            fx(out);
            out.push_str("<v>");
            out.push_str(&to_js_string(*n));
            out.push_str("</v></c>");
        }
        Value::Bool(b) => {
            out.push_str(" t=\"b\">");
            fx(out);
            out.push_str(if *b { "<v>1</v></c>" } else { "<v>0</v></c>" });
        }
        // Resultado de fórmula vai literal ("str"); texto digitado vai na
        // tabela compartilhada ("s"), que é o que economiza espaço.
        Value::Text(t) if formula.existe() => {
            out.push_str(" t=\"str\">");
            fx(out);
            out.push_str("<v>");
            escrever_escapado(out, t);
            out.push_str("</v></c>");
        }
        Value::Text(t) => {
            let _ = write!(out, " t=\"s\"><v>{}</v></c>", textos.add(t));
        }
        Value::Empty if formula.existe() => {
            out.push('>');
            fx(out);
            out.push_str("</c>");
        }
        Value::Empty => out.push_str("/>"),
    }
}

/// Largura de uma coluna só agrupada, sem largura própria: a padrão da grade.
const LARGURA_PADRAO_PX: u32 = 122;

/// ` outlineLevel="n" collapsed="1"` do índice, quando ele tem tópico.
fn atributos_topico(eixo: &feathersheet_core::planilha::sheet::topicos::EixoTopicos, indice: u32) -> String {
    let mut s = String::new();
    let nivel = eixo.nivel(indice);
    if nivel > 0 { let _ = write!(s, " outlineLevel=\"{nivel}\""); }
    if eixo.recolhidas.contains(&indice) { s.push_str(" collapsed=\"1\""); }
    s
}

// ============================================================== validação

/// `DD/MM/AAAA` (ou `AAAA-MM-DD`, do formulário antigo) → número de série do
/// Excel. O inverso do `dias_para_data` do leitor.
fn serie_da_data(texto: &str) -> Option<i64> {
    let t = texto.trim();
    let (y, m, d) = if let Some((d, resto)) = t.split_once('/') {
        let (m, y) = resto.split_once('/')?;
        (y.parse::<i64>().ok()?, m.parse::<i64>().ok()?, d.parse::<i64>().ok()?)
    } else {
        let (y, resto) = t.split_once('-')?;
        let (m, d) = resto.split_once('-')?;
        (y.parse::<i64>().ok()?, m.parse::<i64>().ok()?, d.parse::<i64>().ok()?)
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Algoritmo de Howard Hinnant (days_from_civil), domínio público.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias_desde_1970 = era * 146_097 + doe - 719_468;
    Some(dias_desde_1970 + 25_569)
}

/// `A1:B9 D2` do atributo `sqref`.
fn sqref(ranges: &[feathersheet_core::celulas::refs::Rect]) -> String {
    let mut s = String::new();
    for r in ranges {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&cell_name(r.r0, r.c0));
        if (r.r0, r.c0) != (r.r1, r.c1) {
            s.push(':');
            s.push_str(&cell_name(r.r1, r.c1));
        }
    }
    s
}

/// Texto entre aspas numa fórmula do Excel (`"` dobra).
fn literal_excel(texto: &str) -> String {
    format!("\"{}\"", texto.replace('"', "\"\""))
}

/// `<dataValidations>` da aba, a partir do `Sheet.va` (JSON de
/// `Vec<Validacao>`). Vazio quando a aba não tem regra que o Excel entenda.
///
/// Tudo vai no bloco **clássico**, inclusive a lista que lê outra aba
/// (`'Dados'!$A$2:$A$9`) e o `INDIRECT`: o Excel 2010+ aceita referência a
/// outra aba direto no `<dataValidation>`, e o clássico é o que o
/// LibreOffice, o Google Planilhas e as bibliotecas (openpyxl etc.) leem.
/// O `x14:dataValidation` do `<extLst>` só existe para o Excel 2007 não se
/// perder, e exigiria declarar os namespaces `x14`/`xm` e o `mc:Ignorable`.
///
/// Mensagem de entrada → `showInputMessage`/`promptTitle`/`prompt`; alerta →
/// `errorStyle` (stop/warning/information), `errorTitle` e `error`.
/// `showDropDown` fica de fora: no OOXML `showDropDown="1"` **esconde** a
/// seta, e a falta dele a mostra. Não têm equivalente no Excel e ficam de
/// de tipo próprio: caixa de seleção, e-mail, URL, "contém" e "começa com"
/// vão como `custom` com fórmula equivalente (`xlsx_validacao_custom`).
fn validacoes_xml(sheet: &Sheet) -> String {
    use feathersheet_core::operacoes::validation::{DataOp, NumeroOp, QuandoValidacao, Regra, Validacao};

    let Some(json) = sheet.va.as_deref() else { return String::new() };
    let Ok(lista) = serde_json::from_str::<Vec<Validacao>>(json) else { return String::new() };
    let mut itens = String::new();
    let mut n = 0usize;
    for v in &lista {
        if v.ranges.is_empty() {
            continue;
        }
        // (type, operator, formula1, formula2), fórmulas já em inglês e sem `=`.
        let (tipo, operador, f1, f2): (&str, Option<&str>, String, Option<String>) = match &v.regra {
            Regra::Lista { valores } => ("list", None, literal_excel(&valores.join(",")), None),
            Regra::Intervalo { origem } => {
                let area = format!(
                    "${}${}:${}${}",
                    col_name(origem.c0),
                    origem.r0 + 1,
                    col_name(origem.c1),
                    origem.r1 + 1
                );
                let area = match &origem.sheet {
                    Some(aba) => format!("{}!{area}", quote_sheet(aba)),
                    None => area,
                };
                ("list", None, area, None)
            }
            Regra::Formula { formula } => ("list", None, formula_to_excel(formula), None),
            Regra::Numero { op, a, b, inteiro } => {
                let operador = match op {
                    NumeroOp::Entre => "between",
                    NumeroOp::Fora => "notBetween",
                    NumeroOp::Igual => "equal",
                    NumeroOp::Diferente => "notEqual",
                    NumeroOp::Maior => "greaterThan",
                    NumeroOp::Menor => "lessThan",
                    NumeroOp::MaiorIgual => "greaterThanOrEqual",
                    NumeroOp::MenorIgual => "lessThanOrEqual",
                };
                let dois = matches!(op, NumeroOp::Entre | NumeroOp::Fora);
                let f2 = if dois { Some(to_js_string(b.unwrap_or(*a))) } else { None };
                (if *inteiro { "whole" } else { "decimal" }, Some(operador), to_js_string(*a), f2)
            }
            Regra::Data { op, a, b } => {
                let Some(sa) = serie_da_data(a) else { continue };
                match op {
                    DataOp::Entre => {
                        let sb = b.as_deref().and_then(serie_da_data).unwrap_or(sa);
                        ("date", Some("between"), sa.to_string(), Some(sb.to_string()))
                    }
                    DataOp::Antes => ("date", Some("lessThan"), sa.to_string(), None),
                    DataOp::Depois => ("date", Some("greaterThan"), sa.to_string(), None),
                }
            }
            // Hora: o Excel guarda a fração do dia.
            Regra::Hora { op, a, b } => {
                use feathersheet_core::operacoes::validation::parse_hora_segundos;
                let dia = |t: &str| parse_hora_segundos(&Value::Text(t.to_string())).map(|s| (s as f64 / 86_400.0).to_string());
                let Some(fa) = dia(a) else { continue };
                let operador = match op {
                    NumeroOp::Entre => "between",
                    NumeroOp::Fora => "notBetween",
                    NumeroOp::Igual => "equal",
                    NumeroOp::Diferente => "notEqual",
                    NumeroOp::Maior => "greaterThan",
                    NumeroOp::Menor => "lessThan",
                    NumeroOp::MaiorIgual => "greaterThanOrEqual",
                    NumeroOp::MenorIgual => "lessThanOrEqual",
                };
                let f2 = if matches!(op, NumeroOp::Entre | NumeroOp::Fora) {
                    Some(b.as_deref().and_then(dia).unwrap_or_else(|| fa.clone()))
                } else {
                    None
                };
                ("time", Some(operador), fa, f2)
            }
            // Sem tipo próprio no Excel: personalizada, relativa à primeira
            // célula da regra (caixa, e-mail, URL, contém, começa com).
            outra => {
                let alvo = cell_name(v.ranges[0].r0, v.ranges[0].c0);
                let Some(f) = crate::xlsx_validacao_custom::formula_custom(outra, &alvo) else { continue };
                ("custom", None, f, None)
            }
        };
        n += 1;
        let _ = write!(itens, "<dataValidation type=\"{tipo}\"");
        if let Some(op) = operador {
            let _ = write!(itens, " operator=\"{op}\"");
        }
        // Recusar = "Parar"; Avisar = "Aviso" (deixa passar depois de perguntar).
        let estilo = match v.quando {
            QuandoValidacao::Recusar => "stop",
            QuandoValidacao::Avisar => "warning",
            QuandoValidacao::Informar => "information",
        };
        let _ = write!(itens, " errorStyle=\"{estilo}\" allowBlank=\"1\" showErrorMessage=\"1\"");
        // Limites do Excel: títulos 32 caracteres, textos 255.
        let corta = |t: &Option<String>, max: usize| {
            t.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| t.chars().take(max).collect::<String>())
        };
        if let Some(t) = corta(&v.mensagens.erro_titulo, 32) {
            let _ = write!(itens, " errorTitle=\"{}\"", escape_xml(&t));
        }
        if let Some(msg) = corta(&v.mensagem, 255) {
            let _ = write!(itens, " error=\"{}\"", escape_xml(&msg));
        }
        let (pt, px) = (corta(&v.mensagens.entrada_titulo, 32), corta(&v.mensagens.entrada_texto, 255));
        if pt.is_some() || px.is_some() {
            itens.push_str(" showInputMessage=\"1\"");
            if let Some(t) = pt {
                let _ = write!(itens, " promptTitle=\"{}\"", escape_xml(&t));
            }
            if let Some(t) = px {
                let _ = write!(itens, " prompt=\"{}\"", escape_xml(&t));
            }
        }
        let _ = write!(itens, " sqref=\"{}\"><formula1>{}</formula1>", sqref(&v.ranges), escape_xml(&f1));
        if let Some(f2) = f2 {
            let _ = write!(itens, "<formula2>{}</formula2>", escape_xml(&f2));
        }
        itens.push_str("</dataValidation>");
    }
    if n == 0 {
        return String::new();
    }
    format!("<dataValidations count=\"{n}\">{itens}</dataValidations>")
}

/// `<sheetProtection>`: cada permissão escrita por extenso ("1" = proibido,
/// como no Excel) e a senha no hash SHA-512 do próprio Excel.
fn protecao_xml(p: &feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha) -> String {
    use feathersheet_core::planilha::sheet::protecao::Permissao;
    let mut s = String::from("<sheetProtection");
    if let Some(senha) = &p.senha {
        let _ = write!(s, " algorithmName=\"{}\" hashValue=\"{}\" saltValue=\"{}\" spinCount=\"{}\"",
            escape_xml(&senha.algoritmo), escape_xml(&senha.hash), escape_xml(&senha.sal), senha.giros);
    }
    s.push_str(" sheet=\"1\"");
    for (permissao, _, atributo) in Permissao::TODAS {
        let _ = write!(s, " {atributo}=\"{}\"", if p.permite(permissao) { 0 } else { 1 });
    }
    s.push_str("/>");
    s
}

/// O `sheetN.xml` de uma aba e as relações dela (só quando há link).
fn aba_xml(
    pasta: &Workbook,
    indice: usize,
    estilos: &mut TabelaDeEstilos,
    textos: &mut TextosCompartilhados,
) -> (String, Option<String>) {
    let sheet = &pasta.aba(indice).expect("índice dentro da pasta").sheet;
    let mut chaves = chaves_ocupadas(sheet);
    // Avalia as fórmulas soltas (as de matriz nunca vão empacotadas) para o
    // derramamento existir, e põe as células derramadas na lista.
    for (&k, c) in &sheet.cells {
        if c.raw.starts_with('=') {
            let _ = pasta.valor(indice, key_row(k), key_col(k));
        }
    }
    let areas = sheet.areas_derramadas();
    let ha_derrame = areas.iter().any(|a| a.r0 != a.r1 || a.c0 != a.c1);
    if ha_derrame {
        for a in &areas {
            for r in a.r0..=a.r1 {
                for c in a.c0..=a.c1 {
                    chaves.push(key(r, c));
                }
            }
        }
        chaves.sort_unstable();
        chaves.dedup();
    }
    // Altura (`None` = padrão) e tópico de cada linha que tem algo além das células.
    let topicos = &sheet.topicos;
    let mut alturas: BTreeMap<u32, Option<u32>> = sheet.row_heights.iter().map(|(&r, &h)| (r, Some(h))).collect();
    for &r in topicos.linhas.niveis.keys().chain(topicos.linhas.recolhidas.iter()) {
        alturas.entry(r).or_insert(None);
    }
    let mut links: Vec<(String, String)> = Vec::new();
    let mut escritor =
        Escritor { pasta, indice, sheet, estilos, textos, padroes: HashMap::new(), xfs: HashMap::new(), memo_textos: Vec::new(), ha_derrame };

    // Em ordem: o formato pede, e o Excel "repara" o arquivo que vem fora dela.
    let mut larguras: BTreeMap<u32, Option<u32>> = sheet.col_widths.iter().map(|(&c, &w)| (c, Some(w))).collect();
    for &c in topicos.colunas.niveis.keys().chain(topicos.colunas.recolhidas.iter()) {
        larguras.entry(c).or_insert(None);
    }
    // Colunas ocultas até o fim: um `<col>` só, de N até XFD, no fim da lista.
    let colunas_desde = sheet.colunas_ocultas_desde.filter(|&n| n < MAX_COLS_XLSX);
    if let Some(n) = colunas_desde { larguras.retain(|&c, _| c < n); }
    let cols = if larguras.is_empty() && colunas_desde.is_none() {
        String::new()
    } else {
        let itens: String = larguras
            .iter()
            .map(|(&c, w)| {
                let mut col = format!("<col min=\"{0}\" max=\"{0}\"", c + 1);
                match w {
                    Some(w) => { let _ = write!(col, " width=\"{}\" customWidth=\"1\"", to_js_string(arredondar(px_para_largura(*w)))); }
                    None => { let _ = write!(col, " width=\"{}\"", to_js_string(arredondar(px_para_largura(sheet.largura_padrao.unwrap_or(LARGURA_PADRAO_PX))))); }
                }
                col.push_str(&atributos_topico(&topicos.colunas, c));
                col.push_str("/>");
                col
            })
            .collect();
        let resto = colunas_desde.map_or(String::new(), |n| format!(
            "<col min=\"{}\" max=\"{MAX_COLS_XLSX}\" width=\"{}\" hidden=\"1\"/>", n + 1,
            to_js_string(arredondar(px_para_largura(sheet.largura_padrao.unwrap_or(LARGURA_PADRAO_PX))))));
        format!("<cols>{itens}{resto}</cols>")
    };
    // Linhas ocultas até o fim: `zeroHeight` oculta toda linha não listada,
    // então as visíveis (0..N) vão todas, mesmo vazias; as de N em diante
    // que tiverem células saem com `hidden`.
    let linhas_desde = sheet.linhas_ocultas_desde.filter(|&n| n < MAX_ROWS_XLSX);

    // O XML sai num `String` só, sem montar o corpo à parte e copiá-lo no
    // fim: no `spotify_full` o corpo tem 2,7 GB.
    // Largura padrão das colunas (Formatar ▾ → Largura padrão): vem antes do <cols>.
    // Sempre declarada: sem ela o Excel assume 64 px por coluna e 20 px por linha
    // (esta é igual: 15 pt), e gráficos/layout saem mais estreitos que no app.
    let formato = format!(
        "<sheetFormatPr baseColWidth=\"8\" defaultColWidth=\"{}\" defaultRowHeight=\"15\"{}/>",
        to_js_string(arredondar(px_para_largura(sheet.largura_padrao.unwrap_or(LARGURA_PADRAO_PX)))),
        if linhas_desde.is_some() { " zeroHeight=\"1\"" } else { "" });
    let mut xml = format!("{XML_DECL}<worksheet xmlns=\"{NS}\" xmlns:r=\"{NS_REL}\">{formato}{cols}<sheetData>");
    let corpo = &mut xml;
    let mut celulas = String::new();
    let mut i = 0;
    let mut alt = alturas.iter().peekable();
    // Próxima linha visível ainda não escrita (só com `zeroHeight`).
    let mut vazia = 0_u32;
    let escrever_vazias = |corpo: &mut String, vazia: &mut u32, ate: u32| {
        if let Some(n) = linhas_desde {
            while *vazia < ate.min(n) {
                let _ = write!(corpo, "<row r=\"{}\"/>", *vazia + 1);
                *vazia += 1;
            }
        }
    };
    loop {
        // A próxima linha é a menor entre a das células e a das alturas.
        let linha_celula = chaves.get(i).map(|&k| key_row(k));
        let linha_altura = alt.peek().map(|(&r, _)| r);
        let linha = match (linha_celula, linha_altura) {
            (None, None) => {
                escrever_vazias(corpo, &mut vazia, u32::MAX);
                break;
            }
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
        };
        escrever_vazias(corpo, &mut vazia, linha);
        vazia = vazia.max(linha + 1);
        celulas.clear();
        while let Some(&k) = chaves.get(i) {
            if key_row(k) != linha {
                break;
            }
            if let Some(Some(link)) = escritor.celula(linha, key_col(k), &mut celulas) {
                links.push(link);
            }
            i += 1;
        }
        let extra = if linha_altura == Some(linha) { alt.next().map(|(_, &h)| h) } else { None };
        let visivel_obrigatoria = linhas_desde.is_some_and(|n| linha < n);
        let alem_do_fim = linhas_desde.is_some_and(|n| linha >= n);
        // Linha cujas células não disseram nada e sem altura própria nem tópico não vai.
        if celulas.is_empty() && extra.is_none() && !visivel_obrigatoria {
            continue;
        }
        let _ = write!(corpo, "<row r=\"{}\"", linha + 1);
        match extra.flatten() {
            _ if alem_do_fim => corpo.push_str(" hidden=\"1\""),
            Some(0) => corpo.push_str(" hidden=\"1\""),
            Some(h) => {
                let _ = write!(corpo, " ht=\"{}\" customHeight=\"1\"", to_js_string(arredondar(h as f64 * PX_PARA_PT)));
            }
            None => {}
        }
        corpo.push_str(&atributos_topico(&topicos.linhas, linha));
        corpo.push('>');
        corpo.push_str(&celulas);
        corpo.push_str("</row>");
    }

    drop(chaves);

    let tabelas = lista_json(&sheet.tb);
    let auto_filtro = tabelas
        .first()
        .and_then(|t| Some((t, retangulo(t.get("area")?)?)))
        .map(|(t, area)| crate::xlsx_write_autofiltro::autofiltro_xml(area, t, &mut |e| estilos.dxf(e)))
        .unwrap_or_default();

    // Células mescladas: no esquema vêm depois do `autoFilter`.
    let mesclas: Vec<_> = lista_json(&sheet.mc).iter().filter_map(retangulo).collect();
    let mescladas = if mesclas.is_empty() {
        String::new()
    } else {
        let itens: String = mesclas
            .iter()
            .map(|&(r0, c0, r1, c1)| format!("<mergeCell ref=\"{}:{}\"/>", cell_name(r0, c0), cell_name(r1, c1)))
            .collect();
        format!("<mergeCells count=\"{}\">{itens}</mergeCells>", mesclas.len())
    };

    // Hiperlinks: depois das mescladas, e o endereço mora nas relações da aba.
    let (hiperlinks, relacoes) = if links.is_empty() {
        (String::new(), None)
    } else {
        let refs: String = links
            .iter()
            .enumerate()
            .map(|(i, (r, _))| format!("<hyperlink ref=\"{r}\" r:id=\"rIdLink{}\"/>", i + 1))
            .collect();
        let rels: String = links
            .iter()
            .enumerate()
            .map(|(i, (_, alvo))| {
                format!(
                    "<Relationship Id=\"rIdLink{}\" Type=\"{NS_REL}/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>",
                    i + 1,
                    escape_xml(alvo)
                )
            })
            .collect();
        (
            format!("<hyperlinks>{refs}</hyperlinks>"),
            Some(format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{rels}</Relationships>")),
        )
    };

    let protecao = sheet.protecao.as_ref().map(protecao_xml).unwrap_or_default();
    // Ordem do CT_Worksheet: ... mergeCells, conditionalFormatting,
    // dataValidations, hyperlinks, printOptions, pageMargins ...
    let condicional = crate::xlsx_write_condicional::condicional_xml(sheet, &mut |e| estilos.dxf(e));
    let validacoes = validacoes_xml(sheet);
    let _ = write!(xml, "</sheetData>{protecao}{auto_filtro}{mescladas}{condicional}{validacoes}{hiperlinks}</worksheet>");
    xml.shrink_to_fit();
    (xml, relacoes)
}

/// Os arquivos de dentro do `.xlsx`, já escritos, na ordem do TypeScript.
/// É a parte que precisa da pasta; comprimir fica com o [`zipar`].
fn n_abas(pasta: &Workbook) -> usize { pasta.len() }

/// `xl/metadata.xml` do Excel 365 para matrizes dinâmicas (`cm="1"`).
const METADATA_MATRIZ_DINAMICA: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:xda="http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray"><metadataTypes count="1"><metadataType name="XLDAPR" minSupportedVersion="120000" copy="1" pasteAll="1" pasteValues="1" merge="1" splitFirst="1" rowColShift="1" clearFormats="1" clearComments="1" assign="1" coerce="1" cellMeta="1"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}"><xda:dynamicArrayProperties fDynamic="1" fCollapsed="0"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#;

pub fn montar_partes(pasta: &Workbook) -> Vec<(String, Vec<u8>)> {
    let mut estilos = TabelaDeEstilos::nova();
    let mut textos = TextosCompartilhados::default();
    let mut abas = Vec::with_capacity(pasta.len());
    // Tinta da aba Desenhar: formas livres num `xl/drawings/drawingN.xml`.
    let mut desenhos: Vec<(usize, String, Option<String>)> = Vec::new();
    // Gráficos (`xl/charts/…`), no mesmo desenho da aba: `xlsx_write_graficos`.
    let mut partes_graficos: Vec<crate::xlsx_write_graficos::ParteGrafico> = Vec::new();
    let mut contador_graficos = 0usize;
    // Comentários: `xl/commentsN.xml` + `xl/drawings/vmlDrawingN.vml`.
    let mut comentarios: Vec<(usize, (String, String))> = Vec::new();
    // Tabelas dinâmicas (`xl/pivotCache/…`, `xl/pivotTables/…`): `xlsx_write_dinamica`.
    let mut dinamicas: Vec<crate::xlsx_write_dinamica::ParteDinamica> = Vec::new();
    let mut contador_dinamicas = 0usize;
    let livros_externos = crate::xlsx_write_vinculos::livros(pasta);
    crate::xlsx_write_vinculos::usar_ordem(&livros_externos);
    for i in 0..pasta.len() {
        let (mut xml, mut rels) = aba_xml(pasta, i, &mut estilos, &mut textos);
        let tn = pasta.aba(i).and_then(|a| a.sheet.tn.as_deref());
        let tinta = crate::tinta_aba::desenho_xlsx(tn);
        if let Some(desenho) = crate::xlsx_write_graficos::desenho_da_aba(pasta, i, tinta, &mut contador_graficos) {
            // `drawing` vem depois de `hyperlinks` no CT_Worksheet.
            if let Some(fim) = xml.rfind("</worksheet>") {
                xml.insert_str(fim, "<drawing r:id=\"rIdTinta\"/>");
            }
            let relacao = format!(
                "<Relationship Id=\"rIdTinta\" Type=\"{NS_REL}/drawing\" Target=\"../drawings/drawing{}.xml\"/>",
                i + 1
            );
            rels = Some(match rels {
                Some(mut r) => {
                    if let Some(fim) = r.rfind("</Relationships>") {
                        r.insert_str(fim, &relacao);
                    }
                    r
                }
                None => format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{relacao}</Relationships>"),
            });
            partes_graficos.extend(desenho.partes);
            desenhos.push((i + 1, desenho.xml, desenho.rels));
        }
        // Comentários: `legacyDrawing` vem depois de `drawing` no CT_Worksheet.
        if let Some(c) = pasta.aba(i).and_then(|a| crate::xlsx_write_comentarios::partes(&a.sheet, i + 1)) {
            if let Some(fim) = xml.rfind("</worksheet>") {
                xml.insert_str(fim, "<legacyDrawing r:id=\"rIdVmlNotas\"/>");
            }
            let relacao = format!(
                "<Relationship Id=\"rIdNotas\" Type=\"{NS_REL}/comments\" Target=\"../comments{n}.xml\"/><Relationship Id=\"rIdVmlNotas\" Type=\"{NS_REL}/vmlDrawing\" Target=\"../drawings/vmlDrawing{n}.vml\"/>",
                n = i + 1
            );
            rels = Some(match rels {
                Some(mut r) => {
                    if let Some(fim) = r.rfind("</Relationships>") {
                        r.insert_str(fim, &relacao);
                    }
                    r
                }
                None => format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{relacao}</Relationships>"),
            });
            comentarios.push((i + 1, c));
        }
        // Dinâmicas com destino nesta aba: só nas relações (o XML da aba não as cita).
        let novas = crate::xlsx_write_dinamica::da_aba(pasta, i, &mut contador_dinamicas);
        if !novas.is_empty() {
            let relacao: String = novas.iter().map(crate::xlsx_write_dinamica::relacao_da_aba).collect();
            rels = Some(match rels {
                Some(mut r) => {
                    if let Some(fim) = r.rfind("</Relationships>") {
                        r.insert_str(fim, &relacao);
                    }
                    r
                }
                None => format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{relacao}</Relationships>"),
            });
            dinamicas.extend(novas);
        }
        abas.push((xml, rels));
    }

    let nomes_das_abas = pasta.nomes();
    let n = nomes_das_abas.len();
    let ha_metadata = (0..n).any(|i| {
        pasta.aba(i).is_some_and(|a| a.sheet.areas_derramadas().iter().any(|r| r.r0 != r.r1 || r.c0 != r.c1))
    });
    // Aba oculta vira `state="hidden"` (o modelo só tem bool: `veryHidden`
    // lido volta como `hidden`). O Excel exige ao menos uma aba visível e a
    // aba ativa nunca pode ser oculta: se todas estiverem ocultas, a primeira
    // fica visível; `activeTab` é sempre a primeira visível.
    let mut ocultas: Vec<bool> = (0..n_abas(pasta)).map(|i| pasta.aba(i).is_some_and(|a| a.sheet.oculta)).collect();
    if !ocultas.is_empty() && ocultas.iter().all(|&o| o) { ocultas[0] = false; }
    let aba_ativa = ocultas.iter().position(|&o| !o).unwrap_or(0);
    let entradas_abas: String = nomes_das_abas
        .iter()
        .enumerate()
        .map(|(i, nome)| {
            let estado = if ocultas.get(i).copied().unwrap_or(false) { " state=\"hidden\"" } else { "" };
            format!("<sheet name=\"{}\" sheetId=\"{}\"{estado} r:id=\"rId{}\"/>", escape_xml(nome), i + 1, i + 1)
        })
        .collect();
    let nomeados = pasta.nomeados();
    let nomes_definidos = if nomeados.is_empty() {
        String::new()
    } else {
        let primeira = nomes_das_abas.first().cloned().unwrap_or_else(|| "Planilha1".to_string());
        let itens: String = nomeados
            .iter()
            .map(|nm| {
                let a = &nm.area;
                // A aba do nome; sem ela (ou com uma que não existe mais), a primeira.
                let aba = a.sheet.as_deref().filter(|s| nomes_das_abas.iter().any(|n| n.eq_ignore_ascii_case(s))).unwrap_or(&primeira);
                let aba = nomes_das_abas.iter().find(|n| n.eq_ignore_ascii_case(aba)).unwrap_or(&primeira);
                let area = format!(
                    "{}!${}${}:${}${}",
                    quote_sheet(aba),
                    col_name(a.c0),
                    a.r0 + 1,
                    col_name(a.c1),
                    a.r1 + 1
                );
                format!("<definedName name=\"{}\">{}</definedName>", escape_xml(&nm.nome), escape_xml(&area))
            })
            .collect();
        format!("<definedNames>{itens}</definedNames>")
    };

    let tipos_abas: String = (1..=n)
        .map(|i| {
            format!(
                "<Override PartName=\"/xl/worksheets/sheet{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
            )
        })
        .chain(std::iter::once(crate::xlsx_write_graficos::tipos_de_conteudo(&partes_graficos)))
        .chain(desenhos.iter().map(|(i, _, _)| {
            format!("<Override PartName=\"/xl/drawings/drawing{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>")
        }))
        .chain(comentarios.iter().map(|(i, _)| {
            format!("<Override PartName=\"/xl/comments{i}.xml\" ContentType=\"{}\"/>", crate::xlsx_write_comentarios::TIPO_COMENTARIOS)
        }))
        .chain(std::iter::once(crate::xlsx_write_dinamica::tipos_de_conteudo(&dinamicas)))
        .chain(std::iter::once(crate::xlsx_write_vinculos::tipos_de_conteudo(&livros_externos)))
        .collect();
    let tipo_vml = if comentarios.is_empty() {
        String::new()
    } else {
        format!("<Default Extension=\"vml\" ContentType=\"{}\"/>", crate::xlsx_write_comentarios::TIPO_VML)
    };
    let tipo_metadata = if ha_metadata {
        "<Override PartName=\"/xl/metadata.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml\"/>"
    } else {
        ""
    };
    let rel_metadata = if ha_metadata {
        format!("<Relationship Id=\"rIdMetadata\" Type=\"{NS_REL}/sheetMetadata\" Target=\"metadata.xml\"/>")
    } else {
        String::new()
    };
    // Propriedades do arquivo (Arquivo → Informações no Excel): só quando há autor.
    let props = &pasta.propriedades;
    let (tipo_props, rel_props) = if props.vazia() {
        (String::new(), String::new())
    } else {
        (
            "<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/><Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>".to_string(),
            format!("<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/><Relationship Id=\"rId3\" Type=\"{NS_REL}/extended-properties\" Target=\"docProps/app.xml\"/>"),
        )
    };
    let rels_abas: String = (1..=n)
        .map(|i| format!("<Relationship Id=\"rId{i}\" Type=\"{NS_REL}/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>"))
        .collect();
    // `pivotCaches` vem depois de `definedNames` no CT_Workbook.
    let caches_dinamicas = crate::xlsx_write_dinamica::caches_do_livro(&dinamicas);
    let rels_dinamicas = crate::xlsx_write_dinamica::relacoes_do_livro(&dinamicas);
    crate::xlsx_write_vinculos::usar_ordem(&[]);
    let externas = crate::xlsx_write_vinculos::referencias_do_livro(&livros_externos);
    let rels_externas = crate::xlsx_write_vinculos::relacoes_do_livro(&livros_externos);

    let mut partes: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            format!(
                "{XML_DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>{tipo_vml}{tipo_metadata}{tipo_props}<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>{tipos_abas}<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/></Types>"
            )
            .into_bytes(),
        ),
        (
            "_rels/.rels".into(),
            format!(
                "{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/officeDocument\" Target=\"xl/workbook.xml\"/>{rel_props}</Relationships>"
            )
            .into_bytes(),
        ),
        (
            "xl/workbook.xml".into(),
            format!(
                "{XML_DECL}<workbook xmlns=\"{NS}\" xmlns:r=\"{NS_REL}\"><bookViews><workbookView activeTab=\"{aba_ativa}\" firstSheet=\"{aba_ativa}\"/></bookViews><sheets>{entradas_abas}</sheets>{externas}{nomes_definidos}{caches_dinamicas}</workbook>"
            )
            .into_bytes(),
        ),
        (
            "xl/_rels/workbook.xml.rels".into(),
            format!(
                "{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">{rels_abas}<Relationship Id=\"rIdStyles\" Type=\"{NS_REL}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rIdStrings\" Type=\"{NS_REL}/sharedStrings\" Target=\"sharedStrings.xml\"/>{rel_metadata}{rels_dinamicas}{rels_externas}</Relationships>"
            )
            .into_bytes(),
        ),
        ("xl/styles.xml".into(), estilos_xml(&estilos).into_bytes()),
        ("xl/sharedStrings.xml".into(), textos.xml().into_bytes()),
    ];
    if ha_metadata {
        partes.push(("xl/metadata.xml".into(), METADATA_MATRIZ_DINAMICA.as_bytes().to_vec()));
    }
    if !props.vazia() {
        partes.push(("docProps/core.xml".into(), format!(
            "{XML_DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:creator>{}</dc:creator><cp:lastModifiedBy>{}</cp:lastModifiedBy></cp:coreProperties>",
            escape_xml(&props.autor), escape_xml(&props.modificado_por),
        ).into_bytes()));
        partes.push(("docProps/app.xml".into(), format!(
            "{XML_DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>FeatherSheet</Application><Company>{}</Company></Properties>",
            escape_xml(&props.organizacao),
        ).into_bytes()));
    }
    let mut relacoes = Vec::new();
    for (i, (xml, rels)) in abas.into_iter().enumerate() {
        partes.push((format!("xl/worksheets/sheet{}.xml", i + 1), xml.into_bytes()));
        if let Some(r) = rels {
            relacoes.push((format!("xl/worksheets/_rels/sheet{}.xml.rels", i + 1), r.into_bytes()));
        }
    }
    partes.extend(relacoes);
    for (i, desenho, rels_desenho) in desenhos {
        partes.push((format!("xl/drawings/drawing{i}.xml"), desenho.into_bytes()));
        if let Some(r) = rels_desenho {
            partes.push((format!("xl/drawings/_rels/drawing{i}.xml.rels"), r.into_bytes()));
        }
    }
    partes.extend(partes_graficos.into_iter().map(|p| (p.caminho, p.xml.into_bytes())));
    for (i, (lista, vml)) in comentarios {
        partes.push((format!("xl/comments{i}.xml"), lista.into_bytes()));
        partes.push((format!("xl/drawings/vmlDrawing{i}.vml"), vml.into_bytes()));
    }
    partes.extend(crate::xlsx_write_dinamica::arquivos(dinamicas));
    partes.extend(crate::xlsx_write_vinculos::arquivos(pasta, &livros_externos));
    partes
}

/// A pasta inteira num `.xlsx`.
pub fn gerar_xlsx(pasta: &Workbook) -> Result<Vec<u8>, String> {
    zipar(montar_partes(pasta))
}

thread_local! {
    /// Funções chamadas nas fórmulas exportadas (nome internacional), só
    /// enquanto [`gerar_xlsx_com_funcoes`] está montando o arquivo.
    static FUNCOES_USADAS: std::cell::RefCell<Option<std::collections::BTreeSet<String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Como [`gerar_xlsx`], devolvendo também as funções usadas nas fórmulas
/// (nome internacional, em maiúscula), para avisar das que o Excel não tem.
pub fn gerar_xlsx_com_funcoes(pasta: &Workbook) -> Result<(Vec<u8>, std::collections::BTreeSet<String>), String> {
    FUNCOES_USADAS.with(|f| *f.borrow_mut() = Some(Default::default()));
    let partes = montar_partes(pasta);
    let usadas = FUNCOES_USADAS.with(|f| f.borrow_mut().take()).unwrap_or_default();
    Ok((zipar(partes)?, usadas))
}

// ============================================================== CSV

/// O texto de um valor como a célula o mostra — o `formatValue` do
/// `style.ts`: só número é formatado.
fn texto_formatado(valor: &Value, estilo: Option<&feathersheet_core::celulas::style::Style>) -> String {
    match valor {
        Value::Number(n) if estilo.map(|s| s.format) != Some(NumberFormat::Text) => format_number(*n, estilo),
        v => to_text(v),
    }
}

/// Uma aba em CSV/TSV, do jeito que a grade a mostra (o valor formatado, não
/// o texto digitado). O retângulo vai de A1 até a última célula usada, como
/// no TypeScript; o campo leva aspas quando contém aspas, quebra de linha ou
/// o separador.
pub fn gerar_delimitado(pasta: &mut Workbook, indice: usize, separador: char) -> Result<Vec<u8>, String> {
    let viva = pasta.aba_mut(indice).ok_or_else(|| format!("não existe a aba {indice}"))?;
    let (max_r, max_c) = (viva.sheet.max_row_ts(), viva.sheet.max_col_ts());
    let pasta = &*pasta;
    let sheet = &pasta.aba(indice).expect("acabou de ser achada").sheet;
    let precisa_aspas = |c: char| c == '"' || c == '\n' || c == '\r' || c == separador;
    let escrever_campo = |out: &mut String, campo: &str| {
        if campo.chars().any(precisa_aspas) {
            out.push('"');
            out.push_str(&campo.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(campo);
        }
    };
    let sem_estilo_de_fora = sheet.col_styles.is_empty() && sheet.row_styles.is_empty() && sheet.sheet_style.is_none();
    let mut out = String::new();
    for r in 0..=max_r {
        if r > 0 {
            out.push('\n');
        }
        for c in 0..=max_c {
            if c > 0 {
                out.push(separador);
            }
            // Atalho do literal colunar sem estilo nenhum (nem de coluna,
            // linha ou aba): o texto sai direto do pool, e o número do
            // `format_number` sem estilo — o mesmo que o caminho de baixo.
            let k = key(r, c);
            if sem_estilo_de_fora && !sheet.cells.contains_key(&k) && !sheet.styles.contains_key(&k) {
                match sheet.data.cru_em(r, c) {
                    Some(Cru::Texto(id)) => {
                        escrever_campo(&mut out, feathersheet_core::planilha::data_table::sem_marca(sheet.pool.get(id)));
                        continue;
                    }
                    Some(Cru::Numero(n)) => {
                        escrever_campo(&mut out, &format_number(n, None));
                        continue;
                    }
                    None => {}
                }
            }
            let estilo = sheet.estilo_resolvido(r, c);
            escrever_campo(&mut out, &texto_formatado(&pasta.valor(indice, r, c), estilo.as_ref()));
        }
    }
    Ok(out.into_bytes())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn exportar_lista_as_funcoes_usadas_para_avisar_das_fora_do_excel() {
        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 0, 0, "=SOMA(1;2)".into());
        pasta.escrever_celula(0, 1, 0, "=COUNTUNIQUE(A1:A1)".into());
        let (bytes, usadas) = gerar_xlsx_com_funcoes(&pasta).unwrap();
        assert!(!bytes.is_empty());
        assert!(usadas.contains("SUM") && usadas.contains("COUNTUNIQUE"), "{usadas:?}");
        // Fora da exportação nada é coletado.
        let _ = gerar_xlsx(&pasta).unwrap();
        assert!(FUNCOES_USADAS.with(|f| f.borrow().is_none()));
    }

    #[test]
    fn formato_personalizado_vai_e_volta_do_xlsx() {
        use feathersheet_core::celulas::{refs::Rect, style::format_value};
        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 0, 0, "12".into());
        pasta.escrever_celula(0, 1, 0, "7,25".into());
        for (linha, codigo) in [(0, ";;;"), (1, "0.00\" kWp\"")] {
            let mut e = EstiloParcial::new();
            e.insert("numFmt".into(), Campo::Texto(codigo.into()));
            pasta.editar(0, |s| s.set_style(&Rect::cell(linha, 0), &e));
        }
        let bytes = gerar_xlsx(&pasta).unwrap();
        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let texto = |r| format_value(&lida.valor(0, r, 0),
            lida.aba(0).unwrap().sheet.estilo_resolvido(r, 0).as_ref());
        assert_eq!(texto(0), "");
        assert_eq!(texto(1), "7,25 kWp");
    }

    #[test]
    fn mascara_de_quatro_secoes_vai_e_volta_do_xlsx() {
        use feathersheet_core::celulas::{formato_personalizado::formatar_completo, refs::Rect};
        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 0, 0, "-1234".into());
        let codigo = "#,##0.00_);[Vermelho](#,##0.00);\"-\";[Cor10]@";
        let mut e = EstiloParcial::new();
        e.insert("numFmt".into(), Campo::Texto(codigo.into()));
        pasta.editar(0, |s| s.set_style(&Rect::cell(0, 0), &e));
        let bytes = gerar_xlsx(&pasta).unwrap();
        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let lido = lida.aba(0).unwrap().sheet.estilo_resolvido(0, 0).and_then(|s| s.code).unwrap();
        assert_eq!(lido, "#,##0.00_);[Red](#,##0.00);\"-\";[Color10]@");
        let f = formatar_completo(&lida.valor(0, 0, 0), &lido).unwrap();
        assert_eq!((f.texto.as_str(), f.cor), ("(1.234,00)", Some([255, 0, 0])));
    }

    #[test]
    fn a_referencia_escrita_direto_e_a_do_cell_name() {
        for (r, c) in [(0, 0), (9, 25), (99, 26), (0, 51), (1_048_575, 701), (5, 702), (2_000_000, 16_383)] {
            let mut out = String::new();
            escrever_ref(&mut out, r, c);
            assert_eq!(out, cell_name(r, c));
        }
    }

    #[test]
    fn formula_vai_para_o_ingles_com_a_pontuacao_do_excel() {
        assert_eq!(formula_to_excel("=SOMA(A1:A3;2,5)"), "SUM(A1:A3,2.5)");
        assert_eq!(formula_to_excel("=SOMA(A1; B1)"), "SUM(A1, B1)");
        assert_eq!(formula_to_excel("=SE(A1>1;\"a\"\"b\";FALSO)"), "IF(A1>1,\"a\"\"b\",FALSE)");
        assert_eq!(formula_to_excel("='Meu Plano'!A1+Dados!B2"), "'Meu Plano'!A1+Dados!B2");
        assert_eq!(formula_to_excel("={1\\2;3\\4}"), "{1,2;3,4}");
        assert_eq!(formula_to_excel("=MÉDIA(A1:A2)"), "AVERAGE(A1:A2)");
        // Nome que não é apelido fica como foi escrito.
        assert_eq!(formula_to_excel("=sum(A1)"), "sum(A1)");
    }

    #[test]
    fn nomes_definidos_e_futuras_e_derrame_vao_e_voltam() {
        use feathersheet_core::celulas::refs::Rect;
        let mut pasta = Workbook::new();
        let b = pasta.criar_aba("Meu Plano");
        pasta.escrever_celula(b, 0, 0, "5".into());
        pasta.escrever_celula(b, 1, 0, "7".into());
        let mut area = Rect::new(0, 0, 1, 0);
        area.sheet = Some("Meu Plano".into());
        pasta.definir_nome("Vendas", area);
        pasta.escrever_celula(0, 0, 0, "=SEQUÊNCIA(3)".into());
        pasta.escrever_celula(0, 0, 2, "=PROCX(1;A1:A3;A1:A3)".into());
        let partes = montar_partes(&pasta);
        let texto = |n: &str| String::from_utf8(partes.iter().find(|(k, _)| k == n).unwrap().1.clone()).unwrap();
        assert!(texto("xl/workbook.xml").contains("<definedName name=\"Vendas\">&apos;Meu Plano&apos;!$A$1:$A$2</definedName>")
            || texto("xl/workbook.xml").contains("<definedName name=\"Vendas\">'Meu Plano'!$A$1:$A$2</definedName>"));
        let aba = texto("xl/worksheets/sheet1.xml");
        assert!(aba.contains("cm=\"1\"><f t=\"array\" ref=\"A1:A3\">_xlfn.SEQUENCE(3)</f><v>1</v>"), "{aba}");
        assert!(aba.contains("<c r=\"A2\"><v>2</v></c>") && aba.contains("<v>3</v>"), "{aba}");
        assert!(aba.contains("_xlfn.XLOOKUP("), "{aba}");
        assert!(partes.iter().any(|(k, _)| k == "xl/metadata.xml"));
        let bytes = gerar_xlsx(&pasta).unwrap();
        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let nome = lida.nomeados().into_iter().find(|n| n.nome == "Vendas").unwrap();
        assert_eq!(nome.area.sheet.as_deref(), Some("Meu Plano"));
        let f = lida.aba(0).unwrap().sheet.snapshot(0, 0).unwrap().raw;
        assert!(f.starts_with("=SEQU") && !f.contains("_xlfn"), "{f}");
        let g = lida.aba(0).unwrap().sheet.snapshot(0, 2).unwrap().raw;
        assert!(g.starts_with("=PROCX(") && !g.contains("_xlfn"), "{g}");
    }

    #[test]
    fn funcoes_futuras_levam_prefixo() {
        assert_eq!(formula_to_excel("=PROCX(A1;B:B;C:C)"), "_xlfn.XLOOKUP(A1,B:B,C:C)");
        assert_eq!(formula_to_excel("=FILTRO(A1:A3;B1:B3>1)"), "_xlfn._xlws.FILTER(A1:A3,B1:B3>1)");
        assert_eq!(formula_to_excel("=SOMA(SEQUÊNCIA(3))"), "SUM(_xlfn.SEQUENCE(3))");
        assert_eq!(formula_to_excel("=DEIXAR(x;5;x+1)"), "_xlfn.LET(_xlpm.x,5,_xlpm.x+1)");
    }

    /// O atalho da tradução por padrão tem de dar o mesmo texto da tradução
    /// célula a célula — o `debug_assert` do `formula_empacotada` confere
    /// cada célula; aqui também se confere que o atalho foi mesmo usado.
    #[test]
    fn formula_empacotada_traduz_por_padrao_igual_a_celula_a_celula() {
        let mut pasta = Workbook::new();
        pasta.criar_aba("Meu Plano");
        let formulas = [
            "=A{r}*2,5+B{r}",
            "=SOMA($A$1:A{r}; 1,5)",
            "='Meu Plano'!A{r}+Dados!$B{r}",
            "=SE(A{r}>1;\"A1;x\";FALSO)",
            "=MÉDIA(A{r}:B{r})  &  \"t\"",
            "=A{r}%",
        ];
        {
            let s = &mut pasta.aba_mut(0).unwrap().sheet;
            for r in 0..400u32 {
                s.set_cell(r, 0, &(r * 3).to_string());
                s.set_cell(r, 1, "2");
                for (i, f) in formulas.iter().enumerate() {
                    s.set_cell(r, 2 + i as u32, &f.replace("{r}", &(r + 1).to_string()));
                }
            }
            assert!(s.formulas.padrao_em(200, 2).is_some(), "a fórmula devia estar empacotada");
        }
        let partes = montar_partes(&pasta);
        let aba = String::from_utf8(partes.iter().find(|(n, _)| n == "xl/worksheets/sheet1.xml").unwrap().1.clone()).unwrap();
        assert!(aba.contains("<c r=\"C300\"><f>A300*2.5+B300</f>"), "{}", &aba[..400]);
        assert!(aba.contains("<f>SUM($A$1:A300, 1.5)</f>"));
        assert!(aba.contains("<f>&apos;Meu Plano&apos;!A300+Dados!$B300</f>"));
        assert!(aba.contains("<f>IF(A300&gt;1,&quot;A1;x&quot;,FALSE)</f>"));
        assert!(aba.contains("<f>AVERAGE(A300:B300)  &amp;  &quot;t&quot;</f>"));
    }

    #[test]
    fn formula_que_nao_tokeniza_vai_como_esta() {
        assert_eq!(formula_to_excel("=\"sem fim"), "\"sem fim");
    }

    #[test]
    fn codigo_de_formato_segue_as_casas() {
        let mut e = EstiloParcial::new();
        assert_eq!(codigo_para(&e, None), None);
        assert_eq!(codigo_para(&e, Some("date")).as_deref(), Some("dd/mm/yyyy"));
        e.insert("decimals".into(), Campo::Numero(2.0));
        assert_eq!(codigo_para(&e, None).as_deref(), Some("#,##0.00"));
        e.insert("format".into(), Campo::Texto("percent".into()));
        assert_eq!(codigo_para(&e, Some("date")).as_deref(), Some("0.00%"));
        e.insert("decimals".into(), Campo::Numero(0.0));
        assert_eq!(codigo_para(&e, None).as_deref(), Some("0%"));
    }

    #[test]
    fn cor_css_vira_argb() {
        assert_eq!(para_argb(Some("#ff8800".into())).as_deref(), Some("FFFF8800"));
        assert_eq!(para_argb(Some("#abc".into())).as_deref(), Some("FFAABBCC"));
        assert_eq!(para_argb(Some("red".into())), None);
    }

    #[test]
    fn escape_tira_caractere_de_controle() {
        assert_eq!(escape_xml("a&b<'\"\u{1}\t"), "a&amp;b&lt;&apos;&quot;\t");
    }

    #[test]
    fn csv_leva_aspas_so_quando_precisa() {
        let mut pasta = Workbook::new();
        let s = &mut pasta.aba_mut(0).unwrap().sheet;
        s.set_cell(0, 0, "a,b");
        s.set_cell(0, 1, "12");
        s.set_cell(1, 0, "diz \"oi\"");
        s.set_cell(1, 1, "=B1*2");
        let csv = String::from_utf8(gerar_delimitado(&mut pasta, 0, ',').unwrap()).unwrap();
        assert_eq!(csv, "\"a,b\",12\n\"diz \"\"oi\"\"\",24");
        let tsv = String::from_utf8(gerar_delimitado(&mut pasta, 0, '\t').unwrap()).unwrap();
        assert_eq!(tsv, "a,b\t12\n\"diz \"\"oi\"\"\"\t24");
    }
}
