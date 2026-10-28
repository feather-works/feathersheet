//! Funções matemáticas da planilha em Rust: SOMA, SOMASE, ARRED, ABS, RAIZ, POTÊNCIA, etc.
//! Também: PAR, ÍMPAR, MDC, MMC, MARRED, ROMANO, BASE, trigonometria extra (COT, SEC…), TETO/ARREDMULTB.MAT, COMBINA, MUNIT, SOMASERIES, SOMAX2DY2…

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, collect, err_num, err_value, has, linhas_com_conteudo, make_predicate, LeitorFaixa};
use crate::celulas::refs::Rect;
use crate::funcoes::fn_helpers::{concluir, concluir_matriz, e_div0, e_nd, e_num, e_valor, matriz_de, num, numero, por_elemento, texto, valores_de, Res};
use crate::celulas::values::to_number;
use crate::celulas::values::{Computed, Value};


/// `=SOMA(args...)`
pub fn fn_sum(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => Computed::Scalar(Value::Number(nums.iter().sum())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=SOMASE(faixa; criterio; [faixa_soma])`
pub fn fn_sumif(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let range_node = &args[0];
    let crit_val = ev.eval(&args[1]);
    let pred = make_predicate(&crit_val);
    let sum_node = if has(args, 2) { &args[2] } else { range_node };

    let range_rect = match node_rect(range_node, ev) {
        Some(r) => r,
        None => return err_value(),
    };
    let sum_rect = match node_rect(sum_node, ev) {
        Some(r) => r,
        None => range_rect.clone(),
    };

    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&sum_rect), std::slice::from_ref(&range_rect), std::slice::from_ref(&crit_val)) {
        return Computed::Scalar(Value::Number(grupo.soma));
    }

    let criterio = LeitorFaixa::novo(ev, &range_rect);
    let soma = LeitorFaixa::novo(ev, &sum_rect);
    // Depois da última linha usada da faixa somada a soma é vazia e não
    // acrescenta nada, passe o critério ou não: `A:A` para ali.
    let linhas = linhas_com_conteudo(ev, range_rect.r1 - range_rect.r0 + 1, [&sum_rect]);
    let mut sum = 0.0;
    for r in range_rect.r0..range_rect.r0 + linhas {
        for c in range_rect.c0..=range_rect.c1 {
            let val = criterio.valor(ev, r, c);
            if pred(&val) {
                let r_offset = r - range_rect.r0;
                let c_offset = c - range_rect.c0;
                let sum_r = sum_rect.r0 + r_offset;
                let sum_c = sum_rect.c0 + c_offset;
                let sum_val = soma.valor(ev, sum_r, sum_c);
                if let Value::Number(n) = *sum_val {
                    sum += n;
                }
            }
        }
    }

    Computed::Scalar(Value::Number(sum))
}

/// `=ABS(n)`
pub fn fn_abs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.abs())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=RAIZ(n)` / `=SQRT(n)`
pub fn fn_sqrt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => {
            if n < 0.0 {
                err_num()
            } else {
                Computed::Scalar(Value::Number(n.sqrt()))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=POTENCIA(base; exp)` / `=POWER(base; exp)`
pub fn fn_power(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let base = match ev.number(&args[0]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let exp = match ev.number(&args[1]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let res = base.powf(exp);
    if res.is_finite() {
        Computed::Scalar(Value::Number(res))
    } else {
        err_num()
    }
}

/// `=ARRED(n; [num_digitos])` / `=ROUND(n; [num_digitos])`
pub fn fn_round(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let digits = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        0
    };

    Computed::Scalar(Value::Number(arredondar_casas(n, digits, f64::round)))
}

/// Arredonda `n` em `casas` decimais com `passo` (round/ceil/floor sobre o
/// módulo, o sinal volta depois). Antes de arredondar, `n` deslocado passa por
/// 15 algarismos significativos, como o Excel: `1,005 × 100` é
/// 100,49999999999999 no binário e daria 1,00 em vez de 1,01. Casas fora do
/// alcance do `f64` não estouram: 400 casas devolvem `n`, -400 devolvem 0.
fn arredondar_casas(n: f64, casas: i32, passo: fn(f64) -> f64) -> f64 {
    if !n.is_finite() || n == 0.0 || casas > 308 {
        return n;
    }
    if casas < -308 {
        return 0.0;
    }
    let fator = 10f64.powi(casas.abs());
    let x = if casas >= 0 { n.abs() * fator } else { n.abs() / fator };
    if !x.is_finite() || x >= 1e15 {
        return n; // já não tem casa decimal a arredondar
    }
    let r = passo(crate::celulas::number::round_to_precision(x, 15));
    let r = if casas >= 0 { r / fator } else { r * fator };
    r.copysign(n)
}

/// `=INT(n)`
pub fn fn_int(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.floor())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=TRUNC(n; [num_digitos])`
pub fn fn_trunc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let digits = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        0
    };

    let factor = 10.0f64.powi(digits);
    let trunc = (n * factor).trunc() / factor;
    Computed::Scalar(Value::Number(trunc))
}

/// `=MOD(n; divisor)`
pub fn fn_mod(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let div = match ev.number(&args[1]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if div == 0.0 {
        return Computed::Scalar(Value::erro(crate::celulas::values::ErrorCode::Div0));
    }
    let res = n % div;
    Computed::Scalar(Value::Number(if (res > 0.0 && div < 0.0) || (res < 0.0 && div > 0.0) { res + div } else { res }))
}

/// `=PI()`
pub fn fn_pi(args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 0, 0) {
        return Computed::Scalar(Value::Error(err));
    }
    Computed::Scalar(Value::Number(std::f64::consts::PI))
}

/// `=PRODUTO(args...)` / `=PRODUCT(args...)`
pub fn fn_product(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match collect(args, ev) {
        Ok(nums) => {
            if nums.is_empty() {
                Computed::Scalar(Value::Number(0.0))
            } else {
                let prod: f64 = nums.iter().product();
                Computed::Scalar(Value::Number(prod))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=EXP(n)`
pub fn fn_exp(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => {
            let res = n.exp();
            if res.is_finite() {
                Computed::Scalar(Value::Number(res))
            } else {
                err_num()
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=LN(n)`
pub fn fn_ln(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => {
            if n <= 0.0 {
                err_num()
            } else {
                Computed::Scalar(Value::Number(n.ln()))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=LOG(n; [base])`
pub fn fn_log(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let base = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        10.0
    };
    if n <= 0.0 || base <= 0.0 || base == 1.0 {
        err_num()
    } else {
        let r = if base == 2.0 { n.log2() } else if base == 10.0 { n.log10() } else { n.ln() / base.ln() };
        // Casos exatos (LOG(8;2) = 3): arredonda quando o resultado está a poucos ulps de um inteiro.
        let arred = r.round();
        let r = if (r - arred).abs() < 1e-12 * arred.abs().max(1.0) && base.powf(arred) == n { arred } else { r };
        Computed::Scalar(Value::Number(r))
    }
}

/// `=LOG10(n)`
pub fn fn_log10(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => {
            if n <= 0.0 {
                err_num()
            } else {
                Computed::Scalar(Value::Number(n.log10()))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ARRED.PARA.CIMA(n; [num_digitos])` / `=ROUNDUP(n; [num_digitos])`
pub fn fn_roundup(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let digits = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        0
    };
    let res = arredondar_casas(n, digits, f64::ceil);
    Computed::Scalar(Value::Number(res))
}

/// `=ARRED.PARA.BAIXO(n; [num_digitos])` / `=ROUNDDOWN(n; [num_digitos])`
pub fn fn_rounddown(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let digits = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v.trunc() as i32,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        0
    };
    let res = arredondar_casas(n, digits, f64::floor);
    Computed::Scalar(Value::Number(res))
}

/// `=TETO(n; [significancia])` / `=CEILING(n; [significancia])`
pub fn fn_ceiling(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let sig = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1.0
    };
    if sig == 0.0 {
        return Computed::Scalar(Value::Number(0.0));
    }
    let res = (n / sig).ceil() * sig;
    Computed::Scalar(Value::Number(res))
}

/// `=CHÃO(n; [significancia])` / `=FLOOR(n; [significancia])`
pub fn fn_floor(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => v,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let sig = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1.0
    };
    if sig == 0.0 {
        return Computed::Scalar(Value::Number(0.0));
    }
    let res = (n / sig).floor() * sig;
    Computed::Scalar(Value::Number(res))
}

/// `=SEN(n)` / `=SIN(n)`
pub fn fn_sin(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.sin())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=COS(n)` / `=COS(n)`
pub fn fn_cos(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.cos())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=TAN(n)` / `=TAN(n)`
pub fn fn_tan(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.tan())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ALEATÓRIO()` / `=RAND()`
pub fn fn_rand(args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 0, 0) {
        return Computed::Scalar(Value::Error(err));
    }
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x123456789abcdef0) };
    }
    let val = SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x as f64) / (u64::MAX as f64)
    });
    Computed::Scalar(Value::Number(val))
}

/// `=ALEATÓRIOBENTRE(min; max)` / `=RANDBETWEEN(min; max)`
pub fn fn_randbetween(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let min = match ev.number(&args[0]) {
        Ok(v) => v.ceil() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let max = match ev.number(&args[1]) {
        Ok(v) => v.floor() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if min > max {
        return err_num();
    }
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x987654321fedcba0) };
    }
    let range = (max - min + 1) as u64;
    let val = SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        min + (x % range) as i64
    });
    Computed::Scalar(Value::Number(val as f64))
}

/// `=SOMASES(soma_faixa; criterio_faixa1; criterio1; ...)` / `=SUMIFS(...)`
pub fn fn_sumifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 || args.len() % 2 == 0 {
        return err_value();
    }
    let sum_rect = match node_rect(&args[0], ev) {
        Some(r) => r,
        None => return err_value(),
    };

    struct CriteriaPair {
        rect: Rect,
        pred: crate::funcoes::fn_helpers::Predicate,
        leitor: LeitorFaixa,
    }

    let (rects, crits) = match crate::funcoes::fn_helpers::faixas_e_criterios(args, 1, ev) {
        Ok(x) => x,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if let Some(grupo) = crate::funcoes::fn_helpers::grupo_agregado(ev, Some(&sum_rect), &rects, &crits) {
        return Computed::Scalar(Value::Number(grupo.soma));
    }
    let pairs: Vec<CriteriaPair> = rects.into_iter().zip(crits.iter())
        .map(|(rect, crit)| CriteriaPair { leitor: LeitorFaixa::novo(ev, &rect), rect, pred: make_predicate(crit) })
        .collect();

    let soma = LeitorFaixa::novo(ev, &sum_rect);
    // Como no SOMASE: depois da última linha usada da faixa somada não há
    // o que somar.
    let linhas = linhas_com_conteudo(ev, sum_rect.r1 - sum_rect.r0 + 1, [&sum_rect]);
    let mut sum = 0.0;
    for r in sum_rect.r0..sum_rect.r0 + linhas {
        for c in sum_rect.c0..=sum_rect.c1 {
            let r_off = r - sum_rect.r0;
            let c_off = c - sum_rect.c0;
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
                let val = soma.valor(ev, r, c);
                if let Value::Number(n) = *val {
                    sum += n;
                }
            }
        }
    }

    Computed::Scalar(Value::Number(sum))
}

/// `=SOMARPRODUTO(faixa1; faixa2; ...)` / `=SUMPRODUCT(...)`
pub fn fn_sumproduct(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.is_empty() {
        return err_value();
    }
    let rects: Vec<Option<Rect>> = args.iter().map(|a| node_rect(a, ev)).collect();
    // Só retângulos do mesmo tamanho: a linha toda vazia multiplica 0 e não
    // tem erro, então `A:A` para na última linha usada de todas as faixas.
    let corte = match rects.iter().map(Option::as_ref).collect::<Option<Vec<&Rect>>>() {
        Some(todas) => {
            let tamanho = |r: &Rect| (r.r1 - r.r0 + 1, r.c1 - r.c0 + 1);
            let (linhas, _) = tamanho(todas[0]);
            todas.iter().all(|r| tamanho(r) == tamanho(todas[0]))
                .then(|| linhas_com_conteudo(ev, linhas, todas.iter().copied()))
        }
        None => None,
    };
    let mut cols: Vec<Vec<f64>> = Vec::with_capacity(args.len());
    for (a, rect) in args.iter().zip(&rects) {
        let mut list = Vec::new();
        if let Some(rect) = rect {
            let leitor = LeitorFaixa::novo(ev, rect);
            for r in rect.r0..rect.r0 + corte.unwrap_or(rect.r1 - rect.r0 + 1) {
                for c in rect.c0..=rect.c1 {
                    match *leitor.valor(ev, r, c) {
                        Value::Error(ref e) => return Computed::Scalar(Value::Error(e.clone())),
                        Value::Number(n) => list.push(n),
                        _ => list.push(0.0),
                    }
                }
            }
        } else {
            // Expressão (`(E2:E9>5)*1`, `MÊS(A2:A9)`): avaliada como matriz,
            // como o Excel faz dentro do SOMARPRODUTO.
            let m = match ev.matrix_arg(a) {
                Ok(m) => m,
                Err(e) => return Computed::Scalar(Value::Error(e)),
            };
            for v in m.rows().iter().flatten() {
                match v {
                    Value::Error(e) => return Computed::Scalar(Value::Error(e.clone())),
                    Value::Number(n) => list.push(*n),
                    _ => list.push(0.0),
                }
            }
        }
        cols.push(list);
    }
    let len = cols[0].len();
    if cols.iter().any(|c| c.len() != len) {
        return err_value();
    }
    let mut total = 0.0;
    for i in 0..len {
        let mut p = 1.0;
        for col in &cols {
            p *= col[i];
        }
        total += p;
    }
    Computed::Scalar(Value::Number(total))
}

/// `=SINAL(n)` / `=SIGN(n)`
pub fn fn_sign(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(if n > 0.0 { 1.0 } else if n < 0.0 { -1.0 } else { 0.0 })),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ASEN(n)` / `=ASIN(n)`
pub fn fn_asin(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => {
            if (-1.0..=1.0).contains(&n) {
                Computed::Scalar(Value::Number(n.asin()))
            } else {
                err_num()
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ACOS(n)` / `=ACOS(n)`
pub fn fn_acos(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => {
            if (-1.0..=1.0).contains(&n) {
                Computed::Scalar(Value::Number(n.acos()))
            } else {
                err_num()
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ATG(n)` / `=ATAN(n)`
pub fn fn_atan(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.atan())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=ATAN2(x; y)` / `=ATAN2(x; y)`
pub fn fn_atan2(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let x = match ev.number(&args[0]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let y = match ev.number(&args[1]) {
        Ok(n) => n,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    Computed::Scalar(Value::Number(y.atan2(x)))
}

/// `=SENH(n)` / `=SINH(n)`
pub fn fn_sinh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.sinh())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=COSH(n)` / `=COSH(n)`
pub fn fn_cosh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.cosh())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=TGH(n)` / `=TANH(n)`
pub fn fn_tanh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n.tanh())),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=GRAUS(n)` / `=DEGREES(n)`
pub fn fn_degrees(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n * 180.0 / std::f64::consts::PI)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=RADIANOS(n)` / `=RADIANS(n)`
pub fn fn_radians(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => Computed::Scalar(Value::Number(n * std::f64::consts::PI / 180.0)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=FATORIAL(n)` / `=FACT(n)`
pub fn fn_fact(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) { return Computed::Scalar(Value::Error(err)); }
    match ev.number(&args[0]) {
        Ok(n) => {
            if n < 0.0 || n > 170.0 {
                err_num()
            } else {
                let mut res = 1.0;
                for i in 2..=(n.trunc() as i64) {
                    res *= i as f64;
                }
                Computed::Scalar(Value::Number(res))
            }
        }
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=COMBIN(n; k)` / `=COMBIN(n; k)`
pub fn fn_combin(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let n = match ev.number(&args[0]) {
        Ok(v) => v.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let k = match ev.number(&args[1]) {
        Ok(v) => v.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if n < 0 || k < 0 || k > n {
        return err_num();
    }
    let k = k.min(n - k);
    let mut res = 1.0;
    for i in 1..=k {
        res = res * (n - k + i) as f64 / i as f64;
    }
    Computed::Scalar(Value::Number(res.round()))
}

/// `=PERMUTAÇÃO(n; k)` / `=PERMUT(n; k)`
pub fn fn_permut(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) { return Computed::Scalar(Value::Error(err)); }
    let n = match ev.number(&args[0]) {
        Ok(v) => v.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let k = match ev.number(&args[1]) {
        Ok(v) => v.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if n < 0 || k < 0 || k > n {
        return err_num();
    }
    let mut res = 1.0;
    for i in 0..k {
        res *= (n - i) as f64;
    }
    if res.is_finite() {
        Computed::Scalar(Value::Number(res))
    } else {
        err_num()
    }
}

// ---------------------------------------------------------------------------
// Reescrita limpa (16/10): inteiros, múltiplos, trigonometria extra, bases,
// arredondamentos por múltiplo, combinatória e somas de pares de matrizes.
// ---------------------------------------------------------------------------

/// Números de argumentos de coleção do jeito do Excel para funções como
/// MDC/MMC/SOMAQUAD: dentro de intervalo texto e lógico são ignorados; solto,
/// lógico vale 1/0, texto numérico é lido e texto comum dá `#VALOR!`.
pub(crate) fn numeros_estritos(ev: &Evaluator<'_>, args: &[Node]) -> Res<Vec<f64>> {
    let mut saida = Vec::new();
    for a in args {
        // Referência a uma célula só também é coleção: texto e lógico ficam de fora.
        let ref_unica = matches!(a, Node::Ref { .. });
        for (v, de_colecao) in valores_de(ev, a)? {
            let de_colecao = de_colecao || ref_unica;
            match v {
                Value::Number(n) => saida.push(n),
                Value::Bool(b) if !de_colecao => saida.push(if b { 1.0 } else { 0.0 }),
                Value::Text(_) if !de_colecao => saida.push(to_number(&v).map_err(|_| e_valor())?),
                Value::Error(e) => return Err(e),
                _ => {}
            }
        }
    }
    Ok(saida)
}

/// Os pares (x, y) numéricos de duas matrizes do mesmo tamanho; o par é
/// descartado se qualquer lado não for número (texto, lógico ou vazio).
/// Tamanhos diferentes dão `#N/D`; erro dentro das matrizes propaga.
pub(crate) fn pares_numericos(ev: &Evaluator<'_>, a: &Node, b: &Node) -> Res<Vec<(f64, f64)>> {
    let ma = matriz_de(ev, a)?;
    let mb = matriz_de(ev, b)?;
    if ma.size() != mb.size() {
        return Err(e_nd());
    }
    let mut pares = Vec::new();
    for (x, y) in ma.values().zip(mb.values()) {
        if let Value::Error(e) = x {
            return Err(e.clone());
        }
        if let Value::Error(e) = y {
            return Err(e.clone());
        }
        if let (Value::Number(p), Value::Number(q)) = (x, y) {
            pares.push((*p, *q));
        }
    }
    Ok(pares)
}

const LIMITE_INTEIRO: f64 = 9_007_199_254_740_992.0; // 2^53

fn mdc_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `=PAR(núm)` / `=EVEN(number)` — arredonda para longe do zero até o par mais próximo.
pub fn fn_even(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let x = e.num(0)?;
        numero(if x == 0.0 { 0.0 } else { x.signum() * (x.abs() / 2.0).ceil() * 2.0 })
    })
}

/// `=ÍMPAR(núm)` / `=ODD(number)` — arredonda para longe do zero até o ímpar mais próximo.
pub fn fn_odd(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let x = e.num(0)?;
        let mut a = x.abs().ceil();
        if a % 2.0 == 0.0 {
            a += 1.0;
        }
        numero(if x < 0.0 { -a } else { a })
    })
}

/// `=MDC(núm1; ...)` / `=GCD(number1; ...)` — máximo divisor comum.
pub fn fn_gcd(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let mut g = 0u64;
        for n in numeros_estritos(ev, args)? {
            let n = n.trunc();
            if !(0.0..LIMITE_INTEIRO).contains(&n) {
                return Err(e_num());
            }
            g = mdc_u64(g, n as u64);
        }
        numero(g as f64)
    })())
}

/// `=MMC(núm1; ...)` / `=LCM(number1; ...)` — mínimo múltiplo comum.
pub fn fn_lcm(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let mut m: u128 = 1;
        let mut zero = false;
        for n in numeros_estritos(ev, args)? {
            let n = n.trunc();
            if !(0.0..LIMITE_INTEIRO).contains(&n) {
                return Err(e_num());
            }
            let n = n as u64;
            if n == 0 {
                zero = true;
                continue;
            }
            let g = mdc_u64((m % n as u128) as u64, n) as u128;
            m = m / g * n as u128;
            if m >= LIMITE_INTEIRO as u128 {
                return Err(e_num());
            }
        }
        numero(if zero { 0.0 } else { m as f64 })
    })())
}

/// `=MARRED(núm; múltiplo)` / `=MROUND(number; multiple)` — arredonda ao múltiplo mais próximo.
pub fn fn_mround(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let (n, m) = (e.num(0)?, e.num(1)?);
        if m == 0.0 {
            return numero(0.0);
        }
        if n * m < 0.0 {
            return Err(e_num());
        }
        numero(sem_poeira(quociente_limpo(n, m).round() * m))
    })
}

/// `=QUOCIENTE(numerador; denominador)` / `=QUOTIENT(numerator; denominator)` — parte inteira da divisão.
pub fn fn_quotient(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let (n, d) = (e.num(0)?, e.num(1)?);
        if d == 0.0 {
            return Err(e_div0());
        }
        numero((n / d).trunc() + 0.0)
    })
}

/// `=SOMAQUAD(núm1; ...)` / `=SUMSQ(number1; ...)` — soma dos quadrados.
pub fn fn_sumsq(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        numero(numeros_estritos(ev, args)?.iter().map(|n| n * n).sum())
    })())
}

/// `=RAIZPI(núm)` / `=SQRTPI(number)` — raiz de (núm × π).
pub fn fn_sqrtpi(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let x = e.num(0)?;
        if x < 0.0 {
            return Err(e_num());
        }
        numero((x * std::f64::consts::PI).sqrt())
    })
}

/// `=FATDUPLO(núm)` / `=FACTDOUBLE(number)` — fatorial duplo.
pub fn fn_factdouble(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let n = e.int(0)?;
        if n < -1 {
            return Err(e_num());
        }
        let mut r = 1.0f64;
        let mut i = n;
        while i > 1 {
            r *= i as f64;
            if !r.is_finite() {
                break;
            }
            i -= 2;
        }
        numero(r)
    })
}

/// Algarismos romanos, do maior para o menor valor.
const ROMANOS: [(u32, char); 7] =
    [(1000, 'M'), (500, 'D'), (100, 'C'), (50, 'L'), (10, 'X'), (5, 'V'), (1, 'I')];

/// `=ROMANO(núm; [forma])` / `=ROMAN(number; [form])` — número em algarismos romanos.
/// Forma 0 é a clássica; 1 a 4 deixam a letra subtraída recuar até esse número de
/// passos enquanto o resto do número ainda comporta (499: CDXCIX, LDVLIV, XDIX,
/// VDIV, ID). `VERDADEIRO` vale 0 e `FALSO` vale 4.
pub fn fn_roman(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let forma = if e.tem(1) {
            match e.valor(1) {
                Value::Bool(true) => 0,
                Value::Bool(false) => 4,
                outro => {
                    let f = num(outro)?.trunc();
                    if !(0.0..=4.0).contains(&f) {
                        return Err(e_valor());
                    }
                    f as usize
                }
            }
        } else {
            0
        };
        let n = e.int(0)?;
        if !(0..=3999).contains(&n) {
            return Err(e_valor());
        }
        let mut resto = n as u32;
        let mut saida = String::new();
        for _ in 0..resto / 1000 {
            saida.push('M');
        }
        resto %= 1000;
        // Centenas, dezenas e unidades: posições 2, 4 e 6 da tabela.
        for i in [2usize, 4, 6] {
            let unidade = ROMANOS[i].0;
            let d = resto / unidade;
            resto %= unidade;
            match d {
                0..=3 => (0..d).for_each(|_| saida.push(ROMANOS[i].1)),
                5..=8 => {
                    saida.push(ROMANOS[i - 1].1);
                    (0..d - 5).for_each(|_| saida.push(ROMANOS[i].1));
                }
                _ => {
                    // 4 e 9: letra menor antes da maior; nas formas 1 a 4 a letra
                    // menor pode recuar enquanto o número ainda comporta.
                    let grande = if d == 4 { i - 1 } else { i - 2 };
                    let total = resto + d * unidade; // valor ainda por escrever, deste dígito para baixo
                    let mut j = i;
                    for _ in 0..forma {
                        if j < 6 && ROMANOS[grande].0 - ROMANOS[j + 1].0 <= total {
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    saida.push(ROMANOS[j].1);
                    saida.push(ROMANOS[grande].1);
                    // O que sobra do dígito + resto depois de escrever o par.
                    resto = total - (ROMANOS[grande].0 - ROMANOS[j].0);
                }
            }
        }
        texto(saida)
    })
}

fn escalar(args: &[Node], ev: &Evaluator<'_>, f: impl Fn(f64) -> Res<f64>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(f(e.num(0)?)? + 0.0))
}

/// Excel recusa argumento trigonométrico de módulo igual ou maior que 2^27.
fn angulo(x: f64) -> Res<f64> {
    if x.abs() >= 134_217_728.0 {
        Err(e_num())
    } else {
        Ok(x)
    }
}

/// `=ACOSH(núm)` — cosseno hiperbólico inverso.
pub fn fn_acosh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| if x < 1.0 { Err(e_num()) } else { Ok(x.acosh()) })
}

/// `=ASENH(núm)` / `=ASINH(number)` — seno hiperbólico inverso.
pub fn fn_asinh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| Ok(x.asinh()))
}

/// `=ATANH(núm)` — tangente hiperbólica inversa.
pub fn fn_atanh(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| if x.abs() >= 1.0 { Err(e_num()) } else { Ok(x.atanh()) })
}

/// `=ACOT(núm)` — arco cotangente, entre 0 e π.
pub fn fn_acot(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| Ok(std::f64::consts::FRAC_PI_2 - x.atan()))
}

/// `=ACOTH(núm)` — cotangente hiperbólica inversa.
pub fn fn_acoth(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| {
        if x.abs() <= 1.0 {
            Err(e_num())
        } else {
            Ok(0.5 * ((x + 1.0) / (x - 1.0)).ln())
        }
    })
}

/// `=COT(núm)` — cotangente.
pub fn fn_cot(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| {
        let x = angulo(x)?;
        if x == 0.0 {
            Err(e_div0())
        } else {
            Ok(1.0 / x.tan())
        }
    })
}

/// `=COTH(núm)` — cotangente hiperbólica.
pub fn fn_coth(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| {
        let x = angulo(x)?;
        if x == 0.0 {
            Err(e_div0())
        } else {
            Ok(1.0 / x.tanh())
        }
    })
}

/// `=CSC(núm)` — cossecante.
pub fn fn_csc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| {
        let x = angulo(x)?;
        if x == 0.0 {
            Err(e_div0())
        } else {
            Ok(1.0 / x.sin())
        }
    })
}

/// `=CSCH(núm)` — cossecante hiperbólica.
pub fn fn_csch(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| {
        let x = angulo(x)?;
        if x == 0.0 {
            Err(e_div0())
        } else {
            Ok(1.0 / x.sinh())
        }
    })
}

/// `=SEC(núm)` — secante.
pub fn fn_sec(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| Ok(1.0 / angulo(x)?.cos()))
}

/// `=SECH(núm)` — secante hiperbólica.
pub fn fn_sech(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    escalar(args, ev, |x| Ok(1.0 / angulo(x)?.cosh()))
}

const DIGITOS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// `=BASE(núm; raiz; [comprimento_mín])` / `=BASE(number; radix; [min_length])`
/// — número como texto em outra base (2 a 36).
pub fn fn_base(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 3, |e| {
        let n = e.num(0)?.trunc();
        let raiz = e.int(1)?;
        let minimo = e.int_ou(2, 0)?;
        if !(0.0..LIMITE_INTEIRO).contains(&n) || !(2..=36).contains(&raiz) || !(0..=255).contains(&minimo) {
            return Err(e_num());
        }
        let mut n = n as u64;
        let mut letras = Vec::new();
        if n == 0 {
            letras.push(b'0');
        }
        while n > 0 {
            letras.push(DIGITOS[(n % raiz as u64) as usize]);
            n /= raiz as u64;
        }
        while (letras.len() as i64) < minimo {
            letras.push(b'0');
        }
        letras.reverse();
        texto(String::from_utf8_lossy(&letras).into_owned())
    })
}

/// `=DECIMAL(texto; raiz)` / `=DECIMAL(text; radix)` — texto em outra base para número decimal.
pub fn fn_decimal(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let t = e.texto(0)?.trim().to_string();
        let raiz = e.int(1)?;
        if !(2..=36).contains(&raiz) || t.chars().count() > 255 {
            return Err(e_num());
        }
        let mut total = 0.0f64;
        for c in t.chars() {
            let d = c.to_digit(36).ok_or_else(e_num)? as i64;
            if d >= raiz {
                return Err(e_num());
            }
            total = total * raiz as f64 + d as f64;
            if total >= LIMITE_INTEIRO {
                return Err(e_num());
            }
        }
        numero(total)
    })
}

/// `=ARÁBICO(texto)` / `=ARABIC(text)` — algarismos romanos para número.
pub fn fn_arabic(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let t = e.texto(0)?;
        let t = t.trim();
        if t.chars().count() > 255 {
            return Err(e_valor());
        }
        let (negativo, t) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        };
        let valores: Vec<i64> = t
            .chars()
            .map(|c| match c.to_ascii_uppercase() {
                'I' => Ok(1),
                'V' => Ok(5),
                'X' => Ok(10),
                'L' => Ok(50),
                'C' => Ok(100),
                'D' => Ok(500),
                'M' => Ok(1000),
                _ => Err(e_valor()),
            })
            .collect::<Res<_>>()?;
        let mut total = 0i64;
        for (i, v) in valores.iter().enumerate() {
            if valores.get(i + 1).is_some_and(|prox| prox > v) {
                total -= v;
            } else {
                total += v;
            }
        }
        numero(if negativo { -total } else { total } as f64)
    })
}

/// Quociente `n / s` com a poeira de ponto flutuante limpa (0,3 / 0,1 vale 3).
fn quociente_limpo(n: f64, s: f64) -> f64 {
    let q = n / s;
    let r = q.round();
    if (q - r).abs() <= 1e-12 * q.abs().max(1.0) {
        r
    } else {
        q
    }
}

/// Tira a poeira de ponto flutuante do resultado (1,4000000000000001 vira 1,4):
/// mantém 14 algarismos significativos.
fn sem_poeira(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x + 0.0;
    }
    let escala = 10f64.powi(13 - x.abs().log10().floor() as i32);
    if !escala.is_finite() || !(x * escala).is_finite() {
        return x;
    }
    (x * escala).round() / escala
}

fn para_cima(n: f64, s: f64) -> f64 {
    sem_poeira(quociente_limpo(n, s).ceil() * s)
}

fn para_baixo(n: f64, s: f64) -> f64 {
    sem_poeira(quociente_limpo(n, s).floor() * s)
}

/// `=TETO.MAT(núm; [significância]; [modo])` / `=CEILING.MATH(number; [significance]; [mode])`
/// — arredonda para cima; com modo diferente de zero, negativos vão para longe do zero.
pub fn fn_ceiling_math(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 3, |e| {
        let n = e.num(0)?;
        let s = e.num_ou(1, 1.0)?.abs();
        let modo = e.num_ou(2, 0.0)? != 0.0;
        if s == 0.0 {
            return numero(0.0);
        }
        numero(if n < 0.0 && modo { para_baixo(n, s) } else { para_cima(n, s) })
    })
}

/// `=TETO.PRECISO(núm; [significância])` / `=CEILING.PRECISE(number; [significance])`
/// — sempre para cima, ignora o sinal da significância.
pub fn fn_ceiling_precise(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let n = e.num(0)?;
        let s = e.num_ou(1, 1.0)?.abs();
        numero(if s == 0.0 { 0.0 } else { para_cima(n, s) })
    })
}

/// `=ISO.TETO(núm; [significância])` / `=ISO.CEILING(number; [significance])` — igual a CEILING.PRECISE.
pub fn fn_iso_ceiling(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    fn_ceiling_precise(args, ev)
}

/// `=ARREDMULTB.MAT(núm; [significância]; [modo])` / `=FLOOR.MATH(number; [significance]; [mode])`
/// — arredonda para baixo; com modo diferente de zero, negativos vão para o zero.
pub fn fn_floor_math(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 3, |e| {
        let n = e.num(0)?;
        let s = e.num_ou(1, 1.0)?.abs();
        let modo = e.num_ou(2, 0.0)? != 0.0;
        if s == 0.0 {
            return numero(0.0);
        }
        numero(if n < 0.0 && modo { para_cima(n, s) } else { para_baixo(n, s) })
    })
}

/// `=ARREDMULTB.PRECISO(núm; [significância])` / `=FLOOR.PRECISE(number; [significance])`
/// — sempre para baixo, ignora o sinal da significância.
pub fn fn_floor_precise(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let n = e.num(0)?;
        let s = e.num_ou(1, 1.0)?.abs();
        numero(if s == 0.0 { 0.0 } else { para_baixo(n, s) })
    })
}

/// Combinações C(n, k) em ponto flutuante, sem passar por fatoriais gigantes.
fn binomial(n: u64, k: u64) -> f64 {
    let k = k.min(n - k.min(n));
    let mut r = 1.0f64;
    for i in 1..=k {
        r = r * (n - k + i) as f64 / i as f64;
        if !r.is_finite() {
            break;
        }
    }
    r.round()
}

/// `=COMBINA(núm; núm_escolhido)` / `=COMBINA(number; number_chosen)` — combinações com repetição.
pub fn fn_combina(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let (n, k) = (e.int(0)?, e.int(1)?);
        if n < 0 || k < 0 {
            return Err(e_num());
        }
        if k == 0 {
            return numero(1.0);
        }
        if n == 0 {
            return Err(e_num());
        }
        numero(binomial((n + k - 1) as u64, k as u64))
    })
}

/// `=MULTINOMIAL(núm1; ...)` — (soma)! dividido pelo produto dos fatoriais.
pub fn fn_multinomial(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if args.is_empty() {
            return Err(e_valor());
        }
        let mut soma = 0u64;
        let mut r = 1.0f64;
        for n in numeros_estritos(ev, args)? {
            let n = n.trunc();
            if n < 0.0 || n >= LIMITE_INTEIRO {
                return Err(e_num());
            }
            let k = n as u64;
            soma += k;
            r *= binomial(soma, k);
            if !r.is_finite() {
                return Err(e_num());
            }
        }
        numero(r)
    })())
}

/// `=MUNIT(dimensão)` — matriz identidade.
pub fn fn_munit(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir_matriz((|| {
        if let Some(e) = arity(args, 1, 1) {
            return Err(e);
        }
        let n = ev.number(&args[0])?.trunc();
        if n < 1.0 {
            return Err(e_valor());
        }
        if n > 1000.0 {
            return Err(e_num());
        }
        let n = n as usize;
        Ok((0..n)
            .map(|i| (0..n).map(|j| Value::Number(if i == j { 1.0 } else { 0.0 })).collect())
            .collect())
    })())
}

/// `=SOMASERIES(x; n; m; coeficientes)` / `=SERIESSUM(x; n; m; coefficients)`
/// — soma de a_i · x^(n + i·m).
pub fn fn_seriessum(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if let Some(e) = arity(args, 4, 4) {
            return Err(e);
        }
        let x = ev.number(&args[0])?;
        let n = ev.number(&args[1])?;
        let m = ev.number(&args[2])?;
        let coef = matriz_de(ev, &args[3])?;
        let mut soma = 0.0;
        for (i, c) in coef.values().enumerate() {
            let a = match c {
                Value::Number(a) => *a,
                Value::Empty => 0.0,
                Value::Error(e) => return Err(e.clone()),
                _ => return Err(e_valor()),
            };
            let expoente = n + i as f64 * m;
            if x == 0.0 && expoente == 0.0 {
                return Err(e_num());
            }
            soma += a * x.powf(expoente);
        }
        numero(soma)
    })())
}

fn soma_de_pares(args: &[Node], ev: &Evaluator<'_>, f: impl Fn(f64, f64) -> f64) -> Computed {
    concluir((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let pares = pares_numericos(ev, &args[0], &args[1])?;
        if pares.is_empty() {
            return Err(e_div0());
        }
        numero(pares.iter().map(|(x, y)| f(*x, *y)).sum())
    })())
}

/// `=SOMAX2DY2(matriz_x; matriz_y)` / `=SUMX2MY2(array_x; array_y)` — soma de x² − y².
pub fn fn_sumx2my2(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    soma_de_pares(args, ev, |x, y| x * x - y * y)
}

/// `=SOMAX2SY2(matriz_x; matriz_y)` / `=SUMX2PY2(array_x; array_y)` — soma de x² + y².
pub fn fn_sumx2py2(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    soma_de_pares(args, ev, |x, y| x * x + y * y)
}

/// `=SOMAXMY2(matriz_x; matriz_y)` / `=SUMXMY2(array_x; array_y)` — soma de (x − y)².
pub fn fn_sumxmy2(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    soma_de_pares(args, ev, |x, y| (x - y) * (x - y))
}

/// `=PERCENTOF(dados_subconjunto; dados_total)` — soma do subconjunto dividida pela soma do total.
pub fn fn_percentof(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    concluir((|| {
        if let Some(e) = arity(args, 2, 2) {
            return Err(e);
        }
        let parte: f64 = numeros_estritos(ev, &args[0..1])?.iter().sum();
        let total: f64 = numeros_estritos(ev, &args[1..2])?.iter().sum();
        if total == 0.0 {
            return Err(e_div0());
        }
        numero(parte / total)
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planilha::sheet::Sheet;

    #[test]
    fn test_sumifs_e_sumproduct() {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, "10".into(), None); // A1
        sheet.set_cell_raw(1, 0, "20".into(), None); // A2
        sheet.set_cell_raw(2, 0, "30".into(), None); // A3

        sheet.set_cell_raw(0, 1, "A".into(), None);  // B1
        sheet.set_cell_raw(1, 1, "B".into(), None);  // B2
        sheet.set_cell_raw(2, 1, "A".into(), None);  // B3

        // SOMASES(A1:A3; B1:B3; "A") = 10 + 30 = 40
        sheet.set_cell_raw(0, 2, "=SOMASES(A1:A3; B1:B3; \"A\")".into(), None);
        assert_eq!(sheet.get_value(0, 2), Value::Number(40.0));

        // SOMARPRODUTO(A1:A3; A1:A3) = 100 + 400 + 900 = 1400
        sheet.set_cell_raw(1, 2, "=SOMARPRODUTO(A1:A3; A1:A3)".into(), None);
        assert_eq!(sheet.get_value(1, 2), Value::Number(1400.0));
    }

    #[test]
    fn test_math_g19() {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, "=SINAL(-5)".into(), None);
        assert_eq!(sheet.get_value(0, 0), Value::Number(-1.0));

        sheet.set_cell_raw(0, 1, "=FATORIAL(5)".into(), None);
        assert_eq!(sheet.get_value(0, 1), Value::Number(120.0));

        sheet.set_cell_raw(0, 2, "=COMBIN(5; 2)".into(), None);
        assert_eq!(sheet.get_value(0, 2), Value::Number(10.0));

        sheet.set_cell_raw(0, 3, "=PERMUTAÇÃO(5; 2)".into(), None);
        assert_eq!(sheet.get_value(0, 3), Value::Number(20.0));

        sheet.set_cell_raw(0, 4, "=GRAUS(PI())".into(), None);
        assert_eq!(sheet.get_value(0, 4), Value::Number(180.0));

        sheet.set_cell_raw(0, 5, "=RADIANOS(180)".into(), None);
        if let Value::Number(n) = sheet.get_value(0, 5) {
            assert!((n - std::f64::consts::PI).abs() < 1e-10);
        } else {
            panic!("Expected number");
        }
    }
}

#[cfg(test)]
mod testes_reescrita {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::planilha::sheet::Sheet;

    /// Avalia uma fórmula numa planilha vazia (com A1:A3 = 1, 2, 3).
    fn calc(f: &str) -> Value {
        let mut s = Sheet::new("Plan1");
        for (i, v) in ["1", "2", "3"].iter().enumerate() {
            s.set_cell_raw(i as u32, 0, (*v).into(), None);
        }
        s.set_cell_raw(0, 5, format!("={f}"), None);
        s.get_value(0, 5)
    }

    fn num(f: &str) -> f64 {
        match calc(f) {
            Value::Number(n) => n,
            outro => panic!("{f} deu {outro:?}"),
        }
    }

    fn txt(f: &str) -> String {
        match calc(f) {
            Value::Text(t) => t,
            outro => panic!("{f} deu {outro:?}"),
        }
    }

    fn erro(f: &str) -> ErrorCode {
        match calc(f) {
            Value::Error(e) => e.code,
            outro => panic!("{f} deu {outro:?}"),
        }
    }

    #[test]
    fn arred_corrige_o_binario_e_nao_estoura() {
        assert_eq!(num("ARRED(1,005;2)"), 1.01);
        assert_eq!(num("ARRED(0,285;2)"), 0.29);
        assert_eq!(num("ARRED(-1,005;2)"), -1.01);
        assert_eq!(num("ARRED(1;400)"), 1.0);
        assert_eq!(num("ARRED(1E300;-400)"), 0.0);
        assert_eq!(num("ARRED(1234,5;-2)"), 1200.0);
        assert_eq!(num("ARREDONDAR.PARA.CIMA(0,1*3;1)"), 0.3);
        assert_eq!(num("ARREDONDAR.PARA.BAIXO(-1,99;1)"), -1.9);
    }

    #[test]
    fn par_e_impar() {
        assert_eq!(num("EVEN(1.5)"), 2.0);
        assert_eq!(num("EVEN(3)"), 4.0);
        assert_eq!(num("EVEN(2)"), 2.0);
        assert_eq!(num("EVEN(-1)"), -2.0);
        assert_eq!(num("ODD(1.5)"), 3.0);
        assert_eq!(num("ODD(3)"), 3.0);
        assert_eq!(num("ODD(2)"), 3.0);
        assert_eq!(num("ODD(-1)"), -1.0);
        assert_eq!(num("ODD(-2)"), -3.0);
        assert_eq!(num("ODD(0)"), 1.0);
    }

    #[test]
    fn mdc_mmc_e_afins() {
        assert_eq!(num("GCD(5;2)"), 1.0);
        assert_eq!(num("GCD(24;36)"), 12.0);
        assert_eq!(num("GCD(7;1)"), 1.0);
        assert_eq!(num("GCD(5;0)"), 5.0);
        assert_eq!(erro("GCD(-1;2)"), ErrorCode::Num);
        assert_eq!(num("LCM(5;2)"), 10.0);
        assert_eq!(num("LCM(24;36)"), 72.0);
        assert_eq!(num("LCM(A1:A3)"), 6.0);
        assert_eq!(num("MROUND(10;3)"), 9.0);
        assert_eq!(num("MROUND(-10;-3)"), -9.0);
        assert_eq!(num("MROUND(1.3;0.2)"), 1.4);
        assert_eq!(erro("MROUND(5;-2)"), ErrorCode::Num);
        assert_eq!(num("QUOTIENT(5;2)"), 2.0);
        assert_eq!(num("QUOTIENT(-10;3)"), -3.0);
        assert_eq!(erro("QUOTIENT(1;0)"), ErrorCode::Div0);
        assert_eq!(num("SUMSQ(3;4)"), 25.0);
        assert_eq!(num("SUMSQ(A1:A3)"), 14.0);
        assert!((num("SQRTPI(1)") - std::f64::consts::PI.sqrt()).abs() < 1e-12);
        assert_eq!(erro("SQRTPI(-1)"), ErrorCode::Num);
        assert_eq!(num("FACTDOUBLE(6)"), 48.0);
        assert_eq!(num("FACTDOUBLE(7)"), 105.0);
        assert_eq!(num("FACTDOUBLE(-1)"), 1.0);
        assert_eq!(erro("FACTDOUBLE(-2)"), ErrorCode::Num);
    }

    #[test]
    fn romano_nas_cinco_formas() {
        assert_eq!(txt("ROMAN(499)"), "CDXCIX");
        assert_eq!(txt("ROMAN(499;0)"), "CDXCIX");
        assert_eq!(txt("ROMAN(499;1)"), "LDVLIV");
        assert_eq!(txt("ROMAN(499;2)"), "XDIX");
        assert_eq!(txt("ROMAN(499;3)"), "VDIV");
        assert_eq!(txt("ROMAN(499;4)"), "ID");
        assert_eq!(txt("ROMAN(1994)"), "MCMXCIV");
        assert_eq!(txt("ROMAN(3999)"), "MMMCMXCIX");
        assert_eq!(txt("ROMAN(0)"), "");
        assert_eq!(txt("ROMAN(4;TRUE)"), "IV");
        assert_eq!(txt("ROMAN(499;FALSE)"), "ID");
        assert_eq!(txt("ROMAN(3888)"), "MMMDCCCLXXXVIII");
        assert_eq!(erro("ROMAN(4000)"), ErrorCode::Value);
        assert_eq!(erro("ROMAN(-1)"), ErrorCode::Value);
        assert_eq!(erro("ROMAN(5;5)"), ErrorCode::Value);
    }

    #[test]
    fn arabico_base_e_decimal() {
        assert_eq!(num("ARABIC(\"LVII\")"), 57.0);
        assert_eq!(num("ARABIC(\"mcmxii\")"), 1912.0);
        assert_eq!(num("ARABIC(\"-IV\")"), -4.0);
        assert_eq!(num("ARABIC(\"\")"), 0.0);
        assert_eq!(erro("ARABIC(\"ABC\")"), ErrorCode::Value);
        assert_eq!(txt("BASE(7;2)"), "111");
        assert_eq!(txt("BASE(100;16)"), "64");
        assert_eq!(txt("BASE(15;2;10)"), "0000001111");
        assert_eq!(txt("BASE(0;2)"), "0");
        assert_eq!(erro("BASE(-1;2)"), ErrorCode::Num);
        assert_eq!(erro("BASE(5;1)"), ErrorCode::Num);
        assert_eq!(num("DECIMAL(\"FF\";16)"), 255.0);
        assert_eq!(num("DECIMAL(\"zap\";36)"), 45745.0);
        assert_eq!(num("DECIMAL(\"111\";2)"), 7.0);
        assert_eq!(erro("DECIMAL(\"2\";2)"), ErrorCode::Num);
    }

    #[test]
    fn trigonometria_extra() {
        assert!((num("COT(1)") - 0.6420926159343306).abs() < 1e-12);
        assert!((num("SEC(0)") - 1.0).abs() < 1e-12);
        assert!((num("CSC(1)") - 1.1883951057781212).abs() < 1e-12);
        assert!((num("ACOT(1)") - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert!((num("ACOT(0)") - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((num("ACOTH(2)") - 0.5493061443340548).abs() < 1e-12);
        assert!((num("ASINH(1)") - 0.881373587019543).abs() < 1e-12);
        assert!((num("ACOSH(1)")).abs() < 1e-12);
        assert!((num("ATANH(0.5)") - 0.5493061443340548).abs() < 1e-12);
        assert!((num("SECH(0)") - 1.0).abs() < 1e-12);
        assert!((num("COTH(1)") - 1.3130352854993315).abs() < 1e-12);
        assert!((num("CSCH(1)") - 0.8509181282393216).abs() < 1e-12);
        assert_eq!(erro("COT(0)"), ErrorCode::Div0);
        assert_eq!(erro("CSC(0)"), ErrorCode::Div0);
        assert_eq!(erro("ACOSH(0.5)"), ErrorCode::Num);
        assert_eq!(erro("ATANH(1)"), ErrorCode::Num);
        assert_eq!(erro("ACOTH(1)"), ErrorCode::Num);
    }

    #[test]
    fn teto_e_piso_por_multiplo() {
        assert_eq!(num("CEILING.MATH(24.3;5)"), 25.0);
        assert_eq!(num("CEILING.MATH(6.7)"), 7.0);
        assert_eq!(num("CEILING.MATH(-8.1;2)"), -8.0);
        assert_eq!(num("CEILING.MATH(-5.5;2;1)"), -6.0);
        assert_eq!(num("CEILING.MATH(-5.5;2;0)"), -4.0);
        assert_eq!(num("CEILING.PRECISE(4.3)"), 5.0);
        assert_eq!(num("CEILING.PRECISE(-4.3)"), -4.0);
        assert_eq!(num("CEILING.PRECISE(4.3;-2)"), 6.0);
        assert_eq!(num("ISO.CEILING(-4.3;2)"), -4.0);
        assert_eq!(num("FLOOR.MATH(24.3;5)"), 20.0);
        assert_eq!(num("FLOOR.MATH(6.7)"), 6.0);
        assert_eq!(num("FLOOR.MATH(-8.1;2)"), -10.0);
        assert_eq!(num("FLOOR.MATH(-5.5;2;-1)"), -4.0);
        assert_eq!(num("FLOOR.PRECISE(-3.2)"), -4.0);
        assert_eq!(num("FLOOR.PRECISE(3.2;-1)"), 3.0);
        assert_eq!(num("CEILING.MATH(0.3;0.1)"), 0.3);
        assert_eq!(num("CEILING.MATH(5;0)"), 0.0);
    }

    #[test]
    fn combinatoria_e_matrizes() {
        assert_eq!(num("COMBINA(4;3)"), 20.0);
        assert_eq!(num("COMBINA(10;3)"), 220.0);
        assert_eq!(erro("COMBINA(-1;2)"), ErrorCode::Num);
        assert_eq!(num("MULTINOMIAL(2;3;4)"), 1260.0);
        assert_eq!(num("MULTINOMIAL(1;1;1)"), 6.0);
        assert_eq!(num("SERIESSUM(2;0;1;A1:A3)"), 1.0 * 1.0 + 2.0 * 2.0 + 3.0 * 4.0);
        assert_eq!(num("SERIESSUM(2;1;2;A1:A3)"), 1.0 * 2.0 + 2.0 * 8.0 + 3.0 * 32.0);
        assert_eq!(num("SUMX2MY2({2;3;9;1;8;7;5};{6;5;11;7;5;4;4})"), -55.0);
        assert_eq!(num("SUMX2PY2({2;3;9;1;8;7;5};{6;5;11;7;5;4;4})"), 521.0);
        assert_eq!(num("SUMXMY2({2;3;9;1;8;7;5};{6;5;11;7;5;4;4})"), 79.0);
        assert_eq!(erro("SUMXMY2({1;2};{1;2;3})"), ErrorCode::Na);
        assert_eq!(num("PERCENTOF(A1:A2;A1:A3)"), 0.5);
    }

    #[test]
    fn identidade_e_matriz() {
        let mut s = Sheet::new("Plan1");
        s.set_cell_raw(0, 0, "=MUNIT(3)".into(), None);
        assert_eq!(s.get_value(0, 0), Value::Number(1.0));
        assert_eq!(s.get_value(1, 1), Value::Number(1.0));
        assert_eq!(s.get_value(0, 1), Value::Number(0.0));
        assert_eq!(s.get_value(2, 2), Value::Number(1.0));
        assert_eq!(erro("MUNIT(0)"), ErrorCode::Value);
    }
}
