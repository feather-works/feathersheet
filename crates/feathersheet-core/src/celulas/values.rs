//! O valor de uma célula, e as coerções de planilha.
//!
//! Tradução de `src/core/values.ts`.

use std::cmp::Ordering;

use crate::celulas::number::{format_number, from_js_string};

/// Os códigos de erro que aparecem na célula.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ErrorCode {
    Div0,
    Value,
    Ref,
    Name,
    Error,
    Num,
    Na,
    Spill,
    Circular,
    /// Não é erro: a resposta ainda está a caminho.
    Calculating,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Div0 => "#DIV/0!",
            ErrorCode::Value => "#VALUE!",
            ErrorCode::Ref => "#REF!",
            ErrorCode::Name => "#NAME?",
            ErrorCode::Error => "#ERROR!",
            ErrorCode::Num => "#NUM!",
            ErrorCode::Na => "#N/A",
            ErrorCode::Spill => "#SPILL!",
            ErrorCode::Circular => "#CIRCULAR!",
            ErrorCode::Calculating => "#CALCULANDO!",
        }
    }

    /// O caminho de volta: `"#DIV/0!"` vira [`ErrorCode::Div0`]. Serve para
    /// ler o literal de erro que veio da fórmula ou do arquivo.
    pub fn from_code(texto: &str) -> Option<ErrorCode> {
        Some(match texto {
            "#DIV/0!" => ErrorCode::Div0,
            "#VALUE!" => ErrorCode::Value,
            "#REF!" => ErrorCode::Ref,
            "#NAME?" => ErrorCode::Name,
            "#ERROR!" => ErrorCode::Error,
            "#NUM!" => ErrorCode::Num,
            "#N/A" => ErrorCode::Na,
            "#SPILL!" => ErrorCode::Spill,
            "#CIRCULAR!" => ErrorCode::Circular,
            "#CALCULANDO!" => ErrorCode::Calculating,
            // Os nomes do Excel em português, digitados ou vindos de arquivo.
            "#N/D" => ErrorCode::Na,
            "#VALOR!" => ErrorCode::Value,
            "#NOME?" => ErrorCode::Name,
            "#NÚM!" => ErrorCode::Num,
            _ => return None,
        })
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Um erro na célula, com o motivo quando há um para contar.
///
/// O `detail` é o que o "Explicar este erro" mostra; ele não entra na
/// comparação, porque duas células com `#VALUE!` são o mesmo erro para quem
/// escreve `=A1=B1`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CellError {
    pub code: ErrorCode,
    pub detail: Option<String>,
}

impl CellError {
    pub fn new(code: ErrorCode) -> Self {
        CellError { code, detail: None }
    }

    pub fn com_motivo(code: ErrorCode, detail: impl Into<String>) -> Self {
        CellError { code, detail: Some(detail.into()) }
    }
}

impl PartialEq for CellError {
    fn eq(&self, outro: &Self) -> bool {
        self.code == outro.code
    }
}

impl Eq for CellError {}

impl std::fmt::Display for CellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code.as_str())
    }
}

/// Valor já calculado de uma célula. [`Value::Empty`] é a célula vazia.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    Error(CellError),
}

impl Value {
    pub fn erro(code: ErrorCode) -> Value {
        Value::Error(CellError::new(code))
    }

    pub fn texto(t: impl Into<String>) -> Value {
        Value::Text(t.into())
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }
}

/// Resultado em matriz — o que as funções dinâmicas (FILTRO, ÚNICO,
/// SEQUÊNCIA) devolvem. A célula da fórmula fica com o canto superior
/// esquerdo e o resto "derrama" para as vizinhas, como no Excel.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    rows: Vec<Vec<Value>>,
}

impl Matrix {
    pub fn new(rows: Vec<Vec<Value>>) -> Matrix {
        Matrix { rows }
    }

    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, |l| l.len())
    }

    pub fn size(&self) -> usize {
        self.height() * self.width()
    }

    /// Fora da matriz devolve vazio, em vez de estourar — é o `?? null` do
    /// TypeScript, e o derramamento conta com isso nas bordas.
    pub fn at(&self, row: usize, col: usize) -> &Value {
        self.rows
            .get(row)
            .and_then(|l| l.get(col))
            .unwrap_or(&Value::Empty)
    }

    /// Primeiro valor: é o que sobra quando a matriz cai num lugar escalar.
    pub fn first(&self) -> &Value {
        self.at(0, 0)
    }

    /// Percorre em ordem de leitura.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.rows.iter().flat_map(|l| l.iter())
    }
}

/// Uma função escrita na própria planilha, como as que `LET`, `MAP` e
/// `REDUCE` recebem. Guarda o ambiente em que nasceu, para o corpo continuar
/// enxergando os nomes de lá.
///
/// Mora aqui, junto do `Computed`, porque é isso que ela é: um valor que por
/// acaso é uma função. No TypeScript ela vivia no avaliador e entrava no
/// `Result` pela porta dos fundos, que o `any` abria.
#[derive(Clone, Debug, PartialEq)]
pub struct Lambda {
    pub params: Vec<String>,
    pub body: crate::formula::ast::Node,
    pub closure: std::collections::HashMap<String, Computed>,
}

/// Resultado bruto de uma avaliação: escalar, matriz ou função.
///
/// No TypeScript isto era `Value | Matrix`, e `isMatrix()` separava os dois.
#[derive(Clone, Debug, PartialEq)]
pub enum Computed {
    Scalar(Value),
    Matrix(Matrix),
    Lambda(std::rc::Rc<Lambda>),
}

impl Computed {
    /// Reduz a um valor escalar: de uma matriz sobra o primeiro item.
    ///
    /// Uma função caindo num lugar escalar vira `#VALUE!` — o TypeScript
    /// deixava o objeto passar adiante e ele aparecia como `[object Object]`
    /// na célula, que não é resposta melhor.
    pub fn scalar(self) -> Value {
        match self {
            Computed::Scalar(v) => v,
            Computed::Matrix(m) => m.first().clone(),
            Computed::Lambda(_) => Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "uma função não é um valor de célula",
            )),
        }
    }

    pub fn is_matrix(&self) -> bool {
        matches!(self, Computed::Matrix(_))
    }

    pub fn as_matrix(&self) -> Option<&Matrix> {
        match self {
            Computed::Matrix(m) => Some(m),
            _ => None,
        }
    }

    /// O erro que este resultado carrega, se carregar.
    pub fn error(&self) -> Option<&CellError> {
        match self {
            Computed::Scalar(Value::Error(e)) => Some(e),
            _ => None,
        }
    }

    /// Altura e largura, contando escalar como 1×1.
    pub fn shape(&self) -> (usize, usize) {
        match self {
            Computed::Matrix(m) => (m.height(), m.width()),
            _ => (1, 1),
        }
    }

    /// O item (linha, coluna), repetindo o escalar e esticando a matriz de uma
    /// linha só ou de uma coluna só — é o que faz a conta item a item andar.
    pub fn pick(&self, row: usize, col: usize) -> Value {
        match self {
            Computed::Matrix(m) => {
                let r = if m.height() == 1 { 0 } else { row };
                let c = if m.width() == 1 { 0 } else { col };
                m.at(r, c).clone()
            }
            Computed::Scalar(v) => v.clone(),
            Computed::Lambda(_) => Value::Error(CellError::new(ErrorCode::Value)),
        }
    }
}

impl From<Value> for Computed {
    fn from(v: Value) -> Computed {
        Computed::Scalar(v)
    }
}

impl From<Matrix> for Computed {
    fn from(m: Matrix) -> Computed {
        Computed::Matrix(m)
    }
}

/// Tenta ler texto de moeda ("R$ 5,00", "R$ 1.250,50", "-R$ 5,00") como
/// número. O formato que sai daqui é sempre "moeda", então só o valor volta.
pub fn parse_currency_literal(bruto: &str) -> Option<f64> {
    if !tem_simbolo_de_moeda(bruto) {
        return None;
    }
    let mut t = bruto.trim();
    let mut negativo = false;

    if let Some(resto) = t.strip_prefix('-') {
        negativo = true;
        t = resto.trim();
    } else if t.starts_with('(') && t.ends_with(')') {
        negativo = true;
        t = t[1..t.len() - 1].trim();
    }

    t = tirar_simbolo(t)?.trim();

    if let Some(resto) = t.strip_prefix('-') {
        negativo = !negativo;
        t = resto.trim();
    } else if t.starts_with('(') && t.ends_with(')') {
        negativo = !negativo;
        t = t[1..t.len() - 1].trim();
    }

    if t.is_empty() {
        return None;
    }

    // "1.250,50" -> "1250.50"; "1.250" -> "1250".
    let numero = if t.contains(',') {
        t.replace('.', "").replacen(',', ".", 1)
    } else if so_milhar_com_ponto(t) {
        t.replace('.', "")
    } else {
        t.to_string()
    };

    let val = from_js_string(&numero)?;
    Some(if negativo { -val } else { val })
}

fn tem_simbolo_de_moeda(bruto: &str) -> bool {
    let baixo = bruto.to_ascii_lowercase();
    baixo.contains("r$") || baixo.contains('$')
}

/// Tira o "R$" ou o "$" da frente, sem ligar para maiúscula.
fn tirar_simbolo(t: &str) -> Option<&str> {
    let b = t.as_bytes();
    if b.len() >= 2 && (b[0] == b'R' || b[0] == b'r') && b[1] == b'$' {
        return Some(&t[2..]);
    }
    if !b.is_empty() && b[0] == b'$' {
        return Some(&t[1..]);
    }
    None
}

/// `^\d{1,3}(\.\d{3})+$` — o ponto usado só como separador de milhar.
fn so_milhar_com_ponto(t: &str) -> bool {
    let mut partes = t.split('.');
    let Some(primeira) = partes.next() else {
        return false;
    };
    if primeira.is_empty() || primeira.len() > 3 || !primeira.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut teve_grupo = false;
    for parte in partes {
        if parte.len() != 3 || !parte.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        teve_grupo = true;
    }
    teve_grupo
}

/// Coercao numerica de core/values.ts (diferente do leitor de literal).
/// O legado tenta primeiro retirar todos os pontos e trocar a primeira
/// virgula; se o resultado nao for finito, tenta so a troca da virgula.
fn numero_texto_legado(texto: &str) -> Option<f64> {
    let primeira = texto.replace('.', "").replacen(',', ".", 1);
    from_js_string(&primeira)
        .or_else(|| from_js_string(&texto.replacen(',', ".", 1)))
}

/// Coerção para número seguindo as regras de planilha.
pub fn to_number(v: &Value) -> Result<f64, CellError> {
    match v {
        Value::Empty => Ok(0.0),
        Value::Number(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Error(e) => Err(e.clone()),
        Value::Text(texto) => {
            let t = texto.trim();
            if t.is_empty() {
                return Ok(0.0);
            }
            if let Some(moeda) = parse_currency_literal(t) {
                return Ok(moeda);
            }
            // Segue a ordem exata do core/values.ts antigo: primeiro
            // remove todos os pontos, troca a primeira virgula e tenta Number;
            // se falhar, tenta apenas trocar a primeira virgula.
            // Nao confundir com parse_number_str (digitacao de celulas):
            // o conversor de funcoes legado tem semantica propria.
            if let Some(inner) = t.strip_suffix('%') {
                if let Some(n) = numero_texto_legado(&inner.trim().replace('.', "")) {
                    return Ok(n / 100.0);
                }
            }
            match numero_texto_legado(t) {
                Some(n) => Ok(n),
                None => Err(CellError::com_motivo(
                    ErrorCode::Value,
                    format!("\"{texto}\" não é número"),
                )),
            }
        }
    }
}

pub fn to_text(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Text(t) => t.clone(),
        Value::Bool(b) => if *b { "VERDADEIRO" } else { "FALSO" }.to_string(),
        Value::Error(e) => e.code.as_str().to_string(),
        Value::Number(n) => format_number(*n),
    }
}

pub fn to_boolean(v: &Value) -> Result<bool, CellError> {
    match v {
        Value::Empty => Ok(false),
        Value::Bool(b) => Ok(*b),
        Value::Number(n) => Ok(*n != 0.0),
        Value::Error(e) => Err(e.clone()),
        Value::Text(texto) => {
            let t = texto.trim().to_uppercase();
            match t.as_str() {
                "VERDADEIRO" | "TRUE" => Ok(true),
                "FALSO" | "FALSE" | "" => Ok(false),
                _ => Err(CellError::new(ErrorCode::Value)),
            }
        }
    }
}

/// Ordem de comparação entre tipos, como no Excel: número < texto < booleano.
fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Empty => 0,
        Value::Number(_) => 1,
        Value::Text(_) => 2,
        _ => 3,
    }
}

/// Texto sem ligar para maiúscula. Os dois em ASCII (o caso comum): byte a byte,
/// sem alocar. Fora disso, o `to_lowercase` de sempre, que tem regras que a
/// comparação letra a letra não tem (o sigma final grego).
fn comparar_texto(x: &str, y: &str) -> Ordering {
    if x.is_ascii() && y.is_ascii() {
        x.bytes().map(|b| b.to_ascii_lowercase()).cmp(y.bytes().map(|b| b.to_ascii_lowercase()))
    } else {
        x.to_lowercase().cmp(&y.to_lowercase())
    }
}

/// Compara dois valores — ou devolve o erro, se algum dos lados for um.
pub fn compare(a: &Value, b: &Value) -> Result<Ordering, CellError> {
    if let Value::Error(e) = a {
        return Err(e.clone());
    }
    if let Value::Error(e) = b {
        return Err(e.clone());
    }
    if a.is_empty() && b.is_empty() {
        return Ok(Ordering::Equal);
    }
    // Célula vazia se comporta como o zero, o texto vazio ou o FALSO do outro
    // lado — o tipo de quem está preenchido é que manda.
    let vazio_como = |outro: &Value| match outro {
        Value::Number(_) => Value::Number(0.0),
        Value::Bool(_) => Value::Bool(false),
        _ => Value::Text(String::new()),
    };
    // Emprestado: clonar os dois lados custava duas alocações por comparação
    // (o `(C="Sul")` do SOMARPRODUTO faz uma por linha).
    let a: std::borrow::Cow<'_, Value> = if a.is_empty() { std::borrow::Cow::Owned(vazio_como(b)) } else { std::borrow::Cow::Borrowed(a) };
    let b: std::borrow::Cow<'_, Value> = if b.is_empty() { std::borrow::Cow::Owned(vazio_como(&a)) } else { std::borrow::Cow::Borrowed(b) };

    let ra = type_rank(&a);
    let rb = type_rank(&b);
    if ra != rb {
        return Ok(ra.cmp(&rb));
    }
    Ok(match (&*a, &*b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => comparar_texto(x, y),
        _ => {
            // Texto compara sem ligar para maiúscula, como no TypeScript.
            let sa = to_text(&a).to_lowercase();
            let sb = to_text(&b).to_lowercase();
            sa.cmp(&sb)
        }
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn coercoes_numericas_fieis_ao_typescript_legado() {
        // Sao as regras de toNumber do TS, NAO as de digitar um literal.
        for (texto, esperado) in [
            ("1,5", 1.5),
            ("1.234,56", 1234.56),
            ("1.234", 1234.0),
            ("1.5", 15.0),
            (".5", 5.0),
            ("1,234.56", 1.23456),
            ("2.5%", 0.25),
            ("12,5%", 0.125),
            ("1e3", 1000.0),
            ("R$ 1.250,50", 1250.5),
        ] {
            assert_eq!(to_number(&Value::texto(texto)), Ok(esperado), "entrada: {texto}");
        }
        assert!(to_number(&Value::texto("1,2,3")).is_err());
    }

    #[test]
    fn valores_preservam_textos_logicos_e_propagam_erros() {
        assert_eq!(to_number(&Value::texto("  ")), Ok(0.0));
        assert_eq!(to_number(&Value::Bool(false)), Ok(0.0));
        assert_eq!(to_boolean(&Value::texto("FALSE")), Ok(false));
        assert_eq!(to_text(&Value::Bool(true)), "VERDADEIRO");
        let erro = CellError::com_motivo(ErrorCode::Ref, "origem inválida");
        assert_eq!(to_number(&Value::Error(erro.clone())), Err(erro.clone()));
        assert_eq!(to_boolean(&Value::Error(erro.clone())), Err(erro.clone()));
        assert_eq!(compare(&Value::Error(erro), &Value::Number(0.0)).unwrap_err().code, ErrorCode::Ref);
    }

    #[test]
    fn numero_de_texto() {
        assert_eq!(to_number(&Value::texto("12")), Ok(12.0));
        assert_eq!(to_number(&Value::texto("0,5")), Ok(0.5));
        assert_eq!(to_number(&Value::texto("  ")), Ok(0.0));
        assert_eq!(to_number(&Value::Empty), Ok(0.0));
        assert_eq!(to_number(&Value::Bool(true)), Ok(1.0));
        // Duas vírgulas não são número: só a primeira viraria ponto.
        assert!(to_number(&Value::texto("1,2,3")).is_err());
        let e = to_number(&Value::texto("abc")).unwrap_err();
        assert_eq!(e.code, ErrorCode::Value);
        assert_eq!(e.detail.as_deref(), Some("\"abc\" não é número"));
    }

    #[test]
    fn moeda_vira_numero() {
        assert_eq!(parse_currency_literal("R$ 5,00"), Some(5.0));
        assert_eq!(parse_currency_literal("R$ 1.250,50"), Some(1250.5));
        assert_eq!(parse_currency_literal("-R$ 5,00"), Some(-5.0));
        assert_eq!(parse_currency_literal("R$ -5,00"), Some(-5.0));
        assert_eq!(parse_currency_literal("(R$ 5,00)"), Some(-5.0));
        assert_eq!(parse_currency_literal("$1.250"), Some(1250.0));
        assert_eq!(parse_currency_literal("r$7"), Some(7.0));
        assert_eq!(parse_currency_literal("5,00"), None);
        assert_eq!(parse_currency_literal("R$"), None);
        assert_eq!(to_number(&Value::texto("R$ 1.250,50")), Ok(1250.5));
    }

    #[test]
    fn texto_de_valor() {
        assert_eq!(to_text(&Value::Empty), "");
        assert_eq!(to_text(&Value::Bool(true)), "VERDADEIRO");
        assert_eq!(to_text(&Value::Bool(false)), "FALSO");
        assert_eq!(to_text(&Value::Number(1.5)), "1.5");
        assert_eq!(to_text(&Value::erro(ErrorCode::Div0)), "#DIV/0!");
    }

    #[test]
    fn logico_de_valor() {
        assert_eq!(to_boolean(&Value::texto("verdadeiro")), Ok(true));
        assert_eq!(to_boolean(&Value::texto("FALSE")), Ok(false));
        assert_eq!(to_boolean(&Value::texto("")), Ok(false));
        assert_eq!(to_boolean(&Value::Number(0.0)), Ok(false));
        assert_eq!(to_boolean(&Value::Number(-2.0)), Ok(true));
        assert!(to_boolean(&Value::texto("talvez")).is_err());
    }

    #[test]
    fn ordem_entre_tipos() {
        // número < texto < booleano
        assert_eq!(compare(&Value::Number(9.0), &Value::texto("a")), Ok(Ordering::Less));
        assert_eq!(compare(&Value::texto("a"), &Value::Bool(false)), Ok(Ordering::Less));
        assert_eq!(compare(&Value::Number(1.0), &Value::Number(1.0)), Ok(Ordering::Equal));
        assert_eq!(compare(&Value::texto("A"), &Value::texto("a")), Ok(Ordering::Equal));
    }

    #[test]
    fn vazio_imita_o_outro_lado() {
        assert_eq!(compare(&Value::Empty, &Value::Number(0.0)), Ok(Ordering::Equal));
        assert_eq!(compare(&Value::Empty, &Value::texto("")), Ok(Ordering::Equal));
        assert_eq!(compare(&Value::Empty, &Value::Bool(false)), Ok(Ordering::Equal));
        assert_eq!(compare(&Value::Empty, &Value::Empty), Ok(Ordering::Equal));
        assert_eq!(compare(&Value::Empty, &Value::Number(5.0)), Ok(Ordering::Less));
    }

    #[test]
    fn erro_contamina_a_comparacao() {
        let erro = Value::erro(ErrorCode::Na);
        assert!(compare(&erro, &Value::Number(1.0)).is_err());
        assert!(compare(&Value::Number(1.0), &erro).is_err());
    }

    #[test]
    fn matriz_fora_da_borda_e_vazia() {
        let m = Matrix::new(vec![vec![Value::Number(1.0), Value::Number(2.0)]]);
        assert_eq!(m.height(), 1);
        assert_eq!(m.width(), 2);
        assert_eq!(m.size(), 2);
        assert_eq!(m.at(0, 1), &Value::Number(2.0));
        assert_eq!(m.at(9, 9), &Value::Empty);
        assert_eq!(m.first(), &Value::Number(1.0));
        assert_eq!(Computed::Matrix(m).scalar(), Value::Number(1.0));
    }
}
