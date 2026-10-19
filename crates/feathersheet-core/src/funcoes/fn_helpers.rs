//! O que toda função da planilha usa: aridade, coleta de argumentos e
//! critérios.
//!
//! Tradução de `src/core/fn-helpers.ts`.

use std::cmp::Ordering;

use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::celulas::values::{
    compare, to_boolean, to_number, to_text, CellError, Computed, ErrorCode, Matrix, Value,
};
use crate::formula::evaluator::broadcast_all;

/// A assinatura de toda função da planilha.
///
/// Os argumentos chegam **como árvore**, não avaliados, e isso é de propósito:
/// o `SE` não pode calcular o ramo que não vai usar, e quem soma um intervalo
/// precisa saber que aquilo era um intervalo para percorrer só as células que
/// existem, em vez de materializar dez milhões de valores.
pub type FnImpl = fn(&[Node], &Evaluator<'_>) -> Computed;

/// Se o argumento `i` foi escrito (um argumento em branco não conta).
pub fn has(args: &[Node], i: usize) -> bool {
    !matches!(args.get(i), None | Some(Node::Blank))
}

pub fn err_value() -> Computed {
    Computed::Scalar(Value::erro(ErrorCode::Value))
}

pub fn err_na() -> Computed {
    Computed::Scalar(Value::erro(ErrorCode::Na))
}

pub fn err_num() -> Computed {
    Computed::Scalar(Value::erro(ErrorCode::Num))
}

/// Confere quantos argumentos vieram. `max` igual a `min` quer dizer "exatos".
pub fn arity(args: &[Node], min: usize, max: usize) -> Option<CellError> {
    if args.len() < min || args.len() > max {
        return Some(CellError::com_motivo(
            ErrorCode::Value,
            format!("Número de argumentos inválido ({})", args.len()),
        ));
    }
    None
}

/// Todos os números de todos os argumentos, achatados.
pub fn collect(args: &[Node], ev: &Evaluator<'_>) -> Result<Vec<f64>, CellError> {
    let mut out = Vec::new();
    for a in args {
        if let Some(e) = ev.for_each_number(a, &mut |n| out.push(n)) {
            return Err(e);
        }
    }
    Ok(out)
}

/// Um critério de `SOMASE`/`CONT.SE` já transformado em pergunta.
pub type Predicate = Box<dyn Fn(&Value) -> bool>;

/// Um intervalo lido de uma vez, quando compensa
/// ([`crate::formula::evaluator::EvalContext::faixa_lida`]); senão célula
/// por célula, como antes.
pub struct LeitorFaixa {
    sheet: Option<String>,
    lida: Option<std::sync::Arc<crate::planilha::faixa_lida::FaixaLida>>,
}

impl LeitorFaixa {
    pub fn novo(ev: &Evaluator<'_>, rect: &crate::celulas::refs::Rect) -> Self {
        LeitorFaixa { sheet: rect.sheet.clone(), lida: ev.ctx.faixa_lida(rect) }
    }

    /// A faixa lida de uma vez, se houver.
    pub fn lida(&self) -> Option<&std::sync::Arc<crate::planilha::faixa_lida::FaixaLida>> {
        self.lida.as_ref()
    }

    /// O valor da célula (`row`, `col`) da aba do intervalo.
    pub fn valor(&self, ev: &Evaluator<'_>, row: u32, col: u32) -> std::borrow::Cow<'_, Value> {
        match &self.lida {
            Some(f) => std::borrow::Cow::Borrowed(f.valor(row, col)),
            None => std::borrow::Cow::Owned(ev.ctx.get_value(row, col, self.sheet.as_deref())),
        }
    }
}

/// Os pares `(faixa; critério)` de `args[inicio..]`: as faixas e os
/// critérios já avaliados. Erro de critério ou faixa inválida vira `#VALOR!`
/// (ou o erro do critério), como as funções faziam uma a uma.
pub fn faixas_e_criterios(args: &[Node], inicio: usize, ev: &Evaluator<'_>)
    -> Result<(Vec<crate::celulas::refs::Rect>, Vec<Value>), CellError> {
    let mut rects = Vec::with_capacity((args.len() - inicio) / 2);
    let mut crits = Vec::with_capacity(rects.capacity());
    let mut i = inicio;
    while i + 1 < args.len() {
        let rect = node_rect(&args[i], ev).ok_or_else(|| CellError::new(ErrorCode::Value))?;
        let crit = ev.eval(&args[i + 1]);
        if let Value::Error(e) = crit {
            return Err(e);
        }
        rects.push(rect);
        crits.push(crit);
        i += 2;
    }
    Ok((rects, crits))
}

/// O retângulo de um nó de referência ou intervalo.
/// Retângulo de um argumento que é referência: célula, intervalo ou nome
/// definido (variável de LET/LAMBDA não é nome). Funções que exigem
/// intervalo (SOMASES, CORRESP…) leem por aqui, para o nome valer igual.
pub fn node_rect(node: &Node, ev: &crate::formula::evaluator::Evaluator<'_>) -> Option<crate::celulas::refs::Rect> {
    use crate::celulas::refs::Rect;
    match node {
        Node::Ref { row, col, sheet } => Some(Rect { r0: *row, c0: *col, r1: *row, c1: *col, sheet: sheet.clone() }),
        Node::Range { r0, c0, r1, c1, sheet } => Some(Rect { r0: *r0, c0: *c0, r1: *r1, c1: *c1, sheet: sheet.clone() }),
        Node::Name(nome) if !ev.env.contains_key(&nome.to_uppercase()) => ev.ctx.resolve_name(nome),
        _ => None,
    }
}

/// O grupo agregado das faixas para estes critérios, quando todos são de
/// igualdade simples e o contexto tem o agregado montado (ou consegue
/// montar). `None`: a função varre célula por célula, como sempre. Ver
/// [`crate::planilha::agregado`].
pub fn grupo_agregado(ev: &Evaluator<'_>, soma: Option<&crate::celulas::refs::Rect>,
    faixas: &[crate::celulas::refs::Rect], criterios: &[Value]) -> Option<crate::planilha::agregado::Grupo> {
    use crate::planilha::agregado::ChaveCriterio;
    if faixas.is_empty() || faixas.len() != criterios.len() {
        return None;
    }
    let chaves = criterios.iter().map(ChaveCriterio::do_criterio).collect::<Option<Vec<_>>>()?;
    let agregado = ev.ctx.agregado(soma, faixas)?;
    Some(agregado.grupo(&chaves))
}

/// Transforma o critério em uma pergunta que se faz a cada valor.
///
/// Três formas, na ordem em que são reconhecidas: comparação (`">10"`,
/// `"<>x"`), curinga (`"a*"`, `"b?"`) e igualdade simples.
pub fn make_predicate(criterion: &Value) -> Predicate {
    if let Value::Text(texto) = criterion {
        if let Some((op, resto)) = partir_comparacao(texto.trim()) {
            let alvo = alvo_do_criterio(resto);
            return Box::new(move |v| {
                let Ok(ordem) = compare(v, &alvo) else {
                    return false;
                };
                match op {
                    Comparacao::Eq => ordem.is_eq(),
                    Comparacao::Ne => !ordem.is_eq(),
                    Comparacao::Lt => ordem == Ordering::Less,
                    Comparacao::Le => ordem != Ordering::Greater,
                    Comparacao::Gt => ordem == Ordering::Greater,
                    Comparacao::Ge => ordem != Ordering::Less,
                }
            });
        }
        if texto.contains('*') || texto.contains('?') {
            let padrao = compilar_curinga(texto);
            return Box::new(move |v| combina_curinga(&padrao, &to_text(v)));
        }
    }
    // Igualdade simples (o critério mais comum, `"Sul"` ou `1`): o mesmo
    // resultado do `compare`, sem clonar nem passar para minúscula a cada
    // célula — eram 4 textos novos por célula num `SOMASES` de 1 milhão.
    match criterion {
        Value::Number(x) => {
            let x = *x;
            let igual = move |y: f64| y.partial_cmp(&x).is_none_or(Ordering::is_eq);
            return Box::new(move |v| match v {
                Value::Number(y) => igual(*y),
                Value::Empty => igual(0.0),
                _ => false,
            });
        }
        Value::Text(t) => {
            let alvo = t.clone();
            return Box::new(move |v| match v {
                Value::Text(s) => mesmo_texto(s, &alvo),
                Value::Empty => alvo.is_empty(),
                _ => false,
            });
        }
        _ => {}
    }
    let alvo = criterion.clone();
    Box::new(move |v| compare(v, &alvo) == Ok(Ordering::Equal))
}

/// `compare(a, b) == Ok(Equal)` sem alocar nos casos comuns (número e
/// texto): o `PROCX` de 1 milhão de linhas comparava ~100 vezes cada uma.
pub fn valores_iguais(a: &Value, b: &Value) -> bool {
    let num = |x: f64, y: f64| x.partial_cmp(&y).is_none_or(Ordering::is_eq);
    match (a, b) {
        (Value::Error(_), _) | (_, Value::Error(_)) => false,
        (Value::Number(x), Value::Number(y)) => num(*x, *y),
        (Value::Text(s), Value::Text(t)) => mesmo_texto(s, t),
        (Value::Empty, Value::Empty) => true,
        (Value::Empty, Value::Number(y)) | (Value::Number(y), Value::Empty) => num(0.0, *y),
        (Value::Empty, Value::Text(t)) | (Value::Text(t), Value::Empty) => t.is_empty(),
        (Value::Number(_), Value::Text(_)) | (Value::Text(_), Value::Number(_)) => false,
        _ => compare(a, b) == Ok(Ordering::Equal),
    }
}

/// Texto igual sem ligar para maiúscula, como o `compare`, sem alocar.
fn mesmo_texto(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Comparacao {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// `">= 10"` vira `(Ge, "10")`. Os de dois caracteres são testados primeiro,
/// senão `">="` seria lido como `">"` seguido de lixo.
fn partir_comparacao(texto: &str) -> Option<(Comparacao, &str)> {
    let (op, resto) = if let Some(r) = texto.strip_prefix("<=") {
        (Comparacao::Le, r)
    } else if let Some(r) = texto.strip_prefix(">=") {
        (Comparacao::Ge, r)
    } else if let Some(r) = texto.strip_prefix("<>") {
        (Comparacao::Ne, r)
    } else if let Some(r) = texto.strip_prefix('=') {
        (Comparacao::Eq, r)
    } else if let Some(r) = texto.strip_prefix('<') {
        (Comparacao::Lt, r)
    } else if let Some(r) = texto.strip_prefix('>') {
        (Comparacao::Gt, r)
    } else {
        return None;
    };
    Some((op, resto.trim_start()))
}

/// O que vem depois do operador: número quando dá, texto quando não dá.
fn alvo_do_criterio(resto: &str) -> Value {
    if resto.is_empty() || resto.trim().is_empty() {
        return Value::Text(resto.to_string());
    }
    match crate::celulas::number::from_js_string(&resto.replacen(',', ".", 1)) {
        Some(n) => Value::Number(n),
        None => Value::Text(resto.to_string()),
    }
}

/// Um pedaço de um curinga já entendido.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pedaco {
    /// `*` — qualquer coisa, inclusive nada.
    Tudo,
    /// `?` — um caractere qualquer.
    Um,
    /// Um caractere literal, já em minúscula.
    Igual(char),
}

/// `~*` e `~?` escapam o curinga, como no Excel.
fn compilar_curinga(padrao: &str) -> Vec<Pedaco> {
    let cs: Vec<char> = padrao.chars().collect();
    let mut out = Vec::with_capacity(cs.len());
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '~' && matches!(cs.get(i + 1), Some('*') | Some('?')) {
            out.push(Pedaco::Igual(minuscula(cs[i + 1])));
            i += 2;
            continue;
        }
        out.push(match c {
            '*' => Pedaco::Tudo,
            '?' => Pedaco::Um,
            _ => Pedaco::Igual(minuscula(c)),
        });
        i += 1;
    }
    out
}

fn minuscula(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// O RegExp do TypeScript gerava `.` e `.*` sem a flag `s`.
/// Portanto, nenhum curinga pode consumir os terminadores de linha
/// reconhecidos pela especificação do JavaScript, inclusive U+2028/U+2029.
fn terminador_de_linha_js(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Casamento de curinga, ancorado nas duas pontas e sem ligar para maiúscula.
///
/// É o algoritmo de dois ponteiros com volta atrás: o `*` guarda onde estava e
/// avança um caractere de cada vez quando o resto não fecha. Não usa expressão
/// regular porque o crate não tem dependência — e porque transformar texto da
/// pessoa em expressão regular é como se convida uma injeção para entrar.
fn combina_curinga(padrao: &[Pedaco], texto: &str) -> bool {
    let t: Vec<char> = texto.chars().map(minuscula).collect();
    let (mut ti, mut pi) = (0usize, 0usize);
    let mut volta: Option<(usize, usize)> = None;

    while ti < t.len() {
        match padrao.get(pi) {
            Some(Pedaco::Um) if !terminador_de_linha_js(t[ti]) => {
                ti += 1;
                pi += 1;
            }
            Some(Pedaco::Igual(c)) if *c == t[ti] => {
                ti += 1;
                pi += 1;
            }
            Some(Pedaco::Tudo) => {
                volta = Some((pi, ti));
                pi += 1;
            }
            _ => match volta {
                Some((p, t0)) => {
                    // Um '*' não pode atravessar o terminador de linha:
                    // no TS ele foi compilado para '.*', não para '[\s\S]*'.
                    if terminador_de_linha_js(t[t0]) { return false; }
                    pi = p + 1;
                    ti = t0 + 1;
                    volta = Some((p, t0 + 1));
                }
                None => return false,
            },
        }
    }
    padrao[pi.min(padrao.len())..]
        .iter()
        .all(|p| *p == Pedaco::Tudo)
}

// ---- funções por valor: espalhar sobre matrizes e ler argumentos ----
//
// Para as funções que não olham o intervalo célula a célula (engenharia,
// financeiras, distribuições, datas, texto…):
//
// - **escalar** (HORA, BIN2DEC, DIST.BETA…): [`por_elemento`] avalia os
//   argumentos, espalha sobre matrizes como o Excel (`=HORA(A1:A3)` devolve
//   três horas) e chama a conta uma vez por posição;
// - **de lista** (VPL, MÉDIAA, SOMAQUAD…): lê com [`numeros_de`] /
//   [`valores_de`] e devolve com [`concluir`].
//
// A conta devolve `Res<Value>` e o `?` leva o erro adiante.

/// Resultado de uma conta: valor ou o erro que a planilha vai mostrar.
pub type Res<T> = Result<T, CellError>;

/// Erro sem explicação extra.
pub fn erro(code: ErrorCode) -> CellError {
    CellError::new(code)
}

/// `#NUM!` — argumento fora do domínio ou conta que não converge.
pub fn e_num() -> CellError {
    erro(ErrorCode::Num)
}

/// `#VALOR!` — argumento do tipo errado.
pub fn e_valor() -> CellError {
    erro(ErrorCode::Value)
}

/// `#N/D` — nada encontrado.
pub fn e_nd() -> CellError {
    erro(ErrorCode::Na)
}

/// `#DIV/0!`.
pub fn e_div0() -> CellError {
    erro(ErrorCode::Div0)
}

/// Número de resposta: infinito ou NaN nunca chega à célula, vira `#NUM!`.
pub fn numero(n: f64) -> Res<Value> {
    if n.is_finite() {
        Ok(Value::Number(n))
    } else {
        Err(e_num())
    }
}

/// Texto de resposta.
pub fn texto(s: impl Into<String>) -> Res<Value> {
    Ok(Value::Text(s.into()))
}

/// Converte o resultado da conta no que o avaliador espera.
pub fn concluir(r: Res<Value>) -> Computed {
    match r {
        Ok(v) => Computed::Scalar(v),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// Converte uma resposta em matriz (linhas × colunas).
pub fn concluir_matriz(r: Res<Vec<Vec<Value>>>) -> Computed {
    match r {
        Ok(linhas) if linhas.is_empty() || linhas[0].is_empty() => {
            Computed::Scalar(Value::erro(ErrorCode::Value))
        }
        Ok(linhas) => Computed::Matrix(Matrix::new(linhas)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// Os argumentos já avaliados de uma posição: `None` é argumento omitido
/// (ou deixado em branco, `F(1;;3)`), que a função troca pelo padrão dela.
pub struct Entradas<'v> {
    valores: &'v [Value],
    presentes: &'v [bool],
}

impl Entradas<'_> {
    /// Quantos argumentos foram escritos na fórmula (contando os em branco).
    pub fn len(&self) -> usize {
        self.valores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.valores.is_empty()
    }

    /// Se o argumento `i` foi escrito e não está em branco.
    pub fn tem(&self, i: usize) -> bool {
        self.presentes.get(i).copied().unwrap_or(false)
    }

    /// O valor do argumento `i` (vazio se omitido).
    pub fn valor(&self, i: usize) -> &Value {
        self.valores.get(i).unwrap_or(&Value::Empty)
    }

    /// Número obrigatório.
    pub fn num(&self, i: usize) -> Res<f64> {
        num(self.valor(i))
    }

    /// Número opcional com padrão.
    pub fn num_ou(&self, i: usize, padrao: f64) -> Res<f64> {
        if self.tem(i) {
            self.num(i)
        } else {
            Ok(padrao)
        }
    }

    /// Inteiro obrigatório (a parte decimal é cortada, como no Excel).
    pub fn int(&self, i: usize) -> Res<i64> {
        inteiro(self.valor(i))
    }

    /// Inteiro opcional com padrão.
    pub fn int_ou(&self, i: usize, padrao: i64) -> Res<i64> {
        if self.tem(i) {
            self.int(i)
        } else {
            Ok(padrao)
        }
    }

    /// Lógico obrigatório.
    pub fn logico(&self, i: usize) -> Res<bool> {
        logico(self.valor(i))
    }

    /// Lógico opcional com padrão.
    pub fn logico_ou(&self, i: usize, padrao: bool) -> Res<bool> {
        if self.tem(i) {
            self.logico(i)
        } else {
            Ok(padrao)
        }
    }

    /// Texto obrigatório (número vira texto, erro propaga).
    pub fn texto(&self, i: usize) -> Res<String> {
        texto_de(self.valor(i))
    }

    /// Texto opcional com padrão.
    pub fn texto_ou(&self, i: usize, padrao: &str) -> Res<String> {
        if self.tem(i) {
            self.texto(i)
        } else {
            Ok(padrao.to_string())
        }
    }
}

/// Valor → número, propagando erro (vazio vale 0, texto numérico é lido).
pub fn num(v: &Value) -> Res<f64> {
    to_number(v)
}

/// Valor → inteiro truncado em direção ao zero.
pub fn inteiro(v: &Value) -> Res<i64> {
    let n = num(v)?;
    if !n.is_finite() || n.abs() > 9.0e15 {
        return Err(e_num());
    }
    Ok(n.trunc() as i64)
}

/// Valor → lógico, propagando erro.
pub fn logico(v: &Value) -> Res<bool> {
    to_boolean(v)
}

/// Valor → texto, propagando erro.
pub fn texto_de(v: &Value) -> Res<String> {
    match v {
        Value::Error(e) => Err(e.clone()),
        outro => Ok(to_text(outro)),
    }
}

/// Aplica uma conta escalar a cada posição dos argumentos.
///
/// Confere a quantidade de argumentos (`min..=max`), avalia cada um como
/// matriz (intervalo, expressão de matriz ou valor solto) e espalha: se algum
/// argumento for matriz, a resposta é uma matriz do tamanho do maior.
pub fn por_elemento(
    args: &[Node],
    ev: &Evaluator<'_>,
    min: usize,
    max: usize,
    conta: impl Fn(&Entradas<'_>) -> Res<Value>,
) -> Computed {
    if let Some(e) = arity(args, min, max) {
        return Computed::Scalar(Value::Error(e));
    }
    let presentes: Vec<bool> = args.iter().map(|a| !matches!(a, Node::Blank)).collect();
    let fontes: Vec<Computed> = args
        .iter()
        .map(|a| match a {
            Node::Blank => Computed::Scalar(Value::Empty),
            _ => ev.eval_array(a),
        })
        .collect();
    broadcast_all(&fontes, |vs| {
        match conta(&Entradas { valores: vs, presentes: &presentes }) {
            Ok(v) => v,
            Err(e) => Value::Error(e),
        }
    })
}

/// Os números de um argumento de coleção, do jeito do SOMA: dentro de
/// intervalo ou matriz texto e lógico são ignorados; solto, contam.
pub fn numeros_de(ev: &Evaluator<'_>, node: &Node) -> Res<Vec<f64>> {
    let mut saida = Vec::new();
    match ev.for_each_number(node, &mut |n| saida.push(n)) {
        Some(e) => Err(e),
        None => Ok(saida),
    }
}

/// Os números de vários argumentos, achatados.
pub fn numeros_de_todos(ev: &Evaluator<'_>, args: &[Node]) -> Res<Vec<f64>> {
    let mut saida = Vec::new();
    for a in args {
        saida.extend(numeros_de(ev, a)?);
    }
    Ok(saida)
}

/// Os valores de um argumento de coleção, cada um com a marca de se veio de
/// dentro de um intervalo/matriz (`true`) ou foi escrito solto (`false`).
/// Para as funções com A no fim (MÉDIAA, MÁXIMOA…), que contam texto e
/// lógico de jeitos diferentes conforme a origem.
pub fn valores_de(ev: &Evaluator<'_>, node: &Node) -> Res<Vec<(Value, bool)>> {
    let mut saida = Vec::new();
    match ev.for_each_value(node, &mut |v, de_colecao| saida.push((v.clone(), de_colecao))) {
        Some(e) => Err(e),
        None => Ok(saida),
    }
}

/// O argumento como matriz retangular de valores (célula vazia fica vazia).
pub fn matriz_de(ev: &Evaluator<'_>, node: &Node) -> Res<Matrix> {
    ev.matrix_arg(node)
}

/// Argumento escalar avaliado (para quem não espalha).
pub fn valor_de(ev: &Evaluator<'_>, args: &[Node], i: usize) -> Option<Value> {
    match args.get(i) {
        None | Some(Node::Blank) => None,
        Some(a) => Some(ev.eval(a)),
    }
}

// --- leitura de argumentos um a um ----------------------------------------
//
// Para as funções que leem cada argumento pela posição (sem espalhar sobre
// matrizes): `arg_num(args, ev, 0)?`. Argumento que falta dá `#VALOR!`; um
// argumento em branco explícito (`F(1;;3)`) vale 0 (ou vazio/falso/texto
// vazio), e quem quer o padrão nesse caso testa [`has`] antes.

/// Maior matriz que uma função pode montar.
pub const MAX_CELULAS: usize = 10_000_000;
/// Maior texto de uma célula.
pub const MAX_TEXTO: usize = 32_767;

/// `#REF!`.
pub fn e_ref() -> CellError {
    erro(ErrorCode::Ref)
}

/// `#CALC!` (resultado vazio).
pub fn e_calc() -> CellError {
    erro(ErrorCode::Error)
}

fn arg_no(args: &[Node], i: usize) -> Res<&Node> {
    args.get(i).ok_or_else(e_valor)
}

/// Valor escalar do argumento `i` (de um intervalo, o primeiro).
pub fn arg_valor(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Value> {
    Ok(ev.eval(arg_no(args, i)?))
}

/// Número do argumento `i`: vazio é 0, VERDADEIRO é 1, texto numérico é lido.
pub fn arg_num(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<f64> {
    ev.number(arg_no(args, i)?)
}

/// Número opcional: `padrao` quando o argumento não veio (em branco vale 0).
pub fn arg_num_ou(args: &[Node], ev: &Evaluator<'_>, i: usize, padrao: f64) -> Res<f64> {
    if i >= args.len() { Ok(padrao) } else { arg_num(args, ev, i) }
}

/// Número → inteiro truncado; acima de ±2^53 é `#NÚM!`.
pub fn para_int(n: f64) -> Res<i64> {
    if !n.is_finite() || n.abs() > 9_007_199_254_740_992.0 {
        return Err(e_num());
    }
    Ok(n.trunc() as i64)
}

/// Inteiro do argumento `i` (trunca em direção ao zero).
pub fn arg_int(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<i64> {
    para_int(arg_num(args, ev, i)?)
}

/// Inteiro opcional, como [`arg_num_ou`].
pub fn arg_int_ou(args: &[Node], ev: &Evaluator<'_>, i: usize, padrao: i64) -> Res<i64> {
    if i >= args.len() { Ok(padrao) } else { arg_int(args, ev, i) }
}

/// Texto do argumento `i`.
pub fn arg_texto(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<String> {
    ev.text(arg_no(args, i)?)
}

/// Lógico do argumento `i`.
pub fn arg_logico(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<bool> {
    ev.boolean(arg_no(args, i)?)
}

/// Lógico opcional, como [`arg_num_ou`].
pub fn arg_logico_ou(args: &[Node], ev: &Evaluator<'_>, i: usize, padrao: bool) -> Res<bool> {
    if i >= args.len() { Ok(padrao) } else { arg_logico(args, ev, i) }
}

/// O argumento `i` como matriz (intervalo, expressão de matriz ou valor
/// solto como 1×1). Erro no argumento propaga.
pub fn arg_matriz(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Matrix> {
    ev.matrix_arg(arg_no(args, i)?)
}

/// Os números de **um** argumento (intervalo, matriz, referência ou valor).
/// Dentro de intervalo, matriz ou referência só número conta (texto, lógico e
/// vazio são ignorados); um valor escrito direto é convertido (`"3"` vira 3,
/// VERDADEIRO vira 1, outro texto é `#VALOR!`). Erro numa célula propaga.
pub fn arg_numeros(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Vec<f64>> {
    let no = arg_no(args, i)?;
    let referencia = matches!(no, Node::Ref { .. } | Node::Range { .. });
    let mut saida = Vec::new();
    let mut falha: Option<CellError> = None;
    let erro_da_celula = ev.for_each_value(no, &mut |v, de_colecao| {
        if falha.is_some() {
            return;
        }
        match v {
            Value::Number(n) => saida.push(*n),
            Value::Empty => {}
            _ if de_colecao || referencia => {}
            outro => match to_number(outro) {
                Ok(n) => saida.push(n),
                Err(e) => falha = Some(e),
            },
        }
    });
    match erro_da_celula.or(falha) {
        Some(e) => Err(e),
        None => Ok(saida),
    }
}

/// Monta uma matriz de linhas; sem nenhuma célula, ou grande demais, é erro.
pub fn montar_matriz(linhas: Vec<Vec<Value>>) -> Res<Matrix> {
    let largura = linhas.first().map_or(0, Vec::len);
    if linhas.is_empty() || largura == 0 {
        return Err(e_calc());
    }
    if linhas.len().checked_mul(largura).is_none_or(|n| n > MAX_CELULAS) {
        return Err(e_num());
    }
    Ok(Matrix::new(linhas))
}

/// Monta uma matriz `linhas × colunas` com os dados em ordem de leitura.
pub fn matriz_plana(linhas: usize, colunas: usize, dados: Vec<Value>) -> Res<Matrix> {
    if linhas == 0 || colunas == 0 || dados.len() != linhas * colunas {
        return Err(e_calc());
    }
    montar_matriz(dados.chunks(colunas).map(<[Value]>::to_vec).collect())
}

/// O que uma conta pode devolver: valor ou matriz.
pub trait Saida {
    fn em_computado(self) -> Computed;
}

impl Saida for Value {
    fn em_computado(self) -> Computed {
        match self {
            // Infinito e NaN nunca chegam à célula; `-0` vira `0`.
            Value::Number(n) if !n.is_finite() => Computed::Scalar(Value::Error(e_num())),
            Value::Number(n) if n == 0.0 => Computed::Scalar(Value::Number(0.0)),
            // Texto acima do limite da célula.
            Value::Text(t) if t.chars().count() > MAX_TEXTO => {
                Computed::Scalar(Value::Error(e_valor()))
            }
            v => Computed::Scalar(v),
        }
    }
}

impl Saida for Matrix {
    fn em_computado(self) -> Computed {
        Computed::Matrix(self)
    }
}

/// Confere a aridade (`min..=max`), roda a conta e fecha o resultado: erro
/// vira valor de erro, número não finito vira `#NÚM!` e texto acima de
/// 32.767 caracteres vira `#VALOR!`.
pub fn executar<T: Saida>(
    args: &[Node],
    min: usize,
    max: usize,
    conta: impl FnOnce() -> Res<T>,
) -> Computed {
    if let Some(e) = arity(args, min, max) {
        return Computed::Scalar(Value::Error(e));
    }
    match conta() {
        Ok(r) => r.em_computado(),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// Apoio dos testes das funções: calcula fórmulas sobre a planilha de mentira.
#[cfg(test)]
pub mod apoio_teste {
    use super::*;
    use crate::formula::evaluator::planilha_de_mentira::Fake;
    use crate::formula::parser::parse_formula;

    /// Resultado bruto (escalar ou matriz) da fórmula sobre células `("A1", valor)`.
    pub fn calc_com(celulas: &[(&str, Value)], formula: &str) -> Computed {
        let f = Fake::com(celulas);
        let ev = Evaluator::new(&f);
        let no = parse_formula(formula.trim_start_matches('='))
            .unwrap_or_else(|e| panic!("fórmula não compila: {formula}: {e:?}"));
        ev.eval_array(&no)
    }

    /// Valor escalar da fórmula (de matriz, o primeiro item).
    pub fn calc(formula: &str) -> Value {
        calc_com(&[], formula).scalar()
    }

    /// Valor escalar da fórmula sobre células.
    pub fn calc_cel(celulas: &[(&str, Value)], formula: &str) -> Value {
        calc_com(celulas, formula).scalar()
    }

    /// Matriz da fórmula, em linhas.
    pub fn calc_matriz(formula: &str) -> Vec<Vec<Value>> {
        match calc_com(&[], formula) {
            Computed::Matrix(m) => m.rows().to_vec(),
            outro => vec![vec![outro.scalar()]],
        }
    }

    /// Número com tolerância relativa de 1e-9.
    pub fn close(v: Value, esperado: f64) {
        match v {
            Value::Number(n) => assert!(
                (n - esperado).abs() <= 1e-9 * esperado.abs().max(1.0),
                "obtido {n}, esperado {esperado}"
            ),
            outro => panic!("esperava número {esperado}, veio {outro:?}"),
        }
    }

    /// O valor é um erro com esse código.
    pub fn is_err(v: Value, esperado: ErrorCode) {
        match v {
            Value::Error(e) => assert_eq!(e.code, esperado, "erro diferente"),
            outro => panic!("esperava erro {esperado:?}, veio {outro:?}"),
        }
    }

    /// O valor é esse texto.
    pub fn is_text(v: Value, esperado: &str) {
        assert_eq!(v, Value::Text(esperado.to_string()));
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::formula::evaluator::planilha_de_mentira::Fake;
    use crate::formula::parser::parse_formula;

    fn no(f: &str) -> Node {
        parse_formula(f).expect("devia compilar")
    }

    #[test]
    fn argumento_em_branco_nao_conta() {
        let args = [Node::Num(1.0), Node::Blank];
        assert!(has(&args, 0));
        assert!(!has(&args, 1));
        assert!(!has(&args, 2));
    }

    #[test]
    fn aridade() {
        let um = [Node::Num(1.0)];
        assert!(arity(&um, 1, 1).is_none());
        assert!(arity(&um, 2, 3).is_some());
        assert!(arity(&um, 1, 3).is_none());
        assert!(arity(&[], 1, 1).is_some());
    }

    #[test]
    fn coleta_numeros_de_varios_argumentos() {
        let f = Fake::com(&[
            ("A1", Value::Number(1.0)),
            ("A2", Value::texto("nada")),
            ("A3", Value::Number(3.0)),
        ]);
        let ev = Evaluator::new(&f);
        let args = [no("A1:A3"), no("10")];
        assert_eq!(collect(&args, &ev), Ok(vec![1.0, 3.0, 10.0]));
    }

    #[test]
    fn coleta_devolve_o_erro_do_intervalo() {
        let f = Fake::com(&[("A1", Value::erro(ErrorCode::Div0))]);
        let ev = Evaluator::new(&f);
        assert!(collect(&[no("A1:A2")], &ev).is_err());
    }

    #[test]
    fn criterio_de_comparacao() {
        let p = make_predicate(&Value::texto(">10"));
        assert!(p(&Value::Number(11.0)));
        assert!(!p(&Value::Number(10.0)));

        let p = make_predicate(&Value::texto(">= 10"));
        assert!(p(&Value::Number(10.0)));

        let p = make_predicate(&Value::texto("<>x"));
        assert!(p(&Value::texto("y")));
        assert!(!p(&Value::texto("x")));
    }

    #[test]
    fn criterio_com_virgula_decimal() {
        let p = make_predicate(&Value::texto(">0,5"));
        assert!(p(&Value::Number(0.6)));
        assert!(!p(&Value::Number(0.4)));
    }

    #[test]
    fn criterio_de_igualdade_simples() {
        let p = make_predicate(&Value::Number(7.0));
        assert!(p(&Value::Number(7.0)));
        assert!(!p(&Value::Number(8.0)));

        // Sem operador e sem curinga, texto compara sem ligar para maiúscula.
        let p = make_predicate(&Value::texto("Sim"));
        assert!(p(&Value::texto("sim")));
        assert!(!p(&Value::texto("não")));
    }

    #[test]
    fn curinga() {
        let p = make_predicate(&Value::texto("a*"));
        assert!(p(&Value::texto("abacate")));
        assert!(p(&Value::texto("A")));
        assert!(!p(&Value::texto("banana")));

        let p = make_predicate(&Value::texto("b?nana"));
        assert!(p(&Value::texto("banana")));
        assert!(p(&Value::texto("bInana")));
        assert!(!p(&Value::texto("bnana")));

        let p = make_predicate(&Value::texto("*ana*"));
        assert!(p(&Value::texto("banana")));
        assert!(p(&Value::texto("ana")));
        assert!(!p(&Value::texto("abacate")));
    }

    #[test]
    fn curinga_escapado_com_til() {
        // `~*` é uma estrela literal, não "qualquer coisa".
        let p = make_predicate(&Value::texto("a~*b"));
        assert!(p(&Value::texto("a*b")));
        assert!(!p(&Value::texto("axb")));

        let p = make_predicate(&Value::texto("a~?b"));
        assert!(p(&Value::texto("a?b")));
        assert!(!p(&Value::texto("axb")));

        // O til sozinho não escapa nada e vale por ele mesmo.
        let p = make_predicate(&Value::texto("a~b*"));
        assert!(p(&Value::texto("a~bc")));
        assert!(!p(&Value::texto("ab")));
    }

    #[test]
    fn curinga_ancora_nas_duas_pontas() {
        let p = make_predicate(&Value::texto("a?c"));
        assert!(p(&Value::texto("abc")));
        assert!(!p(&Value::texto("abcd")), "sobra no fim não pode passar");
        assert!(!p(&Value::texto("xabc")), "sobra no começo não pode passar");
    }

    /// O caso que quebra um casamento ingênuo de curinga: o primeiro `*` tem
    /// que voltar atrás para o resto fechar.
    #[test]
    fn curingas_nao_atravessam_quebras_de_linha_igual_ao_typescript() {
        for quebra in ['\n', '\r', '\u{2028}', '\u{2029}'] {
            let valor = Value::texto(format!("a{quebra}b"));
            assert!(!make_predicate(&Value::texto("a?b"))(&valor),
                "coringa '?' consumiu {quebra:?}");
            assert!(!make_predicate(&Value::texto("a*b"))(&valor),
                "coringa '*' consumiu {quebra:?}");
            assert!(make_predicate(&Value::texto(format!("a{quebra}b")))(&valor),
                "quebra literal deve continuar casando");
            assert!(make_predicate(&Value::texto(format!("a*{quebra}b")))(&valor),
                "o curinga deve poder consumir zero caracteres antes da quebra literal");
        }
        assert!(make_predicate(&Value::texto("a?b"))(&Value::texto("aZb")));
        assert!(make_predicate(&Value::texto("a*b"))(&Value::texto("aXYZb")));
    }

    #[test]
    fn curinga_volta_atras() {
        let p = make_predicate(&Value::texto("*b*d"));
        assert!(p(&Value::texto("abcbd")));
        assert!(!p(&Value::texto("abcbe")));
    }
}
