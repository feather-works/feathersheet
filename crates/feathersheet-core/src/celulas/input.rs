use crate::celulas::values::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    General,
    Number,
    Currency,
    Percent,
    Date,
    Time,
    Datetime,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Literal {
    pub value: Value,
    pub format: Option<NumberFormat>,
}

/// Converte texto digitado em literal (port de core/input.ts).
/// Nao aceita numeros invalidos nem perde espacos de texto puro.
pub fn parse_literal(raw: &str) -> Literal {
    if raw.is_empty() { return Literal { value: Value::Empty, format: None }; }
    let t = raw.trim();

    // O que a célula mostra na língua do app (VERDADERO, VRAI) também entra como lógico:
    // senão F2 e Enter numa célula lógica a transformava em texto.
    if let Some(logico) = super::locale::logico_digitado(t) {
        return Literal { value: Value::Bool(logico), format: None };
    }
    if let Some(n) = moeda(t) {
        return Literal { value: Value::Number(n), format: Some(NumberFormat::Currency) };
    }
    if let Some(n) = parse_number_str(t) {
        return Literal { value: Value::Number(n), format: None };
    }
    // O TS interpreta o percentual removendo os pontos e trocando a
    // primeira virgula por ponto; mantemos a regra do formato original.
    if let Some(s) = t.strip_suffix('%') {
        let sem_sinal = s.strip_prefix('-').unwrap_or(s);
        if !sem_sinal.is_empty()
            && sem_sinal.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b',')
        {
            // Ponto sozinho que não é milhar é decimal: "15.5%" é 15,5%, não 155%.
            let num = if !s.contains(',') && !grupos_de_milhar(s, '.') { s.to_owned() }
                else { s.replace('.', "").replacen(',', ".", 1) };
            if let Ok(n) = num.parse::<f64>() {
                if n.is_finite() {
                    return Literal { value: Value::Number(n / 100.0), format: Some(NumberFormat::Percent) };
                }
            }
        }
    }
    if let Some((data, hora)) = t.split_once(' ') {
        if let (Some(d), Some(h)) = (parse_date_text(data), parse_time_text(hora.trim())) {
            return Literal { value: Value::Number(d + h), format: Some(NumberFormat::Datetime) };
        }
    } else {
        if let Some(d) = parse_date_text(t) {
            return Literal { value: Value::Number(d), format: Some(NumberFormat::Date) };
        }
        if let Some(h) = parse_time_text(t) {
            return Literal { value: Value::Number(h), format: Some(NumberFormat::Time) };
        }
    }
    Literal { value: Value::Text(raw.to_owned()), format: None }
}

/// Número digitado ou colado na língua de ponto decimal: a vírgula é milhar (`1,234` é 1234,
/// `$1,234.50`) e sai antes de o texto chegar ao analisador. O analisador não muda com a
/// língua: o arquivo guarda o texto digitado e é lido por ele (ideia do PR #162).
pub fn sem_milhar_digitado(texto: &str, cfg: &super::locale::LocaleConfig) -> Option<String> {
    if cfg.decimal_separator != '.' || cfg.thousands_separator != ',' || !texto.contains(',') { return None; }
    let nucleo = texto.trim().trim_end_matches(['%', ')', ' ']).trim_start_matches(['-', '(']);
    let nucleo = nucleo.strip_prefix(cfg.currency_symbol.as_str()).unwrap_or(nucleo).trim_start();
    let (inteiro, fracao) = nucleo.split_once('.').unwrap_or((nucleo, "0"));
    (grupos_de_milhar(inteiro, ',') && so_digitos(fracao)).then(|| texto.replace(',', ""))
}

/// Identifica somente a funcao na raiz da formula, sem falsos positivos
/// como =AGORAX() ou =DATA_TESTE(). Reusa aliases do motor Rust.
pub fn implied_format(raw: &str) -> Option<NumberFormat> {
    if let Some(formula) = raw.strip_prefix('=') {
        let formula = formula.trim_start();
        let letra = |c: char| c.is_ascii_alphabetic() || c == '_' || ('\u{C0}'..='\u{FF}').contains(&c);
        let mut fim = 0;
        for (indice, c) in formula.char_indices() {
            let aceita = if indice == 0 { letra(c) } else { letra(c) || c.is_ascii_digit() || c == '.' };
            if !aceita { break; }
            fim = indice + c.len_utf8();
        }
        if fim == 0 || !formula[fim..].trim_start().starts_with('(') { return None; }
        return match crate::funcoes::functions::date_result(&formula[..fim].to_uppercase())? {
            crate::funcoes::functions::DateResult::Date => Some(NumberFormat::Date),
            crate::funcoes::functions::DateResult::DateTime => Some(NumberFormat::Datetime),
        };
    }
    parse_literal(raw).format
}

fn so_digitos(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn numero_simples(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    if let Some(i) = s.find(|c| c == '.' || c == ',') {
        so_digitos(&s[..i]) && so_digitos(&s[i + 1..])
    } else { so_digitos(s) }
}

/// Milhar de três em três (`1.234.567`) ou como na Índia, lakh e crore: o último grupo
/// com três e os do meio com dois (`12,34,567`).
fn grupos_de_milhar(s: &str, sep: char) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let partes: Vec<&str> = s.split(sep).collect();
    let [primeiro, meio @ .., ultimo] = partes.as_slice() else { return false };
    if !partes.iter().all(|p| so_digitos(p)) || ultimo.len() != 3 { return false; }
    let ocidental = primeiro.len() <= 3 && meio.iter().all(|p| p.len() == 3);
    let lakh = primeiro.len() <= 2 && meio.iter().all(|p| p.len() == 2);
    ocidental || lakh
}

fn cientifico(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (base, expoente) = if let Some(pos) = s.find(|c| c == 'e' || c == 'E') {
        (&s[..pos], Some(&s[pos + 1..]))
    } else { (s, None) };
    let base_ok = if let Some((inteiro, decimal)) = base.split_once('.') {
        (inteiro.is_empty() || so_digitos(inteiro)) && so_digitos(decimal)
    } else { so_digitos(base) };
    if !base_ok { return false; }
    match expoente {
        Some(e) => so_digitos(e.strip_prefix('+').or_else(|| e.strip_prefix('-')).unwrap_or(e)),
        None => base.contains('.'),
    }
}

/// Reconhece os padroes numericos do TS, na mesma ordem.
pub fn parse_number_str(t: &str) -> Option<f64> {
    let normalizado = (if numero_simples(t) {
        Some(t.replace(',', "."))
    } else if let Some((inteiro, decimal)) = t.rsplit_once(',') {
        if grupos_de_milhar(inteiro, '.') && so_digitos(decimal) {
            Some(t.replace('.', "").replace(',', "."))
        } else { None }
    } else { None })
    .or_else(|| {
        if grupos_de_milhar(t, '.') { Some(t.replace('.', "")) } else { None }
    })
    .or_else(|| {
        if let Some((inteiro, decimal)) = t.rsplit_once('.') {
            if grupos_de_milhar(inteiro, ',') && so_digitos(decimal) {
                return Some(t.replace(',', ""));
            }
        }
        None
    })
    .or_else(|| {
        if grupos_de_milhar(t, ',') { Some(t.replace(',', "")) } else { None }
    })
    .or_else(|| {
        // Milhar com espaço, como em francês: `1 250,50`. O espaço comum só vale na língua
        // que separa milhar assim: em português "01 310" é código e fica texto. O espaço
        // que não quebra (colado de um número formatado) vale sempre.
        if t.contains(' ') && super::locale::thousands_separator() != ' ' { return None; }
        let t = t.replace(['\u{a0}', '\u{202f}'], " ");
        let (inteiro, decimal) = t.rsplit_once(',').unwrap_or((&t, ""));
        (grupos_de_milhar(inteiro, ' ') && (decimal.is_empty() || so_digitos(decimal)) && !t.ends_with(','))
            .then(|| t.replace(' ', "").replace(',', "."))
    })
    .or_else(|| {
        if cientifico(t) { Some(t.to_owned()) } else { None }
    })?;
    normalizado.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Moeda conforme parseCurrencyLiteral: R$ ou $, sinal ou parenteses antes
/// ou depois do simbolo. Preserva negativo e evita aceitar sufixos invalidos.
fn moeda(raw: &str) -> Option<f64> {
    let mut texto = raw.trim();
    let mut negativo = false;
    if let Some(resto) = texto.strip_prefix('-') {
        negativo = true;
        texto = resto.trim();
    } else if let Some(resto) = texto.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        negativo = true;
        texto = resto.trim();
    }
    if texto.starts_with("R$") || texto.starts_with("r$") {
        texto = texto.get(2..)?.trim();
    } else if let Some(resto) = texto.strip_prefix('$') {
        texto = resto.trim();
    } else { return None; }
    if let Some(resto) = texto.strip_prefix('-') {
        negativo = !negativo;
        texto = resto.trim();
    } else if let Some(resto) = texto.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        negativo = !negativo;
        texto = resto.trim();
    }
    if texto.is_empty() { return None; }
    let normalizado = if texto.contains(',') {
        texto.replace('.', "").replacen(',', ".", 1)
    } else if grupos_de_milhar(texto, '.') {
        texto.replace('.', "")
    } else { texto.to_owned() };
    let n = normalizado.parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(if negativo { -n } else { n })
}

pub fn parse_date_text(texto: &str) -> Option<f64> {
    let (ano, mes, dia) = if texto.contains('/') {
        let p: Vec<&str> = texto.split('/').collect();
        if p.len() != 3 || !(1..=2).contains(&p[0].len())
            || !(1..=2).contains(&p[1].len()) || !(2..=4).contains(&p[2].len())
            || !p.iter().all(|s| so_digitos(s)) { return None; }
        let ano = p[2].parse::<i32>().ok()?;
        (if ano < 100 { ano + 2000 } else { ano },
         p[1].parse::<i32>().ok()?, p[0].parse::<i32>().ok()?)
    } else {
        let p: Vec<&str> = texto.split('-').collect();
        if p.len() != 3 || p[0].len() != 4
            || !(1..=2).contains(&p[1].len()) || !(1..=2).contains(&p[2].len())
            || !p.iter().all(|s| so_digitos(s)) { return None; }
        (p[0].parse::<i32>().ok()?, p[1].parse::<i32>().ok()?, p[2].parse::<i32>().ok()?)
    };
    if ano < 100 || !(1..=12).contains(&mes) || dia < 1 { return None; }
    let bissexto = ano % 4 == 0 && (ano % 100 != 0 || ano % 400 == 0);
    let limite = match mes {
        2 if bissexto => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if dia > limite { return None; }
    Some(date_to_serial(ano, mes, dia))
}

pub fn parse_time_text(texto: &str) -> Option<f64> {
    let p: Vec<&str> = texto.split(':').collect();
    if !(2..=3).contains(&p.len()) || !(1..=2).contains(&p[0].len())
        || p[1].len() != 2 || (p.len() == 3 && p[2].len() != 2)
        || !p.iter().all(|s| so_digitos(s))
    { return None; }
    let h = p[0].parse::<u32>().ok()?;
    let m = p[1].parse::<u32>().ok()?;
    let seg = if p.len() == 3 { p[2].parse::<u32>().ok()? } else { 0 };
    if h > 23 || m > 59 || seg > 59 { return None; }
    Some((h * 3600 + m * 60 + seg) as f64 / 86_400.0)
}

/// Dia/mês/ano → serial de data (época do Excel). Exposta ao crate para o
/// assistente de "dividir texto em colunas" resolver uma ordem explícita de
/// campos, em vez da heurística de [`parse_date_text`].
#[inline]
pub(crate) fn date_to_serial(year: i32, month: i32, day: i32) -> f64 {
    let a = (14 - month) / 12;
    let y = year + 4800 - a;
    let m = month + 12 * a - 3;
    let julian = day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    (julian - 2415019) as f64
}

#[cfg(test)]
mod testes_entrada {
    use super::*;

    fn numero(raw: &str, n: f64, formato: Option<NumberFormat>) {
        let lido = parse_literal(raw);
        assert_eq!(lido.value, Value::Number(n), "{raw}");
        assert_eq!(lido.format, formato, "{raw}");
    }

    #[test]
    fn numeros_e_texto_mantem_tipo() {
        numero("12,5", 12.5, None);
        numero("1.234.567,89", 1_234_567.89, None);
        numero("1,234,567.89", 1_234_567.89, None);
        numero("2e3", 2000.0, None);
        numero(".5", 0.5, None);
        // Lakh e crore, como o formato de ₹ mostra: em português e em inglês.
        numero("1.23.45.678,90", 12_345_678.9, None);
        numero("12,34,567.5", 1_234_567.5, None);
        numero("-1,00,000", -100_000.0, None);
        numero("1\u{202f}250,50", 1250.5, None);
        assert_eq!(parse_literal("01 310").value, Value::Text("01 310".into()), "código com espaço não é milhar em português");
        numero("12\u{a0}345\u{a0}678", 12_345_678.0, None);
        assert_eq!(parse_literal("1 25").value, Value::Text("1 25".into()), "grupo curto");
        assert_eq!(parse_literal("1,2,345").value, Value::Text("1,2,345".into()), "grupo do meio com um dígito");
        assert_eq!(parse_literal("123,45,678").value, Value::Text("123,45,678".into()), "três antes do lakh");
        assert_eq!(parse_literal("1.23.4").value, Value::Text("1.23.4".into()));
        assert_eq!(parse_literal("+12").value, Value::Text("+12".into()));
        assert_eq!(parse_literal("NaN").value, Value::Text("NaN".into()));
        assert_eq!(parse_literal("  ").value, Value::Text("  ".into()));
    }

    #[test]
    fn moedas_negativas_porcentagem() {
        numero("R$ 1.234,50", 1234.5, Some(NumberFormat::Currency));
        numero("-R$ 1.234,50", -1234.5, Some(NumberFormat::Currency));
        numero("(R$ 1.234,50)", -1234.5, Some(NumberFormat::Currency));
        numero("R$ -1.234,50", -1234.5, Some(NumberFormat::Currency));
        numero("$50.25", 50.25, Some(NumberFormat::Currency));
        numero("15,5%", 0.155, Some(NumberFormat::Percent));
        assert_eq!(parse_literal("50 R$").value, Value::Text("50 R$".into()));
    }

    /// Casos do PR #162 (ash-171): em inglês a vírgula é milhar ao digitar.
    #[test]
    fn virgula_de_milhar_sai_so_na_lingua_de_ponto_decimal() {
        use crate::celulas::locale::LocaleConfig;
        let en = LocaleConfig::en_us();
        let lido = |t: &str| parse_literal(&sem_milhar_digitado(t, &en).unwrap_or_else(|| t.to_owned()));
        for (t, n, f) in [
            ("1,234", 1234.0, None), ("12,345", 12345.0, None), ("-1,234", -1234.0, None),
            ("1,234.50", 1234.5, None), ("12,34,567.50", 1_234_567.5, None), ("12.34", 12.34, None),
            ("$1,234.50", 1234.5, Some(NumberFormat::Currency)), ("-$1,234.50", -1234.5, Some(NumberFormat::Currency)),
            ("($1,234.50)", -1234.5, Some(NumberFormat::Currency)), ("$-1,234.50", -1234.5, Some(NumberFormat::Currency)),
            ("1,234.50%", 12.345, Some(NumberFormat::Percent)), ("15.5%", 0.155, Some(NumberFormat::Percent)),
        ] {
            assert_eq!(lido(t), Literal { value: Value::Number(n), format: f }, "{t}");
        }
        // Grupo errado, texto comum e fórmula passam como vieram.
        for t in ["12,34", "1,2,345", "1,234 itens", "=SUM(1,234)", "a,b"] {
            assert_eq!(sem_milhar_digitado(t, &en), None, "{t}");
        }
        // Em português nada muda: 1,234 continua 1,234.
        assert_eq!(sem_milhar_digitado("1,234", &LocaleConfig::pt_br()), None);
        numero("1.234,5%", 12.345, Some(NumberFormat::Percent));
        numero("1.234%", 12.34, Some(NumberFormat::Percent));
    }

    #[test]
    fn datas_horas_e_nomes_inteiros_de_funcoes() {
        assert_eq!(parse_literal("29/02/2024").format, Some(NumberFormat::Date));
        assert_eq!(parse_literal("1/2/26").format, Some(NumberFormat::Date));
        assert_eq!(parse_literal("2026-10-12").format, Some(NumberFormat::Date));
        assert_eq!(parse_literal("31/02/2026").value, Value::Text("31/02/2026".into()));
        assert_eq!(parse_literal("29/02/2025").value, Value::Text("29/02/2025".into()));
        assert_eq!(parse_literal("8:03").format, Some(NumberFormat::Time));
        assert_eq!(parse_literal("8:3").value, Value::Text("8:3".into()));
        assert_eq!(parse_literal("18/09/2026 14:30").format, Some(NumberFormat::Datetime));
        assert_eq!(implied_format("=HOJE()"), Some(NumberFormat::Date));
        assert_eq!(implied_format("=DATA(2026;1;1)"), Some(NumberFormat::Date));
        assert_eq!(implied_format("=AGORA()"), Some(NumberFormat::Datetime));
        assert_eq!(implied_format("=NOW()"), Some(NumberFormat::Datetime));
        assert_eq!(implied_format("=AGORAX()"), None);
        assert_eq!(implied_format("=HOJE_TESTE()"), None);
        assert_eq!(implied_format("=HOJE + 1"), None);
    }
}
