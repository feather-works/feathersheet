//! Referência 3D (`Jan:Mar!B5`): o mesmo endereço em todas as abas, da
//! primeira à última na ordem das guias, como no Excel.
use feathersheet_core::{planilha::workbook::Workbook, Value};

fn pasta() -> Workbook {
    let mut w = Workbook::new(); // aba 0 = resumo
    for (i, (nome, v)) in [("Jan", "10"), ("Fev", "20"), ("Mar", "30"), ("Abr", "1000")].into_iter().enumerate() {
        w.criar_aba(nome);
        w.escrever_celula(i + 1, 4, 1, v.into());
        w.escrever_celula(i + 1, 5, 1, "2".into());
    }
    w
}

#[test]
fn soma_media_e_maximo_em_tres_abas() {
    let mut w = pasta();
    w.escrever_celula(0, 0, 0, "=SOMA(Jan:Mar!B5)".into());
    w.escrever_celula(0, 1, 0, "=MÉDIA(Jan:Mar!B5:B6)".into());
    w.escrever_celula(0, 2, 0, "=MÁXIMO('Jan:Mar'!B5)".into());
    w.escrever_celula(0, 3, 0, "=SOMA(Mar:Jan!B5)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(60.));
    assert_eq!(w.valor(0, 1, 0), Value::Number(11.));
    assert_eq!(w.valor(0, 2, 0), Value::Number(30.));
    assert_eq!(w.valor(0, 3, 0), Value::Number(60.), "pontas invertidas valem o mesmo trecho");
}

#[test]
fn mudar_aba_do_meio_recalcula() {
    let mut w = pasta();
    w.escrever_celula(0, 0, 0, "=SOMA(Jan:Mar!B5)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(60.));
    w.escrever_celula(2, 4, 1, "25".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(65.));
}

#[test]
fn aba_que_nao_existe_da_erro() {
    let mut w = pasta();
    w.escrever_celula(0, 0, 0, "=SOMA(Jan:Dez!B5)".into());
    assert!(matches!(w.valor(0, 0, 0), Value::Error(_)));
}


#[test]
fn renomear_uma_ponta_reescreve_a_referencia() {
    let mut w = pasta();
    w.escrever_celula(0, 0, 0, "=SOMA(Jan:Mar!B5)".into());
    assert!(w.renomear_aba(1, "Janeiro"));
    assert_eq!(w.valor(0, 0, 0), Value::Number(60.), "primeira ponta");
    assert!(w.renomear_aba(3, "Mar 26"));
    assert_eq!(w.valor(0, 0, 0), Value::Number(60.), "última ponta, nome com espaço vai entre aspas");
    assert!(!w.renomear_aba(2, "Fev:2"), "dois-pontos é da referência 3D");
}
