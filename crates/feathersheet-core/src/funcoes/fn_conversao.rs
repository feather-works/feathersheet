//! Engenharia: CONVERTER (unidades de medida) e funções de Bessel (J e I).
//!
//! Os nomes das unidades são os do Excel (diferenciam maiúscula de minúscula).
//! Prefixos do SI (`k`, `m`, `u`…) valem para as unidades marcadas, e os
//! prefixos binários (`ki`, `Mi`…) para `bit` e `byte`.

use crate::celulas::values::Computed;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grupo {
    Massa,
    Distancia,
    Tempo,
    Pressao,
    Forca,
    Energia,
    Potencia,
    Magnetismo,
    Temperatura,
    Volume,
    Area,
    Velocidade,
    Informacao,
}

use Grupo::*;

/// (nome, grupo, fator para a unidade-base do grupo, potência do prefixo ou 0
/// quando a unidade não aceita prefixo). Bases: g, m, s, Pa, N, J, W, T, K,
/// m³, m², m/s, bit.
const UNIDADES: &[(&str, Grupo, f64, i32)] = &[
    ("g", Massa, 1.0, 1),
    ("sg", Massa, 14_593.902_937_206_5, 0),
    ("lbm", Massa, 453.592_37, 0),
    ("u", Massa, 1.660_538_782e-24, 1),
    ("ozm", Massa, 28.349_523_125, 0),
    ("grain", Massa, 0.064_798_91, 0),
    ("cwt", Massa, 45_359.237, 0),
    ("shweight", Massa, 45_359.237, 0),
    ("uk_cwt", Massa, 50_802.345_44, 0),
    ("lcwt", Massa, 50_802.345_44, 0),
    ("hweight", Massa, 50_802.345_44, 0),
    ("stone", Massa, 6_350.293_18, 0),
    ("ton", Massa, 907_184.74, 0),
    ("uk_ton", Massa, 1_016_046.908_8, 0),
    ("LTON", Massa, 1_016_046.908_8, 0),
    ("brton", Massa, 1_016_046.908_8, 0),
    ("m", Distancia, 1.0, 1),
    ("mi", Distancia, 1_609.344, 0),
    ("Nmi", Distancia, 1_852.0, 0),
    ("in", Distancia, 0.0254, 0),
    ("ft", Distancia, 0.3048, 0),
    ("yd", Distancia, 0.9144, 0),
    ("ang", Distancia, 1e-10, 1),
    ("ell", Distancia, 1.143, 0),
    ("ly", Distancia, 9.460_730_472_580_8e15, 0),
    ("parsec", Distancia, 3.085_677_581_467_19e16, 0),
    ("pc", Distancia, 3.085_677_581_467_19e16, 0),
    ("Pica", Distancia, 0.0254 / 72.0, 0),
    ("pica", Distancia, 0.0254 / 6.0, 0),
    ("survey_mi", Distancia, 1_609.347_218_694_437_5, 0),
    ("yr", Tempo, 31_557_600.0, 0),
    ("day", Tempo, 86_400.0, 0),
    ("d", Tempo, 86_400.0, 0),
    ("hr", Tempo, 3_600.0, 0),
    ("mn", Tempo, 60.0, 0),
    ("min", Tempo, 60.0, 0),
    ("sec", Tempo, 1.0, 1),
    ("s", Tempo, 1.0, 1),
    ("Pa", Pressao, 1.0, 1),
    ("p", Pressao, 1.0, 1),
    ("atm", Pressao, 101_325.0, 1),
    ("at", Pressao, 101_325.0, 1),
    ("mmHg", Pressao, 133.322_368_421_052_6, 1),
    ("psi", Pressao, 6_894.757_293_168_36, 1),
    ("Torr", Pressao, 133.322_368_421_052_6, 0),
    ("N", Forca, 1.0, 1),
    ("dyn", Forca, 1e-5, 1),
    ("dy", Forca, 1e-5, 1),
    ("lbf", Forca, 4.448_221_615_260_5, 0),
    ("pond", Forca, 0.009_806_65, 1),
    ("J", Energia, 1.0, 1),
    ("e", Energia, 1e-7, 1),
    ("c", Energia, 4.184, 1),
    ("cal", Energia, 4.1868, 1),
    ("eV", Energia, 1.602_176_487e-19, 1),
    ("ev", Energia, 1.602_176_487e-19, 1),
    ("HPh", Energia, 2_684_519.537_696_17, 0),
    ("hh", Energia, 2_684_519.537_696_17, 0),
    ("Wh", Energia, 3_600.0, 1),
    ("wh", Energia, 3_600.0, 1),
    ("flb", Energia, 1.355_817_948_331_400_4, 0),
    ("BTU", Energia, 1_055.055_852_62, 0),
    ("btu", Energia, 1_055.055_852_62, 0),
    ("W", Potencia, 1.0, 1),
    ("w", Potencia, 1.0, 1),
    ("HP", Potencia, 745.699_871_582_27, 0),
    ("h", Potencia, 745.699_871_582_27, 0),
    ("PS", Potencia, 735.498_75, 0),
    ("T", Magnetismo, 1.0, 1),
    ("ga", Magnetismo, 1e-4, 1),
    ("C", Temperatura, 1.0, 0),
    ("cel", Temperatura, 1.0, 0),
    ("F", Temperatura, 1.0, 0),
    ("fah", Temperatura, 1.0, 0),
    ("K", Temperatura, 1.0, 1),
    ("kel", Temperatura, 1.0, 1),
    ("Rank", Temperatura, 1.0, 0),
    ("Reau", Temperatura, 1.0, 0),
    ("tsp", Volume, 4.928_921_593_75e-6, 0),
    ("tspm", Volume, 5e-6, 0),
    ("tbs", Volume, 1.478_676_478_125e-5, 0),
    ("oz", Volume, 2.957_352_956_25e-5, 0),
    ("cup", Volume, 2.365_882_365e-4, 0),
    ("pt", Volume, 4.731_764_73e-4, 0),
    ("us_pt", Volume, 4.731_764_73e-4, 0),
    ("uk_pt", Volume, 5.682_612_5e-4, 0),
    ("qt", Volume, 9.463_529_46e-4, 0),
    ("uk_qt", Volume, 1.136_522_5e-3, 0),
    ("gal", Volume, 3.785_411_784e-3, 0),
    ("uk_gal", Volume, 4.546_09e-3, 0),
    ("l", Volume, 1e-3, 1),
    ("L", Volume, 1e-3, 1),
    ("lt", Volume, 1e-3, 1),
    ("m3", Volume, 1.0, 3),
    ("m^3", Volume, 1.0, 3),
    ("in3", Volume, 1.638_706_4e-5, 0),
    ("in^3", Volume, 1.638_706_4e-5, 0),
    ("ft3", Volume, 0.028_316_846_592, 0),
    ("ft^3", Volume, 0.028_316_846_592, 0),
    ("yd3", Volume, 0.764_554_857_984, 0),
    ("yd^3", Volume, 0.764_554_857_984, 0),
    ("barrel", Volume, 0.158_987_294_928, 0),
    ("bushel", Volume, 0.035_239_070_166_88, 0),
    ("GRT", Volume, 2.831_684_659_2, 0),
    ("regton", Volume, 2.831_684_659_2, 0),
    ("MTON", Volume, 1.132_673_863_68, 0),
    ("m2", Area, 1.0, 2),
    ("m^2", Area, 1.0, 2),
    ("ha", Area, 1e4, 0),
    ("ar", Area, 100.0, 0),
    ("uk_acre", Area, 4_046.856_422_4, 0),
    ("us_acre", Area, 4_046.872_61, 0),
    ("in2", Area, 6.451_6e-4, 0),
    ("in^2", Area, 6.451_6e-4, 0),
    ("ft2", Area, 0.092_903_04, 0),
    ("ft^2", Area, 0.092_903_04, 0),
    ("yd2", Area, 0.836_127_36, 0),
    ("yd^2", Area, 0.836_127_36, 0),
    ("mi2", Area, 2_589_988.110_336, 0),
    ("mi^2", Area, 2_589_988.110_336, 0),
    ("Morgen", Area, 2_500.0, 0),
    ("m/s", Velocidade, 1.0, 1),
    ("m/sec", Velocidade, 1.0, 1),
    ("m/h", Velocidade, 1.0 / 3_600.0, 1),
    ("m/hr", Velocidade, 1.0 / 3_600.0, 1),
    ("mph", Velocidade, 0.447_04, 0),
    ("kn", Velocidade, 1_852.0 / 3_600.0, 0),
    ("admkn", Velocidade, 0.514_773_333_333_333_3, 0),
    ("bit", Informacao, 1.0, 1),
    ("byte", Informacao, 8.0, 1),
];

const PREFIXOS_SI: &[(&str, f64)] = &[
    ("da", 1e1),
    ("Y", 1e24),
    ("Z", 1e21),
    ("E", 1e18),
    ("P", 1e15),
    ("T", 1e12),
    ("G", 1e9),
    ("M", 1e6),
    ("k", 1e3),
    ("h", 1e2),
    ("e", 1e1),
    ("d", 1e-1),
    ("c", 1e-2),
    ("m", 1e-3),
    ("u", 1e-6),
    ("n", 1e-9),
    ("p", 1e-12),
    ("f", 1e-15),
    ("a", 1e-18),
    ("z", 1e-21),
    ("y", 1e-24),
];

const PREFIXOS_BINARIOS: &[(&str, f64)] = &[
    ("ki", 1_024.0),
    ("Mi", 1_048_576.0),
    ("Gi", 1_073_741_824.0),
    ("Ti", 1_099_511_627_776.0),
    ("Pi", 1_125_899_906_842_624.0),
    ("Ei", 1_152_921_504_606_846_976.0),
    ("Zi", 1.180_591_620_717_411_3e21),
    ("Yi", 1.208_925_819_614_629_2e24),
];

/// Acha a unidade (com ou sem prefixo): grupo e fator para a base do grupo.
fn unidade(nome: &str) -> Option<(Grupo, f64)> {
    if let Some(&(_, g, f, _)) = UNIDADES.iter().find(|(n, ..)| *n == nome) {
        return Some((g, f));
    }
    let binarios = PREFIXOS_BINARIOS.iter().filter(|_| true);
    for &(p, mult) in binarios {
        if let Some(resto) = nome.strip_prefix(p) {
            if let Some(&(_, g, f, _)) = UNIDADES.iter().find(|(n, g, ..)| *n == resto && *g == Informacao) {
                return Some((g, f * mult));
            }
        }
    }
    for &(p, mult) in PREFIXOS_SI {
        if let Some(resto) = nome.strip_prefix(p) {
            if let Some(&(_, g, f, pot)) = UNIDADES.iter().find(|(n, ..)| *n == resto) {
                if pot > 0 {
                    return Some((g, f * mult.powi(pot)));
                }
            }
        }
    }
    None
}

/// Temperatura: valor em kelvin a partir de `x` na escala `nome` (e o inverso).
fn temperatura_para_kelvin(x: f64, nome: &str) -> f64 {
    match nome {
        "C" | "cel" => x + 273.15,
        "F" | "fah" => (x - 32.0) * 5.0 / 9.0 + 273.15,
        "Rank" => x * 5.0 / 9.0,
        "Reau" => x * 1.25 + 273.15,
        _ => x, // kelvin, com prefixo já aplicado abaixo
    }
}

fn kelvin_para_temperatura(k: f64, nome: &str) -> f64 {
    match nome {
        "C" | "cel" => k - 273.15,
        "F" | "fah" => (k - 273.15) * 9.0 / 5.0 + 32.0,
        "Rank" => k * 9.0 / 5.0,
        "Reau" => (k - 273.15) * 0.8,
        _ => k,
    }
}

/// `=CONVERTER(núm; de_unidade; para_unidade)` / `=CONVERT(number, from_unit, to_unit)`
pub fn fn_convert(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 3, || {
            let x = arg_num(args, ev, 0)?;
            let (de, para) = (arg_texto(args, ev, 1)?, arg_texto(args, ev, 2)?);
            let (g1, f1) = unidade(&de).ok_or(e_nd())?;
            let (g2, f2) = unidade(&para).ok_or(e_nd())?;
            if g1 != g2 {
                return Err(e_nd());
            }
            if g1 == Temperatura {
                // O prefixo só vale para kelvin; as outras escalas têm f = 1.
                let k = temperatura_para_kelvin(x * f1, &de);
                return numero(kelvin_para_temperatura(k, &para) / f2);
            }
            numero(x * f1 / f2)
    })
}

/// Número de pontos da regra do trapézio sobre o período inteiro (convergência
/// exponencial para integrando periódico).
fn pontos(x: f64, n: i64) -> usize {
    ((2.0 * (x.abs() + n as f64)) as usize + 64).min(200_000)
}

fn ordem(args: &[Node], ev: &Evaluator<'_>) -> Res<i64> {
    let n = arg_num(args, ev, 1)?.trunc();
    if !n.is_finite() || n < 0.0 || n > 1e6 {
        return Err(e_num());
    }
    Ok(n as i64)
}

/// `=BESSELJ(x; n)` / `=BESSELJ(x, n)`
pub fn fn_besselj(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let x = arg_num(args, ev, 0)?;
            let n = ordem(args, ev)?;
            let m = pontos(x, n);
            let soma: f64 = (0..m)
                .map(|k| {
                    let t = std::f64::consts::TAU * k as f64 / m as f64;
                    (n as f64 * t - x * t.sin()).cos()
                })
                .sum();
            numero(soma / m as f64)
    })
}

/// `=BESSELI(x; n)` / `=BESSELI(x, n)`
pub fn fn_besseli(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let x = arg_num(args, ev, 0)?;
            let n = ordem(args, ev)?;
            if x.abs() > 700.0 {
                return Err(e_num());
            }
            let m = pontos(x, n);
            let soma: f64 = (0..m)
                .map(|k| {
                    let t = std::f64::consts::TAU * k as f64 / m as f64;
                    (x * t.cos()).exp() * (n as f64 * t).cos()
                })
                .sum();
            numero(soma / m as f64)
    })
}

/// Regra de Simpson com `n` (par) intervalos.
fn simpson(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let h = (b - a) / n as f64;
    let meio: f64 = (1..n).map(|i| f(a + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 }).sum();
    (f(a) + meio + f(b)) * h / 3.0
}

/// Ordens 0 e 1 por integral e as outras pela recorrência (estável subindo,
/// tanto para K quanto para Y).
fn bessel_por_recorrencia(x: f64, n: i64, ordem0: f64, ordem1: f64, sinal: f64) -> f64 {
    let (mut anterior, mut atual) = (ordem0, ordem1);
    if n == 0 {
        return ordem0;
    }
    for k in 1..n {
        let proximo = sinal * anterior + 2.0 * k as f64 / x * atual;
        anterior = atual;
        atual = proximo;
    }
    atual
}

/// `=BESSELK(x; n)` / `=BESSELK(x, n)`
pub fn fn_besselk(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let x = arg_num(args, ev, 0)?;
            let n = ordem(args, ev)?;
            if x <= 0.0 {
                return Err(e_num());
            }
            // K_ν(x) = ∫₀^∞ e^(−x·cosh t)·cosh(νt) dt; depois de T o integrando some.
            let t = (1.0 + 60.0 / x).acosh();
            let k = |nu: f64| simpson(|u| (-x * u.cosh()).exp() * (nu * u).cosh(), 0.0, t, 4000);
            numero(bessel_por_recorrencia(x, n, k(0.0), k(1.0), 1.0))
    })
}

/// `=BESSELY(x; n)` / `=BESSELY(x, n)`
pub fn fn_bessely(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let x = arg_num(args, ev, 0)?;
            let n = ordem(args, ev)?;
            if x <= 0.0 {
                return Err(e_num());
            }
            // Y_ν(x) = (1/π)∫₀^π sen(x·sen θ − νθ) dθ − (1/π)∫₀^∞ (e^(νt) + e^(−νt)·cos νπ)·e^(−x·senh t) dt.
            let pi = std::f64::consts::PI;
            let m = pontos(x, 1).max(2000) * 2;
            let t = (100.0 / x).asinh() + 2.0;
            let y = |nu: f64| {
                let a = simpson(|th| (x * th.sin() - nu * th).sin(), 0.0, pi, m);
                let b = simpson(|u| ((nu * u).exp() + (-nu * u).exp() * (nu * pi).cos()) * (-x * u.sinh()).exp(), 0.0, t, 20_000);
                (a - b) / pi
            };
            numero(bessel_por_recorrencia(x, n, y(0.0), y(1.0), -1.0))
    })
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::ErrorCode;
    use crate::funcoes::fn_helpers::apoio_teste::*;

    #[test]
    fn converter_unidades() {
        close(calc("CONVERT(1;\"lbm\";\"kg\")"), 0.45359237);
        close(calc("CONVERTER(1;\"mi\";\"km\")"), 1.609344);
        close(calc("CONVERT(68;\"F\";\"C\")"), 20.0);
        close(calc("CONVERT(100;\"C\";\"K\")"), 373.15);
        close(calc("CONVERT(1;\"hr\";\"mn\")"), 60.0);
        close(calc("CONVERT(1;\"kbyte\";\"bit\")"), 8000.0);
        close(calc("CONVERT(1;\"kibyte\";\"byte\")"), 1024.0);
        close(calc("CONVERT(1;\"l\";\"ml\")"), 1000.0);
        close(calc("CONVERT(1;\"km2\";\"m2\")"), 1e6);
        is_err(calc("CONVERT(2,5;\"ft\";\"sec\")"), ErrorCode::Na);
        is_err(calc("CONVERT(1;\"xyz\";\"m\")"), ErrorCode::Na);
        is_err(calc("CONVERT(\"a\";\"m\";\"ft\")"), ErrorCode::Value);
    }

    #[test]
    fn bessel() {
        perto(calc("BESSELJ(1,9;2)"), 0.329925829);
        perto(calc("BESSELI(1,5;1)"), 0.981666428);
        close(calc("BESSELJ(0;0)"), 1.0);
        is_err(calc("BESSELJ(1;-1)"), ErrorCode::Num);
        is_err(calc("BESSELI(\"x\";1)"), ErrorCode::Value);
        perto(calc("BESSELK(1,5;1)"), 0.277387804);
        perto(calc("BESSELY(2,5;1)"), 0.145918138);
        perto(calc("BESSELY(1;0)"), 0.088256964);
        perto(calc("BESSELK(2;3)"), 0.647385390);
        perto(calc("BESSELY(3;2)"), -0.160400393);
        is_err(calc("BESSELK(0;1)"), ErrorCode::Num);
    }

    fn perto(v: crate::celulas::values::Value, esperado: f64) {
        match v {
            crate::celulas::values::Value::Number(n) => {
                assert!((n - esperado).abs() < 1e-6, "obtido {n}, esperado {esperado}")
            }
            o => panic!("esperava número, veio {o:?}"),
        }
    }
}
