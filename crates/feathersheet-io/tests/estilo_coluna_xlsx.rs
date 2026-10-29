//! `<col min=".." max="16384" style="..">`: a planilha toda com um estilo vira o estilo
//! da aba, em vez de 16.384 cópias (29/10/2026); se as colunas de antes têm outro
//! estilo (ou nenhum), cada coluna continua com o seu.
use feathersheet_io::xlsx_read::ler_xlsx_workbook;
use std::io::{Cursor, Write};

fn xlsx(cols: &str) -> Vec<u8> {
    let mut saida = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut saida);
        let o = zip::write::SimpleFileOptions::default();
        let folha = format!(r#"<worksheet><cols>{cols}</cols><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#);
        let estilos = r#"<styleSheet><fonts count="2"><font><sz val="11"/></font><font><b/><sz val="11"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border/></borders><cellXfs count="2"><xf fontId="0" fillId="0" borderId="0"/><xf fontId="1" fillId="0" borderId="0" applyFont="1"/></cellXfs></styleSheet>"#;
        let partes = [
            ("xl/workbook.xml", r#"<workbook xmlns:r="x"><sheets><sheet name="A" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_string()),
            ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.to_string()),
            ("xl/styles.xml", estilos.to_string()),
            ("xl/worksheets/sheet1.xml", folha),
        ];
        for (n, c) in partes { zip.start_file(n, o).unwrap(); zip.write_all(c.as_bytes()).unwrap(); }
        zip.finish().unwrap();
    }
    saida.into_inner()
}

fn negrito(s: &feathersheet_core::planilha::sheet::Sheet, col: u32) -> bool {
    s.estilo_resolvido(0, col).is_some_and(|e| e.bold)
}

#[test]
fn planilha_toda_com_um_estilo_vira_o_estilo_da_aba() {
    let (wb, _) = ler_xlsx_workbook(xlsx(r#"<col min="1" max="2" style="1"/><col min="3" max="16384" style="1"/>"#)).unwrap();
    let s = &wb.aba(0).unwrap().sheet;
    assert!(s.sheet_style.is_some() && s.col_styles.is_empty(), "{} estilos de coluna", s.col_styles.len());
    assert!(negrito(s, 0) && negrito(s, 16_383));
}

#[test]
fn coluna_de_antes_sem_o_estilo_continua_por_coluna() {
    let (wb, _) = ler_xlsx_workbook(xlsx(r#"<col min="2" max="16384" style="1"/>"#)).unwrap();
    let s = &wb.aba(0).unwrap().sheet;
    assert!(s.sheet_style.is_none());
    assert!(!negrito(s, 0) && negrito(s, 1) && negrito(s, 16_383));
}
