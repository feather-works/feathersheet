//! Referências: `A1` de um lado, linha e coluna do outro.
//!
//! Tradução de `src/core/refs.ts`.

/// A .. XFD
pub const MAX_COLS: u32 = 16_384;
/// Cem milhões de linhas (era 10 milhões até 15/10/2026: um .parquet de
/// 13,7 milhões de linhas não cabia).
pub const MAX_ROWS: u32 = 100_000_000;

/// Chave inteira de uma célula.
///
/// No TypeScript isto existia porque `Map<number, …>` é bem mais rápido que
/// `Map<string, …>`, e havia o cuidado de não usar deslocamento de bit (o
/// JavaScript faria a conta em 32 bits e corromperia a chave acima de 131.072
/// linhas). Em Rust o cuidado some — `u64` dá conta — mas a conta continua a
/// mesma, porque é ela que está gravada nos arquivos `.fshz` que já existem.
pub type CellKey = u64;

#[inline]
pub fn key(row: u32, col: u32) -> CellKey {
    row as u64 * MAX_COLS as u64 + col as u64
}

#[inline]
pub fn key_row(k: CellKey) -> u32 {
    (k / MAX_COLS as u64) as u32
}

#[inline]
pub fn key_col(k: CellKey) -> u32 {
    (k % MAX_COLS as u64) as u32
}

/// 0 -> "A", 25 -> "Z", 26 -> "AA"
pub fn col_name(col: u32) -> String {
    let mut nome = Vec::new();
    let mut n = col as i64;
    while n >= 0 {
        nome.push(b'A' + (n % 26) as u8);
        n = n / 26 - 1;
    }
    nome.reverse();
    // Só entram bytes de 'A' a 'Z'.
    String::from_utf8(nome).unwrap_or_default()
}

/// "A" -> 0, "AA" -> 26. `None` quando não é nome de coluna.
///
/// O TypeScript devolvia -1 aqui e quem chamava conferia o limite; em Rust o
/// `None` cobre os dois casos, porque todos os chamadores recusavam tanto o -1
/// quanto o valor acima de [`MAX_COLS`].
pub fn col_index(nome: &str) -> Option<u32> {
    if nome.is_empty() {
        return None;
    }
    let mut n: u64 = 0;
    for b in nome.bytes() {
        // `& !32` é o mesmo truque do TypeScript para subir a letra para
        // maiúscula; byte de acento continua acima de 'Z' e é recusado.
        let c = b & !32;
        if !c.is_ascii_uppercase() {
            return None;
        }
        n = n * 26 + (c - b'A' + 1) as u64;
        if n > MAX_COLS as u64 {
            return None;
        }
    }
    u32::try_from(n - 1).ok()
}

pub fn cell_name(row: u32, col: u32) -> String {
    format!("{}{}", col_name(col), row + 1)
}

/// Um retângulo de células. `sheet` ausente quer dizer "a aba atual".
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
}

impl Rect {
    pub fn new(r0: u32, c0: u32, r1: u32, c1: u32) -> Self {
        Rect { r0, c0, r1, c1, sheet: None }
    }

    pub fn na_aba(r0: u32, c0: u32, r1: u32, c1: u32, sheet: impl Into<String>) -> Self {
        Rect { r0, c0, r1, c1, sheet: Some(sheet.into()) }
    }

    pub fn cell(row: u32, col: u32) -> Self {
        Rect::new(row, col, row, col)
    }

    /// Devolve o mesmo retângulo com `r0<=r1` e `c0<=c1`.
    pub fn normalized(&self) -> Rect {
        Rect {
            r0: self.r0.min(self.r1),
            c0: self.c0.min(self.c1),
            r1: self.r0.max(self.r1),
            c1: self.c0.max(self.c1),
            sheet: self.sheet.clone(),
        }
    }

    pub fn contains(&self, row: u32, col: u32) -> bool {
        row >= self.r0 && row <= self.r1 && col >= self.c0 && col <= self.c1
    }

    /// Quantas células. Uma coluna inteira já passa de 10 milhões, então a
    /// conta é em 64 bits.
    pub fn area(&self) -> u64 {
        let n = self.normalized();
        (n.r1 - n.r0 + 1) as u64 * (n.c1 - n.c0 + 1) as u64
    }

    pub fn is_full_col(&self) -> bool {
        self.r0 == 0 && self.r1 >= MAX_ROWS - 1
    }

    pub fn is_full_row(&self) -> bool {
        self.c0 == 0 && self.c1 >= MAX_COLS - 1
    }
}

/// Retorna o nome da aba como referência de fórmula.
/// Fonte única da regra `modelo/refs.ts::quoteSheet`: somente
/// `[A-Za-zÀ-ÿ_][A-Za-zÀ-ÿ0-9_.]*` dispensa aspas. Demais nomes
/// ficam em aspas simples, com cada aspa interna duplicada.
///
/// Importante: `char::is_alphabetic` permite letras gregas/cirílicas
/// que a regex TypeScript não permite. Não ampliar a regra sem mudar
/// o parser de fórmulas e o formato de arquivo conjuntamente.
pub fn quote_sheet(name: &str) -> String {
    let nome_simples = |nome: &str| !nome.is_empty() && nome.chars().enumerate().all(|(i, c)| {
        let letra = c.is_ascii_alphabetic() || ('\u{C0}'..='\u{FF}').contains(&c);
        if i == 0 {
            letra || c == '_'
        } else {
            letra || c.is_ascii_digit() || c == '_' || c == '.'
        }
    });
    // Aba de outro arquivo (`[Vendas.fshz]Dados`): sem aspas quando o arquivo
    // não tem caminho, espaço nem aspa e a aba é simples — como o Excel
    // escreve e como o tokenizador lê (`[` no começo do token).
    let externo_simples = name.strip_prefix('[').and_then(|resto| resto.split_once(']')).is_some_and(|(arquivo, aba)| {
        !arquivo.is_empty()
            && arquivo.chars().all(|c| c.is_ascii_alphanumeric() || ('\u{C0}'..='\u{FF}').contains(&c) || matches!(c, '_' | '.' | '-'))
            && nome_simples(aba)
    });
    // `AB12!B1` seria lido como a célula AB12 seguida de `!`: precisa de aspas.
    let parece_celula = |nome: &str| nome.find(|c: char| c.is_ascii_digit()).is_some_and(|i| {
        col_index(&nome[..i]).is_some() && nome[i..].bytes().all(|b| b.is_ascii_digit())
    });
    let aba = name.strip_prefix('[').and_then(|resto| resto.split_once(']')).map_or(name, |(_, aba)| aba);
    if (externo_simples || nome_simples(name)) && !parece_celula(aba) {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// "B7" ou "$B$7" -> (linha, coluna), ambas a partir de zero.
pub fn parse_cell_ref(texto: &str) -> Option<(u32, u32)> {
    let t = texto.trim();
    let (letras, digitos) = partir_a1(t)?;
    let col = col_index(letras)?;
    let linha: u32 = digitos.parse().ok()?;
    if linha == 0 {
        return None;
    }
    let row = linha - 1;
    if col >= MAX_COLS || row >= MAX_ROWS {
        return None;
    }
    Some((row, col))
}

/// Separa `$B$7` em ("B", "7"), exigindo o formato inteiro: cifrão opcional,
/// de uma a três letras, cifrão opcional, de um a sete algarismos, e mais
/// nada. É o que a expressão regular `^\$?([A-Za-z]{1,3})\$?(\d{1,7})$` fazia, com
/// oito algarismos em vez de sete: a planilha vai até a linha 10.000.000.
fn partir_a1(t: &str) -> Option<(&str, &str)> {
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && b[i] == b'$' {
        i += 1;
    }
    let inicio_letras = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let letras = &t[inicio_letras..i];
    if letras.is_empty() || letras.len() > 3 {
        return None;
    }
    if i < b.len() && b[i] == b'$' {
        i += 1;
    }
    let inicio_digitos = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i != b.len() {
        return None;
    }
    let digitos = &t[inicio_digitos..i];
    if digitos.is_empty() || digitos.len() > 9 {
        return None;
    }
    Some((letras, digitos))
}

/// Só letras, com cifrão opcional na frente: o lado de uma referência de
/// coluna inteira (`C:C`).
fn so_coluna(parte: &str) -> Option<u32> {
    let t = parte.strip_prefix('$').unwrap_or(parte);
    if t.is_empty() || t.len() > 3 || !t.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    col_index(t).filter(|c| *c < MAX_COLS)
}

/// Só algarismos, com cifrão opcional na frente: o lado de uma referência de
/// linha inteira (`1:10`). Devolve a linha a partir de zero.
fn so_linha(parte: &str) -> Option<u32> {
    let t = parte.strip_prefix('$').unwrap_or(parte);
    if t.is_empty() || t.len() > 9 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = t.parse().ok()?;
    if n == 0 || n > MAX_ROWS {
        return None;
    }
    Some(n - 1)
}

/// "A1", "A1:C9", "C:C" ou "1:10" -> retângulo normalizado.
pub fn parse_range_ref(texto: &str) -> Option<Rect> {
    let t = texto.trim();
    let partes: Vec<&str> = t.split(':').collect();
    if partes.len() == 1 {
        let (row, col) = parse_cell_ref(partes[0])?;
        return Some(Rect::cell(row, col));
    }
    if partes.len() != 2 {
        return None;
    }
    if let (Some(a), Some(b)) = (so_coluna(partes[0]), so_coluna(partes[1])) {
        return Some(Rect::new(0, a, MAX_ROWS - 1, b).normalized());
    }
    if let (Some(a), Some(b)) = (so_linha(partes[0]), so_linha(partes[1])) {
        return Some(Rect::new(a, 0, b, MAX_COLS - 1).normalized());
    }
    let (ra, ca) = parse_cell_ref(partes[0])?;
    let (rb, cb) = parse_cell_ref(partes[1])?;
    Some(Rect::new(ra, ca, rb, cb).normalized())
}

/// Escreve o retângulo como referência legível:
/// `C:C` para coluna inteira, `5:10` para linhas inteiras, `A1` para uma
/// célula, `A1:B10` para o resto.
pub fn format_ref(rect: &Rect) -> String {
    let n = rect.normalized();
    let prefixo = match &n.sheet {
        Some(aba) => format!("{aba}!"),
        None => String::new(),
    };

    if n.is_full_col() {
        let a = col_name(n.c0);
        let b = col_name(n.c1);
        return format!("{prefixo}{a}:{b}");
    }
    if n.is_full_row() {
        return format!("{prefixo}{}:{}", n.r0 + 1, n.r1 + 1);
    }
    let a = cell_name(n.r0, n.c0);
    if n.r0 == n.r1 && n.c0 == n.c1 {
        return format!("{prefixo}{a}");
    }
    format!("{prefixo}{a}:{}", cell_name(n.r1, n.c1))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn quote_sheet_tem_paridade_com_regex_do_typescript() {
        for nome in ["Dados", "Plan1", "Relatório", "Mês_2026", "_tmp", "A.B", "Ñandú"] {
            assert_eq!(quote_sheet(nome), nome, "{nome}");
        }
        for (nome, esperado) in [
            ("Meu Plano", "'Meu Plano'"),
            ("1Dados", "'1Dados'"),
            ("A-B", "'A-B'"),
            ("", "''"),
            ("D'água", "'D''água'"),
            ("Σoma", "'Σoma'"),
            ("😀", "'😀'"),
            (".Oculta", "'.Oculta'"),
        ] {
            assert_eq!(quote_sheet(nome), esperado, "{nome}");
        }
        // A classe literal À-ÿ da regex TS inclui inclusive × (U+D7)
        // e ÷ (U+F7), ainda que não sejam letras Unicode.
        assert_eq!(quote_sheet("A×B"), "A×B");
    }

    /// Aba de outro arquivo: sem aspas na forma simples (como o Excel), entre
    /// aspas com caminho, espaço ou aba com espaço.
    #[test]
    fn aba_de_outro_arquivo_so_tem_aspas_quando_precisa() {
        assert_eq!(quote_sheet("[Vendas.fshz]Dados"), "[Vendas.fshz]Dados");
        assert_eq!(quote_sheet("[vendas-2026.xlsx]Plan1"), "[vendas-2026.xlsx]Plan1");
        assert_eq!(quote_sheet("[Minhas Vendas.fshz]Dados"), "'[Minhas Vendas.fshz]Dados'");
        assert_eq!(quote_sheet("[Vendas.fshz]Meu Plano"), "'[Vendas.fshz]Meu Plano'");
        assert_eq!(quote_sheet("/home/x/[Vendas.fshz]Dados"), "'/home/x/[Vendas.fshz]Dados'");
        assert_eq!(quote_sheet("[d'água.fshz]P"), "'[d''água.fshz]P'");
        assert_eq!(quote_sheet("[Vendas.fshz]S2"), "'[Vendas.fshz]S2'");
    }

    #[test]
    fn nome_que_parece_celula_tem_aspas() {
        assert_eq!(quote_sheet("AB12"), "'AB12'");
        assert_eq!(quote_sheet("S2"), "'S2'");
        assert_eq!(quote_sheet("Plan1"), "Plan1");
        assert_eq!(quote_sheet("Dados1"), "Dados1");
        assert_eq!(quote_sheet("A1B"), "A1B");
    }

    #[test]
    fn fronteiras_de_referencia_e_intervalo_do_typescript() {
        assert_eq!(parse_cell_ref("XFD100000000"), Some((MAX_ROWS - 1, MAX_COLS - 1)));
        assert_eq!(parse_cell_ref("XFE1"), None);
        assert_eq!(parse_cell_ref("A100000001"), None);
        assert_eq!(parse_range_ref("$C:$A"),
            Some(Rect::new(0, 0, MAX_ROWS - 1, 2)));
        assert_eq!(parse_range_ref("$10:$1"),
            Some(Rect::new(0, 0, 9, MAX_COLS - 1)));
        let r = Rect::na_aba(0, 0, 2, 3, "Relatório");
        assert_eq!(r.normalized().sheet, Some("Relatório".into()));
        assert!(r.contains(1, 2));
        assert_eq!(r.area(), 12);
    }

    #[test]
    fn chave_vai_e_volta_no_fim_da_planilha() {
        let (linha, coluna) = (MAX_ROWS - 1, MAX_COLS - 1);
        let k = key(linha, coluna);
        assert_eq!(key_row(k), linha);
        assert_eq!(key_col(k), coluna);
        // O mesmo número que o TypeScript calculava, e que está nos arquivos.
        assert_eq!(k, 99_999_999 * 16_384 + 16_383);
        assert_eq!(k, 1_638_399_999_999);
    }

    #[test]
    fn nome_de_coluna() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(MAX_COLS - 1), "XFD");
    }

    #[test]
    fn indice_de_coluna() {
        assert_eq!(col_index("A"), Some(0));
        assert_eq!(col_index("a"), Some(0));
        assert_eq!(col_index("AA"), Some(26));
        assert_eq!(col_index("XFD"), Some(MAX_COLS - 1));
        assert_eq!(col_index(""), None);
        assert_eq!(col_index("A1"), None);
        assert_eq!(col_index("MÉDIA"), None);
        // Passou do fim da planilha.
        assert_eq!(col_index("XFE"), None);
    }

    #[test]
    fn toda_coluna_vai_e_volta() {
        for c in 0..MAX_COLS {
            assert_eq!(col_index(&col_name(c)), Some(c), "coluna {c}");
        }
    }

    #[test]
    fn le_celula() {
        assert_eq!(parse_cell_ref("B7"), Some((6, 1)));
        assert_eq!(parse_cell_ref("$B$7"), Some((6, 1)));
        assert_eq!(parse_cell_ref(" b7 "), Some((6, 1)));
        assert_eq!(parse_cell_ref("B0"), None);
        assert_eq!(parse_cell_ref("B"), None);
        assert_eq!(parse_cell_ref("7"), None);
        assert_eq!(parse_cell_ref("ABCD1"), None);
        assert_eq!(parse_cell_ref("A999999999"), None);
    }

    #[test]
    fn le_intervalo() {
        assert_eq!(parse_range_ref("A1"), Some(Rect::cell(0, 0)));
        assert_eq!(parse_range_ref("A1:C9"), Some(Rect::new(0, 0, 8, 2)));
        // Fora de ordem: volta normalizado.
        assert_eq!(parse_range_ref("C9:A1"), Some(Rect::new(0, 0, 8, 2)));
        assert_eq!(parse_range_ref("C:C"), Some(Rect::new(0, 2, MAX_ROWS - 1, 2)));
        assert_eq!(parse_range_ref("A:C"), Some(Rect::new(0, 0, MAX_ROWS - 1, 2)));
        assert_eq!(parse_range_ref("1:10"), Some(Rect::new(0, 0, 9, MAX_COLS - 1)));
        assert_eq!(parse_range_ref("A1:B2:C3"), None);
        assert_eq!(parse_range_ref("bobagem"), None);
    }

    #[test]
    fn escreve_referencia() {
        assert_eq!(format_ref(&Rect::cell(0, 0)), "A1");
        assert_eq!(format_ref(&Rect::new(0, 0, 9, 1)), "A1:B10");
        assert_eq!(format_ref(&Rect::new(0, 2, MAX_ROWS - 1, 2)), "C:C");
        assert_eq!(format_ref(&Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1)), "A:XFD");
        assert_eq!(format_ref(&Rect::new(4, 0, 4, MAX_COLS - 1)), "5:5");
        assert_eq!(format_ref(&Rect::na_aba(0, 0, 0, 0, "Dados")), "Dados!A1");
    }

    /// A linha inteira só vale quando a coluna inteira não vale: um retângulo
    /// que cobre a planilha toda sai como `A:XFD`, que é o que o TypeScript
    /// fazia (a coluna é testada primeiro).
    #[test]
    fn a_planilha_inteira_sai_como_coluna() {
        let tudo = Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1);
        assert!(tudo.is_full_col() && tudo.is_full_row());
        assert_eq!(format_ref(&tudo), "A:XFD");
    }

    #[test]
    fn area_nao_estoura() {
        let coluna = Rect::new(0, 0, MAX_ROWS - 1, 0);
        assert_eq!(coluna.area(), MAX_ROWS as u64);
        let tudo = Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1);
        assert_eq!(tudo.area(), MAX_ROWS as u64 * MAX_COLS as u64);
    }
}
