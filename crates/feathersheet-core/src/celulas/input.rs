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
    if let Some(s) = t.strip_suffix('%') {
        // O percentual lê o número como o número puro lê: "1.234%" é 1,234%, como "1.234" é 1,234.
        if let Some(n) = parse_number_str(s) {
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

/// Número digitado ou colado: o separador de milhar da língua sai antes de o texto chegar ao
/// analisador (`1,234` em inglês e `1.234` em português são 1234). Como no Excel, cada separador
/// pede ao menos três dígitos depois dele (`1,2345` é 12345; `12,34` e `1.5` passam como vieram).
/// O analisador não muda com a língua: o arquivo guarda o texto digitado e é lido por ele
/// (ideia do PR #162).
pub fn sem_milhar_digitado(texto: &str, cfg: &super::locale::LocaleConfig) -> Option<String> {
    let (mil, dec) = (cfg.thousands_separator, cfg.decimal_separator);
    // Francês: o milhar é espaço, e o texto copiado de outro programa traz o espaço que não quebra.
    let troca;
    let texto = if mil == ' ' { troca = texto.replace(['\u{a0}', '\u{202f}'], " "); troca.as_str() } else { texto };
    if !texto.contains(mil) { return None; }
    let nucleo = texto.trim().trim_end_matches(['%', ')', ' ']).trim_start_matches(['-', '(']);
    let nucleo = nucleo.strip_prefix(cfg.currency_symbol.as_str()).unwrap_or(nucleo).trim_start();
    let (inteiro, fracao) = nucleo.split_once(dec).unwrap_or((nucleo, "0"));
    let mut grupos = inteiro.split(mil);
    let primeiro = grupos.next()?;
    let tres_ou_mais = so_digitos(primeiro) && grupos.all(|g| g.len() >= 3 && so_digitos(g));
    // "0.169" é decimal de quem digita com ponto (e de dado colado), nunca milhar.
    ((tres_ou_mais || grupos_de_milhar(inteiro, mil)) && primeiro != "0" && so_digitos(fracao)).then(|| texto.replace(mil, ""))
}

/// O número guardado (`1234.5`, ou `1234,5` digitado em português) como a língua o escreve, para
/// a barra de edição: o texto editado passa pelo [`sem_milhar_digitado`], que lê `1234.567` em
/// português como milhar. `None`: não é um número simples (fórmula, texto, data, milhar junto).
pub fn numero_na_lingua(raw: &str, cfg: &super::locale::LocaleConfig) -> Option<String> {
    let num = raw.strip_suffix('%').unwrap_or(raw);
    (numero_simples(num) && num.contains(['.', ','])).then(|| raw.replace(['.', ','], &cfg.decimal_separator.to_string()))
}

/// O separador decimal que uma coluna de texto vindo de fora mostra (colagem, CSV): `Some('.')`
/// ou `Some(',')` quando algum valor só faz sentido de um jeito (`3.14`, `0.5`, `1,234.50`,
/// `1.234.567`); `None` sem sinal nenhum ou com sinal dos dois.
pub fn decimal_da_coluna<'a>(valores: impl Iterator<Item = &'a str>) -> Option<char> {
    let (mut ponto, mut virgula) = (false, false);
    for v in valores {
        let v = v.trim().trim_end_matches('%').trim_start_matches('-');
        let Some(i) = v.rfind(['.', ',']) else { continue };
        let (antes, depois) = (&v[..i], &v[i + 1..]);
        if !so_digitos(depois) || !antes.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b',') { continue; }
        let sep = v.as_bytes()[i] as char;
        let outro = if sep == '.' { ',' } else { '.' };
        let decimal = if antes.contains(sep) {
            // Dois separadores iguais são milhar: o decimal é o outro.
            if !grupos_de_milhar(v, sep) { continue; }
            outro
        } else if antes.contains(outro) {
            if !grupos_de_milhar(antes, outro) { continue; }
            sep
        } else if so_digitos(antes) && (depois.len() != 3 || antes == "0" || antes.len() > 3) {
            sep
        } else { continue };
        if decimal == '.' { ponto = true } else { virgula = true }
    }
    match (ponto, virgula) { (true, false) => Some('.'), (false, true) => Some(','), _ => None }
}

/// O decimal de cada coluna de `linhas` ([`decimal_da_coluna`]).
pub fn decimais_das_colunas(linhas: &[Vec<String>]) -> Vec<Option<char>> {
    let colunas = linhas.iter().map(Vec::len).max().unwrap_or(0);
    (0..colunas).map(|c| decimal_da_coluna(linhas.iter().filter_map(|l| l.get(c)).map(String::as_str))).collect()
}

/// A língua para ler uma coluna cujo decimal é `decimal`: a `padrao` se ela usa esse decimal,
/// senão a de ponto (inglês) ou a de vírgula (português). Sem decimal decidido, a `padrao`.
pub fn lingua_da_coluna(decimal: Option<char>, padrao: Option<&super::locale::LocaleConfig>) -> Option<super::locale::LocaleConfig> {
    use super::locale::LocaleConfig;
    match (decimal, padrao) {
        (Some(d), Some(p)) if p.decimal_separator == d => Some(p.clone()),
        (Some('.'), _) => Some(LocaleConfig::en_us()),
        (Some(_), _) => Some(LocaleConfig::pt_br()),
        (None, p) => p.cloned(),
    }
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
        // Milhar com o espaço que não quebra (número formatado em francês, colado ou de CSV).
        // O espaço comum não vale aqui: "01 310" é código, e o analisador não pode mudar com
        // a língua de quem abre o arquivo. Em francês, a digitação tira o espaço antes.
        if t.contains(' ') { return None; }
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
            ("1,234.50", 1234.5, None), ("12,34,567.50", 1_234_567.5, None), ("12.34", 12.34, None), ("1,2345", 12345.0, None),
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
        // Em português a vírgula é decimal e o milhar é o ponto.
        let pt = LocaleConfig::pt_br();
        for (t, sai) in [("1.234", "1234"), ("-1.234,56", "-1234,56"), ("R$ 1.234.567", "R$ 1234567"), ("1.234%", "1234%")] {
            assert_eq!(sem_milhar_digitado(t, &pt).as_deref(), Some(sai), "{t}");
        }
        for t in ["1,234", "1.5", "12.34", "0.169", "01.02.2025", "192.168.0.7", "=1.234"] {
            assert_eq!(sem_milhar_digitado(t, &pt), None, "{t}");
        }
        // Espanhol é como o português; em francês o milhar é espaço (comum ou o que não quebra).
        assert_eq!(sem_milhar_digitado("1.234,5", &LocaleConfig::es_es()).as_deref(), Some("1234,5"));
        let fr = LocaleConfig::fr_fr();
        for (t, sai) in [("1 234", "1234"), ("1\u{202f}234\u{a0}567,89", "1234567,89"), ("-12 345,5", "-12345,5")] {
            assert_eq!(sem_milhar_digitado(t, &fr).as_deref(), Some(sai), "{t}");
            assert!(matches!(parse_literal(sai).value, Value::Number(_)), "{sai}");
        }
        for t in ["1 23", "12 345 itens", "1.234", "bonjour le monde", "12/11/2026 10:30"] {
            assert_eq!(sem_milhar_digitado(t, &fr), None, "{t}");
        }
        numero("1.234,5%", 12.345, Some(NumberFormat::Percent));
        // No analisador o percentual lê como o número puro: quem digita em português passa antes pela regra acima.
        numero("1.234%", 0.01234, Some(NumberFormat::Percent));
    }

    #[test]
    fn barra_mostra_o_numero_na_lingua_e_a_coluna_diz_o_decimal() {
        use crate::celulas::locale::LocaleConfig;
        let (pt, en) = (LocaleConfig::pt_br(), LocaleConfig::en_us());
        assert_eq!(numero_na_lingua("1234.5", &pt).as_deref(), Some("1234,5"));
        assert_eq!(numero_na_lingua("2,575", &en).as_deref(), Some("2.575"));
        assert_eq!(numero_na_lingua("-12.5%", &pt).as_deref(), Some("-12,5%"));
        for t in ["1234", "=1.5+2", "1.234.567", "1.234,5", "12/11/2026", "v1.5", "1e3"] {
            assert_eq!(numero_na_lingua(t, &pt), None, "{t}");
        }
        // Ida e volta: o que a barra mostra, com um dígito a mais, não vira milhar.
        let mostrado = numero_na_lingua("1234.56", &pt).unwrap() + "7";
        assert_eq!(sem_milhar_digitado(&mostrado, &pt), None);
        assert_eq!(parse_literal(&mostrado).value, Value::Number(1234.567));

        let dec = |v: &[&str]| decimal_da_coluna(v.iter().copied());
        assert_eq!(dec(&["3.142", "3.14"]), Some('.'));
        assert_eq!(dec(&["12,345", "0.5"]), Some('.'));
        assert_eq!(dec(&["1,234.50"]), Some('.'));
        assert_eq!(dec(&["1,234,567"]), Some('.'));
        assert_eq!(dec(&["12345.678"]), Some('.'));
        assert_eq!(dec(&["3,142", "3,14"]), Some(','));
        assert_eq!(dec(&["1.234", "1.234,56"]), Some(','));
        assert_eq!(dec(&["1.234.567"]), Some(','));
        for v in [&["1.234", "5.678"][..], &["1,234"], &["3.14", "3,14"], &["01.02.2025", "192.168.0.7", "abc"], &[]] {
            assert_eq!(dec(v), None, "{v:?}");
        }
        // A coluna com ponto decimal é lida como inglês mesmo com o app em português, e ao contrário.
        let ler = |t: &str, d, padrao: &LocaleConfig| {
            let cfg = lingua_da_coluna(d, Some(padrao)).unwrap();
            parse_literal(&sem_milhar_digitado(t, &cfg).unwrap_or_else(|| t.to_owned())).value
        };
        assert_eq!(ler("3.142", Some('.'), &pt), Value::Number(3.142));
        assert_eq!(ler("12,345", Some('.'), &pt), Value::Number(12345.0));
        assert_eq!(ler("3,142", Some(','), &en), Value::Number(3.142));
        assert_eq!(ler("1.234", Some(','), &en), Value::Number(1234.0));
        assert_eq!(ler("1.234", None, &pt), Value::Number(1234.0));
        assert_eq!(lingua_da_coluna(Some(','), Some(&LocaleConfig::fr_fr())).unwrap().thousands_separator, ' ');
        assert_eq!(lingua_da_coluna(None, None), None);
    }

    #[test]
    fn espaco_comum_nao_e_milhar_no_analisador_em_nenhuma_lingua() {
        // "01 310" de um arquivo em português não vira 1310 quando o app está em francês.
        crate::celulas::locale::set_locale_config(crate::celulas::locale::LocaleConfig::fr_fr());
        let lido = (parse_literal("01 310").value, parse_literal("1 250,50").value, parse_literal("1\u{202f}250,50").value);
        crate::celulas::locale::set_locale_config(crate::celulas::locale::LocaleConfig::pt_br());
        assert_eq!(lido, (Value::Text("01 310".into()), Value::Text("1 250,50".into()), Value::Number(1250.5)));
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
