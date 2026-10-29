//! Pasta com colunas compactadas (blocos de 1/2/4 bytes) grava e reabre igual, e
//! editar depois de compactar continua certo.

use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::workbook::Workbook;

#[test]
fn compactada_grava_reabre_e_edita_igual() {
    let n = 150_000u32;
    let mut wb = Workbook::new();
    {
        let s = &mut wb.aba_mut(0).unwrap().sheet;
        for r in 0..n {
            s.set_cell_valor(r, 0, Value::Number(4887.75 + 0.25 * f64::from(r % 3000)));
            s.set_cell_valor(r, 1, Value::Number(f64::from(r % 97)));
            s.set_cell_valor(r, 2, Value::Text(format!("EH 0{}", r % 3)));
            if r % 5 == 0 { s.set_cell_valor(r, 3, Value::Number(1.0 / f64::from(r + 1))); }
        }
    }
    let ler = |wb: &Workbook| -> Vec<Value> {
        let s = &wb.aba(0).unwrap().sheet;
        (0..n).flat_map(|r| (0..4).map(move |c| (r, c))).map(|(r, c)| s.get_value(r, c)).collect()
    };
    let antes = ler(&wb);
    let (a, d) = wb.compactar_colunas();
    assert!(d * 3 < a, "as colunas compactam: {a} → {d}");
    assert!(ler(&wb) == antes, "compactar mudou algum valor");
    let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap();
    let reaberta = feathersheet_io::para_workbook::para_workbook_soltando(feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
    assert!(ler(&reaberta) == antes, "gravar a compactada perdeu valor");
    let s = &mut wb.aba_mut(0).unwrap().sheet;
    s.set_cell_valor(70_000, 0, Value::Number(0.1));
    s.set_cell_valor(70_001, 1, Value::Empty);
    assert_eq!(s.get_value(70_000, 0), Value::Number(0.1));
    assert_eq!(s.get_value(70_001, 1), Value::Empty);
    assert_eq!(s.get_value(70_002, 0), antes[70_002 * 4]);
}
