//! Número com formato de data, hora ou % volta do `.xlsx` com o valor inteiro
//! (antes voltava o texto formatado lido de novo: a hora e casas decimais sumiam).
use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::workbook::Workbook;

#[test]
fn data_com_hora_porcentagem_e_duracao_voltam_inteiras() {
    let mut w = Workbook::new();
    for (c, (valor, formato)) in [(45000.75, "date"), (0.169123, "percent"), (1.5, "time")].iter().enumerate() {
        w.aba_mut(0).unwrap().sheet.set_cell_valor(
            0, c as u32, Value::Number(*valor)
        );
        let mut e = EstiloParcial::new();
        e.insert("format".into(), Campo::Texto(formato.to_string()));
        w.aba_mut(0).unwrap().sheet.set_style(&Rect::cell(0, c as u32), &e);
    }
    let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&w).expect("gravar");
    let (volta, _) = feathersheet_io::xlsx_read::ler_xlsx_workbook(bytes).expect("ler");
    assert_eq!(volta.valor(0, 0, 0), Value::Number(45000.75), "a hora da data");
    assert_eq!(volta.valor(0, 0, 1), Value::Number(0.169123), "as casas da porcentagem");
    assert_eq!(volta.valor(0, 0, 2), Value::Number(1.5), "duração acima de um dia");
}
