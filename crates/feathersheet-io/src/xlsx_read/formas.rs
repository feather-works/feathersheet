//! Formas com texto dos desenhos das abas (`xl/drawings/drawingN.xml`): os
//! retângulos que fazem papel de botão de macro no Excel. Só o que o
//! FeatherSheet usa para recriar o botão: aba, nome, texto, cor, posição e a
//! macro, quando o arquivo ainda guarda (`<xdr:sp macro="...">`).
use super::{entry_text, sheet::read_rels, xml::{parse_xml, XmlNode}};
use std::io::Cursor;
use zip::ZipArchive;

/// EMU por pixel (96 dpi).
const EMU_POR_PX: f64 = 9525.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Forma {
    /// Índice da aba, na ordem do arquivo.
    pub aba: usize,
    pub nome: String,
    pub texto: String,
    /// Canto de cima à esquerda: célula âncora e deslocamento em pixels.
    pub coluna: u32,
    pub dx: f64,
    pub linha: u32,
    pub dy: f64,
    pub largura: f64,
    pub altura: f64,
    /// Cor de preenchimento `#RRGGBB`, se for uma cor fixa.
    pub cor: Option<String>,
    /// Macro atribuída pelo Excel (`[0]!Modulo.Macro` → `Macro`).
    pub macro_: Option<String>,
}

/// Junta `../drawings/drawing1.xml` a `xl/worksheets/` como o zip guarda.
pub(super) fn caminho_relativo(base: &str, alvo: &str) -> String {
    if let Some(abs) = alvo.strip_prefix('/') { return abs.to_string(); }
    let mut partes: Vec<&str> = base.split('/').collect();
    partes.pop();
    for p in alvo.split('/') {
        match p { ".." => { partes.pop(); } "." | "" => {} _ => partes.push(p) }
    }
    partes.join("/")
}

fn textos(no: &XmlNode, saida: &mut String) {
    if no.name == "t" { saida.push_str(&no.text); }
    if no.name == "p" && !saida.is_empty() { saida.push('\n'); }
    for c in &no.children { textos(c, saida); }
}

fn numero(no: Option<&XmlNode>) -> f64 {
    no.and_then(|n| n.text.trim().parse::<f64>().ok()).unwrap_or(0.0)
}

fn ler_ancora(a: &XmlNode, aba: usize) -> Option<Forma> {
    let sp = a.child("sp")?;
    let de = a.child("from")?;
    let nv = sp.child("nvSpPr").and_then(|n| n.child("cNvPr"));
    let mut texto = String::new();
    if let Some(corpo) = sp.child("txBody") { textos(corpo, &mut texto); }
    let texto = texto.trim().to_string();
    // oneCellAnchor tem <ext>; twoCellAnchor, o <xfrm><a:ext> da forma.
    let ext = a.child("ext").or_else(|| sp.child("spPr")?.child("xfrm")?.child("ext"));
    let medida = |n: &str| ext.and_then(|e| e.attr(n)).and_then(|v| v.parse::<f64>().ok())
        .map(|v| v / EMU_POR_PX).unwrap_or(0.0);
    let cor = preenchimento(sp).flatten();
    let macro_ = sp.attr("macro").map(|m| m.rsplit(['!', '.']).next().unwrap_or(m).to_string())
        .filter(|m| !m.is_empty());
    Some(Forma {
        aba,
        nome: nv.and_then(|n| n.attr("name")).unwrap_or("").to_string(),
        texto,
        coluna: numero(de.child("col")) as u32,
        dx: numero(de.child("colOff")) / EMU_POR_PX,
        linha: numero(de.child("row")) as u32,
        dy: numero(de.child("rowOff")) / EMU_POR_PX,
        largura: medida("cx"),
        altura: medida("cy"),
        cor,
        macro_,
    })
}

/// As formas de todas as abas. Arquivo sem desenho dá lista vazia; falha
/// num desenho só deixa aquele de fora.
pub fn ler_formas(bytes: &[u8]) -> Vec<Forma> {
    let Ok(mut zip) = ZipArchive::new(Cursor::new(std::sync::Arc::<[u8]>::from(bytes))) else { return Vec::new() };
    // O tema do arquivo resolve `schemeClr`/`fillRef` da cor do botão.
    let tema = entry_text(&mut zip, "xl/theme/theme1.xml").unwrap_or_default();
    let _paleta = super::cores::instalar(&tema, "");
    let mut saida = Vec::new();
    para_cada_ancora(&mut zip, |i, a| {
        if let Some(f) = ler_ancora(a, i) { saida.push(f); }
    });
    saida
}

/// Visita cada `*CellAnchor` dos desenhos das abas, com o índice da aba.
fn para_cada_ancora(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, mut f: impl FnMut(usize, &XmlNode)) {
    let Ok(book) = entry_text(zip, "xl/workbook.xml").and_then(|t| parse_xml(&t)) else { return };
    let rels_book = entry_text(zip, "xl/_rels/workbook.xml.rels").map(|t| read_rels(&t)).unwrap_or_default();
    let Some(abas) = book.child("sheets") else { return };
    for (i, aba) in abas.children_named("sheet").enumerate() {
        let Some(alvo) = aba.attr("r:id").and_then(|id| rels_book.get(id)) else { continue };
        let folha = caminho_relativo("xl/workbook.xml", alvo);
        let (dir, arq) = folha.rsplit_once('/').unwrap_or(("", &folha));
        let Ok(rels) = entry_text(zip, &format!("{dir}/_rels/{arq}.rels")) else { continue };
        let rels_xml = parse_xml(&rels).ok();
        let desenhos: Vec<String> = rels_xml.iter().flat_map(|r| r.children_named("Relationship"))
            .filter(|r| r.attr("Type").is_some_and(|t| t.ends_with("/drawing")))
            .filter_map(|r| r.attr("Target")).map(|t| caminho_relativo(&folha, t)).collect();
        for d in desenhos {
            let Ok(x) = entry_text(zip, &d).and_then(|t| parse_xml(&t)) else { continue };
            for a in x.children.iter().filter(|c| c.name.ends_with("CellAnchor")) {
                f(i, a);
            }
        }
    }
}

/// Cor de preenchimento de `spPr` (sólida, ou a do meio do degradê); sem
/// `spPr` próprio, a do `fillRef` do estilo da forma. `Some(None)` = sem
/// preenchimento declarado.
fn preenchimento(sp: &XmlNode) -> Option<Option<String>> {
    let pr = sp.child("spPr");
    if let Some(pr) = pr {
        if pr.child("noFill").is_some() { return Some(None); }
        if let Some(s) = pr.child("solidFill") { return Some(super::cores::cor_drawingml(s)); }
        if let Some(g) = pr.child("gradFill").and_then(|g| g.child("gsLst")) {
            let paradas: Vec<&XmlNode> = g.children_named("gs").collect();
            return Some(paradas.get(paradas.len() / 2).and_then(|p| super::cores::cor_drawingml(p)));
        }
    }
    let r = sp.child("style")?.child("fillRef")?;
    if r.attr("idx") == Some("0") { return Some(None); }
    Some(super::cores::cor_drawingml(r))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminho_do_desenho_a_partir_da_aba() {
        assert_eq!(caminho_relativo("xl/worksheets/sheet1.xml", "../drawings/drawing1.xml"), "xl/drawings/drawing1.xml");
        assert_eq!(caminho_relativo("xl/workbook.xml", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(caminho_relativo("xl/workbook.xml", "/xl/worksheets/sheet2.xml"), "xl/worksheets/sheet2.xml");
    }

    #[test]
    fn forma_com_texto_cor_e_macro() {
        let x = parse_xml(r#"<xdr:wsDr xmlns:xdr="x" xmlns:a="a"><xdr:oneCellAnchor>
            <xdr:from><xdr:col>2</xdr:col><xdr:colOff>9525</xdr:colOff><xdr:row>20</xdr:row><xdr:rowOff>19050</xdr:rowOff></xdr:from>
            <xdr:ext cx="1133475" cy="419100"/>
            <xdr:sp macro="[0]!Modulo1.Botao1_Clique"><xdr:nvSpPr><xdr:cNvPr id="3" name="Shape 3"/></xdr:nvSpPr>
              <xdr:spPr><a:solidFill><a:srgbClr val="548135"/></a:solidFill></xdr:spPr>
              <xdr:txBody><a:p><a:r><a:t>Procuração</a:t></a:r></a:p></xdr:txBody></xdr:sp>
            <xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>"#).unwrap();
        let f = ler_ancora(&x.children[0], 0).unwrap();
        assert_eq!((f.nome.as_str(), f.texto.as_str(), f.coluna, f.linha), ("Shape 3", "Procuração", 2, 20));
        assert_eq!((f.dx, f.dy, f.largura.round(), f.altura.round()), (1.0, 2.0, 119.0, 44.0));
        assert_eq!(f.cor.as_deref(), Some("#548135"));
        assert_eq!(f.macro_.as_deref(), Some("Botao1_Clique"));
    }
}
