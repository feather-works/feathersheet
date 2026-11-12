//! O código de moeda segue os separadores da língua do app: em inglês, `CHF 1,234.50`.
//! Arquivo próprio porque troca a língua do processo inteiro.
use feathersheet_core::celulas::{formato_personalizado::formatar, moedas::MOEDAS};
use feathersheet_core::{planilha::sheet::Sheet, set_locale_config, LocaleConfig, Value};

#[test]
fn moeda_em_ingles_usa_virgula_no_milhar_e_ponto_no_decimal() {
    set_locale_config(LocaleConfig::en_us());
    let codigo = |iso: &str| MOEDAS.iter().find(|m| m.iso == iso).unwrap().codigo_padrao();
    assert_eq!(formatar(&Value::Number(1234.5), &codigo("CHF")).unwrap(), "CHF 1,234.50");
    assert_eq!(formatar(&Value::Number(12_345_678.9), &codigo("INR")).unwrap(), "₹ 1,23,45,678.90");
    assert_eq!(formatar(&Value::Number(1500000.0), &codigo("PKR")).unwrap(), "Rs 15,00,000.00");

    let mut sheet = Sheet::new("Plan1");
    sheet.set_cell(0, 0, "12,34");
    sheet.set_cell(0, 1, "=TYPE(A1)");
    assert_eq!(sheet.get_value(0, 0), Value::Text("12,34".into()));
    assert_eq!(sheet.get_value(0, 1), Value::Number(2.0));
}
