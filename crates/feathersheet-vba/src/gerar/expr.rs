//! Expressões: literais, nomes, chamadas e operadores. Com tipo conhecido sai o operador do
//! JavaScript (`a + b`, `a === b`); com Variant, a função `vba.*` que faz como o VBA.

use super::contexto::{Gerador, RefProc};
use super::funcoes::{constante, num, texto};
use super::tipos::{Motivo, Obj, Tipo, R, T, V};
use crate::sintaxe::{Arg, Expr, Op, TipoProc};

impl Gerador<'_> {
    /// Uma expressão como valor JS.
    pub fn valor(&mut self, e: &Expr) -> R<V> {
        match e {
            Expr::Num(n) => Ok(V::novo(num(*n), T::Num)),
            Expr::Texto(s) => Ok(V::novo(texto(s), T::Texto)),
            Expr::Data(d) => Ok(V::novo(format!("vba.{}({})", self.ajuda("data"), texto(d)), T::Var)),
            Expr::Bool(b) => Ok(V::novo(b.to_string(), T::Bool)),
            Expr::Nada | Expr::Nulo => Ok(V::novo("null", T::Obj)),
            Expr::Vazio => Ok(V::novo("undefined", T::Var)),
            // O parêntese do VBA some: a precedência decide se o JS precisa de um.
            Expr::Paren(x) => self.valor(x),
            Expr::Nome(n) => self.nome(n),
            Expr::Bin(op, a, b) => self.binario(*op, a, b),
            Expr::Neg(x) => {
                let v = self.valor(x)?;
                Ok(V::op(format!("-{}", if v.t == T::Num { v.em(15, true) } else { self.como_num(&v) }), T::Num, 15))
            }
            Expr::Nao(x) => {
                let v = self.valor(x)?;
                Ok(match v.t {
                    T::Num => V::op(format!("~{}", v.em(15, true)), T::Num, 15),
                    T::Bool => V::op(format!("!{}", v.em(15, true)), T::Bool, 15),
                    _ => V::op(format!("!{}", self.como_bool(&v)), T::Bool, 15),
                })
            }
            Expr::Novo(c) => Err(Motivo::novo(format!("New {c}"))),
            Expr::TipoDe(..) => Err(Motivo::novo("TypeOf")),
            Expr::Bang(..) => Err(Motivo::novo("acesso por nome (a!b)")),
            Expr::Membro(..) | Expr::MembroDoCom(_) | Expr::Chamada(..) => self.composto(e),
        }
    }

    /// Converte vários argumentos (só os posicionais; nomeados vão por `argumentos_de`).
    pub fn valores(&mut self, args: &[Arg]) -> R<Vec<V>> {
        args.iter().map(|a| a.valor.as_ref().map_or_else(|| Ok(V::novo("undefined", T::Var)), |e| self.valor(e))).collect()
    }

    fn nome(&mut self, n: &str) -> R<V> {
        let chave = n.to_lowercase();
        if self.escopo.funcao.as_deref() == Some(chave.as_str()) {
            // Dentro da função, o nome dela é a variável do retorno.
            let t = self.tipo_do_retorno();
            return Ok(V::novo("$r", t));
        }
        if let Some(v) = self.var(n).or_else(|| self.global(n)) {
            return match &v.tipo {
                Tipo::Valor(t) => Ok(V::novo(v.js, if v.matriz { T::Matriz } else { *t })),
                Tipo::Faixa(_) => {
                    let f = self.faixa_da_var(&v);
                    self.valor_do_objeto(Obj::Faixa(f))
                }
                Tipo::Aba => Err(Motivo::novo(format!("a aba {n} usada como valor"))),
                Tipo::NovaPasta => Err(Motivo::novo(format!("a pasta {n} usada como valor"))),
                Tipo::Impossivel(m) => Err(Motivo::novo(m.clone())),
            };
        }
        if let Some(r) = self.p.achar(self.modulo, n) {
            return self.chamar_proc(r, &[]);
        }
        if let Some(c) = self.controle_do_modulo(n) {
            return Ok(V::novo(format!("sheet.field({c:?}).value"), T::Var));
        }
        if let Some(c) = constante(n) {
            return Ok(c);
        }
        if let Some(v) = self.embutida_sem_args(n) {
            return Ok(v);
        }
        if matches!(chave.as_str(), "selection" | "activecell") {
            let o = self.objeto_por_nome_publico(n)?;
            return self.valor_do_objeto(o);
        }
        if self.p.codenames.contains_key(&chave) || matches!(chave.as_str(), "activesheet" | "application" | "thisworkbook" | "activeworkbook" | "worksheets" | "sheets") {
            return Err(Motivo::novo(format!("{n} usado como valor")));
        }
        // VBA sem `Option Explicit`: nome desconhecido é uma variável que nasce vazia.
        self.escopo.implicitas.insert(chave);
        let js = self.declarar(n, Tipo::Valor(T::Var), false);
        Ok(V::novo(js, T::Var))
    }

    fn objeto_por_nome_publico(&mut self, n: &str) -> R<Obj> {
        self.objeto(&Expr::Nome(n.to_string()))
    }

    fn tipo_do_retorno(&self) -> T {
        self.escopo.vars.get("$r").map_or(T::Var, |v| match v.tipo {
            Tipo::Valor(t) => t,
            _ => T::Obj,
        })
    }

    /// Membro, chamada, índice de matriz.
    fn composto(&mut self, e: &Expr) -> R<V> {
        match e {
            Expr::Chamada(f, args) => self.chamada(f, args),
            Expr::Membro(base, nome) => self.membro(base, nome, &[]),
            Expr::MembroDoCom(nome) => {
                let o = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.propriedade(o, nome, &[])
            }
            _ => unreachable!("composto só recebe membro e chamada"),
        }
    }

    fn chamada(&mut self, f: &Expr, args: &[Arg]) -> R<V> {
        match f {
            Expr::Nome(n) => self.chamada_por_nome(n, args),
            Expr::Membro(base, nome) => self.membro(base, nome, args),
            Expr::MembroDoCom(nome) => {
                let o = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.propriedade(o, nome, args)
            }
            _ => {
                let o = self.objeto(&Expr::Chamada(Box::new(f.clone()), args.to_vec()))?;
                self.valor_do_objeto(o)
            }
        }
    }

    fn chamada_por_nome(&mut self, n: &str, args: &[Arg]) -> R<V> {
        if let Some(v) = self.var(n).or_else(|| self.global(n)) {
            if v.matriz || matches!(v.tipo, Tipo::Valor(T::Var | T::Matriz)) {
                let mut js = v.js.clone();
                for a in self.valores(args)? {
                    js.push_str(&format!("[{}]", if a.t == T::Num { a.js.clone() } else { self.como_num(&a) }));
                }
                return Ok(V::novo(js, T::Var));
            }
        }
        if let Some(r) = self.p.achar(self.modulo, n) {
            return self.chamar_proc(r, args);
        }
        if n.eq_ignore_ascii_case("intersect") || n.eq_ignore_ascii_case("union") {
            return self.interseccao(n, args);
        }
        let vs = self.valores(args)?;
        if let Some(r) = self.embutida(n, &vs) {
            return r;
        }
        if self.p.e_externa(n) {
            return Err(Motivo::novo(format!("API do Windows ({n})")));
        }
        let o = self.objeto(&Expr::Chamada(Box::new(Expr::Nome(n.into())), args.to_vec())).map_err(|_| Motivo::novo(format!("função {n} sem equivalente")))?;
        self.valor_do_objeto(o)
    }

    /// `Intersect(a, b)` → o intervalo comum (ou `null`, que o `Is Nothing` compara).
    fn interseccao(&mut self, n: &str, args: &[Arg]) -> R<V> {
        let mut faixas = Vec::new();
        for a in args.iter().filter_map(|a| a.valor.as_ref()) {
            match self.objeto(a)? {
                Obj::Faixa(f) => faixas.push(f),
                _ => return Err(Motivo::novo(format!("{n} com algo que não é intervalo"))),
            }
        }
        let [a, b] = faixas.as_slice() else { return Err(Motivo::novo(format!("{n} com mais de dois intervalos"))) };
        let metodo = if n.eq_ignore_ascii_case("intersect") { "intersect" } else { "union" };
        Ok(V::novo(format!("{}.{metodo}({}, {})", a.aba, a.end, b.end), T::Obj))
    }

    fn membro(&mut self, base: &Expr, nome: &str, args: &[Arg]) -> R<V> {
        // `VBA.Len(x)`, `VBA.Mid(…)`.
        if matches!(base, Expr::Nome(b) if b.eq_ignore_ascii_case("vba")) {
            let vs = self.valores(args)?;
            return self.embutida(nome, &vs).unwrap_or_else(|| Err(Motivo::novo(format!("VBA.{nome}"))));
        }
        // `Err.Number` / `Err.Description`: o erro guardado pelo `catch`.
        if matches!(base, Expr::Nome(b) if b.eq_ignore_ascii_case("err")) && self.var("err").is_none() {
            self.ajuda("erro");
            return match nome.to_lowercase().as_str() {
                "number" => Ok(V::novo("vba.erroNumero()", T::Num)),
                "description" => Ok(V::novo("vba.erroTexto()", T::Texto)),
                "source" => Ok(V::novo("''", T::Texto)),
                _ => Err(Motivo::novo(format!("Err.{nome}"))),
            };
        }
        // `Módulo1.Foo(x)`, `Sheet8.Inversor(Target)`.
        if let Expr::Nome(m) = base {
            if let Some(r) = self.p.achar_qualificado(m, nome) {
                return self.chamar_proc(r, args);
            }
        }
        let o = self.objeto(base)?;
        self.propriedade(o, nome, args)
    }

    /// Chama um procedimento do próprio VBA (pedindo para ele entrar na macro).
    pub fn chamar_proc(&mut self, r: RefProc, args: &[Arg]) -> R<V> {
        let nome = self.m.pedir(self.p, r);
        let proc = self.p.proc(r);
        let mut saida: Vec<String> = vec!["undefined".into(); proc.params.len().max(args.len())];
        let mut pos = 0;
        for a in args {
            let i = match &a.nome {
                Some(n) => proc.params.iter().position(|p| p.nome.eq_ignore_ascii_case(n)).ok_or_else(|| Motivo::novo(format!("argumento {n} de {}", proc.nome)))?,
                None => {
                    pos += 1;
                    pos - 1
                }
            };
            if let Some(e) = &a.valor {
                saida[i] = self.argumento_de_proc(e)?;
            }
        }
        while saida.last().is_some_and(|s| s == "undefined") {
            saida.pop();
        }
        let t = match (&proc.tipo, proc.retorno.as_deref()) {
            (TipoProc::Sub | TipoProc::PropriedadeEscrever, _) => T::Var,
            (_, Some("string")) => T::Texto,
            (_, Some("boolean")) => T::Bool,
            (_, Some("long" | "integer" | "double" | "single" | "currency" | "byte")) => T::Num,
            _ => T::Var,
        };
        Ok(V::novo(format!("{nome}({})", saida.join(", ")), t))
    }

    /// Argumento de procedimento: intervalo vai como endereço (`Target`), o resto como valor.
    fn argumento_de_proc(&mut self, e: &Expr) -> R<String> {
        if let Expr::Nome(n) = e {
            if let Some(v) = self.var(n) {
                if matches!(v.tipo, Tipo::Faixa(_) | Tipo::Aba) {
                    return Ok(v.js);
                }
            }
        }
        if !matches!(e, Expr::Nome(_)) {
            if let Ok(Obj::Faixa(f)) = self.objeto(e) {
                return Ok(f.end);
            }
        }
        Ok(self.valor(e)?.js)
    }

    // ------------------------------------------------------------------ operadores

    fn binario(&mut self, op: Op, a: &Expr, b: &Expr) -> R<V> {
        if op == Op::Concat {
            return self.concatenar(a, b);
        }
        if op == Op::Is {
            let (va, vb) = (self.valor(a)?, self.valor(b)?);
            return Ok(V::op(format!("{} === {}", va.em(9, false), vb.em(9, true)), T::Bool, 9));
        }
        let (va, vb) = (self.valor(a)?, self.valor(b)?);
        match op {
            Op::Igual | Op::Dif | Op::Menor | Op::Maior | Op::MenorIg | Op::MaiorIg => Ok(self.comparar(op, &va, &vb)),
            Op::E | Op::Ou | Op::Xor => Ok(self.logico(op, &va, &vb)),
            Op::Like => {
                let (x, y) = (self.como_texto(&va), self.como_texto(&vb));
                Ok(V::novo(format!("vba.{}({x}, {y})", self.ajuda("like")), T::Bool))
            }
            Op::Eqv | Op::Imp => Err(Motivo::novo("operador Eqv/Imp")),
            _ => Ok(self.aritmetico(op, &va, &vb)),
        }
    }

    /// `a & b & c` → `` `…${b}…` `` quando tudo é texto/número; senão `vba.cat(a, b, c)`.
    fn concatenar(&mut self, a: &Expr, b: &Expr) -> R<V> {
        let mut partes = Vec::new();
        fn achatar<'e>(e: &'e Expr, v: &mut Vec<&'e Expr>) {
            match e {
                Expr::Bin(Op::Concat, x, y) => {
                    achatar(x, v);
                    achatar(y, v);
                }
                _ => v.push(e),
            }
        }
        achatar(a, &mut partes);
        achatar(b, &mut partes);
        let vs: Vec<(V, Option<&String>)> = partes.iter().map(|e| Ok((self.valor(e)?, if let Expr::Texto(s) = e { Some(s) } else { None }))).collect::<R<_>>()?;
        if vs.iter().all(|(v, _)| matches!(v.t, T::Texto | T::Num)) {
            let mut s = String::from("`");
            for (v, lit) in &vs {
                match lit {
                    Some(t) => s.push_str(&t.replace('\\', "\\\\").replace('`', "\\`").replace("${", "\\${").replace('\n', "\\n").replace('\r', "\\r")),
                    None => s.push_str(&format!("${{{}}}", v.js)),
                }
            }
            s.push('`');
            return Ok(V::novo(s, T::Texto));
        }
        let js: Vec<String> = vs.into_iter().map(|(v, _)| v.js).collect();
        Ok(V::novo(format!("vba.{}({})", self.ajuda("cat"), js.join(", ")), T::Texto))
    }

    /// `a op b` com a regra do VBA para tipos diferentes.
    pub fn comparar(&mut self, op: Op, a: &V, b: &V) -> V {
        let js_op = match op {
            Op::Igual => "===",
            Op::Dif => "!==",
            Op::Menor => "<",
            Op::Maior => ">",
            Op::MenorIg => "<=",
            _ => ">=",
        };
        let mesmo = a.t == b.t && matches!(a.t, T::Num | T::Texto | T::Bool);
        if mesmo {
            let p = if matches!(op, Op::Igual | Op::Dif) { 9 } else { 10 };
            return V::op(format!("{} {js_op} {}", a.em(p, false), b.em(p, true)), T::Bool, p);
        }
        match op {
            Op::Igual => V::novo(format!("vba.{}({}, {})", self.ajuda("eq"), a.js, b.js), T::Bool),
            Op::Dif => V::op(format!("!vba.{}({}, {})", self.ajuda("eq"), a.js, b.js), T::Bool, 15),
            _ => V::op(format!("vba.{}({}, {}) {js_op} 0", self.ajuda("cmp"), a.js, b.js), T::Bool, 10),
        }
    }

    fn logico(&mut self, op: Op, a: &V, b: &V) -> V {
        if a.t == T::Num && b.t == T::Num {
            let (o, p) = match op {
                Op::E => ("&", 8),
                Op::Ou => ("|", 6),
                _ => ("^", 7),
            };
            return V::op(format!("{} {o} {}", a.em(p, false), b.em(p, true)), T::Num, p);
        }
        let (o, p) = match op {
            Op::E => ("&&", 5),
            Op::Ou => ("||", 4),
            _ => ("!==", 9),
        };
        let (x, y) = (self.bool_em(a, p, false), self.bool_em(b, p, true));
        V::op(format!("{x} {o} {y}"), T::Bool, p)
    }

    fn bool_em(&mut self, v: &V, p: u8, direita: bool) -> String {
        if v.t == T::Bool {
            v.em(p, direita)
        } else {
            self.como_bool(v)
        }
    }

    fn aritmetico(&mut self, op: Op, a: &V, b: &V) -> V {
        if op == Op::Soma && !(a.t == T::Num && b.t == T::Num) && !(a.t == T::Texto && b.t == T::Texto) {
            return V::novo(format!("vba.{}({}, {})", self.ajuda("soma"), a.js, b.js), T::Var);
        }
        let ambos_num = a.t == T::Num && b.t == T::Num;
        match op {
            Op::DivInt => return V::novo(format!("vba.{}({}, {})", self.ajuda("divInt"), a.js, b.js), T::Num),
            // O Mod do VBA arredonda os dois lados antes (55,9999 Mod 10 = 6): nunca o `%` cru.
            Op::Mod => return V::novo(format!("vba.{}({}, {})", self.ajuda("mod"), a.js, b.js), T::Num),
            _ => {}
        }
        let (o, p) = match op {
            Op::Soma => ("+", 12),
            Op::Sub => ("-", 12),
            Op::Mul => ("*", 13),
            Op::Div => ("/", 13),
            Op::Mod => ("%", 13),
            _ => ("**", 14),
        };
        // `**` associa à direita: o da esquerda leva parênteses.
        let (x, y) = if ambos_num || op == Op::Soma { (a.em(p, p == 14), b.em(p, p != 14)) } else { (self.num_em(a, p, p == 14), self.num_em(b, p, p != 14)) };
        V::op(format!("{x} {o} {y}"), if a.t == T::Texto && op == Op::Soma { T::Texto } else { T::Num }, p)
    }

    fn num_em(&mut self, v: &V, p: u8, direita: bool) -> String {
        if v.t == T::Num {
            v.em(p, direita)
        } else {
            self.como_num(v)
        }
    }
}
