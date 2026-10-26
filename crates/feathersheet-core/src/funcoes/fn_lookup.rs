//! Funções de busca e lógica em Rust: SE, SES, SEERRO, E, OU, NÃO, PROCV, ÍNDICE, CORRESP.

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::{broadcast_all, Evaluator};
use crate::funcoes::fn_extra::posicao_xmatch;
use crate::funcoes::fn_helpers::{arity, err_na, err_value, has, valores_iguais, LeitorFaixa};
use crate::celulas::values::{compare, to_boolean, CellError, Computed, ErrorCode, Value};


/// `=SE(teste; valor_se_verdadeiro; [valor_se_falso])` / `=IF(...)`
pub fn fn_if(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let cond_val = match ev.eval_array(&args[0]) {
        Computed::Scalar(v) => v,
        // Teste em matriz (`SE(A1:A3>1;…)`): escolhe posição a posição. Só
        // aqui os dois ramos são avaliados; com teste escalar fica preguiçoso.
        matriz => {
            let senao = if has(args, 2) { ev.eval_array(&args[2]) } else { Computed::Scalar(Value::Bool(false)) };
            return broadcast_all(&[matriz, ev.eval_array(&args[1]), senao], |vs| match to_boolean(&vs[0]) {
                Ok(true) => vs[1].clone(),
                Ok(false) => vs[2].clone(),
                Err(e) => Value::Error(e),
            });
        }
    };
    let b = match to_boolean(&cond_val) {
        Ok(val) => val,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    if b {
        ev.eval_array(&args[1])
    } else if has(args, 2) {
        ev.eval_array(&args[2])
    } else {
        Computed::Scalar(Value::Bool(false))
    }
}

/// `=SEERRO(valor; valor_se_erro)` / `=IFERROR(...)`
pub fn fn_iferror(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.eval_array(&args[0]);
    match &val {
        Computed::Scalar(Value::Error(_)) => ev.eval_array(&args[1]),
        _ => val,
    }
}

/// `=E(args...)` / `=AND(args...)`
pub fn fn_and(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.is_empty() {
        return err_value();
    }
    for a in args {
        let val = ev.eval(a);
        let b = match to_boolean(&val) {
            Ok(v) => v,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        };
        if !b {
            return Computed::Scalar(Value::Bool(false));
        }
    }
    Computed::Scalar(Value::Bool(true))
}

/// `=OU(args...)` / `=OR(args...)`
pub fn fn_or(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.is_empty() {
        return err_value();
    }
    for a in args {
        let val = ev.eval(a);
        let b = match to_boolean(&val) {
            Ok(v) => v,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        };
        if b {
            return Computed::Scalar(Value::Bool(true));
        }
    }
    Computed::Scalar(Value::Bool(false))
}

/// `=NÃO(logico)` / `=NOT(logico)`
pub fn fn_not(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.eval(&args[0]);
    match to_boolean(&val) {
        Ok(b) => Computed::Scalar(Value::Bool(!b)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=PROCV(valor_procurado; matriz_tabela; num_indice_coluna; [procurar_intervalo])` / `=VLOOKUP(...)`
pub fn fn_vlookup(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let lookup_val = ev.eval(&args[0]);
    if let Value::Error(e) = lookup_val {
        return Computed::Scalar(Value::Error(e));
    }

    let rect = match node_rect(&args[1], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let col_idx = match ev.number(&args[2]) {
        Ok(n) => n.trunc() as u32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    if col_idx < 1 || (rect.c0 + col_idx - 1) > rect.c1 {
        return Computed::Scalar(Value::erro(ErrorCode::Ref));
    }

    let modo = match modo_procurar(args, 3, ev, &lookup_val) {
        Ok(m) => m,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let target_col = rect.c0 + col_idx - 1;
    let n = (rect.r1 - rect.r0 + 1) as usize;
    let aba = rect.sheet.as_deref();
    match posicao_xmatch(n, |i| ev.ctx.get_value(rect.r0 + i as u32, rect.c0, aba), &lookup_val, modo, 1) {
        Ok(i) => Computed::Scalar(ev.ctx.get_value(rect.r0 + i as u32, target_col, aba)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// O `procurar_intervalo` de PROCV/PROCH (omitido = VERDADEIRO, aproximado,
/// como no Excel) no `modo` do CORRESPX.
fn modo_procurar(args: &[Node], i: usize, ev: &Evaluator<'_>, alvo: &Value) -> Result<i64, CellError> {
    let aproximado = if has(args, i) { to_boolean(&ev.eval(&args[i]))? } else { true };
    Ok(modo_corresp(if aproximado { 1 } else { 0 }, alvo))
}

/// Tipo de correspondência do CORRESP (1, 0, -1) no `modo` do CORRESPX: 1 é
/// "igual ou o maior menor", -1 "igual ou o menor maior", e 0 exato, com
/// curinga quando o texto tem `*` ou `?`. shortcut: varre tudo em vez da busca
/// binária do Excel; com dados ordenados o resultado é o mesmo.
fn modo_corresp(tipo: i64, alvo: &Value) -> i64 {
    match tipo {
        t if t > 0 => -1,
        t if t < 0 => 1,
        _ if matches!(alvo, Value::Text(t) if t.contains('*') || t.contains('?')) => 2,
        _ => 0,
    }
}

/// `=PROCH(valor_procurado; matriz_tabela; num_indice_linha; [procurar_intervalo])` / `=HLOOKUP(...)`
pub fn fn_hlookup(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let lookup_val = ev.eval(&args[0]);
    if let Value::Error(e) = lookup_val {
        return Computed::Scalar(Value::Error(e));
    }

    let rect = match node_rect(&args[1], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let row_idx = match ev.number(&args[2]) {
        Ok(n) => n.trunc() as u32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    if row_idx < 1 || (rect.r0 + row_idx - 1) > rect.r1 {
        return Computed::Scalar(Value::erro(ErrorCode::Ref));
    }

    let modo = match modo_procurar(args, 3, ev, &lookup_val) {
        Ok(m) => m,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let target_row = rect.r0 + row_idx - 1;
    let n = (rect.c1 - rect.c0 + 1) as usize;
    let aba = rect.sheet.as_deref();
    match posicao_xmatch(n, |i| ev.ctx.get_value(rect.r0, rect.c0 + i as u32, aba), &lookup_val, modo, 1) {
        Ok(i) => Computed::Scalar(ev.ctx.get_value(target_row, rect.c0 + i as u32, aba)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=CORRESP(valor_procurado; matriz_procurada; [tipo_corresp])` / `=MATCH(...)`
pub fn fn_match(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let lookup_val = ev.eval(&args[0]);
    if let Value::Error(e) = lookup_val {
        return Computed::Scalar(Value::Error(e));
    }

    let match_type = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n.trunc() as i64,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };
    let modo = modo_corresp(match_type, &lookup_val);

    // Intervalo: lê célula a célula, sem copiar; matriz (`{1;2;3}`) vem inteira.
    let achado = if let Some(rect) = node_rect(&args[1], ev) {
        let (linha, aba) = (rect.r0 == rect.r1, rect.sheet.as_deref());
        if !linha && rect.c0 != rect.c1 {
            return err_na();
        }
        let n = if linha { rect.c1 - rect.c0 + 1 } else { rect.r1 - rect.r0 + 1 } as usize;
        posicao_xmatch(n, |i| {
            let i = i as u32;
            if linha { ev.ctx.get_value(rect.r0, rect.c0 + i, aba) } else { ev.ctx.get_value(rect.r0 + i, rect.c0, aba) }
        }, &lookup_val, modo, 1)
    } else {
        match ev.matrix_arg(&args[1]) {
            Ok(m) if m.rows().len() > 1 && m.rows().first().map_or(0, Vec::len) > 1 => return err_na(),
            Ok(m) => {
                let itens: Vec<Value> = m.rows().iter().flatten().cloned().collect();
                posicao_xmatch(itens.len(), |i| itens[i].clone(), &lookup_val, modo, 1)
            }
            Err(e) => Err(e),
        }
    };
    match achado {
        Ok(i) => Computed::Scalar(Value::Number((i + 1) as f64)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ÍNDICE(matriz; num_linha; [num_coluna])` / `=INDEX(...)`
pub fn fn_index(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    // Tradução do `INDEX` do `fn-lookup.ts`, na mesma ordem: linha e coluna
    // primeiro, depois a fonte. A versão anterior só aceitava intervalo
    // (`ÍNDICE(MATRIZ.MULT(...); 2; 2)` dava `#VALUE!`), levava linha 0 para a
    // primeira linha em vez de `#REF!`, e não tinha a regra da linha única.
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let row_arg = match ev.number(&args[1]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let col_arg = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1.0
    };

    let rect = node_rect(&args[0], ev);
    let matriz = if rect.is_some() {
        None
    } else {
        match ev.eval_array(&args[0]) {
            Computed::Matrix(m) => Some(m.rows().to_vec()),
            Computed::Scalar(Value::Error(e)) => return Computed::Scalar(Value::Error(e)),
            Computed::Scalar(v) => Some(vec![vec![v]]),
            lambda @ Computed::Lambda(_) => return Computed::Scalar(lambda.scalar()),
        }
    };
    let (altura, largura) = match (&rect, &matriz) {
        (Some(r), _) => ((r.r1 - r.r0 + 1) as i64, (r.c1 - r.c0 + 1) as i64),
        (None, Some(m)) => (m.len() as i64, m.first().map_or(0, |l| l.len()) as i64),
        (None, None) => unreachable!(),
    };

    let mut r = row_arg.trunc() as i64;
    let mut c = col_arg.trunc() as i64;
    // Uma linha só e dois argumentos: o número é a coluna (`ÍNDICE(A1:E1; 3)`).
    if args.len() == 2 && altura == 1 {
        c = r;
        r = 1;
    }
    if r < 1 || r > altura || c < 1 || c > largura {
        return Computed::Scalar(Value::erro(ErrorCode::Ref));
    }
    let valor = match (&rect, &matriz) {
        (Some(rect), _) => ev.ctx.get_value(rect.r0 + (r - 1) as u32, rect.c0 + (c - 1) as u32, rect.sheet.as_deref()),
        (None, Some(m)) => m[(r - 1) as usize][(c - 1) as usize].clone(),
        (None, None) => unreachable!(),
    };
    Computed::Scalar(valor)
}

/// `=PROCX(pesquisa_valor; pesquisa_matriz; matriz_retorno; [se_não_encontrado]; [modo_correspondência]; [modo_pesquisa])` / `=XLOOKUP(...)`
pub fn fn_xlookup(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 6) {
        return Computed::Scalar(Value::Error(err));
    }
    let lookup_val = ev.eval(&args[0]);
    if let Value::Error(e) = lookup_val {
        return Computed::Scalar(Value::Error(e));
    }

    let from_rect = match node_rect(&args[1], ev) {
        Some(r) => r,
        None => return err_value(),
    };
    let to_rect = match node_rect(&args[2], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let mode = if has(args, 4) {
        match ev.number(&args[4]) {
            Ok(n) => n.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        0
    };

    let is_row_vector = from_rect.r0 == from_rect.r1;
    let count = if is_row_vector {
        from_rect.c1 - from_rect.c0 + 1
    } else {
        from_rect.r1 - from_rect.r0 + 1
    };

    // Lidas de uma vez: a mesma lista de produtos é consultada por cada uma
    // das milhões de linhas que a procuram.
    let de = LeitorFaixa::novo(ev, &from_rect);
    let para = LeitorFaixa::novo(ev, &to_rect);
    let from_cell = |idx: u32| {
        if is_row_vector {
            de.valor(ev, from_rect.r0, from_rect.c0 + idx)
        } else {
            de.valor(ev, from_rect.r0 + idx, from_rect.c0)
        }
    };

    let to_cell = |idx: u32| {
        if is_row_vector {
            para.valor(ev, to_rect.r0, to_rect.c0 + idx).into_owned()
        } else {
            para.valor(ev, to_rect.r0 + idx, to_rect.c0).into_owned()
        }
    };

    // Busca exata com a lista lida de uma vez: o índice da lista responde
    // sem varrer (1 milhão de `PROCX` × 200 produtos eram 200 milhões de
    // comparações). Onde o índice não se aplica, varre como sempre.
    if mode == 0 {
        if let Some(achado) = de.lida().and_then(|f| f.posicao_exata(&lookup_val)) {
            return match achado {
                Some(idx) if idx < count => Computed::Scalar(to_cell(idx)),
                _ if has(args, 3) => ev.eval_array(&args[3]),
                _ => err_na(),
            };
        }
    }

    let mut fallback_idx = None;

    for idx in 0..count {
        let v = from_cell(idx);
        let v: &Value = &v;
        if mode == 0 || mode == 2 {
            if valores_iguais(v, &lookup_val) {
                return Computed::Scalar(to_cell(idx));
            }
        } else {
            if compare(v, &lookup_val) == Ok(std::cmp::Ordering::Equal) {
                return Computed::Scalar(to_cell(idx));
            }
            if mode == -1 && compare(v, &lookup_val) == Ok(std::cmp::Ordering::Less) {
                if let Some(fb) = fallback_idx {
                    if compare(v, &from_cell(fb)) == Ok(std::cmp::Ordering::Greater) {
                        fallback_idx = Some(idx);
                    }
                } else {
                    fallback_idx = Some(idx);
                }
            } else if mode == 1 && compare(v, &lookup_val) == Ok(std::cmp::Ordering::Greater) {
                if let Some(fb) = fallback_idx {
                    if compare(v, &from_cell(fb)) == Ok(std::cmp::Ordering::Less) {
                        fallback_idx = Some(idx);
                    }
                } else {
                    fallback_idx = Some(idx);
                }
            }
        }
    }

    if let Some(fb) = fallback_idx {
        return Computed::Scalar(to_cell(fb));
    }

    if has(args, 3) {
        ev.eval_array(&args[3])
    } else {
        err_na()
    }
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }
    fn t(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn procv_e_corresp_aproximados_curinga_e_maiuscula() {
        let cel = [("A1", n(0.0)), ("A2", n(100.0)), ("A3", n(500.0)),
            ("B1", t("x")), ("B2", t("y")), ("B3", t("z")), ("D1", t("ABC")), ("D2", t("ABD"))];
        is_text(calc_cel(&cel, "PROCV(250;A1:B3;2;VERDADEIRO)"), "y");
        is_text(calc_cel(&cel, "PROCV(250;A1:B3;2)"), "y");
        is_err(calc_cel(&cel, "PROCV(250;A1:B3;2;FALSO)"), ErrorCode::Na);
        let lin = [("A1", n(0.0)), ("B1", n(100.0)), ("C1", n(500.0)), ("B2", t("y"))];
        is_text(calc_cel(&lin, "PROCH(250;A1:C2;2)"), "y");
        close(calc_cel(&cel, "CORRESP(250;A1:A3)"), 2.0);
        close(calc_cel(&cel, "CORRESP(\"abc\";D1:D2;0)"), 1.0);
        close(calc_cel(&cel, "CORRESP(\"AB?\";D1:D2;0)"), 1.0);
        close(calc_cel(&cel, "CORRESP(250;{0;100;500})"), 2.0);
        close(calc_cel(&[("A1", t("ABC")), ("B1", n(7.0))], "PROCV(\"ab*\";A1:B1;2;0)"), 7.0);
    }

    #[test]
    fn se_com_teste_em_matriz_escolhe_por_posicao() {
        close(calc_cel(&[("A3", n(1.0)), ("A4", n(2.0))], "SOMA(SE(A3:A4>1;1;0))"), 1.0);
        assert_eq!(calc_matriz("SE({1;2;3}>1;1;0)"), vec![vec![n(0.0)], vec![n(1.0)], vec![n(1.0)]]);
    }
}
