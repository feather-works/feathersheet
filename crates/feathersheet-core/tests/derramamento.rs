//! Matriz dinâmica derramando na pasta inteira: as funções de matriz em
//! português, outra aba lendo a área derramada e o cálculo paralelo (que lê
//! as colunas direto) enxergando os valores derramados.
use feathersheet_core::celulas::values::{ErrorCode, Value};
use feathersheet_core::planilha::workbook::Workbook;

fn n(v: f64) -> Value {
    Value::Number(v)
}

fn t(s: &str) -> Value {
    Value::Text(s.into())
}

#[test]
fn funcoes_de_matriz_derramam() {
    let mut w = Workbook::new();
    // A1:A4 dados, C1 = FILTRO, D1 = CLASSIFICAR, E1 = ÚNICO, F1 = TRANSPOR.
    for (r, v) in ["30", "10", "30", "20"].iter().enumerate() {
        w.escrever_celula(0, r as u32, 0, (*v).into());
    }
    w.escrever_celula(0, 0, 2, "=FILTRO(A1:A4;A1:A4>15)".into());
    w.escrever_celula(0, 0, 3, "=CLASSIFICAR(A1:A4)".into());
    w.escrever_celula(0, 0, 4, "=ÚNICO(A1:A4)".into());
    w.escrever_celula(0, 6, 0, "=TRANSPOR(A1:A4)".into());
    let col = |w: &Workbook, c: u32, linhas: u32| (0..linhas).map(|r| w.valor(0, r, c)).collect::<Vec<_>>();
    assert_eq!(col(&w, 2, 4), vec![n(30.0), n(30.0), n(20.0), Value::Empty]);
    assert_eq!(col(&w, 3, 4), vec![n(10.0), n(20.0), n(30.0), n(30.0)]);
    assert_eq!(col(&w, 4, 4), vec![n(30.0), n(10.0), n(20.0), Value::Empty]);
    let linha: Vec<Value> = (0..4).map(|c| w.valor(0, 6, c)).collect();
    assert_eq!(linha, vec![n(30.0), n(10.0), n(30.0), n(20.0)]);
}

#[test]
fn intervalo_vezes_numero_derrama() {
    let mut w = Workbook::new();
    for r in 0..3 {
        w.escrever_celula(0, r, 0, format!("{}", r + 1));
    }
    w.escrever_celula(0, 0, 1, "=A1:A3*10".into());
    assert_eq!(w.valor(0, 2, 1), n(30.0));
}

#[test]
fn outra_aba_le_a_area_derramada() {
    let mut w = Workbook::new();
    let dados = w.criar_aba("Dados");
    w.escrever_celula(dados, 0, 0, "=SEQUÊNCIA(4)".into());
    w.escrever_celula(0, 0, 0, "=SOMA(Dados!A1:A10)".into());
    w.escrever_celula(0, 1, 0, "=Dados!A4".into());
    assert_eq!(w.valor(0, 0, 0), n(10.0));
    assert_eq!(w.valor(0, 1, 0), n(4.0));
    // A matriz encolhe: quem lê de fora acompanha.
    w.escrever_celula(dados, 0, 0, "=SEQUÊNCIA(2)".into());
    assert_eq!(w.valor(0, 0, 0), n(3.0));
    assert_eq!(w.valor(0, 1, 0), n(0.0)); // =Dados!A4 com A4 vazia vale 0, como no Excel
}

#[test]
fn calculo_paralelo_enxerga_a_area_derramada() {
    let mut w = Workbook::new();
    // A1 derrama A1:A200; B1:B200 é uma coluna de fórmula empacotada que lê A.
    w.escrever_celula(0, 0, 0, "=SEQUÊNCIA(200)".into());
    for r in 0..200 {
        w.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
    }
    w.calcular_em_paralelo();
    for r in [0u32, 1, 57, 199] {
        assert_eq!(w.valor(0, r, 1), n(f64::from(r + 1) * 2.0), "linha {}", r + 1);
    }
}

#[test]
fn spill_com_texto_no_caminho() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "={\"a\";\"b\"}".into());
    assert_eq!(w.valor(0, 1, 0), t("b"));
    w.escrever_celula(0, 1, 0, "ocupado".into());
    assert!(matches!(w.valor(0, 0, 0), Value::Error(e) if e.code == ErrorCode::Spill));
    assert_eq!(w.valor(0, 1, 0), t("ocupado"));
}
