//! Abrir `.xlsb` (formato binário do Excel): valores, datas (também na base 1904), fórmulas com funções novas
//! e nomes definidos. Os arquivos vêm dos testes do `calamine` (tests/arquivos/xlsb/LEIA-ME.md).
use feathersheet_core::celulas::values::Value;
use feathersheet_io::xlsb_read::ler_xlsb;

fn abrir(nome: &str) -> feathersheet_core::planilha::workbook::Workbook {
    let caminho = format!("{}/tests/arquivos/xlsb/{nome}", env!("CARGO_MANIFEST_DIR"));
    ler_xlsb(std::fs::read(caminho).unwrap()).unwrap()
}

#[test]
fn xlsb_abre_valores_datas_formulas_e_nomes() {
    let p = abrir("formula_values.xlsb");
    assert_eq!(p.valor(0, 0, 0), Value::Number(3.0), "=1+2");
    assert_eq!(p.valor(0, 1, 0), Value::Text("Ab".into()), "CONCAT, função nova (User( do calamine)");
    assert_eq!(p.aba(0).unwrap().sheet.get_raw(2, 0), "=ÉERRO(1)", "fórmula em português, como no .xlsx");

    let d = abrir("date_1904.xlsb");
    assert_eq!(d.valor(0, 0, 0), Value::Number(44197.0), "1/1/2021 na base 1904 vira a base 1900");
    assert_eq!(d.aba(0).unwrap().sheet.get_raw(0, 0), "01/01/2021", "com formato de data");

    let i = abrir("issues.xlsb");
    assert_eq!(i.len(), 6);
    assert_eq!(i.valor(0, 2, 0), Value::Text("ab".into()));
    assert_eq!(i.valor(5, 5, 0), Value::Text("☺".into()), "texto com caractere especial");
    assert!(!matches!(i.valor(2, 1, 0), Value::Error(_)), "=B1+OneRange: o nome definido de uma célula existe");
}
