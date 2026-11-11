//! Campo calculado da tabela dinâmica: `ValorA + ValorB`, `[Valor A] * 2`.
//!
//! Pedido do usuário em 23/09/2026. A fórmula é calculada **linha a linha**
//! sobre a origem e vira uma coluna a mais, que a dinâmica agrega como
//! qualquer outra (soma, média…). Para `A + B` dá o mesmo que somar A e somar
//! B; para `A * B` é a soma dos produtos, não o produto das somas.
//!
//! A gramática é pequena de propósito: números (vírgula ou ponto decimal),
//! nomes de campo (a palavra solta, ou `[entre colchetes]` quando tem espaço),
//! `+ - * /`, parênteses e sinal. Célula vazia, texto que não é número e
//! divisão por zero valem 0 — um erro por linha envenenaria a soma inteira.
//!
//! Porte de `src/core/campo-calculado.ts`, arquivado no descarte.
//! O motor Rust é a única implementação ativa e reutilizável em WASM.

use crate::celulas::values::{to_number, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Numero(f64),
    /// Índice da coluna, relativo à origem da dinâmica.
    Campo(usize),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Numero(f64),
    Nome(String),
    Op(char),
}

fn normalizar(nome: &str) -> String {
    nome.trim().to_lowercase()
}

fn lexar(texto: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let cs: Vec<char> = texto.trim().trim_start_matches('=').chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if "+-*/()".contains(c) {
            out.push(Token::Op(c));
            i += 1;
        } else if c == '[' {
            let fim = cs[i + 1..].iter().position(|&x| x == ']').ok_or("falta fechar o colchete")?;
            out.push(Token::Nome(cs[i + 1..i + 1 + fim].iter().collect()));
            i += fim + 2;
        } else if c.is_ascii_digit() {
            let ini = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == ',' || cs[i] == '.') {
                i += 1;
            }
            let t: String = cs[ini..i].iter().collect::<String>().replace(',', ".");
            // Number.isFinite era obrigatório no parser TypeScript: parse()
            // pode aceitar +infinito quando um literal decimal é muito grande.
            let n: f64 = t.parse().map_err(|_| crate::trf!("número inválido: {t}"))?;
            if !n.is_finite() {
                return Err(crate::trf!("número inválido: {t}"));
            }
            out.push(Token::Numero(n));
        } else if c.is_alphabetic() || c == '_' {
            let ini = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '.') {
                i += 1;
            }
            out.push(Token::Nome(cs[ini..i].iter().collect()));
        } else {
            return Err(crate::trf!("não entendi \"{c}\""));
        }
    }
    Ok(out)
}

struct Analisador<'a> {
    t: Vec<Token>,
    p: usize,
    campos: &'a [String],
}

impl Analisador<'_> {
    fn olha(&self) -> Option<&Token> {
        self.t.get(self.p)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut e = self.termo()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.olha().cloned() {
            self.p += 1;
            e = Expr::Bin(op, Box::new(e), Box::new(self.termo()?));
        }
        Ok(e)
    }

    fn termo(&mut self) -> Result<Expr, String> {
        let mut e = self.fator()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.olha().cloned() {
            self.p += 1;
            e = Expr::Bin(op, Box::new(e), Box::new(self.fator()?));
        }
        Ok(e)
    }

    fn fator(&mut self) -> Result<Expr, String> {
        let t = self.olha().cloned().ok_or_else(|| crate::tr!("a fórmula terminou antes da hora"))?;
        self.p += 1;
        match t {
            Token::Op('-') => Ok(Expr::Neg(Box::new(self.fator()?))),
            Token::Op('+') => self.fator(),
            Token::Op('(') => {
                let e = self.expr()?;
                match self.olha() {
                    Some(Token::Op(')')) => {
                        self.p += 1;
                        Ok(e)
                    }
                    _ => Err(crate::tr!("falta fechar o parêntese").into()),
                }
            }
            Token::Numero(n) => Ok(Expr::Numero(n)),
            Token::Nome(nome) => {
                let alvo = normalizar(&nome);
                self.campos
                    .iter()
                    .position(|c| normalizar(c) == alvo)
                    .map(Expr::Campo)
                    .ok_or_else(|| crate::trf!("não existe o campo \"{nome}\""))
            }
            Token::Op(op) => Err(crate::trf!("\"{op}\" fora do lugar")),
        }
    }
}

/// Compila a fórmula contra os nomes dos campos (o cabeçalho da origem, na
/// ordem). Nome é comparado sem diferença de maiúscula e sem espaço nas
/// pontas.
pub fn compilar(formula: &str, campos: &[String]) -> Result<Expr, String> {
    let mut a = Analisador { t: lexar(formula)?, p: 0, campos };
    if a.t.is_empty() {
        return Err(crate::tr!("a fórmula está vazia").into());
    }
    let e = a.expr()?;
    if a.p < a.t.len() {
        return Err(crate::tr!("sobrou coisa no fim da fórmula").into());
    }
    Ok(e)
}

/// Compila vários campos virtuais em sequência. Cada fórmula pode referenciar
/// campos calculados anteriores, evitando materializar colunas na origem.
pub fn compilar_sequencia(
    campos_originais: &[String],
    calculados: &[(String, String)],
) -> Result<Vec<Expr>, String> {
    let mut nomes = campos_originais.to_vec();
    let mut expressoes = Vec::with_capacity(calculados.len());
    for (nome, formula) in calculados {
        if nome.trim().is_empty() {
            return Err(crate::tr!("nome do campo calculado está vazio").into());
        }
        if nomes.iter().any(|existente| normalizar(existente) == normalizar(nome)) {
            return Err(format!("campo duplicado: {nome}"));
        }
        expressoes.push(compilar(formula, &nomes)?);
        nomes.push(nome.clone());
    }
    Ok(expressoes)
}

/// Avalia campos virtuais sequenciais usando apenas os valores da linha atual.
pub fn avaliar_sequencia(expressoes: &[Expr], originais: &[f64]) -> Vec<f64> {
    let mut valores = originais.to_vec();
    for expr in expressoes {
        let resultado = avaliar(expr, &|i| valores.get(i).copied().unwrap_or(0.0));
        valores.push(resultado);
    }
    valores[originais.len()..].to_vec()
}

/// O número de uma célula para a conta: vazio, texto e erro valem 0.
pub fn numero_de(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Empty | Value::Error(_) => 0.0,
        outro => to_number(outro).unwrap_or(0.0),
    }
}

/// Avalia numa linha: `campo(i)` dá o número da coluna `i` dessa linha.
pub fn avaliar(e: &Expr, campo: &dyn Fn(usize) -> f64) -> f64 {
    let r = match e {
        Expr::Numero(n) => *n,
        Expr::Campo(i) => campo(*i),
        Expr::Neg(x) => -avaliar(x, campo),
        Expr::Bin(op, a, b) => {
            let (a, b) = (avaliar(a, campo), avaliar(b, campo));
            match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                _ => {
                    if b == 0.0 {
                        0.0
                    } else {
                        a / b
                    }
                }
            }
        }
    };
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

/// Onde começa a palavra que está sendo digitada no fim da fórmula (byte) e se
/// ela está dentro de um `[` ainda aberto.
fn palavra_em_curso(formula: &str) -> (usize, bool) {
    if let Some(a) = formula.rfind('[').filter(|&a| !formula[a..].contains(']')) {
        return (a + 1, true);
    }
    let ini = formula.char_indices().rev()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_' || *c == '.'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    (ini, false)
}

/// DIN-7: os campos que completam a palavra no fim da fórmula, sem diferença de
/// maiúscula. Depois de um operador, todos; com a fórmula vazia, nenhum (o painel fica limpo).
pub fn sugestoes<'a>(formula: &str, campos: &'a [String]) -> Vec<&'a str> {
    if formula.trim().trim_start_matches('=').is_empty() {
        return Vec::new();
    }
    let (ini, no_colchete) = palavra_em_curso(formula);
    let parte = normalizar(&formula[ini..]);
    // Solto, começar por algarismo é número; entre colchetes pode ser o campo "2024".
    if !no_colchete && parte.starts_with(|c: char| c.is_ascii_digit()) {
        return Vec::new();
    }
    campos.iter().map(String::as_str)
        .filter(|c| { let c = normalizar(c); c.starts_with(&parte) && c != parte })
        .collect()
}

/// Troca a palavra no fim da fórmula pelo campo; entre colchetes quando o nome
/// não é uma palavra solta para o leitor (espaço, hífen, começa por número).
pub fn completar(formula: &str, campo: &str) -> String {
    let (ini, no_colchete) = palavra_em_curso(formula);
    let solta = campo.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && campo.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let antes = &formula[..ini];
    if no_colchete {
        format!("{antes}{campo}]")
    } else if solta {
        format!("{antes}{campo}")
    } else {
        format!("{antes}[{campo}]")
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campos() -> Vec<String> {
        vec!["ValorA".into(), "ValorB".into(), "Valor C".into()]
    }

    fn conta(formula: &str, linha: [f64; 3]) -> f64 {
        let e = compilar(formula, &campos()).unwrap();
        avaliar(&e, &|i| linha[i])
    }

    #[test]
    fn campos_calculados_encadeados() {
        let formulas = vec![
            ("Lucro".to_string(), "ValorA - ValorB".to_string()),
            ("Margem".to_string(), "Lucro / ValorA".to_string()),
        ];
        let expressoes = compilar_sequencia(&campos(), &formulas).unwrap();
        assert_eq!(avaliar_sequencia(&expressoes, &[10.0, 4.0, 0.0]), vec![6.0, 0.6]);
        assert!(compilar_sequencia(&campos(), &[("ValorA".into(), "1".into())]).is_err());
        assert!(compilar_sequencia(&campos(), &[("X".into(), "Y + 1".into())]).is_err());
    }

    #[test]
    fn soma_precedencia_parenteses_e_sinal() {
        assert_eq!(conta("ValorA + ValorB", [2.0, 3.0, 0.0]), 5.0);
        assert_eq!(conta("=valora+VALORB*2", [2.0, 3.0, 0.0]), 8.0);
        assert_eq!(conta("(ValorA + ValorB) * 2", [2.0, 3.0, 0.0]), 10.0);
        assert_eq!(conta("-[Valor C] / 4 + 0,5", [0.0, 0.0, 2.0]), 0.0);
        assert_eq!(conta("ValorA / ValorB", [1.0, 0.0, 0.0]), 0.0, "dividir por zero vale 0");
    }

    #[test]
    fn erros_de_escrita_sao_explicados() {
        assert!(compilar("ValorA + Nada", &campos()).unwrap_err().contains("Nada"));
        assert!(compilar("(ValorA", &campos()).is_err());
        assert!(compilar("ValorA +", &campos()).is_err());
        assert!(compilar("", &campos()).is_err());
        assert!(compilar("ValorA ValorB", &campos()).is_err());
    }

    #[test]
    fn parser_rejeita_literais_infinito_igual_ao_typescript() {
        let colunas = campos();
        let numero_gigante = "9".repeat(400);
        let erro = compilar(&numero_gigante, &colunas).unwrap_err();
        assert!(erro.contains("número inválido"), "{erro}");
        assert!(compilar(&format!("ValorA + {numero_gigante}"), &colunas).is_err());
        assert!(compilar(&format!("-{numero_gigante}"), &colunas).is_err());

        // Uma literal extensa mas finita continua sendo aceita.
        let valida = compilar("12345678901234567890", &colunas).unwrap();
        assert!(avaliar(&valida, &|_| 0.0).is_finite());
        assert_eq!(conta("[Valor C] + 1,5", [0.0, 0.0, 2.0]), 3.5);
    }

    #[test]
    fn numeros_malformados_nao_viram_campos_validos() {
        let colunas = campos();
        assert!(compilar("1,2,3", &colunas).is_err());
        assert!(compilar("3..1", &colunas).is_err());
        assert_eq!(conta("=ValorA*2 + ValorB", [4.0, 3.0, 0.0]), 11.0);
    }

    #[test]
    fn celula_vira_numero() {
        assert_eq!(numero_de(&Value::Empty), 0.0);
        assert_eq!(numero_de(&Value::Text("x".into())), 0.0);
        assert_eq!(numero_de(&Value::Number(2.5)), 2.5);
    }

    #[test]
    fn sugere_e_completa_o_nome_do_campo() {
        let campos: Vec<String> = ["Preço", "Quantidade", "Custo total", "Qtd"].map(String::from).to_vec();
        assert!(sugestoes("", &campos).is_empty());
        assert_eq!(sugestoes("=Preço * ", &campos).len(), 4);
        assert_eq!(sugestoes("=[Preço] * q", &campos), ["Quantidade", "Qtd"]);
        assert_eq!(sugestoes("=[cus", &campos), ["Custo total"]);
        assert!(sugestoes("=Preço * 2", &campos).is_empty(), "número não sugere");
        assert!(sugestoes("=Qtd", &campos).is_empty(), "já completo");
        let anos: Vec<String> = ["2024", "2025"].map(String::from).to_vec();
        assert_eq!(sugestoes("=[2024] + [20", &anos), ["2024", "2025"]);
        assert_eq!(completar("=[2024] + [20", "2025"), "=[2024] + [2025]");
        assert_eq!(completar("=[Preço] * q", "Quantidade"), "=[Preço] * Quantidade");
        assert_eq!(completar("=Preço + cus", "Custo total"), "=Preço + [Custo total]");
        assert_eq!(completar("=[cus", "Custo total"), "=[Custo total]");
        let f = completar("=pr", "Preço") + "*";
        let f = completar(&f, "Quantidade");
        assert_eq!(f, "=Preço*Quantidade");
        assert!(compilar(&f, &campos).is_ok());
    }
}
