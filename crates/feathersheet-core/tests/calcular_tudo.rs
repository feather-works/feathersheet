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

#[test]
fn calcular_tudo_numa_coluna_acumulada_grande_e_linear() {
    // B2=B1+A2 em 20 mil linhas: uma cascata por fórmula era N² visitas (minutos num milhão de linhas).
    let mut w = Workbook::new();
    w.definir_calculo_manual(true);
    for r in 0..20_000u32 {
        w.escrever_celula(0, r, 0, "1".into());
        w.escrever_celula(0, r, 1, if r == 0 { "=A1".into() } else { format!("=B{}+A{}", r, r + 1) });
    }
    w.definir_calculo_manual(false);
    let t = std::time::Instant::now();
    w.calcular_tudo();
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
    assert_eq!(w.valor(0, 19_999, 1), Value::Number(20_000.0));
}

#[test]
fn inserir_que_empurraria_celula_para_fora_e_recusado() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 16_383, "fim".into()); // XFD1
    w.escrever_celula(0, 1, 0, "A2".into());
    assert!(w.inserir_colunas(0, 0, 1).is_none(), "XFD1 sairia da planilha");
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(1, 0), "A2", "nada mudou");
    assert!(w.inserir_linhas(0, 0, 3).is_some(), "linhas ainda cabem");
}
