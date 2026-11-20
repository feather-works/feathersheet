//! Ctrl+Alt+F9: o valor guardado que ficou velho (veio assim do arquivo) volta certo.
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_core::Value;

#[test]
fn calcular_tudo_refaz_a_formula_com_valor_guardado_velho() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "1".into());
    w.escrever_celula(0, 0, 1, "=A1*2".into());
    assert_eq!(w.valor(0, 0, 1), Value::Number(2.0));
    // O caminho de carga de arquivo não avisa quem depende: o B1 fica com o valor guardado.
    w.aba_mut(0).unwrap().sheet.set_cell_valor(0, 0, Value::Number(5.0));
    w.calcular_agora();
    assert_eq!(w.valor(0, 0, 1), Value::Number(2.0), "o F9 só refaz o que está marcado");
    w.calcular_tudo();
    assert_eq!(w.valor(0, 0, 1), Value::Number(10.0));
}
