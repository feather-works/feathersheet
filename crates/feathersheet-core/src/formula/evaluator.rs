//! Avalia a árvore de uma fórmula.
//!
//! Tradução de `src/core/evaluator.ts`.
//!
//! O que ele **não** faz é tão importante quanto o que faz: ele não sabe onde
//! as células moram. Tudo que precisa da planilha passa pelo
//! [`EvalContext`] — é isso que permite testá-lo com uma planilha de mentira,
//! e é isso que vai permitir trocar o armazenamento na etapa 2 sem tocar aqui.
//!
//! ## Por que o contexto é `&self` e não `&mut self`
//!
//! No TypeScript, `getValue` "dispara recálculo sob demanda": ler uma célula
//! pode mudar a planilha. Traduzir isso como `&mut self` contaminaria o
//! avaliador inteiro de `&mut` e brigaria com a recursão — uma fórmula lê uma
//! célula que avalia outra fórmula que lê outra célula. Então o contexto
//! recebe `&self` e quem implementa guarda o que precisa mudar atrás de um
//! `RefCell`. A mutação é detalhe de quem armazena, não do avaliador.

use std::collections::HashMap;
use std::rc::Rc;

use crate::formula::ast::{BinaryOp, Node, UnaryOp};
use crate::funcoes::functions;
use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::{
    compare, to_boolean, to_number, to_text, CellError, Computed, ErrorCode, Lambda, Matrix, Value,
};

/// O trabalho que uma função pede para ser feito fora da hora do cálculo.
pub type AsyncTask = Box<dyn FnOnce() -> Computed + Send>;

/// O que o avaliador precisa saber da planilha.
pub trait EvalContext {
    /// Valor já calculado da célula (dispara recálculo sob demanda).
    fn get_value(&self, row: u32, col: u32, sheet: Option<&str>) -> Value;

    /// Percorre **apenas as células não vazias** do retângulo. É o que torna
    /// possível somar uma coluna inteira declarada (`A:A`) sem varrer dez
    /// milhões de linhas.
    fn for_each_in_range(&self, rect: &Rect, visit: &mut dyn FnMut(&Value, u32, u32));

    /// Quantas células o retângulo cobre de fato (o `COUNT` e o `LINS` usam).
    fn range_size(&self, rect: &Rect) -> u64;

    /// A última linha com conteúdo da aba (a área derramada conta), para quem
    /// varre uma coluna inteira célula por célula parar nela. `None`: não
    /// sabe, e quem pergunta varre o intervalo todo.
    fn ultima_linha_usada(&self, _sheet: Option<&str>) -> Option<u32> {
        None
    }

    /// O retângulo lido de uma vez (e guardado até a aba mudar), para quem
    /// percorre um intervalo grande célula por célula. `None`: ler com
    /// [`Self::get_value`], como sempre.
    fn faixa_lida(&self, _rect: &Rect) -> Option<std::sync::Arc<crate::planilha::faixa_lida::FaixaLida>> {
        None
    }

    /// O agregado por critério das faixas (soma, contagem, máximo e mínimo
    /// por tupla de valores de critério), montado uma vez por conjunto de
    /// faixas. `None`: varrer célula por célula. Ver
    /// [`crate::planilha::agregado`].
    fn agregado(&self, _soma: Option<&Rect>, _criterios: &[Rect])
        -> Option<std::sync::Arc<crate::planilha::agregado::Agregado>> {
        None
    }

    /// Intervalo nomeado, ou `None` se o nome não existir.
    fn resolve_name(&self, name: &str) -> Option<Rect>;

    /// Se o nome é de uma função criada por macro. Quem executa é a
    /// interface, e o valor chega como o de uma pergunta assíncrona.
    fn funcao_externa(&self, _nome: &str) -> bool {
        false
    }

    /// Se a linha está oculta (altura 0: filtro ou "ocultar"). `SUBTOTAL` e
    /// `AGREGAR` usam para pular essas linhas.
    fn linha_oculta(&self, _row: u32, _sheet: Option<&str>) -> bool {
        false
    }

    /// Se a linha faz parte de um grupo da estrutura de tópicos: oculta por
    /// recolher o grupo, ela conta como "oculta à mão" (o `SUBTOTAL` 1–11 a
    /// soma; o 101–111 não), não como filtrada.
    fn linha_no_topico(&self, _row: u32, _sheet: Option<&str>) -> bool {
        false
    }

    /// Se a célula guarda uma fórmula com `SUBTOTAL`/`AGREGAR` (que o
    /// subtotal de fora não deve contar duas vezes).
    fn celula_com_subtotal(&self, _row: u32, _col: u32, _sheet: Option<&str>) -> bool {
        false
    }

    /// Valor que não chega na hora (ver `core/async.ts`, ainda a portar).
    ///
    /// O padrão recusa: um contexto de teste ou uma avaliação avulsa não têm
    /// para onde mandar a pergunta, e é melhor dizer isso do que fingir que a
    /// resposta está a caminho.
    fn request_async(&self, _task_key: &str, _cell: (u32, u32), _start: AsyncTask) -> Computed {
        Computed::Scalar(Value::Error(CellError::com_motivo(
            ErrorCode::Value,
            "este valor só pode ser pedido de dentro de uma célula",
        )))
    }

    /// A cor de fundo (ou da fonte, com `fonte`) gravada na célula, como
    /// `#RRGGBB`; `None` = sem cor. É o que `SOMAR.POR.COR` compara.
    fn cor_celula(&self, _row: u32, _col: u32, _sheet: Option<&str>, _fonte: bool) -> Option<String> {
        None
    }

    /// O texto da fórmula guardada na célula (com o `=`), ou `None` se a
    /// célula não tem fórmula. É o que `FÓRMULATEXTO` e `ÉFÓRMULA` leem.
    fn formula_de(&self, _row: u32, _col: u32, _sheet: Option<&str>) -> Option<String> {
        None
    }

    /// Os nomes das abas da pasta, na ordem das guias, e a aba desta fórmula
    /// (`PLAN`, `PLANS`, `INFORMAÇÃO`). Vazio: o contexto não conhece a pasta.
    fn abas(&self) -> (Vec<String>, Option<String>) {
        (Vec::new(), None)
    }
}

/// Acima disto um intervalo não vira matriz: seria memória demais à toa.
const MATRIX_LIMIT: u64 = 50_000;

/// A planilha inteira, como retângulo.
/// O Excel não tem zero negativo. Desde o Rust 1.81 a soma de uma lista
/// vazia de `f64` dá `-0.0` (`=SOMA(A1:A3)` vazio virava "-0" na tela).
fn sem_zero_negativo(c: Computed) -> Computed {
    match c {
        Computed::Scalar(Value::Number(n)) if n == 0.0 => Computed::Scalar(Value::Number(0.0)),
        outro => outro,
    }
}

pub fn sheet_rect() -> Rect {
    Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1)
}

/// A chave escondida no ambiente que diz que o parâmetro da LAMBDA não veio
/// (`ÉOMITIDO`). Começa com um caractere que nenhum nome digitado tem.
pub fn marca_omitido(param: &str) -> String {
    format!("\u{1}OMITIDO:{}", param.to_uppercase())
}

pub struct Evaluator<'a> {
    pub ctx: &'a dyn EvalContext,
    /// Nomes locais: os que `LET` cria e os parâmetros de uma função da
    /// planilha. Sempre em maiúscula.
    pub env: HashMap<String, Computed>,
    /// Célula que está sendo calculada agora. É o que `LIN()` e `COL()` sem
    /// argumento devolvem; a planilha marca antes de avaliar e desmarca
    /// depois.
    pub here: Option<(u32, u32)>,
}

impl<'a> Evaluator<'a> {
    pub fn new(ctx: &'a dyn EvalContext) -> Evaluator<'a> {
        Evaluator { ctx, env: HashMap::new(), here: None }
    }

    pub fn na_celula(ctx: &'a dyn EvalContext, row: u32, col: u32) -> Evaluator<'a> {
        Evaluator { ctx, env: HashMap::new(), here: Some((row, col)) }
    }

    /// Um avaliador igual a este, com nomes a mais.
    pub fn with_scope<I, K>(&self, bindings: I) -> Evaluator<'a>
    where
        I: IntoIterator<Item = (K, Computed)>,
        K: AsRef<str>,
    {
        let mut env = self.env.clone();
        for (k, v) in bindings {
            env.insert(k.as_ref().to_uppercase(), v);
        }
        Evaluator { ctx: self.ctx, env, here: self.here }
    }

    /// Chama uma função da planilha. Os argumentos podem vir como árvore
    /// (avaliada aqui) ou já prontos.
    pub fn invoke_lambda(&self, lambda: &Lambda, args: &[ArgOrValue<'_>]) -> Computed {
        let mut env = lambda.closure.clone();
        for (i, p) in lambda.params.iter().enumerate() {
            let val = match args.get(i) {
                Some(ArgOrValue::Node(Node::Blank)) | None => {
                    env.insert(marca_omitido(p), Computed::Scalar(Value::Bool(true)));
                    env.insert(p.to_uppercase(), Computed::Scalar(Value::Empty));
                    continue;
                }
                Some(ArgOrValue::Node(n)) => self.eval_array(n),
                Some(ArgOrValue::Value(v)) => v.clone(),
            };
            env.remove(&marca_omitido(p));
            env.insert(p.to_uppercase(), val);
        }
        let filho = Evaluator { ctx: self.ctx, env, here: self.here };
        filho.eval_array(&lambda.body)
    }

    /// Avaliação escalar: uma matriz que chegue aqui vira o seu primeiro valor.
    pub fn eval(&self, node: &Node) -> Value {
        self.eval_array(node).scalar()
    }

    /// Pede um valor que responde depois. Devolve `#CALCULANDO!` enquanto a
    /// resposta não chega, e o valor de verdade nas avaliações seguintes — a
    /// planilha recalcula a célula sozinha quando a tarefa termina.
    ///
    /// A `task_key` precisa descrever a pergunta inteira (função e
    /// argumentos): duas células com a mesma pergunta dividem a mesma tarefa.
    pub fn async_value(&self, task_key: &str, start: AsyncTask) -> Computed {
        match self.here {
            Some(cell) => self.ctx.request_async(task_key, cell, start),
            None => Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "este valor só pode ser pedido de dentro de uma célula",
            ))),
        }
    }

    /// Uma função criada por macro (`makeCustomFunction`): os argumentos são
    /// avaliados aqui — o intervalo de uma linha ou de uma coluna vira lista,
    /// o resto vira tabela — e a pergunta vai à interface com eles na chave
    /// (`macro:NOME` + NUL + os argumentos em JSON).
    fn chamar_externa(&self, nome: &str, args: &[Node]) -> Computed {
        let mut valores: Vec<String> = Vec::with_capacity(args.len());
        for arg in args {
            let Node::Range { r0, c0, r1, c1, sheet } = arg else {
                let v = self.value(arg);
                if let Value::Error(e) = v {
                    return Computed::Scalar(Value::Error(e));
                }
                valores.push(json_de_valor(&v));
                continue;
            };
            let linhas: Vec<Vec<String>> = (*r0..=*r1)
                .map(|r| (*c0..=*c1).map(|c| json_de_valor(&self.ctx.get_value(r, c, sheet.as_deref()))).collect())
                .collect();
            let lista = |itens: Vec<String>| format!("[{}]", itens.join(","));
            valores.push(if linhas.len() == 1 {
                lista(linhas.into_iter().next().unwrap_or_default())
            } else if linhas.iter().all(|l| l.len() == 1) {
                lista(linhas.into_iter().flatten().collect())
            } else {
                lista(linhas.into_iter().map(lista).collect())
            });
        }
        let chave = format!("macro:{nome}\u{0}[{}]", valores.join(","));
        self.async_value(&chave, Box::new(|| Computed::Scalar(Value::Empty)))
    }

    /// Avaliação crua, que pode devolver uma matriz para derramar.
    pub fn eval_array(&self, node: &Node) -> Computed {
        match node {
            Node::Num(n) => Computed::Scalar(Value::Number(*n)),
            Node::Str(s) => Computed::Scalar(Value::Text(s.clone())),
            Node::Bool(b) => Computed::Scalar(Value::Bool(*b)),
            // Código que o motor não conhece vira `#ERROR!`. O TypeScript
            // guardava o texto como veio, mas ele não tinha um conjunto
            // fechado de códigos — aqui tem, e um código inventado não
            // sobreviveria a ida e volta do arquivo de qualquer forma.
            Node::Error(code) => Computed::Scalar(Value::Error(CellError::new(
                ErrorCode::from_code(code).unwrap_or(ErrorCode::Error),
            ))),
            Node::Blank => Computed::Scalar(Value::Empty),
            Node::Ref { row, col, sheet } => {
                Computed::Scalar(self.ctx.get_value(*row, *col, sheet.as_deref()))
            }
            Node::Range { r0, c0, r1, c1, sheet } => self.matrix_of(&Rect {
                r0: *r0,
                c0: *c0,
                r1: *r1,
                c1: *c1,
                sheet: sheet.clone(),
            }),
            Node::Name(name) => {
                let maiusculo = name.to_uppercase();
                if let Some(v) = self.env.get(&maiusculo) {
                    return v.clone();
                }
                match self.ctx.resolve_name(name) {
                    Some(rect) => self.matrix_of(&rect),
                    None => Computed::Scalar(Value::Error(CellError::com_motivo(
                        ErrorCode::Name,
                        format!("\"{name}\" não existe"),
                    ))),
                }
            }
            Node::Array { rows } => Computed::Matrix(Matrix::new(
                rows.iter()
                    .map(|linha| linha.iter().map(|n| self.eval(n)).collect())
                    .collect(),
            )),
            Node::Unary { op, operand } => self.eval_unary(*op, operand),
            Node::Binary { op, left, right } => self.eval_binary(*op, left, right),
            Node::Call { name, args } => {
                let maiusculo = name.to_uppercase();
                if let Some(Computed::Lambda(l)) = self.env.get(&maiusculo) {
                    let l = Rc::clone(l);
                    let nos: Vec<ArgOrValue<'_>> = args.iter().map(ArgOrValue::Node).collect();
                    return self.invoke_lambda(&l, &nos);
                }
                match maiusculo.as_str() {
                    "LET" | "DEIXAR" => self.eval_let(args),
                    "LAMBDA" => self.eval_lambda_def(args),
                    "APLICAR.LAMBDA" => match args.first().map(|n| self.eval_array(n)) {
                        Some(Computed::Lambda(l)) => {
                            let nos: Vec<ArgOrValue<'_>> = args[1..].iter().map(ArgOrValue::Node).collect();
                            self.invoke_lambda(&l, &nos)
                        }
                        _ => Computed::Scalar(Value::erro(ErrorCode::Value)),
                    },
                    "MAP" | "MAPEAR" => self.eval_map(args),
                    "REDUCE" | "REDUZIR" => self.eval_reduce(args),
                    _ => match functions::lookup(&maiusculo) {
                        Some(f) => sem_zero_negativo(f(&self.resolver_referencias(args), self)),
                        None if self.ctx.funcao_externa(&maiusculo) => self.chamar_externa(&maiusculo, args),
                        None => Computed::Scalar(Value::Error(CellError::com_motivo(
                            ErrorCode::Name,
                            format!("Função {name} não existe"),
                        ))),
                    },
                }
            }
        }
    }

    fn eval_let(&self, args: &[Node]) -> Computed {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "LET exige número ímpar de argumentos (pelo menos 3)",
            )));
        }
        let expr_idx = args.len() - 1;
        let mut cur_ev = Evaluator {
            ctx: self.ctx,
            env: self.env.clone(),
            here: self.here,
        };
        for i in (0..expr_idx).step_by(2) {
            let var_name = match &args[i] {
                Node::Name(n) => n.clone(),
                Node::Str(s) => s.clone(),
                _ => {
                    return Computed::Scalar(Value::Error(CellError::com_motivo(
                        ErrorCode::Value,
                        "Nome de variável inválido no LET",
                    )))
                }
            };
            let val = cur_ev.eval_array(&args[i + 1]);
            cur_ev = cur_ev.with_scope([(var_name, val)]);
        }
        cur_ev.eval_array(&args[expr_idx])
    }

    fn eval_lambda_def(&self, args: &[Node]) -> Computed {
        if args.is_empty() {
            return Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "LAMBDA exige pelo menos o corpo da função",
            )));
        }
        let body_idx = args.len() - 1;
        let mut params = Vec::with_capacity(body_idx);
        for arg in &args[..body_idx] {
            let name = match arg {
                Node::Name(n) => n.clone(),
                Node::Str(s) => s.clone(),
                _ => {
                    return Computed::Scalar(Value::Error(CellError::com_motivo(
                        ErrorCode::Value,
                        "Nome de parâmetro inválido na LAMBDA",
                    )))
                }
            };
            params.push(name.to_uppercase());
        }
        let lambda = Lambda {
            params,
            body: args[body_idx].clone(),
            closure: self.env.clone(),
        };
        Computed::Lambda(Rc::new(lambda))
    }

    fn eval_map(&self, args: &[Node]) -> Computed {
        if args.len() != 2 {
            return Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "MAP exige 2 argumentos: intervalo e lambda",
            )));
        }
        let arr = self.eval_array(&args[0]);
        let lambda_comp = self.eval_array(&args[1]);
        let lambda = match lambda_comp {
            Computed::Lambda(l) => l,
            _ => {
                return Computed::Scalar(Value::Error(CellError::com_motivo(
                    ErrorCode::Value,
                    "O segundo argumento de MAP deve ser uma LAMBDA",
                )))
            }
        };

        match arr {
            Computed::Scalar(v) => {
                self.invoke_lambda(&lambda, &[ArgOrValue::Value(Computed::Scalar(v))])
            }
            Computed::Matrix(m) => {
                let h = m.height();
                let w = m.width();
                let mut rows = Vec::with_capacity(h);
                for r in 0..h {
                    let mut line = Vec::with_capacity(w);
                    for c in 0..w {
                        let elem = m.at(r, c).clone();
                        let res = self.invoke_lambda(&lambda, &[ArgOrValue::Value(Computed::Scalar(elem))]);
                        line.push(res.scalar());
                    }
                    rows.push(line);
                }
                Computed::Matrix(crate::celulas::values::Matrix::new(rows))
            }
            Computed::Lambda(_) => Computed::Scalar(Value::erro(ErrorCode::Value)),
        }
    }

    fn eval_reduce(&self, args: &[Node]) -> Computed {
        if args.len() != 3 {
            return Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "REDUCE exige 3 argumentos: valor inicial, intervalo e lambda",
            )));
        }
        let mut acc = self.eval_array(&args[0]);
        let arr = self.eval_array(&args[1]);
        let lambda_comp = self.eval_array(&args[2]);
        let lambda = match lambda_comp {
            Computed::Lambda(l) => l,
            _ => {
                return Computed::Scalar(Value::Error(CellError::com_motivo(
                    ErrorCode::Value,
                    "O terceiro argumento de REDUCE deve ser uma LAMBDA",
                )))
            }
        };

        let items: Vec<Value> = match arr {
            Computed::Scalar(v) => vec![v],
            Computed::Matrix(m) => m.values().cloned().collect(),
            Computed::Lambda(_) => return Computed::Scalar(Value::erro(ErrorCode::Value)),
        };

        for item in items {
            let res = self.invoke_lambda(
                &lambda,
                &[
                    ArgOrValue::Value(acc),
                    ArgOrValue::Value(Computed::Scalar(item)),
                ],
            );
            acc = res;
        }

        acc
    }




    /// Troca os argumentos que são `DESLOC`/`INDIRETO`/`AJUSTARINTERVALO`
    /// pela referência que eles devolvem: assim `SOMA(DESLOC(A1;0;0;9))`
    /// soma o intervalo e `LIN(INDIRETO("B7"))` vê a referência, como no
    /// Excel. Sem nenhum deles, nada é copiado.
    pub fn resolver_referencias<'n>(&self, args: &'n [Node]) -> std::borrow::Cow<'n, [Node]> {
        use crate::funcoes::fn_referencia::{devolve_referencia, referencia};
        let e_ref = |a: &Node| matches!(a, Node::Call { name, .. } if devolve_referencia(name));
        if !args.iter().any(e_ref) {
            return std::borrow::Cow::Borrowed(args);
        }
        std::borrow::Cow::Owned(
            args.iter()
                .map(|a| match a {
                    Node::Call { name, args: dentro } if devolve_referencia(name) => referencia(name, dentro, self)
                        .unwrap_or_else(|e| Node::Error(e.code.as_str().to_string())),
                    outro => outro.clone(),
                })
                .collect(),
        )
    }

    /// Um intervalo vira matriz — é isso que faz `B2:B9>10` produzir uma
    /// coluna de verdadeiro/falso, como no Excel. Intervalo gigante continua
    /// colapsando no primeiro valor, para não estourar a memória.
    pub fn matrix_of(&self, rect: &Rect) -> Computed {
        let altura = (rect.r1 - rect.r0 + 1) as u64;
        let largura = (rect.c1 - rect.c0 + 1) as u64;
        if altura * largura > MATRIX_LIMIT || (altura == 1 && largura == 1) {
            return Computed::Scalar(self.ctx.get_value(rect.r0, rect.c0, rect.sheet.as_deref()));
        }
        let mut rows = Vec::with_capacity(altura as usize);
        for r in rect.r0..=rect.r1 {
            let mut linha = Vec::with_capacity(largura as usize);
            for c in rect.c0..=rect.c1 {
                linha.push(self.ctx.get_value(r, c, rect.sheet.as_deref()));
            }
            rows.push(linha);
        }
        Computed::Matrix(Matrix::new(rows))
    }

    fn eval_unary(&self, op: UnaryOp, operand: &Node) -> Computed {
        let valor = self.eval_array(operand);
        let aplicar = |v: &Value| -> Value {
            if let Value::Error(e) = v {
                return Value::Error(e.clone());
            }
            match to_number(v) {
                Err(e) => Value::Error(e),
                Ok(n) => Value::Number(match op {
                    UnaryOp::Neg => -n,
                    UnaryOp::Percent => n / 100.0,
                    UnaryOp::Plus => n,
                }),
            }
        };
        match valor {
            Computed::Matrix(m) => Computed::Matrix(Matrix::new(
                m.rows()
                    .iter()
                    .map(|linha| linha.iter().map(&aplicar).collect())
                    .collect(),
            )),
            outro => Computed::Scalar(aplicar(&outro.scalar())),
        }
    }

    fn eval_binary(&self, op: BinaryOp, left: &Node, right: &Node) -> Computed {
        let a = self.eval_array(left);
        let b = self.eval_array(right);
        // Com matriz de um lado (ou dos dois), a conta é feita item a item.
        if a.is_matrix() || b.is_matrix() {
            return broadcast_all(&[a, b], |vs| apply_binary(op, &vs[0], &vs[1]));
        }
        Computed::Scalar(apply_binary(op, &a.scalar(), &b.scalar()))
    }

    // ---- o que as funções usam para ler os seus argumentos ----

    /// Valor escalar de um argumento.
    pub fn value(&self, node: &Node) -> Value {
        self.eval(node)
    }

    /// Número escalar de um argumento, propagando erro.
    pub fn number(&self, node: &Node) -> Result<f64, CellError> {
        to_number(&self.eval(node))
    }

    pub fn boolean(&self, node: &Node) -> Result<bool, CellError> {
        to_boolean(&self.eval(node))
    }

    pub fn text(&self, node: &Node) -> Result<String, CellError> {
        match self.eval(node) {
            Value::Error(e) => Err(e),
            v => Ok(to_text(&v)),
        }
    }

    /// Retângulo de um argumento que precisa ser intervalo (ou a própria
    /// célula).
    pub fn rect(&self, node: &Node) -> Result<Rect, CellError> {
        match node {
            Node::Range { r0, c0, r1, c1, sheet } => Ok(Rect {
                r0: *r0,
                c0: *c0,
                r1: *r1,
                c1: *c1,
                sheet: sheet.clone(),
            }),
            Node::Ref { row, col, sheet } => Ok(Rect {
                r0: *row,
                c0: *col,
                r1: *row,
                c1: *col,
                sheet: sheet.clone(),
            }),
            Node::Name(name) => self.ctx.resolve_name(name).ok_or_else(|| {
                CellError::com_motivo(ErrorCode::Name, format!("\"{name}\" não existe"))
            }),
            _ => Err(CellError::com_motivo(ErrorCode::Value, "Esperava um intervalo")),
        }
    }

    /// Argumento como matriz, seja ele um intervalo, uma expressão de matriz
    /// (`B2:B9>10`) ou um valor solto — que vira uma matriz 1×1.
    pub fn matrix_arg(&self, node: &Node) -> Result<Matrix, CellError> {
        match self.eval_array(node) {
            Computed::Matrix(m) => Ok(m),
            Computed::Scalar(Value::Error(e)) => Err(e),
            outro => Ok(Matrix::new(vec![vec![outro.scalar()]])),
        }
    }

    /// Célula de um retângulo, respeitando a aba de origem dele.
    pub fn cell(&self, rect: &Rect, row: u32, col: u32) -> Value {
        self.ctx.get_value(row, col, rect.sheet.as_deref())
    }

    /// Percorre os valores de um argumento: intervalos e matrizes são
    /// expandidos, qualquer outra coisa entra como um valor único.
    ///
    /// O segundo parâmetro do `visit` diz se o valor veio de uma **coleção** —
    /// quem coleta números usa isso para ignorar texto e lógicos lá dentro,
    /// como o Excel faz. É o que faz `MAIOR(SE(...))` ignorar os `FALSO`.
    pub fn for_each_value(
        &self,
        node: &Node,
        visit: &mut dyn FnMut(&Value, bool),
    ) -> Option<CellError> {
        let rect = match node {
            Node::Range { r0, c0, r1, c1, sheet } => Some(Rect {
                r0: *r0,
                c0: *c0,
                r1: *r1,
                c1: *c1,
                sheet: sheet.clone(),
            }),
            // Variável de LET/LAMBDA não é intervalo nomeado: vai pelo valor.
            Node::Name(name) if self.env.contains_key(&name.to_uppercase()) => None,
            Node::Name(name) => match self.ctx.resolve_name(name) {
                Some(r) => Some(r),
                None => {
                    return Some(CellError::com_motivo(
                        ErrorCode::Name,
                        format!("\"{name}\" não existe"),
                    ))
                }
            },
            _ => None,
        };

        if let Some(rect) = rect {
            let mut erro: Option<CellError> = None;
            self.ctx.for_each_in_range(&rect, &mut |v, _r, _c| {
                if erro.is_some() {
                    return;
                }
                if let Value::Error(e) = v {
                    erro = Some(e.clone());
                }
                visit(v, true);
            });
            return erro;
        }

        match self.eval_array(node) {
            Computed::Scalar(Value::Error(e)) => Some(e),
            Computed::Matrix(m) => {
                for v in m.values() {
                    if v.is_empty() {
                        continue;
                    }
                    if let Value::Error(e) = v {
                        return Some(e.clone());
                    }
                    visit(v, true);
                }
                None
            }
            outro => {
                visit(&outro.scalar(), false);
                None
            }
        }
    }

    /// Igual ao [`Evaluator::for_each_value`], mas só com os números.
    ///
    /// Dentro de intervalo ou matriz, texto e lógico **não contam** — fora
    /// dela, contam. É a regra do Excel: `SOMA(A1:A9)` ignora o texto que
    /// estiver no meio, mas `SOMA("2")` vale 2.
    pub fn for_each_number(
        &self,
        node: &Node,
        visit: &mut dyn FnMut(f64),
    ) -> Option<CellError> {
        self.for_each_value(node, &mut |v, de_colecao| match v {
            Value::Number(n) => visit(*n),
            _ if de_colecao => {}
            Value::Bool(b) => visit(if *b { 1.0 } else { 0.0 }),
            Value::Text(_) => {
                if let Ok(n) = to_number(v) {
                    visit(n);
                }
            }
            _ => {}
        })
    }
}

/// Um argumento de função da planilha: ainda como árvore, ou já avaliado.
pub enum ArgOrValue<'n> {
    Node(&'n Node),
    Value(Computed),
}

/// Combina vários resultados item a item, repetindo os escalares.
///
/// É o que faz `SE(A1:A9>10; B1:B9; 0)` andar linha a linha, e o que uma
/// coluna contra uma linha usa para se expandir nas duas direções.
/// O valor como o `JSON.stringify` do JavaScript o escreveria (vazio e erro
/// viram `null`, número não finito também). Só para a chave da função de
/// macro — o `sheet-core` não depende de biblioteca de JSON.
fn json_de_valor(v: &Value) -> String {
    match v {
        Value::Number(n) if n.is_finite() => crate::celulas::number::to_js_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::Text(t) => {
            let mut s = String::with_capacity(t.len() + 2);
            s.push('"');
            for c in t.chars() {
                match c {
                    '"' => s.push_str("\\\""),
                    '\\' => s.push_str("\\\\"),
                    '\n' => s.push_str("\\n"),
                    '\r' => s.push_str("\\r"),
                    '\t' => s.push_str("\\t"),
                    c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
                    c => s.push(c),
                }
            }
            s.push('"');
            s
        }
        _ => "null".to_string(),
    }
}

pub fn broadcast_all(fontes: &[Computed], f: impl Fn(&[Value]) -> Value) -> Computed {
    let mut altura = 1usize;
    let mut largura = 1usize;
    for fonte in fontes {
        let (h, w) = fonte.shape();
        altura = altura.max(h);
        largura = largura.max(w);
    }
    if altura == 1 && largura == 1 {
        let vs: Vec<Value> = fontes.iter().map(|s| s.clone().scalar()).collect();
        return Computed::Scalar(f(&vs));
    }
    let mut rows = Vec::with_capacity(altura);
    for r in 0..altura {
        let mut linha = Vec::with_capacity(largura);
        for c in 0..largura {
            let vs: Vec<Value> = fontes.iter().map(|s| s.pick(r, c)).collect();
            linha.push(f(&vs));
        }
        rows.push(linha);
    }
    Computed::Matrix(Matrix::new(rows))
}

/// Um operador binário aplicado a dois valores escalares.
pub fn apply_binary(op: BinaryOp, a: &Value, b: &Value) -> Value {
    if let Value::Error(e) = a {
        return Value::Error(e.clone());
    }
    if let Value::Error(e) = b {
        return Value::Error(e.clone());
    }

    if op == BinaryOp::Concat {
        return Value::Text(format!("{}{}", to_text(a), to_text(b)));
    }

    if matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    ) {
        let ordem = match compare(a, b) {
            Ok(o) => o,
            Err(e) => return Value::Error(e),
        };
        let resposta = match op {
            BinaryOp::Eq => ordem.is_eq(),
            BinaryOp::Ne => !ordem.is_eq(),
            BinaryOp::Lt => ordem.is_lt(),
            BinaryOp::Le => ordem.is_le(),
            BinaryOp::Gt => ordem.is_gt(),
            _ => ordem.is_ge(),
        };
        return Value::Bool(resposta);
    }

    let x = match to_number(a) {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let y = match to_number(b) {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    match op {
        BinaryOp::Add => Value::Number(x + y),
        BinaryOp::Sub => Value::Number(x - y),
        BinaryOp::Mul => Value::Number(x * y),
        BinaryOp::Div => {
            if y == 0.0 {
                Value::Error(CellError::new(ErrorCode::Div0))
            } else {
                Value::Number(x / y)
            }
        }
        BinaryOp::Pow => {
            let r = x.powf(y);
            if r.is_finite() {
                Value::Number(r)
            } else {
                Value::Error(CellError::new(ErrorCode::Num))
            }
        }
        _ => unreachable!("os outros operadores já foram tratados acima"),
    }
}

#[cfg(test)]
pub(crate) mod planilha_de_mentira {
    use super::*;
    use std::collections::HashMap as Mapa;

    /// Uma planilha de brinquedo para os testes: células soltas num mapa e
    /// intervalos nomeados. É a menor coisa que satisfaz o [`EvalContext`].
    #[derive(Default)]
    pub struct Fake {
        pub celulas: Mapa<(u32, u32), Value>,
        pub nomes: Mapa<String, Rect>,
    }

    impl Fake {
        pub fn com(celulas: &[(&str, Value)]) -> Fake {
            let mut f = Fake::default();
            for (ref_, v) in celulas {
                let (r, c) = crate::celulas::refs::parse_cell_ref(ref_).expect("referência do teste");
                f.celulas.insert((r, c), v.clone());
            }
            f
        }
    }

    impl EvalContext for Fake {
        fn get_value(&self, row: u32, col: u32, _sheet: Option<&str>) -> Value {
            self.celulas.get(&(row, col)).cloned().unwrap_or(Value::Empty)
        }

        fn for_each_in_range(&self, rect: &Rect, visit: &mut dyn FnMut(&Value, u32, u32)) {
            // Em ordem de leitura, para o teste ser previsível — e só o que
            // existe, como a planilha de verdade faz.
            let mut chaves: Vec<&(u32, u32)> = self
                .celulas
                .keys()
                .filter(|(r, c)| rect.contains(*r, *c))
                .collect();
            chaves.sort();
            for (r, c) in chaves {
                visit(&self.celulas[&(*r, *c)], *r, *c);
            }
        }

        fn range_size(&self, rect: &Rect) -> u64 {
            rect.area()
        }

        fn resolve_name(&self, name: &str) -> Option<Rect> {
            self.nomes.get(&name.to_uppercase()).cloned()
        }
    }
}

#[cfg(test)]
mod testes;

