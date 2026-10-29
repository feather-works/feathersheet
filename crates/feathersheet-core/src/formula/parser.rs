//! Da lista de tokens para a árvore: um parser de precedência (Pratt).
//!
//! Tradução de `src/core/parser.ts`.

use crate::formula::ast::{BinaryOp, Node, ParseError, UnaryOp};
use crate::celulas::refs::{col_index, MAX_COLS, MAX_ROWS};
use crate::formula::tokenizer::{tokenize, Token, TokenType};

/// As partes de uma referência já separadas: `Dados!$B$7`.
struct RefParts {
    row: u32,
    col: u32,
    sheet: Option<String>,
}

/// Lê o texto de um token de referência. O nome da aba vai até o **último**
/// `!`, porque o nome pode conter um.
fn parse_a1(texto: &str) -> Option<RefParts> {
    let (sheet, cell) = match texto.rfind('!') {
        Some(p) => (Some(texto[..p].to_string()), &texto[p + 1..]),
        None => (None, texto),
    };
    let (row, col) = crate::celulas::refs::parse_cell_ref(cell)?;
    Some(RefParts { row, col, sheet })
}

/// Como o Excel: fórmula de até 8.192 caracteres. Também limita as cadeias
/// `=1+1+1+…`, que viram uma árvore funda sem passar pelo contador abaixo.
pub const MAX_CARACTERES: usize = 8_192;
/// Parênteses, funções e sinais (`---1`) um dentro do outro: o Excel aceita 64. Sem
/// teto, um `=((((…))))` de milhares de níveis estourava a pilha do parser e fechava
/// o app (#123). Cada nível empilha umas 5 funções do parser.
const MAX_ANINHAMENTO: u32 = 256;
/// Operadores, funções, matrizes e intervalos numa fórmula. A árvore não fica mais
/// funda que o número de nós internos, e cada um nasce de um desses tokens: contar
/// antes de montar garante que o cálculo, o arrasto, o texto, o `clone` e o `drop`
/// (todos recursivos) nunca recebem árvore funda demais, nem a de uma cadeia `1+1+…`.
/// 4.096 é o máximo que cabe nos 8.192 caracteres do Excel (`1+1+…`): tudo que o Excel
/// aceita passa. No build otimizado essa cadeia cabe até numa thread de 1 MB
/// (`tests/corrente_funda.rs`); o de depuração estoura 2 MB com ela.
// shortcut: no build de depuração (testes, `cargo run` sem --release) o teto fica em 1.024;
// tornar os percursos da árvore iterativos se esse build precisar das fórmulas longas.
pub const MAX_OPERACOES: usize = if cfg!(debug_assertions) { 1_024 } else { 4_096 };

struct Parser {
    tokens: Vec<Token>,
    i: usize,
    profundidade: u32,
}

impl Parser {
    fn peek(&self) -> &Token {
        // O tokenizador sempre põe um Eof no fim, então isto nunca falha.
        &self.tokens[self.i.min(self.tokens.len() - 1)]
    }

    fn next(&mut self) -> Token {
        let t = self.peek().clone();
        if self.i < self.tokens.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn expect(&mut self, kind: TokenType, o_que: &str) -> Result<Token, ParseError> {
        let t = self.next();
        if t.kind != kind {
            let achei = if t.text.is_empty() { "fim" } else { t.text.as_str() };
            return Err(ParseError::new(
                format!("Esperava {o_que} mas encontrei \"{achei}\""),
                t.pos,
            ));
        }
        Ok(t)
    }

    fn parse(&mut self) -> Result<Node, ParseError> {
        let node = self.parse_expr(0)?;
        let t = self.peek();
        if t.kind != TokenType::Eof {
            return Err(ParseError::new(
                format!("Sobra \"{}\" no fim da fórmula", t.text),
                t.pos,
            ));
        }
        Ok(node)
    }

    fn parse_expr(&mut self, min_prec: u8) -> Result<Node, ParseError> {
        let mut left = self.parse_unary()?;
        loop {
            let t = self.peek();
            if t.kind != TokenType::Op {
                break;
            }
            let Some(op) = BinaryOp::from_text(&t.text) else {
                break;
            };
            let prec = op.precedence();
            if prec < min_prec {
                break;
            }
            self.next();
            let proximo = if op.right_assoc() { prec } else { prec + 1 };
            let right = self.parse_expr(proximo)?;
            left = Node::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Toda recursão do parser passa por aqui: é onde a profundidade é contada.
    fn parse_unary(&mut self) -> Result<Node, ParseError> {
        self.profundidade += 1;
        if self.profundidade > MAX_ANINHAMENTO {
            return Err(ParseError::new(
                format!("Fórmula aninhada demais: o limite é de {MAX_ANINHAMENTO} níveis"),
                self.peek().pos,
            ));
        }
        let r = self.parse_unary_dentro();
        self.profundidade -= 1;
        r
    }

    fn parse_unary_dentro(&mut self) -> Result<Node, ParseError> {
        let t = self.peek();
        if t.kind == TokenType::Op && (t.text == "-" || t.text == "+") {
            let op = if t.text == "-" { UnaryOp::Neg } else { UnaryOp::Plus };
            self.next();
            let operand = self.parse_unary()?;
            return Ok(Node::Unary { op, operand: Box::new(operand) });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Node, ParseError> {
        let mut node = self.parse_primary()?;
        while self.peek().kind == TokenType::Op && self.peek().text == "%" {
            self.next();
            node = Node::Unary {
                op: UnaryOp::Percent,
                operand: Box::new(node),
            };
        }
        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<Node, ParseError> {
        let t = self.next();
        match t.kind {
            TokenType::Num => Ok(Node::Num(t.value.unwrap_or(0.0))),
            TokenType::Str => Ok(Node::Str(t.text)),
            TokenType::LParen => {
                let dentro = self.parse_expr(0)?;
                self.expect(TokenType::RParen, "\")\"")?;
                Ok(dentro)
            }
            TokenType::Ref => {
                let Some(a) = parse_a1(&t.text) else {
                    return Ok(Node::Error("#REF!".to_string()));
                };
                if self.peek().kind == TokenType::Colon {
                    self.next();
                    let fim = self.next();
                    if fim.kind != TokenType::Ref {
                        return Err(ParseError::new("Intervalo inválido", fim.pos));
                    }
                    let Some(b) = parse_a1(&fim.text) else {
                        return Ok(Node::Error("#REF!".to_string()));
                    };
                    // Num intervalo, quem manda é a aba do lado esquerdo.
                    return Ok(faixa(a.row, a.col, b.row, b.col, a.sheet));
                }
                Ok(Node::Ref { row: a.row, col: a.col, sheet: a.sheet })
            }
            TokenType::LBrace => self.parse_array(t.pos),
            TokenType::Ident => self.parse_ident(t),
            _ => {
                let achei = if t.text.is_empty() { "fim" } else { t.text.as_str() };
                Err(ParseError::new(
                    format!("Fórmula incompleta perto de \"{achei}\""),
                    t.pos,
                ))
            }
        }
    }

    fn parse_ident(&mut self, t: Token) -> Result<Node, ParseError> {
        let maiusculo = t.text.to_uppercase();

        // Chamada de função.
        if self.peek().kind == TokenType::LParen {
            self.next();
            let mut args: Vec<Node> = Vec::new();
            if self.peek().kind != TokenType::RParen {
                loop {
                    // Argumento em branco (`SEQUÊNCIA(3;;;0)`): vale o padrão
                    // da função.
                    let k = self.peek().kind;
                    let arg = if k == TokenType::Comma || k == TokenType::RParen {
                        Node::Blank
                    } else {
                        self.parse_expr(0)?
                    };
                    args.push(arg);
                    if self.peek().kind == TokenType::Comma {
                        self.next();
                        continue;
                    }
                    break;
                }
            }
            self.expect(TokenType::RParen, "\")\"")?;
            let chamada = Node::Call { name: maiusculo.clone(), args };
            // Chamada imediata: `LAMBDA(x; x+1)(5)`.
            if maiusculo == "LAMBDA" && self.peek().kind == TokenType::LParen {
                self.next();
                let mut externos: Vec<Node> = vec![chamada];
                if self.peek().kind != TokenType::RParen {
                    loop {
                        let k = self.peek().kind;
                        let arg = if k == TokenType::Comma || k == TokenType::RParen {
                            Node::Blank
                        } else {
                            self.parse_expr(0)?
                        };
                        externos.push(arg);
                        if self.peek().kind == TokenType::Comma {
                            self.next();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenType::RParen, "\")\"")?;
                return Ok(Node::Call { name: "APLICAR.LAMBDA".to_string(), args: externos });
            }
            return Ok(chamada);
        }

        if maiusculo == "VERDADEIRO" || maiusculo == "TRUE" {
            return Ok(Node::Bool(true));
        }
        if maiusculo == "FALSO" || maiusculo == "FALSE" {
            return Ok(Node::Bool(false));
        }
        if maiusculo.starts_with('#') {
            return Ok(Node::Error(maiusculo));
        }
        if let Some(no) = faixa_inteira_com_aba(&t.text) {
            return Ok(no);
        }
        if t.text.contains('!') {
            return Ok(Node::Error("#REF!".to_string()));
        }

        // Intervalo de colunas inteiras: A:C
        if self.peek().kind == TokenType::Colon {
            let sem_cifrao = maiusculo.replace('$', "");
            if let Some(c0) = col_index(&sem_cifrao) {
                self.next();
                let fim = self.next();
                let c1 = if fim.kind == TokenType::Ident {
                    col_index(&fim.text.replace('$', ""))
                } else {
                    None
                };
                return match c1 {
                    Some(c1) => Ok(faixa(0, c0, MAX_ROWS - 1, c1, None)),
                    None => Err(ParseError::new("Intervalo de colunas inválido", fim.pos)),
                };
            }
        }

        // Não é função nem referência: trata como intervalo nomeado.
        Ok(Node::Name(maiusculo))
    }

    /// Literal de matriz. O `;` começa uma linha nova; o `\` e o `,` seguem na
    /// mesma linha. Assim `{1;2;3}` é uma coluna, que é o que a interface em
    /// pt-BR faz.
    fn parse_array(&mut self, at: usize) -> Result<Node, ParseError> {
        let mut rows: Vec<Vec<Node>> = Vec::new();
        let mut linha: Vec<Node> = Vec::new();
        if self.peek().kind == TokenType::RBrace {
            return Err(ParseError::new("Matriz vazia", at));
        }
        loop {
            linha.push(self.parse_expr(0)?);
            let t = self.peek().clone();
            if t.kind == TokenType::Comma {
                self.next();
                if t.text == ";" {
                    rows.push(std::mem::take(&mut linha));
                }
                continue;
            }
            break;
        }
        rows.push(linha);
        self.expect(TokenType::RBrace, "\"}\"")?;
        let largura = rows[0].len();
        if rows.iter().any(|l| l.len() != largura) {
            return Err(ParseError::new("Matriz com linhas de tamanhos diferentes", at));
        }
        Ok(Node::Array { rows })
    }
}

/// `modulos!C:I` ou `Dados!1:3`, como o tokenizador junta: colunas ou
/// linhas inteiras de outra aba.
fn faixa_inteira_com_aba(texto: &str) -> Option<Node> {
    let p = texto.rfind('!')?;
    let (aba, faixa_txt) = (&texto[..p], &texto[p + 1..]);
    let (a, b) = faixa_txt.split_once(':')?;
    let (a, b) = (a.replace('$', ""), b.replace('$', ""));
    let aba = Some(aba.to_string());
    if let (Some(c0), Some(c1)) = (col_index(&a.to_uppercase()), col_index(&b.to_uppercase())) {
        if a.chars().all(|c| c.is_ascii_alphabetic()) && b.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(faixa(0, c0, MAX_ROWS - 1, c1, aba));
        }
    }
    let (r0, r1) = (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?);
    if r0 == 0 || r1 == 0 || r0 > MAX_ROWS || r1 > MAX_ROWS { return None; }
    Some(faixa(r0 - 1, 0, r1 - 1, MAX_COLS - 1, aba))
}

fn faixa(r0: u32, c0: u32, r1: u32, c1: u32, sheet: Option<String>) -> Node {
    Node::Range {
        r0: r0.min(r1),
        c0: c0.min(c1),
        r1: r0.max(r1),
        c1: c0.max(c1),
        sheet,
    }
}

/// Compila o corpo de uma fórmula (sem o "=").
pub fn parse_formula(src: &str) -> Result<Node, ParseError> {
    if src.chars().nth(MAX_CARACTERES).is_some() {
        return Err(ParseError::new(
            format!("Fórmula grande demais: o limite é de {MAX_CARACTERES} caracteres, como no Excel"),
            MAX_CARACTERES,
        ));
    }
    let tokens = tokenize(src)?;
    let operacoes = tokens.iter().filter(|t| matches!(t.kind,
        TokenType::Op | TokenType::Ident | TokenType::LBrace | TokenType::Colon)).count();
    if operacoes > MAX_OPERACOES {
        return Err(ParseError::solto(format!(
            "Fórmula longa demais: o limite é de {MAX_OPERACOES} operações, funções e intervalos")));
    }
    Parser { tokens, i: 0, profundidade: 0 }.parse()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn arvore(src: &str) -> Node {
        parse_formula(src).expect("devia compilar")
    }

    /// #123: fórmula maliciosa vira erro, não estouro de pilha.
    #[test]
    fn aninhamento_e_tamanho_tem_teto() {
        let fundo = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
        assert!(parse_formula(&fundo).is_err());
        assert!(parse_formula(&format!("{}1", "-".repeat(100_000))).is_err());
        assert!(parse_formula(&format!("{}1{}", "SOMA(".repeat(300), ")".repeat(300))).is_err());
        assert!(parse_formula(&"1".repeat(MAX_CARACTERES + 1)).is_err());
        // Dentro dos limites continua compilando.
        assert!(parse_formula(&format!("{}1{}", "(".repeat(200), ")".repeat(200))).is_ok());
        assert!(parse_formula(&format!("{}1", "1+".repeat(1000))).is_ok());
        assert!(parse_formula(&format!("{}1", "1+".repeat(MAX_OPERACOES + 1))).is_err(), "cadeia funda demais");
    }

    fn num(n: f64) -> Node {
        Node::Num(n)
    }

    #[test]
    fn precedencia() {
        // 1 + 2 * 3 liga o * primeiro.
        assert_eq!(
            arvore("1+2*3"),
            Node::Binary {
                op: BinaryOp::Add,
                left: Box::new(num(1.0)),
                right: Box::new(Node::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(num(2.0)),
                    right: Box::new(num(3.0)),
                }),
            }
        );
    }

    #[test]
    fn potencia_liga_da_direita() {
        assert_eq!(
            arvore("2^3^2"),
            Node::Binary {
                op: BinaryOp::Pow,
                left: Box::new(num(2.0)),
                right: Box::new(Node::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(num(3.0)),
                    right: Box::new(num(2.0)),
                }),
            }
        );
    }

    #[test]
    fn comparacao_liga_por_ultimo() {
        let n = arvore("1+1=2");
        assert!(matches!(n, Node::Binary { op: BinaryOp::Eq, .. }));
    }

    #[test]
    fn parenteses_mandam() {
        assert_eq!(
            arvore("(1+2)*3"),
            Node::Binary {
                op: BinaryOp::Mul,
                left: Box::new(Node::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(num(1.0)),
                    right: Box::new(num(2.0)),
                }),
                right: Box::new(num(3.0)),
            }
        );
    }

    #[test]
    fn menos_e_porcento() {
        assert_eq!(
            arvore("-50%"),
            Node::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(Node::Unary {
                    op: UnaryOp::Percent,
                    operand: Box::new(num(50.0)),
                }),
            }
        );
    }

    #[test]
    fn referencia_e_intervalo() {
        assert_eq!(arvore("B7"), Node::Ref { row: 6, col: 1, sheet: None });
        assert_eq!(arvore("$B$7"), Node::Ref { row: 6, col: 1, sheet: None });
        assert_eq!(
            arvore("A1:C9"),
            Node::Range { r0: 0, c0: 0, r1: 8, c1: 2, sheet: None }
        );
        // Fora de ordem: sai normalizado.
        assert_eq!(
            arvore("C9:A1"),
            Node::Range { r0: 0, c0: 0, r1: 8, c1: 2, sheet: None }
        );
    }

    #[test]
    fn referencia_entre_abas() {
        assert_eq!(
            arvore("Dados!A1"),
            Node::Ref { row: 0, col: 0, sheet: Some("Dados".into()) }
        );
        assert_eq!(
            arvore("'Meu Plano'!B2"),
            Node::Ref { row: 1, col: 1, sheet: Some("Meu Plano".into()) }
        );
        // Num intervalo, quem manda é a aba da esquerda.
        assert_eq!(
            arvore("Dados!A1:B2"),
            Node::Range { r0: 0, c0: 0, r1: 1, c1: 1, sheet: Some("Dados".into()) }
        );
    }

    #[test]
    fn coluna_inteira() {
        assert_eq!(
            arvore("A:C"),
            Node::Range { r0: 0, c0: 0, r1: MAX_ROWS - 1, c1: 2, sheet: None }
        );
    }

    #[test]
    fn chamada_de_funcao() {
        assert_eq!(
            arvore("SOMA(A1;2)"),
            Node::Call {
                name: "SOMA".into(),
                args: vec![Node::Ref { row: 0, col: 0, sheet: None }, num(2.0)],
            }
        );
        // O nome sobe para maiúscula.
        assert!(matches!(arvore("soma()"), Node::Call { name, .. } if name == "SOMA"));
    }

    #[test]
    fn argumento_em_branco() {
        assert_eq!(
            arvore("SEQUÊNCIA(3;;;0)"),
            Node::Call {
                name: "SEQUÊNCIA".into(),
                args: vec![num(3.0), Node::Blank, Node::Blank, num(0.0)],
            }
        );
    }

    #[test]
    fn condicional_com_erro_literal_entre_argumentos_pt_br() {
        let parsed = parse_formula("SE(A1;#N/A;0)").unwrap();
        assert_eq!(parsed, Node::Call {
            name: "SE".into(),
            args: vec![
                Node::Ref { row: 0, col: 0, sheet: None },
                Node::Error("#N/A".into()),
                Node::Num(0.0),
            ],
        });
        let parsed = parse_formula("SE(A1;#DIV/0!;#REF!)").unwrap();
        match parsed {
            Node::Call { name, args } => {
                assert_eq!(name, "SE");
                assert_eq!(args.len(), 3);
                assert_eq!(args[1], Node::Error("#DIV/0!".into()));
                assert_eq!(args[2], Node::Error("#REF!".into()));
            }
            other => panic!("chamada esperada, recebido: {other:?}"),
        }
    }

    #[test]
    fn constante_de_potencia_liga_a_direita_no_parser_nativo() {
        assert_eq!(parse_formula("2^3^2").unwrap(), Node::Binary {
            op: BinaryOp::Pow,
            left: Box::new(Node::Num(2.0)),
            right: Box::new(Node::Binary {
                op: BinaryOp::Pow,
                left: Box::new(Node::Num(3.0)),
                right: Box::new(Node::Num(2.0)),
            }),
        });
    }

    #[test]
    fn logicos_e_erro_literal() {
        assert_eq!(arvore("VERDADEIRO"), Node::Bool(true));
        assert_eq!(arvore("false"), Node::Bool(false));
        assert_eq!(arvore("#N/A"), Node::Error("#N/A".into()));
    }

    #[test]
    fn intervalo_nomeado() {
        assert_eq!(arvore("Vendas"), Node::Name("VENDAS".into()));
    }

    #[test]
    fn matriz_por_ponto_e_virgula_e_coluna() {
        assert_eq!(
            arvore("{1;2;3}"),
            Node::Array { rows: vec![vec![num(1.0)], vec![num(2.0)], vec![num(3.0)]] }
        );
        assert_eq!(
            arvore("{1\\2}"),
            Node::Array { rows: vec![vec![num(1.0), num(2.0)]] }
        );
    }

    #[test]
    fn matriz_torta_reclama() {
        let e = parse_formula("{1\\2;3}").unwrap_err();
        assert!(e.message.contains("tamanhos diferentes"));
        assert!(parse_formula("{}").is_err());
    }

    #[test]
    fn sobra_no_fim() {
        let e = parse_formula("1+1 2").unwrap_err();
        assert!(e.message.contains("Sobra"));
        assert_eq!(e.at, Some(4));
    }

    #[test]
    fn formula_incompleta() {
        assert!(parse_formula("1+").is_err());
        assert!(parse_formula("(1+2").is_err());
        assert!(parse_formula("").is_err());
    }

    #[test]
    fn a_vistoria_visita_todo_mundo() {
        let n = arvore("SOMA(A1:B2;{1;2};-C3%)");
        let mut referencias = 0;
        let mut numeros = 0;
        n.walk(&mut |no| match no {
            Node::Ref { .. } | Node::Range { .. } => referencias += 1,
            Node::Num(_) => numeros += 1,
            _ => {}
        });
        assert_eq!(referencias, 2);
        assert_eq!(numeros, 2);
    }
}
