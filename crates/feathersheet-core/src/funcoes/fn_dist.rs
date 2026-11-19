//! Distribuições de probabilidade e funções especiais: DIST.BETA.N, INV.BETA.N,
//! DISTR.BINOM.N, DIST.BINOM.INTERVALO, INV.BINOM, DIST.QUIQUA.CD, INV.QUIQUA,
//! INV.QUIQUA.CD, DIST.EXPON, DIST.F, DIST.F.CD, INV.F, INV.F.CD, GAMA, DIST.GAMA,
//! INV.GAMA, LNGAMA, LNGAMA.PRECISO, GAUSS, DIST.HIPERGEOM.N, DIST.LOGNORMAL.N,
//! INV.LOGNORMAL.N, DIST.BIN.NEG.N, INV.NORMP.N, PHI, DIST.POISSON, DIST.T.BC,
//! DIST.T.CD, INV.T, INV.T.BC, DIST.WEIBULL, INT.CONFIANÇA.NORM, INT.CONFIANÇA.T,
//! PERMUTAS, FUNERRO, FUNERRO.PRECISO, FUNERROCOMPL, FUNERROCOMPL.PRECISO e as
//! de compatibilidade (DISTBETA, DIST.HIPERGEOM, DIST.LOGNORMAL, DIST.BIN.NEG,
//! DIST.NORMP, DISTT…).
//!
//! A base numérica fica toda aqui, escrita para precisão perto de 1e-15:
//! - gama e ln gama: Lanczos (g = 7) até 10 e série de Stirling acima;
//! - gama incompleta regularizada: série para x < a + 1, fração contínua
//!   (Lentz) acima, com o fator comum calculado de forma estável para `a`
//!   grande;
//! - beta incompleta regularizada: fração contínua com troca de lado, sempre
//!   recebendo `x` e `1 − x` separados para não perder dígitos na cauda;
//! - erf/erfc: série de potências perto do zero e fração contínua na cauda;
//! - inversas: Newton protegido por intervalo (bisseção geométrica quando o
//!   passo sai do intervalo), sempre resolvendo na cauda menor.

use crate::celulas::values::{Computed, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{e_div0, e_num, numero, por_elemento, Entradas, Res};

use std::f64::consts::{FRAC_2_SQRT_PI, PI, SQRT_2};

// ---- constantes ----

/// ln(2π) / 2.
const LN_RAIZ_2PI: f64 = 0.918_938_533_204_672_8;
/// √(2π).
const RAIZ_2PI: f64 = 2.506_628_274_631_000_5;
/// 1/√π.
const INV_RAIZ_PI: f64 = 0.564_189_583_547_756_3;
/// Menor valor usado para evitar divisão por zero nas frações contínuas.
const MINUSCULO: f64 = 1e-300;
/// Critério de parada das séries e frações contínuas.
const EPS: f64 = 1e-16;
/// Teto de graus de liberdade aceito pelas distribuições qui-quadrado e F.
const GL_MAX: f64 = 1e10;
/// Na F o Excel recusa a partir de 10¹⁰.
const GL_MAX_F: f64 = 1e10 - 1.0;

// ---- gama ----

/// Coeficientes de Lanczos para g = 7, n = 9.
const LANCZOS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// Soma de Lanczos e o `t = x + g − 0,5` correspondente, para x ≥ 0,5.
fn lanczos(x: f64) -> (f64, f64) {
    let z = x - 1.0;
    let mut soma = LANCZOS[0];
    for (i, c) in LANCZOS.iter().enumerate().skip(1) {
        soma += c / (z + i as f64);
    }
    (soma, z + 7.5)
}

/// Correção de Stirling: ln Γ(z) − [(z − ½) ln z − z + ½ ln 2π], para z ≥ 10.
fn erro_stirling(z: f64) -> f64 {
    let z2 = 1.0 / (z * z);
    (1.0 / 12.0
        + z2 * (-1.0 / 360.0
            + z2 * (1.0 / 1260.0
                + z2 * (-1.0 / 1680.0
                    + z2 * (1.0 / 1188.0 + z2 * (-691.0 / 360_360.0 + z2 * (1.0 / 156.0)))))))
        / z
}

/// Fatorial exato (no limite do f64) de um inteiro de 0 a 170.
fn fatorial(n: u32) -> f64 {
    (2..=n).fold(1.0, |f, i| f * i as f64)
}

/// Γ(x) para qualquer real que não seja inteiro ≤ 0 (lá devolve NaN).
pub(crate) fn gama(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == x.floor() {
        if x <= 0.0 {
            return f64::NAN;
        }
        if x <= 171.0 {
            return fatorial(x as u32 - 1);
        }
        return f64::INFINITY;
    }
    if x < 0.5 {
        // Reflexão: Γ(x) Γ(1 − x) = π / sen(πx).
        return PI / ((PI * x).sin() * gama(1.0 - x));
    }
    if x > 171.7 {
        return f64::INFINITY;
    }
    let (soma, t) = lanczos(x);
    // t^(x−½) em duas metades para não estourar antes do e^−t.
    let meia = t.powf((x - 0.5) / 2.0);
    RAIZ_2PI * meia * (meia * (-t).exp()) * soma
}

/// ln |Γ(x)|.
pub(crate) fn ln_gama(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 && x == x.floor() {
        return f64::INFINITY;
    }
    if x < 0.5 {
        return (PI / (PI * x).sin().abs()).ln() - ln_gama(1.0 - x);
    }
    if x == x.floor() && x <= 30.0 {
        return fatorial(x as u32 - 1).ln();
    }
    if x >= 10.0 {
        return (x - 0.5) * x.ln() - x + LN_RAIZ_2PI + erro_stirling(x);
    }
    let (soma, t) = lanczos(x);
    LN_RAIZ_2PI + (x - 0.5) * t.ln() - t + soma.ln()
}

/// `d − ln(1 + d)` sem cancelamento perto de zero.
fn d_menos_ln1p(d: f64) -> f64 {
    if d.abs() < 0.5 {
        // d²/2 − d³/3 + d⁴/4 − …
        let mut termo = d * d;
        let mut soma = 0.0;
        let mut k = 2.0;
        loop {
            let parcela = termo / k;
            soma += parcela;
            if parcela.abs() <= soma.abs() * EPS {
                break;
            }
            termo *= -d;
            k += 1.0;
            if k > 200.0 {
                break;
            }
        }
        soma
    } else {
        d - d.ln_1p()
    }
}

/// ln B(a, b), estável também para argumentos grandes.
pub(crate) fn ln_beta(a: f64, b: f64) -> f64 {
    let (p, q) = if a < b { (a, b) } else { (b, a) };
    let s = p + q;
    if p >= 10.0 {
        LN_RAIZ_2PI - 0.5 * q.ln()
            + (p - 0.5) * (p / s).ln()
            + q * (-p / s).ln_1p()
            + erro_stirling(p)
            + erro_stirling(q)
            - erro_stirling(s)
    } else if q >= 10.0 {
        ln_gama(p) - (q - 0.5) * (p / q).ln_1p() - p * s.ln() + p + erro_stirling(q)
            - erro_stirling(s)
    } else {
        ln_gama(p) + ln_gama(q) - ln_gama(s)
    }
}

/// ln C(n, k) por meio da beta: C(n, k) = 1 / ((n + 1) B(k + 1, n − k + 1)).
fn ln_comb(n: f64, k: f64) -> f64 {
    if k == 0.0 || k == n {
        return 0.0;
    }
    -(n + 1.0).ln() - ln_beta(k + 1.0, n - k + 1.0)
}

/// C(n, k) para inteiros não negativos com k ≤ n.
fn comb(n: f64, k: f64) -> f64 {
    let k = k.min(n - k);
    if n <= 1030.0 {
        let mut c = 1.0;
        let mut i = 1.0;
        while i <= k {
            c = c * (n - k + i) / i;
            i += 1.0;
        }
        if c.is_finite() {
            // Ainda inteiro exato no f64: tira o resíduo das divisões.
            return if c < 9.0e15 { c.round() } else { c };
        }
    }
    ln_comb(n, k).exp()
}

// ---- gama incompleta ----

/// ln de xᵃ e⁻ˣ / Γ(a), estável para `a` grande.
fn ln_fator_gama(a: f64, x: f64) -> f64 {
    if a >= 10.0 {
        let d = (x - a) / a;
        -a * d_menos_ln1p(d) + 0.5 * (a / (2.0 * PI)).ln() - erro_stirling(a)
    } else {
        a * x.ln() - x - ln_gama(a)
    }
}

/// Série de P(a, x) sem o fator comum: Σ xⁿ / (a (a+1) … (a+n)).
fn serie_gama(a: f64, x: f64) -> f64 {
    let mut termo = 1.0 / a;
    let mut soma = termo;
    let mut n = a;
    for _ in 0..10_000_000 {
        n += 1.0;
        termo *= x / n;
        soma += termo;
        if termo.abs() <= soma.abs() * EPS {
            break;
        }
    }
    soma
}

/// Fração contínua de Q(a, x) sem o fator comum (Lentz).
fn fracao_gama(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / MINUSCULO;
    let mut d = 1.0 / b;
    let mut h = d;
    let mut i = 1.0;
    while i < 10_000_000.0 {
        let an = -i * (i - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < MINUSCULO {
            d = MINUSCULO;
        }
        c = b + an / c;
        if c.abs() < MINUSCULO {
            c = MINUSCULO;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() <= EPS {
            break;
        }
        i += 1.0;
    }
    h
}

/// Gama incompleta regularizada: (P(a, x), Q(a, x)); a cauda menor é a calculada.
pub(crate) fn gama_reg(a: f64, x: f64) -> (f64, f64) {
    if x <= 0.0 {
        return (0.0, 1.0);
    }
    if x.is_infinite() {
        return (1.0, 0.0);
    }
    let fator = ln_fator_gama(a, x).exp();
    if x < a + 1.0 {
        let p = (fator * serie_gama(a, x)).min(1.0);
        (p, 1.0 - p)
    } else {
        let q = (fator * fracao_gama(a, x)).min(1.0);
        (1.0 - q, q)
    }
}

/// Densidade da gama de escala 1: xᵃ⁻¹ e⁻ˣ / Γ(a).
fn densidade_gama(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return if x < 0.0 {
            0.0
        } else if a == 1.0 {
            1.0
        } else if a > 1.0 {
            0.0
        } else {
            f64::INFINITY
        };
    }
    ln_fator_gama(a, x).exp() / x
}

// ---- erf e normal ----

/// erf(x) pela série 2/√π e^−x² Σ 2ⁿ x²ⁿ⁺¹ / (1·3·…·(2n+1)), para |x| pequeno.
fn erf_serie(x: f64) -> f64 {
    let x2 = x * x;
    let mut termo = x;
    let mut soma = x;
    let mut impar = 1.0;
    for _ in 0..500 {
        impar += 2.0;
        termo *= 2.0 * x2 / impar;
        soma += termo;
        if termo.abs() <= soma.abs() * EPS {
            break;
        }
    }
    FRAC_2_SQRT_PI * (-x2).exp() * soma
}

/// erfc(x) para x ≥ 1,25 pela fração contínua de Q(½, x²).
fn erfc_cauda(x: f64) -> f64 {
    let x2 = x * x;
    if x2 > 750.0 {
        return 0.0;
    }
    x * (-x2).exp() * INV_RAIZ_PI * fracao_gama(0.5, x2)
}

/// Função erro.
pub(crate) fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ax = x.abs();
    let v = if ax < 1.25 {
        erf_serie(ax)
    } else {
        1.0 - erfc_cauda(ax)
    };
    v.copysign(x)
}

/// Função erro complementar, precisa também na cauda.
pub(crate) fn erfc(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 1.25 {
        1.0 - erf_serie(x)
    } else {
        erfc_cauda(x)
    }
}

/// Densidade da normal padrão.
pub(crate) fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / RAIZ_2PI
}

/// Acumulada da normal padrão, Φ(z).
pub(crate) fn norm_cdf(z: f64) -> f64 {
    if z < 0.0 {
        0.5 * erfc(-z / SQRT_2)
    } else {
        1.0 - 0.5 * erfc(z / SQRT_2)
    }
}

/// Inversa da normal padrão: aproximação racional por faixas e dois passos
/// de Halley sobre a erfc, o que leva ao limite do f64.
pub(crate) fn norm_inv(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return if p == 0.0 {
            f64::NEG_INFINITY
        } else if p == 1.0 {
            f64::INFINITY
        } else {
            f64::NAN
        };
    }
    if p > 0.5 {
        return -norm_inv(1.0 - p);
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let mut x = if p < 0.02425 {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    };
    // Refinamento de Halley: f(x) = Φ(x) − p, f' = φ(x), f'' = −x φ(x).
    for _ in 0..3 {
        let e = 0.5 * erfc(-x / SQRT_2) - p;
        let u = e * RAIZ_2PI * (0.5 * x * x).exp();
        let novo = x - u / (1.0 + 0.5 * x * u);
        if !novo.is_finite() {
            break;
        }
        let parar = (novo - x).abs() <= 1e-16 * novo.abs();
        x = novo;
        if parar {
            break;
        }
    }
    x
}

// ---- beta incompleta ----

/// ln de xᵃ yᵇ / B(a, b), com y = 1 − x dado à parte.
fn ln_fator_beta(a: f64, b: f64, x: f64, y: f64) -> f64 {
    if a >= 10.0 && b >= 10.0 {
        // Forma centrada na média, sem os cancelamentos de a ln x + b ln y − ln B.
        let s = a + b;
        let x0 = a / s;
        let y0 = b / s;
        let dx = (x - x0) / x0;
        let dy = (y - y0) / y0;
        -a * d_menos_ln1p(dx) - b * d_menos_ln1p(dy) + 0.5 * (a * b / s).ln()
            - LN_RAIZ_2PI
            - erro_stirling(a)
            - erro_stirling(b)
            + erro_stirling(s)
    } else {
        let lx = if x > 0.5 { (-y).ln_1p() } else { x.ln() };
        let ly = if y > 0.5 { (-x).ln_1p() } else { y.ln() };
        a * lx + b * ly - ln_beta(a, b)
    }
}

/// Fração contínua da beta incompleta (Lentz), sem o fator comum.
fn fracao_beta(a: f64, b: f64, x: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < MINUSCULO {
        d = MINUSCULO;
    }
    d = 1.0 / d;
    let mut h = d;
    let mut m = 1.0;
    while m < 10_000_000.0 {
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < MINUSCULO {
            d = MINUSCULO;
        }
        c = 1.0 + aa / c;
        if c.abs() < MINUSCULO {
            c = MINUSCULO;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < MINUSCULO {
            d = MINUSCULO;
        }
        c = 1.0 + aa / c;
        if c.abs() < MINUSCULO {
            c = MINUSCULO;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() <= EPS {
            break;
        }
        m += 1.0;
    }
    h
}

/// Beta incompleta regularizada: (Iₓ(a, b), 1 − Iₓ(a, b)), com y = 1 − x.
pub(crate) fn beta_reg(a: f64, b: f64, x: f64, y: f64) -> (f64, f64) {
    if x <= 0.0 {
        return (0.0, 1.0);
    }
    if y <= 0.0 {
        return (1.0, 0.0);
    }
    if x > (a + 1.0) / (a + b + 2.0) {
        let (q, p) = beta_reg_direta(b, a, y, x);
        return (p, q);
    }
    if a >= 1e5 && b < 10.0 && x > 0.5 {
        // Com `a` enorme e x perto de 1 a fração direta perde dígitos (os
        // denominadores ficam 1 − x); o complemento pelo lado de y é exato
        // enquanto o resultado não for minúsculo.
        let (q, p) = beta_reg_direta(b, a, y, x);
        if p >= 1e-3 {
            return (p, q);
        }
    }
    beta_reg_direta(a, b, x, y)
}

fn beta_reg_direta(a: f64, b: f64, x: f64, y: f64) -> (f64, f64) {
    let v = (ln_fator_beta(a, b, x, y).exp() * fracao_beta(a, b, x) / a).min(1.0);
    (v, 1.0 - v)
}

/// Densidade beta em [0, 1].
fn densidade_beta(a: f64, b: f64, x: f64, y: f64) -> f64 {
    if x <= 0.0 || y <= 0.0 {
        let (exp, borda) = if x <= 0.0 { (a, x) } else { (b, y) };
        if borda < 0.0 {
            return 0.0;
        }
        return if exp == 1.0 {
            (-ln_beta(a, b)).exp()
        } else if exp > 1.0 {
            0.0
        } else {
            f64::INFINITY
        };
    }
    ln_fator_beta(a, b, x, y).exp() / (x * y)
}

// ---- t, F e qui-quadrado ----

/// Cauda superior da t de Student, P(T > t).
pub(crate) fn t_sup(t: f64, gl: f64) -> f64 {
    if t < 0.0 {
        return 1.0 - t_sup(-t, gl);
    }
    if t.is_infinite() {
        return 0.0;
    }
    let t2 = t * t;
    let x = gl / (gl + t2);
    let y = t2 / (gl + t2);
    0.5 * beta_reg(gl / 2.0, 0.5, x, y).0
}

/// Densidade da t de Student.
fn t_pdf(t: f64, gl: f64) -> f64 {
    (-0.5 * (gl + 1.0) * (t * t / gl).ln_1p() - ln_beta(gl / 2.0, 0.5)).exp() / gl.sqrt()
}

/// (acumulada, cauda superior) da F.
fn f_caudas(f: f64, d1: f64, d2: f64) -> (f64, f64) {
    if f <= 0.0 {
        return (0.0, 1.0);
    }
    if f.is_infinite() {
        return (1.0, 0.0);
    }
    let den = d1 * f + d2;
    beta_reg(d1 / 2.0, d2 / 2.0, d1 * f / den, d2 / den)
}

/// Acumulada da distribuição F.
pub(crate) fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    f_caudas(x, d1, d2).0
}

/// Densidade da F.
fn f_pdf(f: f64, d1: f64, d2: f64) -> f64 {
    if f <= 0.0 {
        return if f < 0.0 {
            0.0
        } else if d1 == 2.0 {
            1.0
        } else if d1 > 2.0 {
            0.0
        } else {
            f64::INFINITY
        };
    }
    let den = d1 * f + d2;
    ln_fator_beta(d1 / 2.0, d2 / 2.0, d1 * f / den, d2 / den).exp() / f
}

// ---- inversão numérica ----

/// Resolve `acumulada(x) = p` (com `q = 1 − p` dado à parte) em `[0, teto]`,
/// usando a cauda menor como alvo. `caudas(x)` devolve (acumulada, superior)
/// e `densidade(x)` a derivada. `teto` pode ser infinito.
fn inverter(
    p: f64,
    q: f64,
    caudas: impl Fn(f64) -> (f64, f64),
    densidade: impl Fn(f64) -> f64,
    chute: f64,
    teto: f64,
) -> Option<f64> {
    if p <= 0.0 {
        return Some(0.0);
    }
    if q <= 0.0 {
        return if teto.is_finite() { Some(teto) } else { None };
    }
    let pela_superior = q < p;
    let g = |x: f64| {
        let (c, s) = caudas(x);
        if pela_superior {
            q - s
        } else {
            c - p
        }
    };
    let mut lo = 0.0_f64;
    let mut hi;
    if teto.is_finite() {
        hi = teto;
    } else {
        hi = if chute.is_finite() && chute > 0.0 { chute * 2.0 } else { 2.0 };
        hi = hi.max(1.0);
        let mut voltas = 0;
        while g(hi) < 0.0 {
            lo = hi;
            hi *= 4.0;
            voltas += 1;
            if voltas > 600 || !hi.is_finite() {
                return None;
            }
        }
    }
    let mut x = if chute > lo && chute < hi { chute } else { 0.5 * (lo + hi) };
    for _ in 0..2000 {
        let v = g(x);
        if v == 0.0 {
            return Some(x);
        }
        if v < 0.0 {
            lo = x;
        } else {
            hi = x;
        }
        let d = densidade(x);
        let mut novo = if d.is_finite() && d > 0.0 { x - v / d } else { f64::NAN };
        if !(novo > lo && novo < hi) {
            // Bisseção, geométrica quando o intervalo cobre várias ordens.
            let base = lo.max(hi * 1e-300);
            novo = if hi / base > 4.0 { (base * hi).sqrt() } else { 0.5 * (lo + hi) };
        }
        if (novo - x).abs() <= 2e-16 * novo.abs() || hi - lo <= 2e-16 * hi {
            return Some(novo);
        }
        x = novo;
    }
    Some(x)
}

/// Inversa da gama de escala 1: x tal que P(a, x) = p (q = 1 − p).
fn gama_inv(p: f64, q: f64, a: f64) -> Option<f64> {
    let z = norm_inv(p);
    let c = 1.0 / (9.0 * a);
    let mut chute = a * (1.0 - c + z * c.sqrt()).powi(3);
    if !(chute.is_finite() && chute > 0.0) {
        chute = ((p.ln() + ln_gama(a + 1.0)) / a).exp();
    }
    inverter(p, q, |x| gama_reg(a, x), |x| densidade_gama(a, x), chute, f64::INFINITY)
}

/// Inversa da beta em [0, 1].
fn beta_inv(p: f64, q: f64, a: f64, b: f64) -> Option<f64> {
    inverter(
        p,
        q,
        |x| beta_reg(a, b, x, 1.0 - x),
        |x| densidade_beta(a, b, x, 1.0 - x),
        a / (a + b),
        1.0,
    )
}

/// t ≥ 0 com P(T > t) = alvo (alvo ≤ ½).
fn t_inv_sup(alvo: f64, gl: f64) -> Option<f64> {
    if alvo >= 0.5 {
        return Some(0.0);
    }
    let chute = -norm_inv(alvo);
    inverter(
        1.0 - alvo,
        alvo,
        |t| {
            let s = t_sup(t, gl);
            (1.0 - s, s)
        },
        |t| t_pdf(t, gl),
        chute,
        f64::INFINITY,
    )
}

/// Inversa da F: x com acumulada p (q = 1 − p).
fn f_inv(p: f64, q: f64, d1: f64, d2: f64) -> Option<f64> {
    inverter(p, q, |x| f_caudas(x, d1, d2), |x| f_pdf(x, d1, d2), 1.0, f64::INFINITY)
}

// ---- probabilidades discretas ----

/// P(X = k) da binomial.
fn binom_pmf(k: f64, n: f64, p: f64) -> f64 {
    if p == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    if p == 1.0 {
        return if k == n { 1.0 } else { 0.0 };
    }
    let lp = k * p.ln();
    let lq = (n - k) * (-p).ln_1p();
    if n <= 1030.0 && lp > -700.0 && lq > -700.0 {
        let c = comb(n, k);
        if c.is_finite() {
            return c * p.powf(k) * (1.0 - p).powf(n - k);
        }
    }
    (ln_comb(n, k) + lp + lq).exp()
}

/// P(X ≤ k) da binomial, pela beta: I₁₋ₚ(n − k, k + 1).
fn binom_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k >= n {
        return 1.0;
    }
    if p == 0.0 {
        return 1.0;
    }
    if p == 1.0 {
        return 0.0;
    }
    beta_reg(n - k, k + 1.0, 1.0 - p, p).0
}

/// P(X = k) da hipergeométrica.
fn hipergeom_pmf(k: f64, n: f64, m: f64, pop: f64) -> f64 {
    let c1 = comb(m, k);
    let c2 = comb(pop - m, n - k);
    let c3 = comb(pop, n);
    let v = c1 / c3 * c2;
    if v.is_finite() && v > 0.0 && c3.is_finite() {
        return v;
    }
    (ln_comb(m, k) + ln_comb(pop - m, n - k) - ln_comb(pop, n)).exp()
}

/// P(X = k) da Poisson.
fn poisson_pmf(k: f64, media: f64) -> f64 {
    if media == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    if k <= 170.0 {
        let v = media.powf(k) * (-media).exp() / fatorial(k as u32);
        if v.is_finite() && v > 1e-290 {
            return v;
        }
    }
    (k * media.ln() - media - ln_gama(k + 1.0)).exp()
}

// ---- leitura de argumentos ----

/// Graus de liberdade truncados, de 1 até `teto` (inclusive).
fn graus(e: &Entradas<'_>, i: usize, teto: f64) -> Res<f64> {
    let g = e.num(i)?.trunc();
    if g < 1.0 || g > teto {
        return Err(e_num());
    }
    Ok(g)
}

/// Probabilidade em [0, 1].
fn probabilidade(e: &Entradas<'_>, i: usize) -> Res<f64> {
    let p = e.num(i)?;
    if !(0.0..=1.0).contains(&p) {
        return Err(e_num());
    }
    Ok(p)
}

/// Número estritamente positivo.
fn positivo(e: &Entradas<'_>, i: usize) -> Res<f64> {
    let v = e.num(i)?;
    if v <= 0.0 {
        return Err(e_num());
    }
    Ok(v)
}

/// Inteiro truncado não negativo.
fn natural(e: &Entradas<'_>, i: usize) -> Res<f64> {
    let v = e.num(i)?.trunc();
    if v < 0.0 {
        return Err(e_num());
    }
    Ok(v)
}

fn achou(v: Option<f64>) -> Res<Value> {
    match v {
        Some(x) => numero(x),
        None => Err(e_num()),
    }
}

// ---- beta ----

/// Conta comum de DIST.BETA.N e DISTBETA.
fn beta_dist(x: f64, a: f64, b: f64, acumulada: bool, ini: f64, fim: f64) -> Res<Value> {
    if a <= 0.0 || b <= 0.0 || ini >= fim || x < ini || x > fim {
        return Err(e_num());
    }
    let largura = fim - ini;
    let u = (x - ini) / largura;
    let v = (fim - x) / largura;
    if acumulada {
        numero(beta_reg(a, b, u, v).0)
    } else {
        numero(densidade_beta(a, b, u, v) / largura)
    }
}

/// `=DIST.BETA.N(x; alfa; beta; cumulativo; [A]; [B])` / `=BETA.DIST(...)` —
/// beta acumulada ou densidade no intervalo [A, B] (padrão [0, 1]).
pub fn fn_beta_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 6, |e| {
        let x = e.num(0)?;
        let a = e.num(1)?;
        let b = e.num(2)?;
        let acumulada = e.logico(3)?;
        let ini = e.num_ou(4, 0.0)?;
        let fim = e.num_ou(5, 1.0)?;
        beta_dist(x, a, b, acumulada, ini, fim)
    })
}

/// `=DISTBETA(x; alfa; beta; [A]; [B])` / `=BETADIST(...)` — beta acumulada
/// (compatibilidade).
pub fn fn_betadist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 5, |e| {
        let x = e.num(0)?;
        let a = e.num(1)?;
        let b = e.num(2)?;
        let ini = e.num_ou(3, 0.0)?;
        let fim = e.num_ou(4, 1.0)?;
        beta_dist(x, a, b, true, ini, fim)
    })
}

/// `=INV.BETA.N(probabilidade; alfa; beta; [A]; [B])` / `=BETA.INV(...)` —
/// inversa da beta acumulada. Também `BETA.ACUM.INV` / `BETAINV`.
pub fn fn_beta_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 5, |e| {
        let p = e.num(0)?;
        let a = e.num(1)?;
        let b = e.num(2)?;
        let ini = e.num_ou(3, 0.0)?;
        let fim = e.num_ou(4, 1.0)?;
        if p <= 0.0 || p > 1.0 || a <= 0.0 || b <= 0.0 || ini >= fim {
            return Err(e_num());
        }
        let x = beta_inv(p, 1.0 - p, a, b).ok_or_else(e_num)?;
        numero(ini + (fim - ini) * x)
    })
}

// ---- binomial e parentes ----

/// `=DISTR.BINOM.N(núm_s; tentativas; probabilidade_s; cumulativo)` /
/// `=BINOM.DIST(...)` — binomial; também `DISTRBINOM` / `BINOMDIST`.
pub fn fn_binom_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let k = e.num(0)?.trunc();
        let n = e.num(1)?.trunc();
        let p = probabilidade(e, 2)?;
        let acumulada = e.logico(3)?;
        if k < 0.0 || k > n {
            return Err(e_num());
        }
        if acumulada {
            numero(binom_cdf(k, n, p))
        } else {
            numero(binom_pmf(k, n, p))
        }
    })
}

/// `=DIST.BINOM.INTERVALO(tentativas; probabilidade_s; núm_s; [núm_s2])` /
/// `=BINOM.DIST.RANGE(...)` — probabilidade de s a s2 sucessos.
pub fn fn_binom_dist_range(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 4, |e| {
        let n = e.num(0)?.trunc();
        let p = probabilidade(e, 1)?;
        let s = e.num(2)?.trunc();
        let s2 = if e.tem(3) { e.num(3)?.trunc() } else { s };
        if n < 0.0 || s < 0.0 || s > n || s2 < s || s2 > n {
            return Err(e_num());
        }
        let mut soma = 0.0;
        let mut k = s;
        while k <= s2 {
            soma += binom_pmf(k, n, p);
            k += 1.0;
        }
        numero(soma.min(1.0))
    })
}

/// `=INV.BINOM(tentativas; probabilidade_s; alfa)` / `=BINOM.INV(...)` —
/// menor k com binomial acumulada ≥ alfa; também `CRIT.BINOM` / `CRITBINOM`.
pub fn fn_binom_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let n = e.num(0)?.trunc();
        let p = probabilidade(e, 1)?;
        let alfa = probabilidade(e, 2)?;
        if n < 0.0 {
            return Err(e_num());
        }
        let alvo = alfa * (1.0 - 4.0 * f64::EPSILON);
        let mut soma = 0.0;
        let mut k = 0.0;
        while k < n {
            soma += binom_pmf(k, n, p);
            if soma >= alvo {
                return numero(k);
            }
            k += 1.0;
        }
        numero(n)
    })
}

/// `=DIST.BIN.NEG.N(núm_f; núm_s; probabilidade_s; cumulativo)` /
/// `=NEGBINOM.DIST(...)` — binomial negativa (fracassos antes do s-ésimo sucesso).
pub fn fn_negbinom_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let f = e.num(0)?.trunc();
        let s = e.num(1)?.trunc();
        let p = probabilidade(e, 2)?;
        let acumulada = e.logico(3)?;
        negbinom(f, s, p, acumulada)
    })
}

/// `=DIST.BIN.NEG(núm_f; núm_s; probabilidade_s)` / `=NEGBINOMDIST(...)` —
/// probabilidade pontual da binomial negativa (compatibilidade).
pub fn fn_negbinomdist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let f = e.num(0)?.trunc();
        let s = e.num(1)?.trunc();
        let p = probabilidade(e, 2)?;
        negbinom(f, s, p, false)
    })
}

fn negbinom(f: f64, s: f64, p: f64, acumulada: bool) -> Res<Value> {
    if f < 0.0 || s < 1.0 {
        return Err(e_num());
    }
    if acumulada {
        if p == 0.0 {
            return numero(0.0);
        }
        if p == 1.0 {
            return numero(1.0);
        }
        return numero(beta_reg(s, f + 1.0, p, 1.0 - p).0);
    }
    if p == 0.0 {
        return numero(0.0);
    }
    if p == 1.0 {
        return numero(if f == 0.0 { 1.0 } else { 0.0 });
    }
    let lp = s * p.ln() + f * (-p).ln_1p();
    let total = f + s - 1.0;
    let c = comb(total, s - 1.0);
    let v = if c.is_finite() && lp > -700.0 {
        c * p.powf(s) * (1.0 - p).powf(f)
    } else {
        (ln_comb(total, s - 1.0) + lp).exp()
    };
    numero(v)
}

/// Conta comum da hipergeométrica.
fn hipergeom(k: f64, n: f64, m: f64, pop: f64, acumulada: bool) -> Res<Value> {
    if pop <= 0.0 || n <= 0.0 || n > pop || m <= 0.0 || m > pop {
        return Err(e_num());
    }
    let minimo = (n - (pop - m)).max(0.0);
    if k < 0.0 || k > n.min(m) || k < minimo {
        return Err(e_num());
    }
    if !acumulada {
        return numero(hipergeom_pmf(k, n, m, pop));
    }
    let mut soma = 0.0;
    let mut i = minimo;
    while i <= k {
        soma += hipergeom_pmf(i, n, m, pop);
        i += 1.0;
    }
    numero(soma.min(1.0))
}

/// `=DIST.HIPERGEOM.N(exemplo_s; exemplo_núm; população_s; núm_população; cumulativo)`
/// / `=HYPGEOM.DIST(...)` — hipergeométrica.
pub fn fn_hypgeom_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 5, 5, |e| {
        let k = e.num(0)?.trunc();
        let n = e.num(1)?.trunc();
        let m = e.num(2)?.trunc();
        let pop = e.num(3)?.trunc();
        let acumulada = e.logico(4)?;
        hipergeom(k, n, m, pop, acumulada)
    })
}

/// `=DIST.HIPERGEOM(exemplo_s; exemplo_núm; população_s; núm_população)` /
/// `=HYPGEOMDIST(...)` — probabilidade pontual (compatibilidade).
pub fn fn_hypgeomdist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let k = e.num(0)?.trunc();
        let n = e.num(1)?.trunc();
        let m = e.num(2)?.trunc();
        let pop = e.num(3)?.trunc();
        hipergeom(k, n, m, pop, false)
    })
}

/// `=DIST.POISSON(x; média; cumulativo)` / `=POISSON.DIST(...)` — Poisson;
/// também `POISSON`.
pub fn fn_poisson_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let k = e.num(0)?.trunc();
        let media = e.num(1)?;
        let acumulada = e.logico(2)?;
        if k < 0.0 || media < 0.0 {
            return Err(e_num());
        }
        if acumulada {
            if media == 0.0 {
                return numero(1.0);
            }
            numero(gama_reg(k + 1.0, media).1)
        } else {
            numero(poisson_pmf(k, media))
        }
    })
}

/// `=PERMUTAS(núm; núm_escolhido)` / `=PERMUTATIONA(...)` — arranjos com
/// repetição: núm elevado a núm_escolhido.
pub fn fn_permutationa(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let n = natural(e, 0)?;
        let k = natural(e, 1)?;
        numero(n.powf(k))
    })
}

// ---- qui-quadrado ----

/// `=DIST.QUIQUA.CD(x; graus_liberdade)` / `=CHISQ.DIST.RT(...)` — cauda
/// direita da qui-quadrado; também `DIST.QUI` / `CHIDIST`.
pub fn fn_chisq_dist_rt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let x = e.num(0)?;
        let gl = graus(e, 1, GL_MAX)?;
        if x < 0.0 {
            return Err(e_num());
        }
        numero(gama_reg(gl / 2.0, x / 2.0).1)
    })
}

/// `=INV.QUIQUA(probabilidade; graus_liberdade)` / `=CHISQ.INV(...)` —
/// inversa da cauda esquerda da qui-quadrado.
pub fn fn_chisq_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let p = probabilidade(e, 0)?;
        let gl = graus(e, 1, GL_MAX)?;
        achou(gama_inv(p, 1.0 - p, gl / 2.0).map(|x| 2.0 * x))
    })
}

/// `=INV.QUIQUA.CD(probabilidade; graus_liberdade)` / `=CHISQ.INV.RT(...)` —
/// inversa da cauda direita; também `INV.QUI` / `CHIINV`.
pub fn fn_chisq_inv_rt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let q = probabilidade(e, 0)?;
        let gl = graus(e, 1, GL_MAX)?;
        achou(gama_inv(1.0 - q, q, gl / 2.0).map(|x| 2.0 * x))
    })
}

// ---- exponencial, gama, Weibull ----

/// `=DIST.EXPON(x; lambda; cumulativo)` / `=EXPON.DIST(...)` — exponencial;
/// também `DISTEXPON` / `EXPONDIST`.
pub fn fn_expon_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let x = e.num(0)?;
        let lambda = e.num(1)?;
        let acumulada = e.logico(2)?;
        if x < 0.0 || lambda <= 0.0 {
            return Err(e_num());
        }
        if acumulada {
            numero(-(-lambda * x).exp_m1())
        } else {
            numero(lambda * (-lambda * x).exp())
        }
    })
}

/// `=GAMA(x)` / `=GAMMA(...)` — função gama.
pub fn fn_gamma(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(gama(e.num(0)?)))
}

/// `=LNGAMA(x)` / `=GAMMALN(...)` — ln da função gama (x > 0).
pub fn fn_gammaln(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(ln_gama(positivo(e, 0)?)))
}

/// `=LNGAMA.PRECISO(x)` / `=GAMMALN.PRECISE(...)` — o mesmo que LNGAMA.
pub fn fn_gammaln_precise(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    fn_gammaln(args, ev)
}

/// `=DIST.GAMA(x; alfa; beta; cumulativo)` / `=GAMMA.DIST(...)` — gama de
/// forma alfa e escala beta; também `DISTGAMA` / `GAMMADIST`.
pub fn fn_gamma_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let x = e.num(0)?;
        let a = e.num(1)?;
        let b = e.num(2)?;
        let acumulada = e.logico(3)?;
        if x < 0.0 || a <= 0.0 || b <= 0.0 {
            return Err(e_num());
        }
        if acumulada {
            numero(gama_reg(a, x / b).0)
        } else {
            numero(densidade_gama(a, x / b) / b)
        }
    })
}

/// `=INV.GAMA(probabilidade; alfa; beta)` / `=GAMMA.INV(...)` — inversa da
/// gama acumulada; também `INVGAMA` / `GAMMAINV`.
pub fn fn_gamma_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let p = probabilidade(e, 0)?;
        let a = positivo(e, 1)?;
        let b = positivo(e, 2)?;
        achou(gama_inv(p, 1.0 - p, a).map(|x| b * x))
    })
}

/// `=DIST.WEIBULL(x; alfa; beta; cumulativo)` / `=WEIBULL.DIST(...)` —
/// Weibull de forma alfa e escala beta; também `WEIBULL`.
pub fn fn_weibull_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let x = e.num(0)?;
        let a = e.num(1)?;
        let b = e.num(2)?;
        let acumulada = e.logico(3)?;
        if x < 0.0 || a <= 0.0 || b <= 0.0 {
            return Err(e_num());
        }
        let r = (x / b).powf(a);
        if acumulada {
            numero(-(-r).exp_m1())
        } else {
            numero(a / b * (x / b).powf(a - 1.0) * (-r).exp())
        }
    })
}

// ---- F ----

/// `=DIST.F(x; graus_liberdade1; graus_liberdade2; cumulativo)` / `=F.DIST(...)`
/// — F acumulada (cauda esquerda) ou densidade.
pub fn fn_f_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let x = e.num(0)?;
        let d1 = graus(e, 1, GL_MAX_F)?;
        let d2 = graus(e, 2, GL_MAX_F)?;
        let acumulada = e.logico(3)?;
        if x < 0.0 {
            return Err(e_num());
        }
        if acumulada {
            numero(f_caudas(x, d1, d2).0)
        } else {
            numero(f_pdf(x, d1, d2))
        }
    })
}

/// `=DIST.F.CD(x; graus_liberdade1; graus_liberdade2)` / `=F.DIST.RT(...)` —
/// cauda direita da F; também `DISTF` / `FDIST`.
pub fn fn_f_dist_rt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let x = e.num(0)?;
        let d1 = graus(e, 1, GL_MAX_F)?;
        let d2 = graus(e, 2, GL_MAX_F)?;
        if x < 0.0 {
            return Err(e_num());
        }
        numero(f_caudas(x, d1, d2).1)
    })
}

/// `=INV.F(probabilidade; graus_liberdade1; graus_liberdade2)` / `=F.INV(...)`
/// — inversa da F acumulada.
pub fn fn_f_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let p = probabilidade(e, 0)?;
        let d1 = graus(e, 1, GL_MAX_F)?;
        let d2 = graus(e, 2, GL_MAX_F)?;
        achou(f_inv(p, 1.0 - p, d1, d2))
    })
}

/// `=INV.F.CD(probabilidade; graus_liberdade1; graus_liberdade2)` /
/// `=F.INV.RT(...)` — inversa da cauda direita; também `INVF` / `FINV`.
pub fn fn_f_inv_rt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let q = probabilidade(e, 0)?;
        let d1 = graus(e, 1, GL_MAX_F)?;
        let d2 = graus(e, 2, GL_MAX_F)?;
        achou(f_inv(1.0 - q, q, d1, d2))
    })
}

// ---- normal e lognormal ----

/// `=GAUSS(z)` / `=GAUSS(...)` — probabilidade entre a média e z desvios: Φ(z) − ½.
pub fn fn_gauss(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(0.5 * erf(e.num(0)? / SQRT_2)))
}

/// `=PHI(x)` / `=PHI(...)` — densidade da normal padrão.
pub fn fn_phi(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(norm_pdf(e.num(0)?)))
}

/// `=DIST.NORMP(z)` / `=NORMSDIST(...)` — normal padrão acumulada (compatibilidade).
pub fn fn_normsdist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(norm_cdf(e.num(0)?)))
}

/// `=INV.NORMP.N(probabilidade)` / `=NORM.S.INV(...)` — inversa da normal
/// padrão; também `INV.NORMP` / `NORMSINV`.
pub fn fn_norm_s_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let p = e.num(0)?;
        if p <= 0.0 || p >= 1.0 {
            return Err(e_num());
        }
        numero(norm_inv(p))
    })
}

/// Conta comum da lognormal.
fn lognormal(x: f64, media: f64, desvio: f64, acumulada: bool) -> Res<Value> {
    if x <= 0.0 || desvio <= 0.0 {
        return Err(e_num());
    }
    let z = (x.ln() - media) / desvio;
    if acumulada {
        numero(norm_cdf(z))
    } else {
        numero(norm_pdf(z) / (x * desvio))
    }
}

/// `=DIST.LOGNORMAL.N(x; média; desv_padrão; cumulativo)` / `=LOGNORM.DIST(...)`
/// — lognormal acumulada ou densidade.
pub fn fn_lognorm_dist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let x = e.num(0)?;
        let media = e.num(1)?;
        let desvio = e.num(2)?;
        let acumulada = e.logico(3)?;
        lognormal(x, media, desvio, acumulada)
    })
}

/// `=DIST.LOGNORMAL(x; média; desv_padrão)` / `=LOGNORMDIST(...)` — lognormal
/// acumulada (compatibilidade).
pub fn fn_lognormdist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let x = e.num(0)?;
        let media = e.num(1)?;
        let desvio = e.num(2)?;
        lognormal(x, media, desvio, true)
    })
}

/// `=INV.LOGNORMAL.N(probabilidade; média; desv_padrão)` / `=LOGNORM.INV(...)`
/// — inversa da lognormal; também `INVLOG` / `LOGINV`.
pub fn fn_lognorm_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let p = e.num(0)?;
        let media = e.num(1)?;
        let desvio = e.num(2)?;
        if p <= 0.0 || p >= 1.0 || desvio <= 0.0 {
            return Err(e_num());
        }
        numero((media + desvio * norm_inv(p)).exp())
    })
}

/// `=INT.CONFIANÇA.NORM(alfa; desv_padrão; tamanho)` / `=CONFIDENCE.NORM(...)`
/// — meia largura do intervalo de confiança pela normal; também
/// `INT.CONFIANÇA` / `CONFIDENCE`.
pub fn fn_confidence_norm(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let alfa = e.num(0)?;
        let desvio = e.num(1)?;
        let n = e.num(2)?.trunc();
        if alfa <= 0.0 || alfa >= 1.0 || desvio <= 0.0 || n < 1.0 {
            return Err(e_num());
        }
        numero(-norm_inv(alfa / 2.0) * desvio / n.sqrt())
    })
}

/// `=INT.CONFIANÇA.T(alfa; desv_padrão; tamanho)` / `=CONFIDENCE.T(...)` —
/// meia largura do intervalo de confiança pela t de Student.
pub fn fn_confidence_t(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let alfa = e.num(0)?;
        let desvio = e.num(1)?;
        let n = e.num(2)?.trunc();
        if alfa <= 0.0 || alfa >= 1.0 || desvio <= 0.0 || n < 1.0 {
            return Err(e_num());
        }
        if n == 1.0 {
            return Err(e_div0());
        }
        let t = t_inv_sup(alfa / 2.0, n - 1.0).ok_or_else(e_num)?;
        numero(t * desvio / n.sqrt())
    })
}

// ---- t de Student ----

/// `=DIST.T.BC(x; graus_liberdade)` / `=T.DIST.2T(...)` — t bicaudal (x ≥ 0).
pub fn fn_t_dist_2t(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let x = e.num(0)?;
        let gl = graus(e, 1, f64::INFINITY)?;
        if x < 0.0 {
            return Err(e_num());
        }
        numero((2.0 * t_sup(x, gl)).min(1.0))
    })
}

/// `=DIST.T.CD(x; graus_liberdade)` / `=T.DIST.RT(...)` — cauda direita da t.
pub fn fn_t_dist_rt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let x = e.num(0)?;
        let gl = graus(e, 1, f64::INFINITY)?;
        numero(t_sup(x, gl))
    })
}

/// `=DISTT(x; graus_liberdade; caudas)` / `=TDIST(...)` — t de uma ou duas
/// caudas para x ≥ 0 (compatibilidade).
pub fn fn_tdist(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let x = e.num(0)?;
        let gl = graus(e, 1, f64::INFINITY)?;
        let caudas = e.num(2)?.trunc();
        if x < 0.0 || (caudas != 1.0 && caudas != 2.0) {
            return Err(e_num());
        }
        numero((caudas * t_sup(x, gl)).min(1.0))
    })
}

/// `=INV.T(probabilidade; graus_liberdade)` / `=T.INV(...)` — inversa da t
/// (cauda esquerda).
pub fn fn_t_inv(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let p = e.num(0)?;
        let gl = graus(e, 1, f64::INFINITY)?;
        if p <= 0.0 || p > 1.0 {
            return Err(e_num());
        }
        if p == 0.5 {
            return numero(0.0);
        }
        if p < 0.5 {
            achou(t_inv_sup(p, gl).map(|t| -t))
        } else {
            achou(t_inv_sup(1.0 - p, gl))
        }
    })
}

/// `=INV.T.BC(probabilidade; graus_liberdade)` / `=T.INV.2T(...)` — inversa
/// bicaudal da t; também `INVT` / `TINV`.
pub fn fn_t_inv_2t(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let p = e.num(0)?;
        let gl = graus(e, 1, f64::INFINITY)?;
        if p <= 0.0 || p > 1.0 {
            return Err(e_num());
        }
        achou(t_inv_sup(p / 2.0, gl))
    })
}

// ---- função erro ----

/// `=FUNERRO(limite_inferior; [limite_superior])` / `=ERF(...)` — função erro,
/// ou a integral entre os dois limites.
pub fn fn_erf(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let inf = e.num(0)?;
        if !e.tem(1) {
            return numero(erf(inf));
        }
        let sup = e.num(1)?;
        if inf >= 0.0 && sup >= 0.0 {
            numero(erfc(inf) - erfc(sup))
        } else if inf <= 0.0 && sup <= 0.0 {
            numero(erfc(-sup) - erfc(-inf))
        } else {
            numero(erf(sup) - erf(inf))
        }
    })
}

/// `=FUNERRO.PRECISO(x)` / `=ERF.PRECISE(...)` — função erro.
pub fn fn_erf_precise(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(erf(e.num(0)?)))
}

/// `=FUNERROCOMPL(x)` / `=ERFC(...)` — função erro complementar.
pub fn fn_erfc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| numero(erfc(e.num(0)?)))
}

/// `=FUNERROCOMPL.PRECISO(x)` / `=ERFC.PRECISE(...)` — o mesmo que FUNERROCOMPL.
pub fn fn_erfc_precise(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    fn_erfc(args, ev)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::planilha::sheet::Sheet;

    /// Valor numérico de uma fórmula (nomes em português).
    fn cal(formula: &str) -> Value {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, formula.into(), None);
        sheet.get_value(0, 0)
    }

    fn perto(formula: &str, esperado: f64, tol: f64) {
        match cal(formula) {
            Value::Number(v) => assert!(
                (v - esperado).abs() <= tol,
                "{formula}: deu {v}, esperado {esperado}"
            ),
            outro => panic!("{formula}: deu {outro:?}"),
        }
    }

    fn erro_num(formula: &str) {
        match cal(formula) {
            Value::Error(e) => assert_eq!(e.code, crate::celulas::values::ErrorCode::Num, "{formula}"),
            outro => panic!("{formula}: esperava #NÚM!, deu {outro:?}"),
        }
    }

    #[test]
    fn funcoes_especiais() {
        assert!((gama(5.0) - 24.0).abs() < 1e-12);
        assert!((gama(0.5) - PI.sqrt()).abs() < 1e-15);
        assert!((ln_gama(100.5) - 361.435_540_467_777_6).abs() < 1e-11);
        assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < 1e-15);
        assert!((erfc(5.0) / 1.537_459_794_428_034_8e-12 - 1.0).abs() < 1e-14);
        assert!((norm_cdf(-10.0) / 7.619_853_024_160_595e-24 - 1.0).abs() < 1e-13);
        assert!((norm_inv(0.975) - 1.959_963_984_540_054).abs() < 1e-15);
        assert!((norm_inv(1e-10) + 6.361_340_902_404_056).abs() < 1e-13);
        // Iₓ(a, a) em x = ½ vale ½ para qualquer a.
        assert!((beta_reg(1e7, 1e7, 0.5, 0.5).0 - 0.5).abs() < 1e-12);
        // Qui-quadrado com 2 graus é exponencial: Q(1, x) = e^−x.
        assert!((gama_reg(1.0, 30.0).1 / (-30.0f64).exp() - 1.0).abs() < 1e-14);
        // t com 2 graus tem forma fechada.
        let t: f64 = 1e4;
        let exato = 0.5 * (1.0 - t / (2.0 + t * t).sqrt());
        assert!((t_sup(t, 2.0) / exato - 1.0).abs() < 1e-7);
    }

    /// As da normal em fn_stats usavam uma erf aproximada (erro na 7ª casa); o GridCraft
    /// e o Excel dão estes valores.
    #[test]
    fn normal_com_precisao_do_excel() {
        perto("=DIST.NORM.N(1;0;1;VERDADEIRO)", 0.841_344_746_068_543, 1e-14);
        perto("=DIST.NORMP.N(1;VERDADEIRO)", 0.841_344_746_068_543, 1e-14);
        perto("=DISTNORM(1;0;1;VERDADEIRO)", 0.841_344_746_068_543, 1e-14);
        perto("=INV.NORM.N(0,95;0;1)", 1.644_853_626_951_472_7, 1e-12);
        perto("=INV.NORM(0,95;0;1)", 1.644_853_626_951_472_7, 1e-12);
    }

    #[test]
    fn beta_e_binomial() {
        perto("=DIST.BETA.N(2;8;10;VERDADEIRO;1;3)", 0.685_470_6, 1e-7);
        perto("=DIST.BETA.N(2;8;10;FALSO;1;3)", 1.483_764_6, 1e-7);
        perto("=INV.BETA.N(0,685470581;8;10;1;3)", 2.0, 1e-8);
        perto("=DISTBETA(2;8;10;1;3)", 0.685_470_6, 1e-7);
        perto("=DISTR.BINOM.N(6;10;0,5;FALSO)", 0.205_078_125, 1e-15);
        perto("=DIST.BINOM.INTERVALO(60;0,75;48)", 0.083_974_967, 1e-8);
        perto("=DIST.BINOM.INTERVALO(60;0,75;45;50)", 0.523_629_793, 1e-8);
        perto("=INV.BINOM(6;0,5;0,75)", 4.0, 0.0);
        // Menor k com acumulada ≥ alfa, inclusive na igualdade exata.
        perto("=INV.BINOM(5;0,5;0,5)", 2.0, 0.0);
        perto("=DIST.BIN.NEG.N(10;5;0,25;VERDADEIRO)", 0.313_514_1, 1e-7);
        perto("=DIST.BIN.NEG.N(10;5;0,25;FALSO)", 0.055_048_7, 1e-7);
        perto("=DIST.HIPERGEOM.N(1;4;8;20;VERDADEIRO)", 0.465_428_3, 1e-7);
        perto("=DIST.HIPERGEOM.N(1;4;8;20;FALSO)", 0.363_261_1, 1e-7);
        perto("=DIST.POISSON(2;5;VERDADEIRO)", 0.124_652_0, 1e-7);
        perto("=DIST.POISSON(2;5;FALSO)", 0.084_224_3, 1e-7);
        perto("=PERMUTATIONA(3;2)", 9.0, 0.0);
        erro_num("=DISTR.BINOM.N(11;10;0,5;FALSO)");
        erro_num("=INV.BETA.N(0;2;3)");
    }

    #[test]
    fn continuas() {
        perto("=DIST.QUIQUA.CD(18,307;10)", 0.050_000_589, 1e-9);
        perto("=INV.QUIQUA(0,93;1)", 3.283_020_287, 1e-8);
        perto("=INV.QUIQUA.CD(0,050001;10)", 18.306_973, 1e-6);
        perto("=DIST.EXPON(0,2;10;VERDADEIRO)", 0.864_664_717, 1e-9);
        perto("=DIST.F(15,2069;6;4;VERDADEIRO)", 0.99, 1e-6);
        perto("=DIST.F(15,2069;6;4;FALSO)", 0.001_223_8, 1e-7);
        perto("=DIST.F.CD(15,2069;6;4)", 0.01, 1e-6);
        perto("=INV.F(0,01;6;4)", 0.109_309_914, 1e-8);
        perto("=INV.F.CD(0,01;6;4)", 15.206_864_9, 1e-6);
        perto("=GAMA(2,5)", 1.329_340_388, 1e-9);
        perto("=GAMA(-3,75)", 0.267_866_129, 1e-9);
        erro_num("=GAMA(0)");
        perto("=LNGAMA(4)", 6f64.ln(), 1e-15);
        perto("=DIST.GAMA(10,00001131;9;2;VERDADEIRO)", 0.068_094, 1e-6);
        perto("=INV.GAMA(0,068094;9;2)", 10.000_011_2, 1e-6);
        perto("=GAUSS(2)", 0.477_249_868, 1e-9);
        perto("=PHI(0,75)", 0.301_137_432, 1e-9);
        perto("=INV.NORMP.N(0,908789)", 1.333_334_673, 1e-8);
        perto("=NORMSDIST(1,333333)", 0.908_788_726, 1e-9);
        perto("=DIST.LOGNORMAL.N(4;3,5;1,2;VERDADEIRO)", 0.039_083_556, 1e-9);
        perto("=INV.LOGNORMAL.N(0,039084;3,5;1,2)", 4.000_025_219, 1e-8);
        perto("=DIST.WEIBULL(105;20;100;VERDADEIRO)", 0.929_581_39, 1e-8);
        perto("=DIST.WEIBULL(105;20;100;FALSO)", 0.035_588_864, 1e-8);
    }

    #[test]
    fn t_confianca_e_erro() {
        perto("=DIST.T.BC(1,959999998;60)", 0.054_644_930, 1e-9);
        perto("=DIST.T.CD(1,959999998;60)", 0.027_322_465, 1e-9);
        perto("=TDIST(1,959999998;60;1)", 0.027_322_465, 1e-9);
        perto("=INV.T(0,75;2)", 0.816_496_581, 1e-9);
        perto("=INV.T.BC(0,546449;60)", 0.606_533_076, 1e-8);
        perto("=INT.CONFIANÇA.NORM(0,05;2,5;50)", 0.692_951_912, 1e-9);
        perto("=INT.CONFIANÇA.T(0,05;1;50)", 0.284_196_855, 1e-9);
        perto("=FUNERRO(0,745)", 0.707_928_920, 1e-9);
        perto("=FUNERRO(0;1)", 0.842_700_793, 1e-9);
        perto("=FUNERROCOMPL(1)", 0.157_299_207, 1e-9);
        erro_num("=TDIST(1;5;3)");
        match cal("=INT.CONFIANÇA.T(0,05;1;1)") {
            Value::Error(e) => assert_eq!(e.code, crate::celulas::values::ErrorCode::Div0),
            outro => panic!("esperava #DIV/0!, deu {outro:?}"),
        }
    }
}
