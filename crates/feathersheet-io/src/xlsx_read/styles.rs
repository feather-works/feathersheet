//! `styles.xml` -> `Style` do motor (`core/style.ts:20-51`). Espelha
//! `readStyles` de `src/io/xlsx.ts:840-911`.

use super::format::{builtin_code, format_from_code};
use super::xml::XmlNode;
use serde::Serialize;
use std::collections::HashMap;

const PX_TO_PT: f64 = 0.75;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Borders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Estilo da linha (`medium`, `thick`, `dashed`…) já traduzido para o
    /// `borderLineStyle` do motor. Vai para o `Style`, não para `borders`.
    #[serde(skip)]
    pub estilo_linha: Option<&'static str>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Style {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// `<u val="double"/>` (ou `doubleAccounting`): sublinhado duplo.
    #[serde(rename = "doubleUnderline", skip_serializing_if = "Option::is_none")]
    pub double_underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    /// `<vertAlign val="superscript|subscript"/>` da fonte (a célula inteira, como o app faz).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superscript: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscript: Option<bool>,
    #[serde(rename = "fontSize", skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i64>,
    #[serde(rename = "fontFamily", skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<Borders>,
    /// Fina (ausente), Média, Grossa ou Tracejada.
    #[serde(rename = "borderLineStyle", skip_serializing_if = "Option::is_none")]
    pub border_line_style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<&'static str>,
    #[serde(rename = "vAlign", skip_serializing_if = "Option::is_none")]
    pub v_align: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// Alinhamento horizontal do Excel que o app não desenha igual (`centerContinuous`, `fill`,
    /// `justify`, `distributed`): a tela usa o mais próximo em `align` e a gravação devolve este.
    #[serde(rename = "alignExcel", skip_serializing_if = "Option::is_none")]
    pub align_excel: Option<&'static str>,
    /// `<alignment shrinkToFit="1"/>`: Reduzir para caber.
    #[serde(rename = "shrinkToFit", skip_serializing_if = "Option::is_none")]
    pub shrink_to_fit: Option<bool>,
    /// `textRotation="255"`: texto vertical empilhado (uma letra embaixo da outra).
    #[serde(rename = "textVertical", skip_serializing_if = "Option::is_none")]
    pub text_vertical: Option<bool>,
    /// `<alignment indent="n"/>`: nível de recuo do texto (0..=15).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<u32>,
    /// Endereço do hiperlink da célula (`<hyperlinks>` da aba), o `link` do
    /// `style.ts`. Não vem do `styles.xml`: quem preenche é o `read_sheet`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// `<protection locked="0" hidden="1"/>` do `xf`: só o que foge do padrão.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Código do formato personalizado (`numFmtId` 164 em diante), como o
    /// Excel grava: `#,##0.00" kWp"`, `;;;`.
    #[serde(rename = "numFmt", skip_serializing_if = "Option::is_none")]
    pub num_fmt: Option<String>,
}

impl Style {
    pub fn has_any(&self) -> bool {
        self.bold.is_some()
            || self.locked.is_some()
            || self.hidden.is_some()
            || self.italic.is_some()
            || self.underline.is_some()
            || self.strike.is_some()
            || self.font_size.is_some()
            || self.font_family.is_some()
            || self.decimals.is_some()
            || self.num_fmt.is_some()
            || self.borders.is_some()
            || self.border_line_style.is_some()
            || self.double_underline.is_some()
            || self.superscript.is_some()
            || self.subscript.is_some()
            || self.align.is_some()
            || self.v_align.is_some()
            || self.wrap.is_some()
            || self.format.is_some()
            || self.color.is_some()
            || self.bg.is_some()
            || self.rotation.is_some()
            || self.indent.is_some()
            || self.align_excel.is_some()
            || self.shrink_to_fit.is_some()
            || self.text_vertical.is_some()
            || self.link.is_some()
    }
}

pub struct XfStyle {
    pub style: Style,
    pub format: &'static str,
    /// Só nas entradas que vêm de `<dxfs>` (estilo diferencial da formatação
    /// condicional). Elas ficam DEPOIS dos `cellXfs` neste mesmo vetor, para
    /// não mudar a assinatura de `read_sheet`; ver [`dxf_do_indice`].
    pub dxf: Option<feathersheet_core::operacoes::rules::CondStyle>,
}

/// O estilo do `dxfId` N (a N-ésima entrada de `<dxfs>`).
pub fn dxf_do_indice(xfs: &[XfStyle], id: usize) -> Option<feathersheet_core::operacoes::rules::CondStyle> {
    let base = xfs.iter().position(|x| x.dxf.is_some())?;
    xfs.get(base + id)?.dxf.clone()
}

/// Cor de `<color>`/`<fgColor>`/`<bgColor>`: `rgb`, `theme` (do
/// `theme1.xml` do arquivo, ver [`super::cores`]) ou `indexed` (paleta
/// legada), com o `tint` na luminância HSL como o Excel. `auto="1"` e
/// `indexed="64"` (cor automática do sistema) não têm cor própria.
pub fn cor_de(no: &XmlNode) -> Option<String> {
    use super::cores;
    if let Some(c) = from_argb(no.attr("rgb")) {
        return Some(match no.attr("tint").and_then(|t| t.parse::<f64>().ok()) {
            Some(t) => cores::hex(cores::aplicar_tint(u32::from_str_radix(&c[1..], 16).ok()?, t)),
            None => c,
        });
    }
    let base = if let Some(t) = no.attr("theme") {
        cores::do_tema(t.parse().ok()?)?
    } else {
        let i: usize = no.attr("indexed")?.parse().ok()?;
        if i >= 64 {
            return None;
        }
        cores::indexada(i)?
    };
    let tint = no.attr("tint").and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.0);
    Some(cores::hex(cores::aplicar_tint(base, tint)))
}

fn liga(no: &XmlNode) -> bool {
    !matches!(no.attr("val"), Some("0") | Some("false"))
}

/// `<dxf>`: fonte (cor, negrito, itálico, sublinhado, tachado) e
/// preenchimento. Borda e formato de número não existem no estilo da regra.
pub fn ler_dxf(dxf: &XmlNode) -> feathersheet_core::operacoes::rules::CondStyle {
    let mut s = feathersheet_core::operacoes::rules::CondStyle::default();
    if let Some(f) = dxf.child("font") {
        if let Some(n) = f.child("b") { s.bold = Some(liga(n)); }
        if let Some(n) = f.child("i") { s.italic = Some(liga(n)); }
        if let Some(n) = f.child("strike") { s.strike = Some(liga(n)); }
        if let Some(n) = f.child("u") { s.underline = Some(n.attr("val") != Some("none")); }
        s.color = f.child("color").and_then(cor_de);
    }
    if let Some(p) = dxf.child("fill").and_then(|f| f.child("patternFill")) {
        if p.attr("patternType") != Some("none") {
            // No `dxf` o preenchimento sólido aparece na `bgColor`.
            s.bg = p.child("bgColor").and_then(cor_de).or_else(|| p.child("fgColor").and_then(cor_de));
        }
    }
    s
}

fn from_argb(argb: Option<&str>) -> Option<String> {
    let a = argb?;
    let valido = (6..=8).contains(&a.len()) && a.chars().all(|c| c.is_ascii_hexdigit());
    if !valido {
        return None;
    }
    let rgb = if a.len() == 8 { &a[2..] } else { a };
    Some(format!("#{}", rgb.to_lowercase()))
}

pub fn read_styles(xml: &str) -> Vec<XfStyle> {
    if xml.is_empty() {
        return Vec::new();
    }
    let root = match super::xml::parse_xml(xml) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut codes: HashMap<i64, String> = HashMap::new();
    for id in 0..=49u32 {
        if let Some(c) = builtin_code(id) {
            codes.insert(id as i64, c.to_string());
        }
    }
    if let Some(num_fmts) = root.child("numFmts") {
        for f in num_fmts.children_named("numFmt") {
            if let Some(id) = f.attr("numFmtId").and_then(|s| s.parse::<i64>().ok()) {
                codes.insert(id, f.attr("formatCode").unwrap_or("").to_string());
            }
        }
    }

    let fonts: Vec<Style> = root
        .child("fonts")
        .map(|n| n.children_named("font").map(read_font).collect())
        .unwrap_or_default();

    let fills: Vec<Option<String>> = root
        .child("fills")
        .map(|n| n.children_named("fill").map(read_fill).collect())
        .unwrap_or_default();

    let borders: Vec<Option<Borders>> = root
        .child("borders")
        .map(|n| n.children_named("border").map(read_border).collect())
        .unwrap_or_default();

    let mut xfs: Vec<XfStyle> = root
        .child("cellXfs")
        .map(|n| {
            n.children_named("xf")
                .map(|xf| read_xf(xf, &codes, &fonts, &fills, &borders))
                .collect()
        })
        .unwrap_or_default();
    if let Some(dxfs) = root.child("dxfs") {
        for d in dxfs.children_named("dxf") {
            xfs.push(XfStyle { style: Style::default(), format: "general", dxf: Some(ler_dxf(d)) });
        }
    }
    xfs
}

fn read_font(f: &XmlNode) -> Style {
    let mut style = Style::default();
    // `<b val="0"/>` desliga (o Excel grava assim ao tirar o negrito).
    if f.child("b").is_some_and(liga) {
        style.bold = Some(true);
    }
    if f.child("i").is_some_and(liga) {
        style.italic = Some(true);
    }
    if let Some(u) = f.child("u").filter(|u| !matches!(u.attr("val"), Some("none"))) {
        style.underline = Some(true);
        if matches!(u.attr("val"), Some("double") | Some("doubleAccounting")) {
            style.double_underline = Some(true);
        }
    }
    if f.child("strike").is_some_and(liga) {
        style.strike = Some(true);
    }
    match f.child("vertAlign").and_then(|v| v.attr("val")) {
        Some("superscript") => style.superscript = Some(true),
        Some("subscript") => style.subscript = Some(true),
        _ => {}
    }
    if let Some(size) = f.child("sz").and_then(|n| n.attr("val")).and_then(|v| v.parse::<f64>().ok()) {
        if size > 0.0 && (size - 11.0).abs() > 0.01 {
            style.font_size = Some((size / PX_TO_PT).round() as i64);
        }
    }
    // Preto puro é a cor automática do texto: não vira estilo, para o tema
    // escuro do aplicativo continuar mostrando o texto claro.
    if let Some(color) = f.child("color").and_then(cor_de).filter(|c| c != "#000000") {
        style.color = Some(color);
    }
    if let Some(name) = f.child("name").and_then(|n| n.attr("val")) {
        if name != "Calibri" {
            style.font_family = Some(name.to_string());
        }
    }
    style
}

fn read_fill(f: &XmlNode) -> Option<String> {
    let pattern = f.child("patternFill")?;
    if matches!(pattern.attr("patternType"), Some("none")) {
        return None;
    }
    // Sólido e hachuras usam a cor da frente; sem ela (ou automática), a de
    // fundo. Hachura vira a cor sólida da frente (o motor não tem padrões).
    pattern.child("fgColor").and_then(cor_de)
        .or_else(|| pattern.child("bgColor").and_then(cor_de))
}

fn read_border(b: &XmlNode) -> Option<Borders> {
    let mut out = Borders::default();
    let mut has = false;
    for side in ["left", "right", "top", "bottom"] {
        if let Some(node) = b.child(side) {
            if let Some(s) = node.attr("style") {
                if s != "none" {
                    has = true;
                    // O traço mais forte manda (o motor guarda um por célula).
                    let traco = match s {
                        "thick" | "double" => Some("Grossa"),
                        "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" => Some("Média"),
                        // `hair` é a linha contínua mais fina do Excel (a do LibreOffice para 0,5 pt): fica Fina.
                        "dashed" | "dotted" | "dashDot" | "dashDotDot" | "slantDashDot" => Some("Tracejada"),
                        _ => None,
                    };
                    let peso = |t: Option<&str>| match t { Some("Grossa") => 3, Some("Média") => 2, Some("Tracejada") => 1, _ => 0 };
                    if peso(traco) > peso(out.estilo_linha) { out.estilo_linha = traco; }
                    match side {
                        "left" => out.left = Some(true),
                        "right" => out.right = Some(true),
                        "top" => out.top = Some(true),
                        "bottom" => out.bottom = Some(true),
                        _ => unreachable!(),
                    }
                    if out.color.is_none() {
                        if let Some(color) = node.child("color").and_then(cor_de).filter(|c| c != "#000000") {
                            out.color = Some(color);
                        }
                    }
                }
            }
        }
    }
    has.then_some(out)
}

fn read_xf(xf: &XmlNode, codes: &HashMap<i64, String>, fonts: &[Style], fills: &[Option<String>], borders: &[Option<Borders>]) -> XfStyle {
    let font_id = xf.attr("fontId").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let mut style = fonts.get(font_id).cloned().unwrap_or_default();

    let fill_id = xf.attr("fillId").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    if let Some(Some(bg)) = fills.get(fill_id) {
        style.bg = Some(bg.clone());
    }

    let border_id = xf.attr("borderId").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    if let Some(Some(border)) = borders.get(border_id) {
        style.borders = Some(border.clone());
        style.border_line_style = border.estilo_linha;
    }

    let num_fmt_id = xf.attr("numFmtId").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let code = codes.get(&num_fmt_id).cloned().unwrap_or_else(|| "General".to_string());
    let parsed = format_from_code(&code);
    // Formato do próprio arquivo: o código vai junto para a célula mostrar
    // exatamente como no Excel (o tipo acima continua valendo para o resto).
    if num_fmt_id >= 164 && !code.trim().is_empty() && !code.eq_ignore_ascii_case("general") {
        style.num_fmt = Some(code.clone());
    }
    if parsed.format != "auto" {
        style.format = Some(parsed.format);
    }
    if let Some(d) = parsed.decimals {
        if d > 0 {
            style.decimals = Some(d);
        }
    }

    if let Some(p) = xf.child("protection") {
        if matches!(p.attr("locked"), Some("0") | Some("false")) { style.locked = Some(false); }
        if matches!(p.attr("hidden"), Some("1") | Some("true")) { style.hidden = Some(true); }
    }
    if let Some(align) = xf.child("alignment") {
        match align.attr("horizontal") {
            Some("left") => style.align = Some("left"),
            Some("center") => style.align = Some("center"),
            Some("right") => style.align = Some("right"),
            // shortcut: Centralizar na seleção centraliza só na célula; desenhar através das vizinhas vazias se pedirem.
            Some("centerContinuous") => { style.align = Some("center"); style.align_excel = Some("centerContinuous") }
            Some("distributed") => { style.align = Some("center"); style.align_excel = Some("distributed") }
            Some("fill") => { style.align = Some("left"); style.align_excel = Some("fill") }
            Some("justify") => { style.align = Some("left"); style.align_excel = Some("justify") }
            _ => {}
        }
        match align.attr("vertical") {
            Some("top") => style.v_align = Some("top"),
            Some("bottom") => style.v_align = Some("bottom"),
            Some("center") => style.v_align = Some("middle"),
            _ => {}
        }
        if matches!(align.attr("wrapText"), Some("1") | Some("true")) {
            style.wrap = Some(true);
        }
        if let Some(n) = align.attr("indent").and_then(|s| s.parse::<u32>().ok()).filter(|n| *n > 0) {
            style.indent = Some(n.min(15));
        }
        if matches!(align.attr("shrinkToFit"), Some("1") | Some("true")) {
            style.shrink_to_fit = Some(true);
        }
        if let Some(rot) = align.attr("textRotation").and_then(|s| s.parse::<f64>().ok()) {
            // 255 não é ângulo: é o texto vertical empilhado (antes virava um giro de -165°).
            if rot == 255.0 {
                style.text_vertical = Some(true);
            } else if rot != 0.0 {
                style.rotation = Some(if rot > 90.0 { 90.0 - rot } else { rot });
            }
        }
    }

    XfStyle { style, format: parsed.format, dxf: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_argb_tira_o_alfa() {
        assert_eq!(from_argb(Some("FFFF0000")), Some("#ff0000".to_string()));
        assert_eq!(from_argb(Some("00FF00")), Some("#00ff00".to_string()));
        assert_eq!(from_argb(None), None);
        assert_eq!(from_argb(Some("xyz")), None);
    }

    #[test]
    fn reduzir_para_caber_e_texto_vertical() {
        let xml = r#"<styleSheet><cellXfs>
            <xf><alignment shrinkToFit="1"/></xf>
            <xf><alignment textRotation="255"/></xf>
            <xf><alignment textRotation="135"/></xf>
          </cellXfs></styleSheet>"#;
        let xfs = read_styles(xml);
        assert_eq!((xfs[0].style.shrink_to_fit, xfs[0].style.text_vertical), (Some(true), None));
        assert_eq!((xfs[1].style.text_vertical, xfs[1].style.rotation), (Some(true), None), "255 é vertical, não giro");
        assert_eq!(xfs[2].style.rotation, Some(-45.0));
        let json = serde_json::to_value(&xfs[0].style).unwrap();
        assert_eq!(json["shrinkToFit"], true);
    }

    #[test]
    fn sobrescrito_e_subscrito_da_fonte() {
        let xml = r#"<styleSheet><fonts><font><vertAlign val="superscript"/></font><font><vertAlign val="subscript"/></font></fonts>
            <cellXfs><xf fontId="0"/><xf fontId="1"/></cellXfs></styleSheet>"#;
        let xfs = read_styles(xml);
        assert_eq!((xfs[0].style.superscript, xfs[0].style.subscript), (Some(true), None));
        assert_eq!((xfs[1].style.superscript, xfs[1].style.subscript), (None, Some(true)));
    }

    #[test]
    fn centralizar_na_selecao_fica_guardado() {
        let xml = r#"<styleSheet><cellXfs><xf><alignment horizontal="centerContinuous"/></xf><xf><alignment horizontal="fill"/></xf></cellXfs></styleSheet>"#;
        let xfs = read_styles(xml);
        assert_eq!((xfs[0].style.align, xfs[0].style.align_excel), (Some("center"), Some("centerContinuous")));
        assert_eq!((xfs[1].style.align, xfs[1].style.align_excel), (Some("left"), Some("fill")));
    }

    #[test]
    fn le_estilo_completo() {
        let xml = r#"<styleSheet>
          <numFmts><numFmt numFmtId="164" formatCode="0.00%"/></numFmts>
          <fonts><font><b/><color rgb="FFFF0000"/></font></fonts>
          <fills><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills>
          <borders><border><left style="thin"/></border></borders>
          <cellXfs>
            <xf fontId="0" fillId="0" borderId="0" numFmtId="164">
              <alignment horizontal="center" vertical="center" wrapText="1"/>
            </xf>
          </cellXfs>
        </styleSheet>"#;
        let xfs = read_styles(xml);
        assert_eq!(xfs.len(), 1);
        let s = &xfs[0].style;
        assert_eq!(s.bold, Some(true));
        assert_eq!(s.color.as_deref(), Some("#ff0000"));
        assert_eq!(s.bg.as_deref(), Some("#ffff00"));
        assert_eq!(s.borders.as_ref().unwrap().left, Some(true));
        assert_eq!(s.align, Some("center"));
        assert_eq!(s.v_align, Some("middle"));
        assert_eq!(s.wrap, Some(true));
        assert_eq!(s.format, Some("percent"));
    }

    #[test]
    fn borda_hair_fica_fina_e_nao_tracejada() {
        let xml = |estilo: &str| format!(r#"<styleSheet><borders><border><left style="{estilo}"/></border></borders><cellXfs><xf borderId="0"/></cellXfs></styleSheet>"#);
        let linha = |estilo: &str| read_styles(&xml(estilo))[0].style.borders.as_ref().unwrap().estilo_linha;
        assert_eq!(linha("hair"), None);
        assert_eq!(linha("dotted"), Some("Tracejada"));
    }

    #[test]
    fn preenchimento_e_fonte_por_tema_indexada_e_negrito_desligado() {
        let tema = r#"<a:theme xmlns:a="a"><a:themeElements><a:clrScheme name="Office">
          <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
          <a:dk2><a:srgbClr val="1F497D"/></a:dk2><a:lt2><a:srgbClr val="EEECE1"/></a:lt2>
          <a:accent1><a:srgbClr val="4F81BD"/></a:accent1><a:accent2><a:srgbClr val="C0504D"/></a:accent2>
          <a:accent3><a:srgbClr val="9BBB59"/></a:accent3></a:clrScheme></a:themeElements></a:theme>"#;
        let xml = r#"<styleSheet>
          <fonts><font><b val="0"/><color theme="1"/></font><font><b/><color indexed="10"/></font><font><color theme="0"/></font></fonts>
          <fills><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>
            <fill><patternFill patternType="solid"><fgColor theme="6" tint="0.59999389629810485"/><bgColor indexed="64"/></patternFill></fill>
            <fill><patternFill patternType="solid"><fgColor theme="1"/><bgColor indexed="64"/></patternFill></fill>
            <fill><patternFill patternType="solid"><fgColor indexed="13"/></patternFill></fill></fills>
          <borders><border><left style="thin"><color theme="5"/></left></border></borders>
          <cellXfs><xf fontId="0" fillId="2" borderId="0"/><xf fontId="1" fillId="3"/><xf fontId="2" fillId="4"/><xf fontId="0" fillId="1"/></cellXfs>
        </styleSheet>"#;
        let _g = super::super::cores::instalar(tema, xml);
        let xfs = read_styles(xml);
        // Tema 6 = accent3 do arquivo (9BBB59), +60 % na luminância.
        assert_eq!(xfs[0].style.bg.as_deref(), Some("#d7e4bd"));
        assert_eq!(xfs[0].style.bold, None, "<b val=\"0\"/> não é negrito");
        assert_eq!(xfs[0].style.color, None, "texto automático (preto) não vira cor");
        assert_eq!(xfs[0].style.borders.as_ref().and_then(|b| b.color.as_deref()), Some("#c0504d"));
        assert_eq!((xfs[1].style.bg.as_deref(), xfs[1].style.color.as_deref(), xfs[1].style.bold), (Some("#000000"), Some("#ff0000"), Some(true)));
        assert_eq!((xfs[2].style.bg.as_deref(), xfs[2].style.color.as_deref()), (Some("#ffff00"), Some("#ffffff")));
        assert_eq!(xfs[3].style.bg, None, "gray125 sem cor não pinta");
    }
}
