//! Funções estatísticas em Rust: MÉDIA, MÁXIMO, MÍNIMO, CONT.NÚM, CONT.VALORES, CONT.SE, etc.

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, collect, err_na, err_num, err_value, has, make_predicate, LeitorFaixa};
use crate::celulas::refs::Rect;
use crate::funcoes::fn_helpers::{concluir, concluir_matriz, e_div0, e_nd, e_num, e_valor, numero, numeros_de, numeros_de_todos, por_elemento, valores_de, Res};
use crate::funcoes::fn_math::pares_numericos;
use crate::celulas::values::to_number;
use crate::celulas::values::{CellError, Computed, ErrorCode, Value};


/// Agregação numérica independente do parser/evaluator: compartilhada com
/// WorksheetFunction das macros, para não implementar outra média.
pub fn media_numerica(nums: &[f64]) -> Result<f64, ErrorCode> {
    if nums.is_empty() { return Err(ErrorCode::Div0); }
    Ok(nums.iter().copied().sum::<f64>() / nums.len() as f64)
}

/// Mediana canônica, usada por fórmulas e macros. Ordena apenas a fatia
/// fornecida, sem clonar os elementos.
pub fn mediana_numerica(nums: &mut [f64]) -> Result<f64, ErrorCode> {
    if nums.is_empty() { return Err(ErrorCode::Num); }
    nums.sort_by(|a,b|a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = nums.len() / 2;
    Ok(if nums.len()%2 == 0 {(nums[mid-1] + nums[mid]) / 2.0} else {nums[mid]})
}

/// `=MÉDIA(args...)` / `=AVERAGE(args...)`
pub fn fn_average(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => match media_numerica(&nums) {
            Ok(n) => Computed::Scalar(Value::Number(n)),
            Err(e) => Computed::Scalar(Value::erro(e)),
        },
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=MÁXIMO(args...)` / `=MAX(args...)`
pub fn fn_max(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            if nums.is_empty() {
                Computed::Scalar(Value::Number(0.0))
            } else {
                let mut max_v = f64::NEG_INFINITY;
                for n in nums {
                    if n > max_v {
                        max_v = n;
                    }
                }
                Computed::Scalar(Value::Number(max_v))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=MÍNIMO(args...)` / `=MIN(args...)`
pub fn fn_min(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            if nums.is_empty() {
                Computed::Scalar(Value::Number(0.0))
            } else {
                let mut min_v = f64::INFINITY;
                for n in nums {
                    if n < min_v {
                        min_v = n;
                    }
                }
                Computed::Scalar(Value::Number(min_v))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=CONT.NÚM(args...)` / `=COUNT(args...)`
pub fn fn_count(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => Computed::Scalar(Value::Number(nums.len() as f64)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=CONT.VALORES(args...)` / `=COUNTA(args...)`
pub fn fn_counta(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    // Cada célula do intervalo conta, não o intervalo inteiro como uma coisa
    // só: o `ev.eval` reduzia `B1:B5` ao primeiro valor e contava 1. Mesma
    // regra do TypeScript: vazio e texto vazio não contam, erro interrompe.
    let mut count = 0.0;
    for a in args {
        let erro = ev.for_each_value(a, &mut |v, _| {
            let vazio = matches!(v, Value::Empty) || matches!(v, Value::Text(t) if t.is_empty());
            if !vazio {
                count += 1.0;
            }
        });
        if let Some(e) = erro {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Number(count))
}

/// `=CONT.SE(faixa; criterio)` / `=COUNTIF(faixa; criterio)`
pub fn fn_countif(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let range_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };
    let crit_val = ev.eval(&args[1]);
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, None, std::slice::from_ref(&range_rect), std::slice::from_ref(&crit_val)) {
        return Computed::Scalar(Value::Number(grupo.linhas as f64));
    }
    let pred = make_predicate(&crit_val);

    let criterio = LeitorFaixa::novo(ev, &range_rect);
    let mut count = 0.0;
    for r in range_rect.r0..=range_rect.r1 {
        for c in range_rect.c0..=range_rect.c1 {
            let val = criterio.valor(ev, r, c);
            if pred(&val) {
                count += 1.0;
            }
        }
    }

    Computed::Scalar(Value::Number(count))
}

/// `=DESVPAD.P(args...)` / `=STDEV.P(args...)`
pub fn fn_stdev_p(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            let n = nums.len();
            if n == 0 {
                return Computed::Scalar(Value::erro(ErrorCode::Div0));
            }
            let mean: f64 = nums.iter().sum::<f64>() / n as f64;
            let variance: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
            Computed::Scalar(Value::Number(variance.sqrt()))
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=DESVPAD.A(args...)` / `=STDEV.S(args...)`
pub fn fn_stdev_s(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            let n = nums.len();
            if n <= 1 {
                return Computed::Scalar(Value::erro(ErrorCode::Div0));
            }
            let mean: f64 = nums.iter().sum::<f64>() / n as f64;
            let variance: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            Computed::Scalar(Value::Number(variance.sqrt()))
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=VAR.P(args...)` / `=VAR.P(args...)`
pub fn fn_var_p(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            let n = nums.len();
            if n == 0 {
                return Computed::Scalar(Value::erro(ErrorCode::Div0));
            }
            let mean: f64 = nums.iter().sum::<f64>() / n as f64;
            let variance: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
            Computed::Scalar(Value::Number(variance))
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=VAR.A(args...)` / `=VAR.S(args...)`
pub fn fn_var_s(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            let n = nums.len();
            if n <= 1 {
                return Computed::Scalar(Value::erro(ErrorCode::Div0));
            }
            let mean: f64 = nums.iter().sum::<f64>() / n as f64;
            let variance: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            Computed::Scalar(Value::Number(variance))
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=MODA.ÚNICA(args...)` / `=MODE.SNGL(args...)`
pub fn fn_mode_sngl(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            if nums.is_empty() {
                return Computed::Scalar(Value::erro(ErrorCode::Na));
            }
            let mut counts = std::collections::HashMap::new();
            for n in &nums {
                let key = n.to_bits();
                *counts.entry(key).or_insert(0usize) += 1;
            }
            let mut max_count = 0;
            let mut mode_val = None;
            for (key, count) in counts {
                if count > max_count {
                    max_count = count;
                    mode_val = Some(f64::from_bits(key));
                }
            }
            if max_count <= 1 {
                Computed::Scalar(Value::erro(ErrorCode::Na))
            } else if let Some(v) = mode_val {
                Computed::Scalar(Value::Number(v))
            } else {
                Computed::Scalar(Value::erro(ErrorCode::Na))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=PERCENTIL.INC(array; k)` / `=PERCENTILE.INC(array; k)`
pub fn fn_percentile_inc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let k = match ev.number(&args[1]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if k < 0.0 || k > 1.0 {
        return Computed::Scalar(Value::erro(ErrorCode::Num));
    }
    match collect(&args[0..1], ev) {
        Ok(mut nums) => {
            if nums.is_empty() {
                return Computed::Scalar(Value::erro(ErrorCode::Num));
            }
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = k * (nums.len() - 1) as f64;
            let i = idx.floor() as usize;
            let frac = idx - i as f64;
            if i + 1 < nums.len() {
                let val = nums[i] + frac * (nums[i + 1] - nums[i]);
                Computed::Scalar(Value::Number(val))
            } else {
                Computed::Scalar(Value::Number(nums[i]))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=QUARTIL.INC(array; quarto)` / `=QUARTILE.INC(array; quarto)`
pub fn fn_quartile_inc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let q = match ev.number(&args[1]) {
        Ok(v) => v.trunc() as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if q < 0 || q > 4 {
        return Computed::Scalar(Value::erro(ErrorCode::Num));
    }
    let k = q as f64 * 0.25;
    match collect(&args[0..1], ev) {
        Ok(mut nums) => {
            if nums.is_empty() {
                return Computed::Scalar(Value::erro(ErrorCode::Num));
            }
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = k * (nums.len() - 1) as f64;
            let i = idx.floor() as usize;
            let frac = idx - i as f64;
            if i + 1 < nums.len() {
                let val = nums[i] + frac * (nums[i + 1] - nums[i]);
                Computed::Scalar(Value::Number(val))
            } else {
                Computed::Scalar(Value::Number(nums[i]))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=MÉDIASE(faixa; criterio; [faixa_media])` / `=AVERAGEIF(...)`
pub fn fn_averageif(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let test_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };
    let crit_val = ev.eval(&args[1]);
    let pred = make_predicate(&crit_val);
    let avg_rect = if crate::funcoes::fn_helpers::has(args, 2) {
        match node_rect(&args[2], ev) {
            Some(r) => r,
            None => test_rect.clone(),
        }
    } else {
        test_rect.clone()
    };

    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&avg_rect), std::slice::from_ref(&test_rect), std::slice::from_ref(&crit_val)) {
        return if grupo.numeros == 0 {
            Computed::Scalar(Value::erro(ErrorCode::Div0))
        } else {
            Computed::Scalar(Value::Number(grupo.soma / grupo.numeros as f64))
        };
    }

    let mut sum = 0.0;
    let mut count = 0;
    for r in test_rect.r0..=test_rect.r1 {
        for c in test_rect.c0..=test_rect.c1 {
            let val = ev.ctx.get_value(r, c, test_rect.sheet.as_deref());
            if pred(&val) {
                let r_off = r - test_rect.r0;
                let c_off = c - test_rect.c0;
                let avg_r = avg_rect.r0 + r_off;
                let avg_c = avg_rect.c0 + c_off;
                let avg_val = ev.ctx.get_value(avg_r, avg_c, avg_rect.sheet.as_deref());
                if let Value::Number(n) = avg_val {
                    sum += n;
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        Computed::Scalar(Value::erro(ErrorCode::Div0))
    } else {
        Computed::Scalar(Value::Number(sum / count as f64))
    }
}

struct CriteriaPair {
    rect: Rect,
    pred: crate::funcoes::fn_helpers::Predicate,
    leitor: LeitorFaixa,
}

/// `=MEDIASES(media_faixa; criterio_faixa1; criterio1; ...)` / `=AVERAGEIFS(...)`
pub fn fn_averageifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 || args.len() % 2 == 0 {
        return err_value();
    }
    let avg_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let (rects, crits) = match crate::funcoes::fn_helpers::faixas_e_criterios(args, 1, ev) {
        Ok(x) => x,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&avg_rect), &rects, &crits) {
        return if grupo.numeros == 0 { Computed::Scalar(Value::erro(ErrorCode::Div0)) } else { Computed::Scalar(Value::Number(grupo.soma / grupo.numeros as f64)) };
    }
    let pairs: Vec<CriteriaPair> = rects.into_iter().zip(crits.iter())
        .map(|(rect, crit)| CriteriaPair { leitor: LeitorFaixa::novo(ev, &rect), rect, pred: make_predicate(crit) })
        .collect();

    let mut sum = 0.0;
    let mut count = 0;
    let alvo = LeitorFaixa::novo(ev, &avg_rect);
    for r in avg_rect.r0..=avg_rect.r1 {
        for c in avg_rect.c0..=avg_rect.c1 {
            let r_off = r - avg_rect.r0;
            let c_off = c - avg_rect.c0;
            let mut pass = true;
            for pair in &pairs {
                let pr = pair.rect.r0 + r_off;
                let pc = pair.rect.c0 + c_off;
                let val = pair.leitor.valor(ev, pr, pc);
                let val: &Value = &val;
                if !(pair.pred)(&val) {
                    pass = false;
                    break;
                }
            }
            if pass {
                let val = alvo.valor(ev, r, c);
                let val: &Value = &val;
                if let Value::Number(n) = val {
                    sum += n;
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        Computed::Scalar(Value::erro(ErrorCode::Div0))
    } else {
        Computed::Scalar(Value::Number(sum / count as f64))
    }
}

/// `=CONT.SES(criterio_faixa1; criterio1; ...)` / `=COUNTIFS(...)`
pub fn fn_countifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 || args.len() % 2 != 0 {
        return err_value();
    }
    let base_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let (rects, crits) = match crate::funcoes::fn_helpers::faixas_e_criterios(args, 0, ev) {
        Ok(x) => x,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, None, &rects, &crits) {
        return Computed::Scalar(Value::Number(grupo.linhas as f64));
    }
    let pairs: Vec<CriteriaPair> = rects.into_iter().zip(crits.iter())
        .map(|(rect, crit)| CriteriaPair { leitor: LeitorFaixa::novo(ev, &rect), rect, pred: make_predicate(crit) })
        .collect();

    let mut count = 0;
    for r in base_rect.r0..=base_rect.r1 {
        for c in base_rect.c0..=base_rect.c1 {
            let r_off = r - base_rect.r0;
            let c_off = c - base_rect.c0;
            let mut pass = true;
            for pair in &pairs {
                let pr = pair.rect.r0 + r_off;
                let pc = pair.rect.c0 + c_off;
                let val = pair.leitor.valor(ev, pr, pc);
                let val: &Value = &val;
                if !(pair.pred)(&val) {
                    pass = false;
                    break;
                }
            }
            if pass {
                count += 1;
            }
        }
    }

    Computed::Scalar(Value::Number(count as f64))
}

/// `=MÁXIMOSES(max_faixa; criterio_faixa1; criterio1; ...)` / `=MAXIFS(...)`
pub fn fn_maxifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 || args.len() % 2 == 0 {
        return err_value();
    }
    let max_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let (rects, crits) = match crate::funcoes::fn_helpers::faixas_e_criterios(args, 1, ev) {
        Ok(x) => x,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&max_rect), &rects, &crits) {
        return Computed::Scalar(Value::Number(if grupo.numeros == 0 { 0.0 } else { grupo.maximo }));
    }
    let pairs: Vec<CriteriaPair> = rects.into_iter().zip(crits.iter())
        .map(|(rect, crit)| CriteriaPair { leitor: LeitorFaixa::novo(ev, &rect), rect, pred: make_predicate(crit) })
        .collect();

    let mut max_val = f64::NEG_INFINITY;
    let mut count = 0;
    let alvo = LeitorFaixa::novo(ev, &max_rect);
    for r in max_rect.r0..=max_rect.r1 {
        for c in max_rect.c0..=max_rect.c1 {
            let r_off = r - max_rect.r0;
            let c_off = c - max_rect.c0;
            let mut pass = true;
            for pair in &pairs {
                let pr = pair.rect.r0 + r_off;
                let pc = pair.rect.c0 + c_off;
                let val = pair.leitor.valor(ev, pr, pc);
                let val: &Value = &val;
                if !(pair.pred)(&val) {
                    pass = false;
                    break;
                }
            }
            if pass {
                let val = alvo.valor(ev, r, c);
                let val: &Value = &val;
                if let Value::Number(n) = *val {
                    if n > max_val {
                        max_val = n;
                    }
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        Computed::Scalar(Value::Number(0.0))
    } else {
        Computed::Scalar(Value::Number(max_val))
    }
}

/// `=MÍNIMOSES(min_faixa; criterio_faixa1; criterio1; ...)` / `=MINIFS(...)`
pub fn fn_minifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 || args.len() % 2 == 0 {
        return err_value();
    }
    let min_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let (rects, crits) = match crate::funcoes::fn_helpers::faixas_e_criterios(args, 1, ev) {
        Ok(x) => x,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&min_rect), &rects, &crits) {
        return Computed::Scalar(Value::Number(if grupo.numeros == 0 { 0.0 } else { grupo.minimo }));
    }
    let pairs: Vec<CriteriaPair> = rects.into_iter().zip(crits.iter())
        .map(|(rect, crit)| CriteriaPair { leitor: LeitorFaixa::novo(ev, &rect), rect, pred: make_predicate(crit) })
        .collect();

    let mut min_val = f64::INFINITY;
    let mut count = 0;
    let alvo = LeitorFaixa::novo(ev, &min_rect);
    for r in min_rect.r0..=min_rect.r1 {
        for c in min_rect.c0..=min_rect.c1 {
            let r_off = r - min_rect.r0;
            let c_off = c - min_rect.c0;
            let mut pass = true;
            for pair in &pairs {
                let pr = pair.rect.r0 + r_off;
                let pc = pair.rect.c0 + c_off;
                let val = pair.leitor.valor(ev, pr, pc);
                let val: &Value = &val;
                if !(pair.pred)(&val) {
                    pass = false;
                    break;
                }
            }
            if pass {
                let val = alvo.valor(ev, r, c);
                let val: &Value = &val;
                if let Value::Number(n) = *val {
                    if n < min_val {
                        min_val = n;
                    }
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        Computed::Scalar(Value::Number(0.0))
    } else {
        Computed::Scalar(Value::Number(min_val))
    }
}

/// `=CONTAR.VAZIO(faixa)` / `=COUNTBLANK(...)`
pub fn fn_countblank(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    let mut count = 0;
    for r in rect.r0..=rect.r1 {
        for c in rect.c0..=rect.c1 {
            let val = ev.ctx.get_value(r, c, rect.sheet.as_deref());
            if matches!(val, Value::Empty) || val == Value::Text(String::new()) {
                count += 1;
            }
        }
    }

    Computed::Scalar(Value::Number(count as f64))
}

/// `=CONT.ÚNICO(args...)` / `=COUNTUNIQUE(args...)`
pub fn fn_countunique(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    use std::collections::HashSet;
    use crate::celulas::values::to_text;
    let mut seen = HashSet::new();
    for a in args {
        let mut err = None;
        ev.for_each_value(a, &mut |v, _| {
            if err.is_some() { return; }
            if let Value::Error(e) = v {
                err = Some(e.clone());
                return;
            }
            if !matches!(v, Value::Empty) {
                let txt = to_text(v);
                if !txt.is_empty() {
                    seen.insert(txt);
                }
            }
        });
        if let Some(e) = err {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Number(seen.len() as f64))
}

/// `=MEDIANA(args...)` / `=MEDIAN(args...)`
pub fn fn_median(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let mut ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    match mediana_numerica(&mut ns) {
        Ok(n) => Computed::Scalar(Value::Number(n)),
        Err(e) => Computed::Scalar(Value::erro(e)),
    }
}

/// `=MAIOR(matriz; k)` / `=LARGE(array; k)`
pub fn fn_large(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let mut ns = match collect(&[args[0].clone()], ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let k = match ev.number(&args[1]) {
        Ok(n) => n as usize,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if k < 1 || k > ns.len() {
        return err_num();
    }
    ns.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Computed::Scalar(Value::Number(ns[k - 1]))
}

/// `=MENOR(matriz; k)` / `=SMALL(array; k)`
pub fn fn_small(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let mut ns = match collect(&[args[0].clone()], ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let k = match ev.number(&args[1]) {
        Ok(n) => n as usize,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if k < 1 || k > ns.len() {
        return err_num();
    }
    ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Computed::Scalar(Value::Number(ns[k - 1]))
}

/// `=ORDEM(num; ref; [ordem])` / `=RANK(number; ref; [order])`
pub fn fn_rank(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = match ev.number(&args[0]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let ns = match collect(&[args[1].clone()], ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let ascending = if has(args, 2) {
        match ev.boolean(&args[2]) {
            Ok(b) => b,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        false
    };

    if !ns.iter().any(|&n| (n - val).abs() < 1e-12) {
        return err_na();
    }
    let mut pos = 1;
    for &n in &ns {
        if if ascending { n < val } else { n > val } {
            pos += 1;
        }
    }
    Computed::Scalar(Value::Number(pos as f64))
}

fn pair_values(args: &[Node], ev: &Evaluator<'_>) -> Result<(Vec<f64>, Vec<f64>), CellError> {
    if let Some(err) = arity(args, 2, 2) {
        return Err(err);
    }
    let xs = collect(&[args[0].clone()], ev)?;
    let ys = collect(&[args[1].clone()], ev)?;
    if xs.len() != ys.len() || xs.is_empty() {
        return Err(CellError::com_motivo(crate::celulas::values::ErrorCode::Value, "Arrays must have same length"));
    }
    Ok((xs, ys))
}

/// `=CORREL(array1; array2)`
pub fn fn_correl(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let (xs, ys) = match pair_values(args, ev) {
        Ok(pair) => pair,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        den_x += dx * dx;
        den_y += dy * dy;
    }
    let den = (den_x * den_y).sqrt();
    if den == 0.0 {
        return err_num();
    }
    Computed::Scalar(Value::Number(num / den))
}

/// `=COVAR(array1; array2)`
pub fn fn_covar(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let (xs, ys) = match pair_values(args, ev) {
        Ok(pair) => pair,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut sum = 0.0;
    for i in 0..xs.len() {
        sum += (xs[i] - mean_x) * (ys[i] - mean_y);
    }
    Computed::Scalar(Value::Number(sum / n))
}

/// `=INCLINAÇÃO(val_y; val_x)` / `=SLOPE(known_y's; known_x's)`
pub fn fn_slope(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let (ys, xs) = match pair_values(args, ev) {
        Ok(pair) => pair,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        num += dx * (ys[i] - mean_y);
        den += dx * dx;
    }
    if den == 0.0 {
        return err_num();
    }
    Computed::Scalar(Value::Number(num / den))
}

/// `=INTERCEPÇÃO(val_y; val_x)` / `=INTERCEPT(known_y's; known_x's)`
pub fn fn_intercept(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let (ys, xs) = match pair_values(args, ev) {
        Ok(pair) => pair,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        num += dx * (ys[i] - mean_y);
        den += dx * dx;
    }
    if den == 0.0 {
        return err_num();
    }
    let slope = num / den;
    Computed::Scalar(Value::Number(mean_y - slope * mean_x))
}

/// `=RQUAD(val_y; val_x)` / `=RSQ(known_y's; known_x's)`
pub fn fn_rsq(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match fn_correl(args, ev) {
        Computed::Scalar(Value::Number(r)) => Computed::Scalar(Value::Number(r * r)),
        other => other,
    }
}

/// `=PREVISÃO(x; val_y; val_x)` / `=FORECAST(...)`
pub fn fn_forecast(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let x = match ev.number(&args[0]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let (ys, xs) = match pair_values(&[args[1].clone(), args[2].clone()], ev) {
        Ok(pair) => pair,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        num += dx * (ys[i] - mean_y);
        den += dx * dx;
    }
    if den == 0.0 {
        return err_num();
    }
    let slope = num / den;
    let intercept = mean_y - slope * mean_x;
    Computed::Scalar(Value::Number(intercept + slope * x))
}

/// `=MÉDIA.GEOMÉTRICA(args...)` / `=GEOMEAN(args...)`
pub fn fn_geomean(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if ns.is_empty() || ns.iter().any(|&n| n <= 0.0) {
        return err_num();
    }
    let sum: f64 = ns.iter().map(|n| n.ln()).sum();
    Computed::Scalar(Value::Number((sum / ns.len() as f64).exp()))
}

/// `=MÉDIA.HARMÔNICA(args...)` / `=HARMEAN(args...)`
pub fn fn_harmean(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if ns.is_empty() || ns.iter().any(|&n| n <= 0.0) {
        return err_num();
    }
    let sum: f64 = ns.iter().map(|n| 1.0 / n).sum();
    Computed::Scalar(Value::Number(ns.len() as f64 / sum))
}

/// `=DESV.MÉDIO(args...)` / `=AVEDEV(args...)`
pub fn fn_avedev(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if ns.is_empty() {
        return err_num();
    }
    let mean = ns.iter().sum::<f64>() / ns.len() as f64;
    let sum: f64 = ns.iter().map(|n| (n - mean).abs()).sum();
    Computed::Scalar(Value::Number(sum / ns.len() as f64))
}

/// `=DESVQ(args...)` / `=DEVSQ(args...)`
pub fn fn_devsq(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if ns.is_empty() {
        return err_num();
    }
    let mean = ns.iter().sum::<f64>() / ns.len() as f64;
    let sum: f64 = ns.iter().map(|n| (n - mean) * (n - mean)).sum();
    Computed::Scalar(Value::Number(sum))
}

/// `=DISTORÇÃO(args...)` / `=SKEW(args...)`
pub fn fn_skew(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = ns.len() as f64;
    if n < 3.0 {
        return err_num();
    }
    let mean = ns.iter().sum::<f64>() / n;
    let var = ns.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let sd = var.sqrt();
    if sd == 0.0 {
        return err_num();
    }
    let sum: f64 = ns.iter().map(|x| ((x - mean) / sd).powi(3)).sum();
    Computed::Scalar(Value::Number((n / ((n - 1.0) * (n - 2.0))) * sum))
}

/// `=CURT(args...)` / `=KURT(args...)`
pub fn fn_kurt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let ns = match collect(args, ev) {
        Ok(ns) => ns,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let n = ns.len() as f64;
    if n < 4.0 {
        return err_num();
    }
    let mean = ns.iter().sum::<f64>() / n;
    let var = ns.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let sd = var.sqrt();
    if sd == 0.0 {
        return err_num();
    }
    let sum: f64 = ns.iter().map(|x| ((x - mean) / sd).powi(4)).sum();
    let term1 = ((n * (n + 1.0)) / ((n - 1.0) * (n - 2.0) * (n - 3.0))) * sum;
    let term2 = (3.0 * (n - 1.0).powi(2)) / ((n - 2.0) * (n - 3.0));
    Computed::Scalar(Value::Number(term1 - term2))
}

fn erf(x: f64) -> f64 {
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let abs_x = x.abs();
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    let t = 1.0 / (1.0 + p * abs_x);
    let y = 1.0 - ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t * (-abs_x * abs_x).exp();
    sign * y
}

fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn norm_inv(p: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::NAN;
    }
    let a = [-39.69683028665376, 220.9460984245205, -275.9285104469687, 138.357751867269, -30.66479806614716, 2.50662827459239];
    let b = [-54.47609879822406, 161.5858368580409, -155.6989798598866, 66.80131188771972, -13.28068155288572];
    let c = [-0.007784894002430293, -0.3223964580411365, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    let d = [0.007784695709041462, 0.3224671290700398, 2.445134137142996, 3.754408661907416];
    let p_low = 0.02425;
    let p_high = 1.0 - p_low;
    if p < p_low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / (((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0))
    } else if p <= p_high {
        let q = p - 0.5;
        let r = q * q;
        ((((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q)
            / ((((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0))
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -((((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / (((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)))
    }
}

fn log_gamma(x: f64) -> f64 {
    let coeff = [
        57.1562356658629235, -59.5979603554754912, 14.1360979747417471, -0.491913816097620199,
        0.339946499848118887e-4, 0.465236289270485756e-4, -0.983744753048795646e-4, 0.158084201380350036e-3,
        -0.210264441724104883e-3, 0.217439618115212643e-3, -0.16431810653676389e-3, 0.844182239838527433e-4,
        -0.261908384015814087e-4, 0.368991826595316227e-5,
    ];
    let mut y = x;
    let mut tmp = x + 5.2421875;
    tmp = (x + 0.5) * tmp.ln() - tmp;
    let mut ser = 0.99999999999999709182;
    for c in &coeff {
        y += 1.0;
        ser += c / y;
    }
    tmp + ((2.5066282746310005 * ser) / x).ln()
}

fn gamma_lower(a: f64, x: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    if x < a + 1.0 {
        let mut ap = a;
        let mut del = 1.0 / a;
        let mut sum = 1.0 / a;
        for _ in 1..100 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-14 { break; }
        }
        sum * (-x + a * x.ln() - log_gamma(a)).exp()
    } else {
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / 1e-30;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..100 {
            let an = -i as f64 * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < 1e-30 { d = 1e-30; }
            c = b + an / c;
            if c.abs() < 1e-30 { c = 1e-30; }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-14 { break; }
        }
        1.0 - h * (-x + a * x.ln() - log_gamma(a)).exp()
    }
}

fn chisq_cdf(x: f64, df: f64) -> f64 {
    gamma_lower(df / 2.0, x / 2.0)
}

fn chisq_pdf(x: f64, df: f64) -> f64 {
    let k = df / 2.0;
    (x.powf(k - 1.0) * (-x / 2.0).exp()) / (2.0f64.powf(k) * (log_gamma(k)).exp())
}

/// A acumulada da t de Student pela beta incompleta — tradução do `tCdf`
/// do `fn-stats.ts`. A versão anterior era uma aproximação pela normal com
/// um fator inventado, que errava na segunda casa: `DIST.T(1; 10;
/// VERDADEIRO)` dava 0,8113 contra 0,8296 (o do Excel e o do TypeScript).
fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let p = 0.5 * beta_incomplete(x, df / 2.0, 0.5);
    if t >= 0.0 { 1.0 - p } else { p }
}

/// Beta incompleta regularizada, por fração contínua — `betaIncomplete` do
/// `fn-stats.ts`, linha a linha (mesmos limites, mesmo 1e-30, mesmo 1e-14).
fn beta_incomplete(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    if x > (a + 1.0) / (a + b + 2.0) {
        return 1.0 - beta_incomplete(1.0 - x, b, a);
    }
    let lbeta = log_gamma(a) + log_gamma(b) - log_gamma(a + b);
    let front = (a * x.ln() + b * (1.0 - x).ln() - lbeta).exp() / a;
    let piso = |v: f64| if v.abs() < 1e-30 { 1e-30 } else { v };
    let mut c = 1.0;
    let mut d = 1.0 / piso(1.0 - (a + b) * x / (a + 1.0));
    let mut h = d;
    for m in 1..=100 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((a + m2 - 1.0) * (a + m2));
        d = 1.0 / piso(1.0 + aa * d);
        c = piso(1.0 + aa / c);
        h *= d * c;
        let aa = -(a + m) * (a + b + m) * x / ((a + m2) * (a + m2 + 1.0));
        d = 1.0 / piso(1.0 + aa * d);
        c = piso(1.0 + aa / c);
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-14 {
            break;
        }
    }
    front * h
}

fn t_pdf(x: f64, df: f64) -> f64 {
    let k = (df + 1.0) / 2.0;
    (log_gamma(k) - log_gamma(df / 2.0)).exp() / ((df * std::f64::consts::PI).sqrt() * (1.0 + x * x / df).powf(k))
}

/// `=DIST.NORM(x; media; desv_pad; acumulado)` / `=NORM.DIST(...)`
pub fn fn_norm_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 4, 4) { return Computed::Scalar(Value::Error(err)); }
    let x = match ev.number(&args[0]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let mean = match ev.number(&args[1]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let sd = match ev.number(&args[2]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    if sd <= 0.0 { return err_num(); }
    let cumulative = match ev.boolean(&args[3]) { Ok(b) => b, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let z = (x - mean) / sd;
    Computed::Scalar(Value::Number(if cumulative { norm_cdf(z) } else { norm_pdf(z) / sd }))
}

/// `=DIST.NORMP(z; acumulado)` / `=NORM.S.DIST(...)`
pub fn fn_norm_s_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let z = match ev.number(&args[0]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let cumulative = match ev.boolean(&args[1]) { Ok(b) => b, Err(e) => return Computed::Scalar(Value::Error(e)) };
    Computed::Scalar(Value::Number(if cumulative { norm_cdf(z) } else { norm_pdf(z) }))
}

/// `=INV.NORM(prob; media; desv_pad)` / `=NORM.INV(...)`
pub fn fn_norm_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) { return Computed::Scalar(Value::Error(err)); }
    let p = match ev.number(&args[0]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let mean = match ev.number(&args[1]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let sd = match ev.number(&args[2]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    if p <= 0.0 || p >= 1.0 || sd <= 0.0 { return err_num(); }
    let z = norm_inv(p);
    if z.is_nan() { err_num() } else { Computed::Scalar(Value::Number(mean + z * sd)) }
}

/// `=DIST.T(x; grau_lib; acumulado)` / `=T.DIST(...)`
pub fn fn_t_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) { return Computed::Scalar(Value::Error(err)); }
    let x = match ev.number(&args[0]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let df = match ev.number(&args[1]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    if df < 1.0 { return err_num(); }
    let cumulative = match ev.boolean(&args[2]) { Ok(b) => b, Err(e) => return Computed::Scalar(Value::Error(e)) };
    Computed::Scalar(Value::Number(if cumulative { t_cdf(x, df) } else { t_pdf(x, df) }))
}

/// `=TESTE.T(matriz1; matriz2; caudas; tipo)` / `=T.TEST(...)`
pub fn fn_t_test(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 4, 4) { return Computed::Scalar(Value::Error(err)); }
    let arr1 = match collect(&[args[0].clone()], ev) { Ok(a) => a, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let arr2 = match collect(&[args[1].clone()], ev) { Ok(a) => a, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let tails = match ev.number(&args[2]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let test_type = match ev.number(&args[3]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    if (tails != 1.0 && tails != 2.0) || (test_type != 1.0 && test_type != 2.0 && test_type != 3.0) {
        return err_num();
    }
    if arr1.is_empty() || arr2.is_empty() { return err_num(); }

    let (t_stat, df) = if test_type == 1.0 {
        if arr1.len() != arr2.len() || arr1.len() < 2 { return err_num(); }
        let diffs: Vec<f64> = arr1.iter().zip(&arr2).map(|(a, b)| a - b).collect();
        let n = diffs.len() as f64;
        let mean_d = diffs.iter().sum::<f64>() / n;
        let var_d = diffs.iter().map(|d| (d - mean_d).powi(2)).sum::<f64>() / (n - 1.0);
        if var_d == 0.0 { return Computed::Scalar(Value::Number(1.0)); }
        (mean_d / (var_d / n).sqrt(), n - 1.0)
    } else if test_type == 2.0 {
        let n1 = arr1.len() as f64;
        let n2 = arr2.len() as f64;
        if n1 < 2.0 || n2 < 2.0 { return err_num(); }
        let m1 = arr1.iter().sum::<f64>() / n1;
        let m2 = arr2.iter().sum::<f64>() / n2;
        let v1 = arr1.iter().map(|x| (x - m1).powi(2)).sum::<f64>() / (n1 - 1.0);
        let v2 = arr2.iter().map(|x| (x - m2).powi(2)).sum::<f64>() / (n2 - 1.0);
        let sp2 = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
        if sp2 == 0.0 { return Computed::Scalar(Value::Number(1.0)); }
        ((m1 - m2) / (sp2 * (1.0 / n1 + 1.0 / n2)).sqrt(), n1 + n2 - 2.0)
    } else {
        let n1 = arr1.len() as f64;
        let n2 = arr2.len() as f64;
        if n1 < 2.0 || n2 < 2.0 { return err_num(); }
        let m1 = arr1.iter().sum::<f64>() / n1;
        let m2 = arr2.iter().sum::<f64>() / n2;
        let v1 = arr1.iter().map(|x| (x - m1).powi(2)).sum::<f64>() / (n1 - 1.0);
        let v2 = arr2.iter().map(|x| (x - m2).powi(2)).sum::<f64>() / (n2 - 1.0);
        let se1 = v1 / n1;
        let se2 = v2 / n2;
        if se1 + se2 == 0.0 { return Computed::Scalar(Value::Number(1.0)); }
        let df_num = (se1 + se2).powi(2);
        let df_den = se1.powi(2) / (n1 - 1.0) + se2.powi(2) / (n2 - 1.0);
        ((m1 - m2) / (se1 + se2).sqrt(), df_num / df_den)
    };

    let p1 = 1.0 - t_cdf(t_stat.abs(), df);
    let p = if tails == 1.0 { p1 } else { 2.0 * p1 };
    Computed::Scalar(Value::Number(p))
}

/// `=DIST.QUI(x; grau_lib; acumulado)` / `=CHISQ.DIST(...)`
pub fn fn_chisq_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) { return Computed::Scalar(Value::Error(err)); }
    let x = match ev.number(&args[0]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let df = match ev.number(&args[1]) { Ok(n) => n, Err(e) => return Computed::Scalar(Value::Error(e)) };
    if x < 0.0 || df < 1.0 { return err_num(); }
    let cumulative = match ev.boolean(&args[2]) { Ok(b) => b, Err(e) => return Computed::Scalar(Value::Error(e)) };
    Computed::Scalar(Value::Number(if cumulative { chisq_cdf(x, df) } else { chisq_pdf(x, df) }))
}

/// `=TESTE.QUI(matriz_real; matriz_esperada)` / `=CHISQ.TEST(...)`
pub fn fn_chisq_test(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let ma = match ev.matrix_arg(&args[0]) { Ok(m) => m, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let me = match ev.matrix_arg(&args[1]) { Ok(m) => m, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let (ra, rb) = (ma.rows(), me.rows());
    let linhas = ra.len();
    let colunas = ra.first().map_or(0, |l| l.len());
    if linhas == 0 || colunas == 0 || linhas != rb.len() || rb.iter().any(|l| l.len() != colunas) { return Computed::Scalar(Value::Error(CellError::new(ErrorCode::Na))); }
    let mut chi2 = 0.0;
    for (la, le) in ra.iter().zip(rb.iter()) {
        for (va, ve) in la.iter().zip(le.iter()) {
            let (o, exp) = match (va, ve) {
                (Value::Number(o), Value::Number(e)) => (*o, *e),
                _ => continue, // texto, vazio e lógico são ignorados, como no Excel
            };
            if exp == 0.0 { return Computed::Scalar(Value::Error(CellError::new(ErrorCode::Div0))); }
            chi2 += (o - exp).powi(2) / exp;
        }
    }
    // Graus de liberdade: (l-1)(c-1); com uma só dimensão, n-1.
    let df = if linhas > 1 && colunas > 1 { ((linhas - 1) * (colunas - 1)) as f64 } else { (linhas * colunas) as f64 - 1.0 };
    if df < 1.0 { return Computed::Scalar(Value::Error(CellError::new(ErrorCode::Na))); }
    Computed::Scalar(Value::Number((1.0 - chisq_cdf(chi2, df)).max(0.0)))
}

/// `=FREQUÊNCIA(matriz_dados; matriz_intervalos)` / `=FREQUENCY(...)`
pub fn fn_frequency(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let data = match collect(&[args[0].clone()], ev) { Ok(a) => a, Err(e) => return Computed::Scalar(Value::Error(e)) };
    let mut bins = match collect(&[args[1].clone()], ev) { Ok(a) => a, Err(e) => return Computed::Scalar(Value::Error(e)) };

    if bins.is_empty() {
        return Computed::Matrix(crate::celulas::values::Matrix::new(vec![vec![Value::Number(data.len() as f64)]]));
    }
    bins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut counts = vec![0.0; bins.len() + 1];
    for &d in &data {
        let mut idx = bins.len();
        for (i, &b) in bins.iter().enumerate() {
            if d <= b {
                idx = i;
                break;
            }
        }
        counts[idx] += 1.0;
    }
    let matrix_rows: Vec<Vec<Value>> = counts.into_iter().map(|c| vec![Value::Number(c)]).collect();
    Computed::Matrix(crate::celulas::values::Matrix::new(matrix_rows))
}

// ---------------------------------------------------------------------------
// Reescrita limpa (16/10): funções com "A", posto, percentis exclusivos,
// covariância, correlação, aparadas, assimetria, modas e Fisher.
// ---------------------------------------------------------------------------

/// Valores das funções com A no fim (MÉDIAA, MÁXIMOA…): dentro de intervalo o
/// texto vale 0 e o lógico 1/0; vazio é ignorado; escrito solto, texto
/// numérico conta e texto comum dá `#VALOR!`.
fn valores_a(ev: &Evaluator<'_>, args: &[Node]) -> Res<Vec<f64>> {
    let mut saida = Vec::new();
    for a in args {
        let ref_unica = matches!(a, Node::Ref { .. });
        for (v, de_colecao) in valores_de(ev, a)? {
            let de_colecao = de_colecao || ref_unica;
            match v {
                Value::Number(n) => saida.push(n),
                Value::Bool(b) => saida.push(if b { 1.0 } else { 0.0 }),
                Value::Text(_) if de_colecao => saida.push(0.0),
                Value::Text(ref t) => {
                    if t.trim().is_empty() {
                        return Err(e_valor());
                    }
                    saida.push(to_number(&v).map_err(|_| e_valor())?);
                }
                Value::Error(e) => return Err(e),
                Value::Empty => {}
            }
        }
    }
    Ok(saida)
}

fn escalar_de(r: Res<f64>) -> Computed {
    concluir(r.and_then(numero))
}

/// Soma dos quadrados dos desvios em relação à média, e a média.
fn desvios(v: &[f64]) -> (f64, f64) {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (m, v.iter().map(|x| (x - m) * (x - m)).sum())
}

fn variancia_de(v: &[f64], amostra: bool) -> Res<f64> {
    let n = v.len();
    if n < if amostra { 2 } else { 1 } {
        return Err(e_div0());
    }
    let (_, ss) = desvios(v);
    Ok(ss / if amostra { n - 1 } else { n } as f64)
}

/// `=MÉDIAA(valor1; ...)` / `=AVERAGEA(value1; ...)` — média contando texto como 0 e lógicos como 0/1.
pub fn fn_averagea(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let v = valores_a(ev, args)?;
        if v.is_empty() {
            return Err(e_div0());
        }
        Ok(v.iter().sum::<f64>() / v.len() as f64)
    })())
}

/// `=MÁXIMOA(valor1; ...)` / `=MAXA(value1; ...)` — maior valor contando texto como 0 e lógicos como 0/1.
pub fn fn_maxa(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        Ok(valores_a(ev, args)?.into_iter().reduce(f64::max).unwrap_or(0.0))
    })())
}

/// `=MÍNIMOA(valor1; ...)` / `=MINA(value1; ...)` — menor valor contando texto como 0 e lógicos como 0/1.
pub fn fn_mina(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        Ok(valores_a(ev, args)?.into_iter().reduce(f64::min).unwrap_or(0.0))
    })())
}

fn variancia_a(args: &[Node], ev: &Evaluator<'_>, amostra: bool, raiz: bool) -> Computed {
    escalar_de((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let v = variancia_de(&valores_a(ev, args)?, amostra)?;
        Ok(if raiz { v.sqrt() } else { v })
    })())
}

/// `=DESVPADA(valor1; ...)` / `=STDEVA(value1; ...)` — desvio padrão da amostra com texto e lógicos.
pub fn fn_stdeva(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    variancia_a(args, ev, true, true)
}

/// `=DESVPADPA(valor1; ...)` / `=STDEVPA(value1; ...)` — desvio padrão da população com texto e lógicos.
pub fn fn_stdevpa(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    variancia_a(args, ev, false, true)
}

/// `=VARA(valor1; ...)` / `=VARA(value1; ...)` — variância da amostra com texto e lógicos.
pub fn fn_vara(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    variancia_a(args, ev, true, false)
}

/// `=VARPA(valor1; ...)` / `=VARPA(value1; ...)` — variância da população com texto e lógicos.
pub fn fn_varpa(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    variancia_a(args, ev, false, false)
}

/// Números de uma matriz/intervalo, ordenados do menor para o maior.
fn numeros_ordenados(ev: &Evaluator<'_>, node: &Node) -> Res<Vec<f64>> {
    let mut v = numeros_de(ev, node)?;
    v.sort_by(|a, b| a.total_cmp(b));
    Ok(v)
}

fn posto(args: &[Node], ev: &Evaluator<'_>, medio: bool) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 3) {
            return Err(e);
        }
        let x = ev.number(&args[0])?;
        let v = numeros_de(ev, &args[1])?;
        let crescente = if has(args, 2) { ev.number(&args[2])? != 0.0 } else { false };
        let melhores = v.iter().filter(|&&n| if crescente { n < x } else { n > x }).count();
        let iguais = v.iter().filter(|&&n| n == x).count();
        if iguais == 0 {
            return Err(e_nd());
        }
        let base = (melhores + 1) as f64;
        Ok(if medio { base + (iguais - 1) as f64 / 2.0 } else { base })
    })())
}

/// `=ORDEM.EQ(núm; ref; [ordem])` / `=RANK.EQ(number; ref; [order])` — posição; empates ficam com a melhor.
pub fn fn_rank_eq(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    posto(args, ev, false)
}

/// `=ORDEM.MED(núm; ref; [ordem])` / `=RANK.AVG(number; ref; [order])` — posição; empates ficam com a média.
pub fn fn_rank_avg(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    posto(args, ev, true)
}

/// Percentil exclusivo `k` (entre 1/(n+1) e n/(n+1)) de uma lista ordenada.
fn percentil_exclusivo(v: &[f64], k: f64) -> Res<f64> {
    let n = v.len();
    if n == 0 {
        return Err(e_num());
    }
    let pos = k * (n as f64 + 1.0);
    if pos < 1.0 || pos > n as f64 {
        return Err(e_num());
    }
    let i = pos.floor() as usize;
    let frac = pos - i as f64;
    Ok(if i < n { v[i - 1] + frac * (v[i] - v[i - 1]) } else { v[n - 1] })
}

/// `=PERCENTIL.EXC(matriz; k)` / `=PERCENTILE.EXC(array; k)` — percentil exclusivo.
pub fn fn_percentile_exc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let v = numeros_ordenados(ev, &args[0])?;
        let k = ev.number(&args[1])?;
        percentil_exclusivo(&v, k)
    })())
}

/// `=QUARTIL.EXC(matriz; quarto)` / `=QUARTILE.EXC(array; quart)` — quartil exclusivo (1 a 3).
pub fn fn_quartile_exc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let v = numeros_ordenados(ev, &args[0])?;
        let q = ev.number(&args[1])?.trunc();
        if !(1.0..=3.0).contains(&q) {
            return Err(e_num());
        }
        percentil_exclusivo(&v, q / 4.0)
    })())
}

fn posto_percentual(args: &[Node], ev: &Evaluator<'_>, exclusivo: bool) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 3) {
            return Err(e);
        }
        let v = numeros_ordenados(ev, &args[0])?;
        let x = ev.number(&args[1])?;
        let sig = if has(args, 2) { ev.number(&args[2])?.trunc() } else { 3.0 };
        if sig < 1.0 {
            return Err(e_num());
        }
        let n = v.len();
        if n == 0 {
            return Err(e_num());
        }
        if x < v[0] || x > v[n - 1] {
            return Err(e_nd());
        }
        let antes = v.partition_point(|&a| a < x);
        let pos = if antes < n && v[antes] == x {
            antes as f64
        } else {
            // Interpola entre a posição do vizinho de baixo (sua primeira
            // ocorrência) e a do vizinho de cima.
            let (lo, hi) = (antes - 1, antes);
            let primeiro = v.partition_point(|&a| a < v[lo]) as f64;
            primeiro + (x - v[lo]) / (v[hi] - v[lo]) * (hi as f64 - primeiro)
        };
        let r = if exclusivo {
            (pos + 1.0) / (n as f64 + 1.0)
        } else if n == 1 {
            1.0
        } else {
            pos / (n as f64 - 1.0)
        };
        // O Excel corta (não arredonda) na significância pedida.
        if sig > 15.0 {
            return Ok(r);
        }
        let t = 10f64.powf(sig);
        Ok((r * t + 1e-9).floor() / t)
    })())
}

/// `=ORDEM.PORCENTUAL.INC(matriz; x; [significância])` / `=PERCENTRANK.INC(array; x; [significance])`
/// — posição relativa de x, de 0 a 1, cortada (não arredondada) na significância.
pub fn fn_percentrank_inc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    posto_percentual(args, ev, false)
}

/// `=ORDEM.PORCENTUAL.EXC(matriz; x; [significância])` / `=PERCENTRANK.EXC(array; x; [significance])`
/// — posição relativa exclusiva de x, entre 0 e 1.
pub fn fn_percentrank_exc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    posto_percentual(args, ev, true)
}

fn covariancia(args: &[Node], ev: &Evaluator<'_>, amostra: bool) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let p = pares_numericos(ev, &args[0], &args[1])?;
        let n = p.len();
        if n < if amostra { 2 } else { 1 } {
            return Err(e_div0());
        }
        let mx = p.iter().map(|q| q.0).sum::<f64>() / n as f64;
        let my = p.iter().map(|q| q.1).sum::<f64>() / n as f64;
        let s: f64 = p.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum();
        Ok(s / if amostra { n - 1 } else { n } as f64)
    })())
}

/// `=COVARIAÇÃO.P(matriz1; matriz2)` / `=COVARIANCE.P(array1; array2)` — covariância da população.
pub fn fn_covariance_p(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    covariancia(args, ev, false)
}

/// `=COVARIAÇÃO.S(matriz1; matriz2)` / `=COVARIANCE.S(array1; array2)` — covariância da amostra.
pub fn fn_covariance_s(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    covariancia(args, ev, true)
}

/// `=PEARSON(matriz1; matriz2)` — coeficiente de correlação de Pearson.
pub fn fn_pearson(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let p = pares_numericos(ev, &args[0], &args[1])?;
        let n = p.len();
        if n < 2 {
            return Err(e_div0());
        }
        let mx = p.iter().map(|q| q.0).sum::<f64>() / n as f64;
        let my = p.iter().map(|q| q.1).sum::<f64>() / n as f64;
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for (x, y) in &p {
            sxy += (x - mx) * (y - my);
            sxx += (x - mx) * (x - mx);
            syy += (y - my) * (y - my);
        }
        let d = (sxx * syy).sqrt();
        if d == 0.0 {
            return Err(e_div0());
        }
        Ok(sxy / d)
    })())
}

/// `=PADRONIZAR(x; média; desv_padrão)` / `=STANDARDIZE(x; mean; standard_dev)` — valor normalizado.
pub fn fn_standardize(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let (x, m, d) = (e.num(0)?, e.num(1)?, e.num(2)?);
        if d <= 0.0 {
            return Err(e_num());
        }
        numero((x - m) / d)
    })
}

/// `=MÉDIA.INTERNA(matriz; porcentagem)` / `=TRIMMEAN(array; percent)` — média sem os extremos.
pub fn fn_trimmean(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let p = ev.number(&args[1])?;
        let v = numeros_ordenados(ev, &args[0])?;
        if !(0.0..1.0).contains(&p) || v.is_empty() {
            return Err(e_num());
        }
        let n = v.len();
        let por_lado = ((n as f64 * p).floor() as usize) / 2;
        let meio = &v[por_lado..n - por_lado];
        Ok(meio.iter().sum::<f64>() / meio.len() as f64)
    })())
}

/// `=PREVISÃO.LINEAR(x; val_y; val_x)` / `=FORECAST.LINEAR(x; known_y's; known_x's)` — valor da reta de regressão em x.
pub fn fn_forecast_linear(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if let Some(e) = arity(args, 3, 3) {
            return Err(e);
        }
        let x = ev.number(&args[0])?;
        let p = pares_numericos(ev, &args[1], &args[2])?;
        let n = p.len();
        if n == 0 {
            return Err(e_div0());
        }
        let my = p.iter().map(|q| q.0).sum::<f64>() / n as f64;
        let mx = p.iter().map(|q| q.1).sum::<f64>() / n as f64;
        let (mut sxy, mut sxx) = (0.0, 0.0);
        for (y, xi) in &p {
            sxy += (xi - mx) * (y - my);
            sxx += (xi - mx) * (xi - mx);
        }
        if sxx == 0.0 {
            return Err(e_div0());
        }
        let b = sxy / sxx;
        Ok(my + b * (x - mx))
    })())
}

/// `=DISTORÇÃO.P(núm1; ...)` / `=SKEW.P(number1; ...)` — assimetria da população.
pub fn fn_skew_p(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar_de((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let v = numeros_de_todos(ev, args)?;
        if v.is_empty() {
            return Err(e_div0());
        }
        let n = v.len() as f64;
        let (m, ss) = desvios(&v);
        let dp = (ss / n).sqrt();
        if dp == 0.0 {
            return Err(e_div0());
        }
        Ok(v.iter().map(|x| ((x - m) / dp).powi(3)).sum::<f64>() / n)
    })())
}

/// `=MODO.MULT(núm1; ...)` / `=MODE.MULT(number1; ...)` — coluna com todos os valores mais frequentes.
pub fn fn_mode_mult(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir_matriz((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let v = numeros_de_todos(ev, args)?;
        let mut contagem: Vec<(f64, usize)> = Vec::new();
        let mut indice: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for n in v {
            let chave = (n + 0.0).to_bits();
            match indice.get(&chave) {
                Some(&i) => contagem[i].1 += 1,
                None => {
                    indice.insert(chave, contagem.len());
                    contagem.push((n, 1));
                }
            }
        }
        let maior = contagem.iter().map(|c| c.1).max().unwrap_or(0);
        if maior < 2 {
            return Err(e_nd());
        }
        Ok(contagem.iter().filter(|c| c.1 == maior).map(|c| vec![Value::Number(c.0)]).collect())
    })())
}

/// `=FISHER(x)` — transformação de Fisher, `x` entre -1 e 1 (exclusive).
pub fn fn_fisher(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let x = e.num(0)?;
        if x.abs() >= 1.0 {
            return Err(e_num());
        }
        numero(0.5 * ((1.0 + x) / (1.0 - x)).ln())
    })
}

/// `=FISHERINV(y)` — inversa da transformação de Fisher (tangente hiperbólica).
pub fn fn_fisherinv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(e.num(0)?.tanh()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planilha::sheet::Sheet;

    #[test]
    fn test_estatisticas_condicionais() {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, "10".into(), None); // A1
        sheet.set_cell_raw(1, 0, "20".into(), None); // A2
        sheet.set_cell_raw(2, 0, "30".into(), None); // A3

        sheet.set_cell_raw(0, 1, "X".into(), None);  // B1
        sheet.set_cell_raw(1, 1, "Y".into(), None);  // B2
        sheet.set_cell_raw(2, 1, "X".into(), None);  // B3

        // MEDIASE(B1:B3; "X"; A1:A3) = (10 + 30)/2 = 20
        sheet.set_cell_raw(0, 2, "=MEDIASE(B1:B3; \"X\"; A1:A3)".into(), None);
        assert_eq!(sheet.get_value(0, 2), Value::Number(20.0));

        // MEDIASES(A1:A3; B1:B3; "X") = 20
        sheet.set_cell_raw(1, 2, "=MEDIASES(A1:A3; B1:B3; \"X\")".into(), None);
        assert_eq!(sheet.get_value(1, 2), Value::Number(20.0));

        // CONT.SES(B1:B3; "X") = 2
        sheet.set_cell_raw(2, 2, "=CONT.SES(B1:B3; \"X\")".into(), None);
        assert_eq!(sheet.get_value(2, 2), Value::Number(2.0));

        // MÁXIMOSES(A1:A3; B1:B3; "X") = 30
        sheet.set_cell_raw(3, 2, "=MÁXIMOSES(A1:A3; B1:B3; \"X\")".into(), None);
        assert_eq!(sheet.get_value(3, 2), Value::Number(30.0));

        // MÍNIMOSES(A1:A3; B1:B3; "X") = 10
        sheet.set_cell_raw(4, 2, "=MÍNIMOSES(A1:A3; B1:B3; \"X\")".into(), None);
        assert_eq!(sheet.get_value(4, 2), Value::Number(10.0));

        // CONTAR.VAZIO(C1:C6) = 1 (célula C6 vazia)
        sheet.set_cell_raw(0, 3, "=CONTAR.VAZIO(C1:C6)".into(), None);
        assert_eq!(sheet.get_value(0, 3), Value::Number(1.0));
    }
}

#[cfg(test)]
mod testes_criterio_excel {
    use crate::celulas::values::Value;
    use crate::planilha::sheet::Sheet;

    /// A1:A3 = 1, 2, texto "2"; B1:B3 = "abc", "b", vazio.
    fn calc(f: &str) -> Value {
        let mut s = Sheet::new("Plan1");
        s.set_cell_raw(0, 0, "1".into(), None);
        s.set_cell_raw(1, 0, "2".into(), None);
        s.set_cell_raw(2, 0, "=\"2\"".into(), None);
        s.set_cell_raw(0, 1, "abc".into(), None);
        s.set_cell_raw(1, 1, "b".into(), None);
        s.set_cell_raw(0, 5, format!("={f}"), None);
        s.get_value(0, 5)
    }

    #[test]
    fn dois_e_igual_dois_contam_o_mesmo() {
        assert_eq!(calc("CONT.SE(A1:A3;\"2\")"), Value::Number(2.0));
        assert_eq!(calc("CONT.SE(A1:A3;\"=2\")"), Value::Number(2.0));
        assert_eq!(calc("SOMASE(A1:A3;\"2\")"), Value::Number(2.0));
        assert_eq!(calc("CONT.SE(A1:A3;\"<>2\")"), Value::Number(1.0));
    }

    #[test]
    fn comparacao_so_olha_o_mesmo_tipo() {
        assert_eq!(calc("CONT.SE(B1:B3;\">0\")"), Value::Number(0.0));
        assert_eq!(calc("CONT.SE(B1:B3;\"<c\")"), Value::Number(2.0));
        assert_eq!(calc("CONT.SE(A1:B3;\"<5\")"), Value::Number(2.0));
    }
}

#[cfg(test)]
mod testes_reescrita {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::planilha::sheet::Sheet;

    /// Avalia a fórmula com A1:A5 = 1, 2, 2, 3, 10; B1:B4 = texto, VERDADEIRO, 4, vazio.
    fn calc(f: &str) -> Value {
        let mut s = Sheet::new("Plan1");
        for (i, v) in ["1", "2", "2", "3", "10"].iter().enumerate() {
            s.set_cell_raw(i as u32, 0, (*v).into(), None);
        }
        s.set_cell_raw(0, 1, "x".into(), None);
        s.set_cell_raw(1, 1, "=TRUE".into(), None);
        s.set_cell_raw(2, 1, "4".into(), None);
        s.set_cell_raw(0, 5, format!("={f}"), None);
        s.get_value(0, 5)
    }

    fn num(f: &str) -> f64 {
        match calc(f) {
            Value::Number(n) => n,
            outro => panic!("{f} deu {outro:?}"),
        }
    }

    fn erro(f: &str) -> ErrorCode {
        match calc(f) {
            Value::Error(e) => e.code,
            outro => panic!("{f} deu {outro:?}"),
        }
    }

    fn perto(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9 * b.abs().max(1.0)
    }

    #[test]
    fn funcoes_com_a_no_fim() {
        // Em intervalo: texto vale 0, VERDADEIRO vale 1, vazio é ignorado.
        assert!(perto(num("AVERAGEA(B1:B4)"), 5.0 / 3.0));
        assert_eq!(num("MAXA(B1:B2)"), 1.0);
        assert_eq!(num("MINA(B1:B3)"), 0.0);
        // Solto: texto numérico conta, texto comum dá #VALOR!.
        assert_eq!(num("AVERAGEA(\"4\";2)"), 3.0);
        assert_eq!(erro("AVERAGEA(\"abc\";2)"), ErrorCode::Value);
        assert_eq!(num("AVERAGEA(TRUE;3)"), 2.0);
        assert!(perto(num("VARA(A1:A5)"), 13.3));
        assert!(perto(num("VARPA(A1:A5)"), 10.64));
        assert!(perto(num("STDEVA(A1:A5)"), 13.3f64.sqrt()));
        assert!(perto(num("STDEVPA(A1:A5)"), 10.64f64.sqrt()));
        assert_eq!(erro("VARA(5)"), ErrorCode::Div0);
        assert_eq!(erro("AVERAGEA(C1:C2)"), ErrorCode::Div0);
    }

    #[test]
    fn posto_e_percentis() {
        assert_eq!(num("RANK.EQ(2;A1:A5)"), 3.0);
        assert_eq!(num("RANK.EQ(2;A1:A5;1)"), 2.0);
        assert_eq!(num("RANK.AVG(2;A1:A5;1)"), 2.5);
        assert_eq!(num("RANK.EQ(10;A1:A5)"), 1.0);
        assert_eq!(erro("RANK.EQ(5;A1:A5)"), ErrorCode::Na);
        // Exemplo do Excel: PERCENTIL.EXC({1;2;3;4}; 0.25) = 1.25
        assert!(perto(num("PERCENTILE.EXC({1;2;3;4};0.25)"), 1.25));
        assert_eq!(erro("PERCENTILE.EXC({1;2;3;4};0.1)"), ErrorCode::Num);
        assert!(perto(num("QUARTILE.EXC({1;2;3;4;5;6;7;8};1)"), 2.25));
        assert!(perto(num("QUARTILE.EXC({1;2;3;4;5;6;7;8};3)"), 6.75));
        assert_eq!(erro("QUARTILE.EXC({1;2;3;4};4)"), ErrorCode::Num);
    }

    #[test]
    fn posto_percentual() {
        // Dados do exemplo do Excel: 13 12 11 8 4 3 2 1 1 1.
        let d = "{13;12;11;8;4;3;2;1;1;1}";
        assert_eq!(num(&format!("PERCENTRANK.INC({d};2)")), 0.333);
        assert_eq!(num(&format!("PERCENTRANK.INC({d};4)")), 0.555);
        assert_eq!(num(&format!("PERCENTRANK.INC({d};8)")), 0.666);
        assert_eq!(num(&format!("PERCENTRANK.INC({d};5)")), 0.583);
        assert_eq!(num(&format!("PERCENTRANK({d};5)")), 0.583);
        assert_eq!(num(&format!("PERCENTRANK.INC({d};5;1)")), 0.5);
        assert_eq!(num(&format!("PERCENTRANK.EXC({d};8)")), 0.636);
        assert_eq!(erro(&format!("PERCENTRANK.INC({d};14)")), ErrorCode::Na);
        assert_eq!(erro(&format!("PERCENTRANK.INC({d};5;0)")), ErrorCode::Num);
    }

    #[test]
    fn covariancia_correlacao_e_previsao() {
        let x = "{3;2;4;5;6}";
        let y = "{9;7;12;15;17}";
        assert!(perto(num(&format!("COVARIANCE.P({x};{y})")), 5.2));
        assert!(perto(num(&format!("COVARIANCE.S({x};{y})")), 6.5));
        assert!(perto(num(&format!("PEARSON({x};{y})")), 0.997054486));
        assert_eq!(erro("COVARIANCE.P({1;2};{1;2;3})"), ErrorCode::Na);
        assert_eq!(erro("PEARSON({1;1;1};{1;2;3})"), ErrorCode::Div0);
        // Exemplo do Excel: PREVISÃO.LINEAR(30; {6;7;9;15;21}; {20;28;31;38;40}) = 10,607
        assert!((num("FORECAST.LINEAR(30;{6;7;9;15;21};{20;28;31;38;40})") - 10.6072530864).abs() < 1e-6);
    }

    #[test]
    fn padronizar_aparada_assimetria_modas_fisher() {
        assert_eq!(num("STANDARDIZE(42;40;1.5)"), 1.3333333333333333);
        assert_eq!(erro("STANDARDIZE(1;0;0)"), ErrorCode::Num);
        // Exemplo do Excel: MÉDIA.INTERNA(dados, 0,2) = 3,778
        assert!((num("TRIMMEAN({4;5;6;7;2;3;4;5;1;2;3};0.2)") - 3.7777777778).abs() < 1e-9);
        assert_eq!(erro("TRIMMEAN({1;2;3};1)"), ErrorCode::Num);
        assert!(perto(num("SKEW.P({3;4;5;2;3;4;5;6;4;7})"), 0.303193339));
        assert_eq!(erro("SKEW.P({2;2;2})"), ErrorCode::Div0);
        assert!(perto(num("FISHER(0.75)"), 0.972955074));
        assert!((num("FISHERINV(0.972955)") - 0.75).abs() < 1e-5);
        assert_eq!(erro("FISHER(1)"), ErrorCode::Num);

        let mut s = Sheet::new("Plan1");
        s.set_cell_raw(0, 0, "=MODE.MULT({1;2;3;1;2;4})".into(), None);
        assert_eq!(s.get_value(0, 0), Value::Number(1.0));
        assert_eq!(s.get_value(1, 0), Value::Number(2.0));
        assert_eq!(erro("MODE.MULT({1;2;3})"), ErrorCode::Na);
    }
}
