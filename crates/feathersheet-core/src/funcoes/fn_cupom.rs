//! Financeiras de título com cupom: CUPDIASINLIQ, CUPDIAS, CUPDIASPRÓX,
//! CUPDATAPRÓX, CUPNÚM, CUPDATAANT, PREÇO, LUCRO, DURAÇÃO, MDURAÇÃO,
//! JUROSACUM, AMORDEGRC, AMORLINC e as de período irregular (PREÇOPRIMINC,
//! LUCROPRIMINC, PREÇOÚLTINC, LUCROÚLTINC).
//!
//! As datas de cupom andam de `12/frequência` meses a partir de uma âncora
//! (o vencimento, ou a data de cupom irregular); se a âncora é o último dia
//! do mês, todas as datas também são. Os dias contam pela base: 0 = 30/360
//! americano, 1 = real/real, 2 = real/360, 3 = real/365, 4 = 30/360 europeu.

use crate::celulas::date_serial::serial_to_ymd;
use crate::celulas::values::Computed;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_date::{dias_360, yearfrac_core, ymd_to_serial};
use crate::funcoes::fn_helpers::*;

const MAX_SERIAL: f64 = 2_958_465.0;

fn data(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<i64> {
    let n = arg_num(args, ev, i)?.floor();
    if !(0.0..=MAX_SERIAL).contains(&n) {
        return Err(e_num());
    }
    Ok(n as i64)
}

fn base(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<i64> {
    let b = if has(args, i) { arg_int(args, ev, i)? } else { 0 };
    if !(0..=4).contains(&b) {
        return Err(e_num());
    }
    Ok(b)
}

fn frequencia(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<i64> {
    let f = arg_int(args, ev, i)?;
    if ![1, 2, 4].contains(&f) {
        return Err(e_num());
    }
    Ok(f)
}

fn dias_no_mes(y: i32, m: u32) -> u32 {
    let proximo = if m == 12 { ymd_to_serial(y + 1, 1, 1) } else { ymd_to_serial(y, m as i32 + 1, 1) };
    (proximo - ymd_to_serial(y, m as i32, 1)) as u32
}

fn fim_do_mes(serial: i64) -> bool {
    let (y, m, d) = serial_to_ymd(serial);
    d == dias_no_mes(y, m)
}

/// `serial` + `meses`; com `fim`, cai no último dia do mês.
fn somar_meses(serial: i64, meses: i64, fim: bool) -> i64 {
    let (y, m, d) = serial_to_ymd(serial);
    let total = y as i64 * 12 + (m as i64 - 1) + meses;
    let (ny, nm) = (total.div_euclid(12) as i32, (total.rem_euclid(12) + 1) as u32);
    let ultimo = dias_no_mes(ny, nm);
    let nd = if fim { ultimo } else { d.min(ultimo) };
    ymd_to_serial(ny, nm as i32, nd as i32) as i64
}

/// Dias entre duas datas pela base.
fn dias(a: i64, b: i64, base: i64) -> f64 {
    match base {
        0 => dias_360(a, b, false) as f64,
        4 => dias_360(a, b, true) as f64,
        _ => (b - a) as f64,
    }
}

/// Tamanho do período de cupom pela base.
fn tamanho(ini: i64, fim: i64, base: i64, f: i64) -> f64 {
    match base {
        1 => (fim - ini) as f64,
        3 => 365.0 / f as f64,
        _ => 360.0 / f as f64,
    }
}

/// O calendário de cupons entre a liquidação e o vencimento.
struct Cupons {
    anterior: i64,
    proximo: i64,
    quantos: i64,
    /// Dias do período, da liquidação ao fim e do início à liquidação.
    e: f64,
    dsc: f64,
    a: f64,
}

fn cupons(s: i64, m: i64, f: i64, b: i64) -> Res<Cupons> {
    if s >= m {
        return Err(e_num());
    }
    let passo = 12 / f;
    let fim = fim_do_mes(m);
    let mut k = 1;
    let (anterior, proximo) = loop {
        let d = somar_meses(m, -k * passo, fim);
        if d <= s {
            break (d, somar_meses(m, -(k - 1) * passo, fim));
        }
        k += 1;
    };
    let e = tamanho(anterior, proximo, b, f);
    let a = dias(anterior, s, b);
    let dsc = match b {
        0 => e - a,
        4 => dias_360(s, proximo, true) as f64,
        _ => (proximo - s) as f64,
    };
    Ok(Cupons { anterior, proximo, quantos: k, e, dsc, a })
}

/// Liquidação, vencimento, frequência e base nos argumentos `i..i+4`.
fn calendario(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Cupons> {
    let (s, m) = (data(args, ev, i)?, data(args, ev, i + 1)?);
    let f = frequencia(args, ev, i + 2)?;
    let b = base(args, ev, i + 3)?;
    cupons(s, m, f, b)
}

/// `=CUPDIASINLIQ(liquidação; vencimento; frequência; [base])` / `=COUPDAYBS(settlement, maturity, frequency, [basis])`
pub fn fn_coupdaybs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.a))
}

/// `=CUPDIAS(liquidação; vencimento; frequência; [base])` / `=COUPDAYS(settlement, maturity, frequency, [basis])`
pub fn fn_coupdays(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.e))
}

/// `=CUPDIASPRÓX(liquidação; vencimento; frequência; [base])` / `=COUPDAYSNC(settlement, maturity, frequency, [basis])`
pub fn fn_coupdaysnc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.dsc))
}

/// `=CUPDATAPRÓX(liquidação; vencimento; frequência; [base])` / `=COUPNCD(settlement, maturity, frequency, [basis])`
pub fn fn_coupncd(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.proximo as f64))
}

/// `=CUPNÚM(liquidação; vencimento; frequência; [base])` / `=COUPNUM(settlement, maturity, frequency, [basis])`
pub fn fn_coupnum(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.quantos as f64))
}

/// `=CUPDATAANT(liquidação; vencimento; frequência; [base])` / `=COUPPCD(settlement, maturity, frequency, [basis])`
pub fn fn_couppcd(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || numero(calendario(args, ev, 0)?.anterior as f64))
}

fn preco(c: &Cupons, taxa: f64, lucro: f64, resgate: f64, f: f64) -> f64 {
    let cupom = 100.0 * taxa / f;
    let fr = c.dsc / c.e;
    let acumulado = c.a / c.e * cupom;
    if c.quantos == 1 {
        return (resgate + cupom) / (1.0 + fr * lucro / f) - acumulado;
    }
    let v = 1.0 + lucro / f;
    let n = c.quantos as f64;
    let cupons: f64 = (1..=c.quantos).map(|k| cupom / v.powf(k as f64 - 1.0 + fr)).sum();
    resgate / v.powf(n - 1.0 + fr) + cupons - acumulado
}

/// A taxa que dá o preço, por bisseção (o preço cai quando a taxa sobe).
fn resolver_taxa(alvo: f64, preco_de: impl Fn(f64) -> f64, piso: f64) -> Res<f64> {
    let (mut lo, mut hi) = (piso, 1.0);
    while preco_de(hi) > alvo {
        hi *= 2.0;
        if hi > 1e6 {
            return Err(e_num());
        }
    }
    if !(preco_de(lo) >= alvo) {
        return Err(e_num());
    }
    for _ in 0..200 {
        let meio = (lo + hi) / 2.0;
        if preco_de(meio) > alvo { lo = meio } else { hi = meio }
    }
    Ok((lo + hi) / 2.0)
}

/// `=PREÇO(liquidação; vencimento; taxa; lucro; resgate; frequência; [base])` / `=PRICE(settlement, maturity, rate, yld, redemption, frequency, [basis])`
pub fn fn_price(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 6, 7, || {
        let (s, m) = (data(args, ev, 0)?, data(args, ev, 1)?);
        let (taxa, lucro, resgate) = (arg_num(args, ev, 2)?, arg_num(args, ev, 3)?, arg_num(args, ev, 4)?);
        let f = frequencia(args, ev, 5)?;
        let c = cupons(s, m, f, base(args, ev, 6)?)?;
        if taxa < 0.0 || lucro < 0.0 || resgate <= 0.0 {
            return Err(e_num());
        }
        numero(preco(&c, taxa, lucro, resgate, f as f64))
    })
}

/// `=LUCRO(liquidação; vencimento; taxa; preço; resgate; frequência; [base])` / `=YIELD(settlement, maturity, rate, pr, redemption, frequency, [basis])`
pub fn fn_yield(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 6, 7, || {
        let (s, m) = (data(args, ev, 0)?, data(args, ev, 1)?);
        let (taxa, pr, resgate) = (arg_num(args, ev, 2)?, arg_num(args, ev, 3)?, arg_num(args, ev, 4)?);
        let f = frequencia(args, ev, 5)?;
        let c = cupons(s, m, f, base(args, ev, 6)?)?;
        if taxa < 0.0 || pr <= 0.0 || resgate <= 0.0 {
            return Err(e_num());
        }
        let ff = f as f64;
        if c.quantos == 1 {
            let comprado = pr / 100.0 + c.a / c.e * taxa / ff;
            return numero((resgate / 100.0 + taxa / ff - comprado) / comprado * ff * c.e / c.dsc);
        }
        numero(resolver_taxa(pr, |y| preco(&c, taxa, y, resgate, ff), -ff + 1e-9)?)
    })
}

fn duracao(args: &[Node], ev: &Evaluator<'_>) -> Res<(f64, f64, f64)> {
    let (s, m) = (data(args, ev, 0)?, data(args, ev, 1)?);
    let (cupom, lucro) = (arg_num(args, ev, 2)?, arg_num(args, ev, 3)?);
    let f = frequencia(args, ev, 4)?;
    let c = cupons(s, m, f, base(args, ev, 5)?)?;
    if cupom < 0.0 || lucro < 0.0 {
        return Err(e_num());
    }
    let ff = f as f64;
    let (pg, v, fr) = (100.0 * cupom / ff, 1.0 + lucro / ff, c.dsc / c.e);
    let (mut num, mut den) = (0.0, 0.0);
    for k in 1..=c.quantos {
        let t = k as f64 - 1.0 + fr;
        let fluxo = pg + if k == c.quantos { 100.0 } else { 0.0 };
        num += t * fluxo / v.powf(t);
        den += fluxo / v.powf(t);
    }
    Ok((num / den / ff, lucro, ff))
}

/// `=DURAÇÃO(liquidação; vencimento; cupom; lucro; frequência; [base])` / `=DURATION(settlement, maturity, coupon, yld, frequency, [basis])`
pub fn fn_duration(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 5, 6, || numero(duracao(args, ev)?.0))
}

/// `=MDURAÇÃO(liquidação; vencimento; cupom; lucro; frequência; [base])` / `=MDURATION(settlement, maturity, coupon, yld, frequency, [basis])`
pub fn fn_mduration(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 5, 6, || {
        let (d, lucro, f) = duracao(args, ev)?;
        numero(d / (1.0 + lucro / f))
    })
}

/// `=JUROSACUM(emissão; primeiro_juro; liquidação; taxa; valor_nominal; frequência; [base]; [método_cálculo])` / `=ACCRINT(issue, first_interest, settlement, rate, par, frequency, [basis], [calc_method])`
pub fn fn_accrint(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 6, 8, || {
        let (emissao, primeiro, s) = (data(args, ev, 0)?, data(args, ev, 1)?, data(args, ev, 2)?);
        let (taxa, par) = (arg_num(args, ev, 3)?, arg_num(args, ev, 4)?);
        let f = frequencia(args, ev, 5)?;
        let b = base(args, ev, 6)?;
        let desde_emissao = if has(args, 7) { arg_logico(args, ev, 7)? } else { true };
        if taxa <= 0.0 || par <= 0.0 || emissao >= s {
            return Err(e_num());
        }
        let passo = 12 / f;
        let fim = fim_do_mes(primeiro);
        let cupom = |k: i64| somar_meses(primeiro, k * passo, fim);
        let ultimo_antes = |d: i64| {
            let mut k = 0;
            while cupom(k) > d {
                k -= 1;
            }
            while cupom(k + 1) <= d {
                k += 1;
            }
            k
        };
        let inicio = if desde_emissao || s <= primeiro { emissao } else { cupom(ultimo_antes(s)) };
        let mut k = ultimo_antes(inicio);
        let mut soma = 0.0;
        while cupom(k) < s {
            let (q0, q1) = (cupom(k), cupom(k + 1));
            let (a, z) = (inicio.max(q0), s.min(q1));
            if z > a {
                soma += dias(a, z, b) / tamanho(q0, q1, b, f);
            }
            k += 1;
        }
        numero(par * taxa / f as f64 * soma)
    })
}

/// Custo, compra, primeiro período, residual, período, taxa e base.
fn depreciacao(args: &[Node], ev: &Evaluator<'_>) -> Res<(f64, f64, f64, i64, f64)> {
    let custo = arg_num(args, ev, 0)?;
    let (compra, primeiro) = (data(args, ev, 1)?, data(args, ev, 2)?);
    let residual = arg_num(args, ev, 3)?;
    let periodo = arg_num(args, ev, 4)?.trunc();
    let taxa = arg_num(args, ev, 5)?;
    let b = base(args, ev, 6)?;
    if b == 2 || custo <= 0.0 || residual < 0.0 || residual > custo || periodo < 0.0 || taxa <= 0.0 || compra > primeiro {
        return Err(e_num());
    }
    let fracao = yearfrac_core(compra, primeiro, b)?;
    Ok((custo, residual, taxa, periodo as i64, fracao))
}

/// `=AMORLINC(custo; data_compra; primeiro_período; residual; período; taxa; [base])` / `=AMORLINC(cost, date_purchased, first_period, salvage, period, rate, [basis])`
pub fn fn_amorlinc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 6, 7, || {
        let (custo, residual, taxa, periodo, fracao) = depreciacao(args, ev)?;
        let cheia = custo * taxa;
        let primeira = fracao * cheia;
        let inteiros = ((custo - residual - primeira) / cheia).trunc() as i64;
        numero(if periodo == 0 {
            primeira
        } else if periodo <= inteiros {
            cheia
        } else if periodo == inteiros + 1 {
            (custo - residual - cheia * inteiros as f64 - primeira).max(0.0)
        } else {
            0.0
        })
    })
}

/// `=AMORDEGRC(custo; data_compra; primeiro_período; residual; período; taxa; [base])` / `=AMORDEGRC(cost, date_purchased, first_period, salvage, period, rate, [basis])`
pub fn fn_amordegrc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 6, 7, || {
        let (mut custo, residual, taxa, periodo, fracao) = depreciacao(args, ev)?;
        let vida = 1.0 / taxa;
        // O Excel recusa as vidas fracionárias entre 0 e 3 e entre 4 e 5.
        if (vida > 0.0 && vida < 1.0) || (vida > 1.0 && vida < 2.0) || (vida > 2.0 && vida < 3.0) || (vida > 4.0 && vida < 5.0) {
            return Err(e_num());
        }
        let coef = if vida < 3.0 { 1.0 } else if vida <= 4.0 { 1.5 } else if vida <= 6.0 { 2.0 } else { 2.5 };
        let taxa = taxa * coef;
        let mut parcela = (fracao * taxa * custo).round();
        custo -= parcela;
        let mut resto = custo - residual;
        for n in 0..periodo {
            parcela = (taxa * custo).round();
            resto -= parcela;
            if resto < 0.0 {
                return numero(if periodo - n <= 1 { (custo * 0.5).round() } else { 0.0 });
            }
            custo -= parcela;
        }
        numero(parcela)
    })
}

/// Um período de cupom irregular partido nos períodos normais que ele cobre:
/// (dias no período irregular, dias até a liquidação, dias da liquidação em
/// diante, tamanho normal) de cada um.
struct Quase {
    dc: f64,
    a: f64,
    dsc: f64,
    nl: f64,
}

fn quase_cupons(ini: i64, fim: i64, s: i64, ancora: i64, para_frente: bool, f: i64, b: i64) -> Vec<Quase> {
    let passo = 12 / f;
    let eom = fim_do_mes(ancora);
    let data_k = |k: i64| somar_meses(ancora, k * passo, eom);
    // Limites dos períodos normais que cobrem [ini, fim], em ordem.
    let mut limites = Vec::new();
    if para_frente {
        let mut k = 0;
        limites.push(data_k(0));
        while *limites.last().unwrap_or(&fim) < fim {
            k += 1;
            limites.push(data_k(k));
        }
    } else {
        let mut k = 0;
        limites.push(data_k(0));
        while *limites.last().unwrap_or(&ini) > ini {
            k -= 1;
            limites.push(data_k(k));
        }
        limites.reverse();
    }
    limites
        .windows(2)
        .map(|w| {
            let (q0, q1) = (w[0], w[1]);
            let comeco = ini.max(q0);
            let parte = |de: i64, ate: i64| if ate > de { dias(de, ate, b) } else { 0.0 };
            Quase {
                dc: parte(comeco, fim.min(q1)),
                a: parte(comeco, s.min(q1)),
                dsc: parte(s.max(comeco), fim.min(q1)),
                nl: tamanho(q0, q1, b, f),
            }
        })
        .collect()
}

/// Liquidação, vencimento, último juro, taxa, (lucro ou preço), resgate, frequência e base.
fn ultimo_irregular(args: &[Node], ev: &Evaluator<'_>) -> Res<(f64, f64, f64, f64, f64, f64)> {
    let (s, m, ultimo) = (data(args, ev, 0)?, data(args, ev, 1)?, data(args, ev, 2)?);
    let (taxa, x, resgate) = (arg_num(args, ev, 3)?, arg_num(args, ev, 4)?, arg_num(args, ev, 5)?);
    let f = frequencia(args, ev, 6)?;
    let b = base(args, ev, 7)?;
    if !(m > s && s > ultimo) || taxa < 0.0 || x < 0.0 || resgate <= 0.0 {
        return Err(e_num());
    }
    let q = quase_cupons(ultimo, m, s, ultimo, true, f, b);
    let soma = |g: fn(&Quase) -> f64| q.iter().map(|p| g(p) / p.nl).sum::<f64>();
    let cupom = 100.0 * taxa / f as f64;
    Ok((cupom, x, resgate, soma(|p| p.dc), soma(|p| p.a), soma(|p| p.dsc)))
}

/// `=PREÇOÚLTINC(liquidação; vencimento; último_juro; taxa; lucro; resgate; frequência; [base])` / `=ODDLPRICE(settlement, maturity, last_interest, rate, yld, redemption, frequency, [basis])`
pub fn fn_oddlprice(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 7, 8, || {
        let (cupom, lucro, resgate, dc, a, dsc) = ultimo_irregular(args, ev)?;
        let f = arg_int(args, ev, 6)? as f64;
        numero((resgate + cupom * dc) / (1.0 + dsc * lucro / f) - cupom * a)
    })
}

/// `=LUCROÚLTINC(liquidação; vencimento; último_juro; taxa; preço; resgate; frequência; [base])` / `=ODDLYIELD(settlement, maturity, last_interest, rate, pr, redemption, frequency, [basis])`
pub fn fn_oddlyield(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 7, 8, || {
        let (cupom, pr, resgate, dc, a, dsc) = ultimo_irregular(args, ev)?;
        if pr <= 0.0 {
            return Err(e_num());
        }
        let f = arg_int(args, ev, 6)? as f64;
        let pago = pr + cupom * a;
        numero(((resgate + cupom * dc) - pago) / pago * f / dsc)
    })
}

/// O preço com primeiro cupom irregular, pela taxa `lucro`.
struct PrimeiroIrregular {
    q: Vec<Quase>,
    /// Período normal em que cai a liquidação (índice em `q`).
    na_liquidacao: usize,
    regulares: i64,
    cupom: f64,
    resgate: f64,
    f: f64,
}

impl PrimeiroIrregular {
    fn preco(&self, lucro: f64) -> f64 {
        let v = 1.0 + lucro / self.f;
        let p = &self.q[self.na_liquidacao];
        let fr = p.dsc / p.nl;
        let nq = (self.q.len() - 1 - self.na_liquidacao) as f64;
        let n = self.regulares as f64;
        let dc: f64 = self.q.iter().map(|p| p.dc / p.nl).sum();
        let a: f64 = self.q.iter().map(|p| p.a / p.nl).sum();
        let regulares: f64 = (1..=self.regulares).map(|k| self.cupom / v.powf(k as f64 + nq + fr)).sum();
        self.resgate / v.powf(n + nq + fr) + self.cupom * dc / v.powf(nq + fr) + regulares - self.cupom * a
    }
}

fn primeiro_irregular(args: &[Node], ev: &Evaluator<'_>) -> Res<(PrimeiroIrregular, f64)> {
    let (s, m, emissao, primeiro) = (data(args, ev, 0)?, data(args, ev, 1)?, data(args, ev, 2)?, data(args, ev, 3)?);
    let (taxa, x, resgate) = (arg_num(args, ev, 4)?, arg_num(args, ev, 5)?, arg_num(args, ev, 6)?);
    let f = frequencia(args, ev, 7)?;
    let b = base(args, ev, 8)?;
    if !(m > primeiro && primeiro > s && s > emissao) || taxa < 0.0 || x < 0.0 || resgate <= 0.0 {
        return Err(e_num());
    }
    let q = quase_cupons(emissao, primeiro, s, primeiro, false, f, b);
    let passo = 12 / f;
    let eom = fim_do_mes(m);
    let mut regulares = 0;
    while somar_meses(primeiro, (regulares + 1) * passo, eom || fim_do_mes(primeiro)) <= m {
        regulares += 1;
    }
    // O período normal da liquidação é o último em que ainda há dias depois dela.
    let na_liquidacao = q.iter().position(|p| p.dsc > 0.0 && p.a < p.dc).unwrap_or(0);
    let cupom = 100.0 * taxa / f as f64;
    Ok((PrimeiroIrregular { q, na_liquidacao, regulares, cupom, resgate, f: f as f64 }, x))
}

/// `=PREÇOPRIMINC(liquidação; vencimento; emissão; primeiro_cupom; taxa; lucro; resgate; frequência; [base])` / `=ODDFPRICE(settlement, maturity, issue, first_coupon, rate, yld, redemption, frequency, [basis])`
pub fn fn_oddfprice(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 8, 9, || {
        let (p, lucro) = primeiro_irregular(args, ev)?;
        numero(p.preco(lucro))
    })
}

/// `=LUCROPRIMINC(liquidação; vencimento; emissão; primeiro_cupom; taxa; preço; resgate; frequência; [base])` / `=ODDFYIELD(settlement, maturity, issue, first_coupon, rate, pr, redemption, frequency, [basis])`
pub fn fn_oddfyield(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 8, 9, || {
        let (p, pr) = primeiro_irregular(args, ev)?;
        if pr <= 0.0 {
            return Err(e_num());
        }
        numero(resolver_taxa(pr, |y| p.preco(y), -p.f + 1e-9)?)
    })
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn perto(v: Value, esperado: f64, tol: f64) {
        match v {
            Value::Number(n) => assert!((n - esperado).abs() < tol, "obtido {n}, esperado {esperado}"),
            o => panic!("esperava número, veio {o:?}"),
        }
    }

    // Datas como DATA(a;m;d) para o teste ler como a ajuda do Excel.
    #[test]
    fn calendario_de_cupons() {
        let args = "DATA(2011;1;25);DATA(2011;11;15);2;1";
        perto(calc(&format!("CUPDIASINLIQ({args})")), 71.0, 1e-9);
        perto(calc(&format!("CUPDIAS({args})")), 181.0, 1e-9);
        perto(calc(&format!("CUPDIASPRÓX({args})")), 110.0, 1e-9);
        perto(calc(&format!("CUPDATAPRÓX({args})")), 40678.0, 1e-9);
        perto(calc(&format!("CUPNÚM({args})")), 2.0, 1e-9);
        perto(calc(&format!("CUPDATAANT({args})")), 40497.0, 1e-9);
        is_err(calc("COUPNUM(DATA(2012;1;1);DATA(2011;1;1);2)"), ErrorCode::Num);
        is_err(calc("COUPNUM(DATA(2011;1;1);DATA(2012;1;1);3)"), ErrorCode::Num);
    }

    #[test]
    fn preco_lucro_e_duracao() {
        perto(calc("PREÇO(DATA(2008;2;15);DATA(2017;11;15);5,75%;6,5%;100;2;0)"), 94.63436, 1e-4);
        perto(calc("LUCRO(DATA(2008;2;15);DATA(2016;11;15);5,75%;95,04287;100;2;0)"), 0.065, 1e-5);
        perto(calc("DURAÇÃO(DATA(2018;7;1);DATA(2048;1;1);8%;9%;2;1)"), 10.9191453, 1e-6);
        perto(calc("MDURATION(DATA(2008;1;1);DATA(2016;1;1);8%;9%;2;1)"), 5.73567, 1e-5);
    }

    #[test]
    fn juros_acumulados_e_amortizacao() {
        perto(calc("JUROSACUM(DATA(2008;3;1);DATA(2008;8;31);DATA(2008;5;1);10%;1000;2;0)"), 16.666667, 1e-5);
        perto(calc("AMORLINC(2400;DATA(2008;8;19);DATA(2008;12;31);300;1;15%;1)"), 360.0, 1e-9);
        perto(calc("AMORDEGRC(2400;DATA(2008;8;19);DATA(2008;12;31);300;1;15%;1)"), 776.0, 1e-9);
    }

    #[test]
    fn periodos_irregulares() {
        perto(calc("PREÇOÚLTINC(DATA(2008;2;7);DATA(2008;6;15);DATA(2007;10;15);3,75%;4,05%;100;2;0)"), 99.87829, 1e-4);
        perto(calc("LUCROÚLTINC(DATA(2008;4;20);DATA(2008;6;15);DATA(2007;12;24);3,75%;99,875;100;2;0)"), 0.045192, 1e-5);
        perto(calc("PREÇOPRIMINC(DATA(2008;11;11);DATA(2021;3;1);DATA(2008;10;15);DATA(2009;3;1);7,85%;6,25%;100;2;1)"), 113.597717, 1e-3);
        perto(calc("LUCROPRIMINC(DATA(2008;11;11);DATA(2021;3;1);DATA(2008;10;15);DATA(2009;3;1);5,75%;84,5;100;2;0)"), 0.0772, 1e-4);
    }
}
