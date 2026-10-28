use super::planilha_de_mentira::Fake;
use super::*;
use crate::formula::parser::parse_formula;

fn calcula(f: &Fake, formula: &str) -> Value {
    let ev = Evaluator::new(f);
    ev.eval(&parse_formula(formula).expect("devia compilar"))
}

fn calcula_cru(f: &Fake, formula: &str) -> Computed {
    let ev = Evaluator::new(f);
    ev.eval_array(&parse_formula(formula).expect("devia compilar"))
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

#[test]
fn aritmetica() {
    let f = Fake::default();
    assert_eq!(calcula(&f, "1+2*3"), num(7.0));
    assert_eq!(calcula(&f, "(1+2)*3"), num(9.0));
    assert_eq!(calcula(&f, "2^3^2"), num(512.0));
    assert_eq!(calcula(&f, "-2^2"), num(4.0)); // o unário liga antes
    assert_eq!(calcula(&f, "50%"), num(0.5));
    assert_eq!(calcula(&f, "10/4"), num(2.5));
}

#[test]
fn divisao_por_zero() {
    let f = Fake::default();
    assert_eq!(calcula(&f, "1/0"), Value::erro(ErrorCode::Div0));
    // Célula vazia vale zero, então isto também é divisão por zero.
    assert_eq!(calcula(&f, "1/A1"), Value::erro(ErrorCode::Div0));
}

#[test]
fn potencia_que_estoura_vira_num() {
    let f = Fake::default();
    assert_eq!(calcula(&f, "10^1000"), Value::erro(ErrorCode::Num));
}

#[test]
fn texto_e_comparacao() {
    let f = Fake::com(&[("A1", Value::texto("oi"))]);
    assert_eq!(calcula(&f, "A1&\" mundo\""), Value::texto("oi mundo"));
    assert_eq!(calcula(&f, "1&2"), Value::texto("12"));
    assert_eq!(calcula(&f, "2>1"), Value::Bool(true));
    assert_eq!(calcula(&f, "\"a\"=\"A\""), Value::Bool(true));
    assert_eq!(calcula(&f, "1<>1"), Value::Bool(false));
    // Número é sempre menor que texto.
    assert_eq!(calcula(&f, "9<\"a\""), Value::Bool(true));
}

#[test]
fn erro_contamina_a_conta() {
    let f = Fake::com(&[("A1", Value::erro(ErrorCode::Na))]);
    assert_eq!(calcula(&f, "A1+1"), Value::erro(ErrorCode::Na));
    assert_eq!(calcula(&f, "1+A1"), Value::erro(ErrorCode::Na));
    assert_eq!(calcula(&f, "A1&\"x\""), Value::erro(ErrorCode::Na));
}

#[test]
fn referencia_e_celula_vazia() {
    let f = Fake::com(&[("B7", num(42.0))]);
    assert_eq!(calcula(&f, "B7"), num(42.0));
    assert_eq!(calcula(&f, "$B$7"), num(42.0));
    assert_eq!(calcula(&f, "C1"), Value::Empty);
    assert_eq!(calcula(&f, "C1+1"), num(1.0));
}

#[test]
fn intervalo_vira_matriz() {
    let f = Fake::com(&[("A1", num(1.0)), ("A2", num(2.0)), ("A3", num(3.0))]);
    let m = calcula_cru(&f, "A1:A3");
    let m = m.as_matrix().expect("devia ser matriz");
    assert_eq!(m.height(), 3);
    assert_eq!(m.width(), 1);
    assert_eq!(m.at(1, 0), &num(2.0));
}

#[test]
fn uma_celula_so_nao_vira_matriz() {
    let f = Fake::com(&[("A1", num(1.0))]);
    assert!(!calcula_cru(&f, "A1:A1").is_matrix());
}

#[test]
fn intervalo_grande_demais_colapsa() {
    // Acima de 50 mil células o intervalo não vira matriz — devolve o
    // canto superior esquerdo em vez de alocar memória à toa.
    let f = Fake::com(&[("A1", num(7.0))]);
    assert_eq!(calcula(&f, "A:A"), num(7.0));
    assert!(!calcula_cru(&f, "A:A").is_matrix());
}

#[test]
fn conta_item_a_item() {
    let f = Fake::com(&[("A1", num(1.0)), ("A2", num(20.0)), ("A3", num(3.0))]);
    let m = calcula_cru(&f, "A1:A3>2");
    let m = m.as_matrix().expect("devia ser matriz");
    assert_eq!(m.at(0, 0), &Value::Bool(false));
    assert_eq!(m.at(1, 0), &Value::Bool(true));
    assert_eq!(m.at(2, 0), &Value::Bool(true));
}

#[test]
fn coluna_contra_linha_se_expande() {
    // {1;2} é coluna, {10\20\30} é linha: 2×1 contra 1×3 dá 2×3.
    let f = Fake::default();
    let m = calcula_cru(&f, "{1;2}*{10\\20\\30}");
    let m = m.as_matrix().expect("devia ser matriz");
    assert_eq!((m.height(), m.width()), (2, 3));
    assert_eq!(m.at(0, 0), &num(10.0));
    assert_eq!(m.at(1, 2), &num(60.0));
}

#[test]
fn matriz_num_lugar_escalar_vira_o_primeiro() {
    let f = Fake::com(&[("A1", num(5.0)), ("A2", num(6.0))]);
    assert_eq!(calcula(&f, "A1:A2"), num(5.0));
}

#[test]
fn menos_aplicado_a_matriz_anda_item_a_item() {
    let f = Fake::default();
    let m = calcula_cru(&f, "-{1;2}");
    let m = m.as_matrix().expect("devia ser matriz");
    assert_eq!(m.at(0, 0), &num(-1.0));
    assert_eq!(m.at(1, 0), &num(-2.0));
}

#[test]
fn intervalo_nomeado() {
    let mut f = Fake::com(&[("A1", num(1.0)), ("A2", num(2.0))]);
    f.nomes.insert("VENDAS".into(), Rect::new(0, 0, 1, 0));
    assert!(calcula_cru(&f, "Vendas").is_matrix());
    assert_eq!(calcula(&f, "Vendas"), num(1.0));
    assert_eq!(calcula(&f, "NãoExiste"), Value::erro(ErrorCode::Name));
}

#[test]
fn erro_literal_depois_de_separador_em_formula_completa() {
    // Regressão integrada: lexer, parser e avaliador agora reconhecem
    // o #N/A antes do ';', sem engolir o próximo argumento da função.
    let f = Fake::default();
    assert_eq!(calcula(&f, "SE(VERDADEIRO;#N/A;0)"), Value::erro(ErrorCode::Na));
    assert_eq!(calcula(&f, "SE(FALSO;#N/A;0)"), num(0.0));
    assert_eq!(calcula(&f, "1+#DIV/0!"), Value::erro(ErrorCode::Div0));
}

#[test]
fn intervalos_com_acentos_e_formulas_com_aspas_escapadas() {
    let f = Fake::com(&[("B2", num(42.0))]);
    assert_eq!(calcula(&f, "'São João'!B2"), num(42.0));
    assert_eq!(calcula(&f, "\"disse \"\"oi\"\"\""), Value::texto("disse \"oi\""));
}

#[test]
fn funcao_que_nao_existe() {
    let f = Fake::default();
    let v = calcula(&f, "BOBAGEM(1)");
    assert_eq!(v, Value::erro(ErrorCode::Name));
    match v {
        Value::Error(e) => assert!(e.detail.unwrap().contains("BOBAGEM")),
        _ => panic!(),
    }
}

#[test]
fn percorre_so_o_que_existe_no_intervalo() {
    let f = Fake::com(&[("A1", num(1.0)), ("A3", num(3.0))]);
    let ev = Evaluator::new(&f);
    let no = parse_formula("A1:A9").unwrap();
    let mut vistos = Vec::new();
    let erro = ev.for_each_value(&no, &mut |v, de_colecao| {
        assert!(de_colecao);
        vistos.push(v.clone());
    });
    assert!(erro.is_none());
    assert_eq!(vistos, vec![num(1.0), num(3.0)]);
}

#[test]
fn dentro_de_colecao_texto_nao_vira_numero() {
    let f = Fake::com(&[("A1", num(1.0)), ("A2", Value::texto("2")), ("A3", Value::Bool(true))]);
    let ev = Evaluator::new(&f);

    let mut dentro = Vec::new();
    ev.for_each_number(&parse_formula("A1:A3").unwrap(), &mut |n| dentro.push(n));
    assert_eq!(dentro, vec![1.0], "texto e lógico de dentro do intervalo não contam");

    let mut solto = Vec::new();
    ev.for_each_number(&parse_formula("\"2\"").unwrap(), &mut |n| solto.push(n));
    assert_eq!(solto, vec![2.0], "texto solto conta");
}

#[test]
fn erro_no_intervalo_e_devolvido() {
    let f = Fake::com(&[("A1", num(1.0)), ("A2", Value::erro(ErrorCode::Div0))]);
    let ev = Evaluator::new(&f);
    let erro = ev.for_each_value(&parse_formula("A1:A3").unwrap(), &mut |_, _| {});
    assert_eq!(erro.map(|e| e.code), Some(ErrorCode::Div0));
}

#[test]
fn retangulo_de_um_argumento() {
    let f = Fake::default();
    let ev = Evaluator::new(&f);
    assert_eq!(ev.rect(&parse_formula("A1:B2").unwrap()), Ok(Rect::new(0, 0, 1, 1)));
    assert_eq!(ev.rect(&parse_formula("C3").unwrap()), Ok(Rect::cell(2, 2)));
    assert!(ev.rect(&parse_formula("1+1").unwrap()).is_err());
}

#[test]
fn valor_que_responde_depois_sem_celula() {
    let f = Fake::default();
    let ev = Evaluator::new(&f);
    let r = ev.async_value("teste", Box::new(|| Computed::Scalar(num(1.0))));
    assert_eq!(r.scalar(), Value::erro(ErrorCode::Value));
}

#[test]
fn nome_local_ganha_do_intervalo_nomeado() {
    let mut f = Fake::com(&[("A1", num(1.0))]);
    f.nomes.insert("X".into(), Rect::cell(0, 0));
    let ev = Evaluator::new(&f);
    let com_x = ev.with_scope([("x", Computed::Scalar(num(99.0)))]);
    assert_eq!(com_x.eval(&parse_formula("X").unwrap()), num(99.0));
    assert_eq!(ev.eval(&parse_formula("X").unwrap()), num(1.0));
}

#[test]
fn funcao_da_planilha_e_chamada_com_os_argumentos() {
    let f = Fake::default();
    let ev = Evaluator::new(&f);
    // DOBRO = função(x) x*2
    let lambda = Lambda {
        params: vec!["X".into()],
        body: parse_formula("X*2").unwrap(),
        closure: HashMap::new(),
    };
    let com = ev.with_scope([("DOBRO", Computed::Lambda(Rc::new(lambda)))]);
    assert_eq!(com.eval(&parse_formula("DOBRO(21)").unwrap()), num(42.0));
}

#[test]
fn funcao_num_lugar_escalar_e_erro() {
    let lambda = Lambda {
        params: vec![],
        body: Node::Num(1.0),
        closure: HashMap::new(),
    };
    assert_eq!(
        Computed::Lambda(Rc::new(lambda)).scalar(),
        Value::erro(ErrorCode::Value)
    );
}

#[test]
fn test_let_lambda_map_reduce() {
    let f = Fake::default();
    let ev = Evaluator::new(&f);

    // LET(x, 10, y, 20, x + y) = 30
    assert_eq!(
        ev.eval(&parse_formula("LET(x; 10; y; 20; x + y)").unwrap()),
        num(30.0)
    );

    // DEIXAR(a; 5; a * 3) = 15
    assert_eq!(
        ev.eval(&parse_formula("DEIXAR(a; 5; a * 3)").unwrap()),
        num(15.0)
    );

    // MAPEAR com LAMBDA
    // MAP({1, 2, 3}, LAMBDA(x, x * 2)) = {2, 4, 6}
    let res_map = ev.eval_array(&parse_formula("MAP({1; 2; 3}; LAMBDA(x; x * 2))").unwrap());
    if let Computed::Matrix(m) = res_map {
        assert_eq!(m.at(0, 0), &num(2.0));
        assert_eq!(m.at(1, 0), &num(4.0));
        assert_eq!(m.at(2, 0), &num(6.0));
    } else {
        panic!("MAP deveria retornar matriz");
    }


    // REDUCE({1, 2, 3, 4}, 0, LAMBDA(acc, val, acc + val)) = 10
    assert_eq!(
        ev.eval(&parse_formula("REDUCE(0; {1; 2; 3; 4}; LAMBDA(acc; val; acc + val))").unwrap()),
        num(10.0)
    );
}

#[test]
fn test_regex_e_posicao() {
    let f = Fake::default();
    let ev = Evaluator::na_celula(&f, 4, 2); // Linha 5 (index 4), Coluna 3 (index 2)

    // REGEXMATCH / REGEXCORRESP
    assert_eq!(
        ev.eval(&parse_formula(r#"REGEXCORRESP("pedido 123"; "\d+")"#).unwrap()),
        Value::Bool(true)
    );
    assert_eq!(
        ev.eval(&parse_formula(r#"REGEXCORRESP("pedido"; "\d+")"#).unwrap()),
        Value::Bool(false)
    );

    // REGEXEXTRACT / REGEXEXTRAIR
    assert_eq!(
        ev.eval(&parse_formula(r#"REGEXEXTRAIR("pedido 123"; "\d+")"#).unwrap()),
        Value::Text("123".into())
    );

    // REGEXREPLACE / REGEXSUBSTITUIR
    assert_eq!(
        ev.eval(&parse_formula(r#"REGEXSUBSTITUIR("abc 123"; "\d+"; "xyz")"#).unwrap()),
        Value::Text("abc xyz".into())
    );

    // LIN e COL sem argumento na célula (5, 3) -> 5 e 3
    assert_eq!(ev.eval(&parse_formula("LIN()").unwrap()), num(5.0));
    assert_eq!(ev.eval(&parse_formula("COL()").unwrap()), num(3.0));

    // LIN(A10) -> 10
    assert_eq!(ev.eval(&parse_formula("LIN(A10)").unwrap()), num(10.0));
    assert_eq!(ev.eval(&parse_formula("COL(C1)").unwrap()), num(3.0));
}

#[test]
fn test_funcoes_registradas_novas() {
    let f = Fake::default();
    let ev = Evaluator::new(&f);

    // DESVPAD / STDEV
    assert_eq!(
        ev.eval(&parse_formula("DESVPAD(10; 20; 30)").unwrap()),
        num(10.0)
    );
    assert_eq!(
        ev.eval(&parse_formula("STDEV(10; 20; 30)").unwrap()),
        num(10.0)
    );

    // MODO / MODE
    assert_eq!(
        ev.eval(&parse_formula("MODO(1; 2; 2; 3)").unwrap()),
        num(2.0)
    );
    assert_eq!(
        ev.eval(&parse_formula("MODE(1; 2; 2; 3)").unwrap()),
        num(2.0)
    );
}

#[test]
fn identificador_com_parenteses_e_funcao_e_lambda_imediata() {
    use crate::planilha::sheet::Sheet;
    let mut s = Sheet::new("Plan1");
    s.set_cell_raw(0, 0, "=LOG10(100)".into(), None);
    s.set_cell_raw(0, 1, "=ATAN2(1;1)".into(), None);
    s.set_cell_raw(0, 2, "=LAMBDA(x;x+1)(5)".into(), None);
    s.set_cell_raw(0, 3, "=LET(f;LAMBDA(x;x*2);f(3))".into(), None);
    s.set_cell_raw(0, 4, "=LOG(8;2)".into(), None);
    s.set_cell_raw(1, 0, "7".into(), None);
    s.set_cell_raw(1, 1, "=A2/LOG10".into(), None);
    assert_eq!(s.get_value(0, 0), num(2.0));
    assert_eq!(s.get_value(0, 1), num(std::f64::consts::FRAC_PI_4));
    assert_eq!(s.get_value(0, 2), num(6.0));
    assert_eq!(s.get_value(0, 3), num(6.0));
    assert_eq!(s.get_value(0, 4), num(3.0));
    // LOG10 sem parênteses continua sendo referência (célula vazia = 0 → #DIV/0!).
    assert!(matches!(s.get_value(1, 1), Value::Error(_)));
}

#[test]
fn teste_qui_quadrado_exemplo_do_excel() {
    use crate::planilha::sheet::Sheet;
    let mut s = Sheet::new("Plan1");
    let reais = [[58.0, 35.0], [11.0, 25.0], [10.0, 23.0]];
    let esp = [[45.35, 47.65], [17.56, 18.44], [16.09, 16.91]];
    for r in 0..3 { for c in 0..2 {
        s.set_cell_raw(r, c, reais[r as usize][c as usize].to_string(), None);
        s.set_cell_raw(r, c + 2, esp[r as usize][c as usize].to_string(), None);
    }}
    s.set_cell_raw(5, 0, "=CHISQ.TEST(A1:B3;C1:D3)".into(), None);
    match s.get_value(5, 0) {
        Value::Number(v) => assert!((v - 0.000308192).abs() < 1e-8, "{v}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn intervalo_acima_de_50_mil_celulas_vira_matriz_inteira() {
    // #114: acima de 50 mil células o intervalo virava a primeira célula,
    // calado, e isto dava 0.
    let f = Fake::com(&[("A3", Value::texto("x")), ("A59999", Value::texto("x"))]);
    assert_eq!(calcula(&f, "SOMARPRODUTO((A1:A60000=\"x\")*1)"), num(2.0));
    // Linha a linha (só faixas e operadores) não monta matriz: conta inteira.
    assert_eq!(calcula(&f, "SOMARPRODUTO((A1:Z1048576=\"x\")*1)"), num(2.0));
    // Grande demais para virar matriz: erro com motivo, nunca valor errado.
    match calcula(&f, "SOMARPRODUTO(SE(VERDADEIRO;(A1:Z1048576=\"x\")*1))") {
        Value::Error(e) => assert_eq!(e.code, ErrorCode::Num),
        outro => panic!("devia dar #NÚM!, deu {outro:?}"),
    }
    // Contexto escalar continua lendo só a primeira célula.
    assert_eq!(calcula(&f, "A3:Z1048576"), Value::texto("x"));
}
