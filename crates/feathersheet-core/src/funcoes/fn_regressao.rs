//! Regressão e previsão: PROJ.LIN, PROJ.LOG e a família PREVISÃO.ETS.
//!
//! PROJ.LIN resolve os mínimos quadrados pelo operador de varredura sobre
//! XᵀX: coluna colinear (pivô quase zero) fica de fora com coeficiente 0,
//! como o Excel faz. PREVISÃO.ETS é o Holt-Winters aditivo (erro, tendência e
//! sazonalidade aditivos, o "AAA" do Excel) com α, β e γ escolhidos pelo
//! menor erro quadrático de um passo. O Excel não publica a otimização dele:
//! os valores ficam perto, não idênticos.

use crate::celulas::values::Value;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_dist::norm_inv;
use crate::funcoes::fn_helpers::*;

// --- PROJ.LIN / PROJ.LOG ------------------------------------------------------

fn numeros_estritos(m: &crate::celulas::values::Matrix) -> Res<Vec<Vec<f64>>> {
    m.rows()
        .iter()
        .map(|l| {
            l.iter()
                .map(|v| match v {
                    Value::Number(n) => Ok(*n),
                    Value::Error(e) => Err(e.clone()),
                    _ => Err(e_valor()),
                })
                .collect()
        })
        .collect()
}

/// y (n) e as colunas de x (n × p), na orientação do y.
fn dados_regressao(args: &[Node], ev: &Evaluator<'_>) -> Res<(Vec<f64>, Vec<Vec<f64>>)> {
    let y = numeros_estritos(&arg_matriz(args, ev, 0)?)?;
    let (h, w) = (y.len(), y.first().map_or(0, Vec::len));
    let em_coluna = w == 1;
    let ys: Vec<f64> = y.iter().flatten().copied().collect();
    let n = ys.len();
    if !has(args, 1) {
        return Ok((ys, vec![(1..=n).map(|i| i as f64).collect()]));
    }
    let x = numeros_estritos(&arg_matriz(args, ev, 1)?)?;
    let (xh, xw) = (x.len(), x.first().map_or(0, Vec::len));
    let vars: Vec<Vec<f64>> = if xh * xw == n {
        vec![x.iter().flatten().copied().collect()]
    } else if em_coluna && xh == h {
        (0..xw).map(|c| x.iter().map(|l| l[c]).collect()).collect()
    } else if h == 1 && xw == w {
        x
    } else {
        return Err(e_ref());
    };
    Ok((ys, vars))
}

struct Ajuste {
    /// x1..xp e o intercepto no fim (0 sem constante).
    coef: Vec<f64>,
    erro_padrao: Vec<Option<f64>>,
    r2: Res<f64>,
    sey: Res<f64>,
    f: Res<f64>,
    gl: f64,
    ssreg: f64,
    ssresid: f64,
}

fn ajustar(y: &[f64], xs: &[Vec<f64>], constante: bool) -> Res<Ajuste> {
    let n = y.len();
    let p = xs.len();
    let q = p + constante as usize;
    if n == 0 || q == 0 {
        return Err(e_valor());
    }
    let coluna = |j: usize, i: usize| if j < p { xs[j][i] } else { 1.0 };
    // Matriz aumentada [[XᵀX, Xᵀy], [yᵀX, yᵀy]].
    let mut m = vec![vec![0.0; q + 1]; q + 1];
    for i in 0..n {
        for a in 0..q {
            for b in 0..q {
                m[a][b] += coluna(a, i) * coluna(b, i);
            }
            m[a][q] += coluna(a, i) * y[i];
        }
        m[q][q] += y[i] * y[i];
    }
    for a in 0..q {
        m[q][a] = m[a][q];
    }
    let diag: Vec<f64> = (0..q).map(|k| m[k][k]).collect();
    let mut mantida = vec![false; q];
    for k in 0..q {
        let d = m[k][k];
        if d <= 1e-12 * diag[k].abs().max(f64::MIN_POSITIVE) {
            continue;
        }
        mantida[k] = true;
        for j in 0..=q {
            m[k][j] /= d;
        }
        for i in 0..=q {
            if i != k {
                let b = m[i][k];
                for j in 0..=q {
                    m[i][j] -= b * m[k][j];
                }
                m[i][k] = -b / d;
            }
        }
        m[k][k] = 1.0 / d;
    }
    let coef: Vec<f64> = (0..q).map(|k| if mantida[k] { m[k][q] } else { 0.0 }).collect();
    let ssresid = m[q][q].max(0.0);
    let media = y.iter().sum::<f64>() / n as f64;
    let sstot: f64 = if constante { y.iter().map(|v| (v - media).powi(2)).sum() } else { y.iter().map(|v| v * v).sum() };
    let ssreg = sstot - ssresid;
    let mantidas = mantida.iter().filter(|&&k| k).count();
    let gl = n as f64 - mantidas as f64;
    let s2 = if gl > 0.0 { Ok(ssresid / gl) } else { Err(e_div0()) };
    let erro_padrao = (0..q)
        .map(|k| match (&s2, mantida[k]) {
            (Ok(s2), true) => Some((m[k][k] * s2).max(0.0).sqrt()),
            (Ok(_), false) => Some(0.0),
            _ => None,
        })
        .collect();
    let nu1 = mantidas as f64 - constante as u8 as f64;
    let f = match &s2 {
        Ok(s2) if *s2 > 0.0 && nu1 > 0.0 => Ok(ssreg / nu1 / s2),
        _ => Err(e_num()),
    };
    let r2 = if sstot > 0.0 { Ok(ssreg / sstot) } else { Err(e_num()) };
    Ok(Ajuste { coef, erro_padrao, r2, sey: s2.map(f64::sqrt), f, gl, ssreg, ssresid })
}

fn saida_regressao(a: Ajuste, p: usize, constante: bool, estatisticas: bool, exp: bool) -> Res<crate::celulas::values::Matrix> {
    let tr = |v: f64| if exp { v.exp() } else { v };
    let mut l0: Vec<Value> = (0..p).rev().map(|j| Value::Number(tr(a.coef[j]))).collect();
    l0.push(Value::Number(if constante { tr(a.coef[p]) } else if exp { 1.0 } else { 0.0 }));
    if !estatisticas {
        return montar_matriz(vec![l0]);
    }
    let largura = p + 1;
    let nd = || Value::Error(e_nd());
    let num = |r: &Res<f64>| match r {
        Ok(v) => Value::Number(*v),
        Err(e) => Value::Error(e.clone()),
    };
    let ep = |j: usize| a.erro_padrao[j].map_or(Value::Error(e_div0()), Value::Number);
    let mut l1: Vec<Value> = (0..p).rev().map(ep).collect();
    l1.push(if constante { ep(p) } else { nd() });
    let resto = |mut l: Vec<Value>| {
        l.resize(largura, nd());
        l
    };
    montar_matriz(vec![
        l0,
        l1,
        resto(vec![num(&a.r2), num(&a.sey)]),
        resto(vec![num(&a.f), Value::Number(a.gl)]),
        resto(vec![Value::Number(a.ssreg), Value::Number(a.ssresid)]),
    ])
}

fn regressao(args: &[Node], ev: &Evaluator<'_>, exp: bool) -> Res<crate::celulas::values::Matrix> {
    let (mut y, xs) = dados_regressao(args, ev)?;
    if exp {
        if y.iter().any(|v| *v <= 0.0) {
            return Err(e_num());
        }
        y.iter_mut().for_each(|v| *v = v.ln());
    }
    let constante = if has(args, 2) { arg_logico(args, ev, 2)? } else { true };
    let estatisticas = if has(args, 3) { arg_logico(args, ev, 3)? } else { false };
    let p = xs.len();
    saida_regressao(ajustar(&y, &xs, constante)?, p, constante, estatisticas, exp)
}

/// `=PROJ.LIN(y_conhecidos; [x_conhecidos]; [constante]; [estatísticas])` / `=LINEST(known_y's, [known_x's], [const], [stats])`
pub fn fn_linest(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 4, || regressao(args, ev, false))
}

/// `=PROJ.LOG(y_conhecidos; [x_conhecidos]; [constante]; [estatísticas])` / `=LOGEST(known_y's, [known_x's], [const], [stats])`
pub fn fn_logest(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 4, || regressao(args, ev, true))
}

// --- PREVISÃO.ETS ------------------------------------------------------------

use crate::celulas::values::Computed;

/// A série em passos iguais, depois de agregar repetidos e completar faltas.
struct Serie {
    y: Vec<f64>,
    passo: f64,
    ultimo: f64,
}

fn agregar_repetidos(vals: &mut [f64], modo: i64) -> f64 {
    let n = vals.len() as f64;
    match modo {
        2 | 3 => n,
        4 => vals.iter().copied().fold(f64::MIN, f64::max),
        5 => {
            vals.sort_by(f64::total_cmp);
            let m = vals.len() / 2;
            if vals.len() % 2 == 0 { (vals[m - 1] + vals[m]) / 2.0 } else { vals[m] }
        }
        6 => vals.iter().copied().fold(f64::MAX, f64::min),
        7 => vals.iter().sum(),
        _ => vals.iter().sum::<f64>() / n,
    }
}

fn preparar_serie(args: &[Node], ev: &Evaluator<'_>, i_val: usize, i_tl: usize, i_comp: usize, i_agr: usize) -> Res<Serie> {
    let valores: Vec<Value> = arg_matriz(args, ev, i_val)?.values().cloned().collect();
    let linha: Vec<Value> = arg_matriz(args, ev, i_tl)?.values().cloned().collect();
    if valores.len() != linha.len() {
        return Err(e_nd());
    }
    let completar = if has(args, i_comp) { arg_int(args, ev, i_comp)? } else { 1 };
    let agregacao = if has(args, i_agr) { arg_int(args, ev, i_agr)? } else { 1 };
    if !(0..=1).contains(&completar) || !(1..=7).contains(&agregacao) {
        return Err(e_num());
    }
    let mut pares: Vec<(f64, Option<f64>)> = Vec::with_capacity(valores.len());
    for (v, t) in valores.iter().zip(&linha) {
        let t = match t {
            Value::Number(n) => *n,
            Value::Error(e) => return Err(e.clone()),
            _ => return Err(e_valor()),
        };
        let v = match v {
            Value::Number(n) => Some(*n),
            Value::Empty => None,
            Value::Error(e) => return Err(e.clone()),
            _ => return Err(e_valor()),
        };
        pares.push((t, v));
    }
    pares.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Repetidos no tempo viram um ponto só.
    let mut pontos: Vec<(f64, Option<f64>)> = Vec::new();
    let mut i = 0;
    while i < pares.len() {
        let t = pares[i].0;
        let mut grupo = Vec::new();
        while i < pares.len() && pares[i].0 == t {
            grupo.extend(pares[i].1);
            i += 1;
        }
        pontos.push((t, (!grupo.is_empty()).then(|| agregar_repetidos(&mut grupo, agregacao))));
    }
    if pontos.len() < 3 {
        return Err(e_num());
    }
    let passo = pontos.windows(2).map(|w| w[1].0 - w[0].0).fold(f64::MAX, f64::min);
    if passo <= 0.0 {
        return Err(e_num());
    }
    let (t0, ultimo) = (pontos[0].0, pontos[pontos.len() - 1].0);
    let total = ((ultimo - t0) / passo).round() as usize + 1;
    if total > 1_000_000 {
        return Err(e_num());
    }
    let mut y: Vec<Option<f64>> = vec![None; total];
    for (t, v) in &pontos {
        let k = (t - t0) / passo;
        if (k - k.round()).abs() > 1e-6 {
            // Os tempos precisam andar em passos iguais.
            return Err(e_num());
        }
        y[k.round() as usize] = *v;
    }
    let y = completar_faltas(y, completar == 1).ok_or_else(e_num)?;
    Ok(Serie { y, passo, ultimo })
}

fn completar_faltas(y: Vec<Option<f64>>, interpolar: bool) -> Option<Vec<f64>> {
    if !interpolar {
        return Some(y.iter().map(|v| v.unwrap_or(0.0)).collect());
    }
    let conhecidos: Vec<usize> = (0..y.len()).filter(|&i| y[i].is_some()).collect();
    let (&primeiro, &ultimo) = (conhecidos.first()?, conhecidos.last()?);
    Some(
        (0..y.len())
            .map(|i| match y[i] {
                Some(v) => v,
                None if i < primeiro => y[primeiro].unwrap_or(0.0),
                None if i > ultimo => y[ultimo].unwrap_or(0.0),
                None => {
                    let a = conhecidos[conhecidos.partition_point(|&k| k < i) - 1];
                    let b = conhecidos[conhecidos.partition_point(|&k| k < i)];
                    let (va, vb) = (y[a].unwrap_or(0.0), y[b].unwrap_or(0.0));
                    va + (vb - va) * (i - a) as f64 / (b - a) as f64
                }
            })
            .collect(),
    )
}

/// Tamanho da estação pela autocorrelação da série sem a tendência linear:
/// o maior pico significativo até a metade da série. 0 = sem estação.
fn detectar_estacao(y: &[f64]) -> usize {
    let n = y.len();
    if n < 4 {
        return 0;
    }
    let xm = (n as f64 - 1.0) / 2.0;
    let ym = y.iter().sum::<f64>() / n as f64;
    let sxy: f64 = y.iter().enumerate().map(|(i, v)| (i as f64 - xm) * (v - ym)).sum();
    let sxx: f64 = (0..n).map(|i| (i as f64 - xm).powi(2)).sum();
    let inc = sxy / sxx;
    let r: Vec<f64> = y.iter().enumerate().map(|(i, v)| v - ym - inc * (i as f64 - xm)).collect();
    let var: f64 = r.iter().map(|v| v * v).sum();
    if var <= 1e-12 {
        return 0;
    }
    let acf = |k: usize| (0..n - k).map(|i| r[i] * r[i + k]).sum::<f64>() / var;
    let limite = 2.0 / (n as f64).sqrt();
    let max_lag = (n / 2).min(8760);
    let valores: Vec<f64> = (0..=max_lag + 1).map(|k| if k < n { acf(k) } else { 0.0 }).collect();
    (2..=max_lag)
        .filter(|&k| valores[k] > limite && valores[k] >= valores[k - 1] && valores[k] >= valores[k + 1])
        .max_by(|&a, &b| valores[a].total_cmp(&valores[b]))
        .unwrap_or(0)
}

struct Modelo {
    alfa: f64,
    beta: f64,
    gama: f64,
    estacao: usize,
    nivel: f64,
    tendencia: f64,
    sazonal: Vec<f64>,
    erros: Vec<f64>,
    previstos: Vec<f64>,
}

fn rodar(y: &[f64], l: usize, alfa: f64, beta: f64, gama: f64) -> Modelo {
    let n = y.len();
    let (mut nivel, mut tend, mut saz) = if l > 0 {
        let m1 = y[..l].iter().sum::<f64>() / l as f64;
        let m2 = y[l..2 * l].iter().sum::<f64>() / l as f64;
        let b = (m2 - m1) / l as f64;
        let l0 = m1 - b * (l as f64 + 1.0) / 2.0;
        (l0, b, (0..l).map(|i| y[i] - (l0 + (i as f64 + 1.0) * b)).collect::<Vec<_>>())
    } else {
        let b = y[1] - y[0];
        (y[0] - b, b, Vec::new())
    };
    let mut erros = Vec::with_capacity(n);
    let mut previstos = Vec::with_capacity(n);
    for (t, &v) in y.iter().enumerate() {
        let s = if l > 0 { saz[t % l] } else { 0.0 };
        let prev = nivel + tend + s;
        previstos.push(prev);
        erros.push(v - prev);
        let novo = alfa * (v - s) + (1.0 - alfa) * (nivel + tend);
        tend = beta * (novo - nivel) + (1.0 - beta) * tend;
        if l > 0 {
            saz[t % l] = gama * (v - novo) + (1.0 - gama) * s;
        }
        nivel = novo;
    }
    Modelo { alfa, beta, gama, estacao: l, nivel, tendencia: tend, sazonal: saz, erros, previstos }
}

fn sse(m: &Modelo) -> f64 {
    m.erros.iter().map(|e| e * e).sum()
}

fn otimizar(y: &[f64], l: usize) -> Modelo {
    let grade: Vec<f64> = (0..10).map(|i| 0.05 + 0.1 * i as f64).collect();
    let gamas: Vec<f64> = if l > 0 { grade.clone() } else { vec![0.0] };
    let mut melhor = (f64::MAX, 0.5, 0.1, 0.1);
    for &a in &grade {
        for &b in &grade {
            for &g in &gamas {
                let e = sse(&rodar(y, l, a, b, g));
                if e < melhor.0 {
                    melhor = (e, a, b, g);
                }
            }
        }
    }
    // Refina um parâmetro por vez, com passo caindo pela metade.
    let (mut e, mut p) = (melhor.0, [melhor.1, melhor.2, melhor.3]);
    let mut passo = 0.05;
    let livres = if l > 0 { 3 } else { 2 };
    for _ in 0..8 {
        for k in 0..livres {
            for sinal in [-1.0, 1.0] {
                let mut q = p;
                q[k] = (q[k] + sinal * passo).clamp(0.001, 0.999);
                let eq = sse(&rodar(y, l, q[0], q[1], q[2]));
                if eq < e {
                    e = eq;
                    p = q;
                }
            }
        }
        passo /= 2.0;
    }
    rodar(y, l, p[0], p[1], p[2])
}

fn estacao_pedida(args: &[Node], ev: &Evaluator<'_>, i: usize, y: &[f64]) -> Res<usize> {
    let s = if has(args, i) { arg_int(args, ev, i)? } else { 1 };
    let l = match s {
        1 => detectar_estacao(y),
        0 => 0,
        s if (2..=8760).contains(&s) => s as usize,
        _ => return Err(e_num()),
    };
    // Sem duas estações inteiras não há como começar a sazonalidade.
    Ok(if y.len() >= 2 * l { l } else { 0 })
}

/// Quantos passos depois do último ponto fica a data alvo.
fn passos_ate(args: &[Node], ev: &Evaluator<'_>, serie: &Serie) -> Res<f64> {
    let alvo = arg_num(args, ev, 0)?;
    let h = (alvo - serie.ultimo) / serie.passo;
    if h < 0.0 {
        return Err(e_num());
    }
    Ok(h)
}

fn prever(m: &Modelo, n: usize, h: f64) -> f64 {
    let k = h.ceil().max(1.0) as usize;
    let s = if m.estacao > 0 { m.sazonal[(n - 1 + k) % m.estacao] } else { 0.0 };
    m.nivel + h * m.tendencia + s
}

/// `=PREVISÃO.ETS(data_alvo; valores; linha_do_tempo; [sazonalidade]; [conclusão_dados]; [agregação])` / `=FORECAST.ETS(target_date, values, timeline, [seasonality], [data_completion], [aggregation])`
pub fn fn_forecast_ets(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 6, || {
        let serie = preparar_serie(args, ev, 1, 2, 4, 5)?;
        let l = estacao_pedida(args, ev, 3, &serie.y)?;
        let h = passos_ate(args, ev, &serie)?;
        numero(prever(&otimizar(&serie.y, l), serie.y.len(), h))
    })
}

/// `=PREVISÃO.ETS.CONFINT(data_alvo; valores; linha_do_tempo; [nível_confiança]; [sazonalidade]; [conclusão_dados]; [agregação])` / `=FORECAST.ETS.CONFINT(target_date, values, timeline, [confidence_level], [seasonality], [data_completion], [aggregation])`
pub fn fn_forecast_ets_confint(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 7, || {
        let serie = preparar_serie(args, ev, 1, 2, 5, 6)?;
        let nivel = if has(args, 3) { arg_num(args, ev, 3)? } else { 0.95 };
        if !(nivel > 0.0 && nivel < 1.0) {
            return Err(e_num());
        }
        let l = estacao_pedida(args, ev, 4, &serie.y)?;
        let h = passos_ate(args, ev, &serie)?;
        let m = otimizar(&serie.y, l);
        let sigma2 = sse(&m) / serie.y.len() as f64;
        let k = h.ceil().max(1.0) as usize;
        let soma: f64 = (1..k)
            .map(|j| {
                let c = m.alfa * (1.0 + j as f64 * m.beta) + if l > 0 && j % l == 0 { m.gama } else { 0.0 };
                c * c
            })
            .sum();
        numero(norm_inv((1.0 + nivel) / 2.0) * (sigma2 * (1.0 + soma)).sqrt())
    })
}

/// `=PREVISÃO.ETS.SAZONALIDADE(valores; linha_do_tempo; [conclusão_dados]; [agregação])` / `=FORECAST.ETS.SEASONALITY(values, timeline, [data_completion], [aggregation])`
pub fn fn_forecast_ets_seasonality(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 4, || {
        let serie = preparar_serie(args, ev, 0, 1, 2, 3)?;
        numero(detectar_estacao(&serie.y) as f64)
    })
}

/// `=PREVISÃO.ETS.STAT(valores; linha_do_tempo; tipo_estatística; [sazonalidade]; [conclusão_dados]; [agregação])` / `=FORECAST.ETS.STAT(values, timeline, statistic_type, [seasonality], [data_completion], [aggregation])`
pub fn fn_forecast_ets_stat(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 6, || {
        let serie = preparar_serie(args, ev, 0, 1, 4, 5)?;
        let tipo = arg_int(args, ev, 2)?;
        let l = estacao_pedida(args, ev, 3, &serie.y)?;
        let m = otimizar(&serie.y, l);
        let n = serie.y.len() as f64;
        let mae = m.erros.iter().map(|e| e.abs()).sum::<f64>() / n;
        match tipo {
            1 => numero(m.alfa),
            2 => numero(m.beta),
            3 => numero(m.gama),
            4 => {
                let ingenuo = serie.y.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (n - 1.0);
                if ingenuo == 0.0 { Err(e_div0()) } else { numero(mae / ingenuo) }
            }
            5 => numero(
                m.erros.iter().zip(&m.previstos).zip(&serie.y)
                    .map(|((e, p), y)| if y.abs() + p.abs() == 0.0 { 0.0 } else { 2.0 * e.abs() / (y.abs() + p.abs()) })
                    .sum::<f64>() / n,
            ),
            6 => numero(mae),
            7 => numero((sse(&m) / n).sqrt()),
            8 => numero(serie.passo),
            _ => Err(e_num()),
        }
    })
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    fn perto(v: &Value, esperado: f64, tol: f64) {
        match v {
            Value::Number(x) => assert!((x - esperado).abs() < tol, "obtido {x}, esperado {esperado}"),
            o => panic!("esperava número, veio {o:?}"),
        }
    }

    #[test]
    fn proj_lin_uma_e_duas_variaveis() {
        // y = 2x + 1
        let cel = [("A1", n(3.0)), ("A2", n(5.0)), ("A3", n(7.0)), ("B1", n(1.0)), ("B2", n(2.0)), ("B3", n(3.0))];
        let m = calc_com(&cel, "PROJ.LIN(A1:A3;B1:B3)");
        let l = &m.as_matrix().unwrap().rows()[0];
        perto(&l[0], 2.0, 1e-9);
        perto(&l[1], 1.0, 1e-9);
        // y = 1 + 2·x1 + 3·x2 (exemplo sem ruído): sai m2, m1, b.
        let cel2 = [
            ("A1", n(6.0)), ("A2", n(8.0)), ("A3", n(12.0)), ("A4", n(15.0)),
            ("B1", n(1.0)), ("B2", n(2.0)), ("B3", n(1.0)), ("B4", n(4.0)),
            ("C1", n(1.0)), ("C2", n(1.0)), ("C3", n(3.0)), ("C4", n(2.0)),
        ];
        let m = calc_com(&cel2, "LINEST(A1:A4;B1:C4;VERDADEIRO;VERDADEIRO)");
        let r = m.as_matrix().unwrap().rows();
        perto(&r[0][0], 3.0, 1e-9);
        perto(&r[0][1], 2.0, 1e-9);
        perto(&r[0][2], 1.0, 1e-9);
        perto(&r[2][0], 1.0, 1e-9);
        perto(&r[3][1], 1.0, 1e-9);
        is_err(r[2][2].clone(), ErrorCode::Na);
    }

    #[test]
    fn proj_lin_com_ruido_bate_com_o_excel() {
        // Exemplo da ajuda do Excel: y {1;9;5;7}, x {0;4;2;3} → inclinação 2, intercepto 1.
        let m = calc_com(&[], "PROJ.LIN({1;9;5;7};{0;4;2;3})");
        let l = &m.as_matrix().unwrap().rows()[0];
        perto(&l[0], 2.0, 1e-9);
        perto(&l[1], 1.0, 1e-9);
    }

    #[test]
    fn proj_log_cresce_exponencial() {
        // y = 3·2^x
        let m = calc_com(&[], "PROJ.LOG({6;12;24;48};{1;2;3;4})");
        let l = &m.as_matrix().unwrap().rows()[0];
        perto(&l[0], 2.0, 1e-9);
        perto(&l[1], 3.0, 1e-9);
        is_err(calc("PROJ.LOG({1;-1};{1;2})"), ErrorCode::Num);
    }

    #[test]
    fn previsao_ets_segue_tendencia_e_estacao() {
        // Reta perfeita: a previsão continua a reta.
        let v = calc("PREVISÃO.ETS(11;{2;4;6;8;10;12;14;16;18;20};{1;2;3;4;5;6;7;8;9;10})");
        perto(&v, 22.0, 0.5);
        // Estação de 4 repetida: detecta 4.
        let s = calc("PREVISÃO.ETS.SAZONALIDADE({1;5;3;9;1;5;3;9;1;5;3;9;1;5;3;9};{1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16})");
        perto(&s, 4.0, 1e-9);
        let p = calc("FORECAST.ETS(17;{1;5;3;9;1;5;3;9;1;5;3;9;1;5;3;9};{1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16})");
        perto(&p, 1.0, 0.5);
        perto(&calc("PREVISÃO.ETS.STAT({1;2;3;4;5};{1;2;3;4;5};8)"), 1.0, 1e-9);
        assert!(matches!(calc("PREVISÃO.ETS.CONFINT(12;{2;4;6;8;10;12;14;16;18;21};{1;2;3;4;5;6;7;8;9;10})"), Value::Number(x) if x > 0.0));
        is_err(calc("PREVISÃO.ETS(0;{1;2;3};{1;2;3})"), ErrorCode::Num);
    }
}
