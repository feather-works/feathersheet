//! Instruções e blocos. Uma instrução que não converte vira comentário com o VBA original
//! e entra na lista de pendências — o resto do procedimento continua sendo convertido.

use super::contexto::{e_inteiro, tipo_declarado, Gerador, Laco, Pendencia};
use super::funcoes::num;
use super::tipos::{Motivo, Obj, Tipo, R, T, V};
use crate::sintaxe::{Caso, Decl, Expr, Instr, Op, TipoInstr};

/// O nome é atribuído em algum lugar do corpo (para saber se o fim do `For` pode ficar sem cópia).
fn atribuido_em(nome: &str, corpo: &[Instr]) -> bool {
    corpo.iter().any(|i| match &i.k {
        TipoInstr::Atribuir { alvo: Expr::Nome(n), .. } => n.eq_ignore_ascii_case(nome),
        TipoInstr::Se { ramos, senao } => ramos.iter().any(|(_, c)| atribuido_em(nome, c)) || senao.as_ref().is_some_and(|s| atribuido_em(nome, s)),
        TipoInstr::Escolha { casos, senao, .. } => casos.iter().any(|(_, c)| atribuido_em(nome, c)) || senao.as_ref().is_some_and(|s| atribuido_em(nome, s)),
        TipoInstr::Para { var, corpo, .. } => var.eq_ignore_ascii_case(nome) || atribuido_em(nome, corpo),
        TipoInstr::ParaCada { corpo, .. } | TipoInstr::Faca { corpo, .. } | TipoInstr::Com { corpo, .. } => atribuido_em(nome, corpo),
        // Chamada pode mudar a variável (ByRef): melhor copiar o fim.
        TipoInstr::Chamar(_) => true,
        _ => false,
    })
}

/// O nome JS do bloco de um rótulo do VBA.
fn rotulo_js(r: &str) -> String {
    format!("rotulo_{}", r.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>())
}

/// A instrução (ou algo dentro dela) faz `GoTo rotulo`.
fn chama(i: &Instr, rotulo: &str) -> bool {
    let em = |c: &[Instr]| c.iter().any(|x| chama(x, rotulo));
    match &i.k {
        TipoInstr::IrPara(n) => n.eq_ignore_ascii_case(rotulo),
        TipoInstr::Se { ramos, senao } => ramos.iter().any(|(_, c)| em(c)) || senao.as_deref().is_some_and(em),
        TipoInstr::Escolha { casos, senao, .. } => casos.iter().any(|(_, c)| em(c)) || senao.as_deref().is_some_and(em),
        TipoInstr::Para { corpo, .. } | TipoInstr::ParaCada { corpo, .. } | TipoInstr::Faca { corpo, .. } | TipoInstr::Com { corpo, .. } => em(corpo),
        _ => false,
    }
}

/// O motivo de uma instrução que o analisador não entendeu.
fn motivo_da_desconhecida(t: &str) -> String {
    let primeira = t.split_whitespace().next().unwrap_or("").to_lowercase();
    match primeira.as_str() {
        "open" | "close" | "line" | "print" | "input" | "kill" | "mkdir" | "rmdir" | "chdir" | "filecopy" | "write" | "get" | "put" | "seek" => "acesso a arquivo".into(),
        _ => "o conversor não entendeu esta linha".into(),
    }
}

impl Gerador<'_> {
    /// Anota a instrução como não convertida: comentário no código e item na lista.
    pub fn pendente(&mut self, i: &Instr, m: &Motivo) {
        let fonte = self.p.fonte(self.modulo, i);
        self.linha(format!("// VBA não convertido ({}):", m.0));
        for l in &fonte {
            self.linha(format!("//   {l}"));
        }
        self.m.pendencias.push(Pendencia { modulo: self.p.modulos[self.modulo].nome.clone(), linha: i.linha, vba: fonte.join("\n"), motivo: m.0.clone(), aviso: false });
    }

    /// Um aviso: a instrução foi convertida, mas o comportamento pode mudar.
    pub fn avisar(&mut self, linha: usize, vba: &str, motivo: &str) {
        let modulo = self.p.modulos[self.modulo].nome.clone();
        if self.m.pendencias.iter().any(|p| p.aviso && p.modulo == modulo && p.linha == linha && p.motivo == motivo) {
            return;
        }
        self.m.pendencias.push(Pendencia { modulo, linha, vba: vba.into(), motivo: motivo.into(), aviso: true });
    }

    pub fn instrucoes(&mut self, v: &[Instr]) {
        for i in v {
            self.instrucao(i);
        }
    }

    fn instrucao(&mut self, i: &Instr) {
        let r = match &i.k {
            TipoInstr::Se { ramos, senao } => self.se(ramos, senao.as_deref()),
            TipoInstr::Escolha { expr, casos, senao } => self.escolha(expr, casos, senao.as_deref()),
            TipoInstr::Para { var, de, ate, passo, corpo } => self.para(var, de, ate, passo.as_ref(), corpo),
            TipoInstr::ParaCada { var, em, corpo } => self.para_cada(var, em, corpo),
            TipoInstr::Faca { antes, depois, corpo } => self.faca(antes.as_ref(), depois.as_ref(), corpo),
            TipoInstr::Com { obj, corpo } => self.bloco_com(obj, corpo),
            _ => self.simples(i).map(|js| self.linha(js)),
        };
        match r {
            Ok(()) => self.m.convertidas += 1,
            Err(m) => self.pendente(i, &m),
        }
    }

    fn simples(&mut self, i: &Instr) -> R<String> {
        Ok(match &i.k {
            TipoInstr::Dim(decls) => self.dim(decls)?,
            TipoInstr::Const(decls) => self.constantes(decls)?,
            TipoInstr::ReDim { preservar, decls } => self.redim(*preservar, decls)?,
            TipoInstr::Atribuir { alvo, valor, set } => self.atribuir(i, alvo, valor, *set)?,
            TipoInstr::Chamar(e) => self.chamar(e)?,
            TipoInstr::Sair(o) => self.sair(o)?,
            TipoInstr::Terminar => {
                self.avisar(i.linha, "End", "End parava a macro inteira; aqui só sai deste procedimento");
                "return;".into()
            }
            TipoInstr::EmErro(t) if t == "goto 0" || t == "goto -1" => String::new(),
            TipoInstr::EmErro(t) if t == "resume next" => {
                self.avisar(i.linha, "On Error Resume Next", "On Error Resume Next: no JavaScript um erro para a macro (não pula para a próxima linha)");
                "// On Error Resume Next (sem equivalente: um erro aqui para a macro)".into()
            }
            // Rótulo que ninguém chama (ou já tratado pelo bloco): não faz nada.
            TipoInstr::Rotulo(_) => String::new(),
            TipoInstr::IrPara(n) if self.rotulos.contains(&n.to_lowercase()) => format!("break {};", rotulo_js(n)),
            TipoInstr::IrPara(_) => return Err(Motivo::novo("GoTo para trás ou para dentro de um bloco (reescrever com laço ou if)")),
            TipoInstr::EmErro(_) => return Err(Motivo::novo("On Error GoTo fora do começo do procedimento (reescrever com try/catch)")),
            TipoInstr::Desconhecida(t) => return Err(Motivo::novo(motivo_da_desconhecida(t))),
            _ => unreachable!("blocos vão por instrucao()"),
        })
    }

    // ------------------------------------------------------------------ rótulos e erros

    /// O corpo do procedimento. `On Error GoTo trata` … `trata:` … vira
    /// `try { … } catch ($erro) { … }`; um `GoTo fim` para a frente vira `break`
    /// de um bloco rotulado que termina no `fim:`.
    pub fn corpo_do_proc(&mut self, corpo: &[Instr]) {
        let rotulo_em = |nome: &str| corpo.iter().position(|i| matches!(&i.k, TipoInstr::Rotulo(r) if r.eq_ignore_ascii_case(nome)));
        let tratador = corpo.iter().enumerate().find_map(|(i, x)| match &x.k {
            TipoInstr::EmErro(t) => t.strip_prefix("goto ").filter(|n| *n != "0" && *n != "-1")
                .and_then(rotulo_em).filter(|&k| k > i).map(|k| (i, k)),
            _ => None,
        });
        let Some((i, k)) = tratador else { return self.com_rotulos(corpo) };
        self.com_rotulos(&corpo[..i]);
        self.m.convertidas += 2;
        self.linha("try {");
        self.nivel += 1;
        self.com_rotulos(&corpo[i + 1..k]);
        self.nivel -= 1;
        let guardar = self.ajuda("erro");
        self.linha("} catch ($erro) {");
        self.nivel += 1;
        self.linha(format!("vba.{guardar}($erro);"));
        self.com_rotulos(&corpo[k + 1..]);
        self.nivel -= 1;
        self.linha("}");
    }

    /// As instruções, com um bloco `rotulo_x: { … }` para cada rótulo que um `GoTo`
    /// anterior chama. Os blocos se aninham: o do último rótulo fica por fora.
    fn com_rotulos(&mut self, v: &[Instr]) {
        let chamados: Vec<(usize, String)> = v.iter().enumerate().filter_map(|(k, x)| match &x.k {
            TipoInstr::Rotulo(r) if v[..k].iter().any(|y| chama(y, r)) => Some((k, r.to_lowercase())),
            _ => None,
        }).collect();
        for (_, r) in chamados.iter().rev() {
            self.linha(format!("{}: {{", rotulo_js(r)));
            self.nivel += 1;
            self.rotulos.push(r.clone());
        }
        let mut inicio = 0;
        for (k, r) in &chamados {
            self.instrucoes(&v[inicio..*k]);
            self.nivel -= 1;
            self.linha("}");
            self.rotulos.retain(|x| x != r);
            self.m.convertidas += 1;
            inicio = k + 1;
        }
        self.instrucoes(&v[inicio..]);
    }

    // ------------------------------------------------------------------ declarações

    /// `Dim`: a variável já foi declarada no topo do procedimento; aqui só a matriz com tamanho.
    fn dim(&mut self, decls: &[Decl]) -> R<String> {
        let mut saida = Vec::new();
        for d in decls {
            if d.novo {
                return Err(Motivo::novo(format!("As New {}", d.tipo.as_deref().unwrap_or(""))));
            }
            if let Some(dims) = d.dims.as_ref().filter(|x| !x.is_empty()) {
                let js = self.var(&d.nome).map_or_else(|| d.nome.clone(), |v| v.js);
                let lim = self.limites(dims)?;
                saida.push(format!("{js} = vba.{}({});", self.ajuda("matriz"), lim.join(", ")));
            }
        }
        Ok(saida.join(" "))
    }

    /// Os limites de cada dimensão para o `vba.matriz`: `n`, ou `[1, n]` no `1 To n`.
    fn limites(&mut self, dims: &[(Option<Expr>, Expr)]) -> R<Vec<String>> {
        dims.iter()
            .map(|(lb, ub)| {
                let ub = self.valor(ub)?.js;
                Ok(match lb {
                    None | Some(Expr::Num(0.0)) => ub,
                    Some(lb) => format!("[{}, {ub}]", self.valor(lb)?.js),
                })
            })
            .collect()
    }

    fn constantes(&mut self, decls: &[Decl]) -> R<String> {
        let mut saida = Vec::new();
        for d in decls {
            let Some(e) = &d.valor else { continue };
            let v = self.valor(e)?;
            let js = self.var(&d.nome).map_or_else(|| d.nome.clone(), |x| x.js);
            saida.push(format!("{js} = {};", v.js));
        }
        Ok(saida.join(" "))
    }

    fn redim(&mut self, preservar: bool, decls: &[Decl]) -> R<String> {
        let mut saida = Vec::new();
        for d in decls {
            let js = match self.var(&d.nome) {
                Some(v) => v.js,
                None => self.declarar(&d.nome, Tipo::Valor(T::Matriz), true),
            };
            let lim = self.limites(d.dims.as_deref().unwrap_or_default())?;
            saida.push(format!("{js} = vba.{}({js}, {preservar}, {});", self.ajuda("redim"), lim.join(", ")));
        }
        Ok(saida.join(" "))
    }

    // ------------------------------------------------------------------ atribuição e chamada

    fn atribuir(&mut self, i: &Instr, alvo: &Expr, valor: &Expr, set: bool) -> R<String> {
        match alvo {
            Expr::Nome(n) => self.atribuir_nome(i, n, valor, set),
            Expr::Chamada(f, idx) if matches!(&**f, Expr::Nome(n) if self.var(n).is_some_and(|v| v.matriz || matches!(v.tipo, Tipo::Valor(T::Var | T::Matriz)))) => {
                let Expr::Nome(n) = &**f else { unreachable!() };
                let js = self.var(n).map(|v| v.js).unwrap_or_default();
                let mut alvo_js = js;
                for a in self.valores(idx)? {
                    alvo_js.push_str(&format!("[{}]", if a.t == T::Num { a.js.clone() } else { self.como_num(&a) }));
                }
                let v = self.valor(valor)?;
                Ok(format!("{alvo_js} = {};", v.js))
            }
            Expr::Membro(base, prop) => {
                let o = self.objeto(base)?;
                self.atribuir_propriedade(o, prop, valor)
            }
            Expr::MembroDoCom(prop) => {
                let o = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.atribuir_propriedade(o, prop, valor)
            }
            Expr::Chamada(..) => {
                // `Range("A3") = "CNPJ"`, `Cells(3, 2) = x`: o valor padrão do intervalo.
                let o = self.objeto(alvo)?;
                self.atribuir_propriedade(o, "value", valor)
            }
            _ => Err(Motivo::novo("atribuição a algo que não é variável nem propriedade")),
        }
    }

    fn atribuir_nome(&mut self, i: &Instr, n: &str, valor: &Expr, set: bool) -> R<String> {
        let chave = n.to_lowercase();
        if self.escopo.funcao.as_deref() == Some(chave.as_str()) {
            let t = self.escopo.vars.get("$r").map(|v| v.tipo.clone()).unwrap_or(Tipo::Valor(T::Var));
            let v = self.valor(valor)?;
            if self.escopo.inteiros.contains("$r") && !self.ja_inteiro(&v) {
                return Ok(format!("$r = vba.{}({});", self.ajuda("cint"), v.js));
            }
            return Ok(format!("$r = {};", self.converter_para(&t, &v)));
        }
        let var = match self.var(n).or_else(|| self.global(n)) {
            Some(v) => v,
            None if self.p.codenames.contains_key(&chave) => return Err(Motivo::novo("atribuir a uma aba")),
            // `TextBox1 = "x"` no código do formulário: o valor padrão do controle.
            None if self.controle_do_modulo(n).is_some() => {
                let c = self.controle_do_modulo(n).unwrap_or_default();
                return self.escrever_no_controle(&c, "value", valor);
            }
            None if set => {
                // `Set rng = Range(…)` sem `Dim`: o tipo vem do que é atribuído. Se não dá para
                // converter, a variável fica marcada: o que a usa depois cai no mesmo motivo.
                let obj = match self.objeto(valor) {
                    Ok(o) => o,
                    Err(_) => {
                        let m = self.motivo_do_objeto(valor);
                        self.declarar(n, Tipo::Impossivel(m.0.clone()), false);
                        return Err(m);
                    }
                };
                let tipo = match obj {
                    Obj::Faixa(_) => Tipo::Faixa(None),
                    Obj::Aba(..) => Tipo::Aba,
                    Obj::NovaPasta(_) => Tipo::NovaPasta,
                    _ => return Err(Motivo::novo("Set de objeto que não é aba nem intervalo")),
                };
                self.declarar(n, tipo, false);
                self.var(n).expect("acabou de declarar")
            }
            None => {
                self.escopo.implicitas.insert(chave.clone());
                self.declarar(n, Tipo::Valor(T::Var), false);
                self.var(n).expect("acabou de declarar")
            }
        };
        if var.por_ref {
            self.avisar(i.linha, n, "parâmetro ByRef alterado: no JavaScript a mudança não volta para quem chamou");
        }
        match &var.tipo {
            Tipo::Impossivel(antes) if set && antes.starts_with("objeto do tipo") => {
                // O `Set` diz o motivo melhor que o `As Object` (ex.: `CreateObject("WScript.Shell")`).
                let m = self.motivo_do_objeto(valor);
                if let Some(v) = self.escopo.vars.get_mut(&chave) {
                    v.tipo = Tipo::Impossivel(m.0.clone());
                }
                Err(m)
            }
            Tipo::Impossivel(m) => Err(Motivo::novo(m.clone())),
            Tipo::NovaPasta => match valor {
                Expr::Nada => Ok(format!("{} = null;", var.js)),
                _ => match self.objeto(valor)? {
                    Obj::NovaPasta(js) => Ok(format!("{} = {js};", var.js)),
                    _ => Err(Motivo::novo("outra pasta de trabalho (só Workbooks.Add converte)")),
                },
            },
            Tipo::Aba => match self.objeto(valor)? {
                Obj::Aba(js, _) => Ok(format!("{} = {js};", var.js)),
                _ => Err(Motivo::novo("Set de aba com algo que não é aba")),
            },
            Tipo::Faixa(_) if set || matches!(valor, Expr::Nada) => match valor {
                Expr::Nada => Ok(format!("{} = null;", var.js)),
                _ => match self.objeto(valor)? {
                    Obj::Faixa(f) => {
                        self.escopo.aba_da_faixa.insert(var.js.clone(), f.aba.clone());
                        Ok(format!("{} = {};", var.js, f.end))
                    }
                    _ => Err(Motivo::novo("Set de intervalo com algo que não é intervalo")),
                },
            },
            Tipo::Faixa(_) => {
                let f = self.faixa_da_var(&var);
                self.atribuir_propriedade(Obj::Faixa(f), "value", valor)
            }
            Tipo::Valor(_) => {
                let v = self.valor(valor)?;
                // `Dim m() As String: m = Split(...)`: a matriz inteira é atribuída; o
                // `As String` vale para cada item, não vira texto ("A,B,C").
                if var.matriz {
                    return Ok(format!("{} = {};", var.js, v.js));
                }
                if self.escopo.inteiros.contains(&var.js) && !self.ja_inteiro(&v) {
                    return Ok(format!("{} = vba.{}({});", var.js, self.ajuda("cint"), v.js));
                }
                Ok(format!("{} = {};", var.js, self.converter_para(&var.tipo, &v)))
            }
        }
    }

    /// Por que o lado direito de um `Set` não converte: a conversão como valor sabe dizer
    /// (`CreateObject` → "automação de outro programa"); a como objeto, só "desconhecido".
    fn motivo_do_objeto(&mut self, e: &Expr) -> Motivo {
        let como_objeto = self.objeto(e).err();
        match self.valor(e) {
            Err(m) if !m.0.contains("desconhecido") && !m.0.contains("não é objeto") => m,
            _ => como_objeto.unwrap_or_else(|| Motivo::novo("objeto sem equivalente")),
        }
    }

    /// O valor já é inteiro com certeza (literal inteiro, variável inteira, `vba.cint`, `Len`…)?
    fn ja_inteiro(&self, v: &V) -> bool {
        v.js.parse::<i64>().is_ok() || self.escopo.inteiros.contains(&v.js) || ["vba.cint(", "vba.len(", "vba.instr(", "vba.linha(", "vba.coluna(", "Math.floor(", "Math.trunc(", "vba.mod(", "vba.divInt("].iter().any(|p| v.js.starts_with(p) && v.atomico)
    }

    /// O valor convertido para o tipo da variável, como o VBA faz ao atribuir.
    fn converter_para(&mut self, t: &Tipo, v: &V) -> String {
        match t {
            Tipo::Valor(T::Texto) if v.t != T::Texto => format!("vba.{}({})", self.ajuda("str"), v.js),
            Tipo::Valor(T::Num) if v.t != T::Num => format!("vba.{}({})", self.ajuda("n"), v.js),
            Tipo::Valor(T::Bool) if v.t != T::Bool => self.como_bool(v),
            _ => v.js.clone(),
        }
    }

    fn chamar(&mut self, e: &Expr) -> R<String> {
        match e {
            Expr::Nome(n) => {
                if let Some(r) = self.p.achar(self.modulo, n) {
                    return Ok(format!("{};", self.chamar_proc(r, &[])?.js));
                }
                if matches!(n.to_lowercase().as_str(), "doevents" | "beep") {
                    return Ok(String::new());
                }
                Err(Motivo::novo(format!("{n} sem equivalente")))
            }
            Expr::Chamada(f, args) => match &**f {
                // `Unload Me` / `Unload UserForm1` fecha o formulário; `Load` não tem o que fazer.
                Expr::Nome(n) if n.eq_ignore_ascii_case("unload") || n.eq_ignore_ascii_case("load") => {
                    let alvo = args.first().and_then(|a| a.valor.as_ref());
                    match alvo {
                        Some(Expr::Nome(f)) if self.formulario_por_nome(f).is_some() => {
                            Ok(if n.eq_ignore_ascii_case("unload") { "sheet.hideComponent();".into() } else { String::new() })
                        }
                        _ => Err(Motivo::novo(format!("{n} de algo que não é formulário"))),
                    }
                }
                Expr::Nome(_) => Ok(format!("{};", self.valor(e)?.js)),
                Expr::Membro(base, m) => self.chamar_membro(base, m, args),
                Expr::MembroDoCom(m) => {
                    let o = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                    self.metodo(o, m, args)
                }
                _ => Err(Motivo::novo("chamada estranha")),
            },
            Expr::Membro(base, m) => self.chamar_membro(base, m, &[]),
            Expr::MembroDoCom(m) => {
                let o = self.com.last().cloned().ok_or_else(|| Motivo::novo("`.membro` fora de With"))??;
                self.metodo(o, m, &[])
            }
            _ => Err(Motivo::novo("instrução estranha")),
        }
    }

    fn chamar_membro(&mut self, base: &Expr, m: &str, args: &[crate::sintaxe::Arg]) -> R<String> {
        if let Expr::Nome(b) = base {
            if b.eq_ignore_ascii_case("debug") {
                let vs = self.valores(args)?;
                return Ok(format!("console.log({});", vs.iter().map(|v| v.js.as_str()).collect::<Vec<_>>().join(", ")));
            }
            if b.eq_ignore_ascii_case("err") && self.var("err").is_none() {
                let e = self.ajuda("erro");
                return match m.to_lowercase().as_str() {
                    "clear" => Ok(format!("vba.{e}(null);")),
                    "raise" => {
                        let vs = self.valores(args)?;
                        let num = vs.first().map_or("1".to_string(), |v| v.js.clone());
                        let texto = vs.get(2).map_or_else(|| format!("'Erro ' + {num}"), |v| v.js.clone());
                        Ok(format!("throw Object.assign(new Error({texto}), {{ number: {num} }});"))
                    }
                    _ => Err(Motivo::novo(format!("Err.{m}"))),
                };
            }
            if b.eq_ignore_ascii_case("vba") || self.p.achar_qualificado(b, m).is_some() {
                return Ok(format!("{};", self.valor(&Expr::Chamada(Box::new(Expr::Membro(Box::new(base.clone()), m.into())), args.to_vec()))?.js));
            }
        }
        let o = self.objeto(base)?;
        self.metodo(o, m, args)
    }

    // ------------------------------------------------------------------ blocos

    fn bloco(&mut self, corpo: &[Instr]) {
        self.nivel += 1;
        self.instrucoes(corpo);
        self.nivel -= 1;
    }

    fn se(&mut self, ramos: &[(Expr, Vec<Instr>)], senao: Option<&[Instr]>) -> R<()> {
        let conds: Vec<String> = ramos.iter().map(|(c, _)| self.valor(c).map(|v| self.como_bool(&v))).collect::<R<_>>()?;
        for (k, ((_, corpo), c)) in ramos.iter().zip(conds).enumerate() {
            self.linha(if k == 0 { format!("if ({c}) {{") } else { format!("}} else if ({c}) {{") });
            self.bloco(corpo);
        }
        if let Some(s) = senao {
            self.linha("} else {");
            self.bloco(s);
        }
        self.linha("}");
        Ok(())
    }

    fn escolha(&mut self, e: &Expr, casos: &[(Vec<Caso>, Vec<Instr>)], senao: Option<&[Instr]>) -> R<()> {
        let v = self.valor(e)?;
        let id = self.novo_id();
        let sel = format!("$caso{id}");
        let alvo = V::novo(sel.clone(), v.t);
        let mut conds = Vec::new();
        for (lista, _) in casos {
            let mut partes = Vec::new();
            for c in lista {
                partes.push(self.caso(&alvo, c)?);
            }
            conds.push(partes.join(" || "));
        }
        self.linha(format!("const {sel} = {};", v.js));
        for (k, ((_, corpo), c)) in casos.iter().zip(conds).enumerate() {
            self.linha(if k == 0 { format!("if ({c}) {{") } else { format!("}} else if ({c}) {{") });
            self.bloco(corpo);
        }
        if let Some(s) = senao {
            self.linha(if casos.is_empty() { "{".to_string() } else { "} else {".to_string() });
            self.bloco(s);
        }
        if !casos.is_empty() || senao.is_some() {
            self.linha("}");
        }
        Ok(())
    }

    fn caso(&mut self, alvo: &V, c: &Caso) -> R<String> {
        Ok(match c {
            Caso::Valor(x) => {
                let v = self.valor(x)?;
                self.comparar(Op::Igual, alvo, &v).js
            }
            Caso::Comparacao(op, x) => {
                let v = self.valor(x)?;
                self.comparar(*op, alvo, &v).js
            }
            Caso::Faixa(a, b) => {
                let (va, vb) = (self.valor(a)?, self.valor(b)?);
                format!("({} && {})", self.comparar(Op::MaiorIg, alvo, &va).js, self.comparar(Op::MenorIg, alvo, &vb).js)
            }
        })
    }

    /// Abre um laço: devolve o índice da linha do cabeçalho (para pôr o rótulo se precisar).
    fn abrir_laco(&mut self, tipo: &'static str, cabecalho: String) -> usize {
        let id = self.novo_id();
        self.lacos.push(Laco { tipo, id, rotulado: false });
        self.linha(cabecalho);
        self.linhas.len() - 1
    }

    fn fechar_laco(&mut self, linha_do_cabecalho: usize, fecho: &str) {
        let l = self.lacos.pop().expect("laço aberto");
        if l.rotulado {
            let cab = &mut self.linhas[linha_do_cabecalho];
            let recuo = cab.len() - cab.trim_start().len();
            cab.insert_str(recuo, &format!("L{}: ", l.id));
        }
        self.linha(fecho);
    }

    fn para(&mut self, var: &str, de: &Expr, ate: &Expr, passo: Option<&Expr>, corpo: &[Instr]) -> R<()> {
        let v = match self.var(var).or_else(|| self.global(var)) {
            Some(v) => v.js,
            None => self.declarar(var, Tipo::Valor(T::Num), false),
        };
        let (vde, vate) = (self.valor(de)?, self.valor(ate)?);
        let inicio = self.como_num(&vde);
        let fixo = matches!(ate, Expr::Num(_)) || matches!(ate, Expr::Nome(n) if self.var(n).is_some() && !atribuido_em(n, corpo));
        let fim = if fixo {
            self.como_num(&vate)
        } else {
            let id = self.novo_id();
            let f = self.como_num(&vate);
            self.linha(format!("const $fim{id} = {f};"));
            format!("$fim{id}")
        };
        let (cond, inc) = match passo {
            None => (format!("{v} <= {fim}"), format!("{v}++")),
            Some(Expr::Num(p)) if *p >= 0.0 => (format!("{v} <= {fim}"), format!("{v} += {}", num(*p))),
            Some(Expr::Neg(x)) if matches!(**x, Expr::Num(_)) => {
                let Expr::Num(p) = **x else { unreachable!() };
                (format!("{v} >= {fim}"), format!("{v} -= {}", num(p)))
            }
            Some(p) => {
                let vp = self.valor(p)?;
                let id = self.novo_id();
                let pj = self.como_num(&vp);
                self.linha(format!("const $passo{id} = {pj};"));
                (format!("($passo{id} >= 0 ? {v} <= {fim} : {v} >= {fim})"), format!("{v} += $passo{id}"))
            }
        };
        let cab = self.abrir_laco("for", format!("for ({v} = {inicio}; {cond}; {inc}) {{"));
        self.bloco(corpo);
        self.fechar_laco(cab, "}");
        Ok(())
    }

    fn para_cada(&mut self, var: &str, em: &Expr, corpo: &[Instr]) -> R<()> {
        let (lista, tipo) = match self.objeto(em) {
            Ok(Obj::Faixa(f)) => {
                let h = self.ajuda("celulas");
                (format!("vba.{h}({}, {})", f.aba, f.end), Some((Tipo::Faixa(None), Some(f.aba))))
            }
            Ok(Obj::Abas) => (format!("vba.{}()", self.ajuda("abas")), Some((Tipo::Aba, None))),
            Ok(_) => return Err(Motivo::novo("For Each sobre coleção do Excel")),
            Err(m) if matches!(em, Expr::Nome(n) if self.var(n).is_some() || self.global(n).is_some()) => {
                let _ = m;
                (self.valor(em)?.js, None)
            }
            Err(m) => match self.valor(em) {
                Ok(v) => (v.js, None),
                Err(_) => return Err(m),
            },
        };
        let js = match self.var(var) {
            Some(v) => v.js,
            None => self.declarar(var, Tipo::Valor(T::Var), false),
        };
        if let Some((t, aba)) = tipo {
            if let Some(v) = self.escopo.vars.get_mut(&var.to_lowercase()) {
                v.tipo = t;
            }
            if let Some(a) = aba {
                self.escopo.aba_da_faixa.insert(js.clone(), a);
            }
        }
        let cab = self.abrir_laco("for", format!("for ({js} of {lista}) {{"));
        self.bloco(corpo);
        self.fechar_laco(cab, "}");
        Ok(())
    }

    fn condicao(&mut self, c: &(bool, Expr)) -> R<String> {
        let v = self.valor(&c.1)?;
        let b = self.como_bool(&v);
        Ok(if c.0 { format!("!({b})") } else { b })
    }

    fn faca(&mut self, antes: Option<&(bool, Expr)>, depois: Option<&(bool, Expr)>, corpo: &[Instr]) -> R<()> {
        let ca = antes.map(|c| self.condicao(c)).transpose()?;
        let cd = depois.map(|c| self.condicao(c)).transpose()?;
        let (cab, fecho) = match (ca, cd) {
            (Some(c), _) => (format!("while ({c}) {{"), "}".to_string()),
            (None, Some(c)) => ("do {".to_string(), format!("}} while ({c});")),
            (None, None) => ("for (;;) {".to_string(), "}".to_string()),
        };
        let l = self.abrir_laco("do", cab);
        self.bloco(corpo);
        self.fechar_laco(l, &fecho);
        Ok(())
    }

    fn bloco_com(&mut self, obj: &Expr, corpo: &[Instr]) -> R<()> {
        let o = self.objeto(obj)?;
        self.com.push(Ok(o));
        self.linha("{");
        self.bloco(corpo);
        self.linha("}");
        self.com.pop();
        Ok(())
    }

    fn sair(&mut self, o: &str) -> R<String> {
        match o {
            "sub" => Ok("return;".into()),
            "function" | "property" => Ok(if self.escopo.funcao.is_some() { "return $r;".into() } else { "return;".into() }),
            "for" | "do" => {
                let n = self.lacos.len();
                let alvo = self.lacos.iter().rposition(|l| l.tipo == o).ok_or_else(|| Motivo::novo(format!("Exit {o} fora de laço")))?;
                if alvo + 1 == n {
                    return Ok("break;".into());
                }
                self.lacos[alvo].rotulado = true;
                Ok(format!("break L{};", self.lacos[alvo].id))
            }
            _ => Err(Motivo::novo(format!("Exit {o}"))),
        }
    }

    /// Junta as declarações do procedimento inteiro (o VBA declara para o procedimento todo,
    /// não para o bloco), para o `let` sair no topo.
    pub fn coletar_declaracoes(&mut self, corpo: &[Instr]) {
        for i in corpo {
            match &i.k {
                TipoInstr::Dim(ds) | TipoInstr::Const(ds) | TipoInstr::ReDim { decls: ds, .. } => {
                    for d in ds {
                        let tipo = tipo_declarado(d.tipo.as_deref());
                        let js = self.declarar(&d.nome, tipo, d.dims.is_some());
                        if e_inteiro(d.tipo.as_deref()) && d.dims.is_none() {
                            self.escopo.inteiros.insert(js);
                        }
                    }
                }
                TipoInstr::Se { ramos, senao } => {
                    ramos.iter().for_each(|(_, c)| self.coletar_declaracoes(c));
                    if let Some(s) = senao {
                        self.coletar_declaracoes(s);
                    }
                }
                TipoInstr::Escolha { casos, senao, .. } => {
                    casos.iter().for_each(|(_, c)| self.coletar_declaracoes(c));
                    if let Some(s) = senao {
                        self.coletar_declaracoes(s);
                    }
                }
                TipoInstr::Para { corpo, .. } | TipoInstr::ParaCada { corpo, .. } | TipoInstr::Faca { corpo, .. } | TipoInstr::Com { corpo, .. } => self.coletar_declaracoes(corpo),
                _ => {}
            }
        }
    }
}
