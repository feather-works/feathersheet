//! Objeto que chega de uma função do próprio VBA (`Set json = JsonConverter.ParseJson(texto)`), sem tipo
//! conhecido na conversão: o acesso fica para a hora de rodar. `x(k)` vira `vba.item(x, k)` (coleção,
//! dicionário ou matriz) e `x.Count`/`x.Exists(k)` viram `vba.membro(x, "count", …)`. Era a maior fatia do que
//! não convertia nos `.xlsm` que usam o VBA-JSON (bateria #143).
// shortcut: o membro dinâmico só acha o que é do prelúdio (coleção, dicionário) ou propriedade simples do objeto
// JS; método de objeto de classe guardado num `Variant` não é achado (a classe só é conhecida pelo tipo).

use super::contexto::Gerador;
use super::tipos::{Tipo, R, T, V};
use crate::sintaxe::{Arg, Expr};

impl Gerador<'_> {
    /// O JS de uma expressão que é objeto dinâmico (variável que recebeu objeto de função do VBA, ou
    /// item/membro de uma), se for.
    pub fn dinamico(&mut self, e: &Expr) -> Option<String> {
        match e {
            Expr::Nome(n) => {
                let v = self.var(n).or_else(|| self.global(n))?;
                (!v.matriz && v.tipo == Tipo::Valor(T::Obj)).then_some(v.js)
            }
            Expr::Chamada(f, args) => {
                let base = self.dinamico(f)?;
                self.item_dinamico(&base, args).ok().map(|v| v.js)
            }
            Expr::Membro(b, n) => {
                let base = self.dinamico(b)?;
                self.membro_dinamico(&base, n, &[]).ok().map(|v| v.js)
            }
            _ => None,
        }
    }

    /// `x(k1, k2)`: o item (de coleção/dicionário pela chave, de matriz pelo índice).
    pub fn item_dinamico(&mut self, base: &str, args: &[Arg]) -> R<V> {
        let a = self.ajuda("item");
        let mut js = base.to_string();
        for v in self.valores(args)? {
            js = format!("vba.{a}({js}, {})", v.js);
        }
        Ok(V::novo(js, T::Var))
    }

    /// `x.Count`, `x.Exists(k)`, `x.Item(k)`, `x.Keys`: o membro na hora de rodar.
    pub fn membro_dinamico(&mut self, base: &str, nome: &str, args: &[Arg]) -> R<V> {
        if nome.eq_ignore_ascii_case("item") {
            return self.item_dinamico(base, args);
        }
        let a = self.ajuda("membro");
        let vs = self.valores(args)?;
        let resto: String = vs.iter().map(|v| format!(", {}", v.js)).collect();
        let t = match nome.to_lowercase().as_str() { "count" => T::Num, "exists" => T::Bool, _ => T::Var };
        Ok(V::novo(format!("vba.{a}({base}, {:?}{resto})", nome.to_lowercase()), t))
    }

    /// `Set x = Funcao(…)` / `Set x = Modulo.Funcao(…)` com uma função do próprio projeto (que não devolve classe).
    pub fn e_chamada_de_funcao_do_projeto(&self, e: &Expr) -> bool {
        match e {
            Expr::Paren(x) => self.e_chamada_de_funcao_do_projeto(x),
            Expr::Nome(n) => self.var(n).is_none() && self.p.achar(self.modulo, n).is_some(),
            Expr::Chamada(f, _) => match &**f {
                Expr::Nome(n) => self.var(n).is_none() && self.p.achar(self.modulo, n).is_some(),
                Expr::Membro(b, n) => matches!(&**b, Expr::Nome(m) if self.p.achar_qualificado(m, n).is_some()),
                _ => false,
            },
            Expr::Membro(b, n) => matches!(&**b, Expr::Nome(m) if self.p.achar_qualificado(m, n).is_some()),
            _ => false,
        }
    }
}
