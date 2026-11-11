//! `Collection` e `Scripting.Dictionary`: viram objetos JS do prelúdio (`vba.colecao()`,
//! `vba.dicionario()`) com os mesmos métodos em minúsculas (`add`, `count`, `item`…).
//! A variável fica com o tipo [`Tipo::Colecao`].

use super::contexto::Gerador;
use super::tipos::{Motivo, Tipo, R, T, V};
use crate::sintaxe::{Arg, Expr};

impl Gerador<'_> {
    /// `New Collection`, `New Dictionary`, `CreateObject("Scripting.Dictionary")`: o JS que cria.
    pub fn criar_colecao(&mut self, e: &Expr) -> Option<String> {
        let tipo = match e {
            Expr::Novo(c) => c.to_lowercase(),
            Expr::Chamada(f, args) => {
                let create = match &**f {
                    Expr::Nome(n) => n.eq_ignore_ascii_case("createobject"),
                    Expr::Membro(b, n) => n.eq_ignore_ascii_case("createobject") && matches!(&**b, Expr::Nome(v) if v.eq_ignore_ascii_case("vba")),
                    _ => false,
                };
                match args.first().and_then(|a| a.valor.as_ref()) {
                    Some(Expr::Texto(t)) if create => t.to_lowercase(),
                    _ => return None,
                }
            }
            _ => return None,
        };
        match tipo.as_str() {
            "collection" | "vba.collection" => Some(format!("vba.{}()", self.ajuda("colecao"))),
            "dictionary" | "scripting.dictionary" => Some(format!("vba.{}()", self.ajuda("dicionario"))),
            _ => None,
        }
    }

    /// O nome JS da variável, se `e` é uma variável que guarda coleção ou dicionário.
    pub fn var_colecao(&mut self, e: &Expr) -> Option<String> {
        let Expr::Nome(n) = e else { return None };
        self.var(n).or_else(|| self.global(n)).filter(|v| v.tipo == Tipo::Colecao).map(|v| v.js)
    }

    /// `c.Count`, `c.Item(k)`, `d.Exists(k)`, `d.Keys`… como valor.
    pub fn membro_colecao(&mut self, js: &str, nome: &str, args: &[Arg]) -> R<V> {
        let vs = self.valores(args)?;
        let lista = vs.iter().map(|v| v.js.as_str()).collect::<Vec<_>>().join(", ");
        let n = nome.to_lowercase();
        let t = match n.as_str() {
            "count" | "comparemode" => T::Num,
            "exists" => T::Bool,
            "item" | "keys" | "items" => T::Var,
            "add" | "remove" | "removeall" => T::Var,
            _ => return Err(Motivo::novo(format!("Collection/Dictionary.{nome}"))),
        };
        if n == "comparemode" { return Ok(V::novo(format!("{js}.compareMode"), t)); }
        Ok(V::novo(format!("{js}.{n}({lista})"), t))
    }

    /// `c.Add x`, `d.Remove k`, `d.RemoveAll` como instrução.
    pub fn chamar_colecao(&mut self, js: &str, nome: &str, args: &[Arg]) -> R<String> {
        Ok(format!("{};", self.membro_colecao(js, nome, args)?.js))
    }

    /// `d(k) = v` / `d.Item(k) = v`.
    pub fn atribuir_colecao(&mut self, js: &str, args: &[Arg], valor: &Expr) -> R<String> {
        let k = self.valores(args)?.into_iter().next().ok_or_else(|| Motivo::novo(feathersheet_core::tr!("item da coleção sem chave")))?;
        let v = self.valor(valor)?;
        Ok(format!("{js}.set({}, {});", k.js, v.js))
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn colecao_e_dicionario_convertem_sem_pendencia() {
        let vba = "Sub T()\n\
            Dim c As New Collection, d As Object, k As Variant, n As Long\n\
            c.Add 3\n\
            c.Add 5, \"cinco\"\n\
            n = c(c.Count) + c.Item(\"cinco\")\n\
            For Each k In c\n n = n + k\n Next k\n\
            c.Remove 1\n\
            Set d = CreateObject(\"Scripting.Dictionary\")\n\
            d.Add \"a\", 1\n\
            d(\"b\") = 2\n\
            If d.Exists(\"b\") Then n = n + d(\"a\") + d.Count\n\
            For Each k In d.Keys\n n = n + Len(k)\n Next k\n\
            End Sub\n";
        let c = crate::converter_texto(&[("Modulo1", vba)], &[]);
        assert!(c.pendencias.iter().all(|p| p.aviso), "{:#?}", c.pendencias);
        let js = &c.macros[0].codigo;
        assert!(js.contains("vba.colecao()") && js.contains("vba.dicionario()"), "{js}");
    }
}
