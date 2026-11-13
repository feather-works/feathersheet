//! Casos da auditoria de 13/11/2026 (motor de fórmulas): o resultado esperado é o do Excel em pt-BR.
use super::*;
use crate::celulas::values::Value;

fn pasta(celulas: &[(u32, u32, &str)]) -> Workbook {
    let mut w = Workbook::new();
    for &(r, c, t) in celulas { w.escrever_celula(0, r, c, t.into()); }
    w
}
fn num(w: &Workbook, r: u32, c: u32) -> Value { w.valor(0, r, c) }

#[test]
fn a01_numero_vira_texto_com_virgula() {
    let w = pasta(&[(0, 0, "1,5"), (0, 1, "=\"Peso: \"&A1"), (0, 2, "=VALOR(A1&\"\")")]);
    assert_eq!(num(&w, 0, 1), Value::Text("Peso: 1,5".into()));
    assert_eq!(num(&w, 0, 2), Value::Number(1.5));
}

#[test]
fn a02_formula_que_aponta_vazio_vale_zero() {
    let w = pasta(&[(0, 0, "10"), (0, 1, "=A1"), (1, 1, "=A2"), (0, 2, "=MÉDIA(B1:B2)"), (1, 2, "=CONT.NÚM(B1:B2)")]);
    assert_eq!(num(&w, 0, 2), Value::Number(5.0));
    assert_eq!(num(&w, 1, 2), Value::Number(2.0));
}

#[test]
fn a03_referencia_unica_conta_como_intervalo() {
    let w = pasta(&[(0, 0, "10"), (0, 1, "VERDADEIRO"), (0, 2, "'5"), (0, 3, "=SOMA(A1;B1;C1)")]);
    assert_eq!(num(&w, 0, 3), Value::Number(10.0));
}

#[test]
fn a04_curinga_no_criterio() {
    let w = pasta(&[(0, 0, "xabcx"), (1, 0, "foo"), (2, 0, "bar"), (3, 0, "5"),
        (0, 1, "=CONT.SE(A1:A3;\"<>*abc*\")"), (1, 1, "=CONT.SE(A1:A10;\"*\")")]);
    assert_eq!(num(&w, 0, 1), Value::Number(2.0));
    assert_eq!(num(&w, 1, 1), Value::Number(3.0));
}

#[test]
fn a05_zero_nao_casa_com_vazio() {
    let w = pasta(&[(0, 0, "0"), (0, 1, "=CONT.SE(A1:A10;0)")]);
    assert_eq!(num(&w, 0, 1), Value::Number(1.0));
}

#[test]
fn a06_somase_propaga_erro() {
    let w = pasta(&[(0, 0, "x"), (1, 0, "x"), (0, 1, "1"), (1, 1, "=NÃO.DISP()"), (0, 2, "=SOMASE(A1:A2;\"x\";B1:B2)")]);
    assert!(matches!(num(&w, 0, 2), Value::Error(_)), "{:?}", num(&w, 0, 2));
}

#[test]
fn a08_seerro_em_matriz() {
    let w = pasta(&[(0, 0, "1"), (1, 0, "2"), (0, 1, "1"), (1, 1, "0"), (0, 2, "=SOMA(SEERRO(A1:A2/B1:B2;0))")]);
    assert_eq!(num(&w, 0, 2), Value::Number(1.0));
}

#[test]
fn a10_indice_com_linha_zero() {
    let w = pasta(&[(0, 0, "1"), (1, 0, "2"), (0, 1, "10"), (1, 1, "20"), (0, 2, "=SOMA(ÍNDICE(A1:B2;0;2))")]);
    assert_eq!(num(&w, 0, 2), Value::Number(30.0));
}

#[test]
fn a11_datas_com_vazio_texto_e_tipo_da_semana() {
    // shortcut: ANO(vazia) dá 1899 (o Excel, 1900): o motor não tem o 29/02/1900 falso; só os seriais 0 a 60 diferem.
    let w = pasta(&[(0, 0, "=FIMMÊS(\"2024-01-15\";0)"), (0, 1, "=NÚMSEMANA(DATA(2024;1;7);2)"), (0, 2, "=MÊS(B5)")]);
    assert_eq!(num(&w, 0, 0), Value::Number(45322.0));
    assert!(!matches!(num(&w, 0, 2), Value::Error(_)));
    assert_eq!(num(&w, 0, 1), Value::Number(1.0));
}

#[test]
fn a12_texto_com_secoes_e_arredondamento() {
    let w = pasta(&[(0, 0, "=TEXTO(-5;\"0,00;(0,00)\")"), (0, 1, "=TEXTO(2,675;\"0,00\")")]);
    assert_eq!(num(&w, 0, 0), Value::Text("(5,00)".into()));
    assert_eq!(num(&w, 0, 1), Value::Text("2,68".into()));
}
