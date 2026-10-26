//! Formato personalizado de número, no código do Excel (`#.##0,00`,
//! `0" kWp"`, `dd/mm/aaaa`, `;;;`…). O código é guardado como o `.xlsx`
//! grava, em inglês: `,` separa milhar e `.` separa decimal. A saída segue
//! o pt-BR: `1.234,56`.
//!
//! Até quatro seções separadas por `;`: positivo; negativo; zero; texto.
//! Seção vazia não mostra nada — `;;;` esconde a célula. Cada seção aceita
//! cor (`[Vermelho]`/`[Red]`, `[Cor10]`/`[Color10]`), condição (`[>=1000]`),
//! `*x` (repete x até encher a largura — quem desenha enche, ver
//! [`Formatado::preencher`]), `_x` (espaço), frações (`# ?/?`, `# ??/16`),
//! notação científica (`0.00E+00`, `##0.0E+0`), `@`, aspas, `\x` e `%`.
//! O que não der para formatar devolve `None` e quem chama usa o formato comum.
use crate::celulas::values::Value;

/// Marca provisória no texto onde vai o preenchimento do `*`.
const MARCA: char = '\u{E000}';

/// O texto formatado com o que só quem desenha usa.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Formatado {
    /// O texto, sem o preenchimento do `*`.
    pub texto: String,
    /// Cor da seção (`[Vermelho]`, `[Cor10]`), em RGB.
    pub cor: Option<[u8; 3]>,
    /// `*x`: posição (em bytes, dentro de `texto`) e o caractere que se
    /// repete ali até o texto encher a largura da célula.
    pub preencher: Option<(usize, char)>,
}

impl Formatado {
    /// O texto com o `*` repetido até ter `largura` caracteres.
    pub fn preenchido(&self, largura: usize) -> String {
        let Some((pos, c)) = self.preencher else { return self.texto.clone() };
        let falta = largura.saturating_sub(self.texto.chars().count());
        let mut t = String::with_capacity(self.texto.len() + falta * c.len_utf8());
        t.push_str(&self.texto[..pos]);
        t.extend(std::iter::repeat(c).take(falta));
        t.push_str(&self.texto[pos..]);
        t
    }
}

/// As oito cores com nome do Excel, em inglês (como o `.xlsx` grava) e em português.
const CORES: [(&str, &str, [u8; 3]); 8] = [
    ("Black", "Preto", [0, 0, 0]),
    ("Blue", "Azul", [0, 0, 255]),
    ("Cyan", "Ciano", [0, 255, 255]),
    ("Green", "Verde", [0, 255, 0]),
    ("Magenta", "Magenta", [255, 0, 255]),
    ("Red", "Vermelho", [255, 0, 0]),
    ("White", "Branco", [255, 255, 255]),
    ("Yellow", "Amarelo", [255, 255, 0]),
];

/// Paleta padrão de 56 cores do Excel (`[Color1]`…`[Color56]`).
const PALETA: [u32; 56] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF,
    0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080,
    0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF,
    0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000, 0x008080, 0x0000FF,
    0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99,
    0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699, 0x969696,
    0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333,
];

/// Número da cor em `[Cor10]`/`[Color10]` (1 a 56).
fn numero_da_cor(nome: &str) -> Option<usize> {
    let minusculo = nome.to_lowercase();
    let n = minusculo.strip_prefix("color").or_else(|| minusculo.strip_prefix("cor"))?;
    n.trim().parse::<usize>().ok().filter(|n| (1..=56).contains(n))
}

/// A cor de `[nome]`, em português ou inglês, sem diferença de maiúsculas.
fn cor_por_nome(nome: &str) -> Option<[u8; 3]> {
    let nome = nome.trim();
    if let Some((_, _, rgb)) = CORES.iter().find(|(en, pt, _)|
        en.eq_ignore_ascii_case(nome) || pt.eq_ignore_ascii_case(nome)) {
        return Some(*rgb);
    }
    let v = PALETA[numero_da_cor(nome)? - 1];
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// O código como o `.xlsx` grava: nomes de cor em inglês (`[Vermelho]` →
/// `[Red]`, `[Cor10]` → `[Color10]`). O resto passa igual.
pub fn para_xlsx(codigo: &str) -> String {
    let mut saida = String::with_capacity(codigo.len());
    let mut it = codigo.chars();
    let mut aspas = false;
    while let Some(c) = it.next() {
        match c {
            '\\' if !aspas => { saida.push(c); if let Some(d) = it.next() { saida.push(d); } }
            '"' => { aspas = !aspas; saida.push(c); }
            '[' if !aspas => {
                let mut dentro = String::new();
                let mut fechou = false;
                for d in it.by_ref() { if d == ']' { fechou = true; break; } dentro.push(d); }
                let traduzido = CORES.iter().find(|(_, pt, _)| pt.eq_ignore_ascii_case(dentro.trim()))
                    .map(|(en, _, _)| en.to_string())
                    .or_else(|| numero_da_cor(&dentro).map(|n| format!("Color{n}")));
                saida.push('[');
                saida.push_str(traduzido.as_deref().unwrap_or(&dentro));
                if fechou { saida.push(']'); }
            }
            _ => saida.push(c),
        }
    }
    saida
}

/// Separa as seções pelo `;` fora de aspas, colchetes e `\x`.
fn secoes(codigo: &str) -> Vec<String> {
    let mut saida = vec![String::new()];
    let (mut aspas, mut colchete, mut escape) = (false, false, false);
    for c in codigo.chars() {
        let atual = saida.last_mut().expect("sempre há uma seção");
        if escape { atual.push(c); escape = false; continue; }
        match c {
            '\\' if !aspas => { escape = true; atual.push(c); }
            '"' => { aspas = !aspas; atual.push(c); }
            '[' if !aspas => { colchete = true; atual.push(c); }
            ']' if !aspas => { colchete = false; atual.push(c); }
            ';' if !aspas && !colchete => saida.push(String::new()),
            _ => atual.push(c),
        }
    }
    saida
}

/// Pedaço de uma seção: texto fixo ou um código (dígito, data, `@`).
#[derive(Debug, Clone, PartialEq)]
enum Peca {
    Texto(String),
    Codigo(char),
    Condicao(String, f64),
    /// `[Vermelho]`, `[Red]`, `[Cor10]`.
    Cor([u8; 3]),
    /// `*x`: repete x até encher a célula.
    Preencher(char),
    /// `E+`/`E-` da notação científica: a letra como foi escrita e se o `+` aparece.
    Exp(char, bool),
}

fn pecas(secao: &str) -> Vec<Peca> {
    let mut saida = Vec::new();
    let mut it = secao.chars().peekable();
    let texto = |s: &mut Vec<Peca>, t: String| if !t.is_empty() { s.push(Peca::Texto(t)) };
    while let Some(c) = it.next() {
        match c {
            'G' | 'g' if it.clone().take(6).collect::<String>().eq_ignore_ascii_case("eneral") => {
                for _ in 0..6 { it.next(); }
                saida.push(Peca::Codigo('G'));
            }
            '"' => {
                let mut t = String::new();
                for d in it.by_ref() { if d == '"' { break; } t.push(d); }
                texto(&mut saida, t);
            }
            '\\' => { if let Some(d) = it.next() { texto(&mut saida, d.to_string()); } }
            // `_x` reserva a largura de x: um espaço. `*x` repete x até
            // encher a célula: quem desenha enche (`Formatado::preencher`).
            '_' => { it.next(); texto(&mut saida, " ".into()); }
            '*' => { if let Some(d) = it.next() { saida.push(Peca::Preencher(d)); } }
            '[' => {
                let mut dentro = String::new();
                for d in it.by_ref() { if d == ']' { break; } dentro.push(d); }
                // [$R$-416] → "R$"; [>=100] → condição; [Red], [h] etc.
                if let Some(moeda) = dentro.strip_prefix('$') {
                    texto(&mut saida, moeda.split('-').next().unwrap_or("").to_string());
                } else if let Some(cond) = condicao(&dentro) {
                    saida.push(cond);
                } else if let Some(rgb) = cor_por_nome(&dentro) {
                    saida.push(Peca::Cor(rgb));
                } else if dentro.eq_ignore_ascii_case("h") || dentro.eq_ignore_ascii_case("hh") {
                    saida.push(Peca::Codigo('H'));
                }
            }
            '0' | '#' | '?' | '.' | ',' | '%' | '@' | '/' => saida.push(Peca::Codigo(c)),
            'y' | 'Y' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' | 'm' | 'M' => saida.push(Peca::Codigo(c.to_ascii_lowercase())),
            'E' | 'e' if matches!(it.peek(), Some('+') | Some('-')) => {
                let mais = it.next() == Some('+');
                saida.push(Peca::Exp(c, mais));
            }
            'A' | 'a' => {
                // AM/PM e A/P.
                let resto: String = it.clone().take(4).collect();
                if resto.to_uppercase().starts_with("M/PM") { for _ in 0..4 { it.next(); } saida.push(Peca::Codigo('P')); }
                else if resto.to_uppercase().starts_with("/P") { for _ in 0..2 { it.next(); } saida.push(Peca::Codigo('P')); }
                // Sem AM/PM, `a` é o ano do Excel em português (`dd/mm/aaaa`).
                else { saida.push(Peca::Codigo('y')); }
            }
            _ => texto(&mut saida, c.to_string()),
        }
    }
    saida
}

fn condicao(t: &str) -> Option<Peca> {
    for op in [">=", "<=", "<>", ">", "<", "="] {
        if let Some(n) = t.strip_prefix(op) {
            return n.trim().parse::<f64>().ok().map(|v| Peca::Condicao(op.to_string(), v));
        }
    }
    None
}

fn cumpre(op: &str, n: f64, v: f64) -> bool {
    match op { ">=" => n >= v, "<=" => n <= v, "<>" => n != v, ">" => n > v, "<" => n < v, _ => n == v }
}

/// O texto da célula com o código `codigo`, ou `None` para cair no formato comum.
pub fn formatar(valor: &Value, codigo: &str) -> Option<String> {
    formatar_completo(valor, codigo).map(|f| f.texto)
}

/// Texto, cor da seção e preenchimento do `*`; `None` cai no formato comum.
pub fn formatar_completo(valor: &Value, codigo: &str) -> Option<Formatado> {
    let secs: Vec<Vec<Peca>> = secoes(codigo).iter().map(|s| pecas(s)).collect();
    let (texto, secao) = match valor {
        Value::Number(n) => formatar_numero(*n, &secs)?,
        Value::Text(t) => {
            // Texto usa a quarta seção, ou a última se ela tiver @; sem ela, passa igual.
            let secao = secs.last().filter(|s| secs.len() >= 4 || s.contains(&Peca::Codigo('@')))?;
            let texto = secao.iter().map(|p| match p {
                Peca::Texto(x) => x.clone(),
                Peca::Codigo('@') => t.clone(),
                Peca::Preencher(_) => MARCA.to_string(),
                _ => String::new(),
            }).collect();
            (texto, secao)
        }
        _ => return None,
    };
    let cor = secao.iter().find_map(|p| match p { Peca::Cor(c) => Some(*c), _ => None });
    let preencher = texto.find(MARCA).zip(secao.iter().find_map(|p| match p {
        Peca::Preencher(c) => Some(*c), _ => None }));
    Some(Formatado { texto: texto.replace(MARCA, ""), cor, preencher })
}

/// Qual seção formata `n` e se ela ainda escreve o sinal de menos (como o
/// Excel: a segunda seção é a dos negativos e não escreve o menos).
fn escolher_secao(n: f64, secs: &[Vec<Peca>]) -> Option<(usize, bool)> {
    // A última seção com @ (antes da quarta) é a do texto, não de número.
    let mut num = secs.len().min(3);
    if secs.len() < 4 && secs.last().is_some_and(|s| s.contains(&Peca::Codigo('@'))) { num -= 1; }
    if num == 0 { return None; }
    let cond = |i: usize| secs.get(i).filter(|_| i < num).and_then(|s| s.iter().find_map(|p| match p {
        Peca::Condicao(op, v) => Some((op.as_str(), *v)), _ => None }));
    if cond(0).is_some() || cond(1).is_some() {
        let cumpriu = |i| cond(i).is_some_and(|(op, v)| cumpre(op, n, v));
        let i = if cumpriu(0) { 0 }
            else if cumpriu(1) { 1 }
            else if cond(0).is_some() && cond(1).is_some() && num >= 3 { 2 }
            else { 1.min(num - 1) };
        return Some((i, i != 1 && n < 0.0));
    }
    Some(if n < 0.0 && num >= 2 { (1, false) }
        else if n == 0.0 && num >= 3 { (2, false) }
        else { (0, n < 0.0) })
}

fn formatar_numero(n: f64, secs: &[Vec<Peca>]) -> Option<(String, &Vec<Peca>)> {
    if !n.is_finite() { return None; }
    let (indice, com_sinal) = escolher_secao(n, secs)?;
    let secao = &secs[indice];
    // Na seção dos negativos o número vai sem sinal.
    let usar = if com_sinal || n >= 0.0 { n } else { n.abs() };
    // Seção vazia esconde; só cor/condição (`[Red][<=100]`) mostra como Geral.
    if secao.is_empty() { return Some((String::new(), secao)); }
    if secao.iter().all(|p| matches!(p, Peca::Condicao(..) | Peca::Cor(_))) {
        let sinal = if com_sinal { "-" } else { "" };
        return Some((format!("{sinal}{}", crate::celulas::style::format_auto(n.abs())), secao));
    }
    let e_data = secao.iter().any(|p| matches!(p, Peca::Codigo('y' | 'd' | 'h' | 's' | 'H' | 'P' | 'm')));
    let texto = if e_data {
        formatar_data(usar, secao)?
    } else if let Some(ie) = secao.iter().position(|p| matches!(p, Peca::Exp(..))) {
        formatar_cientifico(usar, secao, ie, com_sinal)
    } else if let Some(barra) = barra_de_fracao(secao) {
        formatar_fracao(usar, secao, barra, com_sinal)
    } else {
        formatar_digitos(usar, secao, com_sinal)
    };
    Some((texto, secao))
}

fn e_digito(p: &Peca) -> bool { matches!(p, Peca::Codigo('0' | '#' | '?')) }

/// A `/` de uma fração: com dígito antes e dígito ou número depois.
fn barra_de_fracao(secao: &[Peca]) -> Option<usize> {
    let b = secao.iter().position(|p| *p == Peca::Codigo('/'))?;
    let depois = secao.get(b + 1).is_some_and(|p| e_digito(p)
        || matches!(p, Peca::Texto(t) if t.len() == 1 && t.chars().all(|c| c.is_ascii_digit())));
    (b > 0 && e_digito(&secao[b - 1]) && depois).then_some(b)
}

/// `0.00E+00` e `##0.0E+0` (engenharia: expoente múltiplo dos dígitos inteiros).
fn formatar_cientifico(n: f64, secao: &[Peca], ie: usize, com_sinal: bool) -> String {
    let Peca::Exp(letra, mais) = secao[ie] else { return String::new() };
    let mantissa = &secao[..ie];
    let ponto = mantissa.iter().position(|p| *p == Peca::Codigo('.')).unwrap_or(mantissa.len());
    let k = mantissa[..ponto].iter().filter(|p| e_digito(p)).count().max(1) as i32;
    let casas = mantissa[ponto..].iter().filter(|p| e_digito(p)).count() as i32;
    let x = n.abs();
    let mut e = if x == 0.0 { 0 } else { (x.log10().floor() as i32).div_euclid(k) * k };
    let arredonda = |m: f64| { let f = 10f64.powi(casas); (m * f).round() / f };
    if x > 0.0 {
        // log10 pode errar por um na borda (1000 → 2,999…).
        while x / 10f64.powi(e) < 1.0 { e -= k; }
        while arredonda(x / 10f64.powi(e)) >= 10f64.powi(k) { e += k; }
    }
    let m = if x == 0.0 { 0.0 } else { x / 10f64.powi(e) };
    let mut saida = formatar_digitos(if n < 0.0 { -m } else { m }, mantissa, com_sinal);
    saida.push(letra);
    if e < 0 { saida.push('-'); } else if mais { saida.push('+'); }
    let resto = &secao[ie + 1..];
    let minimo = resto.iter().filter(|p| **p == Peca::Codigo('0')).count();
    let mut escreveu = false;
    for p in resto {
        match p {
            p if e_digito(p) => if !escreveu {
                escreveu = true;
                saida.push_str(&format!("{:0minimo$}", e.unsigned_abs()));
            },
            Peca::Texto(t) => saida.push_str(t),
            Peca::Preencher(_) => saida.push(MARCA),
            Peca::Codigo('%') => saida.push('%'),
            _ => {}
        }
    }
    saida
}

/// A melhor fração com denominador até `maximo` (o menor denominador empata).
fn melhor_fracao(x: f64, maximo: u32) -> (f64, f64) {
    let mut melhor = (x.round(), 1.0, (x - x.round()).abs());
    for d in 2..=maximo.max(1) {
        let d = f64::from(d);
        let num = (x * d).round();
        let erro = (x - num / d).abs();
        if erro < melhor.2 - 1e-12 { melhor = (num, d, erro); }
    }
    (melhor.0, melhor.1)
}

/// `# ?/?`, `# ??/16`, `?/?`, `0/100`: inteiro (opcional), numerador e denominador.
fn formatar_fracao(n: f64, secao: &[Peca], barra: usize, com_sinal: bool) -> String {
    // Denominador: dígitos-código (até quantos dígitos) ou número fixo.
    let mut fim = barra + 1;
    let mut den_codigo = String::new();
    while let Some(p) = secao.get(fim) {
        match p {
            Peca::Codigo(c @ ('0' | '?' | '#')) => den_codigo.push(*c),
            Peca::Texto(t) if t.len() == 1 && t.chars().all(|c| c.is_ascii_digit()) => den_codigo.push_str(t),
            _ => break,
        }
        fim += 1;
    }
    let fixo = den_codigo.chars().any(|c| ('1'..='9').contains(&c))
        .then(|| den_codigo.parse::<f64>().ok()).flatten();
    let mut ini = barra;
    while ini > 0 && e_digito(&secao[ini - 1]) { ini -= 1; }
    let pos_int: Vec<usize> = (0..ini).filter(|&i| e_digito(&secao[i])).collect();
    let x = n.abs();
    let (mut inteiro, resto) = if pos_int.is_empty() { (0.0, x) } else { (x.trunc(), x.fract()) };
    let (mut num, den) = match fixo {
        Some(d) => ((resto * d).round(), d),
        None => melhor_fracao(resto, 10u32.pow(den_codigo.len().clamp(1, 4) as u32) - 1),
    };
    if !pos_int.is_empty() && num >= den { inteiro += 1.0; num -= den; }
    let largura = |a: usize, b: usize| b - a;
    // Cada código vazio vira: '0' → "0", '?' → espaço, '#' → nada.
    let encher = |txt: String, codigos: &[Peca], direita: bool| -> String {
        let falta = codigos.len().saturating_sub(txt.chars().count());
        let enchimento: String = codigos.iter().take(falta).map(|p| match p {
            Peca::Codigo('0') if !direita => "0", Peca::Codigo('#') => "", _ => " " }).collect();
        if direita { txt + &enchimento } else { enchimento + &txt }
    };
    let sem_fracao = num == 0.0 && !pos_int.is_empty();
    let txt_int = if inteiro == 0.0 && num == 0.0 { "0".to_string() }
        else if inteiro == 0.0 { String::new() } else { format!("{inteiro:.0}") };
    let codigos_int: Vec<Peca> = pos_int.iter().map(|&i| secao[i].clone()).collect();
    let txt_int = encher(txt_int, &codigos_int, false);
    let txt_num = encher(format!("{num:.0}"), &secao[ini..barra], false);
    let txt_den = if fixo.is_some() { den_codigo.clone() }
        else { encher(format!("{den:.0}"), &secao[barra + 1..fim], true) };
    let mut saida = String::new();
    for (i, p) in secao.iter().enumerate() {
        if pos_int.first() == Some(&i) { saida.push_str(&txt_int); continue; }
        if pos_int.contains(&i) { continue; }
        if i == ini {
            // Sem parte fracionária: espaços no lugar de "n/d".
            if sem_fracao { saida.push_str(&" ".repeat(largura(ini, barra) + 1 + txt_den.chars().count())); }
            else { saida.push_str(&txt_num); saida.push('/'); saida.push_str(&txt_den); }
            continue;
        }
        if (ini..fim).contains(&i) { continue; }
        match p {
            Peca::Texto(t) => saida.push_str(t),
            Peca::Codigo('%') => saida.push('%'),
            Peca::Preencher(_) => saida.push(MARCA),
            _ => {}
        }
    }
    if com_sinal && n < 0.0 && (inteiro > 0.0 || num > 0.0) {
        let i = saida.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
        saida.insert(i, '-');
    }
    saida
}

fn formatar_digitos(n: f64, secao: &[Peca], com_sinal: bool) -> String {
    let digito = |c: &char| matches!(c, '0' | '#' | '?');
    let codigos: Vec<char> = secao.iter().filter_map(|p| match p { Peca::Codigo(c) => Some(*c), _ => None }).collect();
    // "General" com texto em volta: o número como no formato Geral.
    if codigos.contains(&'G') {
        let numero = crate::celulas::style::format_auto(n.abs());
        let sinal = if com_sinal && n < 0.0 { "-" } else { "" };
        return secao.iter().map(|p| match p {
            Peca::Texto(t) => t.clone(),
            Peca::Codigo('G') => format!("{sinal}{numero}"),
            Peca::Preencher(_) => MARCA.to_string(),
            _ => String::new(),
        }).collect();
    }
    let ponto = secao.iter().position(|p| *p == Peca::Codigo('.'));
    let ultimo = secao.iter().rposition(|p| matches!(p, Peca::Codigo(c) if digito(c)));
    let mut valor = n.abs();
    for _ in codigos.iter().filter(|c| **c == '%') { valor *= 100.0; }
    // Vírgulas logo depois do último dígito dividem por mil.
    if let Some(u) = ultimo {
        let mut k = u + 1;
        while secao.get(k) == Some(&Peca::Codigo(',')) { valor /= 1000.0; k += 1; }
    }
    let antes = |i: usize| ponto.map_or(true, |p| i < p);
    let pos_int: Vec<usize> = (0..secao.len()).filter(|&i| antes(i) && matches!(&secao[i], Peca::Codigo(c) if digito(c))).collect();
    let pos_dec: Vec<usize> = (0..secao.len()).filter(|&i| !antes(i) && matches!(&secao[i], Peca::Codigo(c) if digito(c))).collect();
    let milhar = match (pos_int.first(), pos_int.last()) {
        (Some(&a), Some(&b)) => secao[a..=b].contains(&Peca::Codigo(',')),
        _ => false,
    };
    // Empate arredonda para longe do zero, como o Excel (o `{:.N}` do Rust vai ao par).
    let casas = pos_dec.len();
    let fator = 10f64.powi(casas as i32);
    let texto = format!("{:.*}", casas, (valor * fator).round() / fator);
    let (int_txt, dec_txt) = texto.split_once('.').unwrap_or((&texto, ""));
    let int_txt = int_txt.trim_start_matches('0');
    // O que vai em cada posição de dígito (`None` = nada).
    let mut cheio: std::collections::HashMap<usize, String> = Default::default();
    let vazio = |c: char| match c { '0' => Some("0".to_string()), '?' => Some(" ".to_string()), _ => None };
    if milhar {
        // "#,##0": o número agrupado inteiro vai na primeira posição.
        let minimo = pos_int.iter().filter(|&&i| secao[i] == Peca::Codigo('0')).count();
        let mut t = int_txt.to_string();
        while t.len() < minimo { t.insert(0, '0'); }
        let mut agrupado = String::new();
        for (i, ch) in t.chars().enumerate() {
            if i > 0 && (t.len() - i) % 3 == 0 { agrupado.push('.'); }
            agrupado.push(ch);
        }
        if let Some(&p) = pos_int.first() { cheio.insert(p, agrupado); }
    } else {
        // Da direita para a esquerda; o que sobra entra na primeira posição.
        let mut digs: Vec<char> = int_txt.chars().collect();
        for (k, &p) in pos_int.iter().enumerate().rev() {
            let Peca::Codigo(c) = secao[p] else { continue };
            let atual = if k == 0 { std::mem::take(&mut digs).into_iter().collect::<String>() }
                else { digs.pop().map(String::from).unwrap_or_default() };
            let v = if atual.is_empty() { vazio(c).unwrap_or_default() } else { atual };
            cheio.insert(p, v);
        }
    }
    // Casas: da esquerda para a direita; '#' no fim some quando é zero.
    let dec: Vec<char> = dec_txt.chars().collect();
    let mut corte = dec.len();
    while corte > 0 && dec[corte - 1] == '0' && secao[pos_dec[corte - 1]] != Peca::Codigo('0') { corte -= 1; }
    for (k, &p) in pos_dec.iter().enumerate() {
        let Peca::Codigo(c) = secao[p] else { continue };
        cheio.insert(p, if k < corte { dec[k].to_string() } else { vazio(c).filter(|_| c == '?').unwrap_or_default() });
    }
    let tem_decimal = corte > 0 || pos_dec.iter().any(|&p| secao[p] != Peca::Codigo('#'));
    let mut saida = String::new();
    for (i, p) in secao.iter().enumerate() {
        match p {
            Peca::Texto(t) => saida.push_str(t),
            Peca::Codigo('.') => if tem_decimal { saida.push(',') },
            Peca::Codigo('%') => saida.push('%'),
            Peca::Codigo('/') => saida.push('/'),
            Peca::Codigo(_) => if let Some(v) = cheio.get(&i) { saida.push_str(v) },
            Peca::Preencher(_) => saida.push(MARCA),
            Peca::Condicao(..) | Peca::Cor(_) | Peca::Exp(..) => {}
        }
    }
    let tem_digito = saida.chars().any(|c| c.is_ascii_digit() && c != '0');
    if com_sinal && n < 0.0 && tem_digito {
        // O menos vai antes do primeiro dígito, depois do texto fixo da frente.
        let i = saida.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
        saida.insert(i, '-');
    }
    saida
}

const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho",
    "agosto", "setembro", "outubro", "novembro", "dezembro"];
const DIAS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira",
    "sexta-feira", "sábado"];

fn formatar_data(serial: f64, secao: &[Peca]) -> Option<String> {
    use crate::celulas::date_serial::serial_to_ymd;
    if !(0.0..2_958_466.0).contains(&serial) { return None; }
    let dia_int = serial.floor() as i64;
    let (ano, mes, dia) = serial_to_ymd(dia_int);
    let segundos = ((serial - serial.floor()) * 86_400.0).round() as i64;
    let (hh, mm, ss) = (segundos / 3600 % 24, segundos % 3600 / 60, segundos % 60);
    let ampm = secao.contains(&Peca::Codigo('P'));
    // Agrupa letras iguais seguidas: "dd", "mmmm", "yyyy".
    let mut grupos: Vec<(Option<char>, usize, String)> = Vec::new();
    for p in secao {
        match p {
            Peca::Codigo(c) if matches!(c, 'y' | 'm' | 'd' | 'h' | 's' | 'H' | 'P') => {
                match grupos.last_mut() {
                    Some((Some(u), n, _)) if u == c => *n += 1,
                    _ => grupos.push((Some(*c), 1, String::new())),
                }
            }
            Peca::Texto(t) => grupos.push((None, 0, t.clone())),
            Peca::Codigo(c) => grupos.push((None, 0, c.to_string())),
            Peca::Preencher(_) => grupos.push((None, 0, MARCA.to_string())),
            Peca::Exp(c, mais) => grupos.push((None, 0, format!("{c}{}", if *mais { '+' } else { '-' }))),
            Peca::Condicao(..) | Peca::Cor(_) => {}
        }
    }
    let mut saida = String::new();
    for i in 0..grupos.len() {
        let (c, n, t) = &grupos[i];
        let Some(c) = c else { saida.push_str(t); continue };
        // "m" é minuto logo depois de hora ou logo antes de segundo.
        let vizinho = |passo: isize| -> Option<char> {
            let mut j = i as isize + passo;
            while j >= 0 && (j as usize) < grupos.len() {
                if let Some(v) = grupos[j as usize].0 { return Some(v); }
                j += passo;
            }
            None
        };
        let minuto = *c == 'm' && (matches!(vizinho(-1), Some('h' | 'H')) || vizinho(1) == Some('s'));
        let dois = |v: i64| format!("{v:02}");
        saida.push_str(&match (c, n) {
            ('y', 1..=2) => dois(i64::from(ano.rem_euclid(100))),
            ('y', _) => format!("{ano:04}"),
            ('m', _) if minuto => if *n >= 2 { dois(mm) } else { mm.to_string() },
            ('m', 1) => mes.to_string(),
            ('m', 2) => dois(i64::from(mes)),
            ('m', 3) => MESES[(mes as usize - 1) % 12][..3].to_string(),
            ('m', 4) => MESES[(mes as usize - 1) % 12].to_string(),
            ('m', _) => MESES[(mes as usize - 1) % 12][..1].to_string(),
            ('d', 1) => dia.to_string(),
            ('d', 2) => dois(i64::from(dia)),
            ('d', 3) => DIAS[(dia_int + 6).rem_euclid(7) as usize][..3].to_string(),
            ('d', _) => DIAS[(dia_int + 6).rem_euclid(7) as usize].to_string(),
            ('H', _) => (segundos / 3600 + dia_int * 24).to_string(),
            ('h', _) => {
                let h = if ampm { (hh + 11) % 12 + 1 } else { hh };
                if *n >= 2 { dois(h) } else { h.to_string() }
            }
            ('s', _) => if *n >= 2 { dois(ss) } else { ss.to_string() },
            ('P', _) => if hh < 12 { "AM".into() } else { "PM".into() },
            _ => String::new(),
        });
    }
    Some(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    fn f(n: f64, c: &str) -> String { formatar(&Value::Number(n), c).unwrap() }

    #[test]
    fn tres_pontos_e_virgulas_escondem_tudo() {
        assert_eq!(f(12.5, ";;;"), "");
        assert_eq!(f(-3.0, ";;;"), "");
        assert_eq!(f(0.0, ";;;"), "");
        assert_eq!(formatar(&Value::Text("oi".into()), ";;;").as_deref(), Some(""));
    }

    #[test]
    fn digitos_milhar_casas_e_texto_fixo() {
        assert_eq!(f(1234.5, "#,##0.00"), "1.234,50");
        assert_eq!(f(1234.5, "0"), "1235");
        assert_eq!(f(0.5, "0.0#"), "0,5");
        assert_eq!(f(7.25, "0.00\" kWp\""), "7,25 kWp");
        assert_eq!(f(0.256, "0.0%"), "25,6%");
        assert_eq!(f(1_500_000.0, "#,##0,\" mil\""), "1.500 mil");
        assert_eq!(f(-12.0, "0.00"), "-12,00");
        assert_eq!(f(-12.0, "0.00;(0.00)"), "(12,00)");
        assert_eq!(f(0.0, "0;-0;\"zero\""), "zero");
        assert_eq!(f(1234.0, "[$R$-416] #,##0.00"), "R$ 1.234,00");
    }

    #[test]
    fn formatos_da_planilha_v6() {
        assert_eq!(f(75.0, "0.0\\ \"kVA\""), "75,0 kVA");
        assert_eq!(f(27998887766.0, "\"(\"00\") \"00000\"-\"0000"), "(27) 99888-7766");
        assert_eq!(f(29160000.0, "00\".\"000\"-\"000"), "29.160-000");
        assert_eq!(f(450.0, "General\\ \"kWh/mês\""), "450 kWh/mês");
        assert_eq!(f(7.5, "General\\ \"kWp\""), "7,5 kWp");
        assert_eq!(f(1234.5, "_-\"R$\"\\ * #,##0.00_-;\\-\"R$\"\\ * #,##0.00_-;_-\"R$\"\\ * \"-\"??_-;_-@"), " R$ 1.234,50 ");
        assert_eq!(f(220.0, "0\\ \"V\""), "220 V");
    }

    #[test]
    fn datas_e_horas() {
        // 45.000 = 15/03/2023, quarta-feira.
        assert_eq!(f(45000.0, "dd/mm/yyyy"), "15/03/2023");
        assert_eq!(f(45000.0, "d \"de\" mmmm \"de\" yyyy"), "15 de março de 2023");
        assert_eq!(f(45000.0, "ddd, dd/mm"), "qua, 15/03");
        assert_eq!(f(45000.75, "hh:mm"), "18:00");
        assert_eq!(f(1.5, "[h]:mm"), "36:00");
        assert_eq!(f(45000.75, "h:mm AM/PM"), "6:00 PM");
        assert_eq!(f(45000.0, "dd/mm/aaaa"), "15/03/2023");
        assert_eq!(f(45000.0, "mm/aa"), "03/23");
    }

    #[test]
    fn condicoes_e_texto() {
        assert_eq!(f(150.0, "[>=100]\"alto\";[<100]\"baixo\""), "alto");
        assert_eq!(f(50.0, "[>=100]\"alto\";[<100]\"baixo\""), "baixo");
        assert_eq!(formatar(&Value::Text("ES".into()), "0;0;0;\"UF: \"@").as_deref(), Some("UF: ES"));
        assert_eq!(formatar(&Value::Text("x".into()), "0.00"), None);
    }

    fn completo(n: f64, c: &str) -> Formatado { formatar_completo(&Value::Number(n), c).unwrap() }
    const VERMELHO: Option<[u8; 3]> = Some([255, 0, 0]);
    const AZUL: Option<[u8; 3]> = Some([0, 0, 255]);

    #[test]
    fn cores_da_secao_em_portugues_e_ingles() {
        // Excel: #,##0;[Red]-#,##0 pinta o negativo de vermelho.
        let neg = completo(-1234.0, "#,##0;[Vermelho]-#,##0");
        assert_eq!((neg.texto.as_str(), neg.cor), ("-1.234", VERMELHO));
        assert_eq!(completo(-5.0, "0;[Red]-0").cor, VERMELHO);
        assert_eq!(completo(5.0, "0;[Red]-0").cor, None);
        assert_eq!(completo(5.0, "[Azul]0;[Red]0").cor, AZUL);
        assert_eq!(completo(5.0, "[BLUE]0").cor, AZUL);
        // [Color10] é o verde escuro da paleta padrão.
        assert_eq!(completo(5.0, "[Cor10]0").cor, Some([0, 128, 0]));
        assert_eq!(completo(5.0, "[Color10]0").cor, Some([0, 128, 0]));
        assert_eq!(completo(5.0, "[Color57]0").cor, None);
        assert_eq!(completo(0.0, "0;0;[Amarelo]\"zero\"").cor, Some([255, 255, 0]));
        let t = formatar_completo(&Value::Text("a".into()), "0;0;0;[Verde]@").unwrap();
        assert_eq!((t.texto.as_str(), t.cor), ("a", Some([0, 255, 0])));
    }

    #[test]
    fn condicoes_com_cor_e_escala() {
        let c = "[Vermelho][<=100];[Azul][>100]";
        assert_eq!(completo(50.0, c), Formatado { texto: "50".into(), cor: VERMELHO, preencher: None });
        assert_eq!(completo(150.0, c).cor, AZUL);
        // [>=1000] em milhares, o resto normal.
        let mil = "[>=1000]#,##0,\" mil\";0";
        assert_eq!(f(25_000.0, mil), "25 mil");
        assert_eq!(f(999.0, mil), "999");
        // Três seções com condição nas duas primeiras: a terceira é o resto.
        let tres = "[>=1000]\"alto\";[<0]\"negativo\";0.0";
        assert_eq!(f(5000.0, tres), "alto");
        assert_eq!(f(-1.0, tres), "negativo");
        assert_eq!(f(12.0, tres), "12,0");
        // Telefone: [<=9999999]###-####;(###) ###-####.
        let tel = "[<=9999999]###-####;(###) ###-####";
        assert_eq!(f(5551234.0, tel), "555-1234");
        assert_eq!(f(2135551234.0, tel), "(213) 555-1234");
    }

    #[test]
    fn asterisco_preenche_e_sublinhado_reserva() {
        let r = completo(1234.5, "\"R$\"* #,##0.00");
        assert_eq!(r.texto, "R$1.234,50");
        assert_eq!(r.preencher, Some((2, ' ')));
        assert_eq!(r.preenchido(14), "R$    1.234,50");
        // Texto maior que a largura: não enche nada.
        assert_eq!(r.preenchido(3), "R$1.234,50");
        let pontilhado = completo(5.0, "0*.");
        assert_eq!(pontilhado.preenchido(4), "5...");
        let t = formatar_completo(&Value::Text("Total".into()), "@*-").unwrap();
        assert_eq!(t.preenchido(8), "Total---");
        // `_)` reserva a largura do parêntese, para alinhar com o negativo.
        assert_eq!(f(5.0, "0_);(0)"), "5 ");
        assert_eq!(f(-5.0, "0_);(0)"), "(5)");
    }

    #[test]
    fn fracoes() {
        assert_eq!(f(1.5, "# ?/?"), "1 1/2");
        assert_eq!(f(0.5, "# ?/?"), " 1/2");
        assert_eq!(f(-1.5, "# ?/?"), "-1 1/2");
        assert_eq!(f(2.0, "# ?/?"), "2    ");
        assert_eq!(f(0.25, "?/?"), "1/4");
        assert_eq!(f(1.5, "?/?"), "3/2");
        assert_eq!(f(2.3125, "# ??/16"), "2  5/16");
        assert_eq!(f(0.75, "# ??/16"), " 12/16");
        assert_eq!(f(0.37, "0/100"), "37/100");
        assert_eq!(f(1.14159, "# ??/??"), "1 14/99");
        assert_eq!(f(0.2, "# ??/??"), "  1/5 ");
        // Arredondar para cima vira inteiro: 0,99 com um dígito → 1.
        assert_eq!(f(0.99, "# ?/?"), "1    ");
    }

    #[test]
    fn notacao_cientifica() {
        assert_eq!(f(1234.5, "0.00E+00"), "1,23E+03");
        assert_eq!(f(0.000123, "0.00E+00"), "1,23E-04");
        assert_eq!(f(-1234.5, "0.00E+00"), "-1,23E+03");
        assert_eq!(f(0.0, "0.00E+00"), "0,00E+00");
        assert_eq!(f(99999.0, "0.00E+00"), "1,00E+05");
        assert_eq!(f(1000.0, "0E+0"), "1E+3");
        assert_eq!(f(1234.0, "0.0E-0"), "1,2E3");
        // Engenharia: expoente múltiplo de 3.
        assert_eq!(f(12345.0, "##0.0E+0"), "12,3E+3");
        assert_eq!(f(0.00012, "##0.0E+0"), "120,0E-6");
    }

    #[test]
    fn arroba_aspas_barra_percentual_e_milhar() {
        assert_eq!(formatar(&Value::Text("Ana".into()), "\"Nome: \"@").as_deref(), Some("Nome: Ana"));
        assert_eq!(formatar(&Value::Number(3.0), "\"Nome: \"@"), None);
        // `0.00;@`: a segunda seção é de texto, o negativo usa a primeira com sinal.
        assert_eq!(f(-3.0, "0.00;@"), "-3,00");
        assert_eq!(formatar(&Value::Text("x".into()), "0.00;@").as_deref(), Some("x"));
        assert_eq!(f(5.0, "0\\%"), "5%");
        assert_eq!(f(0.05, "0%"), "5%");
        assert_eq!(f(5.0, "\\$0"), "$5");
        assert_eq!(f(5.0, "\"[x]\"0"), "[x]5");
        assert_eq!(f(1_234_567.0, "#,##0,"), "1.235");
        assert_eq!(f(1_234_567_890.0, "#,##0,,\" mi\""), "1.235 mi");
        assert_eq!(f(2_500_000.0, "0.0,,"), "2,5");
        assert_eq!(f(1234567.891, "#,##0.00"), "1.234.567,89");
    }

    #[test]
    fn combinacoes_do_dia_a_dia() {
        // Contábil do Excel: positivo com espaço do parêntese, negativo vermelho entre parênteses.
        let contabil = "#,##0.00_);[Red](#,##0.00)";
        assert_eq!(completo(1234.5, contabil), Formatado { texto: "1.234,50 ".into(), cor: None, preencher: None });
        assert_eq!(completo(-1234.5, contabil),
            Formatado { texto: "(1.234,50)".into(), cor: VERMELHO, preencher: None });
        // Cor e condição em qualquer ordem dentro da seção.
        let c = "[>=1000][Azul]#,##0;[Vermelho][<0]0;0";
        assert_eq!(completo(2500.0, c), Formatado { texto: "2.500".into(), cor: AZUL, preencher: None });
        assert_eq!(completo(-3.0, c), Formatado { texto: "3".into(), cor: VERMELHO, preencher: None });
        assert_eq!(completo(7.0, c).cor, None);
        // Zero com traço, texto em azul só com @.
        assert_eq!(f(0.0, "0.00;-0.00;\"-\""), "-");
        let t = formatar_completo(&Value::Text("ok".into()), "[Azul]@").unwrap();
        assert_eq!((t.texto.as_str(), t.cor), ("ok", AZUL));
        // Fração sem inteiro, negativa.
        assert_eq!(f(-0.25, "?/?"), "-1/4");
        // `*` com texto fixo dos dois lados enche no meio.
        let r = completo(42.0, "\"Total\"*.0");
        assert_eq!(r.preenchido(10), "Total...42");
    }

    #[test]
    fn xlsx_grava_nome_de_cor_em_ingles() {
        assert_eq!(para_xlsx("[Vermelho]0;[cor10]-0;\"[Azul]\";@"), "[Red]0;[Color10]-0;\"[Azul]\";@");
        assert_eq!(para_xlsx("[Red][>=10]0.0;[$R$-416] 0"), "[Red][>=10]0.0;[$R$-416] 0");
        assert_eq!(para_xlsx("\\[Azul]0"), "\\[Azul]0");
    }
}
