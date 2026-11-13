//! As 22 diferenças que o `conferir_corpus` achou entre o motor Rust e o
//! TypeScript (22/09/2026), uma por uma. Cada teste falha no código anterior.
//!
//! Só pela API pública da `Sheet` — escreve a fórmula numa célula e lê o
//! valor —, que é como o resto do projeto vai usar o motor.

use feathersheet_core::funcoes::fn_date::definir_fuso_minutos;
use feathersheet_core::funcoes::fn_text::aplicar_mascara;
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::celulas::values::Value;

/// A planilha do corpus: A1:A5 = 10..50, B1:B5 = texto, data, lógicos, moeda.
fn planilha() -> Sheet {
    let mut s = Sheet::new("Planilha1");
    for r in 0..5u32 {
        s.set_cell_raw(r, 0, ((r + 1) * 10).to_string(), None);
    }
    for (r, bruto) in ["texto", "01/03/2026", "VERDADEIRO", "FALSO", "R$ 1.250,50"].iter().enumerate() {
        s.set_cell_raw(r as u32, 1, (*bruto).into(), None);
    }
    s
}

fn calc(formula: &str) -> Value {
    let mut s = planilha();
    s.set_cell_raw(20, 10, formula.into(), None);
    s.get_value(20, 10)
}

fn numero(formula: &str) -> f64 {
    match calc(formula) {
        Value::Number(n) => n,
        outro => panic!("{formula} deu {outro:?}"),
    }
}

fn texto(formula: &str) -> String {
    match calc(formula) {
        Value::Text(t) => t,
        outro => panic!("{formula} deu {outro:?}"),
    }
}

#[test]
fn cont_valores_conta_cada_celula_do_intervalo() {
    assert_eq!(numero("=CONT.VALORES(B1:B5)"), 5.0);
    assert_eq!(numero("=COUNTA(A1:A5; B1)"), 6.0);
}

#[test]
fn concatenar_junta_o_intervalo_inteiro() {
    assert_eq!(texto("=CONCAT(A1:A3)"), "102030");
    assert_eq!(texto("=CONCAT(A1:A2; \"x\")"), "1020x");
}

#[test]
fn concatenar_e_a_funcao_antiga_sem_intervalo() {
    // CONCATENAR = CONCATENATE: só os argumentos (intervalo entra pela 1ª célula).
    assert_eq!(texto("=CONCATENAR(\"a\"; \"b\"; 1)"), "ab1");
    assert_eq!(texto("=CONCATENATE(\"a\"; A1:A3)"), "a10");
    assert_eq!(texto("=CONCAT(\"a\"; A1:A3)"), "a102030");
}

#[test]
fn nomes_oficiais_e_apelidos_de_exibicao() {
    use feathersheet_core::funcoes::functions::canonical;
    assert_eq!(canonical("ÉCÉL.VAZIA"), Some("ISBLANK"));
    assert_eq!(canonical("ÉCÉL"), Some("ISBLANK"));
    assert_eq!(canonical("CONCATENAR"), Some("CONCATENATE"));
    assert_eq!(canonical("CONCATENATE"), None, "é função própria, não apelido");
    for (apelido, real) in [("É.ERRO", "ISERROR"), ("ÉERRO", "ISERROR"), ("ORDENAR", "SORT"),
        ("CLASSIFICAR", "SORT"), ("COMBINAÇÃO", "COMBIN"), ("PERMUTAÇÃO", "PERMUT")] {
        assert_eq!(canonical(apelido), Some(real), "{apelido}");
    }
    assert_eq!(canonical("COMBIN"), Some("COMBIN"));
    assert_eq!(numero("=COMBIN(5; 2)"), 10.0);
    assert_eq!(numero("=PERMUT(5; 2)"), 20.0);
}

#[test]
fn dia_da_semana_de_um_domingo_e_1() {
    // 1º/03/2026 foi domingo.
    assert_eq!(numero("=WEEKDAY(B2)"), 1.0);
    assert_eq!(numero("=WEEKDAY(DATA(2026; 9; 22))"), 3.0, "22/09/2026 é terça");
}

#[test]
fn dias_uteis_contam_segunda_a_sexta() {
    assert_eq!(numero("=DIATRABALHOTOTAL(DATA(2026; 3; 1); DATA(2026; 3; 10))"), 7.0);
    // 5 dias úteis depois do domingo 1º/03 é a sexta 6/03.
    assert_eq!(numero("=DIATRABALHO(DATA(2026; 3; 1); 5)"), numero("=DATA(2026; 3; 6)"));
}

#[test]
fn matrizes_aceitam_matriz_escrita_na_formula() {
    assert_eq!(numero("=MATRIZ.DETERM({1\\2;3\\4})"), -2.0);
    assert_eq!(numero("=ÍNDICE(MATRIZ.MULT({1\\2;3\\4}; {5\\6;7\\8}); 2; 2)"), 50.0);
    assert!((numero("=ÍNDICE(MATRIZ.INVERSO({1\\2;3\\4}); 1; 1)") + 2.0).abs() < 1e-12);
    // E intervalo continua valendo.
    assert_eq!(numero("=MATRIZ.DETERM(A1:A1)"), 10.0);
}

#[test]
fn dist_t_acumulada_bate_com_o_excel() {
    let p = numero("=DIST.T(1; 10; VERDADEIRO)");
    assert!((p - 0.829_553_433_856_926_4).abs() < 1e-12, "deu {p}");
    let q = numero("=DIST.T(-2; 5; VERDADEIRO)");
    assert!((q - 0.050_969_739_414_929_17).abs() < 1e-9, "deu {q}");
}

#[test]
fn hoje_segue_o_fuso_de_quem_usa() {
    // 14 horas de diferença mudam o dia em qualquer hora do relógio.
    definir_fuso_minutos(-14 * 60);
    let atras = numero("=HOJE()");
    definir_fuso_minutos(14 * 60);
    let adiante = numero("=HOJE()");
    definir_fuso_minutos(0);
    assert!(adiante - atras >= 1.0, "o fuso não mudou o dia: {atras} e {adiante}");
    let agora = numero("=AGORA()");
    let hoje = numero("=HOJE()");
    assert!(agora >= hoje && agora < hoje + 1.0);
}

/// Os mesmos casos do teste do TypeScript (`test/core/texto-mascara.test.ts`).
#[test]
fn texto_entende_mascara_em_pt_br_e_em_ingles() {
    let casos: &[(f64, &str, &str)] = &[
        (1250.5, "R$ #.##0,00", "R$ 1.250,50"),
        (1250.5, "R$ #,##0.00", "R$ 1.250,50"),
        (1250.5, "0", "1251"),
        (1234567.891, "#.##0", "1.234.568"),
        (1234567.891, "#,##0", "1.234.568"),
        (0.256, "0%", "26%"),
        (0.256, "0,0%", "25,6%"),
        (0.256, "0.0%", "25,6%"),
        (-5.0, "R$ 0,00", "-R$ 5,00"),
        (3.5, "0,##", "3,5"),
        (3.0, "0,##", "3"),
        (7.0, "000", "007"),
        (1.5, "0.00", "1,50"),
        (0.5, "#,##", ",5"),
    ];
    for (n, mascara, esperado) in casos {
        assert_eq!(aplicar_mascara(*n, mascara), *esperado, "TEXTO({n}; \"{mascara}\")");
    }
    assert_eq!(texto("=TEXTO(1250,5; \"R$ #.##0,00\")"), "R$ 1.250,50");
}

#[test]
fn indice_como_no_typescript() {
    assert_eq!(numero("=ÍNDICE({1\\2;3\\4}; 2; 1)"), 3.0, "sobre matriz escrita");
    assert_eq!(numero("=ÍNDICE(A1:A5; 3)"), 30.0);
    // Linha 0 é a coluna inteira, como no Excel (auditoria de 13/11/2026; antes dava #REF!).
    assert_eq!(numero("=SOMA(ÍNDICE(A1:A5; 0; 1))"), 150.0, "linha 0 é a coluna inteira");
    let mut s = planilha();
    for c in 0..5u32 {
        s.set_cell_raw(10, c, (c * 7).to_string(), None);
    }
    s.set_cell_raw(20, 10, "=ÍNDICE(A11:E11; 3)".into(), None);
    assert_eq!(s.get_value(20, 10), Value::Number(14.0), "linha única: o número é a coluna");
}
