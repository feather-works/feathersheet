use crate::celulas::{locale::LocaleConfig, values::Value};

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
    let locale = super::locale::get_locale_config();
    parse_literal_com_locale(raw, &locale)
}

/// Variante determinística de [`parse_literal`] para entrada numérica.
pub fn parse_literal_com_locale(raw: &str, locale: &LocaleConfig) -> Literal {
    if raw.is_empty() { return Literal { value: Value::Empty, format: None }; }
    let t = raw.trim();

    // O que a célula mostra na língua do app (VERDADERO, VRAI) também entra como lógico:
    // senão F2 e Enter numa célula lógica a transformava em texto.
    if let Some(logico) = super::locale::logico_digitado(t) {
        return Literal { value: Value::Bool(logico), format: None };
    }
    if let Some(n) = moeda(t, locale) {
        return Literal { value: Value::Number(n), format: Some(NumberFormat::Currency) };
    }
    if let Some(n) = parse_number_str_com_locale(t, locale) {
        return Literal { value: Value::Number(n), format: None };
    }
    if let Some(s) = t.strip_suffix('%') {
        if let Some(n) = parse_number_str_com_locale(s, locale) {
            return Literal { value: Value::Number(n / 100.0), format: Some(NumberFormat::Percent) };
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

/// Milhar de três em três (`1.234.567`) ou como na Índia, lakh e crore: o último grupo
/// com três e os do meio com dois (`12,34,567`).
fn grupos_de_milhar(s: &str, sep: char) -> bool {
    let partes: Vec<&str> = s.split(sep).collect();
    let [primeiro, meio @ .., ultimo] = partes.as_slice() else { return false };
    if !partes.iter().all(|p| so_digitos(p)) || ultimo.len() != 3 { return false; }
    let ocidental = primeiro.len() <= 3 && meio.iter().all(|p| p.len() == 3);
    let lakh = primeiro.len() <= 2 && meio.iter().all(|p| p.len() == 2);
    ocidental || lakh
}

/// Reconhece um número segundo a configuração regional em uso.
pub fn parse_number_str(t: &str) -> Option<f64> {
    let locale = super::locale::get_locale_config();
    parse_number_str_com_locale(t, &locale)
}

/// Texto numérico canônico para caminhos internos que voltam pelo analisador de entrada.
pub fn numero_para_entrada(n: f64) -> String {
    let raw = crate::celulas::number::to_js_string(n);
    let decimal = super::locale::decimal_separator();
    if decimal == '.' { raw } else { raw.replace('.', &decimal.to_string()) }
}

/// Analisa separadores decimais e de milhar sem consultar estado global.
pub fn parse_number_str_com_locale(t: &str, locale: &LocaleConfig) -> Option<f64> {
    let tem_espaco_nao_separavel = t.contains('\u{a0}') || t.contains('\u{202f}');
    let espacos_normalizados = tem_espaco_nao_separavel.then(|| t.replace(['\u{a0}', '\u{202f}'], " "));
    let principal = if locale.thousands_separator == ' ' {
        numero_com_separadores(espacos_normalizados.as_deref().unwrap_or(t), locale.decimal_separator, Some(' '))
    } else {
        numero_com_separadores(t, locale.decimal_separator, Some(locale.thousands_separator))
    };
    principal
        .or_else(|| espacos_normalizados.as_deref()
            .and_then(|s| numero_com_separadores(s, locale.decimal_separator, Some(' '))))
        .or_else(|| {
            // Com os dois sinais, ou com mais de um decimal, o formato alternativo
            // é inequívoco e mantém dados colados de outra língua compatíveis.
            let alternativo = (t.contains('.') && t.contains(','))
                || t.matches(locale.decimal_separator).count() > 1;
            (alternativo && separadores_ponto_virgula(locale))
                .then(|| numero_com_separadores(t, locale.thousands_separator, Some(locale.decimal_separator)))
                .flatten()
        })
        // A notação científica histórica usa ponto mesmo numa interface com vírgula.
        .or_else(|| (locale.decimal_separator != '.' && (t.contains('e') || t.contains('E')))
            .then(|| numero_com_separadores(t, '.', None)).flatten())
        // `.5` não pode ser agrupamento e já era aceito antes da regionalização.
        .or_else(|| (locale.decimal_separator != '.' && (t.starts_with('.') || t.starts_with("-.")))
            .then(|| numero_com_separadores(t, '.', None)).flatten())
}

fn separadores_ponto_virgula(locale: &LocaleConfig) -> bool {
    matches!((locale.decimal_separator, locale.thousands_separator), ('.', ',') | (',', '.'))
}

fn numero_com_separadores(t: &str, decimal: char, milhar: Option<char>) -> Option<f64> {
    if milhar == Some(decimal) { return None; }
    let (negativo, t) = t.strip_prefix('-').map_or((false, t), |resto| (true, resto));
    if t.is_empty() || t.starts_with('+') { return None; }
    let pos_expoente = t.char_indices().find(|(_, c)| matches!(c, 'e' | 'E')).map(|(i, _)| i);
    let (mantissa, expoente) = match pos_expoente {
        Some(i) => {
            let expoente = &t[i + 1..];
            if expoente.contains(['e', 'E']) { return None; }
            let digitos = expoente.strip_prefix(['+', '-']).unwrap_or(expoente);
            if !so_digitos(digitos) { return None; }
            (&t[..i], Some(expoente))
        }
        None => (t, None),
    };
    let mut partes = mantissa.split(decimal);
    let inteiro = partes.next()?;
    let fracao = partes.next();
    if partes.next().is_some() || fracao.is_some_and(|f| !so_digitos(f)) { return None; }
    if inteiro.is_empty() {
        fracao?;
    } else if let Some(sep) = milhar.filter(|sep| inteiro.contains(*sep)) {
        if expoente.is_some() || !grupos_de_milhar(inteiro, sep) { return None; }
    } else if !so_digitos(inteiro) { return None; }

    let mut normalizado = String::with_capacity(t.len() + usize::from(negativo));
    if negativo { normalizado.push('-'); }
    if inteiro.is_empty() {
        normalizado.push('0');
    } else if let Some(sep) = milhar {
        normalizado.extend(inteiro.chars().filter(|c| *c != sep));
    } else { normalizado.push_str(inteiro); }
    if let Some(fracao) = fracao {
        normalizado.push('.');
        normalizado.push_str(fracao);
    }
    if let Some(expoente) = expoente {
        normalizado.push('e');
        normalizado.push_str(expoente);
    }
    normalizado.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Moeda com posição, espaço e símbolo definidos pela configuração regional.
fn moeda(raw: &str, locale: &LocaleConfig) -> Option<f64> {
    let mut texto = raw.trim();
    let mut negativo = false;
    if let Some(resto) = texto.strip_prefix('-') {
        negativo = true;
        texto = resto.trim();
    } else if let Some(resto) = texto.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        negativo = true;
        texto = resto.trim();
    }
    texto = tirar_simbolo_moeda(texto, locale)?;
    if let Some(resto) = texto.strip_prefix('-') {
        if negativo { return None; }
        negativo = true;
        texto = resto;
    } else if let Some(resto) = texto.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        if negativo { return None; }
        negativo = true;
        texto = resto;
    }
    if texto.is_empty() { return None; }
    let n = parse_number_str_com_locale(texto, locale)?;
    Some(if negativo { -n } else { n })
}

fn tirar_simbolo_moeda<'a>(texto: &'a str, locale: &LocaleConfig) -> Option<&'a str> {
    let simbolo = &locale.currency_symbol;
    if simbolo.is_empty() { return None; }
    let resto = if locale.currency_before {
        tirar_prefixo_sem_caixa(texto, simbolo)?
    } else { tirar_sufixo_sem_caixa(texto, simbolo)? };
    if locale.currency_space {
        if locale.currency_before { tirar_espaco_inicio(resto) } else { tirar_espaco_fim(resto) }
    } else {
        let vizinho = if locale.currency_before { resto.chars().next() } else { resto.chars().next_back() };
        (!vizinho.is_some_and(char::is_whitespace)).then_some(resto)
    }
}

fn tirar_prefixo_sem_caixa<'a>(texto: &'a str, prefixo: &str) -> Option<&'a str> {
    let encontrado = texto.get(..prefixo.len())?;
    encontrado.eq_ignore_ascii_case(prefixo).then(|| &texto[prefixo.len()..])
}

fn tirar_sufixo_sem_caixa<'a>(texto: &'a str, sufixo: &str) -> Option<&'a str> {
    let inicio = texto.len().checked_sub(sufixo.len())?;
    texto.get(inicio..)?.eq_ignore_ascii_case(sufixo).then(|| &texto[..inicio])
}

fn espaco_de_moeda(c: char) -> bool { matches!(c, ' ' | '\u{a0}' | '\u{202f}') }

fn tirar_espaco_inicio(texto: &str) -> Option<&str> {
    espaco_de_moeda(texto.chars().next()?).then(|| texto.trim_start_matches(espaco_de_moeda))
}

fn tirar_espaco_fim(texto: &str) -> Option<&str> {
    espaco_de_moeda(texto.chars().next_back()?).then(|| texto.trim_end_matches(espaco_de_moeda))
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
    use crate::celulas::locale::LocaleConfig;

    fn numero(raw: &str, n: f64, formato: Option<NumberFormat>) {
        let lido = parse_literal(raw);
        assert_eq!(lido.value, Value::Number(n), "{raw}");
        assert_eq!(lido.format, formato, "{raw}");
    }

    fn numero_na_locale(cfg: &LocaleConfig, raw: &str, n: f64, formato: Option<NumberFormat>) {
        let lido = parse_literal_com_locale(raw, cfg);
        assert_eq!(lido.value, Value::Number(n), "{}: {raw}", cfg.code);
        assert_eq!(lido.format, formato, "{}: {raw}", cfg.code);
    }

    fn texto_na_locale(cfg: &LocaleConfig, raw: &str) {
        assert_eq!(parse_literal_com_locale(raw, cfg).value, Value::Text(raw.to_owned()), "{}: {raw}", cfg.code);
    }

    #[test]
    fn numeros_moedas_e_porcentagens_respeitam_locale() {
        let en = LocaleConfig::en_us();
        texto_na_locale(&en, "12,34");
        texto_na_locale(&en, "1,23");
        numero_na_locale(&en, "1,234", 1234.0, None);
        numero_na_locale(&en, "12,345", 12345.0, None);
        numero_na_locale(&en, "1,234.50", 1234.5, None);
        numero_na_locale(&en, "12.34", 12.34, None);
        numero_na_locale(&en, "$1,234.50", 1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&en, "1,234.50%", 12.345, Some(NumberFormat::Percent));
        numero_na_locale(&en, "15.5%", 0.155, Some(NumberFormat::Percent));
        numero_na_locale(&en, "12,34,567", 1_234_567.0, None);
        numero_na_locale(&en, "12,34,567.50", 1_234_567.5, None);
        numero_na_locale(&en, "1.234", 1.234, None);

        let pt = LocaleConfig::pt_br();
        numero_na_locale(&pt, "12,34", 12.34, None);
        numero_na_locale(&pt, "1.234", 1234.0, None);
        numero_na_locale(&pt, "1,234", 1.234, None);
        numero_na_locale(&pt, "1.234,50", 1234.5, None);
        numero_na_locale(&pt, "R$ 12,34", 12.34, Some(NumberFormat::Currency));
        numero_na_locale(&pt, "R$ 1.234,50", 1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&pt, "12,34%", 0.1234, Some(NumberFormat::Percent));
        numero_na_locale(&pt, "15,5%", 0.155, Some(NumberFormat::Percent));

        let es = LocaleConfig::es_es();
        numero_na_locale(&es, "1.234,50", 1234.5, None);
        numero_na_locale(&es, "1.234,50 €", 1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&es, "1.234,50%", 12.345, Some(NumberFormat::Percent));
        numero_na_locale(&es, "15,5%", 0.155, Some(NumberFormat::Percent));

        let fr = LocaleConfig::fr_fr();
        numero_na_locale(&fr, "1 234,50", 1234.5, None);
        numero_na_locale(&fr, "1\u{202f}234,50", 1234.5, None);
        numero_na_locale(&fr, "1 234,50 €", 1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&fr, "1 234,50%", 12.345, Some(NumberFormat::Percent));
        numero_na_locale(&fr, "15,5%", 0.155, Some(NumberFormat::Percent));
    }

    #[test]
    fn parser_localizado_valida_formato_inteiro() {
        let en = LocaleConfig::en_us();
        numero_na_locale(&en, "-$1,234.50", -1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&en, "($1,234.50)", -1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&en, "$-1,234.50", -1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&en, "1,234.567890123", 1234.567890123, None);
        numero_na_locale(&en, "-1.25e-3", -0.00125, None);
        numero_na_locale(&en, "2E+3", 2000.0, None);
        texto_na_locale(&en, "12,34");
        texto_na_locale(&en, "1,2,345");
        texto_na_locale(&en, "1,234.5.0");
        texto_na_locale(&en, "$ 1,234.50");
        texto_na_locale(&en, "€1,234.50");
        texto_na_locale(&en, "1,234.50$");
        texto_na_locale(&en, "1,2,3%");
        texto_na_locale(&en, "1e");

        let pt = LocaleConfig::pt_br();
        numero_na_locale(&pt, "-R$ 1.234,50", -1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&pt, "R$ -1.234,50", -1234.5, Some(NumberFormat::Currency));
        numero_na_locale(&pt, "1,5e2", 150.0, None);
        numero_na_locale(&pt, "1,234,567.89", 1_234_567.89, None);
        texto_na_locale(&pt, "R$1.234,50");
        texto_na_locale(&pt, "$ 1.234,50");

        let es = LocaleConfig::es_es();
        numero_na_locale(&es, "-1.234,50 €", -1234.5, Some(NumberFormat::Currency));
        texto_na_locale(&es, "€ 1.234,50");

        let fr = LocaleConfig::fr_fr();
        numero_na_locale(&fr, "(1\u{a0}234,50 €)", -1234.5, Some(NumberFormat::Currency));
        texto_na_locale(&fr, "1 23,50");
        texto_na_locale(&fr, "1.234,50");
    }

    #[test]
    fn agrupamento_invalido_nao_vira_decimal_em_numeros_moedas_e_porcentagens() {
        let en = LocaleConfig::en_us();
        for raw in ["1,23", "12,34", "1,23.45", "$1,23", "$12,34", "$1,23.45", "1,23%", "12,34%", "1,23.45%"] {
            texto_na_locale(&en, raw);
        }

        let pt = LocaleConfig::pt_br();
        for raw in ["1.23", "12.34", "1.23,45", "R$ 1.23", "R$ 12.34", "R$ 1.23,45", "1.23%", "12.34%", "1.23,45%"] {
            texto_na_locale(&pt, raw);
        }

        let es = LocaleConfig::es_es();
        for raw in ["1.23", "12.34", "1.23,45", "1.23 €", "12.34 €", "1.23,45 €", "1.23%", "12.34%", "1.23,45%"] {
            texto_na_locale(&es, raw);
        }

        let fr = LocaleConfig::fr_fr();
        for raw in ["1 23", "12 34", "1.234,50", "1 23 €", "12 34 €", "1.234,50 €", "1 23%", "12 34%", "1.234,50%"] {
            texto_na_locale(&fr, raw);
        }
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
        numero_na_locale(&LocaleConfig::en_us(), "$50.25", 50.25, Some(NumberFormat::Currency));
        numero("15,5%", 0.155, Some(NumberFormat::Percent));
        assert_eq!(parse_literal("50 R$").value, Value::Text("50 R$".into()));
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
