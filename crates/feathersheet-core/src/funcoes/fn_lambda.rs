//! Auxiliares de LAMBDA e agrupamento: PORLINHA, PORCOL, VARRER,
//! CRIARMATRIZ, ÉOMITIDO, AGRUPARPOR e PIVOTARPOR.
//!
//! A função pode vir como LAMBDA ou pelo nome de uma função da planilha
//! (`AGRUPARPOR(A2:A9;B2:B9;SOMA)`, `PORLINHA(A1:C3;MÁXIMO)`), como no Excel.

use std::collections::HashMap;
use std::rc::Rc;

use crate::celulas::values::{compare, to_boolean, to_text, Computed, Lambda, Matrix, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::{marca_omitido, ArgOrValue, Evaluator};
use crate::funcoes::fn_helpers::*;
use crate::funcoes::functions;

/// O que se aplica: uma LAMBDA ou uma função da tabela pelo nome.
pub enum Funcao {
    Lambda(Rc<Lambda>),
    Nome(String),
}

/// O argumento `i` como função.
pub fn funcao_do_arg(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Funcao> {
    let no = args.get(i).ok_or_else(e_valor)?;
    if let Node::Name(n) = no {
        if !ev.env.contains_key(&n.to_uppercase()) && functions::lookup(&n.to_uppercase()).is_some() {
            return Ok(Funcao::Nome(n.clone()));
        }
    }
    match ev.eval_array(no) {
        Computed::Lambda(l) => Ok(Funcao::Lambda(l)),
        Computed::Scalar(Value::Error(e)) => Err(e),
        _ => Err(e_valor()),
    }
}

fn literal(v: &Value) -> Node {
    match v {
        Value::Number(n) => Node::Num(*n),
        Value::Text(t) => Node::Str(t.clone()),
        Value::Bool(b) => Node::Bool(*b),
        Value::Empty => Node::Blank,
        Value::Error(e) => Node::Error(e.code.as_str().to_string()),
    }
}

/// Chama a função com estes valores já prontos.
pub fn aplicar(ev: &Evaluator<'_>, f: &Funcao, valores: Vec<Computed>) -> Computed {
    match f {
        Funcao::Lambda(l) => {
            let args: Vec<ArgOrValue<'_>> = valores.into_iter().map(ArgOrValue::Value).collect();
            ev.invoke_lambda(l, &args)
        }
        Funcao::Nome(nome) => {
            let args = valores
                .iter()
                .map(|c| match c {
                    Computed::Matrix(m) => Node::Array {
                        rows: m.rows().iter().map(|l| l.iter().map(literal).collect()).collect(),
                    },
                    Computed::Scalar(v) => literal(v),
                    Computed::Lambda(_) => Node::Error("#VALUE!".into()),
                })
                .collect();
            ev.eval_array(&Node::Call { name: nome.clone(), args })
        }
    }
}

/// O resultado tem de ser um valor só; matriz é `#CALC!`, como no Excel.
fn um_valor(c: Computed) -> Value {
    match c {
        Computed::Scalar(v) => v,
        Computed::Matrix(m) if m.size() == 1 => m.first().clone(),
        _ => Value::Error(e_calc()),
    }
}

fn coluna(valores: Vec<Value>) -> Computed {
    Computed::Matrix(Matrix::new(valores.into_iter().map(|v| vec![v]).collect()))
}

/// `=PORLINHA(matriz; lambda)` / `=BYROW(array, lambda)`
pub fn fn_byrow(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
        let m = arg_matriz(args, ev, 0)?;
        let f = funcao_do_arg(args, ev, 1)?;
        let linhas = m
            .rows()
            .iter()
            .map(|l| vec![um_valor(aplicar(ev, &f, vec![Computed::Matrix(Matrix::new(vec![l.clone()]))]))])
            .collect();
        montar_matriz(linhas)
    })
}

/// `=PORCOL(matriz; lambda)` / `=BYCOL(array, lambda)`
pub fn fn_bycol(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
        let m = arg_matriz(args, ev, 0)?;
        let f = funcao_do_arg(args, ev, 1)?;
        let linha = (0..m.width())
            .map(|c| {
                let col = (0..m.height()).map(|r| m.at(r, c).clone()).collect();
                um_valor(aplicar(ev, &f, vec![coluna(col)]))
            })
            .collect();
        montar_matriz(vec![linha])
    })
}

/// `=VARRER([valor_inicial]; matriz; lambda)` / `=SCAN([initial_value], array, lambda)`
pub fn fn_scan(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 3, || {
        let mut acc = match &args[0] {
            Node::Blank => Computed::Scalar(Value::Empty),
            no => ev.eval_array(no),
        };
        let m = arg_matriz(args, ev, 1)?;
        let f = funcao_do_arg(args, ev, 2)?;
        let mut linhas = Vec::with_capacity(m.height());
        for l in m.rows() {
            let mut saida = Vec::with_capacity(l.len());
            for v in l {
                acc = Computed::Scalar(um_valor(aplicar(ev, &f, vec![acc, Computed::Scalar(v.clone())])));
                saida.push(acc.clone().scalar());
            }
            linhas.push(saida);
        }
        montar_matriz(linhas)
    })
}

/// `=CRIARMATRIZ(linhas; colunas; lambda)` / `=MAKEARRAY(rows, cols, lambda)`
pub fn fn_makearray(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 3, || {
        let (h, w) = (arg_int(args, ev, 0)?, arg_int(args, ev, 1)?);
        if h < 1 || w < 1 {
            return Err(e_valor());
        }
        if h.saturating_mul(w) > MAX_CELULAS as i64 {
            return Err(e_num());
        }
        let f = funcao_do_arg(args, ev, 2)?;
        let num = |x: i64| Computed::Scalar(Value::Number(x as f64));
        montar_matriz(
            (1..=h).map(|r| (1..=w).map(|c| um_valor(aplicar(ev, &f, vec![num(r), num(c)]))).collect()).collect(),
        )
    })
}

/// `=ÉOMITIDO(argumento)` / `=ISOMITTED(argument)`
pub fn fn_isomitted(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || match &args[0] {
        Node::Name(n) => Ok(Value::Bool(ev.env.contains_key(&marca_omitido(n)))),
        Node::Blank => Ok(Value::Bool(true)),
        _ => Ok(Value::Bool(false)),
    })
}

// --- AGRUPARPOR e PIVOTARPOR ------------------------------------------------

/// Chave de agrupamento: o tipo entra junto, para `1` e `"1"` não se juntarem.
fn chave(vals: &[Value]) -> String {
    let mut s = String::new();
    for v in vals {
        let tipo = match v {
            Value::Number(_) => 'n',
            Value::Text(_) => 't',
            Value::Bool(_) => 'b',
            Value::Empty => 'e',
            Value::Error(_) => 'x',
        };
        s.push(tipo);
        s.push_str(&to_text(v).to_lowercase());
        s.push('\u{0}');
    }
    s
}

fn comparar(a: &Value, b: &Value) -> std::cmp::Ordering {
    compare(a, b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Os cabeçalhos: 0 não tem, 1 tem e não mostra, 2 não tem e gera, 3 tem e
/// mostra. Omitido: tem se a primeira linha dos valores é texto e a segunda
/// não. Devolve (tem cabeçalho, mostra).
fn modo_cabecalho(args: &[Node], ev: &Evaluator<'_>, i: usize, valores: &Matrix) -> Res<(bool, bool)> {
    if has(args, i) {
        return match arg_int(args, ev, i)? {
            0 => Ok((false, false)),
            1 => Ok((true, false)),
            2 => Ok((false, true)),
            3 => Ok((true, true)),
            _ => Err(e_valor()),
        };
    }
    let texto = |r: usize| (0..valores.width()).all(|c| matches!(valores.at(r, c), Value::Text(_)));
    let tem = valores.height() > 1 && texto(0) && !texto(1);
    Ok((tem, false))
}

/// Linhas que passam no filtro (sem a de cabeçalho).
fn linhas_filtradas(args: &[Node], ev: &Evaluator<'_>, i: usize, altura: usize, inicio: usize) -> Res<Vec<usize>> {
    if !has(args, i) {
        return Ok((inicio..altura).collect());
    }
    let filtro = arg_matriz(args, ev, i)?;
    let plano: Vec<&Value> = filtro.values().collect();
    if plano.len() != altura {
        return Err(e_valor());
    }
    let mut saida = Vec::new();
    for r in inicio..altura {
        if to_boolean(plano[r])? {
            saida.push(r);
        }
    }
    Ok(saida)
}

fn linha_de(m: &Matrix, r: usize) -> Vec<Value> {
    m.rows()[r].clone()
}

fn agregar(ev: &Evaluator<'_>, f: &Funcao, valores: &Matrix, col: usize, linhas: &[usize]) -> Value {
    if linhas.is_empty() {
        return Value::Empty;
    }
    um_valor(aplicar(ev, f, vec![coluna(linhas.iter().map(|&r| valores.at(r, col).clone()).collect())]))
}

/// Grupos na ordem de chegada: (valores da chave, linhas).
fn agrupar(campos: &Matrix, linhas: &[usize]) -> Vec<(Vec<Value>, Vec<usize>)> {
    let mut indice: HashMap<String, usize> = HashMap::new();
    let mut grupos: Vec<(Vec<Value>, Vec<usize>)> = Vec::new();
    for &r in linhas {
        let k = linha_de(campos, r);
        let pos = *indice.entry(chave(&k)).or_insert_with(|| {
            grupos.push((k, Vec::new()));
            grupos.len() - 1
        });
        grupos[pos].1.push(r);
    }
    grupos
}

/// A ordem pedida: inteiros (coluna 1, 2…; negativo = decrescente).
fn ordem_pedida(args: &[Node], ev: &Evaluator<'_>, i: usize, padrao: Vec<i64>) -> Res<Vec<i64>> {
    if !has(args, i) {
        return Ok(padrao);
    }
    arg_matriz(args, ev, i)?.values().map(|v| inteiro(v)).collect()
}

/// `=AGRUPARPOR(campos_linha; valores; função; [cabeçalhos]; [profundidade_total]; [ordem]; [filtro]; [relação])` / `=GROUPBY(row_fields, values, function, [field_headers], [total_depth], [sort_order], [filter_array], [field_relationship])`
pub fn fn_groupby(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 8, || {
        let campos = arg_matriz(args, ev, 0)?;
        let valores = arg_matriz(args, ev, 1)?;
        if campos.height() != valores.height() {
            return Err(e_valor());
        }
        let f = funcao_do_arg(args, ev, 2)?;
        let (tem_cab, mostra_cab) = modo_cabecalho(args, ev, 3, &valores)?;
        let profundidade = if has(args, 4) { arg_int(args, ev, 4)? } else { 1 };
        let k = campos.width();
        let ordem = ordem_pedida(args, ev, 5, (1..=k as i64).collect())?;
        let relacao_tabela = has(args, 7) && arg_int(args, ev, 7)? == 1;
        let linhas = linhas_filtradas(args, ev, 6, campos.height(), tem_cab as usize)?;
        let m = valores.width();

        let mut folhas: Vec<(Vec<Value>, Vec<Value>, Vec<usize>)> = agrupar(&campos, &linhas)
            .into_iter()
            .map(|(chave, ls)| {
                let res = (0..m).map(|c| agregar(ev, &f, &valores, c, &ls)).collect();
                (chave, res, ls)
            })
            .collect();
        let coluna_de = |linha: &(Vec<Value>, Vec<Value>, Vec<usize>), i: usize| -> Value {
            if i < k { linha.0[i].clone() } else { linha.1.get(i - k).cloned().unwrap_or(Value::Empty) }
        };
        folhas.sort_by(|a, b| {
            for &o in &ordem {
                let i = (o.unsigned_abs() as usize).saturating_sub(1);
                let c = comparar(&coluna_de(a, i), &coluna_de(b, i));
                let c = if o < 0 { c.reverse() } else { c };
                if c.is_ne() {
                    return c;
                }
            }
            std::cmp::Ordering::Equal
        });

        let total = |ls: &[usize]| -> Vec<Value> { (0..m).map(|c| agregar(ev, &f, &valores, c, ls)).collect() };
        let niveis_sub = if relacao_tabela { 0 } else { (profundidade.unsigned_abs() as usize).saturating_sub(1).min(k.saturating_sub(1)) };
        let no_topo = profundidade < 0;
        let mut saida: Vec<Vec<Value>> = Vec::new();
        if mostra_cab {
            let mut cab: Vec<Value> = (0..k)
                .map(|c| if tem_cab { campos.at(0, c).clone() } else { Value::Text(format!("Campo {}", c + 1)) })
                .collect();
            cab.extend((0..m).map(|c| if tem_cab { valores.at(0, c).clone() } else { Value::Text(format!("Valor {}", c + 1)) }));
            saida.push(cab);
        }
        let rotulo_total = |prefixo: &[Value]| -> Vec<Value> {
            let mut l: Vec<Value> = prefixo.to_vec();
            if l.is_empty() {
                l.push(Value::Text("Total".into()));
            }
            l.resize(k, Value::Empty);
            l
        };
        let geral = profundidade != 0;
        if geral && no_topo {
            let mut l = rotulo_total(&[]);
            l.extend(total(&linhas));
            saida.push(l);
        }
        if niveis_sub == 0 {
            saida.extend(folhas.iter().map(|(ch, res, _)| ch.iter().chain(res).cloned().collect()));
        } else {
            // Subtotal do primeiro nível de campos: as folhas ficam juntas pelo prefixo, na ordem já feita.
            let mut prefixos: Vec<(Vec<Value>, Vec<usize>)> = Vec::new();
            let mut pos: HashMap<String, usize> = HashMap::new();
            for (i, (ch, _, _)) in folhas.iter().enumerate() {
                let p = ch[..niveis_sub].to_vec();
                let j = *pos.entry(chave(&p)).or_insert_with(|| {
                    prefixos.push((p, Vec::new()));
                    prefixos.len() - 1
                });
                prefixos[j].1.push(i);
            }
            for (p, idx) in prefixos {
                let ls: Vec<usize> = idx.iter().flat_map(|&i| folhas[i].2.iter().copied()).collect();
                let mut sub = rotulo_total(&p);
                sub.extend(total(&ls));
                if no_topo {
                    saida.push(sub.clone());
                }
                saida.extend(idx.iter().map(|&i| folhas[i].0.iter().chain(&folhas[i].1).cloned().collect()));
                if !no_topo {
                    saida.push(sub);
                }
            }
        }
        if geral && !no_topo {
            let mut l = rotulo_total(&[]);
            l.extend(total(&linhas));
            saida.push(l);
        }
        montar_matriz(saida)
    })
}

/// `=PIVOTARPOR(campos_linha; campos_coluna; valores; função; [cabeçalhos]; [total_linhas]; [ordem_linhas]; [total_colunas]; [ordem_colunas]; [filtro]; [relativo_a])` / `=PIVOTBY(row_fields, col_fields, values, function, [field_headers], [row_total_depth], [row_sort_order], [col_total_depth], [col_sort_order], [filter_array], [relative_to])`
pub fn fn_pivotby(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 4, 11, || {
        let campos_l = arg_matriz(args, ev, 0)?;
        let campos_c = arg_matriz(args, ev, 1)?;
        let valores = arg_matriz(args, ev, 2)?;
        let h = valores.height();
        if campos_l.height() != h || campos_c.height() != h {
            return Err(e_valor());
        }
        let f = funcao_do_arg(args, ev, 3)?;
        let (tem_cab, mostra_cab) = modo_cabecalho(args, ev, 4, &valores)?;
        // shortcut: só o total geral (profundidade 2 vale como 1) e ordem pelas chaves; relativo_a é ignorado.
        let total_l = if has(args, 5) { arg_int(args, ev, 5)? } else { 1 };
        let total_c = if has(args, 7) { arg_int(args, ev, 7)? } else { 1 };
        let desc = |i: usize| -> Res<bool> { Ok(ordem_pedida(args, ev, i, vec![1])?.first().is_some_and(|o| *o < 0)) };
        let (desc_l, desc_c) = (desc(6)?, desc(8)?);
        let linhas = linhas_filtradas(args, ev, 9, h, tem_cab as usize)?;
        let (kl, kc, m) = (campos_l.width(), campos_c.width(), valores.width());

        let ordenar = |mut g: Vec<(Vec<Value>, Vec<usize>)>, desc: bool| {
            g.sort_by(|a, b| {
                let c = a.0.iter().zip(&b.0).map(|(x, y)| comparar(x, y)).find(|c| c.is_ne()).unwrap_or(std::cmp::Ordering::Equal);
                if desc { c.reverse() } else { c }
            });
            g
        };
        let grupos_l = ordenar(agrupar(&campos_l, &linhas), desc_l);
        let grupos_c = ordenar(agrupar(&campos_c, &linhas), desc_c);
        let em = |ls: &[usize], outro: &[usize]| -> Vec<usize> { ls.iter().copied().filter(|r| outro.contains(r)).collect() };
        let celulas = |ls: &[usize]| -> Vec<Value> { (0..m).map(|c| agregar(ev, &f, &valores, c, ls)).collect() };
        let nome_valor = |c: usize| if tem_cab { valores.at(0, c).clone() } else { Value::Text(format!("Valor {}", c + 1)) };

        let mut colunas: Vec<(Vec<Value>, Vec<usize>)> = grupos_c.clone();
        if total_c != 0 {
            let mut rot = vec![Value::Text("Total".into())];
            rot.resize(kc, Value::Empty);
            let item = (rot, linhas.clone());
            if total_c < 0 { colunas.insert(0, item) } else { colunas.push(item) }
        }
        let mut saida: Vec<Vec<Value>> = Vec::new();
        for nivel in 0..kc {
            let mut l = vec![Value::Empty; kl];
            for (ch, _) in &colunas {
                l.push(ch[nivel].clone());
                l.extend(std::iter::repeat_n(Value::Empty, m - 1));
            }
            saida.push(l);
        }
        if m > 1 || mostra_cab {
            let mut l: Vec<Value> = (0..kl)
                .map(|c| if !mostra_cab { Value::Empty } else if tem_cab { campos_l.at(0, c).clone() } else { Value::Text(format!("Campo {}", c + 1)) })
                .collect();
            for _ in &colunas {
                l.extend((0..m).map(nome_valor));
            }
            saida.push(l);
        }
        let mut corpo: Vec<(Vec<Value>, Vec<usize>)> = grupos_l;
        if total_l != 0 {
            let mut rot = vec![Value::Text("Total".into())];
            rot.resize(kl, Value::Empty);
            let item = (rot, linhas.clone());
            if total_l < 0 { corpo.insert(0, item) } else { corpo.push(item) }
        }
        for (ch, ls) in &corpo {
            let mut l = ch.clone();
            for (_, cs) in &colunas {
                l.extend(celulas(&em(ls, cs)));
            }
            saida.push(l);
        }
        montar_matriz(saida)
    })
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::Value;
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }
    fn t(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn porlinha_porcol_varrer_criarmatriz() {
        let cel = [("A1", n(1.0)), ("B1", n(2.0)), ("A2", n(3.0)), ("B2", n(4.0))];
        let m = calc_com(&cel, "PORLINHA(A1:B2;LAMBDA(l;SOMA(l)))");
        assert_eq!(m.as_matrix().unwrap().rows(), &[vec![n(3.0)], vec![n(7.0)]]);
        let m = calc_com(&cel, "BYCOL(A1:B2;MÁXIMO)");
        assert_eq!(m.as_matrix().unwrap().rows(), &[vec![n(3.0), n(4.0)]]);
        let m = calc_com(&cel, "VARRER(0;A1:B2;LAMBDA(a;v;a+v))");
        assert_eq!(m.as_matrix().unwrap().rows(), &[vec![n(1.0), n(3.0)], vec![n(6.0), n(10.0)]]);
        let m = calc_matriz("CRIARMATRIZ(2;3;LAMBDA(l;c;l*c))");
        assert_eq!(m, vec![vec![n(1.0), n(2.0), n(3.0)], vec![n(2.0), n(4.0), n(6.0)]]);
    }

    #[test]
    fn variavel_de_let_vale_em_funcao_de_colecao() {
        let cel = [("A1", n(1.0)), ("A2", n(2.0)), ("A3", n(3.0))];
        assert_eq!(calc_cel(&cel, "DEIXAR(x;A1:A3;SOMA(x))"), n(6.0));
        assert_eq!(calc_cel(&cel, "LET(x;A1:A3;CONT.NÚM(x))"), n(3.0));
    }

    #[test]
    fn eomitido_ve_o_parametro_que_faltou() {
        assert_eq!(calc("LAMBDA(a;b;SE(ÉOMITIDO(b);a;a+b))(5)"), n(5.0));
        assert_eq!(calc("LAMBDA(a;b;SE(ISOMITTED(b);a;a+b))(5;2)"), n(7.0));
    }

    #[test]
    fn agruparpor_soma_e_total() {
        let cel = [
            ("A1", t("Região")), ("B1", t("Vendas")),
            ("A2", t("Sul")), ("B2", n(10.0)),
            ("A3", t("Norte")), ("B3", n(5.0)),
            ("A4", t("Sul")), ("B4", n(1.0)),
        ];
        let m = calc_com(&cel, "AGRUPARPOR(A1:A4;B1:B4;SOMA)");
        assert_eq!(m.as_matrix().unwrap().rows(), &[
            vec![t("Norte"), n(5.0)],
            vec![t("Sul"), n(11.0)],
            vec![t("Total"), n(16.0)],
        ]);
        let m = calc_com(&cel, "GROUPBY(A2:A4;B2:B4;LAMBDA(x;CONT.NÚM(x));0;0;-2)");
        assert_eq!(m.as_matrix().unwrap().rows(), &[vec![t("Sul"), n(2.0)], vec![t("Norte"), n(1.0)]]);
    }

    #[test]
    fn pivotarpor_cruza_linhas_e_colunas() {
        let cel = [
            ("A1", t("Sul")), ("B1", t("Maçã")), ("C1", n(1.0)),
            ("A2", t("Sul")), ("B2", t("Uva")), ("C2", n(2.0)),
            ("A3", t("Norte")), ("B3", t("Maçã")), ("C3", n(4.0)),
        ];
        let m = calc_com(&cel, "PIVOTARPOR(A1:A3;B1:B3;C1:C3;SOMA)");
        assert_eq!(m.as_matrix().unwrap().rows(), &[
            vec![Value::Empty, t("Maçã"), t("Uva"), t("Total")],
            vec![t("Norte"), n(4.0), Value::Empty, n(4.0)],
            vec![t("Sul"), n(1.0), n(2.0), n(3.0)],
            vec![t("Total"), n(5.0), n(2.0), n(7.0)],
        ]);
    }
}
