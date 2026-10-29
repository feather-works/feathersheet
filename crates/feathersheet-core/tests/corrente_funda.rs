//! Corrente funda de dependências (saldo acumulado): calcular o fim de uma
//! coluna de 200 mil `K{n}=K{n-1}+I{n}` não pode estourar a pilha.
use feathersheet_core::celulas::values::{ErrorCode, Value};
use feathersheet_core::planilha::workbook::Workbook;

fn corrente(n: u32) -> Workbook {
    let mut w = Workbook::new();
    for r in 0..n {
        w.escrever_celula(0, r, 0, "1".into());
        let f = if r == 0 { "=A1".to_string() } else { format!("=B{}+A{}", r, r + 1) };
        w.escrever_celula(0, r, 1, f);
    }
    w
}

#[test]
fn corrente_de_duzentas_mil_cabe_numa_thread_de_2_mib() {
    let n = 200_000;
    let w = corrente(n);
    let v = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || w.valor(0, n - 1, 1))
        .unwrap()
        .join()
        .expect("não pode estourar a pilha");
    assert_eq!(v, Value::Number(f64::from(n)));
}

#[test]
fn meio_da_corrente_e_mudanca_depois_de_calcular() {
    let n = 30_000;
    let mut w = corrente(n);
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n)));
    assert_eq!(w.valor(0, 9_999, 1), Value::Number(10_000.0));
    // Mudar um valor no começo precisa refletir no fim.
    w.escrever_celula(0, 0, 0, "1001".into());
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n) + 1000.0));
}

#[test]
fn circular_continua_detectada_em_corrente_funda() {
    let mut w = corrente(5_000);
    // Fecha a corrente: o começo passa a depender do fim.
    w.escrever_celula(0, 0, 1, "=B5000+A1".into());
    match w.valor(0, 4_999, 1) {
        Value::Error(e) => assert_eq!(e.code, ErrorCode::Circular),
        outro => panic!("esperava #CIRCULAR, veio {outro:?}"),
    }
}

#[test]
fn fim_da_corrente_lido_de_outra_aba_nao_estoura_a_pilha() {
    // A aba Resumo do arquivo do usuário tem `=Dados!K1000001`; abrir a
    // Resumo primeiro derrubava a janela (13/10/2026).
    let n = 200_000;
    let mut w = corrente(n);
    let resumo = w.criar_aba("Resumo");
    let nome = w.nomes()[0].clone();
    w.escrever_celula(resumo, 0, 0, format!("='{nome}'!B{n}"));
    let v = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || w.valor(resumo, 0, 0))
        .unwrap()
        .join()
        .expect("não pode estourar a pilha");
    assert_eq!(v, Value::Number(f64::from(n)));
}

#[test]
fn consolidar_leva_os_resultados_para_a_coluna_e_mudanca_continua_valendo() {
    let n = 5_000;
    let mut w = corrente(n);
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n)));
    // Aos poucos: o limite é respeitado.
    assert_eq!(w.consolidar_calculos(1_000), 1_000);
    let resto = w.consolidar_calculos(usize::MAX);
    assert_eq!(resto as u32, n - 1_000, "o resto vai na rodada seguinte");
    assert_eq!(w.consolidar_calculos(usize::MAX), 0, "nada mais a mover");
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n)));
    assert_eq!(w.valor(0, 1_234, 1), Value::Number(1_235.0));
    // Depois de consolidar, mudar o começo ainda recalcula o fim.
    w.escrever_celula(0, 0, 0, "11".into());
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n) + 10.0));
    w.consolidar_calculos(usize::MAX);
    assert_eq!(w.valor(0, n - 1, 1), Value::Number(f64::from(n) + 10.0));
}

/// A fórmula mais longa que o Excel aceita (8.192 caracteres, `1+1+…` = 4.096 operações)
/// calcula, arrasta e refaz o texto numa thread de 1 MB. Só no build otimizado, que é o
/// do aplicativo: no de depuração o teto é menor (ver `parser::MAX_OPERACOES`).
#[cfg(not(debug_assertions))]
#[test]
fn formula_mais_longa_do_excel_cabe_na_pilha() {
    std::thread::Builder::new().stack_size(1 << 20).spawn(|| {
        let f = format!("=B1{}", "+1".repeat(4_094));
        let mut s = feathersheet_core::planilha::sheet::Sheet::new("x");
        s.set_cell(0, 1, "2");
        s.set_cell(0, 2, &f);
        assert_eq!(s.get_value(0, 2), feathersheet_core::Value::Number(4096.0));
        let r = feathersheet_core::celulas::refs::Rect::new(0, 2, 0, 2);
        s.preencher(&r, &feathersheet_core::celulas::refs::Rect::new(0, 2, 3, 2));
        assert!(s.get_raw(3, 2).starts_with("=B4+1"));
        assert_eq!(s.get_raw(0, 2).len(), f.len());
    }).unwrap().join().expect("estourou a pilha");
}
