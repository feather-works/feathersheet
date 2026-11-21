//! Coluna oculta no .xlsx: vai com `hidden` e a largura de antes, e reexibir devolve a largura.
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::{xlsx_read::ler_xlsx_workbook, xlsx_write::gerar_xlsx};

#[test]
fn coluna_oculta_guarda_a_largura_no_xlsx() {
    let mut w = Workbook::new();
    w.editar(0, |s| {
        s.definir_dimensao(true, 2, 145);
        s.alterar_visibilidade_dimensoes(true, true, 2, 3).unwrap();
    });
    let bytes = gerar_xlsx(&w).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut xml).unwrap();
    assert!(xml.contains("<col min=\"3\" max=\"3\" width=\"20.71\" hidden=\"1\" customWidth=\"1\""), "{xml}");
    assert!(xml.contains("<col min=\"4\" max=\"4\" width=") && xml.matches("hidden=\"1\"").count() == 2, "{xml}");

    let (mut lida, _) = ler_xlsx_workbook(bytes).unwrap();
    let aba = &mut lida.aba_mut(0).unwrap().sheet;
    assert_eq!(aba.col_widths.get(&2), Some(&0), "segue oculta");
    aba.alterar_visibilidade_dimensoes(true, false, 2, 3).unwrap();
    assert_eq!(aba.col_widths.get(&2), Some(&145), "reexibir devolve a largura do arquivo");
    assert!(aba.col_widths.get(&3).is_none());
}
