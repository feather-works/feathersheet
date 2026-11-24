//! `ListObjects` (as tabelas do Excel), achadas em 4 dos 54 `.xlsm` da bateria (#143). Cada parte vira um intervalo
//! cujo endereço sai na hora de rodar de `aba.tableRange(nome, parte)` (API das macros), então o resto
//! (`.Rows.Count`, `.Resize`, atribuir valor, `.Copy`) é o que a conversão de intervalos já faz.
// shortcut: `ListRows.Add`, `ListColumns.Add`, `.Resize` da tabela e `.ShowTotals` (mudar a tabela) não convertem;
// só ler as partes e escrever nas células delas.

use super::contexto::Gerador;
use super::tipos::{Faixa, Motivo, Obj, R, T, V};
use crate::sintaxe::Arg;

impl Gerador<'_> {
    /// `ws.ListObjects("Vendas")` / `ws.ListObjects(1)`.
    pub fn tabela_do_argumento(&mut self, aba: &str, args: &[Arg]) -> R<Obj> {
        let quem = self.arg_js(args, &["index"], 0)?;
        Ok(Obj::Tabela(aba.into(), quem))
    }

    /// O intervalo de uma parte da tabela (`range`, `data`, `header`, `totals`, `col:N`, `datacol:N`).
    fn parte_da_tabela(aba: &str, quem: &str, parte: &str) -> Obj {
        Obj::Faixa(Faixa::dinamica(aba, format!("{aba}.tableRange({quem}, {parte})")))
    }

    pub fn membro_da_tabela(&mut self, o: Obj, n: &str, args: &[Arg]) -> R<Obj> {
        Ok(match (o, n) {
            (Obj::Tabela(aba, quem), "range") => Self::parte_da_tabela(&aba, &quem, "'range'"),
            (Obj::Tabela(aba, quem), "databodyrange") => Self::parte_da_tabela(&aba, &quem, "'data'"),
            (Obj::Tabela(aba, quem), "headerrowrange") => Self::parte_da_tabela(&aba, &quem, "'header'"),
            (Obj::Tabela(aba, quem), "totalsrowrange") => Self::parte_da_tabela(&aba, &quem, "'totals'"),
            (Obj::Tabela(aba, quem), "listrows") if args.is_empty() => Obj::LinhasTabela(aba, quem),
            (Obj::Tabela(aba, quem), "listcolumns") if !args.is_empty() => {
                let coluna = self.arg_js(args, &["index"], 0)?;
                Obj::ColunaTabela(aba, quem, coluna)
            }
            (Obj::ColunaTabela(aba, quem, c), "range") => Self::parte_da_tabela(&aba, &quem, &format!("'col:' + {c}")),
            (Obj::ColunaTabela(aba, quem, c), "databodyrange") => Self::parte_da_tabela(&aba, &quem, &format!("'datacol:' + {c}")),
            (_, n) => return Err(Motivo::novo(format!("ListObject.{n}"))),
        })
    }

    pub fn propriedade_da_tabela(&mut self, o: Obj, n: &str, args: &[Arg]) -> R<V> {
        match (&o, n) {
            (Obj::LinhasTabela(aba, quem), "count") => Ok(V::novo(format!("{aba}.tableRange({quem}, 'rows')"), T::Num)),
            (Obj::Tabela(_, quem), "name") => Ok(V::novo(quem.clone(), T::Texto)),
            _ => {
                let parte = self.membro_da_tabela(o, n, args)?;
                self.valor_do_objeto(parte)
            }
        }
    }
}
