//! DESLOC, INDIRETO, FÓRMULATEXTO, PLAN e PLANS numa pasta de verdade:
//! referência que sai de uma conta acompanha a edição (são voláteis) e o
//! texto da fórmula vem da célula gravada.
use feathersheet_core::{planilha::workbook::Workbook, Value};

fn n(v: f64) -> Value {
    Value::Number(v)
}

#[test]
fn desloc_e_indireto_acompanham_a_edicao() {
    let mut w = Workbook::new();
    w.criar_aba("Dados");
    for (r, v) in [(0, "1"), (1, "2"), (2, "3")] {
        w.escrever_celula(1, r, 0, v.into());
    }
    w.escrever_celula(0, 0, 1, "3".into());
    w.escrever_celula(0, 0, 0, "=SOMA(DESLOC(Dados!A1;0;0;B1))".into());
    w.escrever_celula(0, 1, 0, "=SOMA(INDIRETO(\"Dados!A1:A\"&B1))".into());
    assert_eq!(w.valor(0, 0, 0), n(6.0));
    assert_eq!(w.valor(0, 1, 0), n(6.0));
    // A célula de dentro do intervalo calculado muda: as duas acompanham.
    w.escrever_celula(1, 2, 0, "10".into());
    assert_eq!(w.valor(0, 0, 0), n(13.0));
    assert_eq!(w.valor(0, 1, 0), n(13.0));
    w.escrever_celula(0, 0, 1, "2".into());
    assert_eq!(w.valor(0, 0, 0), n(3.0));
}

#[test]
fn formulatexto_plan_e_plans() {
    let mut w = Workbook::new();
    w.criar_aba("Segunda");
    w.escrever_celula(0, 0, 0, "=1+2".into());
    w.escrever_celula(0, 0, 1, "=FÓRMULATEXTO(A1)".into());
    w.escrever_celula(0, 0, 2, "=ÉFÓRMULA(A1)".into());
    w.escrever_celula(0, 0, 3, "=PLANS()".into());
    w.escrever_celula(0, 0, 4, "=PLAN(Segunda!A1)".into());
    w.escrever_celula(1, 0, 0, "=PLAN()".into());
    assert_eq!(w.valor(0, 0, 1), Value::Text("=1+2".into()));
    assert_eq!(w.valor(0, 0, 2), Value::Bool(true));
    assert_eq!(w.valor(0, 0, 3), n(2.0));
    assert_eq!(w.valor(0, 0, 4), n(2.0));
    assert_eq!(w.valor(1, 0, 0), n(2.0));
}
