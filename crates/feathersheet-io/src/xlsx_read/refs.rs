//! Referência A1 e deslocamento de fórmula compartilhada. Espelha
//! `src/core/refs.ts` (colName/colIndex/parseCellRef) e
//! `src/core/formula-text.ts` (ParsedRef/formatRef/mapReferences), na parte
//! que `shiftFormula` (`src/io/xlsx.ts:1033-1043`) usa.

pub const MAX_COLS: i64 = 16384;
pub const MAX_ROWS: i64 = 100_000_000;

pub fn col_name(col: i64) -> String {
    let mut name = String::new();
    let mut n = col;
    while n >= 0 {
        let letra = (b'A' + (n % 26) as u8) as char;
        name.insert(0, letra);
        n = n / 26 - 1;
    }
    name
}

/// "A" -> 0, "AA" -> 26. -1 se inválido.
pub fn col_index(name: &str) -> i64 {
    // Mais de 3 letras passa de XFD (e 14 estourariam o i64).
    if name.len() > 3 {
        return -1;
    }
    let mut n: i64 = 0;
    for ch in name.chars() {
        let c = (ch as u32) & !32; // maiúscula
        if !(65..=90).contains(&c) {
            return -1;
        }
        n = n * 26 + (c as i64 - 64);
    }
    n - 1
}

#[derive(Debug, Clone, Copy)]
pub struct CellRef {
    pub row: i64,
    pub col: i64,
}

/// "A1" -> {row, col}, ou nenhum se inválido/fora do alcance. Espelha
/// `parseCellRef` (`core/refs.ts:81-88`).
pub fn parse_cell_ref(text: &str) -> Option<CellRef> {
    let t = text.trim();
    let letras_fim = t.find(|c: char| c.is_ascii_digit())?;
    let (letras, digitos) = t.split_at(letras_fim);
    if letras.is_empty() || !letras.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if digitos.is_empty() || !digitos.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = col_index(letras);
    let row: i64 = digitos.parse().ok()?;
    let row = row - 1;
    if col < 0 || col >= MAX_COLS || row < 0 || row >= MAX_ROWS {
        return None;
    }
    Some(CellRef { row, col })
}

/// Uma referência já separada em linha/coluna + cadeado de `$` por eixo,
/// com aba opcional — igual `ParsedRef` de `core/formula-text.ts:3-9`.
pub struct ParsedRef {
    pub sheet: Option<String>,
    pub col_lock: bool,
    pub row_lock: bool,
    pub col: i64,
    pub row: i64,
}

fn quote_sheet(name: &str) -> String {
    let simples = name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    if simples {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// Monta o texto de uma referência de volta, preservando os cifrões — igual
/// `formatRef(ref: ParsedRef)` (`core/formula-text.ts:12-16`).
pub fn format_ref(r: &ParsedRef) -> String {
    if r.col < 0 || r.row < 0 {
        return "#REF!".to_string();
    }
    let prefix = match &r.sheet {
        Some(s) => format!("{}!", quote_sheet(s)),
        None => String::new(),
    };
    format!(
        "{prefix}{}{}{}{}",
        if r.col_lock { "$" } else { "" },
        col_name(r.col),
        if r.row_lock { "$" } else { "" },
        r.row + 1
    )
}

/// "Plan1!" ou "'Meu Plano'!" a partir da posição `at` (em bytes, texto
/// ASCII-compatível o bastante pro que uma fórmula do Excel usa). Igual
/// `matchSheetPrefix` (`core/formula-text.ts:91-102`).
fn match_sheet_prefix(chars: &[char], at: usize) -> Option<(String, usize)> {
    if at >= chars.len() {
        return None;
    }
    if chars[at] == '\'' {
        let mut i = at + 1;
        let mut nome = String::new();
        while i < chars.len() {
            if chars[i] == '\'' {
                if chars.get(i + 1) == Some(&'\'') {
                    nome.push('\'');
                    i += 2;
                    continue;
                }
                if chars.get(i + 1) == Some(&'!') {
                    return Some((nome, i + 2 - at));
                }
                return None;
            }
            nome.push(chars[i]);
            i += 1;
        }
        return None;
    }
    let ident_char = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    if !(chars[at].is_alphabetic() || chars[at] == '_') {
        return None;
    }
    let mut i = at;
    while i < chars.len() && ident_char(chars[i]) {
        i += 1;
    }
    if chars.get(i) != Some(&'!') {
        return None;
    }
    if at > 0 {
        let anterior = chars[at - 1];
        if anterior.is_alphanumeric() || anterior == '_' || anterior == '.' || anterior == '$' {
            return None;
        }
    }
    Some((chars[at..i].iter().collect(), i + 1 - at))
}

/// `^(\$?)([A-Za-z]{1,3})(\$?)(\d{1,8})` a partir de `at`.
fn match_ref(chars: &[char], at: usize) -> Option<(bool, String, bool, String, usize)> {
    let mut i = at;
    let col_lock = chars.get(i) == Some(&'$');
    if col_lock {
        i += 1;
    }
    let letras_ini = i;
    while i < chars.len() && chars[i].is_ascii_alphabetic() && i - letras_ini < 3 {
        i += 1;
    }
    if i == letras_ini {
        return None;
    }
    let letras: String = chars[letras_ini..i].iter().collect();
    let row_lock = chars.get(i) == Some(&'$');
    if row_lock {
        i += 1;
    }
    let digitos_ini = i;
    while i < chars.len() && chars[i].is_ascii_digit() && i - digitos_ini < 8 {
        i += 1;
    }
    if i == digitos_ini {
        return None;
    }
    let digitos: String = chars[digitos_ini..i].iter().collect();
    Some((col_lock, letras, row_lock, digitos, i - at))
}

/// Reescreve toda referência A1 de uma fórmula (que já começa com `=`),
/// preservando texto entre aspas. Igual `mapReferences`
/// (`core/formula-text.ts:30-89`), sem qualificação de aba estrangeira no
/// resultado de `fn` porque fórmula compartilhada de `.xlsx` nunca cruza
/// aba.
pub fn map_references(raw: &str, mut fn_: impl FnMut(&ParsedRef) -> String) -> String {
    if !raw.starts_with('=') {
        return raw.to_string();
    }
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::from("=");
    let mut i = 1;
    let mut in_string = false;

    while i < chars.len() {
        let c = chars[i];

        if in_string {
            out.push(c);
            if c == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    out.push('"');
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        let qualified = match_sheet_prefix(&chars, i);
        let ref_start = match &qualified {
            Some((_, len)) => i + len,
            None => i,
        };
        let m = match_ref(&chars, ref_start);
        let previous = if i > 0 { chars[i - 1] } else { '\0' };
        let glued = previous.is_alphanumeric() || previous == '_' || previous == '.' || previous == '$';

        if let Some((col_lock, letras, row_lock, digitos, len)) = &m {
            if qualified.is_some() || !glued {
                let after = chars.get(ref_start + len).copied().unwrap_or('\0');
                if qualified.is_none() && (after == '(' || after.is_alphabetic() || after == '_') {
                    out.push_str(&chars[i..ref_start + len].iter().collect::<String>());
                    i = ref_start + len;
                    continue;
                }
                let col = col_index(letras);
                let row: i64 = digitos.parse::<i64>().unwrap_or(1) - 1;
                let parsed = ParsedRef {
                    sheet: qualified.as_ref().map(|(n, _)| n.clone()),
                    col_lock: *col_lock,
                    row_lock: *row_lock,
                    col,
                    row,
                };
                out.push_str(&fn_(&parsed));
                i = ref_start + len;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }
    out
}

/// Desloca todas as referências não travadas de uma fórmula compartilhada
/// por `d_row`/`d_col`, respeitando `$` por eixo. Igual `shiftFormula`
/// (`src/io/xlsx.ts:1033-1043`).
pub fn shift_formula(raw: &str, d_row: i64, d_col: i64) -> String {
    map_references(raw, |r| {
        format_ref(&ParsedRef {
            sheet: r.sheet.clone(),
            col_lock: r.col_lock,
            row_lock: r.row_lock,
            row: if r.row_lock { r.row } else { r.row + d_row },
            col: if r.col_lock { r.col } else { r.col + d_col },
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_name_e_index_vao_e_voltam() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_index("A"), 0);
        assert_eq!(col_index("AA"), 26);
        assert_eq!(col_index("z"), 25);
    }

    #[test]
    fn shift_formula_desloca_e_respeita_cifrao() {
        assert_eq!(shift_formula("=A1*10", 1, 0), "=A2*10");
        // $A1: só a coluna trancada — a linha desloca normalmente.
        assert_eq!(shift_formula("=$A1*10", 1, 0), "=$A2*10");
        // A$1: só a linha trancada — não desloca deslocando linha.
        assert_eq!(shift_formula("=A$1*10", 1, 0), "=A$1*10");
        assert_eq!(shift_formula("=A1+B2", 2, 1), "=B3+C4");
    }

    #[test]
    fn nao_desloca_referencia_dentro_de_texto() {
        assert_eq!(shift_formula(r#"=SOMA(A1;"B2 não é referência")"#, 1, 0), r#"=SOMA(A2;"B2 não é referência")"#);
    }

    #[test]
    fn nao_confunde_funcao_com_referencia() {
        // LOG10( não é uma referência, mesmo parecendo col+linha.
        assert_eq!(shift_formula("=LOG10(A1)", 1, 0), "=LOG10(A2)");
    }
}
