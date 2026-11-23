//! Fórmula antiga do `.xlsx` (sem `cm`): interseção implícita, como no Excel de antes do 365 e no
//! LibreOffice; a moderna (com `cm`) derrama; o `@` gravado pelo Excel 365 (`_xlfn.SINGLE`) calcula.
use feathersheet_core::celulas::values::Value;
use feathersheet_io::{xlsx_read::ler_xlsx_workbook, xlsx_write::gerar_xlsx};
use std::io::{Cursor, Write};

fn xlsx(celulas: &str) -> Vec<u8> {
    let mut saida = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut saida);
        let o = zip::write::SimpleFileOptions::default();
        let folha = format!(r#"<worksheet><sheetData>{celulas}</sheetData></worksheet>"#);
        let partes = [
            ("xl/workbook.xml", r#"<workbook xmlns:r="x"><sheets><sheet name="P" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_string()),
            ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#.to_string()),
            ("xl/worksheets/sheet1.xml", folha),
        ];
        for (n, c) in partes { zip.start_file(n, o).unwrap(); zip.write_all(c.as_bytes()).unwrap(); }
        zip.finish().unwrap();
    }
    saida.into_inner()
}

#[test]
fn formula_antiga_faz_intersecao_e_moderna_derrama() {
    let dados = r#"<row r="1"><c r="A1"><v>1</v></c><c r="D1" cm="1"><f t="array" ref="D1:D3">A1:A3*10</f><v>10</v></c></row>
<row r="2"><c r="A2"><v>2</v></c><c r="B2"><f>A1:A3*10</f><v>20</v></c><c r="C2"><f>_xlfn.SINGLE(A:A)</f><v>2</v></c><c r="E2"><f>SUM(A1:A3)</f><v>6</v></c></row>
<row r="3"><c r="A3"><v>3</v></c></row>"#;
    let (wb, _) = ler_xlsx_workbook(xlsx(dados)).unwrap();
    assert_eq!(wb.valor(0, 1, 1), Value::Number(20.0), "B2 antiga: A2*10");
    assert_eq!(wb.valor(0, 2, 1), Value::Empty, "e não derrama em B3");
    assert_eq!(wb.valor(0, 1, 2), Value::Number(2.0), "o @ do Excel 365 (_xlfn.SINGLE) calcula");
    assert_eq!(wb.valor(0, 2, 3), Value::Number(30.0), "a moderna (cm) derrama");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1, 4), "=SOMA(A1:A3)", "fórmula comum fica como está");
    // Ao salvar, o Excel recebe o @ como ele grava.
    let mut z = zip::ZipArchive::new(Cursor::new(gerar_xlsx(&wb).unwrap())).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut s).unwrap();
    assert!(s.contains("_xlfn.SINGLE(A1:A3*10)"), "{s}");
}

#[test]
fn matriz_bloqueada_volta_do_xlsx_sem_virar_single() {
    use feathersheet_core::planilha::workbook::Workbook;
    let mut w = Workbook::new();
    for r in 0..3 { w.escrever_celula(0, r, 0, format!("{}", r + 1)); }
    w.escrever_celula(0, 2, 1, "ocupada".into());
    w.escrever_celula(0, 0, 1, "=A1:A3*10".into());
    assert!(matches!(w.valor(0, 0, 1), Value::Error(_)), "#DESPEJO!");
    let bytes = gerar_xlsx(&w).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    assert!(z.by_name("xl/metadata.xml").is_ok(), "o cm precisa do metadata.xml");
    let (de_volta, _) = ler_xlsx_workbook(bytes).unwrap();
    assert_eq!(de_volta.aba(0).unwrap().sheet.get_raw(0, 1), "=A1:A3*10", "a fórmula do usuário não muda");
    assert!(matches!(de_volta.valor(0, 0, 1), Value::Error(_)), "segue bloqueada");
}
