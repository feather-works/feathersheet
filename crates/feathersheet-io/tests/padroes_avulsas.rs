//! #45: uma fórmula avulsa registrada depois da coluna empacotada não pode
//! tomar o número de padrão da coluna ao reabrir.
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::{da_workbook::pasta_gravar_bytes, fshz_read::ler_fshz, para_workbook::para_workbook};

#[test]
fn avulsa_nova_nao_troca_a_formula_da_coluna() {
    let mut w = Workbook::new();
    for r in 0..300 {
        w.escrever_celula(0, r, 0, (r + 1).to_string());
        w.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
    }
    for r in 0..4 {
        w.escrever_celula(0, r, 3, "1".into());
    }
    w.escrever_celula(0, 4, 3, "=SOMA(D1:D4)".into());
    w.consolidar_calculos(usize::MAX);
    let reaberta = para_workbook(&ler_fshz(&pasta_gravar_bytes(&w, 0).unwrap()).unwrap());
    let s = &reaberta.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(0, 1), "=A1*2");
    assert_eq!(s.get_raw(299, 1), "=A300*2");
    assert_eq!(s.get_raw(4, 3), "=SOMA(D1:D4)");
}

/// #55: texto que parece número ou fórmula continua texto depois de salvar e
/// reabrir, numa aba pequena e numa coluna esparsa de aba grande.
#[test]
fn texto_que_parece_numero_volta_texto() {
    use feathersheet_core::Value;
    for linhas in [10u32, 400] {
        let mut w = Workbook::new();
        {
            let s = &mut w.aba_mut(0).unwrap().sheet;
            for r in 0..linhas {
                s.set_cell_raw(r, 0, (r + 1).to_string(), None);
            }
            s.set_cell_valor(0, 1, Value::Text("2024".into()));
            s.set_cell_valor(1, 1, Value::Text("=x".into()));
            s.set_cell_valor(2, 1, Value::Text("001".into()));
            s.set_cell_valor(3, 1, Value::Text("nome".into()));
        }
        let reaberta = para_workbook(&ler_fshz(&pasta_gravar_bytes(&w, 0).unwrap()).unwrap());
        let s = &reaberta.aba(0).unwrap().sheet;
        assert_eq!(s.get_value(0, 1), Value::Text("2024".into()), "{linhas} linhas");
        assert_eq!(s.get_value(1, 1), Value::Text("=x".into()), "{linhas} linhas");
        assert_eq!(s.get_value(2, 1), Value::Text("001".into()), "{linhas} linhas");
        assert_eq!(s.get_value(3, 1), Value::Text("nome".into()), "{linhas} linhas");
        assert_eq!(s.get_value(linhas - 1, 0), Value::Number(linhas as f64));
    }
}
