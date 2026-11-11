//! O texto dos números em en-US e numa língua que só existe como arquivo
//! (de-DE, moeda depois). Fica num arquivo à parte porque o idioma é
//! global no processo: dentro de `src/`, este teste rodaria ao mesmo tempo
//! que os outros (que esperam pt-BR) e trocaria o idioma deles no meio.
//!
//! Os esperados saíram do `formatValue` do `style.ts` com `setAppLocale('en-US')`.

use feathersheet_core::celulas::locale::{set_locale_config, LocaleConfig};
use feathersheet_core::celulas::style::{format_number, NumberFormat, Style};

fn com(format: NumberFormat, decimals: Option<u8>) -> Style {
    Style { format, decimals, ..Style::default() }
}

#[test]
fn numeros_em_ingles_e_de_volta_ao_portugues() {
    set_locale_config(LocaleConfig::en_us());
    let casos: [(f64, Option<Style>, &str); 10] = [
        (1234567.891, None, "1,234,567.891"),
        (-0.5, None, "-0.5"),
        (1250.5, Some(com(NumberFormat::Number, None)), "1,250.50"),
        (-1250.5, Some(com(NumberFormat::Currency, None)), "-$1,250.50"),
        (1234.5, Some(com(NumberFormat::Currency, Some(0))), "$1,235"),
        (0.1234, Some(com(NumberFormat::Percent, None)), "12.34%"),
        (-1234.5, Some(com(NumberFormat::Accounting, None)), "($ 1,234.50)"), // o símbolo da língua (era "R$" fixo)
        (12345.678, Some(com(NumberFormat::Scientific, None)), "1.23E+04"),
        (2.5, Some(com(NumberFormat::Integer, None)), "3"),
        (1e21, None, "1,000,000,000,000,000,000,000"),
    ];
    for (n, estilo, esperado) in &casos {
        assert_eq!(format_number(*n, estilo.as_ref()), *esperado, "{n}");
    }
    // Uma língua que não vem embutida: chega inteira do arquivo de língua,
    // com a moeda depois do número. Tudo no mesmo teste, em sequência: o
    // idioma é global, e dois testes trocando-o ao mesmo tempo se atropelam.
    set_locale_config(LocaleConfig {
        code: "de-DE".into(),
        name: "Deutsch".into(),
        decimal_separator: ',',
        thousands_separator: '.',
        formula_arg_separator: ';',
        currency_symbol: "€".into(),
        currency_code: "EUR".into(),
        currency_before: false,
        currency_space: true,
    });
    assert_eq!(format_number(1234567.891, None), "1.234.567,891");
    assert_eq!(format_number(-1250.5, Some(&com(NumberFormat::Currency, None))), "-1.250,50\u{a0}€");
    // O lógico aparece na palavra de cada língua e, digitado de volta (F2 e Enter), continua lógico.
    use feathersheet_core::celulas::input::parse_literal;
    use feathersheet_core::celulas::style::format_value;
    use feathersheet_core::celulas::values::Value;
    // As palavras chegam do dicionário de funções do app (`definir_logicos`); aqui, à mão.
    use feathersheet_core::celulas::locale::definir_logicos;
    for (sim, nao) in [("TRUE", "FALSE"), ("VERDADERO", "FALSO"), ("VRAI", "FAUX"), ("VERDADEIRO", "FALSO")] {
        definir_logicos(sim, nao);
        for (valor, texto) in [(true, sim), (false, nao)] {
            assert_eq!(format_value(&Value::Bool(valor), None), texto);
            assert_eq!(parse_literal(texto).value, Value::Bool(valor), "{texto} digitado de volta");
            assert_eq!(parse_literal(if valor { "true" } else { "FALSO" }).value, Value::Bool(valor), "formas de sempre");
        }
        assert_eq!(format_value(&Value::Text("VERDADEIRO".into()), None), "VERDADEIRO", "texto não é traduzido");
        // Dentro de fórmula o lógico vira texto na mesma palavra, e esse texto volta a ser lógico.
        let mut w = feathersheet_core::planilha::workbook::Workbook::new();
        for (r, raw) in ["=1>0", "=A1&\"\"", "=CONCAT(A1;\"!\")", "=SE(A2;1;2)", "=ESQUERDA(1=2;2)"].into_iter().enumerate() {
            w.escrever_celula(0, r as u32, 0, raw.into());
        }
        assert_eq!(w.valor(0, 1, 0), Value::Text(sim.into()), "=A1&\"\"");
        assert_eq!(w.valor(0, 2, 0), Value::Text(format!("{sim}!")), "CONCAT");
        assert_eq!(w.valor(0, 3, 0), Value::Number(1.0), "o texto da língua vale como lógico no SE");
        assert_eq!(w.valor(0, 4, 0), Value::Text(nao.chars().take(2).collect()), "ESQUERDA");
    }
    assert_eq!(parse_literal("VRAI").value, Value::Text("VRAI".into()), "em português VRAI é texto");
    definir_logicos("", "");
    assert_eq!(format_value(&Value::Bool(true), None), "VERDADEIRO", "sem dicionário, português");
    // Em francês o espaço comum separa milhar ao digitar; em português não (é código).
    set_locale_config(LocaleConfig::fr_fr());
    assert_eq!(feathersheet_core::celulas::input::parse_literal("1 250,50").value, feathersheet_core::celulas::values::Value::Number(1250.5));
    set_locale_config(LocaleConfig::pt_br());
    assert_eq!(format_number(-1250.5, Some(&com(NumberFormat::Currency, None))), "-R$\u{a0}1.250,50");
}
