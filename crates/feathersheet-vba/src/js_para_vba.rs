//! O caminho de volta do conversor: macro em JavaScript (a API `sheet`/`wb`/`app` do FeatherSheet) → VBA que o
//! Excel roda. Cobre o que as macros do dia a dia usam: células (`get`/`set`/`range`/`cell`), laços, `if`,
//! `switch`, funções, `alert`/`confirm`/`prompt`, texto e conta (`Math`, métodos de texto) e o `docs.fill`.
//! O que não tem equivalente não some: vira comentário `' JS não convertido` e entra nas pendências.
//!
//! Cuidados de língua (dinheiro, decimal): número no código VBA sai sempre com ponto (o VBA lê assim em qualquer
//! Windows); `setFormula` vai pelo `.Formula` em inglês (a conversão é a do `.xlsx`, passada por quem chama); o
//! `toFixed` do JS dá ponto, e o `Format` do VBA dá o separador do Windows: a tradução troca de volta para ponto.

use falcao_sintaxe::arvore::{Arvore, Chave, No, NoId, OpAtrib, OpBinario, OpLogico, OpUnario, TextoRef};
use falcao_sintaxe::Lexico;
use std::collections::HashMap;

/// O resultado: o código VBA do módulo (sem as linhas `Attribute`) e o que não converteu.
#[derive(Debug, Default, PartialEq)]
pub struct Traducao {
    pub vba: String,
    pub pendencias: Vec<String>,
}

/// O que uma expressão é, para escolher `Set`, `&` e o objeto das chamadas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tipo {
    Valor,
    Texto,
    /// Uma aba (`Worksheets("X")`, `ActiveSheet`).
    Aba,
    /// Um intervalo (`Range("A1")`).
    Faixa,
}

struct V {
    s: String,
    t: Tipo,
}

impl V {
    fn val(s: impl Into<String>) -> Self { V { s: s.into(), t: Tipo::Valor } }
    fn txt(s: impl Into<String>) -> Self { V { s: s.into(), t: Tipo::Texto } }
}

type R<T> = Result<T, String>;

const RESERVADAS: &[&str] = &["and", "as", "boolean", "byref", "byval", "call", "case", "close", "const", "date", "dim", "do",
    "double", "each", "else", "elseif", "empty", "end", "error", "exit", "false", "for", "function", "get", "goto", "if", "in",
    "input", "integer", "is", "len", "let", "like", "line", "long", "loop", "me", "mid", "mod", "name", "new", "next", "not",
    "nothing", "null", "object", "on", "open", "option", "optional", "or", "print", "private", "property", "public", "range",
    "redim", "rem", "resume", "select", "set", "single", "static", "step", "stop", "string", "sub", "then", "to", "true", "type",
    "until", "variant", "wend", "while", "with", "xor", "cells", "val", "left", "right", "trim", "split", "join", "int", "abs"];

struct Tradutor<'a> {
    arv: &'a Arvore,
    lex: &'a Lexico<'a>,
    fonte: &'a str,
    formula: &'a dyn Fn(&str) -> String,
    saida: String,
    pend: Vec<String>,
    recuo: usize,
    /// O procedimento atual: (nome VBA, é Function).
    proc: Option<(String, bool)>,
    /// Laços abertos: `true` = For (sai com Exit For), `false` = Do.
    lacos: Vec<bool>,
    tipos: HashMap<String, Tipo>,
    /// Algum número virou texto: o módulo ganha a função `JsTexto`.
    usa_js_texto: bool,
    /// Traduzindo um evento: (procedimento VBA, cabeçalho com a assinatura do Excel).
    evento: Option<(&'a str, &'a str)>,
}

/// Valor → texto como o JavaScript faz (`"Total: " + 1234.5` dá `Total: 1234.5`): número com ponto em qualquer
/// língua do Windows (o `&` e o `CStr` do VBA usariam a vírgula do pt-BR), lógico em minúsculas.
const JS_TEXTO: &str = "Private Function JsTexto(v)\r\n    Select Case VarType(v)\r\n        Case 11\r\n            JsTexto = IIf(v, \"true\", \"false\")\r\n\
        Case 2, 3, 4, 5, 6, 14\r\n            JsTexto = Replace(CStr(v), Mid(CStr(1.5), 2, 1), \".\")\r\n        Case Else\r\n            JsTexto = CStr(v)\r\n\
    End Select\r\nEnd Function\r\n";

/// Traduz o módulo `js`. `nome` é o do Sub que recebe o código solto (fora de funções). `formula` converte uma
/// fórmula da planilha (`=SOMA(A1;2,5)`) para o `.Formula` do Excel (`=SUM(A1,2.5)`).
pub fn traduzir(js: &str, nome: &str, formula: &dyn Fn(&str) -> String) -> Traducao {
    traduzir_com(js, nome, None, formula)
}

/// Os eventos do JS (`aoMudar`…) e o procedimento VBA de cada um, com a assinatura que o Excel exige.
pub const EVENTOS: [(&str, &str, &str); 6] = [
    ("aoMudar", "Worksheet_Change", "Private Sub Worksheet_Change(ByVal Target As Range)"),
    ("aoSelecionar", "Worksheet_SelectionChange", "Private Sub Worksheet_SelectionChange(ByVal Target As Range)"),
    ("aoAtivar", "Worksheet_Activate", "Private Sub Worksheet_Activate()"),
    ("aoAbrir", "Workbook_Open", "Private Sub Workbook_Open()"),
    ("antesDeSalvar", "Workbook_BeforeSave", "Private Sub Workbook_BeforeSave(ByVal SaveAsUI As Boolean, Cancel As Boolean)"),
    ("antesDeFechar", "Workbook_BeforeClose", "Private Sub Workbook_BeforeClose(Cancel As Boolean)"),
];

/// Um módulo de evento (`Eventos/<aba>/aoMudar`): o código vira o procedimento do evento no módulo da aba ou da
/// pasta. `valores.intervalo` é o `Target`, `valores.comoSalvar` o `SaveAsUI`, e `wb.cancelSave()`/`cancelClose()`
/// é o `Cancel = True`. O convertido do VBA (a função `Worksheet_Change(Target)` e a chamada no fim) volta à forma
/// de antes.
pub fn traduzir_evento(js: &str, evento: &str, formula: &dyn Fn(&str) -> String) -> Option<Traducao> {
    let e = EVENTOS.iter().find(|(n, _, _)| *n == evento)?;
    Some(traduzir_com(js, e.1, Some((e.1, e.2)), formula))
}

fn traduzir_com(js: &str, nome: &str, evento: Option<(&str, &str)>, formula: &dyn Fn(&str) -> String) -> Traducao {
    let (arv, lex) = match falcao_sintaxe::analisar_script(js) {
        Ok(x) => x,
        Err(e) => {
            let vba = js.lines().map(|l| format!("' {l}")).collect::<Vec<_>>().join("\r\n");
            return Traducao { vba, pendencias: vec![feathersheet_core::trf!("o JavaScript não pôde ser lido: {}", e.mensagem)] };
        }
    };
    let mut t = Tradutor { arv: &arv, lex: &lex, fonte: js, formula, saida: String::new(), pend: Vec::new(), recuo: 0,
        proc: None, lacos: Vec::new(), tipos: HashMap::new(), usa_js_texto: false, evento };
    let topo: Vec<NoId> = arv.lista(arv.corpo).to_vec();
    let (funcoes, soltos): (Vec<NoId>, Vec<NoId>) = topo.into_iter().filter(|&id| !t.e_preludio(id) && !t.e_cola_de_evento(id))
        .partition(|&id| matches!(arv.no(id), No::DeclFuncao(_)));
    let mut declaradas = Vec::new();
    for id in &funcoes {
        if let No::DeclFuncao(f) = arv.no(*id) {
            if let Some(a) = arv.funcao(*f).nome { declaradas.push(lex.atomos.texto(a).to_string()); }
            t.funcao(*f);
        }
    }
    // `Ola();` no fim só roda a função quando a macro roda; no Excel a pessoa roda o Sub direto.
    let so_chama = |id: &NoId| matches!(arv.no(*id), No::Expressao(e) if matches!(arv.no(*e),
        No::Chamada { func, args, .. } if args.len == 0 && matches!(arv.no(*func), No::Nome(a) if declaradas.iter().any(|d| d == lex.atomos.texto(*a)))));
    let ja_tem_evento = evento.is_some_and(|(n, _)| declaradas.iter().any(|d| d.eq_ignore_ascii_case(n)));
    if !soltos.is_empty() && !soltos.iter().all(so_chama) && !ja_tem_evento {
        let n = t.nome_vba(nome);
        let n = if evento.is_none() && declaradas.iter().any(|d| t.nome_vba(d).eq_ignore_ascii_case(&n)) { format!("{n}_Principal") } else { n };
        t.proc = Some((n.clone(), false));
        let cabeca = evento.map_or_else(|| format!("Sub {n}()"), |(_, c)| c.to_string());
        t.linha(&cabeca);
        t.recuo += 1;
        for id in soltos { t.instr(id); }
        t.recuo -= 1;
        t.linha("End Sub");
        t.proc = None;
    }
    if t.usa_js_texto { t.linha(""); t.saida.push_str(JS_TEXTO); }
    Traducao { vba: t.saida, pendencias: t.pend }
}

impl<'a> Tradutor<'a> {
    fn linha(&mut self, s: &str) {
        self.saida.push_str(&"    ".repeat(self.recuo));
        self.saida.push_str(s);
        self.saida.push_str("\r\n");
    }

    fn trecho(&self, id: NoId) -> String {
        let ini = self.arv.posicao(id) as usize;
        let resto = self.fonte.get(ini..).unwrap_or("");
        resto.lines().next().unwrap_or("").trim().chars().take(120).collect()
    }

    /// O que não converte: comentário no lugar e uma pendência.
    fn pendente(&mut self, id: NoId, motivo: &str) {
        let trecho = self.trecho(id);
        let linha = self.fonte[..(self.arv.posicao(id) as usize).min(self.fonte.len())].matches('\n').count() + 1;
        self.linha(&format!("' JS não convertido ({motivo}): {trecho}"));
        // O motivo é um texto fixo deste arquivo (ou montado com `.{p}`): passa pelo dicionário.
        let motivo = feathersheet_core::tr!(motivo);
        self.pend.push(feathersheet_core::trf!("linha {linha}: {motivo}: {trecho}"));
    }

    /// O valor como texto do jeito do JS (ver [`JS_TEXTO`]).
    fn como_texto(&mut self, v: V) -> String {
        if v.t == Tipo::Texto { return v.s; }
        self.usa_js_texto = true;
        format!("JsTexto({})", v.s)
    }

    fn atomo(&self, a: falcao_sintaxe::Atomo) -> String { self.lex.atomos.texto(a).to_string() }

    fn texto(&self, t: TextoRef) -> String {
        match t {
            TextoRef::Faixa(a, b) => self.fonte.get(a as usize..b as usize).unwrap_or("").to_string(),
            TextoRef::Tabela(i) => String::from_utf16_lossy(self.lex.texto(i)),
        }
    }

    /// O nome JS como nome VBA válido (sem `$`, sem palavra reservada).
    fn nome_vba(&self, n: &str) -> String {
        let limpo: String = n.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
        let limpo = if limpo.starts_with(|c: char| c.is_ascii_digit() || c == '_') { format!("x{limpo}") } else { limpo };
        if RESERVADAS.contains(&limpo.to_lowercase().as_str()) { format!("{limpo}_") } else { limpo }
    }

    /// O bloco `const vba = {…}` que o conversor de ida põe no topo: no VBA ele não existe.
    fn e_preludio(&self, id: NoId) -> bool {
        let No::Var { decls, .. } = self.arv.no(id) else { return false };
        self.arv.lista(*decls).iter().any(|&d| matches!(self.arv.no(d), No::Declarador { alvo, .. }
            if matches!(self.arv.no(*alvo), No::Nome(a) if self.lex.atomos.texto(*a) == "vba")))
    }

    /// No módulo de evento convertido do VBA: a chamada do fim (`Worksheet_Change(valores.intervalo…)`), a caixa
    /// do `Cancel` (`$cancelar`) e o apelido do codinome (`const Planilha1 = wb.sheet(…)`, que no VBA já existe).
    fn e_cola_de_evento(&self, id: NoId) -> bool {
        if self.evento.is_none() { return false; }
        let texto = self.trecho(id);
        if texto.contains("$cancelar") { return true; }
        match *self.arv.no(id) {
            No::Expressao(e) => matches!(*self.arv.no(e), No::Chamada { func, .. }
                if matches!(*self.arv.no(func), No::Nome(a) if self.evento.is_some_and(|(n, _)| self.lex.atomos.texto(a).eq_ignore_ascii_case(n)))),
            No::Var { .. } => texto.contains("= wb.sheet("),
            _ => false,
        }
    }

    fn funcao(&mut self, f: falcao_sintaxe::arvore::FuncaoId) {
        let func = self.arv.funcao(f).clone();
        let nome = func.nome.map(|a| self.atomo(a)).unwrap_or_else(|| "SemNome".into());
        let nome = self.nome_vba(&nome);
        let mut params = Vec::new();
        for &p in self.arv.lista(func.params) {
            match self.arv.no(p) {
                No::Nome(a) => params.push(self.nome_vba(&self.atomo(*a))),
                _ => { self.pend.push(feathersheet_core::trf!("função {nome}: parâmetro com valor padrão ou desestruturado")); params.push(format!("p{}", params.len())); }
            }
        }
        let e_funcao = self.arv.lista(func.corpo).iter().any(|&id| self.devolve_valor(id));
        let tipo = if e_funcao { "Function" } else { "Sub" };
        match self.evento.filter(|(n, _)| n.eq_ignore_ascii_case(&nome)) {
            Some((_, cabeca)) => self.linha(cabeca),
            None => self.linha(&format!("{tipo} {nome}({})", params.join(", "))),
        }
        let antes = std::mem::take(&mut self.tipos);
        self.proc = Some((nome, e_funcao));
        self.recuo += 1;
        let corpo = self.arv.lista(func.corpo).to_vec();
        let n = corpo.len();
        for (i, id) in corpo.into_iter().enumerate() {
            // O último `return x` não precisa do Exit.
            if i + 1 == n {
                if let No::Retorne(v) = self.arv.no(id) {
                    if let Some(v) = v.opcao() { self.retorno(id, v, false); continue; }
                    continue;
                }
            }
            self.instr(id);
        }
        self.recuo -= 1;
        self.linha(&format!("End {tipo}"));
        self.linha("");
        self.proc = None;
        self.tipos = antes;
    }

    /// Há `return valor` aqui (sem entrar em funções de dentro)?
    fn devolve_valor(&self, id: NoId) -> bool {
        match *self.arv.no(id) {
            No::Retorne(v) => v.existe(),
            No::Bloco(l) => self.arv.lista(l).iter().any(|&x| self.devolve_valor(x)),
            No::Se { sim, nao, .. } => self.devolve_valor(sim) || nao.opcao().is_some_and(|n| self.devolve_valor(n)),
            No::Para { corpo, .. } | No::ParaDe { corpo, .. } | No::ParaEm { corpo, .. } | No::Enquanto { corpo, .. } | No::FacaEnquanto { corpo, .. } => self.devolve_valor(corpo),
            No::Escolha { casos, .. } => self.arv.lista(casos).iter().any(|&c| matches!(self.arv.no(c), No::Caso { corpo, .. } if self.arv.lista(*corpo).iter().any(|&x| self.devolve_valor(x)))),
            _ => false,
        }
    }

    fn retorno(&mut self, id: NoId, v: NoId, sair: bool) {
        let Some((nome, e_funcao)) = self.proc.clone() else { return self.pendente(id, "return fora de função") };
        if e_funcao {
            match self.expr(v) {
                Ok(x) => {
                    let set = if matches!(x.t, Tipo::Aba | Tipo::Faixa) { "Set " } else { "" };
                    self.linha(&format!("{set}{nome} = {}", x.s));
                }
                Err(m) => return self.pendente(id, &m),
            }
        }
        if sair { self.linha(if e_funcao { "Exit Function" } else { "Exit Sub" }); }
    }

    fn bloco(&mut self, id: NoId) {
        self.recuo += 1;
        match *self.arv.no(id) {
            No::Bloco(l) => for x in self.arv.lista(l).to_vec() { self.instr(x); },
            _ => self.instr(id),
        }
        self.recuo -= 1;
    }

    fn instr(&mut self, id: NoId) {
        let no = *self.arv.no(id);
        match no {
            No::Vazio => {}
            No::Bloco(l) => for x in self.arv.lista(l).to_vec() { self.instr(x); },
            No::Expressao(e) => if let Err(m) = self.comando(e) { self.pendente(id, &m) },
            No::Var { decls, .. } => {
                for d in self.arv.lista(decls).to_vec() {
                    let No::Declarador { alvo, valor } = *self.arv.no(d) else { continue };
                    let No::Nome(a) = *self.arv.no(alvo) else { self.pendente(d, "desestruturação"); continue };
                    let n = self.nome_vba(&self.atomo(a));
                    self.linha(&format!("Dim {n}"));
                    if let Some(v) = valor.opcao() {
                        match self.expr(v) {
                            Ok(x) => {
                                self.tipos.insert(n.clone(), x.t);
                                let set = if matches!(x.t, Tipo::Aba | Tipo::Faixa) { "Set " } else { "" };
                                self.linha(&format!("{set}{n} = {}", x.s));
                            }
                            Err(m) => self.pendente(d, &m),
                        }
                    }
                }
            }
            // `if (!Cancel?.$ref) Cancel = { $ref: 1, v: Cancel }`: a caixa do ByRef; no VBA o parâmetro já é ByRef.
            No::Se { .. } if self.trecho(id).contains("$ref") => {}
            No::Se { .. } => self.se(id, false),
            No::Para { inicio, teste, passo, corpo } => self.para(id, inicio, teste, passo, corpo),
            No::ParaDe { esq, dir, corpo, .. } => {
                let var = match *self.arv.no(esq) {
                    No::Var { decls, .. } => self.arv.lista(decls).first().and_then(|&d| match *self.arv.no(d) {
                        No::Declarador { alvo, .. } => match *self.arv.no(alvo) { No::Nome(a) => Some(a), _ => None },
                        _ => None }),
                    No::Nome(a) => Some(a),
                    _ => None,
                };
                let Some(a) = var else { return self.pendente(id, "for…of com desestruturação") };
                let n = self.nome_vba(&self.atomo(a));
                match self.expr(dir) {
                    Ok(x) => {
                        self.linha(&format!("Dim {n}"));
                        self.linha(&format!("For Each {n} In {}", x.s));
                        self.lacos.push(true);
                        self.bloco(corpo);
                        self.lacos.pop();
                        self.linha(&format!("Next {n}"));
                    }
                    Err(m) => self.pendente(id, &m),
                }
            }
            No::Enquanto { teste, corpo } => match self.expr(teste) {
                Ok(c) => {
                    self.linha(&format!("Do While {}", c.s));
                    self.lacos.push(false);
                    self.bloco(corpo);
                    self.lacos.pop();
                    self.linha("Loop");
                }
                Err(m) => self.pendente(id, &m),
            },
            No::FacaEnquanto { corpo, teste } => match self.expr(teste) {
                Ok(c) => {
                    self.linha("Do");
                    self.lacos.push(false);
                    self.bloco(corpo);
                    self.lacos.pop();
                    self.linha(&format!("Loop While {}", c.s));
                }
                Err(m) => self.pendente(id, &m),
            },
            No::Break(None) => match self.lacos.last() {
                Some(true) => self.linha("Exit For"),
                Some(false) => self.linha("Exit Do"),
                None => {}
            },
            No::Retorne(v) => match v.opcao() {
                Some(v) => self.retorno(id, v, true),
                None => {
                    let sair = if self.proc.as_ref().is_some_and(|p| p.1) { "Exit Function" } else { "Exit Sub" };
                    self.linha(sair);
                }
            },
            No::Escolha { disc, casos } => self.escolha(id, disc, casos),
            No::Lance(e) => match self.expr(e) {
                Ok(x) => self.linha(&format!("Err.Raise vbObjectError + 513, , {}", x.s)),
                Err(m) => self.pendente(id, &m),
            },
            No::Continue(_) => self.pendente(id, "continue (o VBA não tem)"),
            No::Tente { .. } => self.pendente(id, "try/catch"),
            No::DeclFuncao(_) => self.pendente(id, "função dentro de função"),
            _ => self.pendente(id, "comando sem equivalente"),
        }
    }

    fn se(&mut self, id: NoId, senao: bool) {
        let No::Se { teste, sim, nao } = *self.arv.no(id) else { return };
        let c = match self.expr(teste) { Ok(c) => c, Err(m) => return self.pendente(id, &m) };
        self.linha(&format!("{} {} Then", if senao { "ElseIf" } else { "If" }, c.s));
        self.bloco(sim);
        if let Some(n) = nao.opcao() {
            if matches!(self.arv.no(n), No::Se { .. }) { return self.se(n, true); }
            self.linha("Else");
            self.bloco(n);
        }
        self.linha("End If");
    }

    /// `for (let i = a; i <= b; i++)` vira `For i = a To b`; o resto, `Do While`.
    fn para(&mut self, id: NoId, inicio: NoId, teste: NoId, passo: NoId, corpo: NoId) {
        if let Some(cabeca) = self.for_simples(inicio, teste, passo) {
            let var = cabeca.0.clone();
            self.linha(&format!("Dim {var}"));
            self.linha(&format!("For {var} = {} To {}{}", cabeca.1, cabeca.2, cabeca.3));
            self.lacos.push(true);
            self.bloco(corpo);
            self.lacos.pop();
            self.linha(&format!("Next {var}"));
            return;
        }
        if let Some(i) = inicio.opcao() {
            if matches!(self.arv.no(i), No::Var { .. }) { self.instr(i) } else if let Err(m) = self.comando(i) { return self.pendente(id, &m) }
        }
        let c = match teste.opcao().map(|t| self.expr(t)).transpose() { Ok(c) => c.map_or("True".into(), |c| c.s), Err(m) => return self.pendente(id, &m) };
        self.linha(&format!("Do While {c}"));
        self.lacos.push(false);
        self.bloco(corpo);
        if let Some(p) = passo.opcao() { self.recuo += 1; if let Err(m) = self.comando(p) { self.pendente(p, &m) } self.recuo -= 1; }
        self.lacos.pop();
        self.linha("Loop");
    }

    /// (variável, início, fim, " Step n").
    fn for_simples(&mut self, inicio: NoId, teste: NoId, passo: NoId) -> Option<(String, String, String, String)> {
        let (var, ini) = match *self.arv.no(inicio.opcao()?) {
            No::Var { decls, .. } if decls.len == 1 => match *self.arv.no(self.arv.lista(decls)[0]) {
                No::Declarador { alvo, valor } => match *self.arv.no(alvo) { No::Nome(a) => (a, valor.opcao()?), _ => return None },
                _ => return None,
            },
            No::Atribuicao { op: OpAtrib::Simples, alvo, valor } => match *self.arv.no(alvo) { No::Nome(a) => (a, valor), _ => return None },
            _ => return None,
        };
        let No::Binario { op, esq, dir } = *self.arv.no(teste.opcao()?) else { return None };
        if !matches!(*self.arv.no(esq), No::Nome(a) if a == var) { return None; }
        let passo_n: f64 = match *self.arv.no(passo.opcao()?) {
            No::Atualizacao { incremento, alvo, .. } if matches!(*self.arv.no(alvo), No::Nome(a) if a == var) => if incremento { 1.0 } else { -1.0 },
            No::Atribuicao { op: OpAtrib::Bin(o), alvo, valor } if matches!(*self.arv.no(alvo), No::Nome(a) if a == var) => {
                let No::Numero(n) = *self.arv.no(valor) else { return None };
                match o { OpBinario::Soma => n, OpBinario::Sub => -n, _ => return None }
            }
            _ => return None,
        };
        let fim = self.expr(dir).ok()?.s;
        let fim = match (op, passo_n > 0.0) {
            (OpBinario::MenorIgual, true) | (OpBinario::MaiorIgual, false) => fim,
            (OpBinario::Menor, true) => format!("{fim} - 1"),
            (OpBinario::Maior, false) => format!("{fim} + 1"),
            _ => return None,
        };
        let ini = self.expr(ini).ok()?.s;
        let step = if passo_n == 1.0 { String::new() } else { format!(" Step {}", numero(passo_n)) };
        Some((self.nome_vba(&self.atomo(var)), ini, fim, step))
    }

    fn escolha(&mut self, id: NoId, disc: NoId, casos: falcao_sintaxe::arvore::Lista) {
        let d = match self.expr(disc) { Ok(d) => d, Err(m) => return self.pendente(id, &m) };
        self.linha(&format!("Select Case {}", d.s));
        let casos = self.arv.lista(casos).to_vec();
        let mut juntos: Vec<String> = Vec::new();
        for c in casos {
            let No::Caso { teste, corpo } = *self.arv.no(c) else { continue };
            let corpo: Vec<NoId> = self.arv.lista(corpo).iter().copied().filter(|&x| !matches!(self.arv.no(x), No::Break(None))).collect();
            match teste.opcao() {
                Some(t) => match self.expr(t) { Ok(v) => juntos.push(v.s), Err(m) => { self.pendente(c, &m); continue; } },
                None => juntos.push("Else".into()),
            }
            // `case 1: case 2: …` sem corpo: vão juntos no mesmo Case.
            if corpo.is_empty() { continue; }
            let rotulo = if juntos.iter().any(|j| j == "Else") { "Case Else".to_string() } else { format!("Case {}", juntos.join(", ")) };
            juntos.clear();
            self.linha(&rotulo);
            self.recuo += 1;
            for x in corpo { self.instr(x); }
            self.recuo -= 1;
        }
        self.linha("End Select");
    }

    /// Uma expressão usada como comando: atribuição, `i++`, chamada.
    fn comando(&mut self, e: NoId) -> R<()> {
        match *self.arv.no(e) {
            No::Atribuicao { op, alvo, valor } => {
                let v = self.expr(valor)?;
                let (alvo_s, alvo_t) = self.alvo(alvo)?;
                let set = if matches!(v.t, Tipo::Aba | Tipo::Faixa) { "Set " } else { "" };
                let tipo_novo = v.t;
                let novo = match op {
                    OpAtrib::Simples => v.s,
                    OpAtrib::Bin(OpBinario::Soma) if v.t == Tipo::Texto || alvo_t == Tipo::Texto => {
                        let x = self.como_texto(v);
                        format!("{alvo_s} & {}", par(&x))
                    }
                    OpAtrib::Bin(OpBinario::Soma) => format!("{alvo_s} + {}", par(&v.s)),
                    OpAtrib::Bin(o) => format!("{alvo_s} {} {}", op_bin(o).ok_or("operador sem equivalente")?, par(&v.s)),
                    OpAtrib::Log(_) => return Err("&&=, ||= ou ??=".into()),
                };
                if let No::Nome(a) = *self.arv.no(alvo) { let n = self.nome_vba(&self.atomo(a)); self.tipos.insert(n, tipo_novo); }
                self.linha(&format!("{set}{alvo_s} = {novo}"));
                Ok(())
            }
            No::Atualizacao { incremento, alvo, .. } => {
                let (a, _) = self.alvo(alvo)?;
                self.linha(&format!("{a} = {a} {} 1", if incremento { "+" } else { "-" }));
                Ok(())
            }
            No::Chamada { func, args, .. } if self.evento.is_some() && matches!(*self.arv.no(func), No::Membro { obj, prop, .. }
                if matches!(*self.arv.no(obj), No::Nome(a) if self.lex.atomos.texto(a) == "wb")
                    && matches!(self.lex.atomos.texto(prop), "cancelSave" | "cancelClose")) => {
                let _ = args;
                self.linha("Cancel = True");
                Ok(())
            }
            No::Chamada { func, args, .. } => self.chamada_comando(func, args),
            No::Paren(x) => self.comando(x),
            _ => {
                let v = self.expr(e)?;
                self.linha(&format!("Call {}", v.s));
                Ok(())
            }
        }
    }

    /// O lado esquerdo de uma atribuição: variável ou `faixa.value`.
    fn alvo(&mut self, alvo: NoId) -> R<(String, Tipo)> {
        match *self.arv.no(alvo) {
            No::Nome(a) => {
                let n = self.nome_vba(&self.atomo(a));
                let t = self.tipos.get(&n).copied().unwrap_or(Tipo::Valor);
                Ok((n, t))
            }
            No::Membro { obj, prop, .. } => {
                let p = self.atomo(prop);
                let o = self.expr(obj)?;
                // `Cancel.v = true`: a caixa do ByRef do conversor; no VBA é o próprio parâmetro.
                if p == "v" && o.t == Tipo::Valor { return Ok((o.s, Tipo::Valor)); }
                if o.t != Tipo::Faixa { return Err(feathersheet_core::trf!("atribuir .{p}")); }
                match p.as_str() {
                    "value" => Ok((format!("{}.Value", o.s), Tipo::Valor)),
                    "formula" => Ok((format!("{}.Formula", o.s), Tipo::Texto)),
                    "numberFormat" => Ok((format!("{}.NumberFormat", o.s), Tipo::Texto)),
                    _ => Err(feathersheet_core::trf!("atribuir .{p} de intervalo")),
                }
            }
            _ => Err("atribuição a algo que não é variável".into()),
        }
    }

    fn args(&mut self, args: falcao_sintaxe::arvore::Lista) -> R<Vec<V>> {
        self.arv.lista(args).to_vec().into_iter().map(|a| self.expr(a)).collect()
    }

    /// Chamadas que são comandos no VBA (sem parênteses e sem valor).
    fn chamada_comando(&mut self, func: NoId, args: falcao_sintaxe::arvore::Lista) -> R<()> {
        let lista = self.arv.lista(args).to_vec();
        if let No::Nome(a) = *self.arv.no(func) {
            let n = self.atomo(a);
            match n.as_str() {
                "alert" => { let v = self.args(args)?; self.linha(&format!("MsgBox {}", v.first().map_or("\"\"".into(), |x| x.s.clone()))); return Ok(()); }
                "log" => { let v = self.args(args)?; let partes: Vec<String> = v.into_iter().map(|x| x.s).collect(); self.linha(&format!("Debug.Print {}", partes.join("; "))); return Ok(()); }
                _ => {}
            }
        }
        if let No::Membro { obj, prop, .. } = *self.arv.no(func) {
            let p = self.atomo(prop);
            // docs.fill(modelo, { "#X#": valor }, destino): o Localizar e Substituir do Word.
            if matches!(*self.arv.no(obj), No::Nome(a) if self.lex.atomos.texto(a) == "docs") && p == "fill" && lista.len() == 3 {
                return self.docs_fill(lista[0], lista[1], lista[2]);
            }
            let o = self.expr(obj)?;
            let v = self.args(args)?;
            let a = |i: usize| v.get(i).map(|x| x.s.clone()).ok_or_else(|| feathersheet_core::trf!("{p} sem argumento"));
            match (o.t, p.as_str()) {
                (Tipo::Aba, "set" | "write") => {
                    if let (Some(&end), Some(&valor)) = (lista.first(), lista.get(1)) {
                        if let No::Array(_) = self.arv.no(valor) { return self.set_matriz(&o.s, end, valor); }
                    }
                    let set = format!("{}.Value = {}", faixa(&o.s, &a(0)?), a(1)?);
                    self.linha(&set);
                    return Ok(());
                }
                (Tipo::Aba, "setFormula") => {
                    let f = self.formula_vba(lista.get(1).copied())?;
                    let set = format!("{}.{f}", faixa(&o.s, &a(0)?));
                    self.linha(&set);
                    return Ok(());
                }
                (Tipo::Aba, "setNumberFormat") => { let l = format!("{}.NumberFormat = {}", faixa(&o.s, &a(0)?), a(1)?); self.linha(&l); return Ok(()); }
                (Tipo::Aba, "setBold") => { let l = format!("{}.Font.Bold = {}", faixa(&o.s, &a(0)?), v.get(1).map_or("True".into(), |x| x.s.clone())); self.linha(&l); return Ok(()); }
                (Tipo::Aba, "setItalic") => { let l = format!("{}.Font.Italic = {}", faixa(&o.s, &a(0)?), v.get(1).map_or("True".into(), |x| x.s.clone())); self.linha(&l); return Ok(()); }
                (Tipo::Aba, "clear") => { let l = format!("{}.Clear", faixa(&o.s, &a(0)?)); self.linha(&l); return Ok(()); }
                (Tipo::Aba, "clearContents") => { let l = format!("{}.ClearContents", faixa(&o.s, &a(0)?)); self.linha(&l); return Ok(()); }
                (Tipo::Aba, "activate") => { let l = format!("{}.Activate", if o.s.is_empty() { "ActiveSheet".into() } else { o.s.clone() }); self.linha(&l); return Ok(()); }
                (Tipo::Faixa, "set") => { let l = format!("{}.Value = {}", o.s, a(0)?); self.linha(&l); return Ok(()); }
                (Tipo::Faixa, "setFormula") => { let f = self.formula_vba(lista.first().copied())?; let l = format!("{}.{f}", o.s); self.linha(&l); return Ok(()); }
                (Tipo::Faixa, "clear") => { let l = format!("{}.Clear", o.s); self.linha(&l); return Ok(()); }
                (Tipo::Faixa, "clearContents") => { let l = format!("{}.ClearContents", o.s); self.linha(&l); return Ok(()); }
                (Tipo::Faixa, "select" | "activate") => { let l = format!("{}.Select", o.s); self.linha(&l); return Ok(()); }
                _ => {}
            }
            if o.s == "Application" {
                match p.as_str() {
                    "calculate" => { self.linha("Application.Calculate"); return Ok(()); }
                    "status" => { let l = format!("Application.StatusBar = {}", a(0)?); self.linha(&l); return Ok(()); }
                    _ => {}
                }
            }
        }
        let v = self.expr_chamada(func, args)?;
        self.linha(&format!("Call {}", v.s));
        Ok(())
    }

    /// `sheet.set("A1", [[1, 2], [3, 4]])`: uma linha por célula, a partir do canto.
    fn set_matriz(&mut self, aba: &str, end: NoId, valor: NoId) -> R<()> {
        let No::Texto(t) = *self.arv.no(end) else { return Err("matriz com endereço calculado".into()) };
        let canto = self.texto(t);
        let canto = canto.split(':').next().unwrap_or("A1").to_string();
        let No::Array(linhas) = *self.arv.no(valor) else { return Err("matriz".into()) };
        let linhas = self.arv.lista(linhas).to_vec();
        let base = faixa(aba, &vba_texto(&canto));
        for (r, l) in linhas.iter().enumerate() {
            let celulas = match *self.arv.no(*l) { No::Array(c) => self.arv.lista(c).to_vec(), _ => vec![*l] };
            for (c, x) in celulas.into_iter().enumerate() {
                let v = self.expr(x)?;
                self.linha(&format!("{base}.Offset({r}, {c}).Value = {}", v.s));
            }
        }
        Ok(())
    }

    /// `Formula = "=SUM(A1,2.5)"` (texto fixo, pela conversão do `.xlsx`) ou `FormulaLocal = expr` (calculada:
    /// vale no Excel na mesma língua do FeatherSheet).
    fn formula_vba(&mut self, f: Option<NoId>) -> R<String> {
        let f = f.ok_or("setFormula sem a fórmula")?;
        if let No::Texto(t) = *self.arv.no(f) {
            let ingles = (self.formula)(&self.texto(t));
            return Ok(format!("Formula = {}", vba_texto(&ingles)));
        }
        let v = self.expr(f)?;
        self.pend.push(feathersheet_core::tr!("fórmula montada na macro foi como FormulaLocal: no Excel, ela precisa estar na língua do Excel").into());
        Ok(format!("FormulaLocal = {}", v.s))
    }

    /// `docs.fill(modelo, {...}, destino)` → o Word pelo `CreateObject`, um Localizar e Substituir por marcador.
    fn docs_fill(&mut self, modelo: NoId, trocas: NoId, destino: NoId) -> R<()> {
        let m = self.expr(modelo)?;
        let d = self.expr(destino)?;
        let No::Objeto(props) = *self.arv.no(trocas) else { return Err("docs.fill com as trocas fora de um objeto escrito".into()) };
        let mut pares = Vec::new();
        for p in self.arv.lista(props).to_vec() {
            let No::Propriedade { chave, valor, .. } = *self.arv.no(p) else { return Err("docs.fill com espalhar".into()) };
            let k = match chave {
                Chave::Texto(t) => vba_texto(&self.texto(t)),
                Chave::Atomo(a) => vba_texto(&self.atomo(a)),
                _ => return Err("docs.fill com chave calculada".into()),
            };
            let v = self.expr(valor)?;
            pares.push((k, self.como_texto(v)));
        }
        self.linha("Dim wdApp, wdDoc");
        self.linha("Set wdApp = CreateObject(\"Word.Application\")");
        self.linha(&format!("Set wdDoc = wdApp.Documents.Open({})", m.s));
        for (k, v) in pares {
            self.linha(&format!("wdDoc.Content.Find.Execute FindText:={k}, ReplaceWith:={v}, Replace:=2"));
        }
        self.linha(&format!("wdDoc.SaveAs2 {}", d.s));
        self.linha("wdDoc.Close False");
        self.linha("wdApp.Quit");
        Ok(())
    }

    fn expr(&mut self, id: NoId) -> R<V> {
        match *self.arv.no(id) {
            No::Numero(n) => Ok(V::val(numero(n))),
            No::Texto(t) => Ok(V::txt(vba_texto(&self.texto(t)))),
            No::Verdadeiro => Ok(V::val("True")),
            No::Falso => Ok(V::val("False")),
            No::Nulo => Ok(V::val("Empty")),
            No::Paren(x) => { let v = self.expr(x)?; Ok(V { s: format!("({})", v.s), t: v.t }) }
            No::Nome(a) => {
                let n = self.atomo(a);
                match n.as_str() {
                    "undefined" => Ok(V::val("Empty")),
                    "sheet" => Ok(V { s: String::new(), t: Tipo::Aba }),
                    "wb" => Ok(V::val("ThisWorkbook")),
                    "app" => Ok(V::val("Application")),
                    _ => { let n = self.nome_vba(&n); let t = self.tipos.get(&n).copied().unwrap_or(Tipo::Valor); Ok(V { s: n, t }) }
                }
            }
            No::Template { pedacos, exprs } => {
                let p = self.arv.lista(pedacos).to_vec();
                let e = self.arv.lista(exprs).to_vec();
                let mut partes = Vec::new();
                for (i, pid) in p.iter().enumerate() {
                    if let No::PedacoTemplate(k) = *self.arv.no(*pid) {
                        let t = self.lex.template(k).cozido.as_ref().map(|u| String::from_utf16_lossy(u)).unwrap_or_default();
                        if !t.is_empty() { partes.push(vba_texto(&t)); }
                    }
                    if let Some(x) = e.get(i) { let v = self.expr(*x)?; partes.push(self.como_texto(v)); }
                }
                Ok(V::txt(if partes.is_empty() { "\"\"".into() } else { partes.join(" & ") }))
            }
            No::Array(l) => {
                let v = self.args(l)?;
                Ok(V::val(format!("Array({})", v.into_iter().map(|x| x.s).collect::<Vec<_>>().join(", "))))
            }
            No::Unario { op, arg } => {
                let v = self.expr(arg)?;
                match op {
                    OpUnario::Menos => Ok(V::val(format!("-{}", par(&v.s)))),
                    OpUnario::Mais => Ok(V::val(format!("Val({})", v.s))),
                    OpUnario::Nao => Ok(V::val(format!("Not {}", par(&v.s)))),
                    _ => Err("operador unário sem equivalente (typeof, void, delete, ~)".into()),
                }
            }
            No::Binario { op, esq, dir } => {
                let (a, b) = (self.expr(esq)?, self.expr(dir)?);
                if op == OpBinario::Soma {
                    if a.t == Tipo::Texto || b.t == Tipo::Texto {
                        let (x, y) = (self.como_texto(a), self.como_texto(b));
                        return Ok(V::txt(format!("{} & {}", par(&x), par(&y))));
                    }
                    return Ok(V::val(format!("{} + {}", par(&a.s), par(&b.s))));
                }
                let o = op_bin(op).ok_or("operador sem equivalente (in, instanceof, deslocamento)")?;
                Ok(V::val(format!("{} {o} {}", par(&a.s), par(&b.s))))
            }
            No::Logico { op, esq, dir } => {
                let (a, b) = (self.expr(esq)?, self.expr(dir)?);
                let o = match op { OpLogico::E => "And", OpLogico::Ou => "Or", OpLogico::Coalesce => return Err("?? (sem equivalente)".into()) };
                Ok(V::val(format!("{} {o} {}", par(&a.s), par(&b.s))))
            }
            No::Condicional { teste, sim, nao } => {
                let (c, a, b) = (self.expr(teste)?, self.expr(sim)?, self.expr(nao)?);
                let t = if a.t == b.t { a.t } else { Tipo::Valor };
                Ok(V { s: format!("IIf({}, {}, {})", c.s, a.s, b.s), t })
            }
            No::Membro { obj, prop, .. } => self.membro(obj, &self.atomo(prop)),
            No::Indice { obj, prop, .. } => {
                let (o, i) = (self.expr(obj)?, self.expr(prop)?);
                Ok(V::val(format!("{}({})", o.s, i.s)))
            }
            No::Chamada { func, args, .. } => self.expr_chamada(func, args),
            // `{ $ref: 1, v: x }`: a caixa do ByRef do conversor vale o `x`.
            No::Objeto(l) => {
                let props = self.arv.lista(l).to_vec();
                let v = props.iter().find_map(|&p| match *self.arv.no(p) {
                    No::Propriedade { chave: Chave::Atomo(a), valor, .. } if self.lex.atomos.texto(a) == "v" => Some(valor),
                    _ => None,
                });
                let tem_ref = props.iter().any(|&p| matches!(*self.arv.no(p), No::Propriedade { chave: Chave::Atomo(a), .. } if self.lex.atomos.texto(a) == "$ref"));
                match (tem_ref, v) { (true, Some(v)) => self.expr(v), _ => Err("objeto literal".into()) }
            }
            _ => Err("expressão sem equivalente".into()),
        }
    }

    fn membro(&mut self, obj: NoId, p: &str) -> R<V> {
        if let No::Nome(a) = *self.arv.no(obj) {
            match (self.lex.atomos.texto(a), p) {
                ("Math", "PI") => return Ok(V::val("WorksheetFunction.Pi()")),
                ("sheet", "lastRow") => return Ok(V::val("(ActiveSheet.UsedRange.Row + ActiveSheet.UsedRange.Rows.Count - 1)")),
                ("sheet", "lastColumn") => return Ok(V::val("(ActiveSheet.UsedRange.Column + ActiveSheet.UsedRange.Columns.Count - 1)")),
                ("sheet", "name") => return Ok(V::txt("ActiveSheet.Name")),
                ("valores", "intervalo") if self.evento.is_some() => return Ok(V::txt("Target.Address(False, False)")),
                ("valores", "comoSalvar") if self.evento.is_some() => return Ok(V::val("SaveAsUI")),
                ("wb", "path") => return Ok(V::txt("ThisWorkbook.Path")),
                ("wb", "name") => return Ok(V::txt("ThisWorkbook.Name")),
                _ => {}
            }
        }
        let o = self.expr(obj)?;
        match (o.t, p) {
            (Tipo::Faixa, "value") => Ok(V::val(format!("{}.Value", o.s))),
            (Tipo::Faixa, "text") => Ok(V::txt(format!("{}.Text", o.s))),
            (Tipo::Faixa, "address") => Ok(V::txt(format!("{}.Address(False, False)", o.s))),
            (Tipo::Faixa, "row") => Ok(V::val(format!("{}.Row", o.s))),
            (Tipo::Faixa, "column") => Ok(V::val(format!("{}.Column", o.s))),
            (Tipo::Aba, "name") => Ok(V::txt(format!("{}.Name", if o.s.is_empty() { "ActiveSheet" } else { &o.s }))),
            (Tipo::Texto, "length") | (Tipo::Valor, "length") => Ok(V::val(format!("Len({})", o.s))),
            (Tipo::Valor, "v") => Ok(o),
            _ => Err(feathersheet_core::trf!("propriedade .{p}")),
        }
    }

    /// As ajudas `vba.*` que o conversor de ida usa imitam funções do próprio VBA: voltam para elas.
    fn ajuda_vba(&mut self, p: &str, args: falcao_sintaxe::arvore::Lista) -> R<V> {
        let v = self.args(args)?;
        let s: Vec<String> = v.iter().map(|x| x.s.clone()).collect();
        let a = |i: usize| s.get(i).cloned().ok_or_else(|| feathersheet_core::trf!("{p} sem argumento"));
        let todos = s.join(", ");
        Ok(match p {
            "celula" => V { s: format!("Cells({}, {})", a(0)?, a(1)?), t: Tipo::Faixa },
            "faixa" => V { s: format!("Range({}, {})", a(0)?, a(1)?), t: Tipo::Faixa },
            "cint" => V::val(format!("CInt({})", a(0)?)),
            "faixaInt" => V::val(format!("CLng({})", a(0)?)),
            "n" => V::val(a(0)?),
            "str" => V::txt(format!("CStr({})", a(0)?)),
            "eq" => V::val(format!("({} = {})", par(&a(0)?), par(&a(1)?))),
            "cmp" => { let (x, y) = (par(&a(0)?), par(&a(1)?)); V::val(format!("IIf({x} < {y}, -1, IIf({x} > {y}, 1, 0))")) }
            "soma" => V::val(format!("({} + {})", par(&a(0)?), par(&a(1)?))),
            "cat" => V::txt(s.iter().map(|x| par(x)).collect::<Vec<_>>().join(" & ")),
            "mod" => V::val(format!("({} Mod {})", par(&a(0)?), par(&a(1)?))),
            "mid" => V::txt(format!("Mid({todos})")),
            "len" => V::val(format!("Len({todos})")),
            "trim" => V::txt(format!("Trim({todos})")),
            "left" => V::txt(format!("Left({todos})")),
            "right" => V::txt(format!("Right({todos})")),
            "replace" => V::txt(format!("Replace({todos})")),
            "split" => V::val(format!("Split({todos})")),
            "instr" => V::val(format!("InStr({todos})")),
            // Round do VBA é bancário (2,5 → 2), como a ajuda que o conversor usava.
            "round" => V::val(format!("Round({todos})")),
            "ubound" => V::val(format!("UBound({todos})")),
            "lbound" => V::val(format!("LBound({todos})")),
            "isNumeric" => V::val(format!("IsNumeric({todos})")),
            "vazio" => V::val(format!("({} & \"\" = \"\")", par(&a(0)?))),
            "msgBox" => V::val(format!("MsgBox({todos})")),
            "linha" => V::val(format!("Range({}).Row", a(0)?)),
            _ => return Err(feathersheet_core::trf!("ajuda vba.{p} do conversor")),
        })
    }

    fn expr_chamada(&mut self, func: NoId, args: falcao_sintaxe::arvore::Lista) -> R<V> {
        match *self.arv.no(func) {
            No::Nome(a) => {
                let n = self.atomo(a);
                let v = self.args(args)?;
                let a0 = || v.first().map(|x| x.s.clone()).ok_or("chamada sem argumento".to_string());
                match n.as_str() {
                    "confirm" => Ok(V::val(format!("(MsgBox({}, vbYesNo) = vbYes)", a0()?))),
                    "prompt" => Ok(V::txt(format!("InputBox({}{})", a0()?, v.get(1).map(|d| format!(", , {}", d.s)).unwrap_or_default()))),
                    "String" => Ok(V::txt(format!("CStr({})", a0()?))),
                    // Val lê ponto em qualquer Windows, como o Number do JS.
                    "Number" | "parseFloat" => Ok(V::val(format!("Val({})", a0()?))),
                    "parseInt" => Ok(V::val(format!("Int(Val({}))", a0()?))),
                    "isNaN" => Ok(V::val(format!("(Not IsNumeric({}))", a0()?))),
                    _ => {
                        let nome = self.nome_vba(&n);
                        Ok(V::val(format!("{nome}({})", v.into_iter().map(|x| x.s).collect::<Vec<_>>().join(", "))))
                    }
                }
            }
            No::Membro { obj, prop, .. } => {
                let p = self.atomo(prop);
                if let No::Nome(a) = *self.arv.no(obj) {
                    let base = self.lex.atomos.texto(a).to_string();
                    if base == "Math" {
                        let v = self.args(args)?;
                        let s: Vec<String> = v.into_iter().map(|x| x.s).collect();
                        let a0 = s.first().cloned().unwrap_or_default();
                        return Ok(V::val(match p.as_str() {
                            "floor" => format!("Int({a0})"),
                            "ceil" => format!("-Int(-{})", par(&a0)),
                            // Math.round arredonda .5 para cima (−2,5 → −2); Int(x + 0.5) faz o mesmo. Round do VBA é bancário.
                            "round" => format!("Int({} + 0.5)", par(&a0)),
                            "trunc" => format!("Fix({a0})"),
                            "abs" => format!("Abs({a0})"),
                            "sqrt" => format!("Sqr({a0})"),
                            "pow" => format!("({} ^ {})", par(&a0), par(s.get(1).map_or("1", |x| x))),
                            "max" => format!("WorksheetFunction.Max({})", s.join(", ")),
                            "min" => format!("WorksheetFunction.Min({})", s.join(", ")),
                            "random" => "Rnd()".into(),
                            _ => return Err(format!("Math.{p}")),
                        }));
                    }
                    if base == "vba" {
                        return self.ajuda_vba(&p, args);
                    }
                    if base == "wb" && p == "sheet" {
                        let v = self.args(args)?;
                        return Ok(V { s: format!("Worksheets({})", v.first().map(|x| x.s.clone()).ok_or("wb.sheet sem nome")?), t: Tipo::Aba });
                    }
                }
                let o = self.expr(obj)?;
                let v = self.args(args)?;
                let a = |i: usize| v.get(i).map(|x| x.s.clone()).ok_or_else(|| feathersheet_core::trf!("{p} sem argumento"));
                let f = |s: &str| faixa(&o.s, s);
                Ok(match (o.t, p.as_str()) {
                    (Tipo::Aba, "get" | "value") => V::val(format!("{}.Value", f(&a(0)?))),
                    (Tipo::Aba, "getText") => V::txt(format!("{}.Text", f(&a(0)?))),
                    (Tipo::Aba, "range") => V { s: f(&a(0)?), t: Tipo::Faixa },
                    (Tipo::Aba, "cell") => {
                        let pre = if o.s.is_empty() { String::new() } else { format!("{}.", o.s) };
                        V { s: format!("{pre}Cells({}, {})", a(0)?, a(1)?), t: Tipo::Faixa }
                    }
                    (Tipo::Aba, "getLastRow") => {
                        let pre = if o.s.is_empty() { String::new() } else { format!("{}.", o.s) };
                        V::val(format!("{pre}Cells({pre}Rows.Count, {}).End(xlUp).Row", v.first().map_or("\"A\"".into(), |x| x.s.clone())))
                    }
                    (Tipo::Faixa, "getValue") => V::val(format!("{}.Value", o.s)),
                    (Tipo::Faixa, "offset") => V { s: format!("{}.Offset({}, {})", o.s, a(0)?, a(1)?), t: Tipo::Faixa },
                    (Tipo::Texto | Tipo::Valor, "toUpperCase") => V::txt(format!("UCase({})", o.s)),
                    (Tipo::Texto | Tipo::Valor, "toLowerCase") => V::txt(format!("LCase({})", o.s)),
                    (Tipo::Texto | Tipo::Valor, "trim") => V::txt(format!("Trim({})", o.s)),
                    (Tipo::Texto | Tipo::Valor, "includes") => V::val(format!("(InStr(1, {}, {}, vbBinaryCompare) > 0)", o.s, a(0)?)),
                    (Tipo::Texto | Tipo::Valor, "indexOf") => V::val(format!("(InStr(1, {}, {}, vbBinaryCompare) - 1)", o.s, a(0)?)),
                    (Tipo::Texto | Tipo::Valor, "startsWith") => { let t = a(0)?; V::val(format!("(Left({}, Len({t})) = {t})", o.s)) }
                    (Tipo::Texto | Tipo::Valor, "endsWith") => { let t = a(0)?; V::val(format!("(Right({}, Len({t})) = {t})", o.s)) }
                    (Tipo::Texto | Tipo::Valor, "substring") => match v.get(1) {
                        Some(b) => V::txt(format!("Mid({}, {} + 1, {} - {})", o.s, par(&a(0)?), par(&b.s), par(&a(0)?))),
                        None => V::txt(format!("Mid({}, {} + 1)", o.s, par(&a(0)?))),
                    },
                    (Tipo::Texto | Tipo::Valor, "substr") => V::txt(format!("Mid({}, {} + 1{})", o.s, par(&a(0)?), v.get(1).map(|n| format!(", {}", n.s)).unwrap_or_default())),
                    // replace com texto troca só a primeira ocorrência, como o JS.
                    (Tipo::Texto | Tipo::Valor, "replace") => V::txt(format!("Replace({}, {}, {}, 1, 1)", o.s, a(0)?, a(1)?)),
                    (Tipo::Texto | Tipo::Valor, "replaceAll") => V::txt(format!("Replace({}, {}, {})", o.s, a(0)?, a(1)?)),
                    (Tipo::Texto | Tipo::Valor, "split") => V::val(format!("Split({}, {})", o.s, a(0)?)),
                    (Tipo::Valor, "join") => V::txt(format!("Join({}, {})", o.s, a(0)?)),
                    // toFixed dá ponto; o Format do VBA dá o separador do Windows (vírgula em pt-BR): volta para ponto.
                    (Tipo::Valor, "toFixed") => {
                        let casas: usize = v.first().and_then(|x| x.s.parse().ok()).unwrap_or(0);
                        let mascara = if casas == 0 { "0".to_string() } else { format!("0.{}", "0".repeat(casas)) };
                        V::txt(format!("Replace(Format({}, \"{mascara}\"), Mid(CStr(1.5), 2, 1), \".\")", o.s))
                    }
                    _ => return Err(feathersheet_core::trf!("método .{p}()")),
                })
            }
            _ => Err("chamada de algo que não é nome nem método".into()),
        }
    }
}

/// `Range(a)` na aba (vazia = a ativa).
fn faixa(aba: &str, end: &str) -> String {
    // `vba.celula(l, c)`/`vba.faixa(a, b)` já são o objeto (`Cells(l, c)`): só ganha a aba.
    let objeto = end.starts_with("Cells(") || end.starts_with("Range(");
    match (aba.is_empty(), objeto) {
        (true, true) => end.to_string(),
        (false, true) => format!("{aba}.{end}"),
        (true, false) => format!("Range({end})"),
        (false, false) => format!("{aba}.Range({end})"),
    }
}

fn par(s: &str) -> String {
    // Simples = sem espaço fora de parênteses e de aspas (`Cells(i, 1).Value`, `"a b"`, `-x`).
    let (mut nivel, mut aspas) = (0i32, false);
    let composto = s.chars().any(|c| {
        match c { '"' => aspas = !aspas, '(' if !aspas => nivel += 1, ')' if !aspas => nivel -= 1, _ => {} }
        c == ' ' && !aspas && nivel == 0
    });
    if composto { format!("({s})") } else { s.to_string() }
}

fn op_bin(o: OpBinario) -> Option<&'static str> {
    Some(match o {
        OpBinario::Sub => "-", OpBinario::Mul => "*", OpBinario::Div => "/", OpBinario::Resto => "Mod", OpBinario::Potencia => "^",
        OpBinario::Igual | OpBinario::Identico => "=", OpBinario::Diferente | OpBinario::NaoIdentico => "<>",
        OpBinario::Menor => "<", OpBinario::Maior => ">", OpBinario::MenorIgual => "<=", OpBinario::MaiorIgual => ">=",
        OpBinario::E => "And", OpBinario::Ou => "Or", OpBinario::OuExclusivo => "Xor",
        _ => return None,
    })
}

/// Número no código VBA: sempre com ponto (o VBA lê o código assim em qualquer língua do Windows).
fn numero(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

/// Texto como literal do VBA: aspas dobradas; quebra de linha e tabulação viram `vbLf`/`vbTab`.
fn vba_texto(t: &str) -> String {
    let mut partes = Vec::new();
    let mut atual = String::new();
    for c in t.chars() {
        let especial = match c { '\n' => Some("vbLf"), '\r' => Some("vbCr"), '\t' => Some("vbTab"), _ => None };
        match especial {
            Some(e) => {
                if !atual.is_empty() { partes.push(format!("\"{}\"", atual.replace('"', "\"\""))); atual.clear(); }
                partes.push(e.to_string());
            }
            None => atual.push(c),
        }
    }
    if !atual.is_empty() || partes.is_empty() { partes.push(format!("\"{}\"", atual.replace('"', "\"\""))); }
    partes.join(" & ")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn vba(js: &str) -> Traducao {
        traduzir(js, "Macro1", &|f: &str| f.trim_start_matches('=').replace(';', ",").replace("SOMA", "SUM").replace("2,5", "2.5"))
    }

    #[test]
    fn celulas_lacos_e_if() {
        let t = vba(r#"
            const total = sheet.get("B1") * 2;
            for (let i = 1; i <= 10; i++) {
                if (sheet.cell(i, 1).value === "") { break; }
                sheet.set("C" + i, total);
            }
            if (total > 100) { alert("Alto: " + total); } else if (total > 50) { alert("Médio"); } else { log("baixo"); }
        "#);
        assert!(t.pendencias.is_empty(), "{:?}", t.pendencias);
        for esperado in ["Sub Macro1()", "Dim total", "total = Range(\"B1\").Value * 2", "For i = 1 To 10", "If Cells(i, 1).Value = \"\" Then",
            "Exit For", "Range(\"C\" & JsTexto(i)).Value = total", "Next i", "MsgBox \"Alto: \" & JsTexto(total)", "Private Function JsTexto(v)", "ElseIf total > 50 Then", "Debug.Print \"baixo\"", "End Sub"] {
            assert!(t.vba.contains(esperado), "faltou {esperado}:\n{}", t.vba);
        }
    }

    #[test]
    fn funcao_com_retorno_vira_function_e_while_vira_do() {
        let t = vba("function Dobro(x) { return x * 2; }\nfunction Conta() { let n = 0; while (n < 5) { n += 1; } alert(Dobro(n)); }");
        assert!(t.vba.contains("Function Dobro(x)\r\n    Dobro = x * 2\r\nEnd Function"), "{}", t.vba);
        assert!(t.vba.contains("Sub Conta()") && t.vba.contains("Do While n < 5") && t.vba.contains("n = n + 1") && t.vba.contains("MsgBox Dobro(n)"), "{}", t.vba);
    }

    /// Dinheiro e língua: número com ponto no código, fórmula em inglês, toFixed com ponto como no JS, round igual.
    #[test]
    fn moeda_decimal_formula_e_arredondamento() {
        let t = vba(r#"sheet.set("A1", 1234.5); sheet.setFormula("B1", "=SOMA(A1;2,5)"); const s = (1234.5).toFixed(2); const r = Math.round(-2.5); sheet.setNumberFormat("A1", "R$ #,##0.00");"#);
        assert!(t.vba.contains("Range(\"A1\").Value = 1234.5"), "{}", t.vba);
        assert!(t.vba.contains("Range(\"B1\").Formula = \"SUM(A1,2.5)\""), "{}", t.vba);
        assert!(t.vba.contains("Mid(CStr(1.5), 2, 1), \".\")"), "{}", t.vba);
        assert!(t.vba.contains("Int(-2.5 + 0.5)") || t.vba.contains("Int((-2.5) + 0.5)"), "{}", t.vba);
        assert!(t.vba.contains("Range(\"A1\").NumberFormat = \"R$ #,##0.00\""), "{}", t.vba);
    }

    #[test]
    fn outra_aba_matriz_switch_e_word() {
        let t = vba(r##"
            const s = wb.sheet("Dados");
            s.set("A1", [["Nome", "Valor"], ["Ana", 10]]);
            switch (s.get("B2")) { case 1: case 2: alert("pouco"); break; default: alert("muito"); }
            docs.fill("modelo.docx", { "#NOME#": s.get("A2"), "#V#": s.getText("B2") }, wb.path + "\\saida.docx");
        "##);
        assert!(t.pendencias.is_empty(), "{:?}", t.pendencias);
        for esperado in ["Set s = Worksheets(\"Dados\")", "s.Range(\"A1\").Offset(1, 1).Value = 10", "Select Case s.Range(\"B2\").Value",
            "Case 1, 2", "Case Else", "End Select", "Set wdApp = CreateObject(\"Word.Application\")",
            "wdDoc.Content.Find.Execute FindText:=\"#NOME#\", ReplaceWith:=JsTexto(s.Range(\"A2\").Value), Replace:=2",
            "ReplaceWith:=s.Range(\"B2\").Text", "wdDoc.SaveAs2 ThisWorkbook.Path & \"\\saida.docx\""] {
            assert!(t.vba.contains(esperado), "faltou {esperado}:\n{}", t.vba);
        }
    }

    #[test]
    fn o_que_nao_converte_vira_comentario_e_pendencia() {
        let t = vba("try { x(); } catch (e) {}\nconst o = new Date();\nsheet.set('A1', 1);");
        assert_eq!(t.pendencias.len(), 2, "{:?}", t.pendencias);
        assert!(t.vba.contains("' JS não convertido (try/catch)"), "{}", t.vba);
        assert!(t.vba.contains("Range(\"A1\").Value = 1"));
        let ruim = vba("function (");
        assert!(ruim.vba.starts_with("' function (") && ruim.pendencias.len() == 1);
    }

    #[test]
    fn chamada_solta_da_propria_funcao_nao_vira_sub() {
        let t = vba("function Ola() { alert('oi'); }\nOla();");
        assert_eq!(t.vba.matches("Sub Ola()").count(), 1, "{}", t.vba);
        assert!(!t.vba.contains("Call Ola"), "{}", t.vba);
        let t = vba("function Macro1() { alert('a'); }\nMacro1(); alert('depois');");
        assert!(t.vba.contains("Sub Macro1_Principal()"), "nome colidindo: {}", t.vba);
    }

    /// O evento convertido do VBA (função + chamada no fim + caixa do Cancel) volta com a assinatura do Excel.
    #[test]
    fn evento_convertido_volta_com_a_assinatura_do_excel() {
        let js = "const Planilha1 = wb.sheet(\"Dados\");\nfunction Worksheet_Change(Target) {\n  if (vba.eq(Target, \"A1\")) { vba.msgBox(\"mudou\"); }\n}\nWorksheet_Change(valores.intervalo || Planilha1.selection().address);\n";
        let t = traduzir_evento(js, "aoMudar", &|f: &str| f.to_string()).unwrap();
        assert!(t.pendencias.is_empty(), "{:?}\n{}", t.pendencias, t.vba);
        assert!(t.vba.starts_with("Private Sub Worksheet_Change(ByVal Target As Range)") && t.vba.contains("If (Target = \"A1\") Then") && t.vba.contains("MsgBox(\"mudou\")"), "{}", t.vba);
        assert!(!t.vba.contains("Planilha1") && t.vba.matches("End Sub").count() == 1, "um procedimento só: {}", t.vba);
        let salvar = "function Workbook_BeforeSave(SaveAsUI, Cancel) {\n  if (!Cancel?.$ref) Cancel = { $ref: 1, v: Cancel };\n  if (vba.eq(sheet.value(\"B2\"), \"\")) { Cancel.v = true; }\n}\nconst $cancelar = { $ref: 1, v: false };\nWorkbook_BeforeSave(valores.comoSalvar === true, $cancelar);\nif ($cancelar.v) wb.cancelSave();\n";
        let t = traduzir_evento(salvar, "antesDeSalvar", &|f: &str| f.to_string()).unwrap();
        assert!(t.pendencias.is_empty(), "{:?}\n{}", t.pendencias, t.vba);
        assert!(t.vba.contains("Private Sub Workbook_BeforeSave(ByVal SaveAsUI As Boolean, Cancel As Boolean)") && t.vba.contains("If (Range(\"B2\").Value = \"\") Then") && t.vba.contains("Cancel = True"), "{}", t.vba);
    }

    /// Evento escrito aqui (sem função): o código vira o corpo do evento; `valores.intervalo` é o Target.
    #[test]
    fn evento_escrito_aqui_vira_o_corpo() {
        let t = traduzir_evento("if (valores.intervalo === \"B2\") { sheet.set(\"C2\", sheet.get(\"B2\") * 1.1); }\n", "aoMudar", &|f: &str| f.to_string()).unwrap();
        assert!(t.vba.starts_with("Private Sub Worksheet_Change(ByVal Target As Range)") && t.vba.contains("If Target.Address(False, False) = \"B2\" Then"), "{}", t.vba);
        let t = traduzir_evento("if (!confirm(\"Fechar?\")) wb.cancelClose();", "antesDeFechar", &|f: &str| f.to_string()).unwrap();
        assert!(t.vba.contains("Private Sub Workbook_BeforeClose(Cancel As Boolean)") && t.vba.contains("Cancel = True"), "{}", t.vba);
        assert!(traduzir_evento("x", "aoVoar", &|f: &str| f.to_string()).is_none());
    }

    #[test]
    fn nomes_reservados_e_texto_com_aspas_e_quebra() {
        let t = vba("let end = 'Ele disse \"oi\"\\nfim'; alert(end);");
        assert!(t.vba.contains("Dim end_") && t.vba.contains("end_ = \"Ele disse \"\"oi\"\"\" & vbLf & \"fim\""), "{}", t.vba);
    }
}
