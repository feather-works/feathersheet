//! O que uma expressão do VBA vira no JavaScript: um valor (com o tipo que dá para saber
//! em tempo de conversão) ou um objeto do Excel (aba, intervalo, a fonte de um intervalo…).

/// O tipo de um valor, até onde a conversão sabe. Serve para escolher entre o operador do
/// JavaScript (`+`, `===`) e a função `vba.*` que imita o VBA com tipo desconhecido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum T {
    Num,
    Texto,
    Bool,
    /// Matriz (`Split`, `Array`, `Dim v(10)`).
    Matriz,
    /// Objeto que não é do Excel (o que `Intersect` devolve, `Nothing`).
    Obj,
    /// Variant: o valor de uma célula, uma variável sem tipo.
    Var,
}

/// Um valor JavaScript pronto: o código e o tipo.
#[derive(Clone, Debug)]
pub struct V {
    pub js: String,
    pub t: T,
    /// O código é atômico (nome, chamada, literal): não precisa de parênteses em volta.
    pub atomico: bool,
    /// A precedência do operador de fora, na escala do JavaScript (`**` 14, `*` 13, `+` 12,
    /// `<` 10, `===` 9, `&` 8, `^` 7, `|` 6, `&&` 5, `||` 4, `?:` 3); 0 = não se sabe.
    pub prec: u8,
}

impl V {
    pub fn novo(js: impl Into<String>, t: T) -> Self {
        Self { js: js.into(), t, atomico: true, prec: 100 }
    }

    /// Uma expressão com operador (vai entre parênteses se entrar em outra).
    pub fn composto(js: impl Into<String>, t: T) -> Self {
        Self { js: js.into(), t, atomico: false, prec: 0 }
    }

    /// Uma expressão com operador de precedência conhecida.
    pub fn op(js: impl Into<String>, t: T, prec: u8) -> Self {
        Self { js: js.into(), t, atomico: false, prec }
    }

    /// O código como operando de um operador de precedência `p`: parênteses só se precisa
    /// (do lado direito, a mesma precedência também pede — `a - (b - c)`).
    pub fn em(&self, p: u8, direita: bool) -> String {
        if self.atomico || self.prec > p || (self.prec == p && !direita && p != 14) {
            self.js.clone()
        } else {
            format!("({})", self.js)
        }
    }

    /// O código, entre parênteses se não for atômico.
    pub fn par(&self) -> String {
        if self.atomico {
            self.js.clone()
        } else {
            format!("({})", self.js)
        }
    }
}

/// Um intervalo: a aba (código JS que dá a API dela) e o endereço (código JS que dá o
/// texto `"B3"`/`"A1:C5"`). `literal` guarda o endereço quando ele é fixo (`"B3"`).
#[derive(Clone, Debug)]
pub struct Faixa {
    pub aba: String,
    pub end: String,
    pub literal: Option<String>,
    /// É linha inteira (`Rows(3)`, `.EntireRow`) / coluna inteira.
    pub inteira: Inteira,
    /// `Cells(Rows.Count, c)`: a última célula da coluna `c` (código JS da coluna). O
    /// `.End(xlUp)` dela é o `getLastRow` da API — sem andar célula por célula.
    pub fundo_da_coluna: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inteira {
    Nao,
    Linhas,
    Colunas,
}

impl Faixa {
    /// Um endereço fixo (`"B3"`).
    pub fn fixa(aba: &str, end: &str) -> Self {
        Self { aba: aba.into(), end: format!("{end:?}"), literal: Some(end.into()), inteira: Inteira::Nao, fundo_da_coluna: None }
    }

    /// Um endereço calculado em JS.
    pub fn dinamica(aba: &str, end: String) -> Self {
        Self { aba: aba.into(), end, literal: None, inteira: Inteira::Nao, fundo_da_coluna: None }
    }

    /// O intervalo é uma célula só, com certeza (`Cells(l, c)`, `"B3"`)?
    pub fn uma_celula(&self) -> bool {
        self.literal.as_deref().is_some_and(|l| !l.contains(':'))
    }
}

/// Um objeto do Excel, resolvido.
#[derive(Clone, Debug)]
pub enum Obj {
    /// A API de uma aba (`sheet`, `Sheet8`, `wb.sheet("X")`) e o nome, quando se sabe.
    Aba(String, Option<String>),
    Faixa(Faixa),
    /// Um pedaço do intervalo: `Font`, `Interior`, `Borders`, `Borders(xlEdgeBottom)`.
    Parte(Faixa, Parte),
    /// A pasta de trabalho (`ThisWorkbook`, `ActiveWorkbook`).
    Pasta,
    /// A coleção de abas (`Worksheets`, `Sheets`).
    Abas,
    /// `Application`.
    App,
    /// `WorksheetFunction`.
    FuncoesDaPlanilha,
    /// `Rows`/`Columns` de uma aba, para `Rows.Count` e `Rows(5)`.
    Contagem(String, Inteira),
    /// `.Rows`/`.Columns` de um intervalo, para `.Rows.Count`.
    Dimensao(Faixa, Inteira),
    /// `ActiveWorkbook.Names` (os intervalos nomeados).
    Nomes,
    /// `Worksheet.Sort` e `.Sort.SortFields` da aba (código JS da aba): as chaves ficam
    /// guardadas em tempo de execução (`vba.ordem`) até o `.Apply`.
    Ordem(String),
    /// `Range.Validation`: `.Delete` e `.Add` viram a validação de dados da API.
    Validacao(Faixa),
    /// `ActiveWindow`/`Windows(…)`: rolar, zoom e ativar janela não mudam a planilha.
    Janela,
    /// `Workbooks`: só o `.Add` (a pasta nova) converte; abrir outra pasta não.
    Pastas,
    /// A pasta nova de `Workbooks.Add` (código JS dela): só a lista das abas
    /// copiadas, para o `SaveAs` gravar num arquivo novo.
    NovaPasta(String),
    /// `.Sheets`/`.Worksheets` da pasta nova.
    AbasNovas(String),
    /// Uma aba da pasta nova: (pasta, posição do 1 em JS).
    AbaNova(String, String),
    /// Um formulário (UserForm), pelo nome: `UserForm1.Show`, `Me.Hide`.
    Formulario(String),
    /// Um controle do formulário aberto (`TextBox1`, `Me.TextBox1`): `sheet.field("TextBox1")`.
    Controle(String),
}

/// A parte de um intervalo.
#[derive(Clone, Debug, PartialEq)]
pub enum Parte {
    Fonte,
    Interior,
    /// As bordas; `Some(lado)` é uma borda só (`xlEdgeBottom` → `"bottom"`).
    Bordas(Option<String>),
}

/// Por que um trecho não foi convertido (vai para o comentário e para a lista).
#[derive(Clone, Debug, PartialEq)]
pub struct Motivo(pub String);

impl Motivo {
    pub fn novo(m: impl Into<String>) -> Self {
        Self(m.into())
    }
}

/// Resultado de uma conversão de trecho.
pub type R<X> = Result<X, Motivo>;

/// O que uma variável guarda (decidido pelo `Dim … As …` e pelo `Set`).
#[derive(Clone, Debug, PartialEq)]
pub enum Tipo {
    Valor(T),
    /// `As Worksheet`: guarda a API da aba.
    Aba,
    /// `As Range`: guarda o endereço (texto). A aba fica anotada na conversão.
    Faixa(Option<String>),
    /// `As Workbook`: a pasta nova de `Workbooks.Add` (`vba.novaPasta()`).
    NovaPasta,
    /// Objeto que não tem como converter (`As Word.Application`, `As Collection`).
    Impossivel(String),
}
