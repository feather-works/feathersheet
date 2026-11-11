//! O léxico das fórmulas, em português do Brasil.
//!
//! Tradução de `src/core/tokenizer.ts`. As duas regras que fazem dele um
//! tokenizador pt-BR e não um do Excel em inglês:
//!
//! - **a vírgula entre dígitos é decimal** (`0,5`); fora disso ela separa
//!   argumentos, e o separador recomendado é o `;`;
//! - o nome da aba entre aspas simples entra no mesmo token da referência
//!   (`'Meu Plano'!B2`).
//!
//! Ele **não lê fórmula em inglês**: em `SUM(A1:A3,2.5)` a vírgula entre
//! dígitos viraria decimal. Quem traduz do Excel tem varredor próprio.

use crate::formula::ast::ParseError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenType {
    Num,
    Str,
    Ref,
    Ident,
    Op,
    LParen,
    RParen,
    /// `,`, `;` e `\` — o texto diz qual foi, e isso importa dentro de `{}`.
    Comma,
    LBrace,
    RBrace,
    Colon,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenType,
    pub text: String,
    /// Só para [`TokenType::Num`].
    pub value: Option<f64>,
    /// Índice em caracteres dentro do corpo da fórmula.
    pub pos: usize,
}

impl Token {
    fn novo(kind: TokenType, text: impl Into<String>, pos: usize) -> Token {
        Token { kind, text: text.into(), value: None, pos }
    }
}

const OPS_2: [&str; 3] = ["<>", "<=", ">="];
const OPS_1: [char; 10] = ['+', '-', '*', '/', '^', '&', '=', '<', '>', '%'];

fn e_digito(c: char) -> bool {
    c.is_ascii_digit()
}

/// Letra, incluindo acento: `MÉDIA` é um nome de função como qualquer outro.
fn e_letra(c: char) -> bool {
    c.is_ascii_alphabetic() || (c as u32) > 127
}

fn e_letra_de_nome(c: char) -> bool {
    e_letra(c) || e_digito(c) || c == '_' || c == '.'
}

/// `^\$?[A-Za-z]{1,3}\$?[0-9]{1,8}$` — o que pode ser uma referência A1.
pub fn parece_referencia(texto: &str) -> bool {
    let b = texto.as_bytes();
    let mut i = 0;
    if i < b.len() && b[i] == b'$' {
        i += 1;
    }
    let inicio = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let letras = i - inicio;
    if !(1..=3).contains(&letras) {
        return false;
    }
    if i < b.len() && b[i] == b'$' {
        i += 1;
    }
    let inicio = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let digitos = i - inicio;
    (1..=8).contains(&digitos) && i == b.len()
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, ParseError> {
    tokenize_com_decimal(src, ',')
}

/// O motor conserva fórmulas canônicas; o editor inglês distingue vírgula
/// de argumentos da vírgula decimal ao converter a entrada.
pub fn tokenize_com_decimal(src: &str, decimal: char) -> Result<Vec<Token>, ParseError> {
    let s: Vec<char> = src.chars().collect();
    let n = s.len();
    let em = |de: usize, ate: usize| -> String { s[de..ate].iter().collect() };
    let olhar = |i: usize| -> Option<char> { s.get(i).copied() };

    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0usize;

    while i < n {
        let c = s[i];
        if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
            i += 1;
            continue;
        }
        let inicio = i;

        // Texto entre aspas duplas, com "" valendo uma aspa.
        if c == '"' {
            i += 1;
            let mut out = String::new();
            let mut fechou = false;
            while i < n {
                if s[i] == '"' {
                    if olhar(i + 1) == Some('"') {
                        out.push('"');
                        i += 2;
                        continue;
                    }
                    fechou = true;
                    break;
                }
                out.push(s[i]);
                i += 1;
            }
            if !fechou {
                return Err(ParseError::new("Texto sem aspas de fechamento", inicio));
            }
            i += 1; // a aspa que fecha
            tokens.push(Token::novo(TokenType::Str, out, inicio));
            continue;
        }

        // Número. A vírgula só continua o número se vier dígito depois. Logo depois
        // de um valor (`#N/A,1`, `A1,2`), `,1` não é 0,1: é o separador e o 1.
        let depois_de_valor = tokens.last().is_some_and(|t| matches!(t.kind,
            TokenType::Num | TokenType::Str | TokenType::Ref | TokenType::Ident | TokenType::RParen | TokenType::RBrace));
        let decimal_inicia = (c == '.' || c == decimal) && !(c == ',' && depois_de_valor);
        if e_digito(c) || (decimal_inicia && olhar(i + 1).is_some_and(e_digito)) {
            while i < n && e_digito(s[i]) {
                i += 1;
            }
            if olhar(i) == Some('.') || (olhar(i) == Some(decimal) && olhar(i + 1).is_some_and(e_digito))
            {
                i += 1;
                while i < n && e_digito(s[i]) {
                    i += 1;
                }
            }
            if olhar(i) == Some('e') || olhar(i) == Some('E') {
                let salvo = i;
                i += 1;
                if olhar(i) == Some('+') || olhar(i) == Some('-') {
                    i += 1;
                }
                if olhar(i).is_some_and(e_digito) {
                    while i < n && e_digito(s[i]) {
                        i += 1;
                    }
                } else {
                    i = salvo;
                }
            }
            let texto = em(inicio, i);
            let valor = crate::celulas::number::from_js_string(&texto.replacen(',', ".", 1));
            tokens.push(Token {
                kind: TokenType::Num,
                text: texto,
                value: valor,
                pos: inicio,
            });
            continue;
        }

        // Literal de erro: #DIV/0!, #N/A, #VALUE! …
        if c == '#' {
            i += 1;
            // Separadores de argumentos/matriz não fazem parte do erro.
            // Sem ';' aqui, SE(A1;#N/A;0) era engolido como um token.
            while i < n && s[i] != '!' && !s[i].is_whitespace()
                && !matches!(s[i], ',' | ';' | '\\' | ')' | '}' | ':')
            {
                i += 1;
            }
            if olhar(i) == Some('!') {
                i += 1;
            }
            tokens.push(Token::novo(
                TokenType::Ident,
                em(inicio, i).to_uppercase(),
                inicio,
            ));
            continue;
        }

/// `C`, `$C`, `12`, `$12`: metade de um intervalo de colunas ou linhas inteiras.
fn metade_de_faixa_inteira(t: &str) -> Option<bool> {
    let t = t.strip_prefix('$').unwrap_or(t);
    if !t.is_empty() && t.len() <= 3 && t.chars().all(|c| c.is_ascii_alphabetic()) { return Some(true); }
    if !t.is_empty() && t.len() <= 9 && t.chars().all(|c| c.is_ascii_digit()) { return Some(false); }
    None
}

/// Depois de `Aba!C`, lê `:I` (ou `Aba!1` e `:3`): devolve o fim e onde parou.
/// Só junta metades do mesmo tipo (coluna com coluna, linha com linha).
fn faixa_inteira_depois(s: &[char], j: usize, inicio: &str) -> Option<(String, usize)> {
    let tipo = metade_de_faixa_inteira(inicio)?;
    if s.get(j) != Some(&':') { return None; }
    let mut k = j + 1;
    if s.get(k) == Some(&'$') { k += 1; }
    while k < s.len() && s[k].is_ascii_alphanumeric() { k += 1; }
    let fim: String = s[j + 1..k].iter().collect();
    (metade_de_faixa_inteira(&fim) == Some(tipo)).then_some((fim, k))
}

fn achar_fechamento_colchete(s: &[char], inicio: usize) -> Option<usize> {
    let mut depth = 0usize;
    for k in inicio..s.len() {
        if s[k] == '[' {
            depth += 1;
        } else if s[k] == ']' {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        }
    }
    None
}

        // 'Nome da aba'!A1 ou 'Nome da Tabela'[Coluna]
        if c == '\'' {
            let mut j = i + 1;
            let mut nome = String::new();
            let mut fechou = false;
            while j < n {
                if s[j] == '\'' {
                    if olhar(j + 1) == Some('\'') {
                        nome.push('\'');
                        j += 2;
                        continue;
                    }
                    fechou = true;
                    break;
                }
                nome.push(s[j]);
                j += 1;
            }
            if !fechou {
                return Err(ParseError::new(
                    "Aspas simples sem fechamento",
                    inicio,
                ));
            }
            j += 1; // pula a aspa que fecha
            if olhar(j) == Some('!') {
                j += 1; // pula !
                let target = if olhar(j) == Some('\'') {
                    j += 1;
                    let mut t_nome = String::new();
                    let mut t_fechou = false;
                    while j < n {
                        if s[j] == '\'' {
                            if olhar(j + 1) == Some('\'') {
                                t_nome.push('\'');
                                j += 2;
                                continue;
                            }
                            t_fechou = true;
                            break;
                        }
                        t_nome.push(s[j]);
                        j += 1;
                    }
                    if !t_fechou {
                        return Err(ParseError::new(
                            "Aspas simples sem fechamento na referência",
                            inicio,
                        ));
                    }
                    j += 1;
                    format!("'{t_nome}'")
                } else {
                    let mut k = j;
                    while k < n && (e_letra_de_nome(s[k]) || s[k] == '$' || s[k] == ' ') {
                        k += 1;
                    }
                    let res = em(j, k).trim().to_string();
                    j = k;
                    res
                };
                if olhar(j) == Some('[') {
                    if let Some(fim) = achar_fechamento_colchete(&s, j) {
                        i = fim + 1;
                        tokens.push(Token::novo(
                            TokenType::Ident,
                            em(inicio, i),
                            inicio,
                        ));
                        continue;
                    } else {
                        return Err(ParseError::new(
                            format!("Colchete sem fechamento depois de '{nome}'!{target}"),
                            inicio,
                        ));
                    }
                }
                // 'Minha aba'!C:I — colunas (ou linhas) inteiras de outra aba.
                if let Some((fim, k)) = faixa_inteira_depois(&s, j, &target) {
                    i = k;
                    tokens.push(Token::novo(TokenType::Ident, format!("{nome}!{target}:{fim}"), inicio));
                    continue;
                }
                if parece_referencia(&target) {
                    i = j;
                    tokens.push(Token::novo(
                        TokenType::Ref,
                        format!("{nome}!{target}"),
                        inicio,
                    ));
                    continue;
                }
                i = j;
                tokens.push(Token::novo(
                    TokenType::Ident,
                    format!("{nome}!{target}"),
                    inicio,
                ));
                continue;
            }
            if olhar(j) == Some('[') {
                if let Some(fim) = achar_fechamento_colchete(&s, j) {
                    i = fim + 1;
                    tokens.push(Token::novo(
                        TokenType::Ident,
                        em(inicio, i),
                        inicio,
                    ));
                    continue;
                } else {
                    return Err(ParseError::new(
                        format!("Colchete sem fechamento depois de '{nome}'"),
                        inicio,
                    ));
                }
            }
            i = j;
            tokens.push(Token::novo(
                TokenType::Ident,
                format!("'{nome}'"),
                inicio,
            ));
            continue;
        }

        // [Vendas.fshz]Dados!A1 — referência a outro arquivo sem aspas, como o
        // Excel escreve quando nem o arquivo nem a aba têm espaço. Arquivo e
        // aba entram no mesmo token da referência, igual a 'Aba'!A1; com
        // caminho ou espaço a forma é entre aspas ('/x/[Vendas.fshz]Dados'!A1).
        if c == '[' {
            let fecha = (i + 1..n).find(|&k| s[k] == ']' || s[k] == '[').filter(|&k| s[k] == ']');
            if let Some(fecha) = fecha {
                let mut j = fecha + 1;
                while j < n && e_letra_de_nome(s[j]) {
                    j += 1;
                }
                if fecha > i + 1 && j > fecha + 1 && olhar(j) == Some('!') {
                    let nome = em(i, j);
                    let inicio_ref = j + 1;
                    let mut k = inicio_ref;
                    while k < n && (e_letra_de_nome(s[k]) || s[k] == '$') {
                        k += 1;
                    }
                    let referencia = em(inicio_ref, k);
                    // [Vendas.fshz]Dados!A:A — colunas (ou linhas) inteiras.
                    if let Some((fim, depois)) = faixa_inteira_depois(&s, k, &referencia) {
                        i = depois;
                        tokens.push(Token::novo(TokenType::Ident, format!("{nome}!{referencia}:{fim}"), inicio));
                        continue;
                    }
                    if parece_referencia(&referencia) {
                        i = k;
                        tokens.push(Token::novo(TokenType::Ref, format!("{nome}!{referencia}"), inicio));
                        continue;
                    }
                }
            }
        }

        // Nome de função, intervalo nomeado ou referência.
        if e_letra(c) || c == '$' || c == '_' {
            while i < n && (e_letra_de_nome(s[i]) || s[i] == '$') {
                i += 1;
            }
            let mut texto = em(inicio, i);
            // Jan:Dez!B5 — referência 3D: as duas abas formam um nome só,
            // que o avaliador abre nas abas do meio (`Evaluator::abas_3d`).
            if olhar(i) == Some(':') && !parece_referencia(&texto) {
                let mut k = i + 1;
                while k < n && e_letra_de_nome(s[k]) { k += 1; }
                if k > i + 1 && olhar(k) == Some('!') && !parece_referencia(&em(i + 1, k)) {
                    i = k;
                    texto = em(inicio, i);
                }
            }
            // Plan1!A1 — o nome da aba entra no mesmo token da referência.
            if olhar(i) == Some('!') && !parece_referencia(&texto) {
                let inicio_ref = i + 1;
                let mut j = inicio_ref;
                while j < n && (e_letra_de_nome(s[j]) || s[j] == '$' || s[j] == ' ') {
                    j += 1;
                }
                let referencia = em(inicio_ref, j).trim().to_string();
                if olhar(j) == Some('[') {
                    if let Some(fim) = achar_fechamento_colchete(&s, j) {
                        i = fim + 1;
                        tokens.push(Token::novo(
                            TokenType::Ident,
                            em(inicio, i),
                            inicio,
                        ));
                        continue;
                    } else {
                        return Err(ParseError::new(
                            format!("Colchete sem fechamento depois de '{texto}'!{referencia}"),
                            inicio,
                        ));
                    }
                }
                // modulos!C:I — colunas (ou linhas) inteiras de outra aba.
                if let Some((fim, k)) = faixa_inteira_depois(&s, j, &referencia) {
                    i = k;
                    tokens.push(Token::novo(TokenType::Ident, format!("{texto}!{referencia}:{fim}"), inicio));
                    continue;
                }
                if parece_referencia(&referencia) {
                    i = j;
                    tokens.push(Token::novo(
                        TokenType::Ref,
                        format!("{texto}!{referencia}"),
                        inicio,
                    ));
                    continue;
                }
            }
            // Referência estruturada: `Tabela1[Valor]`, `Tabela1[#Tudo]`.
            if olhar(i) == Some('[') && !parece_referencia(&texto) {
                if let Some(fim) = achar_fechamento_colchete(&s, i) {
                    i = fim + 1;
                    tokens.push(Token::novo(
                        TokenType::Ident,
                        em(inicio, i),
                        inicio,
                    ));
                    continue;
                } else {
                    return Err(ParseError::new(
                        format!("Colchete sem fechamento depois de '{texto}'"),
                        inicio,
                    ));
                }
            }
            // Identificador seguido de "(" é sempre função (`LOG10(100)`, `ATAN2(..)`),
            // mesmo quando parece uma referência A1.
            let kind = if parece_referencia(&texto) && olhar(i) != Some('(') {
                TokenType::Ref
            } else {
                TokenType::Ident
            };
            tokens.push(Token::novo(kind, texto, inicio));
            continue;
        }

        let simples = match c {
            '(' => Some(TokenType::LParen),
            ')' => Some(TokenType::RParen),
            '{' => Some(TokenType::LBrace),
            '}' => Some(TokenType::RBrace),
            // Dentro de `{}`, o `\` separa colunas e o `;` separa linhas.
            ',' | ';' | '\\' => Some(TokenType::Comma),
            ':' => Some(TokenType::Colon),
            _ => None,
        };
        if let Some(kind) = simples {
            i += 1;
            tokens.push(Token::novo(kind, c.to_string(), inicio));
            continue;
        }

        if i + 1 < n {
            let dois: String = s[i..i + 2].iter().collect();
            if OPS_2.contains(&dois.as_str()) {
                i += 2;
                tokens.push(Token::novo(TokenType::Op, dois, inicio));
                continue;
            }
        }
        if OPS_1.contains(&c) {
            i += 1;
            tokens.push(Token::novo(TokenType::Op, c.to_string(), inicio));
            continue;
        }

        return Err(ParseError::new(format!("Caractere inesperado \"{c}\""), i));
    }

    tokens.push(Token::novo(TokenType::Eof, "", i));
    Ok(tokens)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tipos(src: &str) -> Vec<TokenType> {
        tokenize(src).unwrap().iter().map(|t| t.kind).collect()
    }

    fn textos(src: &str) -> Vec<String> {
        tokenize(src)
            .unwrap()
            .iter()
            .map(|t| t.text.clone())
            .collect()
    }

    #[test]
    fn a_virgula_entre_digitos_e_decimal() {
        let t = tokenize("0,5").unwrap();
        assert_eq!(t[0].kind, TokenType::Num);
        assert_eq!(t[0].value, Some(0.5));
        assert_eq!(t.len(), 2); // o número e o fim
    }

    #[test]
    fn a_virgula_fora_de_digitos_separa() {
        assert_eq!(
            tipos("SOMA(A1,B1)"),
            vec![
                TokenType::Ident,
                TokenType::LParen,
                TokenType::Ref,
                TokenType::Comma,
                TokenType::Ref,
                TokenType::RParen,
                TokenType::Eof
            ]
        );
    }

    #[test]
    fn numero_com_expoente() {
        let t = tokenize("1,5e3").unwrap();
        assert_eq!(t[0].value, Some(1500.0));
        // "2e" não é expoente: o "e" volta a ser nome.
        assert_eq!(tipos("2e"), vec![TokenType::Num, TokenType::Ident, TokenType::Eof]);
    }

    #[test]
    fn texto_com_aspas_dobradas() {
        let t = tokenize("\"diz \"\"oi\"\"\"").unwrap();
        assert_eq!(t[0].kind, TokenType::Str);
        assert_eq!(t[0].text, "diz \"oi\"");
    }

    #[test]
    fn texto_sem_fechar_reclama() {
        let e = tokenize("\"sem fim").unwrap_err();
        assert_eq!(e.at, Some(0));
        assert!(e.message.contains("aspas"));
    }

    #[test]
    fn aba_entre_aspas_simples() {
        let t = tokenize("'Meu Plano'!B2").unwrap();
        assert_eq!(t[0].kind, TokenType::Ref);
        assert_eq!(t[0].text, "Meu Plano!B2");
    }

    #[test]
    fn aba_com_aspa_no_nome() {
        let t = tokenize("'Do Ze''s'!A1").unwrap();
        assert_eq!(t[0].text, "Do Ze's!A1");
    }

    #[test]
    fn aba_sem_aspas_simples() {
        let t = tokenize("Dados!A1").unwrap();
        assert_eq!(t[0].kind, TokenType::Ref);
        assert_eq!(t[0].text, "Dados!A1");
    }

    #[test]
    fn nome_de_funcao_com_acento() {
        let t = tokenize("MÉDIA(A1:A9)").unwrap();
        assert_eq!(t[0].kind, TokenType::Ident);
        assert_eq!(t[0].text, "MÉDIA");
        // E a posição continua contando caracteres, não bytes.
        assert_eq!(t[1].pos, 5);
    }

    #[test]
    fn literal_de_erro() {
        let t = tokenize("#N/A").unwrap();
        assert_eq!(t[0].kind, TokenType::Ident);
        assert_eq!(t[0].text, "#N/A");
        assert_eq!(tokenize("#DIV/0!").unwrap()[0].text, "#DIV/0!");
    }

    /// Separa corretamente um literal de erro de seus argumentos,
    /// inclusive em pt-BR (';'), matrizes ('\\') e fórmulas com dois-pontos.
    #[test]
    fn literal_de_erro_respeita_delimitadores_e_separadores() {
        for separador in [';', ',', '\\', ':', '}'] {
            let expressao = format!("#N/A{separador}1");
            let tokens = tokenize(&expressao).unwrap();
            assert_eq!(tokens[0].kind, TokenType::Ident);
            assert_eq!(tokens[0].text, "#N/A");
            assert_eq!(tokens[1].pos, 4);
            assert_eq!(tokens[1].text, separador.to_string());
        }
        let itens = tokenize("SE(A1;#N/A;0)").unwrap();
        assert_eq!(itens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![TokenType::Ident, TokenType::LParen, TokenType::Ref,
                TokenType::Comma, TokenType::Ident, TokenType::Comma,
                TokenType::Num, TokenType::RParen, TokenType::Eof]);
        assert_eq!(itens[4].text, "#N/A");
        assert_eq!(itens[5].text, ";");
        assert_eq!(tokenize("#DIV/0!;1").unwrap()[0].text, "#DIV/0!");
    }

    #[test]
    fn acentos_e_aspas_preservam_posicoes_de_caracteres() {
        let tokens = tokenize("MÉDIA('São João'!$B$2;2,5)").unwrap();
        let referencia = tokens.iter().find(|t| t.kind == TokenType::Ref).unwrap();
        assert_eq!(referencia.text, "São João!$B$2");
        assert_eq!(referencia.pos, 6);
        let num = tokens.iter().find(|t| t.kind == TokenType::Num).unwrap();
        assert_eq!(num.value, Some(2.5));
    }

    #[test]
    fn operadores_de_dois_caracteres() {
        assert_eq!(textos("A1<>B1"), vec!["A1", "<>", "B1", ""]);
        assert_eq!(textos("A1<=B1"), vec!["A1", "<=", "B1", ""]);
    }

    #[test]
    fn caractere_inesperado() {
        let e = tokenize("A1 ? B1").unwrap_err();
        assert_eq!(e.at, Some(3));
        assert!(e.message.contains('?'));
    }

    /// Qualquer coisa acima de 127 conta como letra, porque é assim que
    /// `MÉDIA` funciona — inclusive `§`, que vira nome e não dá erro. É o
    /// comportamento do TypeScript, anotado aqui para não voltar por engano.
    #[test]
    fn acima_de_127_e_sempre_letra() {
        assert_eq!(tipos("§"), vec![TokenType::Ident, TokenType::Eof]);
    }

    /// Referência a outro arquivo, sem aspas (`[Vendas.fshz]Dados!A1`) e com
    /// caminho entre aspas: o arquivo e a aba vão no mesmo token.
    #[test]
    fn referencia_a_outro_arquivo_sem_e_com_aspas() {
        assert_eq!(textos("[Vendas.fshz]Dados!A1"), vec!["[Vendas.fshz]Dados!A1", ""]);
        assert_eq!(tipos("[Vendas.fshz]Dados!A1"), vec![TokenType::Ref, TokenType::Eof]);
        assert_eq!(
            tipos("SOMA([Vendas.fshz]Dados!A1:A10)"),
            vec![
                TokenType::Ident, TokenType::LParen, TokenType::Ref, TokenType::Colon,
                TokenType::Ref, TokenType::RParen, TokenType::Eof
            ]
        );
        assert_eq!(textos("'/home/x/[Vendas.fshz]Dados'!$B$2")[0], "/home/x/[Vendas.fshz]Dados!$B$2");
        assert_eq!(textos("[Vendas.fshz]Dados!A:A")[0], "[Vendas.fshz]Dados!A:A");
        // Colchete solto continua sendo erro.
        assert!(tokenize("[Vendas.fshz]").is_err());
        assert!(tokenize("[]Dados!A1").is_err());
    }

    #[test]
    fn matriz_usa_ponto_e_virgula_e_barra() {
        assert_eq!(
            tipos("{1;2}"),
            vec![
                TokenType::LBrace,
                TokenType::Num,
                TokenType::Comma,
                TokenType::Num,
                TokenType::RBrace,
                TokenType::Eof
            ]
        );
    }
}
