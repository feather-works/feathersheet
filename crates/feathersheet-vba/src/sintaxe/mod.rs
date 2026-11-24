//! A árvore do VBA: módulo → procedimentos → instruções → expressões. O analisador é
//! tolerante: instrução que ele não entende vira [`Instr::Desconhecida`] com o texto
//! original, para o gerador marcar como não convertida — nunca derruba o módulo inteiro.

mod expr;
mod instr;
mod precompilar;

pub use instr::analisar;

/// Um operador binário, na precedência do VBA (ver `expr.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Pot,
    Mul,
    Div,
    DivInt,
    Mod,
    Soma,
    Sub,
    Concat,
    Igual,
    Dif,
    Menor,
    Maior,
    MenorIg,
    MaiorIg,
    Like,
    Is,
    E,
    Ou,
    Xor,
    Eqv,
    Imp,
}

/// Uma expressão.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Texto(String),
    Data(String),
    Bool(bool),
    /// `Nothing`.
    Nada,
    /// `Empty`.
    Vazio,
    /// `Null`.
    Nulo,
    Nome(String),
    /// `a.b`.
    Membro(Box<Expr>, String),
    /// `.b` dentro de um `With`.
    MembroDoCom(String),
    /// `a(args)`: chamada, índice de matriz ou propriedade com argumento (o gerador decide).
    Chamada(Box<Expr>, Vec<Arg>),
    /// `a!b` (acesso por nome numa coleção).
    Bang(Box<Expr>, String),
    Bin(Op, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Nao(Box<Expr>),
    /// `New Classe`.
    Novo(String),
    /// `TypeOf x Is Tipo`.
    TipoDe(Box<Expr>, String),
    /// `(expr)` — guardado porque `Foo (x)` passa `x` por valor e muda o sentido de `Call`.
    Paren(Box<Expr>),
}

/// Um argumento: posicional ou nomeado (`Replace:=wdReplaceAll`); `valor` vazio é argumento
/// omitido (`f(1, , 3)`).
#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub nome: Option<String>,
    pub valor: Option<Expr>,
}

/// Uma variável declarada (`Dim x As Long`, `Dim v(1 To 10) As String`, `Dim ws As New X`).
#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub nome: String,
    /// O tipo em minúsculas (`"long"`, `"range"`, `"word.application"`); `None` = Variant.
    pub tipo: Option<String>,
    /// `Some` quando é matriz: os limites de cada dimensão (vazio = matriz dinâmica `()`).
    pub dims: Option<Vec<(Option<Expr>, Expr)>>,
    pub novo: bool,
    /// Valor de `Const`.
    pub valor: Option<Expr>,
}

/// Um caso do `Select Case`.
#[derive(Clone, Debug, PartialEq)]
pub enum Caso {
    Valor(Expr),
    Faixa(Expr, Expr),
    /// `Case Is > 5`.
    Comparacao(Op, Expr),
}

/// Uma instrução e as linhas (base 1) que ela ocupa no módulo.
#[derive(Clone, Debug, PartialEq)]
pub struct Instr {
    pub k: TipoInstr,
    pub linha: usize,
    pub ate: usize,
}

/// O que a instrução faz.
#[derive(Clone, Debug, PartialEq)]
pub enum TipoInstr {
    Dim(Vec<Decl>),
    Const(Vec<Decl>),
    ReDim { preservar: bool, decls: Vec<Decl> },
    Atribuir { alvo: Expr, valor: Expr, set: bool },
    /// Chamada como instrução (`Foo a, b`, `Call Foo(a)`, `Range("A1").Select`).
    Chamar(Expr),
    Se { ramos: Vec<(Expr, Vec<Instr>)>, senao: Option<Vec<Instr>> },
    Escolha { expr: Expr, casos: Vec<(Vec<Caso>, Vec<Instr>)>, senao: Option<Vec<Instr>> },
    Para { var: String, de: Expr, ate: Expr, passo: Option<Expr>, corpo: Vec<Instr> },
    ParaCada { var: String, em: Expr, corpo: Vec<Instr> },
    /// `Do [While|Until c] … Loop [While|Until c]`: (até?, condição).
    Faca { antes: Option<(bool, Expr)>, depois: Option<(bool, Expr)>, corpo: Vec<Instr> },
    Com { obj: Expr, corpo: Vec<Instr> },
    /// `Exit Sub/Function/For/Do/Property`, em minúsculas.
    Sair(String),
    /// `End` sozinho (termina tudo) e `Stop`.
    Terminar,
    Rotulo(String),
    IrPara(String),
    /// `On Error …` (o texto depois de `On Error`, em minúsculas).
    EmErro(String),
    /// O que o analisador não entendeu (ou o que não tem como entrar no JS: `Open … For
    /// Input`, `Line Input #1`). O texto fica na pendência.
    Desconhecida(String),
}

/// Que tipo de procedimento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoProc {
    Sub,
    Funcao,
    /// `Property Get` (lê, como função) / `Let`/`Set` (escreve, como Sub).
    PropriedadeLer,
    PropriedadeEscrever,
}

/// Um parâmetro.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub nome: String,
    pub por_valor: bool,
    pub opcional: bool,
    pub padrao: Option<Expr>,
    pub tipo: Option<String>,
    pub matriz: bool,
}

/// Um procedimento (`Sub`, `Function`, `Property`).
#[derive(Clone, Debug, PartialEq)]
pub struct Proc {
    pub nome: String,
    pub tipo: TipoProc,
    pub publico: bool,
    pub params: Vec<Param>,
    pub retorno: Option<String>,
    pub corpo: Vec<Instr>,
    pub linha: usize,
}

/// Um módulo analisado.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Modulo {
    pub procs: Vec<Proc>,
    /// Variáveis e constantes do módulo (fora dos procedimentos).
    pub variaveis: Vec<Decl>,
    pub constantes: Vec<Decl>,
    /// Funções da API do Windows (`Declare … Lib "…"`): nome → linha.
    pub externas: Vec<(String, usize)>,
    /// Os `Enum` do módulo (os membros estão em `constantes`).
    pub enums: Vec<String>,
    /// Os `Type … End Type` (registros): nome e campos.
    pub registros: Vec<(String, Vec<Decl>)>,
    /// Trechos de módulo que não entraram (`Type`, lixo): (linha, texto).
    pub ignorados: Vec<(usize, String)>,
}
