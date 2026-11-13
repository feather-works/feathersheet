//! Power Query (linguagem M) ↔ script do FeatherQuery.
//!
//! As duas pontas são texto. O M vira as linhas do script (`fonte.metodo(…);`, colunas pelo
//! nome), e o `script::ler` de sempre confere coluna por coluna e monta as etapas. O que o M
//! tem e o FeatherQuery não faz vira `// M não convertido:` com o texto original, como no
//! conversor de VBA: nada some calado. Na volta, cada etapa vira um `#"Nome" = Table.X(…)`
//! para colar no Editor Avançado do Excel.
use super::*;
use serde_json::{json, Value as Json};

// ───────────────────────── léxico do M ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum T {
    Id(String),
    Txt(String),
    Num(f64),
    S(&'static str),
}

#[derive(Clone, Debug)]
struct Tk {
    t: T,
    ini: usize,
    fim: usize,
}

const SIMBOLOS: &[&str] = &[
    "=>", "<>", "<=", ">=", "..", "=", "<", ">", "(", ")", "{", "}", "[", "]", ",", "&", "+", "-", "*", "/", "?", "@", "!", ";",
];

fn lexico(s: &str) -> Result<Vec<Tk>, String> {
    let cs: Vec<(usize, char)> = s.char_indices().collect();
    let pos_de = |k: usize| cs.get(k).map(|c| c.0).unwrap_or(s.len());
    let avancar_ate = |i: &mut usize, alvo: usize| while *i < cs.len() && cs[*i].0 < alvo { *i += 1 };
    let (mut i, mut out) = (0, Vec::new());
    while i < cs.len() {
        let (pos, c) = cs[i];
        let resto = &s[pos..];
        if c.is_whitespace() { i += 1; continue; }
        if resto.starts_with("//") {
            while i < cs.len() && cs[i].1 != '\n' { i += 1; }
            continue;
        }
        if resto.starts_with("/*") {
            let k = resto[2..].find("*/").ok_or_else(|| tr!("comentário /* sem o */ de fechamento").to_owned())?;
            avancar_ate(&mut i, pos + 2 + k + 2);
            continue;
        }
        let quoted_id = c == '#' && resto[1..].starts_with('"');
        if c == '"' || quoted_id {
            i += if quoted_id { 2 } else { 1 };
            let mut txt = String::new();
            loop {
                let Some(&(p, ch)) = cs.get(i) else { return Err(tr!("texto sem as aspas de fechamento").into()) };
                if ch == '"' {
                    if cs.get(i + 1).map(|x| x.1) == Some('"') { txt.push('"'); i += 2; continue; }
                    i += 1;
                    break;
                }
                // #(lf), #(cr,lf), #(tab), #(#) e #(00E9) dentro do texto.
                if ch == '#' && s[p..].starts_with("#(") {
                    if let Some(k) = s[p..].find(')') {
                        let mut r = String::new();
                        let ok = s[p + 2..p + k].split(',').all(|parte| {
                            let x = match parte.trim() {
                                "lf" => Some('\n'), "cr" => Some('\r'), "tab" => Some('\t'), "#" => Some('#'),
                                h if h.len() == 4 || h.len() == 8 => u32::from_str_radix(h, 16).ok().and_then(char::from_u32),
                                _ => None,
                            };
                            x.map(|x| r.push(x)).is_some()
                        });
                        if ok { txt.push_str(&r); avancar_ate(&mut i, p + k + 1); continue; }
                    }
                }
                txt.push(ch);
                i += 1;
            }
            out.push(Tk { t: if quoted_id { T::Id(txt) } else { T::Txt(txt) }, ini: pos, fim: pos_de(i) });
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|x| x.1.is_ascii_digit())) {
            i += 1;
            while i < cs.len() {
                let ch = cs[i].1;
                let expoente = matches!(ch, '+' | '-') && matches!(cs[i - 1].1, 'e' | 'E');
                let ponto = ch == '.' && cs.get(i + 1).is_some_and(|x| x.1.is_ascii_digit());
                if !(ch.is_ascii_alphanumeric() || ponto || expoente) { break; }
                i += 1;
            }
            let txt = &s[pos..pos_de(i)];
            let n = match txt.strip_prefix("0x").or_else(|| txt.strip_prefix("0X")) {
                Some(h) => i64::from_str_radix(h, 16).ok().map(|v| v as f64),
                None => txt.parse::<f64>().ok(),
            };
            out.push(Tk { t: T::Num(n.ok_or_else(|| trf!("número inválido: {txt}"))?), ini: pos, fim: pos_de(i) });
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '#' {
            i += 1;
            while i < cs.len() {
                let ch = cs[i].1;
                let ponto = ch == '.' && cs.get(i + 1).is_some_and(|x| x.1.is_alphabetic() || x.1 == '_');
                if !(ch.is_alphanumeric() || ch == '_' || ponto) { break; }
                i += 1;
            }
            out.push(Tk { t: T::Id(s[pos..pos_de(i)].to_owned()), ini: pos, fim: pos_de(i) });
            continue;
        }
        let sim = SIMBOLOS.iter().find(|x| resto.starts_with(**x)).ok_or_else(|| trf!("caractere inesperado: {c}"))?;
        i += sim.chars().count();
        out.push(Tk { t: T::S(sim), ini: pos, fim: pos_de(i) });
    }
    Ok(out)
}

// ───────────────────────── expressões ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum E {
    Num(f64),
    Txt(String),
    Logico(bool),
    Nulo,
    Id(String),
    /// `[Coluna]` (ou `_[Coluna]`) dentro de um `each`.
    Campo(String),
    Lista(Vec<E>),
    Registro(Vec<(String, E)>),
    Chamada(Box<E>, Vec<E>),
    /// `x{…}`
    Item(Box<E>, Box<E>),
    /// `x[Campo]`
    Acesso(Box<E>, String),
    Cada(Box<E>),
    Funcao(Vec<String>, Box<E>),
    Bin(&'static str, Box<E>, Box<E>),
    Nao(Box<E>),
    Neg(Box<E>),
    Se(Box<E>, Box<E>, Box<E>),
    Tipo(String),
    /// Texto de M pronto (só na volta, script → M).
    Cru(String),
}

struct Leitor<'a> {
    tk: &'a [Tk],
    i: usize,
    src: &'a str,
}

type R<X> = Result<X, String>;

impl<'a> Leitor<'a> {
    fn ver(&self) -> Option<&T> { self.tk.get(self.i).map(|t| &t.t) }
    fn eh(&self, s: &str) -> bool { matches!(self.ver(), Some(T::S(x)) if *x == s) }
    fn eh_palavra(&self, w: &str) -> bool { matches!(self.ver(), Some(T::Id(x)) if x == w) }
    fn erro<X>(&self) -> R<X> {
        let perto = self.tk.get(self.i).map(|t| self.src[t.ini..t.fim].to_owned()).unwrap_or_else(|| tr!("fim do texto").into());
        Err(trf!("não entendi o M perto de \"{perto}\""))
    }
    fn esperar(&mut self, s: &str) -> R<()> {
        if self.eh(s) { self.i += 1; Ok(()) } else { self.erro() }
    }
    fn esperar_palavra(&mut self, w: &str) -> R<()> {
        if self.eh_palavra(w) { self.i += 1; Ok(()) } else { self.erro() }
    }

    /// Onde fecha o `(`/`[`/`{` que está em `i`.
    fn fecho(&self, i: usize) -> Option<usize> {
        let mut fundo = 0i32;
        for (k, t) in self.tk.iter().enumerate().skip(i) {
            match &t.t {
                T::S("(" | "[" | "{") => fundo += 1,
                T::S(")" | "]" | "}") => { fundo -= 1; if fundo == 0 { return Some(k); } }
                _ => {}
            }
        }
        None
    }

    /// Nome de campo entre `[` e `]`: um token só (`#"Nome"`) ou o texto cru (`[Data de Venda]`).
    fn nome_entre(&self, a: usize, b: usize) -> String {
        match &self.tk[a..b] {
            [Tk { t: T::Id(x), .. }] => x.clone(),
            [] => String::new(),
            v => self.src[v[0].ini..v[v.len() - 1].fim].trim().to_owned(),
        }
    }

    fn expr(&mut self) -> R<E> {
        if self.eh_palavra("let") {
            self.i += 1;
            let mut ligacoes = Vec::new();
            loop {
                let nome = match self.ver() { Some(T::Id(x)) => x.clone(), _ => return self.erro() };
                self.i += 1;
                self.esperar("=")?;
                ligacoes.push((nome, self.expr()?));
                if self.eh(",") { self.i += 1; continue; }
                break;
            }
            self.esperar_palavra("in")?;
            let corpo = self.expr()?;
            // let interno: só o valor final interessa (o M é avaliado preguiçoso).
            return Ok(ligacoes.into_iter().rev().fold(corpo, |acc, _| acc));
        }
        if self.eh_palavra("each") { self.i += 1; return Ok(E::Cada(Box::new(self.expr()?))); }
        if self.eh_palavra("if") {
            self.i += 1;
            let c = self.expr()?;
            self.esperar_palavra("then")?;
            let a = self.expr()?;
            self.esperar_palavra("else")?;
            let b = self.expr()?;
            return Ok(E::Se(Box::new(c), Box::new(a), Box::new(b)));
        }
        if self.eh("(") {
            if let Some(f) = self.fecho(self.i) {
                if matches!(self.tk.get(f + 1).map(|t| &t.t), Some(T::S("=>"))) {
                    let params = self.tk[self.i + 1..f].iter().filter_map(|t| match &t.t {
                        T::Id(x) if x != "as" && x != "optional" && x != "nullable" => Some(x.clone()),
                        _ => None,
                    }).collect();
                    self.i = f + 2;
                    return Ok(E::Funcao(params, Box::new(self.expr()?)));
                }
            }
        }
        self.ou()
    }

    fn ou(&mut self) -> R<E> {
        let mut e = self.e_()?;
        while self.eh_palavra("or") { self.i += 1; e = E::Bin("or", Box::new(e), Box::new(self.e_()?)); }
        Ok(e)
    }
    fn e_(&mut self) -> R<E> {
        let mut e = self.nao()?;
        while self.eh_palavra("and") { self.i += 1; e = E::Bin("and", Box::new(e), Box::new(self.nao()?)); }
        Ok(e)
    }
    fn nao(&mut self) -> R<E> {
        if self.eh_palavra("not") { self.i += 1; return Ok(E::Nao(Box::new(self.nao()?))); }
        self.relacao()
    }
    fn relacao(&mut self) -> R<E> {
        let e = self.soma()?;
        if self.eh_palavra("is") {
            self.i += 1;
            if self.eh_palavra("nullable") { self.i += 1; }
            let tipo = match self.ver() { Some(T::Id(t)) => t.clone(), _ => return self.erro() };
            self.i += 1;
            return Ok(E::Bin("is", Box::new(e), Box::new(E::Id(tipo))));
        }
        for op in ["=", "<>", "<", ">", "<=", ">="] {
            if self.eh(op) {
                self.i += 1;
                let op: &'static str = SIMBOLOS.iter().find(|x| **x == op).copied().unwrap_or("=");
                return Ok(E::Bin(op, Box::new(e), Box::new(self.soma()?)));
            }
        }
        Ok(e)
    }
    fn soma(&mut self) -> R<E> {
        let mut e = self.produto()?;
        loop {
            let op = if self.eh("+") { "+" } else if self.eh("-") { "-" } else if self.eh("&") { "&" } else { break };
            self.i += 1;
            e = E::Bin(op, Box::new(e), Box::new(self.produto()?));
        }
        Ok(e)
    }
    fn produto(&mut self) -> R<E> {
        let mut e = self.unario()?;
        loop {
            let op = if self.eh("*") { "*" } else if self.eh("/") { "/" } else { break };
            self.i += 1;
            e = E::Bin(op, Box::new(e), Box::new(self.unario()?));
        }
        Ok(e)
    }
    fn unario(&mut self) -> R<E> {
        if self.eh("-") { self.i += 1; return Ok(E::Neg(Box::new(self.unario()?))); }
        if self.eh("+") { self.i += 1; return self.unario(); }
        let mut e = self.primario()?;
        loop {
            if self.eh("(") {
                self.i += 1;
                let mut args = Vec::new();
                while !self.eh(")") {
                    args.push(self.expr()?);
                    if self.eh(",") { self.i += 1; } else { break; }
                }
                self.esperar(")")?;
                e = E::Chamada(Box::new(e), args);
            } else if self.eh("{") {
                self.i += 1;
                let dentro = self.expr()?;
                self.esperar("}")?;
                e = E::Item(Box::new(e), Box::new(dentro));
            } else if self.eh("[") {
                let f = self.fecho(self.i).ok_or_else(|| tr!("[ sem ] de fechamento").to_owned())?;
                let nome = self.nome_entre(self.i + 1, f);
                self.i = f + 1;
                e = if e == E::Id("_".into()) { E::Campo(nome) } else { E::Acesso(Box::new(e), nome) };
            } else if self.eh("?") {
                self.i += 1;
            } else {
                break;
            }
        }
        Ok(e)
    }
    fn primario(&mut self) -> R<E> {
        let Some(t) = self.ver().cloned() else { return self.erro() };
        match t {
            T::Num(n) => { self.i += 1; Ok(E::Num(n)) }
            T::Txt(s) => { self.i += 1; Ok(E::Txt(s)) }
            T::Id(x) => {
                self.i += 1;
                Ok(match x.as_str() {
                    "true" => E::Logico(true),
                    "false" => E::Logico(false),
                    "null" => E::Nulo,
                    "type" => {
                        if self.eh_palavra("nullable") { self.i += 1; }
                        let nome = match self.ver() { Some(T::Id(n)) => n.clone(), _ => return self.erro() };
                        self.i += 1;
                        // type table [A = text]: o desenho da tabela não interessa.
                        if self.eh("[") || self.eh("{") {
                            self.i = self.fecho(self.i).map(|f| f + 1).unwrap_or(self.tk.len());
                        }
                        E::Tipo(nome)
                    }
                    _ => E::Id(x),
                })
            }
            T::S("(") => {
                self.i += 1;
                let e = self.expr()?;
                self.esperar(")")?;
                Ok(e)
            }
            T::S("{") => {
                self.i += 1;
                let mut itens = Vec::new();
                while !self.eh("}") {
                    itens.push(self.expr()?);
                    if self.eh("..") { return self.erro(); }
                    if self.eh(",") { self.i += 1; } else { break; }
                }
                self.esperar("}")?;
                Ok(E::Lista(itens))
            }
            T::S("[") => {
                let f = self.fecho(self.i).ok_or_else(|| tr!("[ sem ] de fechamento").to_owned())?;
                let mut fundo = 0;
                let registro = self.tk[self.i + 1..f].iter().any(|t| match t.t {
                    T::S("(" | "[" | "{") => { fundo += 1; false }
                    T::S(")" | "]" | "}") => { fundo -= 1; false }
                    T::S("=") => fundo == 0,
                    _ => false,
                });
                if !registro && f > self.i + 1 {
                    let nome = self.nome_entre(self.i + 1, f);
                    self.i = f + 1;
                    return Ok(E::Campo(nome));
                }
                self.i += 1;
                let mut campos = Vec::new();
                while self.i < f {
                    let ini = self.i;
                    while self.i < f && !self.eh("=") { self.i += 1; }
                    let nome = self.nome_entre(ini, self.i);
                    self.esperar("=")?;
                    campos.push((nome, self.expr()?));
                    if self.eh(",") { self.i += 1; }
                }
                self.esperar("]")?;
                Ok(E::Registro(campos))
            }
            _ => self.erro(),
        }
    }
}

/// Uma expressão inteira (sobrar token é erro).
fn expressao(tk: &[Tk], src: &str) -> R<E> {
    let mut l = Leitor { tk, i: 0, src };
    let e = l.expr()?;
    if l.i < tk.len() { return l.erro(); }
    Ok(e)
}

struct Ligacao {
    nome: String,
    expr: R<E>,
    texto: String,
}

/// As etapas do `let … in …` de cima. Cada uma é lida sozinha: uma que não entendo
/// não estraga as outras.
fn ligacoes(src: &str) -> R<Vec<Ligacao>> {
    let tk = lexico(src)?;
    let texto = |a: usize, b: usize| if a < b { src[tk[a].ini..tk[b - 1].fim].to_owned() } else { String::new() };
    if !matches!(tk.first().map(|t| &t.t), Some(T::Id(x)) if x == "let") {
        // Consulta de uma linha só (`Excel.CurrentWorkbook(){[Name="T"]}[Content]`).
        let fim = tk.iter().rposition(|t| t.t != T::S(";")).map(|k| k + 1).unwrap_or(0);
        return Ok(vec![Ligacao { nome: "Fonte".into(), expr: expressao(&tk[..fim], src), texto: texto(0, fim) }]);
    }
    let mut saida = Vec::new();
    let mut i = 1;
    loop {
        let Some(T::Id(nome)) = tk.get(i).map(|t| t.t.clone()) else {
            return Err(tr!("esperava o nome de uma etapa depois do let").into());
        };
        if tk.get(i + 1).map(|t| &t.t) != Some(&T::S("=")) {
            return Err(trf!("esperava = depois de {nome}"));
        }
        let ini = i + 2;
        let (mut j, mut fundo, mut lets) = (ini, 0i32, 0i32);
        while j < tk.len() {
            match &tk[j].t {
                T::S("(" | "[" | "{") => fundo += 1,
                T::S(")" | "]" | "}") => fundo -= 1,
                T::Id(x) if x == "let" => lets += 1,
                T::Id(x) if x == "in" && fundo == 0 => { if lets == 0 { break; } lets -= 1; }
                T::S(",") if fundo == 0 && lets == 0 => break,
                _ => {}
            }
            j += 1;
        }
        saida.push(Ligacao { nome, expr: expressao(&tk[ini..j], src), texto: texto(ini, j) });
        match tk.get(j).map(|t| &t.t) {
            Some(T::S(",")) => i = j + 1,
            _ => break,
        }
    }
    Ok(saida)
}

/// Texto colado parece M (e não o script do FeatherQuery)?
pub(super) fn parece_m(texto: &str) -> bool {
    let t = texto.trim_start();
    let comeca = t.starts_with("let") && t[3..].starts_with(|c: char| c.is_whitespace());
    let m = ["Table.", "Excel.", "Csv.Document", "File.Contents", "Folder.Files", "Web.Contents", " in\n", "\nin"]
        .iter().any(|x| t.contains(x));
    // O script do FeatherQuery também começa com `let fonte = …;` e nunca tem `Table.`.
    (comeca || t.starts_with("section ")) && m && !t.contains("fonte.")
}

/// As consultas de um documento `section Section1; shared Nome = let … in …;`
/// (o que o Excel guarda dentro do `.xlsx`).
pub(super) fn consultas_da_secao(texto: &str) -> R<Vec<(String, String)>> {
    let tk = lexico(texto)?;
    let mut saida = Vec::new();
    let mut i = 0;
    while i < tk.len() {
        if matches!(&tk[i].t, T::Id(x) if x == "shared") {
            let (Some(T::Id(nome)), Some(T::S("="))) = (tk.get(i + 1).map(|t| t.t.clone()), tk.get(i + 2).map(|t| t.t.clone())) else {
                i += 1;
                continue;
            };
            let ini = i + 3;
            let mut j = ini;
            let mut fundo = 0i32;
            while j < tk.len() {
                match &tk[j].t {
                    T::S("(" | "[" | "{") => fundo += 1,
                    T::S(")" | "]" | "}") => fundo -= 1,
                    T::S(";") if fundo == 0 => break,
                    _ => {}
                }
                j += 1;
            }
            if ini < j { saida.push((nome, texto[tk[ini].ini..tk[j - 1].fim].to_owned())); }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    Ok(saida)
}

// ───────────────────────── M → script ─────────────────────────

/// O resultado de converter um M colado.
#[derive(Debug, Default)]
pub(super) struct DoM {
    /// A fonte do M em forma de script (`lerArquivo("x.xlsx")`), se reconhecida.
    pub fonte: Option<String>,
    /// `fonte.planilha("Dados");` / `fonte.tabela("T");`, quando o M navega no arquivo.
    pub navegacao: Option<String>,
    /// Linhas de etapa (e comentários do que não converteu).
    pub linhas: Vec<String>,
    pub nao_convertidas: usize,
}

impl DoM {
    /// O script inteiro, com `primeira` (a linha `let` da fonte atual) no topo.
    pub fn script(&self, primeira: &str, com_navegacao: bool) -> String {
        let mut t = vec![primeira.to_owned()];
        if com_navegacao { t.extend(self.navegacao.clone()); }
        if let Some(f) = &self.fonte {
            t.push(format!("// {}", trf!("No Power Query a fonte era {f}; aqui vale a escolhida em Obter dados.")));
        }
        t.extend(self.linhas.iter().cloned());
        t.join("\n") + "\n"
    }
}

fn num_json(n: f64) -> Json {
    if n.fract() == 0.0 && n.abs() < 1e15 { json!(n as i64) } else { json!(n) }
}

fn linha(metodo: &str, mut args: Vec<Json>) -> String {
    while args.last().is_some_and(Json::is_null) { args.pop(); }
    let args: Vec<String> = args.iter().map(|a| match a.as_f64() {
        Some(n) if a.is_number() => num_json(n).to_string(),
        _ => a.to_string(),
    }).collect();
    format!("fonte.{metodo}({});", args.join(", "))
}

fn txt(e: &E) -> Option<String> { if let E::Txt(s) = e { Some(s.clone()) } else { None } }
fn num(e: &E) -> Option<f64> {
    match e { E::Num(n) => Some(*n), E::Neg(x) => num(x).map(|n| -n), _ => None }
}
fn funcao(e: &E) -> Option<(&str, &[E])> {
    if let E::Chamada(f, args) = e { if let E::Id(nome) = f.as_ref() { return Some((nome.as_str(), args.as_slice())); } }
    None
}
/// `"A"` ou `{"A", "B"}`.
fn nomes(e: &E) -> Option<Vec<String>> {
    match e { E::Txt(s) => Some(vec![s.clone()]), E::Lista(v) => v.iter().map(txt).collect(), _ => None }
}
/// `{{"A", x}, {"B", y}}` ou só `{"A", x}`.
fn pares(e: &E) -> Option<Vec<(String, &E)>> {
    let E::Lista(v) = e else { return None };
    if v.first().is_some_and(|x| matches!(x, E::Txt(_))) && v.len() >= 2 {
        return Some(vec![(txt(&v[0])?, &v[1])]);
    }
    v.iter().map(|x| match x {
        E::Lista(p) if p.len() >= 2 => Some((txt(&p[0])?, &p[1])),
        _ => None,
    }).collect()
}
fn literal(e: &E) -> Option<Json> {
    match e {
        E::Txt(s) => Some(json!(s)),
        E::Num(_) | E::Neg(_) => num(e).map(num_json),
        E::Logico(b) => Some(json!(b)),
        E::Nulo => Some(Json::Null),
        _ => None,
    }
}
fn texto_do_numero(n: f64) -> String {
    // O critério e a fórmula são lidos em pt-BR: vírgula decimal.
    num_json(n).to_string().replace('.', ",")
}
fn cada(e: &E) -> Option<&E> {
    match e { E::Cada(x) => Some(x), E::Funcao(p, x) if p.len() == 1 => Some(x), _ => None }
}

fn tipo(e: &E) -> Option<&'static str> {
    let nome = match e { E::Tipo(n) => n.as_str(), E::Id(n) => n.as_str(), _ => return None };
    Some(match nome {
        "text" | "Text.Type" => "texto",
        "number" | "Number.Type" | "Currency.Type" | "Decimal.Type" | "Double.Type" | "Percentage.Type" | "Single.Type" => "decimal",
        "Int64.Type" | "Int32.Type" | "Int16.Type" | "Int8.Type" | "Byte.Type" => "inteiro",
        "logical" | "Logical.Type" => "logico",
        "date" | "Date.Type" => "data",
        "datetime" | "datetimezone" | "DateTime.Type" | "DateTimeZone.Type" => "dataHora",
        "time" | "Time.Type" => "hora",
        "any" | "Any.Type" => "",
        _ => return None,
    })
}

/// `contagem` do FeatherQuery conta só números (o CONT.NÚM): vai como
/// `List.Count(List.Select([c], each _ is number))`, tratado à parte.
const AGREGACOES_M: &[(&str, &str)] = &[
    ("List.Sum", "soma"), ("List.Average", "media"), ("List.Max", "maximo"), ("List.Min", "minimo"),
    ("List.Median", "mediana"), ("List.First", "primeiro"), ("List.Last", "ultimo"),
    ("List.NonNullCount", "contarValores"), ("List.Count", "contarValores"),
];

const PARTES_DATA: &[(&str, &str)] = &[
    ("Date.Year", "ano"), ("Date.MonthName", "mes"), ("Date.Day", "dia"), ("Date.DayOfWeekName", "diaSemana"),
];

/// Funções que valem igual dos dois lados: (M, fórmula em pt-BR, fórmula em inglês).
const FUNCOES: &[(&str, &str, &str)] = &[
    ("Text.Upper", "MAIÚSCULA", "UPPER"), ("Text.Lower", "MINÚSCULA", "LOWER"),
    ("Text.Proper", "PRI.MAIÚSCULA", "PROPER"), ("Text.Trim", "ARRUMAR", "TRIM"),
    ("Text.Length", "NÚM.CARACT", "LEN"), ("Text.Start", "ESQUERDA", "LEFT"), ("Text.End", "DIREITA", "RIGHT"),
    ("Number.Abs", "ABS", "ABS"), ("Number.From", "VALOR", "VALUE"), ("Number.Round", "ARRED", "ROUND"),
    ("Date.Year", "ANO", "YEAR"), ("Date.Month", "MÊS", "MONTH"), ("Date.Day", "DIA", "DAY"),
];

const FORMATOS_TEXTO: &[(&str, &str)] = &[
    ("Text.Trim", "aparar"), ("Text.Clean", "limpar"), ("Text.Upper", "maiusculas"),
    ("Text.Lower", "minusculas"), ("Text.Proper", "titulo"),
];

const EXTRAIR_TEXTO: &[(&str, &str)] = &[
    ("Text.BeforeDelimiter", "antes"), ("Text.AfterDelimiter", "depois"), ("Text.BetweenDelimiters", "entre"),
    ("Text.Start", "primeiros"), ("Text.End", "ultimos"), ("Text.Length", "tamanho"),
];

const JUNCOES: &[(&str, &str)] = &[
    ("JoinKind.LeftOuter", "esquerda"), ("JoinKind.RightOuter", "direita"), ("JoinKind.FullOuter", "total"),
    ("JoinKind.Inner", "interna"), ("JoinKind.LeftAnti", "anti_esquerda"), ("JoinKind.RightAnti", "anti_direita"),
];

/// `List.Min({[A], [B]})` ↔ `MÍNIMO([A];[B])`.
const LISTAS_FORMULA: &[(&str, &str)] = &[
    ("List.Min", "MÍNIMO"), ("List.Max", "MÁXIMO"), ("List.Sum", "SOMA"), ("List.Average", "MÉDIA"),
];

/// A expressão como fórmula da planilha (`=[Preço]*[Qtd]`), quando dá.
fn formula(e: &E) -> Option<String> {
    let par = |a: &E, b: &E| Some((formula(a)?, formula(b)?));
    Some(match e {
        E::Num(n) => texto_do_numero(*n),
        E::Txt(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        E::Logico(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
        // null na coluna nova: a célula fica vazia.
        E::Nulo => "\"\"".into(),
        E::Campo(c) => format!("[{c}]"),
        E::Neg(x) => format!("-{}", formula(x)?),
        E::Nao(x) => format!("NÃO({})", formula(x)?),
        E::Bin("and", a, b) => { let (a, b) = par(a, b)?; format!("E({a};{b})") }
        E::Bin("or", a, b) => { let (a, b) = par(a, b)?; format!("OU({a};{b})") }
        E::Bin(op, a, b) => { let (a, b) = par(a, b)?; format!("({a}{op}{b})") }
        E::Se(c, a, b) => format!("SE({};{};{})", formula(c)?, formula(a)?, formula(b)?),
        E::Chamada(..) => {
            let (f, args) = funcao(e)?;
            if let Some((_, pt)) = LISTAS_FORMULA.iter().find(|(m, _)| *m == f) {
                let [E::Lista(itens)] = args else { return None };
                let itens: Vec<String> = itens.iter().map(formula).collect::<Option<_>>()?;
                return Some(format!("{pt}({})", itens.join(";")));
            }
            let a: Vec<String> = args.iter().map(formula).collect::<Option<_>>()?;
            // O M conta do 0; o EXT.TEXTO, do 1.
            if let ("Text.Middle", [_, ini, _]) = (f, args) {
                let ini = match ini {
                    E::Num(n) => texto_do_numero(*n + 1.0),
                    E::Bin("-", x, um) if **um == E::Num(1.0) => formula(x)?,
                    outro => format!("{}+1", formula(outro)?),
                };
                return Some(format!("EXT.TEXTO({};{ini};{})", a[0], a[2]));
            }
            match (f, a.as_slice()) {
                ("Text.From", [x]) => format!("({x}&\"\")"),
                ("Number.Round", [x]) => format!("ARRED({x};0)"),
                _ => {
                    let (_, pt, _) = FUNCOES.iter().find(|(m, _, _)| *m == f)?;
                    format!("{pt}({})", a.join(";"))
                }
            }
        }
        _ => return None,
    })
}

/// Uma comparação simples de uma coluna: `[UF] = "SP"`, `[Valor] > 100`, `Text.Contains([Nome], "x")`.
fn criterio(e: &E) -> Option<(String, Json)> {
    if let E::Bin(op, a, b) = e {
        let inverso = |op: &str| match op { "<" => ">", ">" => "<", "<=" => ">=", ">=" => "<=", x => x }.to_owned();
        let (c, op, lit) = match (a.as_ref(), b.as_ref()) {
            (E::Campo(c), lit) => (c.clone(), op.to_string(), lit),
            (lit, E::Campo(c)) => (c.clone(), inverso(op), lit),
            _ => return None,
        };
        if !["=", "<>", "<", ">", "<=", ">="].contains(&op.as_str()) { return None; }
        let crit = match lit {
            E::Nulo if op == "=" || op == "<>" => json!(op),
            E::Logico(b) if op == "=" => json!(b),
            E::Txt(s) if op == "=" => json!(if s.is_empty() { "=".to_owned() } else { s.clone() }),
            E::Txt(s) => json!(format!("{op}{s}")),
            _ => {
                let n = num(lit)?;
                if op == "=" { num_json(n) } else { json!(format!("{op}{}", texto_do_numero(n))) }
            }
        };
        return Some((c, crit));
    }
    let (f, args) = funcao(e)?;
    let (E::Campo(c), Some(s)) = (args.first()?, args.get(1).and_then(txt)) else { return None };
    let padrao = match f {
        "Text.Contains" => format!("*{s}*"),
        "Text.StartsWith" => format!("{s}*"),
        "Text.EndsWith" => format!("*{s}"),
        _ => return None,
    };
    Some((c.clone(), json!(padrao)))
}

fn juntar_ops<'e>(e: &'e E, op: &str, saida: &mut Vec<&'e E>) {
    match e {
        E::Bin(o, a, b) if *o == op => { juntar_ops(a, op, saida); juntar_ops(b, op, saida); }
        _ => saida.push(e),
    }
}

/// Converte o M em linhas do script.
pub(super) fn de_m(texto: &str) -> R<DoM> {
    let mut c = Conversor::default();
    let mut constantes: Vec<(String, E)> = Vec::new();
    for l in ligacoes(texto)? {
        match l.expr.map(|e| trocar_constantes(e, &constantes)) {
            // `coluna = "Status"` usada depois num filtro: é parâmetro, não etapa.
            Ok(e) if literal(&e).is_some() && !c.anteriores.is_empty() => { constantes.push((l.nome, e)); continue; }
            Ok(e) => c.etapa(&e, &l.texto),
            Err(_) => c.nao_convertida(&l.texto),
        }
        c.anteriores.push(l.nome);
    }
    if c.divisao_pendente.is_some() { c.dom.nao_convertidas += 1; }
    Ok(c.dom)
}

/// Troca o nome de uma constante (`col = "x"`) pelo valor dela, em toda a expressão.
fn trocar_constantes(e: E, constantes: &[(String, E)]) -> E {
    if constantes.is_empty() { return e; }
    let t = |x: Box<E>| Box::new(trocar_constantes(*x, constantes));
    let v = |x: Vec<E>| x.into_iter().map(|y| trocar_constantes(y, constantes)).collect();
    match e {
        E::Id(n) => constantes.iter().find(|(c, _)| *c == n).map(|(_, v)| v.clone()).unwrap_or(E::Id(n)),
        E::Lista(x) => E::Lista(v(x)),
        E::Registro(r) => E::Registro(r.into_iter().map(|(k, x)| (k, trocar_constantes(x, constantes))).collect()),
        E::Chamada(f, a) => E::Chamada(f, v(a)),
        E::Item(a, b) => E::Item(t(a), t(b)),
        E::Acesso(a, c) => E::Acesso(t(a), c),
        E::Cada(x) => E::Cada(t(x)),
        E::Funcao(p, x) => E::Funcao(p, t(x)),
        E::Bin(op, a, b) => E::Bin(op, t(a), t(b)),
        E::Nao(x) => E::Nao(t(x)),
        E::Neg(x) => E::Neg(t(x)),
        E::Se(a, b, c) => E::Se(t(a), t(b), t(c)),
        outro => outro,
    }
}

#[derive(Default)]
struct Conversor {
    dom: DoM,
    anteriores: Vec<String>,
    /// Logo depois da fonte de arquivo: o leitor já usa a 1ª linha como cabeçalho.
    cabecalho_automatico: bool,
    /// `Table.TransformColumns` com `Splitter`: a lista vira linhas no `ExpandListColumn` seguinte.
    divisao_pendente: Option<(String, String)>,
    /// As colunas de tabela dos `Table.NestedJoin` já feitos (o `Expand` delas já vem feito).
    juncoes: Vec<String>,
}

impl Conversor {
    fn nao_convertida(&mut self, texto: &str) {
        self.dom.nao_convertidas += 1;
        for (i, l) in texto.lines().enumerate() {
            let prefixo = if i == 0 { tr!("M não convertido:") } else { " " };
            self.dom.linhas.push(format!("// {prefixo} {}", l.trim_end()));
        }
    }

    fn por(&mut self, linhas: Vec<String>) {
        self.cabecalho_automatico = false;
        self.dom.linhas.extend(linhas);
    }

    fn consulta(&self, e: &E) -> Option<String> {
        match e {
            // As chaves das fontes: "Consulta: X" não é traduzida (#151); "Tabela: X" é.
            E::Id(n) if !self.anteriores.contains(n) => Some(format!("Consulta: {n}")),
            _ => self.tabela_do_livro(e).map(|nome| trf!("Tabela: {nome}")),
        }
    }

    /// `Excel.CurrentWorkbook(){[Name="T"]}[Content]` → "T".
    fn tabela_do_livro(&self, e: &E) -> Option<String> {
        let E::Acesso(base, campo) = e else { return None };
        let E::Item(livro, filtro) = base.as_ref() else { return None };
        let (f, _) = funcao(livro)?;
        let E::Registro(r) = filtro.as_ref() else { return None };
        (f == "Excel.CurrentWorkbook" && campo == "Content").then_some(())?;
        r.iter().find(|(k, _)| k == "Name").and_then(|(_, v)| txt(v))
    }

    /// Fonte e navegação: `Excel.Workbook(File.Contents("x"))`, `Fonte{[Item="Dados",Kind="Sheet"]}[Data]`…
    fn fonte(&mut self, e: &E) -> bool {
        let s = |t: &str| serde_json::to_string(t).unwrap_or_default();
        if let Some(t) = self.tabela_do_livro(e) {
            self.dom.fonte = Some(format!("lerTabela({})", s(&t)));
            return true;
        }
        if let E::Acesso(base, campo) = e {
            if let E::Item(_, filtro) = base.as_ref() {
                if let (E::Registro(r), "Data") = (filtro.as_ref(), campo.as_str()) {
                    let valor = |k: &str| r.iter().find(|(c, _)| c == k).and_then(|(_, v)| txt(v));
                    let item = valor("Item").or_else(|| valor("Name"));
                    if let Some(item) = item {
                        let metodo = if valor("Kind").as_deref() == Some("Table") { "tabela" } else { "planilha" };
                        self.dom.navegacao = Some(format!("fonte.{metodo}({});", s(&item)));
                        self.cabecalho_automatico = metodo == "planilha";
                        return true;
                    }
                }
            }
        }
        let Some((f, args)) = funcao(e) else { return false };
        let caminho = || args.first().and_then(|a| funcao(a)).filter(|(g, _)| *g == "File.Contents" || *g == "Web.Contents")
            .and_then(|(_, a)| a.first().and_then(txt));
        let fonte = match f {
            "Excel.Workbook" | "Parquet.Document" => caminho().map(|p| format!("lerArquivo({})", s(&p))),
            "Csv.Document" | "Json.Document" => caminho().map(|p| {
                let web = funcao(&args[0]).is_some_and(|(g, _)| g == "Web.Contents");
                let leitor = if web { "lerWeb" } else if f == "Json.Document" { "lerArquivo" } else { "lerCsv" };
                format!("{leitor}({})", s(&p))
            }),
            "Folder.Files" | "Folder.Contents" => args.first().and_then(txt).map(|p| format!("lerPasta({})", s(&p))),
            "Web.Page" => caminho().map(|p| format!("lerWeb({})", s(&p))),
            "Table.FromRows" | "#table" => Some("dadosDigitados()".into()),
            _ => None,
        };
        let Some(fonte) = fonte else { return false };
        self.cabecalho_automatico = f == "Csv.Document";
        self.dom.fonte = Some(fonte);
        true
    }

    fn etapa(&mut self, e: &E, texto: &str) {
        if self.anteriores.is_empty() && self.fonte(e) { return; }
        if self.dom.navegacao.is_none() && matches!(e, E::Acesso(..)) && self.fonte(e) { return; }
        if let E::Id(n) = e {
            if self.anteriores.contains(n) { return; }
            if self.anteriores.is_empty() {
                self.dom.fonte = Some(format!("lerConsulta({})", serde_json::to_string(n).unwrap_or_default()));
                return;
            }
        }
        match funcao(e).and_then(|(f, args)| self.tabela(f, args)) {
            Some(linhas) => self.por(linhas),
            None => self.nao_convertida(texto),
        }
    }

    fn tabela(&mut self, f: &str, a: &[E]) -> Option<Vec<String>> {
        let col = |e: &E| txt(e).map(Json::String);
        let cols = |e: &E| nomes(e).map(|v| Json::Array(v.into_iter().map(Json::String).collect()));
        let arg = |i: usize| a.get(i);
        let n_ou = |i: usize, padrao: f64| match arg(i) { None => Some(padrao), Some(x) => num(x) };
        let um = |l: String| Some(vec![l]);
        match f {
            "Table.PromoteHeaders" if self.cabecalho_automatico => Some(Vec::new()),
            "Table.PromoteHeaders" => um(linha("usarPrimeiraLinhaComoCabecalho", vec![])),
            "Table.DemoteHeaders" => um(linha("usarCabecalhoComoPrimeiraLinha", vec![])),
            "Table.Buffer" => Some(Vec::new()),
            "Table.TransformColumnTypes" => {
                let mut saida = Vec::new();
                for (c, t) in pares(arg(1)?)? {
                    let t = tipo(t)?;
                    if !t.is_empty() { saida.push(linha("mudarTipo", vec![json!(c), json!(t), json!(true)])); }
                }
                Some(saida)
            }
            "Table.SelectRows" => self.filtro(cada(arg(1)?)?),
            "Table.Sort" => {
                let chaves: Vec<Json> = match arg(1)? {
                    E::Txt(c) => vec![json!([c, "asc"])],
                    E::Lista(v) => v.iter().map(|k| match k {
                        E::Txt(c) => Some(json!([c, "asc"])),
                        E::Lista(p) => {
                            let desc = matches!(p.get(1), Some(E::Id(o)) if o == "Order.Descending") || p.get(1).and_then(num) == Some(1.0);
                            Some(json!([txt(p.first()?)?, if desc { "desc" } else { "asc" }]))
                        }
                        _ => None,
                    }).collect::<Option<_>>()?,
                    _ => return None,
                };
                um(linha("ordenar", vec![Json::Array(chaves)]))
            }
            "Table.SelectColumns" => um(linha("selecionarColunas", vec![cols(arg(1)?)?])),
            "Table.RemoveColumns" => um(linha("removerColunas", vec![cols(arg(1)?)?])),
            "Table.ReorderColumns" => um(linha("reordenarColunas", vec![cols(arg(1)?)?])),
            "Table.RenameColumns" => {
                let p: Vec<(String, String)> = pares(arg(1)?)?.into_iter().map(|(a, b)| Some((a, txt(b)?))).collect::<Option<_>>()?;
                if let [(de, para)] = p.as_slice() { return um(linha("renomearColuna", vec![json!(de), json!(para)])); }
                um(linha("renomearColunas", vec![json!(p.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>())]))
            }
            "Table.DuplicateColumn" => um(linha("duplicarColuna", vec![col(arg(1)?)?, arg(2).and_then(col).unwrap_or(Json::Null)])),
            "Table.ReplaceValue" => {
                let (de, para) = (literal(arg(1)?)?, literal(arg(2)?)?);
                (matches!(arg(3)?, E::Id(r) if r == "Replacer.ReplaceValue")).then_some(())?;
                Some(nomes(arg(4)?)?.into_iter().map(|c| linha("substituirValores", vec![json!(c), de.clone(), para.clone()])).collect())
            }
            "Table.ReplaceErrorValues" => pares(arg(1)?)?.into_iter()
                .map(|(c, v)| Some(linha("substituirErros", vec![json!(c), literal(v)?]))).collect(),
            "Table.RemoveRowsWithErrors" => Some(nomes(arg(1)?)?.into_iter().map(|c| linha("removerErros", vec![json!(c)])).collect()),
            "Table.Distinct" => um(linha("removerDuplicadas", vec![match arg(1) { Some(x) => cols(x)?, None => Json::Null }])),
            "Table.FirstN" => um(linha("manterPrimeiras", vec![num_json(num(arg(1)?)?)])),
            "Table.LastN" => um(linha("removerLinhas", vec![json!("manter_ultimas"), num_json(num(arg(1)?)?)])),
            "Table.Skip" | "Table.RemoveFirstN" => um(linha("removerLinhas", vec![json!("primeiras"), num_json(n_ou(1, 1.0)?)])),
            "Table.RemoveLastN" => um(linha("removerLinhas", vec![json!("ultimas"), num_json(n_ou(1, 1.0)?)])),
            "Table.Transpose" => um(linha("transpor", vec![])),
            "Table.ReverseRows" => um(linha("inverterLinhas", vec![])),
            "Table.FillDown" | "Table.FillUp" => Some(nomes(arg(1)?)?.into_iter()
                .map(|c| linha("preencher", vec![json!(c), json!(f == "Table.FillUp")])).collect()),
            "Table.AddIndexColumn" => um(linha("adicionarIndice", vec![
                num_json(n_ou(2, 0.0)?), num_json(n_ou(3, 1.0)?), col(arg(1)?)?,
            ])),
            "Table.Group" => {
                let chaves = nomes(arg(1)?)?;
                let lista = match arg(2)? { E::Lista(v) if v.first().is_some_and(|x| matches!(x, E::Txt(_))) => vec![arg(2)?], E::Lista(v) => v.iter().collect(), _ => return None };
                let mut agregados = Vec::new();
                for item in lista {
                    let E::Lista(p) = item else { return None };
                    let nome = txt(p.first()?)?;
                    let corpo = cada(p.get(1)?)?;
                    let (g, ga) = funcao(corpo)?;
                    let (coluna, como) = match (g, ga) {
                        // Contar linhas: as preenchidas da chave do grupo (que nunca é vazia).
                        ("Table.RowCount", [E::Id(x)]) if x == "_" => (chaves.first()?.clone(), "contarValores"),
                        ("List.Count", [x]) if funcao(x).is_some_and(|(h, _)| h == "List.Distinct" || h == "List.Select") => {
                            let (h, d) = funcao(x)?;
                            let E::Campo(c) = d.first()? else { return None };
                            (c.clone(), if h == "List.Distinct" { "distintos" } else { "contagem" })
                        }
                        (g, [E::Campo(c)]) => (c.clone(), AGREGACOES_M.iter().find(|(m, _)| *m == g)?.1),
                        _ => return None,
                    };
                    agregados.push(json!([coluna, como, nome]));
                }
                um(linha("agruparPor", vec![json!(chaves), Json::Array(agregados)]))
            }
            "Table.Pivot" => {
                let como = match arg(4) {
                    None => "primeiro",
                    Some(E::Id(g)) => AGREGACOES_M.iter().find(|(m, _)| *m == g.as_str())?.1,
                    Some(_) => return None,
                };
                um(linha("pivotarColuna", vec![col(arg(2)?)?, col(arg(3)?)?, json!(como)]))
            }
            "Table.UnpivotOtherColumns" | "Table.Unpivot" => {
                let metodo = if f == "Table.Unpivot" { "despivotarSelecionadas" } else { "despivotarColunas" };
                um(linha(metodo, vec![cols(arg(1)?)?, col(arg(2)?)?, col(arg(3)?)?]))
            }
            "Table.NestedJoin" | "Table.Join" => {
                let fonte = self.consulta(arg(2)?)?;
                let modo_i = if f == "Table.NestedJoin" { 5 } else { 4 };
                let modo = match arg(modo_i) {
                    None => "esquerda",
                    Some(E::Id(k)) => JUNCOES.iter().find(|(m, _)| *m == k.as_str())?.1,
                    Some(_) => return None,
                };
                let um_ou_lista = |e: &E| { let v = nomes(e)?; Some(if v.len() == 1 { json!(v[0]) } else { json!(v) }) };
                let l = linha("mesclarConsultas", vec![json!(fonte), um_ou_lista(arg(1)?)?, um_ou_lista(arg(3)?)?, json!(modo)]);
                if f == "Table.NestedJoin" { self.juncoes.push(txt(arg(4)?)?); }
                um(l)
            }
            "Table.ExpandTableColumn" => {
                let coluna = txt(arg(1)?)?;
                let i = self.juncoes.iter().position(|j| *j == coluna)?;
                self.juncoes.remove(i);
                um(format!("// {}", tr!("(Expandir: a mesclagem do FeatherQuery já traz as colunas da outra consulta.)")))
            }
            "Table.Combine" => {
                let E::Lista(v) = arg(0)? else { return None };
                let anterior = self.anteriores.last()?.clone();
                (v.first()? == &E::Id(anterior)).then_some(())?;
                v[1..].iter().map(|x| Some(linha("acrescentarConsultas", vec![json!(self.consulta(x)?)]))).collect()
            }
            "Table.AddColumn" => self.coluna_nova(&txt(arg(1)?)?, cada(arg(2)?)?),
            "Table.TransformColumns" => {
                let mut saida = Vec::new();
                for (c, fx) in pares(arg(1)?)? {
                    saida.extend(self.transformar(&c, fx)?);
                }
                Some(saida)
            }
            "Table.ExpandListColumn" => {
                let c = txt(arg(1)?)?;
                let (pc, sep) = self.divisao_pendente.take()?;
                (pc == c).then(|| vec![linha("dividirEmLinhas", vec![json!(c), json!(sep)])])
            }
            "Table.SplitColumn" => {
                let c = txt(arg(1)?)?;
                let (g, ga) = funcao(arg(2)?)?;
                match g {
                    "Splitter.SplitTextByDelimiter" => um(linha("dividirColuna", vec![json!(c), json!(txt(ga.first()?)?)])),
                    "Splitter.SplitTextByEachDelimiter" => um(linha("dividirColuna", vec![json!(c), json!(nomes(ga.first()?)?.first()?.clone())])),
                    "Splitter.SplitTextByPositions" => {
                        let E::Lista(p) = ga.first()? else { return None };
                        let p: Vec<Json> = p.iter().map(|x| num(x).map(num_json)).collect::<Option<_>>()?;
                        um(linha("dividirPorPosicao", vec![json!(c), Json::Array(p)]))
                    }
                    "Splitter.SplitTextByRepeatedLengths" => um(linha("dividirPorPosicao", vec![json!(c), json!([]), num_json(num(ga.first()?)?)])),
                    _ => None,
                }
            }
            "Table.CombineColumns" => {
                let origem = nomes(arg(1)?)?;
                let (g, ga) = funcao(arg(2)?)?;
                (g == "Combiner.CombineTextByDelimiter").then_some(())?;
                let sep = txt(ga.first()?)?;
                Some(vec![
                    linha("mesclarColunas", vec![json!(origem), json!(sep), col(arg(3)?)?]),
                    linha("removerColunas", vec![json!(origem)]),
                ])
            }
            _ => None,
        }
    }

    fn filtro(&mut self, cond: &E) -> Option<Vec<String>> {
        if let Some((c, crit)) = criterio(cond) {
            return Some(vec![linha("filtrar", vec![json!(c), crit])]);
        }
        for op in ["and", "or"] {
            if matches!(cond, E::Bin(o, ..) if *o == op) {
                let mut partes = Vec::new();
                juntar_ops(cond, op, &mut partes);
                let condicoes: Vec<Json> = partes.iter().map(|p| criterio(p).map(|(c, k)| json!([c, k]))).collect::<Option<_>>()?;
                return Some(vec![linha("filtrarVarias", vec![Json::Array(condicoes), json!(op == "or")])]);
            }
        }
        if let Some(("List.Contains", [E::Lista(valores), E::Campo(c)])) = funcao(cond) {
            let valores: Vec<Json> = valores.iter().map(literal).collect::<Option<_>>()?;
            return Some(vec![linha("filtrarValores", vec![json!(c), Json::Array(valores)])]);
        }
        // O "Remover linhas em branco" do Power Query.
        if let E::Nao(x) = cond {
            let (f, a) = funcao(x)?;
            let interno = a.first().and_then(funcao).filter(|(g, _)| *g == "List.RemoveMatchingItems")?;
            let campos = interno.1.first().and_then(funcao).filter(|(g, _)| *g == "Record.FieldValues");
            if f == "List.IsEmpty" && campos.is_some() {
                return Some(vec![linha("removerLinhas", vec![json!("vazias")])]);
            }
        }
        None
    }

    fn extracao(c: &str, f: &str, a: &[E], nome: Option<&str>) -> Option<String> {
        let como = EXTRAIR_TEXTO.iter().find(|(m, _)| *m == f)?.1;
        let (texto, texto2, quantos) = match como {
            "antes" | "depois" => (Some(txt(a.get(1)?)?), None, None),
            "entre" => (Some(txt(a.get(1)?)?), Some(txt(a.get(2)?)?), None),
            "primeiros" | "ultimos" => (None, None, Some(num(a.get(1)?)?)),
            _ => (None, None, None),
        };
        Some(linha("extrairTexto", vec![
            json!(c), json!(como), json!(texto), json!(texto2), quantos.map(num_json).unwrap_or(Json::Null),
            nome.map(|n| json!(n)).unwrap_or(Json::Null),
        ]))
    }

    fn transformar(&mut self, c: &str, fx: &E) -> Option<Vec<String>> {
        if let E::Id(f) = fx {
            let como = FORMATOS_TEXTO.iter().find(|(m, _)| *m == f.as_str())?.1;
            return Some(vec![linha("formatarTexto", vec![json!(c), json!(como)])]);
        }
        if let Some(("Splitter.SplitTextByDelimiter", a)) = funcao(fx) {
            self.divisao_pendente = Some((c.to_owned(), txt(a.first()?)?));
            return Some(Vec::new());
        }
        let corpo = cada(fx)?;
        if let E::Bin("&", a, b) = corpo {
            return match (a.as_ref(), b.as_ref()) {
                (E::Txt(t), E::Id(x)) if x == "_" => Some(vec![linha("formatarTexto", vec![json!(c), json!("prefixo"), json!(t)])]),
                (E::Id(x), E::Txt(t)) if x == "_" => Some(vec![linha("formatarTexto", vec![json!(c), json!("sufixo"), json!(t)])]),
                _ => None,
            };
        }
        let (f, a) = funcao(corpo)?;
        (a.first()? == &E::Id("_".into())).then_some(())?;
        if f == "Text.Split" {
            self.divisao_pendente = Some((c.to_owned(), txt(a.get(1)?)?));
            return Some(Vec::new());
        }
        if let Some((_, como)) = FORMATOS_TEXTO.iter().find(|(m, _)| *m == f) {
            return Some(vec![linha("formatarTexto", vec![json!(c), json!(como)])]);
        }
        Some(vec![Self::extracao(c, f, a, None)?])
    }

    fn coluna_nova(&mut self, nome: &str, e: &E) -> Option<Vec<String>> {
        if let Some((f, a)) = funcao(e) {
            if let (Some((_, parte)), [E::Campo(c)]) = (PARTES_DATA.iter().find(|(m, _)| *m == f), a) {
                return Some(vec![linha("extrair", vec![json!(c), json!(parte), json!(nome)])]);
            }
            if let ("Date.ToText", [E::Campo(c), E::Txt(fmt)]) = (f, a) {
                if fmt == "yyyy-MM" { return Some(vec![linha("extrair", vec![json!(c), json!("anoMes"), json!(nome)])]); }
            }
            if let Some(E::Campo(c)) = a.first() {
                if let Some(l) = Self::extracao(c, f, a, Some(nome)) { return Some(vec![l]); }
            }
            if f == "Text.Combine" {
                let E::Lista(itens) = a.first()? else { return None };
                let campo = |x: &E| match x {
                    E::Campo(c) => Some(c.clone()),
                    _ => match funcao(x)? { ("Text.From", [E::Campo(c)]) => Some(c.clone()), _ => None },
                };
                let origem: Vec<String> = itens.iter().map(campo).collect::<Option<_>>()?;
                let sep = a.get(1).map(txt).unwrap_or(Some(String::new()))?;
                return Some(vec![linha("mesclarColunas", vec![json!(origem), json!(sep), json!(nome)])]);
            }
        }
        if let E::Se(cond, a, b) = e {
            if let (Some(entao), Some(senao)) = (txt(a), txt(b)) {
                let simples = match cond.as_ref() {
                    E::Bin(op, x, v) => match (x.as_ref(), literal(v)) {
                        (E::Campo(c), Some(v)) if !v.is_null() => {
                            let op = match *op { "=" => "=", "<>" => "!=", ">" => ">", "<" => "<", _ => "" };
                            let v = v.as_f64().map(texto_do_numero).unwrap_or_else(|| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()));
                            (!op.is_empty()).then(|| (c.clone(), op, v))
                        }
                        _ => None,
                    },
                    outro => funcao(outro).and_then(|(f, args)| {
                        let op = match f { "Text.Contains" => "contem", "Text.StartsWith" => "comeca_com", "Text.EndsWith" => "termina_com", _ => return None };
                        match args { [E::Campo(c), E::Txt(v), ..] => Some((c.clone(), op, v.clone())), _ => None }
                    }),
                };
                if let Some((c, op, v)) = simples {
                    return Some(vec![linha("colunaCondicional", vec![json!(nome), json!(c), json!(op), json!(v), json!(entao), json!(senao)])]);
                }
            }
        }
        let f = formula(e)?;
        Some(vec![linha("colunaPersonalizada", vec![json!(nome), json!(format!("={f}"))])])
    }
}

// ───────────────────────── fórmula → M ─────────────────────────

/// Lê a fórmula da coluna personalizada (`=[Preço]*[Qtd]`) como expressão, para escrever em M.
fn de_formula(f: &str) -> Option<E> {
    let s = f.trim().strip_prefix('=')?;
    let cs: Vec<char> = s.chars().collect();
    let mut tk: Vec<FTk> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() { i += 1; continue; }
        if c == '[' {
            let fim = cs[i..].iter().position(|x| *x == ']')? + i;
            tk.push(FTk::Campo(cs[i + 1..fim].iter().collect()));
            i = fim + 1;
        } else if c == '"' {
            let mut t = String::new();
            i += 1;
            loop {
                let ch = *cs.get(i)?;
                if ch == '"' { if cs.get(i + 1) == Some(&'"') { t.push('"'); i += 2; continue; } i += 1; break; }
                t.push(ch);
                i += 1;
            }
            tk.push(FTk::Txt(t));
        } else if c.is_ascii_digit() {
            let ini = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || (cs[i] == ',' && cs.get(i + 1).is_some_and(char::is_ascii_digit))) { i += 1; }
            let n: String = cs[ini..i].iter().collect();
            tk.push(FTk::Num(n.replace(',', ".").parse().ok()?));
        } else if c.is_alphabetic() || c == '_' {
            let ini = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '.' || cs[i] == '_') { i += 1; }
            tk.push(FTk::Nome(cs[ini..i].iter().collect::<String>().to_uppercase()));
        } else {
            let dois: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            let op = ["<>", "<=", ">="].iter().find(|o| **o == dois).copied()
                .or_else(|| ["+", "-", "*", "/", "&", "=", "<", ">", "(", ")", ";"].iter().find(|o| o.starts_with(c)).copied())?;
            i += op.len();
            tk.push(FTk::Op(op));
        }
    }
    let mut p = LeitorFormula { tk, i: 0 };
    let e = p.comparacao()?;
    (p.i == p.tk.len()).then_some(e)
}

#[derive(Clone, Debug, PartialEq)]
enum FTk { Campo(String), Txt(String), Num(f64), Nome(String), Op(&'static str) }

struct LeitorFormula { tk: Vec<FTk>, i: usize }

impl LeitorFormula {
    fn op(&self, o: &str) -> bool { matches!(self.tk.get(self.i), Some(FTk::Op(x)) if *x == o) }
    fn bin(&mut self, ops: &[&'static str], prox: fn(&mut Self) -> Option<E>) -> Option<E> {
        let mut e = prox(self)?;
        while let Some(o) = ops.iter().find(|o| self.op(o)).copied() {
            self.i += 1;
            e = E::Bin(o, Box::new(e), Box::new(prox(self)?));
        }
        Some(e)
    }
    fn comparacao(&mut self) -> Option<E> { self.bin(&["=", "<>", "<", ">", "<=", ">="], Self::concat) }
    fn concat(&mut self) -> Option<E> { self.bin(&["&"], Self::soma) }
    fn soma(&mut self) -> Option<E> { self.bin(&["+", "-"], Self::produto) }
    fn produto(&mut self) -> Option<E> { self.bin(&["*", "/"], Self::unario) }
    fn unario(&mut self) -> Option<E> {
        if self.op("-") { self.i += 1; return Some(E::Neg(Box::new(self.unario()?))); }
        let t = self.tk.get(self.i)?.clone();
        self.i += 1;
        Some(match t {
            FTk::Campo(c) => E::Campo(c),
            FTk::Txt(s) => E::Txt(s),
            FTk::Num(n) => E::Num(n),
            FTk::Op("(") => { let e = self.comparacao()?; self.op(")").then_some(())?; self.i += 1; e }
            FTk::Nome(n) if !self.op("(") => match n.as_str() {
                "VERDADEIRO" | "TRUE" => E::Logico(true),
                "FALSO" | "FALSE" => E::Logico(false),
                _ => return None,
            },
            FTk::Nome(n) => {
                self.i += 1;
                let mut a = Vec::new();
                while !self.op(")") {
                    a.push(self.comparacao()?);
                    if self.op(";") { self.i += 1; } else { break; }
                }
                self.op(")").then_some(())?;
                self.i += 1;
                let chamar = |f: &str, a: Vec<E>| E::Chamada(Box::new(E::Id(f.into())), a);
                let mut a = a.into_iter();
                let mut prox = || a.next();
                match n.as_str() {
                    "SE" | "IF" => E::Se(Box::new(prox()?), Box::new(prox()?), Box::new(prox().unwrap_or(E::Logico(false)))),
                    "E" | "AND" => E::Bin("and", Box::new(prox()?), Box::new(prox()?)),
                    "OU" | "OR" => E::Bin("or", Box::new(prox()?), Box::new(prox()?)),
                    "NÃO" | "NOT" => E::Nao(Box::new(prox()?)),
                    "EXT.TEXTO" | "MID" => {
                        let (t, ini, q) = (prox()?, prox()?, prox()?);
                        let ini = match ini { E::Num(n) => E::Num(n - 1.0), outro => E::Bin("-", Box::new(outro), Box::new(E::Num(1.0))) };
                        chamar("Text.Middle", vec![t, ini, q])
                    }
                    _ if LISTAS_FORMULA.iter().any(|(_, pt)| *pt == n) => {
                        let (m, _) = LISTAS_FORMULA.iter().find(|(_, pt)| *pt == n)?;
                        chamar(m, vec![E::Lista(std::iter::from_fn(prox).collect())])
                    }
                    _ => {
                        let (m, _, _) = FUNCOES.iter().find(|(_, pt, en)| *pt == n || *en == n)?;
                        let resto: Vec<E> = std::iter::from_fn(prox).collect();
                        chamar(m, resto)
                    }
                }
            }
            _ => return None,
        })
    }
}

// ───────────────────────── script → M ─────────────────────────

fn m_txt(s: &str) -> String {
    let mut r = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => r.push_str("\"\""),
            '\n' => r.push_str("#(lf)"),
            '\r' => r.push_str("#(cr)"),
            '\t' => r.push_str("#(tab)"),
            _ => r.push(c),
        }
    }
    r.push('"');
    r
}

const PALAVRAS_M: &[&str] = &[
    "and", "as", "each", "else", "error", "false", "if", "in", "is", "let", "meta", "not", "null", "or",
    "otherwise", "section", "shared", "then", "true", "try", "type",
];

fn m_id(nome: &str) -> String {
    let simples = nome.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !PALAVRAS_M.contains(&nome);
    if simples { nome.to_owned() } else { format!("#{}", m_txt(nome)) }
}

fn m_campo(nome: &str) -> String {
    let simples = !nome.is_empty() && nome.trim() == nome
        && nome.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '_');
    if simples { format!("[{nome}]") } else { format!("[#{}]", m_txt(nome)) }
}

fn m_num(n: f64) -> String { num_json(n).to_string() }

fn m_lit(v: &Json) -> String {
    match v {
        Json::String(s) => m_txt(s),
        Json::Number(n) => n.as_f64().map(m_num).unwrap_or_else(|| n.to_string()),
        Json::Bool(b) => b.to_string(),
        _ => "null".into(),
    }
}

fn m(e: &E) -> String {
    let lista = |v: &[E]| v.iter().map(m).collect::<Vec<_>>().join(", ");
    match e {
        E::Num(n) => m_num(*n),
        E::Txt(s) => m_txt(s),
        E::Logico(b) => b.to_string(),
        E::Nulo => "null".into(),
        E::Id(x) | E::Cru(x) => x.clone(),
        E::Campo(c) => m_campo(c),
        E::Lista(v) => format!("{{{}}}", lista(v)),
        E::Registro(r) => format!("[{}]", r.iter().map(|(k, v)| format!("{k} = {}", m(v))).collect::<Vec<_>>().join(", ")),
        E::Chamada(f, a) => format!("{}({})", m(f), lista(a)),
        E::Item(b, i) => format!("{}{{{}}}", m(b), m(i)),
        E::Acesso(b, c) => format!("{}{}", m(b), m_campo(c)),
        E::Cada(x) => format!("each {}", m(x)),
        E::Funcao(p, x) => format!("({}) => {}", p.join(", "), m(x)),
        E::Bin(op, a, b) => format!("({} {op} {})", m(a), m(b)),
        E::Nao(x) => format!("not {}", m(x)),
        E::Neg(x) => format!("-{}", m(x)),
        E::Se(c, a, b) => format!("if {} then {} else {}", m(c), m(a), m(b)),
        E::Tipo(t) => format!("type {t}"),
    }
}

fn chamar(f: &str, a: Vec<E>) -> E { E::Chamada(Box::new(E::Id(f.into())), a) }

/// A condição de M de um critério do filtro (`">100"`, `"SP"`, `"*ab*"`, `"<>"`).
fn condicao(campo: &E, crit: &Json) -> E {
    let c = || Box::new(campo.clone());
    let bin = |op: &'static str, v: E| E::Bin(op, c(), Box::new(v));
    let valor = |r: &str| match r.replace(',', ".").parse::<f64>() {
        Ok(n) if !r.is_empty() && r.trim() == r => E::Num(n),
        _ => E::Txt(r.to_owned()),
    };
    let s = match crit {
        Json::String(s) => s.as_str(),
        Json::Number(n) => return bin("=", E::Num(n.as_f64().unwrap_or(0.0))),
        Json::Bool(b) => return bin("=", E::Logico(*b)),
        _ => return bin("=", E::Nulo),
    };
    let (op, r): (&'static str, &str) = ["<>", ">=", "<=", "=", ">", "<"].iter()
        .find_map(|o| s.strip_prefix(o).map(|r| (*o, r))).unwrap_or(("", s));
    let vazio = || E::Bin("or", Box::new(bin("=", E::Nulo)), Box::new(bin("=", E::Txt(String::new()))));
    match (op, r) {
        ("=", "") => vazio(),
        ("<>", "") => E::Nao(Box::new(vazio())),
        ("" | "=" | "<>", r) if r.contains('*') && !r.contains('?') => {
            let meio = r.trim_matches('*');
            let f = match (r.starts_with('*'), r.ends_with('*')) {
                (true, true) => "Text.Contains",
                (false, true) => "Text.StartsWith",
                (true, false) => "Text.EndsWith",
                _ => "",
            };
            if f.is_empty() || meio.contains('*') { return bin(if op == "<>" { "<>" } else { "=" }, E::Txt(r.to_owned())); }
            let e = chamar(f, vec![campo.clone(), E::Txt(meio.to_owned())]);
            if op == "<>" { E::Nao(Box::new(e)) } else { e }
        }
        ("", r) => bin("=", valor(r)),
        (op, r) => bin(op, valor(r)),
    }
}

/// Nomes das etapas como o Power Query em cada língua.
fn nome_etapa(metodo: &str) -> String {
    match metodo {
        "filtrar" | "filtrarVarias" | "filtrarValores" => tr!("Linhas Filtradas"),
        "ordenar" => tr!("Linhas Classificadas"),
        "selecionarColunas" => tr!("Outras Colunas Removidas"),
        "removerColunas" => tr!("Colunas Removidas"),
        "renomearColuna" | "renomearColunas" => tr!("Colunas Renomeadas"),
        "mudarTipo" => tr!("Tipo Alterado"),
        "substituirValores" => tr!("Valor Substituído"),
        "substituirErros" => tr!("Erros Substituídos"),
        "removerErros" => tr!("Erros Removidos"),
        "mesclarColunas" => tr!("Colunas Mescladas"),
        "dividirEmLinhas" | "dividirColuna" | "dividirPorPosicao" => tr!("Dividir Coluna"),
        "removerDuplicadas" => tr!("Duplicatas Removidas"),
        "agruparPor" => tr!("Linhas Agrupadas"),
        "pivotar" | "pivotarColuna" => tr!("Coluna Dinâmica"),
        "mesclarConsultas" => tr!("Consultas Mescladas"),
        "acrescentarConsultas" => tr!("Consulta Acrescentada"),
        "despivotarColunas" | "despivotarSelecionadas" => tr!("Colunas Não Dinâmicas"),
        "usarPrimeiraLinhaComoCabecalho" => tr!("Cabeçalhos Promovidos"),
        "usarCabecalhoComoPrimeiraLinha" => tr!("Cabeçalhos Rebaixados"),
        "adicionarIndice" => tr!("Índice Adicionado"),
        "manterPrimeiras" => tr!("Linhas Superiores Mantidas"),
        "removerLinhas" => tr!("Linhas Removidas"),
        "transpor" => tr!("Tabela Transposta"),
        "inverterLinhas" => tr!("Linhas Invertidas"),
        "preencher" => tr!("Preenchido"),
        "reordenarColunas" => tr!("Colunas Reordenadas"),
        "duplicarColuna" => tr!("Coluna Duplicada"),
        "formatarTexto" => tr!("Texto Formatado"),
        _ => tr!("Personalização Adicionada"),
    }.to_owned()
}

struct Escritor<'a> {
    /// (nome, expressão, comentários que vão na linha de cima).
    etapas: Vec<(String, String, Vec<String>)>,
    comentarios: Vec<String>,
    fontes: &'a HashMap<String, Tabela>,
    sem_equivalente: usize,
}

impl Escritor<'_> {
    fn anterior(&self) -> String { self.etapas.last().map(|(n, _, _)| m_id(n)).unwrap_or_else(|| "Fonte".into()) }

    fn por(&mut self, nome: &str, expr: String) {
        let base = nome.to_owned();
        let mut nome = base.clone();
        let mut k = 1;
        while self.etapas.iter().any(|(n, _, _)| *n == nome) { nome = format!("{base}{k}"); k += 1; }
        let comentarios = std::mem::take(&mut self.comentarios);
        self.etapas.push((nome, expr, comentarios));
    }

    /// Nome da coluna em M: texto, ou `Table.ColumnNames(…){n}` quando o script usa o número.
    fn col(&self, v: &Json) -> String {
        match v {
            Json::Number(n) => format!("Table.ColumnNames({}){{{}}}", self.anterior(), n),
            Json::String(s) => m_txt(s),
            _ => "null".into(),
        }
    }
    fn campo(&self, v: &Json) -> E {
        match v {
            Json::String(s) => E::Campo(s.clone()),
            _ => E::Cru(format!("Record.Field(_, {})", self.col(v))),
        }
    }
    fn cols(&self, v: &Json) -> String {
        match v {
            Json::Array(a) => format!("{{{}}}", a.iter().map(|c| self.col(c)).collect::<Vec<_>>().join(", ")),
            Json::Null => "{}".into(),
            outro => format!("{{{}}}", self.col(outro)),
        }
    }
    fn nome_col(&self, v: &Json) -> String {
        match v { Json::String(s) => s.clone(), Json::Number(n) => format!("Coluna {}", n.as_u64().unwrap_or(0) + 1), _ => String::new() }
    }

    fn referencia(&self, fonte: &str) -> String {
        let tabela = { let nome = ""; trf!("Tabela: {nome}") };
        if let Some(t) = fonte.strip_prefix(tabela.as_str()).or_else(|| fonte.strip_prefix("Tabela: ")) {
            return format!("Excel.CurrentWorkbook(){{[Name = {}]}}[Content]", m_txt(t));
        }
        let nome = fonte.strip_prefix("Consulta: ").unwrap_or(fonte);
        m_id(nome)
    }

    fn sem_equivalente(&mut self, comando: &str) {
        self.sem_equivalente += 1;
        self.comentarios.push(trf!("Sem equivalente no Power Query: {comando}"));
    }

    fn fonte(&mut self, chamada: &str, args: &[Json], navegacao: Option<(&str, String)>, manual: Option<&Tabela>) {
        let a0 = args.first().and_then(Json::as_str).unwrap_or("");
        let arquivo = |p: &str| format!("File.Contents({})", m_txt(p));
        let promover = |e: &mut Self| e.por(&tr!("Cabeçalhos Promovidos"), format!("Table.PromoteHeaders({}, [PromoteAllScalars = true])", e.anterior()));
        match chamada {
            "lerCsv" => {
                self.comentarios.push(tr!("Ajuste o Delimiter se o arquivo separa por ponto e vírgula.").into());
                self.por("Fonte", format!("Csv.Document({}, [Delimiter = \",\", Encoding = 65001, QuoteStyle = QuoteStyle.Csv])", arquivo(a0)));
                promover(self);
            }
            "lerArquivo" if a0.to_lowercase().ends_with(".parquet") => self.por("Fonte", format!("Parquet.Document({})", arquivo(a0))),
            // O Excel só lê SQLite pelo ODBC (driver "SQLite3 ODBC Driver" instalado à parte).
            "lerArquivo" if [".sqlite", ".sqlite3", ".db"].iter().any(|e| a0.to_lowercase().ends_with(e)) => {
                self.comentarios.push(tr!("O Excel lê SQLite pelo ODBC: instale o SQLite3 ODBC Driver.").into());
                self.por("Fonte", format!("Odbc.DataSource({}, [HierarchicalNavigation = true])", m_txt(&format!("Driver={{SQLite3 ODBC Driver}};Database={a0}"))));
                if let Some((_, t)) = navegacao {
                    self.por(&format!("{t}_Table"), format!("Fonte{{[Name = {}, Kind = \"Table\"]}}[Data]", m_txt(&t)));
                }
            }
            "lerArquivo" => {
                if !a0.to_lowercase().contains(".xls") {
                    self.comentarios.push(tr!("O Excel não lê .fshz: salve o arquivo como .xlsx e troque o caminho.").into());
                }
                self.por("Fonte", format!("Excel.Workbook({}, null, true)", arquivo(a0)));
                match navegacao {
                    Some(("tabela", t)) => self.por(&format!("{t}_Table"), format!("Fonte{{[Item = {}, Kind = \"Table\"]}}[Data]", m_txt(&t))),
                    Some((_, p)) => {
                        self.por(&format!("{p}_Sheet"), format!("Fonte{{[Item = {}, Kind = \"Sheet\"]}}[Data]", m_txt(&p)));
                        promover(self);
                    }
                    None => {
                        self.por(&tr!("Navegação"), "Fonte{0}[Data]".into());
                        promover(self);
                    }
                }
            }
            "lerPasta" => {
                self.comentarios.push(tr!("Use Combinar arquivos no Excel para juntar o conteúdo da pasta.").into());
                self.por("Fonte", format!("Folder.Files({})", m_txt(a0)));
            }
            "lerWeb" => {
                self.comentarios.push(tr!("Se a página for HTML, troque Csv.Document por Web.Page.").into());
                self.por("Fonte", format!("Csv.Document(Web.Contents({}))", m_txt(a0)));
                promover(self);
            }
            "lerConsulta" => self.por("Fonte", m_id(a0)),
            "lerTabela" => self.por("Fonte", format!("Excel.CurrentWorkbook(){{[Name = {}]}}[Content]", m_txt(a0))),
            "dadosDigitados" => {
                let t = manual.cloned().unwrap_or(Tabela { cabecalho: Vec::new(), linhas: Vec::new() });
                let celula = |v: &Value| match v {
                    Value::Number(n) => m_num(*n),
                    Value::Text(s) => m_txt(s),
                    Value::Bool(b) => b.to_string(),
                    _ => "null".into(),
                };
                let linhas: Vec<String> = t.linhas.iter().take(10_000)
                    .map(|l| format!("{{{}}}", l.iter().map(celula).collect::<Vec<_>>().join(", "))).collect();
                let cab = t.cabecalho.iter().map(|c| m_txt(c)).collect::<Vec<_>>().join(", ");
                self.por("Fonte", format!("#table({{{cab}}}, {{{}}})", linhas.join(", ")));
            }
            _ => {
                // lerAba: o Power Query só enxerga tabela ou nome definido da pasta.
                self.comentarios.push(trf!("Crie no Excel uma tabela chamada \"{a0}\" com estes dados."));
                self.por("Fonte", format!("Excel.CurrentWorkbook(){{[Name = {}]}}[Content]", m_txt(a0)));
            }
        }
    }

    fn agregado(&self, a: &Json) -> Option<(String, String)> {
        let item = a.as_array()?;
        let campo = m(&self.campo(item.first()?));
        let como = item.get(1).and_then(Json::as_str).unwrap_or("soma");
        let corpo = match como {
            "contagem" => format!("List.Count(List.Select({campo}, each _ is number))"),
            "contarValores" => format!("List.NonNullCount({campo})"),
            "distintos" => format!("List.Count(List.Distinct({campo}))"),
            outro => {
                let (f, _) = AGREGACOES_M.iter().find(|(_, c)| *c == outro)?;
                format!("{f}({campo})")
            }
        };
        let nome = item.get(2).and_then(Json::as_str).map(str::to_owned).unwrap_or_else(|| {
            let rotulo = feathersheet_core::consulta::query::Agregacao::do_str(como).rotulo();
            format!("{rotulo} de {}", self.nome_col(&item[0]))
        });
        Some((nome, corpo))
    }

    fn etapa(&mut self, metodo: &str, a: &[Json], comando: &str) {
        let p = self.anterior();
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Json::Null);
        let s = |i: usize| a.get(i).and_then(Json::as_str).map(str::to_owned);
        let n = |i: usize, padrao: f64| a.get(i).and_then(Json::as_f64).unwrap_or(padrao);
        let lista = |i: usize| a.get(i).and_then(Json::as_array).cloned().unwrap_or_default();
        let nome = nome_etapa(metodo);
        let expr: Option<String> = match metodo {
            "filtrar" => Some(format!("Table.SelectRows({p}, each {})", m(&condicao(&self.campo(&arg(0)), &arg(1))))),
            "filtrarVarias" => {
                let op = if a.get(1).and_then(Json::as_bool).unwrap_or(false) { "or" } else { "and" };
                let partes: Vec<String> = lista(0).iter().filter_map(|par| {
                    let par = par.as_array()?;
                    Some(m(&condicao(&self.campo(par.first()?), par.get(1)?)))
                }).collect();
                Some(format!("Table.SelectRows({p}, each {})", partes.join(&format!(" {op} "))))
            }
            "filtrarValores" => Some(format!("Table.SelectRows({p}, each List.Contains({{{}}}, {}))",
                lista(1).iter().map(m_lit).collect::<Vec<_>>().join(", "), m(&self.campo(&arg(0))))),
            "ordenar" => Some(format!("Table.Sort({p}, {{{}}})", lista(0).iter().filter_map(|k| {
                let k = k.as_array()?;
                let ordem = if k.get(1).and_then(Json::as_str) == Some("desc") { "Order.Descending" } else { "Order.Ascending" };
                Some(format!("{{{}, {ordem}}}", self.col(k.first()?)))
            }).collect::<Vec<_>>().join(", "))),
            "selecionarColunas" => Some(format!("Table.SelectColumns({p}, {})", self.cols(&arg(0)))),
            "removerColunas" => Some(format!("Table.RemoveColumns({p}, {})", self.cols(&arg(0)))),
            "reordenarColunas" => Some(format!("Table.ReorderColumns({p}, {})", self.cols(&arg(0)))),
            "renomearColuna" => Some(format!("Table.RenameColumns({p}, {{{{{}, {}}}}})", self.col(&arg(0)), m_lit(&arg(1)))),
            "renomearColunas" => Some(format!("Table.RenameColumns({p}, {{{}}})", lista(0).iter().filter_map(|par| {
                let par = par.as_array()?;
                Some(format!("{{{}, {}}}", self.col(par.first()?), m_lit(par.get(1)?)))
            }).collect::<Vec<_>>().join(", "))),
            "mudarTipo" => {
                let t = match s(1).as_deref() {
                    Some("texto" | "link") => "type text",
                    Some("inteiro") => "Int64.Type",
                    Some("logico") => "type logical",
                    Some("data") => "type date",
                    Some("dataHora") => "type datetime",
                    Some("hora") => "type time",
                    _ => "type number",
                };
                Some(format!("Table.TransformColumnTypes({p}, {{{{{}, {t}}}}})", self.col(&arg(0))))
            }
            "substituirValores" => Some(format!("Table.ReplaceValue({p}, {}, {}, Replacer.ReplaceValue, {{{}}})",
                m_lit(&arg(1)), m_lit(&arg(2)), self.col(&arg(0)))),
            "substituirErros" => Some(format!("Table.ReplaceErrorValues({p}, {{{{{}, {}}}}})", self.col(&arg(0)), m_lit(&arg(1)))),
            "removerErros" => Some(format!("Table.RemoveRowsWithErrors({p}, {{{}}})", self.col(&arg(0)))),
            "mesclarColunas" => {
                let partes: Vec<String> = lista(0).iter().map(|c| format!("Text.From({})", m(&self.campo(c)))).collect();
                Some(format!("Table.AddColumn({p}, {}, each Text.Combine({{{}}}, {}), type text)",
                    m_txt(&s(2).unwrap_or_else(|| "Mesclado".into())), partes.join(", "), m_lit(&arg(1))))
            }
            "extrair" => {
                let campo = m(&self.campo(&arg(0)));
                let parte = s(1).unwrap_or_default();
                let corpo = match parte.as_str() {
                    "trimestre" => format!("Text.From(Date.Year({campo})) & \" T\" & Text.From(Date.QuarterOfYear({campo}))"),
                    "anoMes" => format!("Date.ToText({campo}, \"yyyy-MM\")"),
                    outra => PARTES_DATA.iter().find(|(_, x)| *x == outra).map(|(f, _)| format!("{f}({campo})")).unwrap_or_default(),
                };
                let nome = s(2).unwrap_or_else(|| format!("{} ({parte})", self.nome_col(&arg(0))));
                (!corpo.is_empty()).then(|| format!("Table.AddColumn({p}, {}, each {corpo})", m_txt(&nome)))
            }
            "dividirEmLinhas" => {
                let c = self.col(&arg(0));
                Some(format!("Table.ExpandListColumn(Table.TransformColumns({p}, {{{{{c}, Splitter.SplitTextByDelimiter({}, QuoteStyle.Csv)}}}}), {c})", m_lit(&arg(1))))
            }
            "dividirColuna" => Some(format!("Table.SplitColumn({p}, {}, Splitter.SplitTextByDelimiter({}, QuoteStyle.Csv))", self.col(&arg(0)), m_lit(&arg(1)))),
            "dividirPorPosicao" => {
                let divisor = match a.get(2).and_then(Json::as_f64) {
                    Some(cada) => format!("Splitter.SplitTextByRepeatedLengths({})", m_num(cada)),
                    None => format!("Splitter.SplitTextByPositions({{{}}})", lista(1).iter().map(m_lit).collect::<Vec<_>>().join(", ")),
                };
                Some(format!("Table.SplitColumn({p}, {}, {divisor})", self.col(&arg(0))))
            }
            "removerDuplicadas" => Some(match arg(0) {
                Json::Null => format!("Table.Distinct({p})"),
                c => format!("Table.Distinct({p}, {})", self.cols(&c)),
            }),
            "agruparPor" => {
                let aggs: Option<Vec<String>> = lista(1).iter().map(|x| self.agregado(x).map(|(nome, corpo)| format!("{{{}, each {corpo}}}", m_txt(&nome)))).collect();
                aggs.map(|aggs| format!("Table.Group({p}, {}, {{{}}})", self.cols(&arg(0)), aggs.join(", ")))
            }
            "pivotar" => {
                let (colunas, valores) = (lista(1), lista(2));
                match (colunas.as_slice(), valores.as_slice()) {
                    ([c], [v]) => {
                        let v = v.as_array().cloned().unwrap_or_default();
                        let como = v.get(1).and_then(Json::as_str).unwrap_or("soma");
                        let agg = AGREGACOES_M.iter().find(|(_, x)| *x == como).map(|(f, _)| *f).unwrap_or("List.Count");
                        let valor = v.first().cloned().unwrap_or(Json::Null);
                        let mut manter = lista(0);
                        manter.push(c.clone());
                        manter.push(valor.clone());
                        Some(format!("Table.Pivot(Table.SelectColumns({p}, {}), List.Distinct(Table.Column({p}, {})), {}, {}, {agg})",
                            self.cols(&Json::Array(manter)), self.col(c), self.col(c), self.col(&valor)))
                    }
                    _ => None,
                }
            }
            "mesclarConsultas" => {
                let fonte = s(0).unwrap_or_default();
                let outra = self.referencia(&fonte);
                let rotulo = fonte.split_once(": ").map(|(_, n)| n.to_owned()).unwrap_or_else(|| fonte.clone());
                let cab = self.fontes.get(&fonte).map(|t| t.cabecalho.clone());
                let lista_de = |v: &Json| match v { Json::Array(x) => x.clone(), outro => vec![outro.clone()] };
                let direita: Vec<String> = lista_de(&arg(2)).iter().map(|c| match (c, &cab) {
                    (Json::Number(n), Some(cab)) => cab.get(n.as_u64().unwrap_or(0) as usize).map(|x| m_txt(x)).unwrap_or_else(|| "null".into()),
                    (Json::Number(n), None) => format!("Table.ColumnNames({outra}){{{n}}}"),
                    (outro, _) => m_lit(outro),
                }).collect();
                let modo = s(3).unwrap_or_else(|| "esquerda".into());
                let tipo = JUNCOES.iter().find(|(_, x)| *x == modo).map(|(k, _)| *k).unwrap_or("JoinKind.LeftOuter");
                self.por(&nome, format!("Table.NestedJoin({p}, {}, {outra}, {{{}}}, {}, {tipo})",
                    self.cols(&Json::Array(lista_de(&arg(1)))), direita.join(", "), m_txt(&rotulo)));
                let junta = self.anterior();
                match cab {
                    Some(cab) => {
                        let nomes = cab.iter().map(|c| m_txt(c)).collect::<Vec<_>>().join(", ");
                        self.por(&tr!("Expandido"), format!("Table.ExpandTableColumn({junta}, {}, {{{nomes}}}, {{{nomes}}})", m_txt(&rotulo)));
                    }
                    None => self.comentarios.push(trf!("Expanda a coluna {rotulo} no Excel para trazer as colunas da outra consulta.")),
                }
                return;
            }
            "acrescentarConsultas" => Some(format!("Table.Combine({{{p}, {}}})", self.referencia(&s(0).unwrap_or_default()))),
            "despivotarColunas" => Some(format!("Table.UnpivotOtherColumns({p}, {}, {}, {})", self.cols(&arg(0)),
                m_txt(&s(1).unwrap_or_else(|| "Atributo".into())), m_txt(&s(2).unwrap_or_else(|| "Valor".into())))),
            "usarPrimeiraLinhaComoCabecalho" => Some(format!("Table.PromoteHeaders({p}, [PromoteAllScalars = true])")),
            "usarCabecalhoComoPrimeiraLinha" => Some(format!("Table.DemoteHeaders({p})")),
            "adicionarIndice" => Some(format!("Table.AddIndexColumn({p}, {}, {}, {}, Int64.Type)",
                m_txt(&s(2).unwrap_or_else(|| "Índice".into())), m_num(n(0, 0.0)), m_num(n(1, 1.0)))),
            "colunaCondicional" => {
                let campo = self.campo(&arg(1));
                let valor = s(3).unwrap_or_default();
                let v = || match valor.replace(',', ".").parse::<f64>() { Ok(x) => E::Num(x), Err(_) => E::Txt(valor.clone()) };
                let cond = match s(2).as_deref() {
                    Some("!=") => E::Bin("<>", Box::new(campo), Box::new(v())),
                    Some(">") => E::Bin(">", Box::new(campo), Box::new(v())),
                    Some("<") => E::Bin("<", Box::new(campo), Box::new(v())),
                    Some("contem") => chamar("Text.Contains", vec![campo, E::Txt(valor.clone())]),
                    Some("comeca_com") => chamar("Text.StartsWith", vec![campo, E::Txt(valor.clone())]),
                    Some("termina_com") => chamar("Text.EndsWith", vec![campo, E::Txt(valor.clone())]),
                    _ => E::Bin("=", Box::new(campo), Box::new(v())),
                };
                Some(format!("Table.AddColumn({p}, {}, each if {} then {} else {})", m_txt(&s(0).unwrap_or_default()),
                    m(&cond), m_lit(&arg(4)), m_lit(&arg(5))))
            }
            "manterPrimeiras" => Some(format!("Table.FirstN({p}, {})", m_num(n(0, 1.0)))),
            "formatarTexto" => {
                let c = self.col(&arg(0));
                let f = match s(1).as_deref() {
                    Some("prefixo") => Some(format!("each {} & _", m_lit(&arg(2)))),
                    Some("sufixo") => Some(format!("each _ & {}", m_lit(&arg(2)))),
                    Some(como) => FORMATOS_TEXTO.iter().find(|(_, x)| *x == como).map(|(f, _)| (*f).to_owned()),
                    None => None,
                };
                f.map(|f| format!("Table.TransformColumns({p}, {{{{{c}, {f}, type text}}}})"))
            }
            "preencher" => Some(format!("Table.{}({p}, {{{}}})",
                if a.get(1).and_then(Json::as_bool).unwrap_or(false) { "FillUp" } else { "FillDown" }, self.col(&arg(0)))),
            "removerLinhas" => match s(0).as_deref() {
                Some("primeiras") => Some(format!("Table.Skip({p}, {})", m_num(n(1, 1.0)))),
                Some("ultimas") => Some(format!("Table.RemoveLastN({p}, {})", m_num(n(1, 1.0)))),
                Some("manter_ultimas") => Some(format!("Table.LastN({p}, {})", m_num(n(1, 1.0)))),
                Some("vazias") => Some(format!("Table.SelectRows({p}, each not List.IsEmpty(List.RemoveMatchingItems(Record.FieldValues(_), {{\"\", null}})))")),
                _ => None,
            },
            "transpor" => Some(format!("Table.Transpose({p})")),
            "inverterLinhas" => Some(format!("Table.ReverseRows({p})")),
            "colunaPersonalizada" => {
                let formula = s(1).unwrap_or_default();
                de_formula(&formula).map(|e| format!("Table.AddColumn({p}, {}, each {})", m_txt(&s(0).unwrap_or_default()), m(&e)))
            }
            "extrairTexto" => {
                let campo = || m(&self.campo(&arg(0)));
                let como = s(1).unwrap_or_default();
                let corpo = |alvo: &str| match como.as_str() {
                    "antes" => Some(format!("Text.BeforeDelimiter({alvo}, {})", m_lit(&arg(2)))),
                    "depois" => Some(format!("Text.AfterDelimiter({alvo}, {})", m_lit(&arg(2)))),
                    "entre" => Some(format!("Text.BetweenDelimiters({alvo}, {}, {})", m_lit(&arg(2)), m_lit(&arg(3)))),
                    "primeiros" => Some(format!("Text.Start({alvo}, {})", m_num(n(4, 1.0)))),
                    "ultimos" => Some(format!("Text.End({alvo}, {})", m_num(n(4, 1.0)))),
                    "tamanho" => Some(format!("Text.Length({alvo})")),
                    _ => None,
                };
                match s(5) {
                    Some(nome) => corpo(&campo()).map(|c| format!("Table.AddColumn({p}, {}, each {c})", m_txt(&nome))),
                    None => corpo("_").map(|c| format!("Table.TransformColumns({p}, {{{{{}, each {c}}}}})", self.col(&arg(0)))),
                }
            }
            "duplicarColuna" => Some(format!("Table.DuplicateColumn({p}, {}, {})", self.col(&arg(0)),
                m_txt(&s(1).unwrap_or_else(|| format!("{} - Cópia", self.nome_col(&arg(0))))))),
            _ => None,
        };
        match expr {
            Some(e) => self.por(&nome, e),
            None => self.sem_equivalente(comando),
        }
    }

    fn texto(&self) -> String {
        let mut t = String::from("let\n");
        for (i, (nome, expr, comentarios)) in self.etapas.iter().enumerate() {
            for c in comentarios { t.push_str(&format!("    // {c}\n")); }
            let fim = if i + 1 < self.etapas.len() { "," } else { "" };
            t.push_str(&format!("    {} = {expr}{fim}\n", m_id(nome)));
        }
        for c in &self.comentarios { t.push_str(&format!("    // {c}\n")); }
        t.push_str(&format!("in\n    {}", self.anterior()));
        t
    }
}

/// O M das etapas do script, para o Editor Avançado do Excel. Devolve também quantas
/// etapas ficaram sem equivalente (comentadas no texto).
pub(super) fn para_m(codigo: &str, manual: Option<&Tabela>, fontes: &HashMap<String, Tabela>) -> R<(String, usize)> {
    let mut lista = script::comandos(codigo).into_iter().map(|(_, c)| c);
    let primeiro = lista.next().ok_or_else(|| tr!("O script está vazio.").to_owned())?;
    let (var, chamada) = ["let ", "const ", "var "].iter().find_map(|p| primeiro.strip_prefix(p))
        .and_then(|r| r.split_once('='))
        .map(|(v, c)| (v.trim().to_owned(), c.trim().to_owned()))
        .ok_or_else(|| tr!("Linha 1: o script começa com let fonte = …;").to_owned())?;
    let separar = |c: &str| -> Option<(String, Vec<Json>)> {
        let (nome, resto) = c.split_once('(')?;
        let args = resto.trim_end().strip_suffix(')')?;
        Some((nome.trim().to_owned(), serde_json::from_str(&format!("[{args}]")).ok()?))
    };
    let (leitor, args_fonte) = separar(&chamada).ok_or_else(|| tr!("Linha 1: o script começa com let fonte = …;").to_owned())?;
    let comandos: Vec<String> = lista.collect();
    let navegacao = comandos.iter().find_map(|c| {
        let (m, a) = separar(c.strip_prefix(&format!("{var}."))?)?;
        let item = a.first()?.as_str()?.to_owned();
        matches!(m.as_str(), "planilha" | "tabela").then(|| (if m == "tabela" { "tabela" } else { "planilha" }, item))
    });
    let mut esc = Escritor { etapas: Vec::new(), comentarios: Vec::new(), fontes, sem_equivalente: 0 };
    esc.fonte(&leitor, &args_fonte, navegacao, manual);
    for comando in &comandos {
        if comando == "executar()" || *comando == format!("{var}.executar()") { continue; }
        let Some((metodo, args)) = comando.strip_prefix(&format!("{var}.")).and_then(separar) else {
            esc.sem_equivalente(comando);
            continue;
        };
        if matches!(metodo.as_str(), "planilha" | "tabela" | "navegar") { continue; }
        esc.etapa(&metodo, &args, comando);
    }
    Ok((esc.texto(), esc.sem_equivalente))
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::consulta::query::Passo;

    fn tx(s: &str) -> Value { Value::Text(s.into()) }

    fn vendas() -> Tabela {
        Tabela {
            cabecalho: vec!["Data".into(), "UF".into(), "Produto".into(), "Valor".into(), "Qtd".into()],
            linhas: vec![
                vec![Value::Number(46000.0), tx("SP"), tx("Caneta Azul"), Value::Number(10.0), Value::Number(2.0)],
                vec![Value::Number(46031.0), tx("RJ"), tx("Lápis"), Value::Number(250.0), Value::Number(1.0)],
                vec![Value::Number(46062.0), tx("SP"), tx("Caderno"), Value::Number(120.0), Value::Number(3.0)],
                vec![Value::Number(46093.0), tx("MG"), tx("Caneta Preta"), Value::Number(300.0), Value::Number(5.0)],
            ],
        }
    }

    fn rodar(script: &str) -> Tabela {
        let passos = script::ler(script, &vendas(), &HashMap::new()).unwrap_or_else(|e| panic!("{e}\n{script}"));
        script::gerar("Planilha1", &vendas(), &passos, &HashMap::new()).unwrap().1
    }

    /// O M que o Excel em português gera, como vem do Editor Avançado.
    const M_EXCEL: &str = r#"let
    Fonte = Excel.Workbook(File.Contents("C:\Users\ana\Vendas.xlsx"), null, true),
    Dados_Sheet = Fonte{[Item="Dados",Kind="Sheet"]}[Data],
    #"Cabeçalhos Promovidos" = Table.PromoteHeaders(Dados_Sheet, [PromoteAllScalars=true]),
    #"Tipo Alterado" = Table.TransformColumnTypes(#"Cabeçalhos Promovidos",{{"Data", type date}, {"UF", type text}, {"Valor", Currency.Type}, {"Qtd", Int64.Type}}),
    #"Linhas Filtradas" = Table.SelectRows(#"Tipo Alterado", each ([UF] <> "RJ") and [Valor] > 50),
    #"Personalização Adicionada" = Table.AddColumn(#"Linhas Filtradas", "Total", each [Valor] * [Qtd]),
    #"Ano Inserido" = Table.AddColumn(#"Personalização Adicionada", "Ano", each Date.Year([Data]), Int64.Type),
    #"Texto em Maiúscula" = Table.TransformColumns(#"Ano Inserido",{{"Produto", Text.Upper, type text}}),
    #"Linhas Agrupadas" = Table.Group(#"Texto em Maiúscula", {"UF"}, {{"Soma", each List.Sum([Total]), type number}, {"Linhas", each Table.RowCount(_), Int64.Type}}),
    #"Linhas Classificadas" = Table.Sort(#"Linhas Agrupadas",{{"Soma", Order.Descending}}),
    #"Algo Novo" = Table.Profile(#"Linhas Classificadas")
in
    #"Algo Novo""#;

    #[test]
    fn m_do_excel_vira_script_e_roda() {
        assert!(parece_m(M_EXCEL));
        let d = de_m(M_EXCEL).unwrap();
        assert_eq!(d.fonte.as_deref(), Some(r#"lerArquivo("C:\\Users\\ana\\Vendas.xlsx")"#));
        assert_eq!(d.navegacao.as_deref(), Some(r#"fonte.planilha("Dados");"#));
        assert_eq!(d.nao_convertidas, 1, "{:#?}", d.linhas);
        assert!(d.linhas.iter().any(|l| l.contains("Table.Profile")), "o que não converte fica comentado");
        let script = d.script("let fonte = lerAba(\"Planilha1\");", false);
        assert!(!parece_m(&script));
        let t = rodar(&script);
        assert_eq!(t.cabecalho, vec!["UF".to_string(), "Soma".into(), "Linhas".into()]);
        // RJ e o SP de 10 ficam fora do filtro: MG 300×5 = 1500 primeiro (desc), depois SP 120×3.
        assert_eq!(t.linhas.len(), 2);
        assert_eq!(t.linhas[0][0], tx("MG"));
        assert_eq!(t.linhas[0][1], Value::Number(1500.0));
        assert_eq!(t.linhas[1][1], Value::Number(360.0));
        assert_eq!(t.linhas[1][2], Value::Number(1.0));
    }

    #[test]
    fn script_vai_para_m_e_volta_com_o_mesmo_resultado() {
        let script = r#"let fonte = lerAba("Planilha1");
fonte.filtrar("UF", "<>RJ");
fonte.filtrarVarias([["Valor", ">=100"], ["Produto", "Caneta*"]], true);
fonte.filtrarValores("UF", ["SP", "MG"]);
fonte.renomearColuna("Qtd", "Quantidade");
fonte.mudarTipo("Valor", "decimal", true);
fonte.colunaPersonalizada("Total", "=SE([Valor]>100;[Valor]*[Quantidade];0)");
fonte.extrairTexto("Produto", "primeiros", null, null, 3, "Sigla");
fonte.formatarTexto("Produto", "maiusculas");
fonte.formatarTexto("UF", "prefixo", "UF-");
fonte.colunaCondicional("Faixa", "Valor", ">", "200", "alto", "baixo");
fonte.adicionarIndice(1, 1, "N");
fonte.duplicarColuna("UF", "UF2");
fonte.substituirValores("UF2", "UF-SP", "São Paulo");
fonte.agruparPor(["UF"], [["Total", "soma", "Soma"], ["Valor", "maximo", "Maior"], ["Valor", "contagem", "Linhas"]]);
fonte.ordenar([["Soma", "desc"]]);
fonte.manterPrimeiras(5);
"#;
        let esperado = rodar(script);
        let (texto_m, sem) = para_m(script, None, &HashMap::new()).unwrap();
        assert_eq!(sem, 0, "{texto_m}");
        assert!(texto_m.contains("\n    Fonte = Excel.CurrentWorkbook()"), "{texto_m}");
        assert!(texto_m.contains("List.Count(List.Select([Valor], each _ is number))"), "{texto_m}");
        assert!(texto_m.contains("Table.Group("), "{texto_m}");
        let d = de_m(&texto_m).unwrap();
        assert_eq!(d.nao_convertidas, 0, "{texto_m}\n{:#?}", d.linhas);
        let volta = rodar(&d.script("let fonte = lerAba(\"Planilha1\");", false));
        assert_eq!(volta.cabecalho, esperado.cabecalho, "{texto_m}");
        assert_eq!(volta.linhas, esperado.linhas, "{texto_m}");
    }

    #[test]
    fn colunas_removidas_e_despivotar_viram_etapas_pelo_cabecalho_do_ponto() {
        let m = r#"let
    Fonte = Excel.CurrentWorkbook(){[Name="Vendas"]}[Content],
    #"Colunas Removidas" = Table.RemoveColumns(Fonte,{"Data", "Produto"}),
    #"Colunas Não Dinâmicas" = Table.Unpivot(#"Colunas Removidas", {"Valor", "Qtd"}, "Atributo", "Valor"),
    #"Coluna Dinâmica" = Table.Pivot(#"Colunas Não Dinâmicas", List.Distinct(#"Colunas Não Dinâmicas"[Atributo]), "Atributo", "Valor", List.Sum)
in
    #"Coluna Dinâmica""#;
        let d = de_m(m).unwrap();
        assert_eq!(d.fonte.as_deref(), Some(r#"lerTabela("Vendas")"#));
        assert_eq!(d.nao_convertidas, 0, "{:#?}", d.linhas);
        let passos = script::ler(&d.script("let fonte = lerAba(\"Planilha1\");", false), &vendas(), &HashMap::new()).unwrap();
        assert!(matches!(&passos[0], Passo::Colunas { manter } if manter == &vec![1, 3, 4]));
        assert!(matches!(&passos[1], Passo::Despivotar { manter, .. } if manter == &vec![0]));
        assert!(matches!(&passos[2], Passo::Pivotar { linhas, colunas, .. } if linhas == &vec![0] && colunas == &vec![1]));
    }

    #[test]
    fn juntar_por_nome_da_outra_consulta_e_expandir_some() {
        let m = r#"let
    Fonte = Excel.CurrentWorkbook(){[Name="Vendas"]}[Content],
    #"Consultas Mescladas" = Table.NestedJoin(Fonte, {"UF"}, Estados, {"Sigla"}, "Estados", JoinKind.Inner),
    #"Tipo Alterado" = Table.TransformColumnTypes(#"Consultas Mescladas", {{"UF", type text}}),
    #"Estados Expandido" = Table.ExpandTableColumn(#"Tipo Alterado", "Estados", {"Nome"}, {"Estados.Nome"}),
    #"Consulta Acrescentada" = Table.Combine({#"Estados Expandido", Outras})
in
    #"Consulta Acrescentada""#;
        let d = de_m(m).unwrap();
        assert_eq!(d.nao_convertidas, 0, "{:#?}", d.linhas);
        let script = d.script("let fonte = lerAba(\"Planilha1\");", false);
        assert!(script.contains(r#"fonte.mesclarConsultas("Consulta: Estados", "UF", "Sigla", "interna");"#), "{script}");
        assert!(script.contains(r#"fonte.acrescentarConsultas("Consulta: Outras");"#), "{script}");
        let mut fontes = HashMap::new();
        fontes.insert("Consulta: Estados".to_owned(), Tabela {
            cabecalho: vec!["Nome".into(), "Sigla".into()],
            linhas: vec![vec![tx("São Paulo"), tx("SP")]],
        });
        let passos = script::ler(&script.replace("fonte.acrescentarConsultas(\"Consulta: Outras\");", ""), &vendas(), &fontes).unwrap();
        assert!(matches!(&passos[0], Passo::Juntar { direita: IndiceColuna::Single(1), .. }), "{passos:?}");
        let (texto_m, _) = para_m(&script, None, &fontes).unwrap();
        assert!(texto_m.contains(r#"Table.NestedJoin(Fonte, {"UF"}, Estados, {"Sigla"}, "Estados", JoinKind.Inner)"#), "{texto_m}");
        assert!(texto_m.contains(r#"Table.ExpandTableColumn(#"Consultas Mescladas", "Estados", {"Nome", "Sigla"}"#), "{texto_m}");
    }

    #[test]
    fn lexico_e_m_escrito_a_mao() {
        let m = "let\r\n  /* comentário */ src = Csv.Document(File.Contents(\"a \"\"b\"\".csv\"),[Delimiter=\";\"]),\n  // outro\n  x = Table.SelectRows(src, each Text.Contains([Nome do Cliente], \"#(lf)\")),\n  y = Table.FillDown(x, {\"UF\"})\nin y";
        let d = de_m(m).unwrap();
        assert_eq!(d.fonte.as_deref(), Some(r#"lerCsv("a \"b\".csv")"#));
        assert_eq!(d.linhas, vec![
            r#"fonte.filtrar("Nome do Cliente", "*\n*");"#.to_owned(),
            r#"fonte.preencher("UF", false);"#.to_owned(),
        ]);
        assert!(de_m("let x = in y").is_ok_and(|d| d.nao_convertidas == 1));
        // Constante usada no filtro, e uma etapa do meio que parece fonte não troca a fonte.
        let d = de_m("let\n F = Excel.CurrentWorkbook(){[Name=\"T\"]}[Content],\n col = \"Status\",\n x = Table.SelectRows(F, each [Critério] <> col),\n y = Json.Document(Web.Contents(\"https://a\"))\nin y").unwrap();
        assert_eq!(d.fonte.as_deref(), Some(r#"lerTabela("T")"#));
        assert_eq!(d.linhas[0], r#"fonte.filtrar("Critério", "<>Status");"#);
        assert_eq!(d.nao_convertidas, 1);
        assert_eq!(de_m("let F = Json.Document(Web.Contents(\"https://a\")) in F").unwrap().fonte.as_deref(), Some(r#"lerWeb("https://a")"#));
        assert!(de_m("let x = \"sem fim").is_err());
    }

    #[test]
    fn formula_vai_e_volta() {
        for f in ["=([Valor]*[Qtd])", "=SE(([Valor]>100);MAIÚSCULA([UF]);\"x\")", "=EXT.TEXTO([Produto];2;3)", "=(([Valor]*1,5)&\" R$\")"] {
            let e = de_formula(f).unwrap_or_else(|| panic!("{f}"));
            let volta = format!("={}", formula(&e).unwrap());
            let e2 = de_formula(&volta).unwrap();
            assert_eq!(m(&e), m(&e2), "{f} → {volta}");
        }
        assert!(de_formula("=PROCV([A];X;2)").is_none(), "função sem equivalente não vira M errado");
        let e = de_formula("=MÍNIMO([A];[B])").unwrap();
        assert_eq!(m(&e), "List.Min({[A], [B]})");
        assert_eq!(formula(&e).as_deref(), Some("MÍNIMO([A];[B])"));
    }

    /// `FEATHERSHEET_PQ_XLSX=arquivo.xlsx cargo test consultas_de_um_xlsx_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn consultas_de_um_xlsx_real() {
        let caminho = std::env::var("FEATHERSHEET_PQ_XLSX").expect("FEATHERSHEET_PQ_XLSX");
        let secao = feathersheet_io::power_query::secao_do_arquivo(std::path::Path::new(&caminho)).unwrap().expect("sem Power Query");
        for (nome, m) in consultas_da_secao(&secao).unwrap() {
            let d = de_m(&m).unwrap();
            println!("== {nome}: {} linha(s), {} sem equivalente; fonte {:?} {:?}", d.linhas.len(), d.nao_convertidas, d.fonte, d.navegacao);
            for l in &d.linhas { println!("   {l}"); }
        }
    }

    #[test]
    fn secao_do_xlsx_lista_as_consultas() {
        let s = "section Section1;\r\n\r\nshared Vendas = let\n    Fonte = 1\nin\n    Fonte;\r\n\r\nshared #\"Minha Consulta\" = Excel.CurrentWorkbook(){[Name=\"T\"]}[Content];";
        let c = consultas_da_secao(s).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].0, "Vendas");
        assert!(c[0].1.starts_with("let") && c[0].1.ends_with("Fonte"));
        assert_eq!(c[1].0, "Minha Consulta");
    }
}
