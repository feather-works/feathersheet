//! Funções de data e hora em Rust: HOJE, AGORA, DATA, ANO, MÊS, DIA, DIA.DA.SEMANA, NÚM.SEMANA, DATADIF, DATAM, FIMMÊS, DIATRABALHO, DIATRABALHOTOTAL,
//! HORA, MINUTO, SEGUNDO, TEMPO, DATA.VALOR, VALOR.TEMPO, DIAS, DIAS360, NÚMSEMANAISO, FRAÇÃOANO, DIATRABALHOTOTAL.INTL, DIATRABALHO.INTL.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::formula::ast::Node;
use crate::celulas::date_serial::{timestamp_to_serial, serial_to_ymd};
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{
    arity, e_num, e_valor, err_num, has, numero, numeros_de, por_elemento, Res,
};
use crate::celulas::values::{CellError, Computed, ErrorCode, Value};

/// Converte (Ano, Mês, Dia) em serial numérico do Excel/FeatherSheet.
pub fn ymd_to_serial(y: i32, m: i32, d: i32) -> f64 {
    let (year, month) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let yoe = (year - era * 400) as i64;
    let doy = (153 * (month - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as i64;
    let days = era as i64 * 146097 + doe - 719468;
    let ms = (days as f64) * 86_400_000.0;
    timestamp_to_serial(ms)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn date_arg(node: &Node, ev: &Evaluator<'_>) -> Result<f64, CellError> {
    serial_do_valor(&ev.value(node))
}

/// A regra do [`date_arg`] para um valor já lido (item de matriz).
fn serial_do_valor(val: &Value) -> Result<f64, CellError> {
    match val {
        Value::Number(n) => Ok(*n),
        Value::Text(s) => s.trim().parse::<f64>().map_err(|_| CellError::new(ErrorCode::Value)),
        Value::Error(e) => Err(e.clone()),
        _ => Err(CellError::new(ErrorCode::Value)),
    }
}

/// Ano, mês ou dia de cada data: `MÊS(A2:A9)` dá a matriz dos meses
/// (dentro de SOMARPRODUTO, FILTRO…), uma data só dá um número.
fn parte_da_data(args: &[Node], ev: &Evaluator<'_>, max: usize, parte: fn(f64) -> f64) -> Computed {
    por_elemento(args, ev, 1, max, |e| {
        let serial = serial_do_valor(e.valor(0))?;
        if serial < 0.0 { return Err(CellError::new(ErrorCode::Num)); }
        Ok(Value::Number(parte(serial)))
    })
}

/// O dia da semana de um serial, 0 = domingo — o `getUTCDay` do TypeScript.
///
/// O serial 0 é 30/12/1899, um sábado (6). A conta anterior somava 1 em vez
/// de 6 e errava dois dias: 1º/03/2026, um domingo, saía terça — e com ela
/// `DIA.DA.SEMANA`, `NÚMSEMANA`, `DIATRABALHO` e `DIATRABALHOTOTAL`.
fn dia_da_semana(serial: i64) -> i64 {
    (serial + 6).rem_euclid(7)
}

/// O fuso de quem usa, em minutos a leste de UTC (Brasília: -180). Quem
/// hospeda o motor informa por [`definir_fuso_minutos`]: o `std` não sabe o
/// fuso do sistema e o `sheet-core` não tem dependência. Sem isso, `HOJE()`
/// e `AGORA()` seguiam o relógio de UTC, e às 21h de Brasília já era
/// "amanhã" — o TypeScript usa a hora local.
static FUSO_MINUTOS: AtomicI32 = AtomicI32::new(0);

/// Ver [`FUSO_MINUTOS`]. O TypeScript entrega `-new Date().getTimezoneOffset()`.
pub fn definir_fuso_minutos(minutos: i32) {
    FUSO_MINUTOS.store(minutos, Ordering::Relaxed);
}

/// A hora de agora no fuso de quem usa, em milissegundos.
fn agora_local_ms() -> f64 {
    crate::celulas::relogio::agora_ms() + FUSO_MINUTOS.load(Ordering::Relaxed) as f64 * 60_000.0
}

/// `=HOJE()` / `=TODAY()`
pub fn fn_today(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    let ms = agora_local_ms();
    let serial = timestamp_to_serial(ms);
    let (y, m, d) = serial_to_ymd(serial as i64);
    let today_serial = ymd_to_serial(y, m as i32, d as i32);
    Computed::Scalar(Value::Number(today_serial))
}

/// `=AGORA()` / `=NOW()`
pub fn fn_now(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    let ms = agora_local_ms();
    let serial = timestamp_to_serial(ms);
    Computed::Scalar(Value::Number(serial))
}

/// `=DATA(ano; mês; dia)` / `=DATE(year; month; day)`
pub fn fn_date(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let y = match ev.number(&args[0]) {
        Ok(v) => v as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let m = match ev.number(&args[1]) {
        Ok(v) => v as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let d = match ev.number(&args[2]) {
        Ok(v) => v as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    Computed::Scalar(Value::Number(ymd_to_serial(y, m, d)))
}

/// `=ANO(data)` / `=YEAR(date)`
pub fn fn_year(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    parte_da_data(args, ev, 1, |s| serial_to_ymd(s as i64).0 as f64)
}

/// `=MÊS(data)` / `=MONTH(date)`
pub fn fn_month(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    parte_da_data(args, ev, 1, |s| serial_to_ymd(s as i64).1 as f64)
}

/// `=DIA(data)` / `=DAY(date)`
pub fn fn_day(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    parte_da_data(args, ev, 1, |s| serial_to_ymd(s as i64).2 as f64)
}

/// `=DIA.DA.SEMANA(data; [tipo])` / `=WEEKDAY(date; [return_type])`
pub fn fn_weekday(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let serial = serial_do_valor(e.valor(0))?;
        if serial < 0.0 { return Err(CellError::new(ErrorCode::Num)); }
        // Tipo do Excel → dia que vale 1 (0 = domingo); o tipo 3 conta de 0.
        let (primeiro, base) = match e.int_ou(1, 1)? {
            1 | 17 => (0, 1),
            2 | 11 => (1, 1),
            3 => (1, 0),
            t @ 12..=16 => (t - 10, 1),
            _ => return Err(CellError::new(ErrorCode::Num)),
        };
        Ok(Value::Number(((dia_da_semana(serial as i64) - primeiro).rem_euclid(7) + base) as f64))
    })
}

/// `=NÚM.SEMANA(data; [tipo])` / `=WEEKNUM(date; [return_type])`
pub fn fn_weeknum(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let serial = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if serial < 0.0 {
        return err_num();
    }
    let (y, _, _) = serial_to_ymd(serial as i64);
    let jan1_serial = ymd_to_serial(y, 1, 1);
    let diff_days = (serial.floor() - jan1_serial.floor()) as i64;
    let jan1_weekday = dia_da_semana(jan1_serial as i64); // 0 = Domingo
    let week_num = ((diff_days + jan1_weekday) / 7) + 1;
    Computed::Scalar(Value::Number(week_num as f64))
}

/// `=DATADIF(data_inicial; data_final; unidade)` / `=DATEDIF(...)`
pub fn fn_datedif(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let a = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let b = match date_arg(&args[1], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let raw_unit = match ev.text(&args[2]) {
        Ok(u) => u,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    if a < 0.0 || b < 0.0 || b < a {
        return err_num();
    }

    let u_str = raw_unit.trim().to_uppercase();
    let unit = match u_str.as_str() {
        "A" => "Y",
        "AM" => "YM",
        "AD" => "YD",
        other => other,
    };

    let (y1, m1, d1) = serial_to_ymd(a as i64);
    let (y2, m2, d2) = serial_to_ymd(b as i64);

    let res = match unit {
        "Y" => {
            let mut diff = y2 - y1;
            if m2 < m1 || (m2 == m1 && d2 < d1) {
                diff -= 1;
            }
            diff as f64
        }
        "M" => {
            let mut diff = (y2 - y1) * 12 + (m2 as i32 - m1 as i32);
            if d2 < d1 {
                diff -= 1;
            }
            diff as f64
        }
        "D" => b.floor() - a.floor(),
        "YM" => {
            let mut diff = m2 as i32 - m1 as i32;
            if d2 < d1 {
                diff -= 1;
            }
            if diff < 0 {
                diff += 12;
            }
            diff as f64
        }
        "YD" => {
            let mut cmp_serial = ymd_to_serial(y1, m2 as i32, d2 as i32);
            if cmp_serial < a {
                cmp_serial = ymd_to_serial(y1 + 1, m2 as i32, d2 as i32);
            }
            cmp_serial.floor() - a.floor()
        }
        "MD" => {
            if d2 >= d1 {
                (d2 - d1) as f64
            } else {
                let prev_last = days_in_month(y2, if m2 == 1 { 12 } else { m2 - 1 });
                (prev_last - d1 + d2) as f64
            }
        }
        _ => return err_num(),
    };

    Computed::Scalar(Value::Number(res))
}

/// `=DATAM(data_inicial; meses)` / `=EDATE(...)`
pub fn fn_edate(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let start = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let months = match ev.number(&args[1]) {
        Ok(m) => m.trunc() as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    let (y, m, d) = serial_to_ymd(start as i64);
    let total_months = (m as i32 - 1) + months;
    let target_y = y + total_months.div_euclid(12);
    let target_m = (total_months.rem_euclid(12) + 1) as u32;
    let max_day = days_in_month(target_y, target_m);
    let target_d = d.min(max_day);

    Computed::Scalar(Value::Number(ymd_to_serial(target_y, target_m as i32, target_d as i32)))
}

/// `=FIMMÊS(data_inicial; meses)` / `=EOMONTH(...)`
pub fn fn_eomonth(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let start = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let months = match ev.number(&args[1]) {
        Ok(m) => m.trunc() as i32,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    let (y, m, _) = serial_to_ymd(start as i64);
    let total_months = (m as i32 - 1) + months;
    let target_y = y + total_months.div_euclid(12);
    let target_m = (total_months.rem_euclid(12) + 1) as u32;
    let max_day = days_in_month(target_y, target_m);

    Computed::Scalar(Value::Number(ymd_to_serial(target_y, target_m as i32, max_day as i32)))
}

/// `=DIATRABALHOTOTAL(data_inicial; data_final; [feriados])` / `=NETWORKDAYS(...)`
pub fn fn_networkdays(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let a = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let b = match date_arg(&args[1], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    let mut holidays = HashSet::new();
    if has(args, 2) {
        ev.for_each_number(&args[2], &mut |h| {
            holidays.insert(h.floor() as i64);
        });
    }

    let start = a.floor().min(b.floor()) as i64;
    let end = a.floor().max(b.floor()) as i64;
    let mut count = 0;
    for curr in start..=end {
        let dow = dia_da_semana(curr); // 0 = Domingo, 6 = Sábado
        if dow != 0 && dow != 6 && !holidays.contains(&curr) {
            count += 1;
        }
    }

    let res = if a <= b { count } else { -count };
    Computed::Scalar(Value::Number(res as f64))
}

/// `=DIATRABALHO(data_inicial; dias; [feriados])` / `=WORKDAY(...)`
pub fn fn_workday(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let start = match date_arg(&args[0], ev) {
        Ok(s) => s,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let days = match ev.number(&args[1]) {
        Ok(d) => d.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    let mut holidays = HashSet::new();
    if has(args, 2) {
        ev.for_each_number(&args[2], &mut |h| {
            holidays.insert(h.floor() as i64);
        });
    }

    let mut curr = start.floor() as i64;
    let mut count = days.abs();
    let step = if days >= 0 { 1 } else { -1 };

    while count > 0 {
        curr += step;
        let dow = dia_da_semana(curr); // 0 = Domingo, 6 = Sábado
        if dow != 0 && dow != 6 && !holidays.contains(&curr) {
            count -= 1;
        }
    }

    Computed::Scalar(Value::Number(curr as f64))
}

// ---- hora, valor de texto, intervalos e dias úteis ----

/// Maior serial de data que o Excel aceita (31/12/9999).
const SERIAL_MAX: i64 = 2_958_465;

/// Hora de um texto: `hh:mm`, `hh:mm:ss`, com `AM`/`PM` opcional.
fn hora_de_texto(texto: &str) -> Option<f64> {
    let t = texto.trim();
    let maiuscula = t.to_uppercase();
    let (corpo, periodo) = if let Some(r) = maiuscula.strip_suffix("PM") {
        (r.trim().to_string(), Some(true))
    } else if let Some(r) = maiuscula.strip_suffix("AM") {
        (r.trim().to_string(), Some(false))
    } else {
        (maiuscula.clone(), None)
    };
    let partes: Vec<&str> = corpo.split(':').collect();
    if !(2..=3).contains(&partes.len()) || partes.iter().any(|p| p.is_empty()) {
        return None;
    }
    let digitos = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    if !partes.iter().all(|p| digitos(p)) {
        // segundos com fração ("10:30:15.5")
        let ultima = partes[partes.len() - 1];
        if partes.len() != 3 || !partes[..2].iter().all(|p| digitos(p))
            || ultima.parse::<f64>().is_err()
        {
            return None;
        }
    }
    let mut h: f64 = partes[0].parse().ok()?;
    let m: f64 = partes[1].parse().ok()?;
    let seg: f64 = if partes.len() == 3 { partes[2].parse().ok()? } else { 0.0 };
    match periodo {
        Some(pm) => {
            if !(1.0..=12.0).contains(&h) {
                return None;
            }
            h %= 12.0;
            if pm {
                h += 12.0;
            }
        }
        None => {
            if h > 23.0 {
                return None;
            }
        }
    }
    if m > 59.0 || seg >= 60.0 {
        return None;
    }
    Some((h * 3600.0 + m * 60.0 + seg) / 86_400.0)
}

/// Serial de um texto que é data, hora ou data e hora (não vale número escrito).
fn serial_de_texto(t: &str) -> Option<f64> {
    let t = t.trim();
    let (data, hora) = match t.find([' ', 'T']) {
        Some(i) => (&t[..i], Some(t[i + 1..].trim())),
        None => (t, None),
    };
    if let Some(d) = crate::celulas::input::parse_date_text(data) {
        return match hora {
            None => Some(d),
            Some(h) => hora_de_texto(h).map(|f| d + f),
        };
    }
    hora_de_texto(t)
}

/// Serial a partir de um valor: número, lógico, vazio ou texto (data, hora,
/// data e hora, ou número escrito). Erro propaga.
fn serial_de(v: &Value) -> Res<f64> {
    if let Value::Text(t) = v {
        if let Some(n) = serial_de_texto(t) {
            return Ok(n);
        }
    }
    crate::celulas::values::to_number(v)
}

/// Serial de data válido do Excel: de 0 a 31/12/9999.
fn serial_valido(n: f64) -> Res<f64> {
    if n.is_finite() && (0.0..(SERIAL_MAX + 1) as f64).contains(&n) {
        Ok(n)
    } else {
        Err(e_num())
    }
}

/// Os segundos do dia de um serial, arredondados ao segundo (como o Excel:
/// 0,99999999 já é meia-noite do dia seguinte), 0..86 400.
fn segundos_do_dia(serial: f64) -> Res<i64> {
    let serial = serial_valido(serial)?;
    let frac = serial - serial.floor();
    Ok((frac * 86_400.0).round() as i64 % 86_400)
}

/// `=HORA(núm_série)` / `=HOUR(serial_number)`
pub fn fn_hour(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let seg = segundos_do_dia(serial_de(e.valor(0))?)?;
        numero((seg / 3600) as f64)
    })
}

/// `=MINUTO(núm_série)` / `=MINUTE(serial_number)`
pub fn fn_minute(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let seg = segundos_do_dia(serial_de(e.valor(0))?)?;
        numero(((seg / 60) % 60) as f64)
    })
}

/// `=SEGUNDO(núm_série)` / `=SECOND(serial_number)`
pub fn fn_second(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let seg = segundos_do_dia(serial_de(e.valor(0))?)?;
        numero((seg % 60) as f64)
    })
}

/// `=TEMPO(hora; minuto; segundo)` / `=TIME(hour; minute; second)`
pub fn fn_time(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 3, 3, |e| {
        let (h, m, s) = (e.int(0)?, e.int(1)?, e.int(2)?);
        if [h, m, s].iter().any(|v| v.abs() > 32767) {
            return Err(e_num());
        }
        let total = h * 3600 + m * 60 + s;
        if total < 0 {
            return Err(e_num());
        }
        numero((total % 86_400) as f64 / 86_400.0)
    })
}

/// `=DATA.VALOR(texto_data)` / `=DATEVALUE(date_text)`
pub fn fn_datevalue(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| match e.valor(0) {
        Value::Error(er) => Err(er.clone()),
        Value::Text(t) => {
            let n = serial_de_texto(t).ok_or_else(e_valor)?;
            numero(n.floor())
        }
        _ => Err(e_valor()),
    })
}

/// `=VALOR.TEMPO(texto_hora)` / `=TIMEVALUE(time_text)`
pub fn fn_timevalue(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| match e.valor(0) {
        Value::Error(er) => Err(er.clone()),
        Value::Text(t) => {
            let n = serial_de_texto(t).ok_or_else(e_valor)?;
            numero(n - n.floor())
        }
        _ => Err(e_valor()),
    })
}

/// `=DIAS(data_final; data_inicial)` / `=DAYS(end_date; start_date)`
pub fn fn_days(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| {
        let fim = serial_valido(serial_de(e.valor(0))?)?.trunc();
        let ini = serial_valido(serial_de(e.valor(1))?)?.trunc();
        numero(fim - ini)
    })
}

fn ultimo_dia_de_fevereiro(y: i32, m: u32, d: u32) -> bool {
    m == 2 && d == days_in_month(y, 2)
}

/// Dias entre duas datas em ano de 360 dias. `europeu` escolhe o método
/// europeu; senão o americano (NASD), que trata o fim de fevereiro à parte.
pub(crate) fn dias_360(ini: i64, fim: i64, europeu: bool) -> i64 {
    let (y1, m1, mut d1) = serial_to_ymd(ini);
    let (y2, m2, mut d2) = serial_to_ymd(fim);
    if europeu {
        d1 = d1.min(30);
        d2 = d2.min(30);
    } else {
        let fev_ini = ultimo_dia_de_fevereiro(y1, m1, d1);
        let fev_fim = ultimo_dia_de_fevereiro(y2, m2, d2);
        if fev_ini && fev_fim {
            d2 = 30;
        }
        if fev_ini {
            d1 = 30;
        }
        if d2 == 31 && d1 >= 30 {
            d2 = 30;
        }
        if d1 == 31 {
            d1 = 30;
        }
    }
    (y2 as i64 - y1 as i64) * 360 + (m2 as i64 - m1 as i64) * 30 + (d2 as i64 - d1 as i64)
}

/// `=DIAS360(data_inicial; data_final; [método])` / `=DAYS360(start_date; end_date; [method])`
pub fn fn_days360(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 3, |e| {
        let ini = serial_valido(serial_de(e.valor(0))?)?.trunc();
        let fim = serial_valido(serial_de(e.valor(1))?)?.trunc();
        let europeu = e.logico_ou(2, false)?;
        numero(dias_360(ini as i64, fim as i64, europeu) as f64)
    })
}

/// `=NÚMSEMANAISO(data)` / `=ISOWEEKNUM(date)`
pub fn fn_isoweeknum(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let s = serial_valido(serial_de(e.valor(0))?)?.floor() as i64;
        let dow = dia_da_semana(s);
        let iso = if dow == 0 { 7 } else { dow };
        // A quinta-feira da mesma semana decide o ano da semana.
        let quinta = s - iso + 4;
        let (ano, _, _) = serial_to_ymd(quinta);
        let primeiro = ymd_to_serial(ano, 1, 1) as i64;
        numero(((quinta - primeiro) / 7 + 1) as f64)
    })
}

fn bissexto(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Fração de ano entre dois seriais (inteiros), pelas bases do Excel:
/// 0 = 30/360 americano, 1 = real/real, 2 = real/360, 3 = real/365,
/// 4 = 30/360 europeu.
pub(crate) fn yearfrac_core(inicio: i64, fim: i64, base: i64) -> Result<f64, CellError> {
    if !(0..=4).contains(&base) || inicio < 0 || fim < 0 {
        return Err(e_num());
    }
    let (ini, fim) = if inicio <= fim { (inicio, fim) } else { (fim, inicio) };
    let dias = (fim - ini) as f64;
    match base {
        0 => Ok(dias_360(ini, fim, false).abs() as f64 / 360.0),
        4 => Ok(dias_360(ini, fim, true).abs() as f64 / 360.0),
        2 => Ok(dias / 360.0),
        3 => Ok(dias / 365.0),
        _ => {
            if dias == 0.0 {
                return Ok(0.0);
            }
            let (y1, m1, d1) = serial_to_ymd(ini);
            let (y2, m2, d2) = serial_to_ymd(fim);
            let tamanho = if y1 == y2 {
                if bissexto(y1) { 366.0 } else { 365.0 }
            } else if y2 == y1 + 1 && (m1, d1) >= (m2, d2) {
                // Até um ano, cruzando a virada: 366 se o 29/02 cai no intervalo.
                let fev29_1 = bissexto(y1) && ini <= ymd_to_serial(y1, 2, 29) as i64;
                let fev29_2 = bissexto(y2) && fim >= ymd_to_serial(y2, 2, 29) as i64;
                if fev29_1 || fev29_2 { 366.0 } else { 365.0 }
            } else {
                // Mais de um ano: média do tamanho dos anos tocados.
                let anos = (y2 - y1 + 1) as f64;
                let total = ymd_to_serial(y2 + 1, 1, 1) - ymd_to_serial(y1, 1, 1);
                total / anos
            };
            Ok(dias / tamanho)
        }
    }
}

/// `=FRAÇÃOANO(data_inicial; data_final; [base])` / `=YEARFRAC(start_date; end_date; [basis])`
pub fn fn_yearfrac(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 3, |e| {
        let ini = serial_valido(serial_de(e.valor(0))?)?.trunc();
        let fim = serial_valido(serial_de(e.valor(1))?)?.trunc();
        let base = e.int_ou(2, 0)?;
        numero(yearfrac_core(ini as i64, fim as i64, base)?)
    })
}

/// Dias de fim de semana (0 = domingo … 6 = sábado) a partir do argumento
/// `fim_de_semana`: código 1–7, 11–17 ou texto de sete 0/1 começando na segunda.
fn fim_de_semana(v: &Value, presente: bool) -> Res<[bool; 7]> {
    let mut fds = [false; 7];
    if !presente {
        fds[0] = true;
        fds[6] = true;
        return Ok(fds);
    }
    let n = match v {
        Value::Error(e) => return Err(e.clone()),
        // Texto só vale como sete 0/1 (segunda a domingo), e nem todos 1.
        Value::Text(t) => {
            let t = t.trim();
            if t.chars().count() != 7 || !t.chars().all(|c| c == '0' || c == '1') || !t.contains('0') {
                return Err(e_valor());
            }
            for (i, c) in t.chars().enumerate() {
                fds[(i + 1) % 7] = c == '1';
            }
            return Ok(fds);
        }
        outro => crate::celulas::values::to_number(outro)?.trunc() as i64,
    };
    codigo_fim_de_semana(n)
}

fn codigo_fim_de_semana(n: i64) -> Res<[bool; 7]> {
    let mut fds = [false; 7];
    match n {
        1..=7 => {
            fds[(n - 2).rem_euclid(7) as usize] = true;
            fds[(n - 1).rem_euclid(7) as usize] = true;
        }
        11..=17 => fds[(n - 11) as usize] = true,
        _ => return Err(e_num()),
    }
    Ok(fds)
}

fn feriados_de(args: &[Node], i: usize, ev: &Evaluator<'_>) -> Res<HashSet<i64>> {
    let mut set = HashSet::new();
    if has(args, i) {
        for h in numeros_de(ev, &args[i])? {
            set.insert(h.floor() as i64);
        }
    }
    Ok(set)
}

/// `=DIATRABALHOTOTAL.INTL(data_inicial; data_final; [fim_de_semana]; [feriados])` /
/// `=NETWORKDAYS.INTL(start_date; end_date; [weekend]; [holidays])`
pub fn fn_networkdays_intl(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let feriados = match feriados_de(args, 3, ev) {
        Ok(f) => f,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let corte = &args[..args.len().min(3)];
    por_elemento(corte, ev, 2, 3, |e| {
        let a = serial_valido(serial_de(e.valor(0))?)?.floor();
        let b = serial_valido(serial_de(e.valor(1))?)?.floor();
        let fds = fim_de_semana(e.valor(2), e.tem(2))?;
        let (ini, fim) = (a.min(b) as i64, a.max(b) as i64);
        let n = (ini..=fim)
            .filter(|d| !fds[dia_da_semana(*d) as usize] && !feriados.contains(d))
            .count() as f64;
        numero(if a <= b { n } else { -n })
    })
}

/// `=DIATRABALHO.INTL(data_inicial; dias; [fim_de_semana]; [feriados])` /
/// `=WORKDAY.INTL(start_date; days; [weekend]; [holidays])`
pub fn fn_workday_intl(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let feriados = match feriados_de(args, 3, ev) {
        Ok(f) => f,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let corte = &args[..args.len().min(3)];
    por_elemento(corte, ev, 2, 3, |e| {
        let inicio = serial_valido(serial_de(e.valor(0))?)?.floor();
        let dias = e.int(1)?;
        let fds = fim_de_semana(e.valor(2), e.tem(2))?;
        let passo = if dias >= 0 { 1 } else { -1 };
        let mut atual = inicio as i64;
        let mut falta = dias.abs();
        while falta > 0 {
            atual += passo;
            if atual < 0 || atual > SERIAL_MAX {
                return Err(e_num());
            }
            if !fds[dia_da_semana(atual) as usize] && !feriados.contains(&atual) {
                falta -= 1;
            }
        }
        numero(atual as f64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planilha::sheet::Sheet;

    #[test]
    fn test_funcoes_de_data() {
        let mut sheet = Sheet::new("Plan1");

        // DATA(2026; 9; 22) -> serial 46287
        sheet.set_cell_raw(0, 0, "=DATA(2026; 9; 22)".into(), None);
        let s = sheet.get_value(0, 0);
        assert_eq!(s, Value::Number(46287.0));

        sheet.set_cell_raw(1, 0, "=ANO(A1)".into(), None);
        assert_eq!(sheet.get_value(1, 0), Value::Number(2026.0));

        sheet.set_cell_raw(2, 0, "=MÊS(A1)".into(), None);
        assert_eq!(sheet.get_value(2, 0), Value::Number(9.0));

        sheet.set_cell_raw(3, 0, "=DIA(A1)".into(), None);
        assert_eq!(sheet.get_value(3, 0), Value::Number(22.0));

        // FIMMÊS(DATA(2026; 9; 22); 0) -> DATA(2026; 9; 30) = 46295
        sheet.set_cell_raw(4, 0, "=FIMMÊS(A1; 0)".into(), None);
        assert_eq!(sheet.get_value(4, 0), Value::Number(46295.0));

        // DATAM(DATA(2026; 9; 22); 2) -> DATA(2026; 11; 22) = 46348
        sheet.set_cell_raw(5, 0, "=DATAM(A1; 2)".into(), None);
        assert_eq!(sheet.get_value(5, 0), Value::Number(46348.0));

        // DATADIF(DATA(2020; 1; 1); DATA(2026; 1; 1); "Y") = 6
        sheet.set_cell_raw(6, 0, "=DATADIF(DATA(2020; 1; 1); DATA(2026; 1; 1); \"Y\")".into(), None);
        assert_eq!(sheet.get_value(6, 0), Value::Number(6.0));
    }

    fn calc(f: &str) -> Value {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, format!("={f}"), None);
        sheet.get_value(0, 0)
    }

    #[test]
    fn hora_minuto_segundo_e_tempo() {
        assert_eq!(calc("HORA(0,75)"), Value::Number(18.0));
        assert_eq!(calc("MINUTO(0,0625)"), Value::Number(30.0));
        assert_eq!(calc("SEGUNDO(0,000694444)"), Value::Number(0.0));
        assert_eq!(calc("TEMPO(12; 0; 0)"), Value::Number(0.5));
        assert_eq!(calc("TEMPO(27; 0; 0)"), Value::Number(0.125));
        assert!(matches!(calc("TEMPO(0; 0; -1)"), Value::Error(_)));
    }

    #[test]
    fn valores_de_texto_e_intervalos() {
        assert_eq!(calc("DATA.VALOR(\"2023-01-15\")"), calc("DATA(2023; 1; 15)"));
        assert_eq!(calc("DATA.VALOR(\"15/01/2023\")"), calc("DATA(2023; 1; 15)"));
        assert_eq!(calc("VALOR.TEMPO(\"18:00\")"), Value::Number(0.75));
        assert_eq!(calc("DIAS(DATA(2023; 3; 1); DATA(2023; 1; 1))"), Value::Number(59.0));
        // Exemplos da documentação do Excel.
        assert_eq!(calc("DIAS360(DATA(2011; 1; 1); DATA(2011; 12; 31))"), Value::Number(360.0));
        assert_eq!(calc("DIAS360(DATA(2011; 1; 30); DATA(2011; 2; 1))"), Value::Number(1.0));
        assert_eq!(calc("NÚMSEMANAISO(DATA(2012; 3; 9))"), Value::Number(10.0));
        assert_eq!(calc("NÚMSEMANAISO(DATA(2021; 1; 3))"), Value::Number(53.0));
        assert_eq!(calc("FRAÇÃOANO(DATA(2012; 1; 1); DATA(2012; 7; 30))"), Value::Number(0.58055555555555555));
        assert_eq!(calc("FRAÇÃOANO(DATA(2012; 1; 1); DATA(2012; 7; 30); 3)"), Value::Number(211.0 / 365.0));
    }

    #[test]
    fn dias_uteis_com_fim_de_semana_proprio() {
        // Janeiro de 2023: dia 2 é segunda. Fim de semana 11 = só domingo.
        assert_eq!(calc("DIATRABALHOTOTAL.INTL(DATA(2023; 1; 2); DATA(2023; 1; 8))"), Value::Number(5.0));
        assert_eq!(calc("DIATRABALHOTOTAL.INTL(DATA(2023; 1; 2); DATA(2023; 1; 8); 11)"), Value::Number(6.0));
        assert_eq!(calc("DIATRABALHOTOTAL.INTL(DATA(2023; 1; 2); DATA(2023; 1; 8); \"0000011\")"), Value::Number(5.0));
        assert_eq!(calc("DIATRABALHO.INTL(DATA(2023; 1; 6); 1)"), calc("DATA(2023; 1; 9)"));
        assert_eq!(calc("DIATRABALHO.INTL(DATA(2023; 1; 6); 1; 11)"), calc("DATA(2023; 1; 7)"));
    }
}
