//! O modelo de objetos do Excel (`Range`, `Cells`, `Worksheets`, `Selection`, `.Font`,
//! `.Interior`, `.Borders`…) na API das macros do FeatherSheet (`sheet`, `wb`, `app`).
//! O intervalo vira um endereço em texto (`"B3"`) mais a aba; o resto são as chamadas da API.

use super::contexto::{js_nome, Gerador};
use super::funcoes::{num, valor_da_constante};
use super::tipos::{Faixa, Inteira, Motivo, Obj, Parte, Tipo, R, T, V};
use crate::sintaxe::{Arg, Expr, Op};

/// Coluna (do 1) → letras.
pub fn letra(mut c: u32) -> String {
    let mut s = Vec::new();
    while c > 0 {
        s.push(char::from(b'A' + ((c - 1) % 26) as u8));
        c = (c - 1) / 26;
    }
    s.iter().rev().collect()
}

/// `"B3"` → (linha, coluna), do 1. Sem `$`.
pub fn celula(end: &str) -> Option<(u32, u32)> {
    let e = end.replace('$', "");
    let i = e.find(|c: char| c.is_ascii_digit())?;
    let (l, n) = e.split_at(i);
    // Mais de 3 letras passa de XFD (`Range("Faixa1")` é nome, não célula) e estouraria a conta.
    if l.is_empty() || l.len() > 3 || !l.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let col = l.to_ascii_uppercase().bytes().fold(0u32, |a, b| a * 26 + u32::from(b - b'A' + 1));
    Some((n.parse().ok()?, col))
}

/// O lado de uma borda (`xlEdgeBottom` → `"bottom"`).
fn lado_da_borda(e: &Expr) -> Option<&'static str> {
    let n = match e {
        Expr::Nome(n) => valor_da_constante(n)?,
        Expr::Num(x) => *x as i64,
        _ => return None,
    };
    Some(match n {
        7 => "left",
        8 => "top",
        9 => "bottom",
        10 => "right",
        11 => "insideV",
        12 => "insideH",
        5 | 6 => "diagonal",
        _ => return None,
    })
}

/// O nome (sem diferenciar maiúsculas) de uma constante na expressão.
fn e_constante(e: &Expr, nome: &str) -> bool {
    matches!(e, Expr::Nome(n) if n.eq_ignore_ascii_case(nome))
}

impl Gerador<'_> {
    /// A aba "ativa" agora.
    pub fn obj_atual(&self) -> Obj {
        Obj::Aba(self.atual.clone(), self.atual_nome.clone())
    }

    /// A aba de `Range`/`Cells`/`Rows`/`Columns`/`UsedRange` sem qualificador: no módulo de
    /// uma aba é sempre a própria aba (mesmo depois de `Outra.Select`); nos demais, a ativa.
    pub fn obj_implicito(&self) -> Obj {
        match self.p.modulos[self.modulo].aba.clone() {
            Some(nome) => Obj::Aba(js_nome(&self.p.modulos[self.modulo].nome), Some(nome)),
            None => self.obj_atual(),
        }
    }

    /// Resolve uma expressão que é objeto do Excel.
    pub fn objeto(&mut self, e: &Expr) -> R<Obj> {
        match e {
            Expr::Nome(n) => self.objeto_por_nome(n),
            Expr::Paren(x) => self.objeto(x),
            Expr::MembroDoCom(nome) => {
                let base = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.membro_objeto(base, nome, &[])
            }
            Expr::Membro(base, nome) => {
                if let Some(r) = self.qualificado(base, nome) {
                    return r;
                }
                let b = self.objeto(base)?;
                self.membro_objeto(b, nome, &[])
            }
            Expr::Chamada(f, args) => self.objeto_chamado(f, args),
            _ => Err(Motivo::novo("não é objeto do Excel")),
        }
    }

    /// `Sheet8.Proc` como objeto não existe: deixa o chamador tratar como procedimento.
    fn qualificado(&mut self, base: &Expr, nome: &str) -> Option<R<Obj>> {
        let Expr::Nome(m) = base else { return None };
        self.p.achar_qualificado(m, nome).map(|_| Err(Motivo::novo("procedimento, não objeto")))
    }

    fn objeto_chamado(&mut self, f: &Expr, args: &[Arg]) -> R<Obj> {
        if let Expr::Nome(n) = f {
            let implicito = self.obj_implicito();
            match n.to_lowercase().as_str() {
                "range" | "cells" | "rows" | "columns" if self.var(n).is_none() => return self.membro_objeto(implicito, n, args),
                "worksheets" | "sheets" => return self.membro_objeto(Obj::Abas, "item", args),
                _ => {}
            }
        }
        // `Selection.Borders(xlEdgeBottom)`, `Worksheets("MAIN")`, `ws.Range("A1")`.
        match f {
            Expr::Membro(base, nome) => {
                if let Some(r) = self.qualificado(base, nome) {
                    return r;
                }
                let b = self.objeto(base)?;
                self.membro_objeto(b, nome, args)
            }
            Expr::MembroDoCom(nome) => {
                let base = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.membro_objeto(base, nome, args)
            }
            _ => {
                let b = self.objeto(f)?;
                self.membro_objeto(b, "item", args)
            }
        }
    }

    fn objeto_por_nome(&mut self, n: &str) -> R<Obj> {
        if let Some(v) = self.var(n).or_else(|| self.global(n)) {
            return match &v.tipo {
                Tipo::Aba => Ok(Obj::Aba(v.js.clone(), None)),
                Tipo::Faixa(_) => Ok(Obj::Faixa(self.faixa_da_var(&v))),
                Tipo::NovaPasta => Ok(Obj::NovaPasta(v.js.clone())),
                Tipo::Impossivel(m) => Err(Motivo::novo(m.clone())),
                Tipo::Valor(_) => Err(Motivo::novo(format!("{n} não é objeto do Excel"))),
            };
        }
        if let Some((var, nome)) = self.aba_por_codename(n) {
            return Ok(Obj::Aba(var, Some(nome)));
        }
        if let Some(f) = self.formulario_por_nome(n) {
            return Ok(Obj::Formulario(f));
        }
        if let Some(c) = self.controle_do_modulo(n) {
            return Ok(Obj::Controle(c));
        }
        let atual = self.atual.clone();
        let implicita = match self.obj_implicito() {
            Obj::Aba(a, _) => a,
            _ => atual.clone(),
        };
        Ok(match n.to_lowercase().as_str() {
            "activesheet" => self.obj_atual(),
            "me" => match self.p.modulos[self.modulo].aba.clone() {
                Some(_) => self.obj_implicito(),
                None => Obj::Pasta,
            },
            "thisworkbook" | "activeworkbook" => Obj::Pasta,
            "application" => Obj::App,
            "selection" => Obj::Faixa(Faixa::dinamica(&atual, format!("{atual}.selection().address"))),
            "activecell" => Obj::Faixa(Faixa { unica: true, ..Faixa::dinamica(&atual, format!("{atual}.activeCell().address")) }),
            "worksheets" | "sheets" => Obj::Abas,
            "worksheetfunction" => Obj::FuncoesDaPlanilha,
            "cells" => Obj::Faixa(Faixa::fixa(&implicita, "A1:XFD1048576")),
            "rows" => Obj::Contagem(implicita, Inteira::Linhas),
            "columns" => Obj::Contagem(implicita, Inteira::Colunas),
            "usedrange" => Obj::Faixa(Faixa::dinamica(&implicita, format!("({implicita}.getDataRange()?.address ?? 'A1')"))),
            "workbooks" => Obj::Pastas,
            "py" => return Err(Motivo::novo("xlwings (Python chamado pelo Excel — a macro do FeatherSheet ainda não roda Python)")),
            "activewindow" | "windows" => Obj::Janela,
            _ if self.p.e_externa(n) => return Err(Motivo::novo("API do Windows (Declare)")),
            _ => return Err(Motivo::novo(format!("objeto {n} desconhecido"))),
        })
    }

    /// `obj.nome(args)` quando o resultado é outro objeto.
    pub fn membro_objeto(&mut self, obj: Obj, nome: &str, args: &[Arg]) -> R<Obj> {
        let n = nome.to_lowercase();
        match obj {
            Obj::App | Obj::Pasta => self.membro_da_pasta(&n, args),
            Obj::Abas => match n.as_str() {
                "item" => self.aba_do_argumento(args),
                "add" => self.aba_nova_da_pasta(args).map(|js| Obj::Aba(js, None)),
                _ => Err(Motivo::novo(format!("Worksheets.{nome}"))),
            },
            Obj::Aba(aba, nome_aba) => self.membro_da_aba(&aba, nome_aba, &n, args),
            Obj::Faixa(f) => self.membro_da_faixa(f, &n, args),
            Obj::Parte(f, Parte::Bordas(None)) if n == "item" && args.len() == 1 => {
                let lado = args[0].valor.as_ref().and_then(lado_da_borda).ok_or_else(|| Motivo::novo("borda com lado calculado"))?;
                Ok(Obj::Parte(f, Parte::Bordas(Some(lado.into()))))
            }
            Obj::Contagem(aba, eixo) if n == "item" && args.len() == 1 => self.linha_ou_coluna(&aba, eixo, &args[0]),
            Obj::Ordem(aba) if matches!(n.as_str(), "sortfields" | "sort") => Ok(Obj::Ordem(aba)),
            Obj::Janela => Ok(Obj::Janela),
            Obj::Pastas => match n.as_str() {
                "add" => Ok(Obj::NovaPasta(format!("vba.{}()", self.ajuda("novaPasta")))),
                "open" => Err(Motivo::novo("abrir outra pasta de trabalho (Workbooks.Open)")),
                _ => Err(Motivo::novo("outra pasta de trabalho aberta (Workbooks(…))")),
            },
            Obj::NovaPasta(p) => match n.as_str() {
                "sheets" | "worksheets" if args.is_empty() => Ok(Obj::AbasNovas(p)),
                "sheets" | "worksheets" => self.aba_nova(p, args),
                _ => Err(Motivo::novo(format!("Workbook.{nome} da pasta nova"))),
            },
            Obj::AbasNovas(p) if n == "item" => self.aba_nova(p, args),
            Obj::AbasNovas(_) | Obj::AbaNova(..) => Err(Motivo::novo(format!("membro {nome} da pasta nova"))),
            Obj::Formulario(f) => match self.p.controle(&f, nome) {
                Some(c) => Ok(Obj::Controle(c.to_string())),
                None => Err(Motivo::novo(format!("{f}.{nome} do formulário"))),
            },
            Obj::Controle(c) => Err(Motivo::novo(format!("{c}.{nome} do controle do formulário"))),
            Obj::Condicoes(f) if n == "add" => self.condicao_nova(&f, args).map(Obj::Condicao),
            Obj::Condicao(c) if n == "interior" => Ok(Obj::EstiloCondicao(c, Parte::Interior)),
            Obj::Condicao(c) if n == "font" => Ok(Obj::EstiloCondicao(c, Parte::Fonte)),
            Obj::Condicoes(_) | Obj::Condicao(_) | Obj::EstiloCondicao(..) => Err(Motivo::novo(format!("FormatConditions: {nome} (só Add, Delete, Interior e Font convertem)"))),
            Obj::Dimensao(..) | Obj::Contagem(..) | Obj::Parte(..) | Obj::FuncoesDaPlanilha | Obj::Nomes | Obj::Ordem(_) | Obj::Validacao(_) => Err(Motivo::novo(format!("membro {nome}"))),
        }
    }

    /// `Worksheets.Add(Before, After, Count, Type)` → `wb.addSheet(null, { after: … })` (a aba nova).
    pub fn aba_nova_da_pasta(&mut self, args: &[Arg]) -> R<String> {
        let achar = |chaves: &[&str], pos: usize| args.iter().find(|a| a.nome.as_deref().is_some_and(|n| chaves.iter().any(|c| n.eq_ignore_ascii_case(c)))).or_else(|| args.get(pos).filter(|a| a.nome.is_none())).and_then(|a| a.valor.clone());
        if achar(&["count"], 2).is_some_and(|c| c != Expr::Num(1.0)) || achar(&["type"], 3).is_some() {
            return Err(Motivo::novo("Worksheets.Add com Count ou Type (uma aba de planilha por vez)"));
        }
        let mut pos = Vec::new();
        for (chave, p) in [("before", 0), ("after", 1)] {
            if let Some(e) = achar(&[chave], p) {
                let aba = match self.objeto(&e)? {
                    Obj::Aba(js, _) => js,
                    _ => return Err(Motivo::novo(format!("Worksheets.Add {chave}:= que não é uma aba"))),
                };
                pos.push(format!("{chave}: {aba}"));
            }
        }
        Ok(format!("wb.addSheet(null, {{ {} }})", pos.join(", ")))
    }

    fn membro_da_pasta(&mut self, n: &str, args: &[Arg]) -> R<Obj> {
        let atual = self.atual.clone();
        Ok(match n {
            "activesheet" => self.obj_atual(),
            "activeworkbook" | "thisworkbook" => Obj::Pasta,
            "selection" => Obj::Faixa(Faixa::dinamica(&atual, format!("{atual}.selection().address"))),
            "activecell" => Obj::Faixa(Faixa { unica: true, ..Faixa::dinamica(&atual, format!("{atual}.activeCell().address")) }),
            "worksheetfunction" => Obj::FuncoesDaPlanilha,
            "worksheets" | "sheets" if args.is_empty() => Obj::Abas,
            "worksheets" | "sheets" => self.aba_do_argumento(args)?,
            "names" => Obj::Nomes,
            "activewindow" | "windows" => Obj::Janela,
            "range" | "cells" => return self.membro_da_aba(&atual, self.atual_nome.clone(), n, args),
            "workbooks" if args.is_empty() => Obj::Pastas,
            "workbooks" => return Err(Motivo::novo("outra pasta de trabalho aberta (Workbooks(…))")),
            _ => return Err(Motivo::novo(format!("Application/Workbook.{n}"))),
        })
    }

    /// `newBook.Sheets(1)`: a aba pela posição (o nome de uma aba da pasta nova não se sabe).
    fn aba_nova(&mut self, p: String, args: &[Arg]) -> R<Obj> {
        let Some(Some(e)) = args.first().map(|a| a.valor.as_ref()) else { return Err(Motivo::novo("Sheets sem argumento")) };
        if matches!(e, Expr::Texto(_)) {
            return Err(Motivo::novo("aba da pasta nova pelo nome"));
        }
        let v = self.valor(e)?;
        Ok(Obj::AbaNova(p, self.como_num(&v)))
    }

    /// `Worksheets("MAIN")`, `Worksheets(2)`.
    fn aba_do_argumento(&mut self, args: &[Arg]) -> R<Obj> {
        let Some(Some(e)) = args.first().map(|a| a.valor.as_ref()) else { return Err(Motivo::novo("Worksheets sem argumento")) };
        if let Expr::Texto(nome) = e {
            let var = self.aba_por_nome(nome);
            return Ok(Obj::Aba(var, Some(nome.clone())));
        }
        let v = self.valor(e)?;
        Ok(Obj::Aba(format!("wb.sheet({})", v.js), None))
    }

    fn membro_da_aba(&mut self, aba: &str, nome_aba: Option<String>, n: &str, args: &[Arg]) -> R<Obj> {
        Ok(match n {
            "range" => Obj::Faixa(self.faixa_de_range(aba, args)?),
            "cells" if args.is_empty() => Obj::Faixa(Faixa::fixa(aba, "A1:XFD1048576")),
            "cells" => Obj::Faixa(self.faixa_de_cells(aba, args)?),
            "rows" if args.is_empty() => Obj::Contagem(aba.into(), Inteira::Linhas),
            "columns" if args.is_empty() => Obj::Contagem(aba.into(), Inteira::Colunas),
            "rows" => self.linha_ou_coluna(aba, Inteira::Linhas, &args[0])?,
            "columns" => self.linha_ou_coluna(aba, Inteira::Colunas, &args[0])?,
            "usedrange" => Obj::Faixa(Faixa::dinamica(aba, format!("({aba}.getDataRange()?.address ?? 'A1')"))),
            "shapes" | "buttons" | "chartobjects" | "pictures" => return Err(Motivo::novo("formas e botões do Excel (use Inserir → Botão de macro)")),
            "sort" => Obj::Ordem(aba.into()),
            "autofilter" => return Err(Motivo::novo("filtro do Excel (AutoFilter — converter à mão)")),
            "pagesetup" => return Err(Motivo::novo("configuração de página")),
            "parent" => Obj::Pasta,
            _ => {
                let _ = nome_aba;
                return Err(Motivo::novo(format!("Worksheet.{n}")));
            }
        })
    }

    /// `Rows(5)`, `Columns("B")`, `Columns(2)`.
    fn linha_ou_coluna(&mut self, aba: &str, eixo: Inteira, a: &Arg) -> R<Obj> {
        let e = a.valor.as_ref().ok_or_else(|| Motivo::novo("Rows/Columns sem argumento"))?;
        let mut f = match (eixo, e) {
            (Inteira::Linhas, Expr::Num(n)) => Faixa::fixa(aba, &format!("{0}:{0}", num(*n))),
            (Inteira::Colunas, Expr::Num(n)) => Faixa::fixa(aba, &format!("{0}:{0}", letra(*n as u32))),
            (_, Expr::Texto(t)) => Faixa::fixa(aba, &if t.contains(':') { t.clone() } else { format!("{t}:{t}") }),
            (Inteira::Linhas, _) => {
                let v = self.valor(e)?;
                Faixa::dinamica(aba, format!("`${{{0}}}:${{{0}}}`", v.js))
            }
            _ => {
                let v = self.valor(e)?;
                let l = self.ajuda("letra");
                Faixa::dinamica(aba, format!("`${{vba.{l}({0})}}:${{vba.{l}({0})}}`", v.js))
            }
        };
        f.inteira = eixo;
        Ok(Obj::Faixa(f))
    }

    /// `Range("A1")`, `Range("A1:B5")`, `Range(Cells(1, 1), Cells(5, 2))`, `Range(x)`.
    fn faixa_de_range(&mut self, aba: &str, args: &[Arg]) -> R<Faixa> {
        let exprs: Vec<&Expr> = args.iter().filter_map(|a| a.valor.as_ref()).collect();
        match exprs.as_slice() {
            [Expr::Texto(t)] => Ok(Faixa::fixa(aba, t)),
            [e] => {
                if let Ok(Obj::Faixa(f)) = self.objeto(e) {
                    return Ok(f);
                }
                let v = self.valor(e)?;
                let end = self.como_texto(&v);
                Ok(Faixa { unica: coluna_e_linha(e), ..Faixa::dinamica(aba, end) })
            }
            [a, b] => {
                let fa = self.ponta(aba, a)?;
                let fb = self.ponta(aba, b)?;
                if let (Some(x), Some(y)) = (&fa.literal, &fb.literal) {
                    if let (Some(p), Some(q)) = (celula(x), celula(y)) {
                        let end = format!("{}{}:{}{}", letra(p.1.min(q.1)), p.0.min(q.0), letra(p.1.max(q.1)), p.0.max(q.0));
                        return Ok(Faixa::fixa(aba, &end));
                    }
                }
                let h = self.ajuda("faixa");
                Ok(Faixa::dinamica(aba, format!("vba.{h}({}, {})", fa.end, fb.end)))
            }
            _ => Err(Motivo::novo("Range com argumentos estranhos")),
        }
    }

    /// Uma das pontas do `Range(a, b)`: `Cells(…)` ou um endereço.
    fn ponta(&mut self, aba: &str, e: &Expr) -> R<Faixa> {
        if let Expr::Texto(t) = e {
            return Ok(Faixa::fixa(aba, t));
        }
        match self.objeto(e) {
            Ok(Obj::Faixa(f)) => Ok(f),
            _ => {
                let v = self.valor(e)?;
                let end = self.como_texto(&v);
                Ok(Faixa::dinamica(aba, end))
            }
        }
    }

    /// `Cells(linha, coluna)`: endereço fixo quando os dois são números; senão `` `B${i}` ``.
    fn faixa_de_cells(&mut self, aba: &str, args: &[Arg]) -> R<Faixa> {
        let exprs: Vec<&Expr> = args.iter().filter_map(|a| a.valor.as_ref()).collect();
        let [l, c] = exprs.as_slice() else { return Err(Motivo::novo("Cells com um argumento só")) };
        let fundo = matches!(l, Expr::Membro(b, m) if m.eq_ignore_ascii_case("count") && matches!(&**b, Expr::Nome(r) if r.eq_ignore_ascii_case("rows")));
        let coluna_letra = match c {
            Expr::Num(n) => Some(letra(*n as u32)),
            Expr::Texto(t) if t.chars().all(|x| x.is_ascii_alphabetic()) => Some(t.to_uppercase()),
            _ => None,
        };
        let mut f = match (l, &coluna_letra) {
            (Expr::Num(n), Some(col)) => Faixa::fixa(aba, &format!("{col}{}", num(*n))),
            (_, Some(col)) => {
                let v = self.valor(l)?;
                Faixa { unica: true, ..Faixa::dinamica(aba, format!("`{col}${{{}}}`", v.js)) }
            }
            _ => {
                let vl = self.valor(l)?;
                let vc = self.valor(c)?;
                let h = self.ajuda("celula");
                Faixa { unica: true, ..Faixa::dinamica(aba, format!("vba.{h}({}, {})", vl.js, vc.js)) }
            }
        };
        if fundo {
            f.fundo_da_coluna = Some(match &coluna_letra {
                Some(col) => format!("{col:?}"),
                None => {
                    let vc = self.valor(c)?;
                    vc.js
                }
            });
        }
        Ok(f)
    }

    fn membro_da_faixa(&mut self, f: Faixa, n: &str, args: &[Arg]) -> R<Obj> {
        let aba = f.aba.clone();
        let posicionais = || args.iter().filter_map(|a| a.valor.as_ref()).collect::<Vec<_>>();
        Ok(match n {
            "font" => Obj::Parte(f, Parte::Fonte),
            "interior" => Obj::Parte(f, Parte::Interior),
            "borders" if args.is_empty() => Obj::Parte(f, Parte::Bordas(None)),
            "borders" => {
                let lado = args[0].valor.as_ref().and_then(lado_da_borda).ok_or_else(|| Motivo::novo("borda com lado calculado"))?;
                Obj::Parte(f, Parte::Bordas(Some(lado.into())))
            }
            "rows" if args.is_empty() => Obj::Dimensao(f, Inteira::Linhas),
            "columns" if args.is_empty() => Obj::Dimensao(f, Inteira::Colunas),
            "entirerow" | "entirecolumn" => {
                let (h, eixo) = if n == "entirerow" { (self.ajuda("linhas"), Inteira::Linhas) } else { (self.ajuda("colunas"), Inteira::Colunas) };
                let mut g = Faixa::dinamica(&aba, format!("vba.{h}({})", f.end));
                g.inteira = eixo;
                Obj::Faixa(g)
            }
            "offset" => {
                let p = posicionais();
                let dl = p.first().map(|e| self.valor(e)).transpose()?.map_or("0".into(), |v| v.js);
                let dc = p.get(1).map(|e| self.valor(e)).transpose()?.map_or("0".into(), |v| v.js);
                self.deslocado(&f, &dl, &dc)
            }
            "resize" => {
                let p = posicionais();
                let vl = p.first().map(|e| self.valor(e)).transpose()?.map_or("1".into(), |v| v.js);
                let vc = p.get(1).map(|e| self.valor(e)).transpose()?.map_or("1".into(), |v| v.js);
                Obj::Faixa(Faixa::dinamica(&aba, format!("{aba}.resize({}, {vl}, {vc}).address", f.end)))
            }
            "cells" | "item" if args.len() == 2 => {
                let p = posicionais();
                let vl = self.valor(p[0])?;
                let vc = self.valor(p[1])?;
                let (a, b) = (self.como_num(&vl), self.como_num(&vc));
                self.deslocado(&f, &format!("{a} - 1"), &format!("{b} - 1"))
            }
            "end" => self.fim_da_faixa(f, args)?,
            "currentregion" => Obj::Faixa(Faixa::dinamica(&aba, format!("{aba}.getRegion({}).address", f.end))),
            "mergearea" | "cells" => Obj::Faixa(f),
            "worksheet" | "parent" => Obj::Aba(aba, None),
            "shaperange" => return Err(Motivo::novo("formas e botões do Excel (use Inserir → Botão de macro)")),
            "validation" => Obj::Validacao(f),
            "sort" => return Err(Motivo::novo("Range.Sort usado como objeto")),
            "autofilter" => return Err(Motivo::novo("filtro do Excel (AutoFilter — converter à mão)")),
            "formatconditions" => Obj::Condicoes(f),
            "specialcells" => return Err(Motivo::novo("SpecialCells (converter à mão)")),
            _ => return Err(Motivo::novo(format!("Range.{n}"))),
        })
    }

    /// `.Offset(dl, dc)`: no próprio código quando o endereço e os números são fixos.
    fn deslocado(&mut self, f: &Faixa, dl: &str, dc: &str) -> Obj {
        if let (Some(l), Ok(a), Ok(b)) = (f.literal.as_deref().and_then(celula), dl.parse::<i64>(), dc.parse::<i64>()) {
            let (nl, nc) = (i64::from(l.0) + a, i64::from(l.1) + b);
            if nl >= 1 && nc >= 1 {
                return Obj::Faixa(Faixa::fixa(&f.aba, &format!("{}{nl}", letra(nc as u32))));
            }
        }
        Obj::Faixa(Faixa::dinamica(&f.aba, format!("{}.offset({}, {dl}, {dc}).address", f.aba, f.end)))
    }

    /// `.End(xlUp)`: o `getLastRow` quando é `Cells(Rows.Count, c)`; senão o Ctrl+seta.
    fn fim_da_faixa(&mut self, f: Faixa, args: &[Arg]) -> R<Obj> {
        let dir = args.first().and_then(|a| a.valor.as_ref()).ok_or_else(|| Motivo::novo("End sem direção"))?;
        if let (Some(col), true) = (&f.fundo_da_coluna, e_constante(dir, "xlUp")) {
            let l = self.ajuda("letra");
            let coluna = if col.starts_with('"') { col.clone() } else { format!("vba.{l}({col})") };
            let cel = if col.starts_with('"') { format!("`{}${{{}.getLastRow({coluna}) || 1}}`", col.trim_matches('"'), f.aba) } else { format!("`${{{coluna}}}${{{}.getLastRow({coluna}) || 1}}`", f.aba) };
            return Ok(Obj::Faixa(Faixa { unica: true, ..Faixa::dinamica(&f.aba, cel) }));
        }
        let d = self.valor(dir)?;
        let h = self.ajuda("fim");
        Ok(Obj::Faixa(Faixa { unica: true, ..Faixa::dinamica(&f.aba, format!("vba.{h}({}, {}, {})", f.aba, f.end, d.js)) }))
    }

    // ------------------------------------------------------------------ ler

    /// O valor padrão de um objeto (`Range("A1")` sozinho é o valor da célula).
    pub fn valor_do_objeto(&mut self, o: Obj) -> R<V> {
        match o {
            Obj::Faixa(f) => Ok(self.valor_da_faixa(&f)),
            Obj::Controle(c) => Ok(V::novo(format!("sheet.field({c:?}).value"), T::Var)),
            _ => Err(Motivo::novo("objeto usado como valor")),
        }
    }

    /// `Range.Value`: o valor da célula, ou a matriz (do 1, como no VBA) quando pode ser mais de uma.
    fn valor_da_faixa(&mut self, f: &Faixa) -> V {
        if f.uma_celula() {
            V::novo(format!("{}.value({})", f.aba, f.end), T::Var)
        } else {
            V::novo(format!("vba.{}({}, {})", self.ajuda("ler"), f.aba, f.end), T::Var)
        }
    }

    /// `obj.propriedade` lida.
    pub fn propriedade(&mut self, o: Obj, nome: &str, args: &[Arg]) -> R<V> {
        let n = nome.to_lowercase();
        match o {
            Obj::Faixa(f) => self.propriedade_da_faixa(f, &n, args),
            Obj::Controle(c) => propriedade_do_controle(&c, &n),
            Obj::Formulario(f) => match self.p.controle(&f, nome) {
                Some(c) => Ok(V::novo(format!("sheet.field({c:?}).value"), T::Var)),
                None => Err(Motivo::novo(format!("{f}.{nome} do formulário"))),
            },
            Obj::Aba(aba, nome_aba) => match n.as_str() {
                "name" | "codename" => Ok(nome_aba.map_or_else(|| V::novo(format!("{aba}.name"), T::Texto), |x| V::novo(format!("{x:?}"), T::Texto))),
                _ => {
                    let o = self.membro_objeto(Obj::Aba(aba, nome_aba), nome, args)?;
                    self.valor_do_objeto(o)
                }
            },
            Obj::Contagem(_, eixo) if n == "count" => Ok(V::novo(if eixo == Inteira::Linhas { "1048576" } else { "16384" }, T::Num)),
            Obj::Dimensao(f, eixo) if n == "count" => Ok(V::novo(format!("{}.{}({})", f.aba, if eixo == Inteira::Linhas { "rowCount" } else { "columnCount" }, f.end), T::Num)),
            Obj::Abas if n == "count" => Ok(V::novo("wb.sheetCount", T::Num)),
            Obj::AbasNovas(p) if n == "count" => Ok(V::novo(format!("{p}.abas.length"), T::Num)),
            Obj::Pasta | Obj::App => match n.as_str() {
                "name" => Ok(V::novo("wb.name", T::Texto)),
                // A pasta e o caminho do arquivo aberto (vazios numa pasta nunca salva).
                "path" => Ok(V::novo("wb.path", T::Texto)),
                "fullname" => Ok(V::novo("wb.fullName", T::Texto)),
                "caller" => Err(Motivo::novo("Application.Caller (quem chamou a macro)")),
                _ => {
                    let o = self.membro_objeto(o, nome, args)?;
                    self.valor_do_objeto(o)
                }
            },
            Obj::FuncoesDaPlanilha => self.funcao_da_planilha(&n, args),
            _ => Err(Motivo::novo(format!("propriedade {nome}"))),
        }
    }

    fn propriedade_da_faixa(&mut self, f: Faixa, n: &str, args: &[Arg]) -> R<V> {
        let (aba, end) = (f.aba.clone(), f.end.clone());
        let fixa = f.literal.as_deref().and_then(celula);
        Ok(match n {
            "value" | "value2" => self.valor_da_faixa(&f),
            "text" => V::novo(format!("{aba}.getText({end})"), T::Texto),
            "formula" | "formulalocal" => V::novo(format!("{aba}.formula({end})"), T::Texto),
            "row" => fixa.map_or_else(|| V::novo(format!("vba.{}({end})", self.ajuda("linha")), T::Num), |(l, _)| V::novo(l.to_string(), T::Num)),
            "column" => fixa.map_or_else(|| V::novo(format!("vba.{}({end})", self.ajuda("coluna")), T::Num), |(_, c)| V::novo(c.to_string(), T::Num)),
            "address" => V::novo(format!("vba.{}({end})", self.ajuda("endAbs")), T::Texto),
            "count" | "countlarge" => V::novo(format!("vba.{}({end})", self.ajuda("contar")), T::Num),
            "mergecells" => V::novo(format!("{aba}.isMerged({end})"), T::Bool),
            _ => {
                let o = self.membro_da_faixa(f, n, args)?;
                return match o {
                    Obj::Faixa(_) => self.valor_do_objeto(o),
                    _ => Err(Motivo::novo(format!("ler Range.{n}"))),
                };
            }
        })
    }

    /// `WorksheetFunction.Sum(Range("A1:A9"))` → `app.fn.sum(...)`.
    fn funcao_da_planilha(&mut self, n: &str, args: &[Arg]) -> R<V> {
        let mut vs = Vec::new();
        for a in args.iter().filter_map(|a| a.valor.as_ref()) {
            vs.push(match self.objeto(a) {
                Ok(Obj::Faixa(f)) => format!("{}.range({}).values()", f.aba, f.end),
                _ => self.valor(a)?.js,
            });
        }
        let api = match n {
            "sum" => "sum",
            "average" => "average",
            "count" => "count",
            "counta" => "countA",
            "max" => "max",
            "min" => "min",
            "median" => "median",
            "countif" => "countIf",
            "sumif" => "sumIf",
            "averageif" => "averageIf",
            "vlookup" => "vlookup",
            "hlookup" => "hlookup",
            "match" => "match",
            "index" => "index",
            "rounddown" => return Ok(V::novo(format!("vba.{}({})", self.ajuda("arredBaixo"), vs.join(", ")), T::Num)),
            "roundup" => return Ok(V::novo(format!("vba.{}({})", self.ajuda("arredCima"), vs.join(", ")), T::Num)),
            "round" => return Ok(V::novo(format!("vba.{}({})", self.ajuda("round"), vs.join(", ")), T::Num)),
            _ => return Err(Motivo::novo(format!("WorksheetFunction.{n} (sem equivalente em app.fn)"))),
        };
        Ok(V::novo(format!("app.fn.{api}({})", vs.join(", ")), T::Var))
    }
}

/// `TextBox1.Value`, `.Text`, `.Caption`, `.Enabled`, `.Visible` → `sheet.field("TextBox1")`.
fn propriedade_do_controle(c: &str, n: &str) -> R<V> {
    let (p, t) = match n {
        "value" | "text" => ("value", T::Var),
        "caption" => ("caption", T::Texto),
        "enabled" => ("enabled", T::Bool),
        "visible" => ("visible", T::Bool),
        _ => return Err(Motivo::novo(format!("{c}.{n} do controle do formulário"))),
    };
    Ok(V::novo(format!("sheet.field({c:?}).{p}"), t))
}

impl Gerador<'_> {
    /// `UserForm1` (ou `Me` no código do formulário) → o nome do formulário.
    pub fn formulario_por_nome(&self, n: &str) -> Option<String> {
        if n.eq_ignore_ascii_case("me") {
            return self.p.formulario_do_modulo(self.modulo).map(String::from);
        }
        self.p.formulario(n).map(String::from)
    }

    /// Um controle do formulário cujo código está sendo escrito (`TextBox1` sozinho).
    pub fn controle_do_modulo(&self, n: &str) -> Option<String> {
        let f = self.p.formulario_do_modulo(self.modulo)?;
        self.p.controle(f, n).map(String::from)
    }
}

/// `Range("A" & i)`: uma letra de coluna e um número de linha, sem `:` nem `,`.
fn coluna_e_linha(e: &Expr) -> bool {
    match e {
        Expr::Bin(Op::Concat, a, b) => matches!(&**a, Expr::Texto(t) if !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic() || c == '$')) && !matches!(&**b, Expr::Texto(_) | Expr::Bin(Op::Concat, ..)),
        _ => false,
    }
}
