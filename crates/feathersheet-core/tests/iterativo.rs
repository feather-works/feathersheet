//! Cálculo iterativo. Arquivo à parte porque a opção é global no processo, como o idioma.
use feathersheet_core::planilha::iterativo;
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_core::Value;

fn numero(s: &Sheet, r: u32, c: u32) -> f64 {
    match s.get_value(r, c) { Value::Number(n) => n, outro => panic!("esperava número, veio {outro:?}") }
}

#[test]
fn ciclo_da_circular_desligado_e_converge_ligado() {
    let mut s = Sheet::new("Plan1");
    // A1 = 100 + B1 ; B1 = A1 * 0,5  →  A1 = 200, B1 = 100
    s.set_cell_raw(0, 0, "=100+B1".into(), None);
    s.set_cell_raw(0, 1, "=A1*0,5".into(), None);
    assert!(matches!(s.get_value(0, 0), Value::Error(_)), "desligado: #CIRCULAR!");

    iterativo::definir(Some((100, 0.001)));
    s.set_cell_raw(0, 0, "=100+B1".into(), None); // suja o ciclo de novo
    assert!((numero(&s, 0, 0) - 200.0).abs() < 0.01, "A1 = {}", numero(&s, 0, 0));
    assert!((numero(&s, 0, 1) - 100.0).abs() < 0.01, "B1 = {}", numero(&s, 0, 1));

    // Contador: =C1+1 anda o máximo de iterações a cada recálculo, como no Excel.
    iterativo::definir(Some((10, 0.0)));
    s.set_cell_raw(0, 2, "=C1+1".into(), None);
    assert_eq!(numero(&s, 0, 2), 10.0);

    // Quem só lê o ciclo recebe o valor final.
    s.set_cell_raw(0, 3, "=A1+1".into(), None);
    assert!((numero(&s, 0, 3) - 201.0).abs() < 0.01);

    iterativo::definir(None);
    s.set_cell_raw(0, 0, "=100+B1".into(), None);
    assert!(matches!(s.get_value(0, 0), Value::Error(_)), "desligado de novo: #CIRCULAR!");

    // Uma função só: a opção é global e os testes do arquivo rodariam em paralelo.
    ligar_a_opcao_e_o_f9_refazem_o_ciclo();
}

fn ligar_a_opcao_e_o_f9_refazem_o_ciclo() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "=A1+1".into());
    w.escrever_celula(0, 0, 1, "=A1*2".into());
    assert!(matches!(w.valor(0, 0, 0), Value::Error(_)));
    iterativo::definir(Some((5, 0.0)));
    w.calcular_agora();
    assert_eq!(w.valor(0, 0, 0), Value::Number(5.0), "o #CIRCULAR! vira número ao ligar");
    assert_eq!(w.valor(0, 0, 1), Value::Number(10.0), "quem lê o ciclo acompanha");
    w.calcular_agora();
    assert_eq!(w.valor(0, 0, 0), Value::Number(10.0), "o F9 anda mais 5 iterações");
    iterativo::definir(None);
    w.calcular_agora();
    assert!(matches!(w.valor(0, 0, 0), Value::Error(_)), "desligar devolve o #CIRCULAR!");
}
