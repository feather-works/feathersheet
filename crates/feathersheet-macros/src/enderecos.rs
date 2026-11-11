//! Conversão das referências de macro de src/macros/enderecos.ts.
//! Sem expressão regular por célula: varredura linear do texto da fórmula.
//! A API pública da macro usa linhas e colunas 1-based; internamente é 0-based.
use feathersheet_core::{
    celulas::refs::{col_name, MAX_COLS, MAX_ROWS},
};

/// Desloca somente eixos relativos em A1; preserva aspas duplas,
/// nomes de função, nome de aba e coordenadas absolutas com `$`.
pub fn deslocar_formula(formula: &str, dl: i64, dc: i64) -> String {
    use feathersheet_core::operacoes::fill::shift_formula;
    // O deslocamento da alça de preenchimento e da macro é o MESMO.
    // Satura apenas para caber no parâmetro i32 do motor; qualquer
    // referência fora da grade acaba convertida para #REF!.
    let dr = dl.clamp(i64::from(i32::MIN),i64::from(i32::MAX)) as i32;
    let dc = dc.clamp(i64::from(i32::MIN),i64::from(i32::MAX)) as i32;
    shift_formula(formula,dr,dc)
}

#[derive(Debug, Clone, Copy)]
struct Eixo {
    indice: i64,
    absoluto: bool,
}

/// Eixo R1C1: RC herda posição atual; R[-2] é relativo;
/// R42 é absoluto. Não aceita índices negativos ou zero.
fn eixo(bytes: &[u8], cursor: &mut usize, atual: i64) -> Option<Eixo> {
    if *cursor >= bytes.len() {
        return Some(Eixo { indice: atual, absoluto: false });
    }
    if bytes[*cursor] == b'[' {
        let mut fim = *cursor + 1;
        if fim < bytes.len() && bytes[fim] == b'-' { fim += 1; }
        let inicio = fim;
        while fim < bytes.len() && bytes[fim].is_ascii_digit() { fim += 1; }
        if fim == inicio || fim >= bytes.len() || bytes[fim] != b']' { return None; }
        let deslocamento = std::str::from_utf8(&bytes[*cursor+1..fim])
            .ok()?.parse::<i64>().ok()?;
        *cursor = fim + 1;
        return Some(Eixo { indice: atual.checked_add(deslocamento)?,
            absoluto: false });
    }
    if bytes[*cursor].is_ascii_digit() {
        let inicio = *cursor;
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_digit() {
            *cursor += 1;
        }
        return Some(Eixo {
            indice: std::str::from_utf8(&bytes[inicio..*cursor]).ok()?
                .parse::<i64>().ok()?,
            absoluto: true,
        });
    }
    Some(Eixo { indice: atual, absoluto: false })
}

fn nome_caractere(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.')
}

/// Parte de uma referência R1C1: `RC` (célula), `R` (linha inteira) ou `C`
/// (coluna inteira). Um eixo ausente significa "inteira".
struct Parte {
    fim: usize,
    linha: Option<Eixo>,
    coluna: Option<Eixo>,
}

fn parte(bytes: &[u8], inicio: usize, l: i64, c: i64) -> Option<Parte> {
    if !matches!(bytes.get(inicio), Some(b'R') | Some(b'C'))
        || (inicio > 0 && nome_caractere(bytes[inicio-1])) {
        return None;
    }
    let mut pos = inicio;
    let mut linha = None;
    let mut coluna = None;
    if bytes[pos] == b'R' {
        pos += 1;
        linha = Some(eixo(bytes, &mut pos, l)?);
    }
    if bytes.get(pos) == Some(&b'C') {
        pos += 1;
        coluna = Some(eixo(bytes, &mut pos, c)?);
    }
    Some(Parte { fim: pos, linha, coluna })
}

fn fim_valido(bytes: &[u8], pos: usize) -> bool {
    !bytes.get(pos).is_some_and(|b| nome_caractere(*b) || *b == b'(')
}

fn linha_valida(e: Eixo) -> bool { e.indice >= 1 && e.indice <= i64::from(MAX_ROWS) }
fn coluna_valida(e: Eixo) -> bool { e.indice >= 1 && e.indice <= i64::from(MAX_COLS) }
fn txt_linha(e: Eixo) -> String {
    format!("{}{}", if e.absoluto { "$" } else { "" }, e.indice)
}
fn txt_coluna(e: Eixo) -> String {
    format!("{}{}", if e.absoluto { "$" } else { "" }, col_name((e.indice - 1) as u32))
}

/// Uma ponta já validada: célula, linha inteira ou coluna inteira.
enum Ponta { Celula(String), Linha(String), Coluna(String) }

fn ponta(p: &Parte) -> Option<Ponta> {
    match (p.linha, p.coluna) {
        (Some(l), Some(c)) => {
            if !linha_valida(l) || !coluna_valida(c) { return None; }
            Some(Ponta::Celula(format!("{}{}", txt_coluna(c), txt_linha(l))))
        }
        (Some(l), None) => linha_valida(l).then(|| Ponta::Linha(txt_linha(l))),
        (None, Some(c)) => coluna_valida(c).then(|| Ponta::Coluna(txt_coluna(c))),
        (None, None) => None,
    }
}

/// Identifica uma referência R1C1 inteira (célula, linha/coluna inteira ou
/// intervalo entre elas), com limites de palavra, sem confundir com nomes de
/// funções. Retorna o fim do token e o texto A1 (`#REF!` se fora da grade).
fn token(bytes: &[u8], inicio: usize, l: i64, c: i64) -> Option<(usize, String)> {
    let p1 = parte(bytes, inicio, l, c)?;
    // Tenta o intervalo `ref:ref`.
    if bytes.get(p1.fim) == Some(&b':') {
        if let Some(p2) = parte(bytes, p1.fim + 1, l, c) {
            if fim_valido(bytes, p2.fim) {
                let texto = match (ponta(&p1), ponta(&p2)) {
                    (Some(Ponta::Celula(a)), Some(Ponta::Celula(b)))
                    | (Some(Ponta::Linha(a)), Some(Ponta::Linha(b)))
                    | (Some(Ponta::Coluna(a)), Some(Ponta::Coluna(b))) => format!("{a}:{b}"),
                    _ => "#REF!".into(),
                };
                return Some((p2.fim, texto));
            }
        }
    }
    if !fim_valido(bytes, p1.fim) { return None; }
    let texto = match ponta(&p1) {
        Some(Ponta::Celula(a)) => a,
        Some(Ponta::Linha(a)) => format!("{a}:{a}"),
        Some(Ponta::Coluna(a)) => format!("{a}:{a}"),
        None => "#REF!".into(),
    };
    Some((p1.fim, texto))
}

/// Copia um nome de aba entre apóstrofos (`'a''b'!`) sem interpretar o conteúdo.
fn copiar_apostrofos(formula: &str, i: usize, saida: &mut String) -> usize {
    let bytes = formula.as_bytes();
    let mut j = i + 1;
    while j < bytes.len() {
        if bytes[j] == b'\'' {
            if bytes.get(j+1) == Some(&b'\'') { j += 2; continue; }
            j += 1;
            break;
        }
        j += 1;
    }
    saida.push_str(&formula[i..j]);
    j
}

/// Traduz uma fórmula Excel R1C1 para A1 na célula (linha, coluna) 1-based.
/// Referências em literais de texto, inclusive `""` escapadas, não mudam.
/// `C3:C13` vira `$C:$M`, `R5:R9` vira `$5:$9`; `C3` sozinha vira `$C:$C`.
pub fn r1c1_para_a1(formula: &str, linha: u32, coluna: u32) -> String {
    if !formula.starts_with('=') { return formula.into(); }
    let bytes = formula.as_bytes();
    let mut i = 0usize;
    let mut texto = false;
    let mut saida = String::with_capacity(formula.len() + 8);
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if texto && bytes.get(i+1) == Some(&b'"') {
                saida.push_str("\"\"");
                i += 2;
                continue;
            }
            texto = !texto;
            saida.push('"');
            i += 1;
            continue;
        }
        if !texto {
            if bytes[i] == b'\'' {
                i = copiar_apostrofos(formula, i, &mut saida);
                continue;
            }
            if let Some((fim, celula)) = token(bytes, i, i64::from(linha), i64::from(coluna)) {
                saida.push_str(&celula);
                i = fim;
                continue;
            }
        }
        // Índices não-ASCII nunca são divididos entre bytes UTF-8.
        let caractere = formula[i..].chars().next().expect("cursor valido");
        saida.push(caractere);
        i += caractere.len_utf8();
    }
    saida
}

// ---- A1 -> R1C1 ----

enum PontaA1 {
    Celula { col: i64, col_abs: bool, lin: i64, lin_abs: bool },
    Coluna { col: i64, abs: bool },
    Linha { lin: i64, abs: bool },
}

fn ponta_a1(texto: &str) -> Option<PontaA1> {
    let b = texto.as_bytes();
    let mut i = 0;
    let col_abs = b.get(i) == Some(&b'$');
    if col_abs { i += 1; }
    let ini_letras = i;
    while i < b.len() && b[i].is_ascii_alphabetic() { i += 1; }
    let letras = &texto[ini_letras..i];
    let lin_abs = i < b.len() && b[i] == b'$';
    if lin_abs { i += 1; }
    let ini_num = i;
    while i < b.len() && b[i].is_ascii_digit() { i += 1; }
    let numeros = &texto[ini_num..i];
    if i != b.len() { return None; }
    let col = if letras.is_empty() { None } else {
        if letras.len() > 3 { return None; }
        let v = letras.bytes().fold(0i64, |a, x| a * 26 + i64::from(x.to_ascii_uppercase() - b'A' + 1));
        (v >= 1 && v <= i64::from(MAX_COLS)).then_some(v)
    };
    let lin = if numeros.is_empty() { None } else {
        let v: i64 = numeros.parse().ok()?;
        (v >= 1 && v <= i64::from(MAX_ROWS)).then_some(v)
    };
    match (letras.is_empty(), numeros.is_empty()) {
        (false, false) => Some(PontaA1::Celula {
            col: col?, col_abs, lin: lin?, lin_abs }),
        (false, true) if !lin_abs => Some(PontaA1::Coluna { col: col?, abs: col_abs }),
        // `$5`: o cifrão inicial pertence à linha.
        (true, false) if !lin_abs => Some(PontaA1::Linha { lin: lin?, abs: col_abs }),
        _ => None,
    }
}

fn eixo_r1c1(letra: char, valor: i64, absoluto: bool, atual: i64) -> String {
    if absoluto { format!("{letra}{valor}") }
    else if valor == atual { letra.to_string() }
    else { format!("{letra}[{}]", valor - atual) }
}

fn ponta_para_r1c1(p: &PontaA1, l: i64, c: i64) -> String {
    match *p {
        PontaA1::Celula { col, col_abs, lin, lin_abs } =>
            format!("{}{}", eixo_r1c1('R', lin, lin_abs, l), eixo_r1c1('C', col, col_abs, c)),
        PontaA1::Coluna { col, abs } => eixo_r1c1('C', col, abs, c),
        PontaA1::Linha { lin, abs } => eixo_r1c1('R', lin, abs, l),
    }
}

/// Traduz uma fórmula A1 para R1C1 relativa à célula (linha, coluna) 1-based.
/// Inversa de `r1c1_para_a1`: `$C:$M` vira `C3:C13`, `$5:$9` vira `R5:R9`.
/// Textos entre aspas, nomes de aba e nomes de função não mudam.
pub fn a1_para_r1c1(formula: &str, linha: u32, coluna: u32) -> String {
    if !formula.starts_with('=') { return formula.into(); }
    let bytes = formula.as_bytes();
    let (l, c) = (i64::from(linha), i64::from(coluna));
    let mut i = 0usize;
    let mut texto = false;
    let mut saida = String::with_capacity(formula.len() + 8);
    let ref_char = |b: u8| b.is_ascii_alphanumeric() || b == b'$' || b == b'_' || b == b'.';
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if texto && bytes.get(i+1) == Some(&b'"') {
                saida.push_str("\"\"");
                i += 2;
                continue;
            }
            texto = !texto;
            saida.push('"');
            i += 1;
            continue;
        }
        if !texto {
            if bytes[i] == b'\'' {
                i = copiar_apostrofos(formula, i, &mut saida);
                continue;
            }
            if ref_char(bytes[i]) && (i == 0 || !ref_char(bytes[i-1])) {
                let mut fim = i;
                while fim < bytes.len() && ref_char(bytes[fim]) { fim += 1; }
                let nao_funcao = |p: usize| !matches!(bytes.get(p), Some(b'(') | Some(b'!'));
                if let Some(p1) = ponta_a1(&formula[i..fim]).filter(|_| nao_funcao(fim)) {
                    // Intervalo?
                    if bytes.get(fim) == Some(&b':') {
                        let mut f2 = fim + 1;
                        while f2 < bytes.len() && ref_char(bytes[f2]) { f2 += 1; }
                        if let Some(p2) = ponta_a1(&formula[fim+1..f2]).filter(|_| nao_funcao(f2)) {
                            saida.push_str(&ponta_para_r1c1(&p1, l, c));
                            saida.push(':');
                            saida.push_str(&ponta_para_r1c1(&p2, l, c));
                            i = f2;
                            continue;
                        }
                    }
                    // Coluna/linha soltas só existem em intervalos.
                    if matches!(p1, PontaA1::Celula { .. }) {
                        saida.push_str(&ponta_para_r1c1(&p1, l, c));
                        i = fim;
                        continue;
                    }
                }
                saida.push_str(&formula[i..fim]);
                i = fim;
                continue;
            }
        }
        let caractere = formula[i..].chars().next().expect("cursor valido");
        saida.push(caractere);
        i += caractere.len_utf8();
    }
    saida
}

/// Retorno da máscara numérica Excel compatível com o modelo da planilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mascara {
    pub formato: feathersheet_core::celulas::style::NumberFormat,
    pub casas: Option<u8>,
    /// Código personalizado (dd, mmmm/yyyy…) quando a máscara não é uma das conhecidas.
    pub codigo: Option<String>,
}

/// Conversão das máscaras aceitas por `mascaraParaEstilo` em
/// src/macros/enderecos.ts. Máscaras desconhecidas produzem erro explícito.
pub fn mascara_para_estilo(valor: &str) -> Result<Mascara, String> {
    use feathersheet_core::celulas::style::NumberFormat as F;
    let mascara = valor.trim();
    let lower = mascara.to_ascii_lowercase();
    let (formato, casas) = if mascara.is_empty() || lower == "general" || lower == "geral" {
        (F::Auto,None)
    } else if mascara == "@" {
        (F::Text,None)
    } else if lower == "[h]:mm" || lower == "[h]:mm:ss" {
        (F::Duration,None)
    } else if let Some(n) = mascara.strip_suffix('%').and_then(|x| casas_simples(x, "0")) {
        (F::Percent,Some(n))
    } else if let Some((mantissa,exp)) = lower.split_once("e+") {
        if !exp.is_empty() && exp.bytes().all(|b|b == b'0') {
            match casas_simples(mantissa, "0") {
                Some(n) => (F::Scientific,Some(n)),
                None => return Err(feathersheet_core::trf!("Máscara não suportada: {valor}")),
            }
        } else { return Err(feathersheet_core::trf!("Máscara não suportada: {valor}")); }
    } else if ["dd/mm/yyyy hh:mm","dd/mm/yyyy hh:mm:ss",
        "dd-mm-yyyy hh:mm","dd-mm-yyyy hh:mm:ss"].contains(&lower.as_str()) {
        (F::Datetime,None)
    } else if ["dd/mm/yyyy","dd-mm-yyyy","yyyy-mm-dd","yyyy/mm/dd",
        "d/m/yy","d/m/yyyy"].contains(&lower.as_str()) {
        (F::Date,None)
    } else if ["hh:mm", "h:mm", "hh:mm:ss", "h:mm:ss",
        "hh:mm am/pm", "hh:mm:ss am/pm"].contains(&lower.as_str()) {
        (F::Time,None)
    } else if let Some((moeda, digits)) = ["R$", "$", "€", "£"].iter()
        .find_map(|moeda| {
            let entre_aspas = format!("\"{moeda}\"");
            mascara.strip_prefix(*moeda)
                .or_else(|| mascara.strip_prefix(entre_aspas.as_str()))
                .and_then(|x| x.trim_start().strip_prefix("#,##0"))
                .and_then(casas_decimais)
                .map(|d| (*moeda, d))
        }) {
        // Outra moeda que não o real: o código fica, senão a célula mostraria "R$".
        if moeda != "R$" && codigo_personalizado_valido(mascara) {
            return Ok(Mascara { formato: F::Currency, casas: Some(digits), codigo: Some(mascara.to_string()) });
        }
        (F::Currency,Some(digits))
    } else if mascara == "0" {
        (F::Integer,None)
    } else if let Some(resto) = mascara.strip_prefix("#,##0") {
        (F::Number,Some(casas_decimais(resto)
            .ok_or_else(||feathersheet_core::trf!("Máscara não suportada: {valor}"))?))
    } else if let Some(n) = casas_simples(mascara,"0") {
        (F::Number,Some(n))
    } else {
        use feathersheet_core::celulas::formato_personalizado::formatar;
        use feathersheet_core::celulas::values::Value;
        if codigo_personalizado_valido(mascara)
            && formatar(&Value::Number(45000.0), mascara).is_some() {
            return Ok(Mascara { formato: F::Auto, casas: None, codigo: Some(mascara.to_string()) });
        }
        return Err(feathersheet_core::trf!("Máscara de número não suportada: \"{valor}\""));
    };
    Ok(Mascara { formato, casas, codigo: None })
}

/// A máscara do Excel que a célula tem (o `Range.NumberFormat` lido): o código gravado,
/// senão a máscara equivalente ao formato.
pub(crate) fn mascara_do_estilo(s: &feathersheet_core::celulas::style::Style) -> String {
    use feathersheet_core::celulas::style::NumberFormat as F;
    if let Some(c) = &s.code { return c.clone(); }
    let casas = |padrao: u8| { let n = s.decimals.unwrap_or(padrao); if n == 0 { String::new() } else { format!(".{}", "0".repeat(n.into())) } };
    match s.format {
        F::Auto => "General".into(),
        F::Text => "@".into(),
        F::Integer => "0".into(),
        F::Number => format!("#,##0{}", casas(2)),
        F::Currency => format!("R$ #,##0{}", casas(2)),
        F::Accounting => format!("_-R$ * #,##0{}_-", casas(2)),
        F::Percent => format!("0{}%", casas(0)),
        F::Scientific => format!("0{}E+00", casas(2)),
        F::Date => "dd/mm/yyyy".into(),
        F::Datetime => "dd/mm/yyyy hh:mm".into(),
        F::Time => "hh:mm:ss".into(),
        F::Duration => "[h]:mm:ss".into(),
    }
}

/// Valida um código personalizado: fora de aspas/colchetes/`\x` só valem os
/// códigos de número e data do Excel, separadores comuns e AM/PM, e precisa
/// haver ao menos um código (senão seria só texto fixo, ex.: erro de digitação).
fn codigo_personalizado_valido(codigo: &str) -> bool {
    let mut tem_codigo = false;
    let mut it = codigo.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' => { if !it.by_ref().any(|d| d == '"') { return false; } }
            '[' => { if !it.by_ref().any(|d| d == ']') { return false; } }
            '\\' | '_' | '*' => { if it.next().is_none() { return false; } }
            '0' | '#' | '?' | '%' | '@' | 'y' | 'Y' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' | 'm' | 'M' => tem_codigo = true,
            '.' | ',' => {}
            'E' | 'e' if matches!(it.peek(), Some('+') | Some('-')) => { it.next(); tem_codigo = true; }
            'A' | 'a' => {
                let resto: String = it.clone().take(4).collect::<String>().to_uppercase();
                if resto.starts_with("M/PM") { for _ in 0..4 { it.next(); } tem_codigo = true; }
                else if resto.starts_with('/') && resto[1..].starts_with('P') { for _ in 0..2 { it.next(); } tem_codigo = true; }
                else { return false; }
            }
            c if c.is_ascii_alphabetic() => return false,
            _ => {} // espaço, : / - ( ) ; $ € £ + etc.
        }
    }
    tem_codigo
}

fn casas_decimais(resto: &str) -> Option<u8> {
    if resto.is_empty() { return Some(0); }
    let decimais = resto.strip_prefix('.')?;
    if decimais.is_empty() || !decimais.bytes().all(|b|b == b'0') {
        return None;
    }
    u8::try_from(decimais.len()).ok()
}

fn casas_simples(valor: &str, base: &str) -> Option<u8> {
    let resto = valor.strip_prefix(base)?;
    casas_decimais(resto)
}

/// Coluna Excel base 1, número ou texto A:Z, AA:XFD.
pub fn coluna_texto(valor: &str) -> Result<u32, String> {
    feathersheet_core::celulas::refs::col_index(valor.trim())
        .ok_or_else(||feathersheet_core::trf!("Coluna inválida: {valor}"))
}
pub fn coluna_numero(valor: u32) -> Result<u32, String> {
    if valor == 0 || valor > MAX_COLS { Err(feathersheet_core::trf!("Coluna inválida: {valor}")) }
    else { Ok(valor - 1) }
}
pub fn linha_numero(valor: u32) -> Result<u32, String> {
    if valor == 0 || valor > MAX_ROWS { Err(feathersheet_core::trf!("Linha inválida: {valor}")) }
    else { Ok(valor - 1) }
}
/// RGB VBA: `r + g * 256 + b * 65536`, representação numérica BGR.
pub fn rgb_vba(r: u8, g: u8, b: u8) -> u32 {
    u32::from(r) + u32::from(g) * 256 + u32::from(b) * 65_536
}
pub fn vba_cor_hex(cor: u32) -> String {
    format!("#{:02x}{:02x}{:02x}", cor & 255, (cor >> 8) & 255, (cor >> 16) & 255)
}
pub fn cor_hex(texto: &str) -> Result<String, String> {
    let cor = texto.trim().strip_prefix('#').ok_or_else(|| feathersheet_core::tr!("Cor precisa começar com #"))?;
    if cor.len() == 3 && cor.bytes().all(|b|b.is_ascii_hexdigit()) {
        return Ok(format!("#{}{}{}{}{}{}", &cor[0..1],&cor[0..1],
            &cor[1..2],&cor[1..2],&cor[2..3],&cor[2..3]).to_lowercase());
    }
    if cor.len() == 6 && cor.bytes().all(|b|b.is_ascii_hexdigit()) {
        return Ok(format!("#{cor}").to_lowercase());
    }
    Err(feathersheet_core::trf!("Cor inválida: {texto}"))
}

/// Interseção/união dos intervalos do módulo TS, sem expandir células.
pub fn intersecao(a: &feathersheet_core::celulas::refs::Rect,
    b: &feathersheet_core::celulas::refs::Rect)
    -> Option<feathersheet_core::celulas::refs::Rect> {
    use feathersheet_core::celulas::refs::Rect;
    let (r0,c0,r1,c1)=(a.r0.max(b.r0),a.c0.max(b.c0),
        a.r1.min(b.r1),a.c1.min(b.c1));
    (r0<=r1 && c0<=c1).then(|| Rect::new(r0,c0,r1,c1))
}
pub fn uniao(a: &feathersheet_core::celulas::refs::Rect,
    b: &feathersheet_core::celulas::refs::Rect)
    -> feathersheet_core::celulas::refs::Rect {
    use feathersheet_core::celulas::refs::Rect;
    Rect::new(a.r0.min(b.r0),a.c0.min(b.c0),
        a.r1.max(b.r1),a.c1.max(b.c1))
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn referencia_relativa_absoluta_e_mesclada() {
        assert_eq!(r1c1_para_a1("=R[-1]C[-2]+R3C4+RC",4,5),
            "=C3+$D$3+E4");
        assert_eq!(r1c1_para_a1("=R[0]C[-1]+R[0]C[-2]",2,3),
            "=B2+A2");
        assert_eq!(r1c1_para_a1("=R[-1]C",1,1),"=#REF!");
    }
    #[test]
    fn nao_altera_textos_aspas_e_nomes() {
        assert_eq!(r1c1_para_a1("=IF(RC=1;\"RC\"\"R1C1\";\"R[2]C\")",4,5),
            "=IF(E4=1;\"RC\"\"R1C1\";\"R[2]C\")");
        assert_eq!(r1c1_para_a1("=SOMA(R1C1;ERROR1C1;R3C4)",5,5),
            "=SOMA($A$1;ERROR1C1;$D$3)");
    }
    #[test]
    fn copia_a1_muda_somente_eixos_relativos() {
        assert_eq!(deslocar_formula("=A1+$B2+C$3+$D$4+\"A1\"",1,2),
            "=C2+$B3+E$3+$D$4+\"A1\"");
        assert_eq!(deslocar_formula("=A1",-1,0),"=#REF!");
    }
    #[test]
    fn mascara_excel_tipo_casas_cor_e_intervalo() {
        use feathersheet_core::celulas::style::NumberFormat as F;
        assert_eq!(mascara_para_estilo("0.00%").unwrap(),Mascara {formato:F::Percent,casas:Some(2),codigo:None});
        assert_eq!(mascara_para_estilo("R$ #,##0.00").unwrap(),Mascara {formato:F::Currency,casas:Some(2),codigo:None});
        assert_eq!(mascara_para_estilo("General").unwrap().formato,F::Auto);
        assert!(mascara_para_estilo("FORMATO_INVENTADO").is_err());
        assert_eq!(vba_cor_hex(rgb_vba(255,0,128)),"#ff0080");
        assert_eq!(cor_hex("#F0A").unwrap(),"#ff00aa");
        let a=feathersheet_core::celulas::refs::Rect::new(0,0,2,2);
        let b=feathersheet_core::celulas::refs::Rect::new(1,1,4,4);
        assert_eq!(intersecao(&a,&b),Some(feathersheet_core::celulas::refs::Rect::new(1,1,2,2)));
        assert_eq!(uniao(&a,&b),feathersheet_core::celulas::refs::Rect::new(0,0,4,4));
    }
    #[test]
    fn colunas_e_linhas_inteiras_em_r1c1() {
        assert_eq!(r1c1_para_a1("=SOMA(C3:C13)",1,1),"=SOMA($C:$M)");
        assert_eq!(r1c1_para_a1("=SOMA(R5:R9)",1,1),"=SOMA($5:$9)");
        assert_eq!(r1c1_para_a1("=SOMA(C3)",1,1),"=SOMA($C:$C)");
        assert_eq!(r1c1_para_a1("=SOMA(R2)",1,1),"=SOMA($2:$2)");
        assert_eq!(r1c1_para_a1("=SOMA(C[1])",2,3),"=SOMA(D:D)");
        assert_eq!(r1c1_para_a1("=SOMA(R[-1])",5,3),"=SOMA(4:4)");
        assert_eq!(r1c1_para_a1("=SOMA(C[-1]:C[1])",2,3),"=SOMA(B:D)");
        assert_eq!(r1c1_para_a1("=SOMA(C:C)",2,3),"=SOMA(C:C)");
        assert_eq!(r1c1_para_a1("=SOMA(R:R)",7,3),"=SOMA(7:7)");
        assert_eq!(r1c1_para_a1("=SOMA(R[1]C:R[3]C)",1,2),"=SOMA(B2:B4)");
        assert_eq!(r1c1_para_a1("=RC+R[1]C+RC[-2]+R3C5+R[2]C[3]",4,5),
            "=E4+E5+C4+$E$3+H6");
        assert_eq!(r1c1_para_a1("=SOMA(R1C1:R2C2)",9,9),"=SOMA($A$1:$B$2)");
        assert_eq!(r1c1_para_a1("=SOMA(C99999)",1,1),"=SOMA(#REF!)");
    }
    #[test]
    fn r1c1_com_outra_aba_e_dentro_de_funcao() {
        assert_eq!(r1c1_para_a1("=VLOOKUP(MAIN!R29C5;inversores!C3:C13;10;0)",1,1),
            "=VLOOKUP(MAIN!$E$29;inversores!$C:$M;10;0)");
        assert_eq!(r1c1_para_a1("=SOMA('Minha Aba'!C3:C13)",1,1),
            "=SOMA('Minha Aba'!$C:$M)");
        assert_eq!(r1c1_para_a1("=CONCATENATE(\"C3:C13\";COUNTIF(R1:R3;\"x\"))",1,1),
            "=CONCATENATE(\"C3:C13\";COUNTIF($1:$3;\"x\"))");
        assert_eq!(r1c1_para_a1("=ROUND(RC;2)+CONCAT(C1)",3,3),"=ROUND(C3;2)+CONCAT($A:$A)");
    }
    #[test]
    fn a1_para_r1c1_volta() {
        assert_eq!(a1_para_r1c1("=SOMA($C:$M)",1,1),"=SOMA(C3:C13)");
        assert_eq!(a1_para_r1c1("=SOMA($5:$9)",1,1),"=SOMA(R5:R9)");
        assert_eq!(a1_para_r1c1("=SOMA(D:D)",2,3),"=SOMA(C[1]:C[1])");
        assert_eq!(a1_para_r1c1("=E4+E5+C4+$E$3+H6",4,5),
            "=RC+R[1]C+RC[-2]+R3C5+R[2]C[3]");
        assert_eq!(a1_para_r1c1("=VLOOKUP(MAIN!$E$29;inversores!$C:$M;10;0)",1,1),
            "=VLOOKUP(MAIN!R29C5;inversores!C3:C13;10;0)");
        assert_eq!(a1_para_r1c1("=CONCATENATE(\"A1\";LOG10(A1);Q1!B2)",1,1),
            "=CONCATENATE(\"A1\";LOG10(RC);Q1!R[1]C[1])");
        let f="=VLOOKUP(MAIN!$E$29;'a b'!$C:$M;10;0)+$5:$9";
        assert_eq!(r1c1_para_a1(&a1_para_r1c1(f,7,7),7,7),f.replace("+$5:$9","+$5:$9"));
    }
    #[test]
    fn limite_excel_e_unicode_nao_travam() {
        assert_eq!(r1c1_para_a1("=\"ação\"&RC",1,1),"=\"ação\"&A1");
        assert_eq!(r1c1_para_a1("=R1C16385",1,1),"=#REF!");
    }
}
