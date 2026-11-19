//! Como cada aba aparecia no Excel (`<sheetView>`): linhas de grade e
//! painéis congelados. Não é dado da planilha (o motor não guarda), e sim
//! estado da janela: a GUI aplica ao abrir. Lê só o começo de cada
//! `sheetN.xml`, até o `<sheetData>`, para não percorrer aba grande de novo.

use super::formas::caminho_relativo;
use super::refs::parse_cell_ref;
use super::sheet::read_rels;
use super::xml::parse_xml;
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive;

/// O `<sheetView>` de uma aba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VistaAba {
    /// `showGridLines` (padrão: ligado).
    pub linhas_grade: bool,
    /// Painel congelado: linhas e colunas fixas (`<pane state="frozen">`).
    pub congelar: (u32, u32),
    /// `showZeros` (padrão: ligado); desligado, célula com zero aparece em branco.
    pub zeros: bool,
}

impl Default for VistaAba {
    fn default() -> Self {
        Self { linhas_grade: true, congelar: (0, 0), zeros: true }
    }
}

/// Teto do trecho lido antes do `<sheetData>`.
const LIMITE_CABECALHO: usize = 1024 * 1024;

fn cabecalho<R: Read + Seek>(zip: &mut ZipArchive<R>, caminho: &str) -> Option<String> {
    let mut f = zip.by_name(caminho).ok()?;
    let mut buf = Vec::new();
    let mut bloco = [0u8; 16 * 1024];
    loop {
        let n = f.read(&mut bloco).ok()?;
        if n == 0 { break; }
        buf.extend_from_slice(&bloco[..n]);
        if let Some(p) = buf.windows(10).position(|w| w == b"<sheetData") {
            buf.truncate(p);
            break;
        }
        if buf.len() > LIMITE_CABECALHO { return None; }
    }
    let mut texto = String::from_utf8_lossy(&buf).into_owned();
    if !texto.contains("</worksheet>") {
        texto.push_str("</worksheet>");
    }
    Some(texto)
}

/// O `<sheetView>` de um trecho de `sheetN.xml`.
pub fn vista_do_xml(xml: &str) -> VistaAba {
    let mut vista = VistaAba::default();
    let Some(raiz) = parse_xml(xml).ok() else { return vista };
    let Some(v) = raiz.child("sheetViews").and_then(|s| s.child("sheetView")) else { return vista };
    vista.linhas_grade = !matches!(v.attr("showGridLines"), Some("0") | Some("false"));
    vista.zeros = !matches!(v.attr("showZeros"), Some("0") | Some("false"));
    if let Some(p) = v.child("pane").filter(|p| matches!(p.attr("state"), Some("frozen") | Some("frozenSplit"))) {
        let n = |a: &str| p.attr(a).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0).max(0.0) as u32;
        let (mut linhas, mut colunas) = (n("ySplit"), n("xSplit"));
        // Sem os números, o canto de baixo à direita diz onde o painel corta.
        if linhas == 0 && colunas == 0 {
            if let Some(c) = p.attr("topLeftCell").and_then(parse_cell_ref) {
                linhas = c.row.max(0) as u32;
                colunas = c.col.max(0) as u32;
            }
        }
        vista.congelar = (linhas, colunas);
    }
    vista
}

fn texto_inteiro<R: Read + Seek>(zip: &mut ZipArchive<R>, caminho: &str) -> Option<String> {
    let f = zip.by_name(caminho).ok()?;
    let mut t = String::new();
    f.take(super::LIMITE_ENTRADA_XML).read_to_string(&mut t).ok()?;
    Some(t)
}

fn ler_vistas<R: Read + Seek>(mut zip: ZipArchive<R>) -> Vec<VistaAba> {
    let Some(book) = texto_inteiro(&mut zip, "xl/workbook.xml").and_then(|t| parse_xml(&t).ok()) else { return Vec::new() };
    let rels = texto_inteiro(&mut zip, "xl/_rels/workbook.xml.rels").map(|t| read_rels(&t)).unwrap_or_default();
    let Some(abas) = book.child("sheets") else { return Vec::new() };
    let caminhos: Vec<Option<String>> = abas.children_named("sheet")
        .map(|a| a.attr("r:id").and_then(|id| rels.get(id)).map(|alvo| caminho_relativo("xl/workbook.xml", alvo)))
        .collect();
    caminhos.into_iter()
        .map(|c| c.and_then(|c| cabecalho(&mut zip, &c)).map(|x| vista_do_xml(&x)).unwrap_or_default())
        .collect()
}

/// Uma vista por aba, na ordem do `workbook.xml`.
pub fn ler_vistas_bytes(bytes: Vec<u8>) -> Vec<VistaAba> {
    ZipArchive::new(Cursor::new(bytes)).map(ler_vistas).unwrap_or_default()
}

/// [`ler_vistas_bytes`] direto do arquivo, sem carregá-lo inteiro.
pub fn ler_vistas_arquivo(caminho: &std::path::Path) -> Vec<VistaAba> {
    std::fs::File::open(caminho).ok()
        .and_then(|f| ZipArchive::new(std::io::BufReader::new(f)).ok())
        .map(ler_vistas).unwrap_or_default()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grade_desligada_e_painel_congelado() {
        assert!(!vista_do_xml(r#"<worksheet><sheetViews><sheetView showZeros="0"/></sheetViews></worksheet>"#).zeros);
        let x = r#"<worksheet><sheetViews><sheetView showGridLines="0" workbookViewId="0">
            <pane xSplit="1" ySplit="4" topLeftCell="B5" activePane="bottomRight" state="frozen"/></sheetView></sheetViews></worksheet>"#;
        assert_eq!(vista_do_xml(x), VistaAba { linhas_grade: false, congelar: (4, 1), zeros: true });
        let sem = r#"<worksheet><sheetViews><sheetView workbookViewId="0"/></sheetViews></worksheet>"#;
        assert_eq!(vista_do_xml(sem), VistaAba::default());
        // Divisão comum (não congelada) não vira painel congelado.
        let div = r#"<worksheet><sheetViews><sheetView><pane ySplit="2000" state="split"/></sheetView></sheetViews></worksheet>"#;
        assert_eq!(vista_do_xml(div).congelar, (0, 0));
    }

    #[test]
    fn le_so_o_comeco_da_aba() {
        let mut saida = Cursor::new(Vec::new());
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(&mut saida);
            let o = zip::write::SimpleFileOptions::default();
            for (p, d) in [
                ("xl/workbook.xml", r#"<workbook><sheets><sheet name="A" r:id="rId1"/><sheet name="B" r:id="rId2"/></sheets></workbook>"#),
                ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>"#),
                ("xl/worksheets/sheet1.xml", r#"<worksheet><sheetViews><sheetView showGridLines="0"/></sheetViews><sheetData><row r="1"/></sheetData></worksheet>"#),
                ("xl/worksheets/sheet2.xml", r#"<worksheet><sheetData/></worksheet>"#),
            ] {
                z.start_file(p, o).unwrap();
                z.write_all(d.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        let v = ler_vistas_bytes(saida.into_inner());
        assert_eq!(v.len(), 2);
        assert!(!v[0].linhas_grade && v[1].linhas_grade);
    }

    #[test]
    fn zeros_ocultos_vao_e_voltam_do_xlsx() {
        let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
        wb.escrever_celula(0, 0, 0, "0".into());
        let vistas = vec![VistaAba { zeros: false, ..VistaAba::default() }];
        let bytes = crate::xlsx_write::com_vistas(vistas, || crate::xlsx_write::gerar_xlsx(&wb)).unwrap();
        let v = ler_vistas_bytes(bytes);
        assert!(!v[0].zeros && v[0].linhas_grade);
    }
}
