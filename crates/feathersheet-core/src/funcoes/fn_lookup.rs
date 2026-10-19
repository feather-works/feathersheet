//! Funções de busca e lógica em Rust: SE, SES, SEERRO, E, OU, NÃO, PROCV, ÍNDICE, CORRESP.

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, err_na, err_value, has, valores_iguais, LeitorFaixa};
use crate::celulas::values::{compare, to_boolean, Computed, ErrorCode, Value};


/// `=SE(teste; valor_se_verdadeiro; [valor_se_falso])` / `=IF(...)`
pub fn fn_if(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let cond_val = ev.eval(&args[0]);
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

    let exact_match = if has(args, 3) {
        match to_boolean(&ev.eval(&args[3])) {
            Ok(b) => !b,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        true
    };

    let target_col = rect.c0 + col_idx - 1;

    for r in rect.r0..=rect.r1 {
        let cell_val = ev.ctx.get_value(r, rect.c0, rect.sheet.as_deref());
        if exact_match {
            if cell_val == lookup_val {
                let res = ev.ctx.get_value(r, target_col, rect.sheet.as_deref());
                return Computed::Scalar(res);
            }
        } else {
            if compare(&cell_val, &lookup_val) == Ok(std::cmp::Ordering::Equal) {
                let res = ev.ctx.get_value(r, target_col, rect.sheet.as_deref());
                return Computed::Scalar(res);
            }
        }
    }

    err_na()
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

    let exact_match = if has(args, 3) {
        match to_boolean(&ev.eval(&args[3])) {
            Ok(b) => !b,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        true
    };

    let target_row = rect.r0 + row_idx - 1;

    for c in rect.c0..=rect.c1 {
        let cell_val = ev.ctx.get_value(rect.r0, c, rect.sheet.as_deref());
        if exact_match {
            if cell_val == lookup_val {
                let res = ev.ctx.get_value(target_row, c, rect.sheet.as_deref());
                return Computed::Scalar(res);
            }
        } else {
            if compare(&cell_val, &lookup_val) == Ok(std::cmp::Ordering::Equal) {
                let res = ev.ctx.get_value(target_row, c, rect.sheet.as_deref());
                return Computed::Scalar(res);
            }
        }
    }

    err_na()
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

    let rect = match node_rect(&args[1], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let match_type = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };

    let is_row_vector = rect.r0 == rect.r1;
    let count = if is_row_vector {
        rect.c1 - rect.c0 + 1
    } else {
        rect.r1 - rect.r0 + 1
    };

    for idx in 0..count {
        let (r, c) = if is_row_vector {
            (rect.r0, rect.c0 + idx)
        } else {
            (rect.r0 + idx, rect.c0)
        };
        let val = ev.ctx.get_value(r, c, rect.sheet.as_deref());
        if match_type == 0 {
            if val == lookup_val {
                return Computed::Scalar(Value::Number((idx + 1) as f64));
            }
        } else {
            if compare(&val, &lookup_val) == Ok(std::cmp::Ordering::Equal) {
                return Computed::Scalar(Value::Number((idx + 1) as f64));
            }
        }
    }

    err_na()
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
