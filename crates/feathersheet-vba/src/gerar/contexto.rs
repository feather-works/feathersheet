//! O estado de uma conversão: o programa inteiro (todos os módulos), a macro sendo montada
//! (o que ela pede de ajuda, de aba e de procedimento) e o procedimento sendo escrito
//! (variáveis, `With`, laços, a aba ativa).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::tipos::{Faixa, Motivo, Obj, Tipo, T};
use crate::ovba::TipoDeModulo;
use crate::sintaxe::{Instr, Modulo, Proc};

/// Um módulo analisado, com o que a conversão precisa dele.
pub struct InfoModulo {
    pub nome: String,
    pub tipo: TipoDeModulo,
    pub linhas: Vec<String>,
    pub ast: Modulo,
    /// No módulo de uma aba: o nome da aba (`"MAIN"`).
    pub aba: Option<String>,
}

/// Todos os módulos, e onde achar cada procedimento.
pub struct Programa {
    pub modulos: Vec<InfoModulo>,
    /// Codename em minúsculas → (nome da variável JS, nome da aba).
    pub codenames: HashMap<String, (String, String)>,
    /// Formulário em minúsculas → (nome como escrito, nomes dos controles).
    pub formularios: HashMap<String, (String, Vec<String>)>,
}

/// Um procedimento: (módulo, índice em `procs`).
pub type RefProc = (usize, usize);

impl Programa {
    pub fn proc(&self, r: RefProc) -> &Proc {
        &self.modulos[r.0].ast.procs[r.1]
    }

    /// O procedimento `nome` visto do módulo `de`: o do próprio módulo, senão um público de
    /// módulo comum.
    pub fn achar(&self, de: usize, nome: &str) -> Option<RefProc> {
        let igual = |p: &Proc| p.nome.eq_ignore_ascii_case(nome);
        if let Some(i) = self.modulos[de].ast.procs.iter().position(igual) {
            return Some((de, i));
        }
        self.modulos.iter().enumerate().filter(|(_, m)| m.tipo == TipoDeModulo::Padrao).find_map(|(mi, m)| m.ast.procs.iter().position(|p| p.publico && igual(p)).map(|pi| (mi, pi)))
    }

    /// `Modulo.Proc` ou `Sheet8.Proc`.
    pub fn achar_qualificado(&self, modulo: &str, nome: &str) -> Option<RefProc> {
        let mi = self.modulos.iter().position(|m| m.nome.eq_ignore_ascii_case(modulo))?;
        self.modulos[mi].ast.procs.iter().position(|p| p.nome.eq_ignore_ascii_case(nome)).map(|pi| (mi, pi))
    }

    /// O formulário (UserForm) com esse nome, como escrito.
    pub fn formulario(&self, nome: &str) -> Option<&str> {
        self.formularios.get(&nome.to_lowercase()).map(|(n, _)| n.as_str())
    }

    /// O controle `nome` do formulário `form`, como escrito.
    pub fn controle(&self, form: &str, nome: &str) -> Option<&str> {
        self.formularios.get(&form.to_lowercase())?.1.iter().find(|c| c.eq_ignore_ascii_case(nome)).map(String::as_str)
    }

    /// O formulário cujo código é o módulo `mi`.
    pub fn formulario_do_modulo(&self, mi: usize) -> Option<&str> {
        self.formulario(&self.modulos[mi].nome)
    }

    /// A função da API do Windows (`Declare`) com esse nome, em algum módulo.
    pub fn e_externa(&self, nome: &str) -> bool {
        self.modulos.iter().any(|m| m.ast.externas.iter().any(|(n, _)| n.eq_ignore_ascii_case(nome)))
    }

    /// O texto das linhas de uma instrução, para o comentário.
    pub fn fonte(&self, modulo: usize, i: &Instr) -> Vec<String> {
        let l = &self.modulos[modulo].linhas;
        (i.linha..=i.ate.max(i.linha)).filter_map(|n| l.get(n - 1)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }
}

/// Um trecho que não foi convertido.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Pendencia {
    pub modulo: String,
    pub linha: usize,
    pub vba: String,
    pub motivo: String,
    /// Convertida, mas pode se comportar diferente (não é trecho que ficou para trás).
    pub aviso: bool,
}

/// A macro sendo montada: o que ela usa e o que ficou para trás.
#[derive(Default)]
pub struct Macro {
    pub ajudas: BTreeSet<&'static str>,
    /// Variável JS → nome da aba (`Sheet8` → `"MAIN"`).
    pub abas: BTreeMap<String, String>,
    /// Procedimentos pedidos e o nome JS de cada um.
    pub nomes: HashMap<RefProc, String>,
    pub fila: Vec<RefProc>,
    /// Variáveis de módulo: (módulo, nome em minúsculas) → nome JS.
    pub globais: HashMap<(usize, String), String>,
    /// Nomes já usados no topo da macro (funções e variáveis de módulo).
    pub topo: BTreeSet<String>,
    pub pendencias: Vec<Pendencia>,
    /// Instruções convertidas e não convertidas.
    pub convertidas: usize,
}

impl Macro {
    /// O nome JS do procedimento (pedindo para ele entrar na macro).
    pub fn pedir(&mut self, p: &Programa, r: RefProc) -> String {
        if let Some(n) = self.nomes.get(&r) {
            return n.clone();
        }
        let base = js_nome(&p.proc(r).nome);
        let nome = if self.topo.contains(&base) { js_nome(&format!("{}_{}", p.modulos[r.0].nome, p.proc(r).nome)) } else { base };
        self.topo.insert(nome.clone());
        self.nomes.insert(r, nome.clone());
        self.fila.push(r);
        nome
    }
}

/// Uma variável do procedimento.
#[derive(Clone, Debug)]
pub struct Var {
    pub js: String,
    pub tipo: Tipo,
    /// Declarada com `Dim v()` / `v(10)`.
    pub matriz: bool,
    /// Parâmetro passado por referência (o padrão do VBA).
    pub por_ref: bool,
}

/// O escopo de um procedimento.
#[derive(Default)]
pub struct Escopo {
    pub vars: HashMap<String, Var>,
    /// A ordem das declarações (para o `let` sair estável).
    pub ordem: Vec<String>,
    /// Variáveis que não estavam declaradas (o VBA sem `Option Explicit` cria na hora).
    pub implicitas: BTreeSet<String>,
    /// O nome (minúsculo) da função, cujo nome guarda o retorno.
    pub funcao: Option<String>,
    /// A aba de cada variável `As Range` (o endereço mora na variável; a aba, aqui).
    pub aba_da_faixa: HashMap<String, String>,
    /// Variáveis inteiras (`As Long`/`Integer`/`Byte`), pelo nome JS: atribuir a elas
    /// arredonda, como no VBA.
    pub inteiros: BTreeSet<String>,
}

/// O tipo declarado é inteiro (`Long`, `Integer`, `Byte`)?
pub fn e_inteiro(t: Option<&str>) -> bool {
    matches!(t, Some("long" | "integer" | "byte" | "longlong" | "longptr"))
}

/// Um laço aberto (para o `Exit For`/`Exit Do` achar o seu).
pub struct Laco {
    pub tipo: &'static str,
    pub id: usize,
    pub rotulado: bool,
}

/// Quem escreve um procedimento.
pub struct Gerador<'a> {
    pub p: &'a Programa,
    pub m: &'a mut Macro,
    pub modulo: usize,
    pub escopo: Escopo,
    pub linhas: Vec<String>,
    pub nivel: usize,
    /// A pilha dos `With`: o objeto, ou o motivo de não dar para converter.
    pub com: Vec<Result<Obj, Motivo>>,
    pub lacos: Vec<Laco>,
    pub contador: usize,
    /// A aba "ativa" a esta altura do código (muda com `Sheets("X").Select`).
    pub atual: String,
    /// O nome da aba ativa, quando conhecido.
    pub atual_nome: Option<String>,
    /// Rótulos com bloco aberto (`rotulo_x: { … }`): um `GoTo x` vira `break rotulo_x;`.
    pub rotulos: Vec<String>,
    /// Depois de `On Error Resume Next` (até o `On Error GoTo 0`): cada instrução vai num
    /// `try`, e o erro só fica anotado no `Err`.
    pub pular_erro: bool,
}

/// Palavras que não podem ser nome de variável no JavaScript (ou que o código gerado usa).
const RESERVADAS: &[&str] = &[
    "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else", "enum", "export", "extends", "false", "finally", "for", "function", "if", "import", "in", "instanceof",
    "new", "null", "return", "super", "switch", "this", "throw", "true", "try", "typeof", "var", "void", "while", "with", "yield", "let", "static", "implements", "interface", "package", "private", "protected", "public",
    "await", "arguments", "eval", "undefined", "NaN", "Infinity", "sheet", "wb", "app", "vba", "valores", "console", "planilha", "range", "cell", "cells", "worksheets", "sheets", "alert", "confirm", "prompt", "Math",
    "String", "Number", "Date", "Array", "Object", "JSON", "window", "document", "of",
];

/// Um nome do VBA como nome válido no JavaScript.
pub fn js_nome(n: &str) -> String {
    let limpo: String = n.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let limpo = if limpo.chars().next().is_some_and(|c| c.is_ascii_digit()) { format!("_{limpo}") } else { limpo };
    if RESERVADAS.contains(&limpo.as_str()) {
        format!("{limpo}_")
    } else {
        limpo
    }
}

/// O tipo de uma variável pelo `As …` da declaração.
pub fn tipo_declarado(t: Option<&str>) -> Tipo {
    match t.unwrap_or("variant") {
        "string" => Tipo::Valor(T::Texto),
        "long" | "integer" | "double" | "single" | "currency" | "byte" | "longlong" | "longptr" | "decimal" | "date" => Tipo::Valor(T::Num),
        "boolean" => Tipo::Valor(T::Bool),
        "worksheet" => Tipo::Aba,
        "range" => Tipo::Faixa(None),
        "variant" | "vbmsgboxresult" => Tipo::Valor(T::Var),
        "workbook" => Tipo::NovaPasta,
        "collection" | "dictionary" | "scripting.dictionary" => Tipo::Impossivel(format!("objeto {} (sem equivalente ainda)", t.unwrap_or(""))),
        x if x.starts_with("word.") => Tipo::Impossivel("automação do Word".into()),
        x if x.starts_with("outlook.") => Tipo::Impossivel("automação do Outlook".into()),
        x => Tipo::Impossivel(format!("objeto do tipo {x}")),
    }
}

/// O valor inicial de uma variável desse tipo, como o VBA (texto vazio, zero, falso).
pub fn valor_inicial(t: &Tipo) -> &'static str {
    match t {
        Tipo::Valor(T::Texto) => "''",
        Tipo::Valor(T::Num) => "0",
        Tipo::Valor(T::Bool) => "false",
        Tipo::Aba | Tipo::Faixa(_) | Tipo::NovaPasta | Tipo::Impossivel(_) | Tipo::Valor(T::Obj) => "null",
        Tipo::Valor(T::Var | T::Matriz) => "undefined",
    }
}

impl Gerador<'_> {
    /// Escreve uma linha no nível de recuo atual.
    pub fn linha(&mut self, s: impl AsRef<str>) {
        let s = s.as_ref();
        if s.is_empty() {
            return;
        }
        self.linhas.push(format!("{}{s}", "  ".repeat(self.nivel)));
    }

    /// Pede uma função `vba.*` para a macro.
    pub fn ajuda(&mut self, nome: &'static str) -> &'static str {
        debug_assert!(super::ajuda::existe(nome), "ajuda {nome} não existe");
        self.m.ajudas.insert(nome);
        nome
    }

    /// A variável JS da aba de codename `c` (anotando para declarar no topo).
    pub fn aba_por_codename(&mut self, c: &str) -> Option<(String, String)> {
        let (var, nome) = self.p.codenames.get(&c.to_lowercase())?.clone();
        self.m.abas.insert(var.clone(), nome.clone());
        Some((var, nome))
    }

    /// A aba pelo nome fixo (`Worksheets("MAIN")`): a variável do codename, se houver.
    pub fn aba_por_nome(&mut self, nome: &str) -> String {
        let achado = self.p.codenames.values().find(|(_, n)| n.eq_ignore_ascii_case(nome)).cloned();
        match achado {
            Some((var, n)) => {
                self.m.abas.insert(var.clone(), n);
                var
            }
            None => format!("wb.sheet({nome:?})"),
        }
    }

    /// Um número novo, para nomes internos (`$fim1`, laço `L2`).
    pub fn novo_id(&mut self) -> usize {
        self.contador += 1;
        self.contador
    }

    /// A variável do procedimento ou do módulo, pelo nome do VBA.
    pub fn var(&self, nome: &str) -> Option<Var> {
        self.escopo.vars.get(&nome.to_lowercase()).cloned()
    }

    /// A variável de módulo (do módulo atual) com esse nome: (nome JS, tipo).
    pub fn global(&mut self, nome: &str) -> Option<Var> {
        let chave = nome.to_lowercase();
        let m = &self.p.modulos[self.modulo].ast;
        let d = m.variaveis.iter().chain(&m.constantes).find(|d| d.nome.to_lowercase() == chave)?.clone();
        let js = match self.m.globais.get(&(self.modulo, chave.clone())) {
            Some(j) => j.clone(),
            None => {
                let base = js_nome(&d.nome);
                let j = if self.m.topo.contains(&base) { js_nome(&format!("{}_{}", self.p.modulos[self.modulo].nome, d.nome)) } else { base };
                self.m.topo.insert(j.clone());
                self.m.globais.insert((self.modulo, chave), j.clone());
                j
            }
        };
        Some(Var { js, tipo: tipo_declarado(d.tipo.as_deref()), matriz: d.dims.is_some(), por_ref: false })
    }

    /// Declara uma variável do procedimento (se ainda não existe) e devolve o nome JS.
    pub fn declarar(&mut self, nome: &str, tipo: Tipo, matriz: bool) -> String {
        let chave = nome.to_lowercase();
        if let Some(v) = self.escopo.vars.get(&chave) {
            return v.js.clone();
        }
        let js = js_nome(nome);
        self.escopo.ordem.push(chave.clone());
        self.escopo.vars.insert(chave, Var { js: js.clone(), tipo, matriz, por_ref: false });
        js
    }

    /// A faixa guardada numa variável `As Range`.
    pub fn faixa_da_var(&self, v: &Var) -> Faixa {
        let aba = self.escopo.aba_da_faixa.get(&v.js).cloned().unwrap_or_else(|| match self.obj_implicito() {
            Obj::Aba(a, _) => a,
            _ => self.atual.clone(),
        });
        Faixa::dinamica(&aba, v.js.clone())
    }
}
