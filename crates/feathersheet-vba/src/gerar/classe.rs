//! Módulos de classe do VBA (`Class1`, `New Class1`), achados na bateria de `.xlsm` reais (#143): eram a
//! maior fatia do que não convertia. Cada objeto vira um objeto JS simples com os campos (as variáveis do
//! módulo); cada método vira uma `function Classe_metodo($eu, …)` que entra na macro como qualquer
//! procedimento (a fila de `Macro::pedir`), e `novo_Classe()` monta o objeto e roda o `Class_Initialize`.
//! `Property Get`/`Let`/`Set` viram `Classe_get_x($eu)`/`Classe_set_x($eu, v)`.
// `Type … End Type` usa a mesma estrutura (módulo sintético só com campos, `InfoModulo::registro`).
// shortcut: o registro é copiado por referência (`a = b` no VBA copia os campos); sem matriz de registros.
// shortcut: sem `Class_Terminate`, `Implements`, eventos de classe (`WithEvents`/`RaiseEvent`) nem `Is`/`TypeOf`
// com a classe; o tipo do objeto se perde quando passa por função ou por `Variant` (o membro então vira pendência).

use super::contexto::{js_nome, tipo_declarado, valor_inicial, Gerador};
use super::tipos::{Motivo, Tipo, R, T, V};
use crate::sintaxe::{Arg, Expr, TipoProc};

impl Gerador<'_> {
    /// O valor inicial de uma variável: o registro (`Type`) já nasce com os campos.
    pub fn inicial(&mut self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Objeto(c) if self.p.modulos[*c].registro => self.criar_objeto(*c),
            _ => valor_inicial(tipo).to_string(),
        }
    }

    /// O módulo de classe com esse nome.
    pub fn classe(&self, nome: &str) -> Option<usize> {
        (0..self.p.modulos.len()).find(|&i| self.p.e_classe(i) && self.p.modulos[i].nome.eq_ignore_ascii_case(nome))
    }

    /// O tipo pelo `As …`, sabendo das classes do projeto.
    pub fn tipo_de(&self, t: Option<&str>) -> Tipo {
        match t.and_then(|t| self.classe(t)) {
            Some(c) => Tipo::Objeto(c),
            None => tipo_declarado(t),
        }
    }

    /// `novo_Classe()`, pondo a fábrica na macro na primeira vez.
    pub fn criar_objeto(&mut self, c: usize) -> String {
        let classe = self.p.modulos[c].nome.clone();
        let nome = js_nome(&format!("novo_{classe}"));
        if !self.m.fabricas.contains_key(&nome) {
            // Reserva antes de montar: um campo `As New` da própria classe não entra em laço.
            self.m.fabricas.insert(nome.clone(), String::new());
            self.m.topo.insert(nome.clone());
            let decls = self.p.modulos[c].ast.variaveis.clone();
            let mut campos = Vec::new();
            for d in &decls {
                let tipo = self.tipo_de(d.tipo.as_deref());
                let inicial = match (&tipo, d.novo, d.dims.is_some()) {
                    (_, _, true) => "[]".to_string(),
                    (Tipo::Objeto(o), true, _) => self.criar_objeto(*o),
                    (_, true, _) => d.tipo.as_deref().and_then(|t| self.criar_colecao(&Expr::Novo(t.to_string()))).unwrap_or_else(|| "null".into()),
                    _ => self.inicial(&tipo),
                };
                campos.push(format!("{}: {inicial}", js_nome(&d.nome)));
            }
            let origem = if self.p.modulos[c].registro { "Type" } else { "Módulo de classe" };
            let mut corpo = vec![format!("// {origem} {classe}"), format!("function {nome}() {{"), format!("  const $eu = {{ {} }};", campos.join(", "))];
            if let Some(r) = self.metodo_da_classe(c, "class_initialize", &[TipoProc::Sub]) {
                let ini = self.m.pedir(self.p, r);
                corpo.push(format!("  {ini}($eu);"));
            }
            corpo.push("  return $eu;".into());
            corpo.push("}".into());
            self.m.fabricas.insert(nome.clone(), corpo.join("\n"));
        }
        format!("{nome}()")
    }

    /// O procedimento `nome` da classe `c`, de um desses tipos.
    pub fn metodo_da_classe(&self, c: usize, nome: &str, tipos: &[TipoProc]) -> Option<(usize, usize)> {
        self.p.modulos[c].ast.procs.iter().position(|p| p.nome.eq_ignore_ascii_case(nome) && tipos.contains(&p.tipo)).map(|i| (c, i))
    }

    /// A classe de um valor: `New Classe`, variável `As Classe`, `Me` dentro da classe, função `As Classe`.
    pub fn classe_de_expr(&mut self, e: &Expr) -> Option<usize> {
        match e {
            Expr::Novo(n) => self.classe(n),
            Expr::Paren(x) => self.classe_de_expr(x),
            _ => self.var_objeto(e).map(|(_, c)| c).or_else(|| {
                let nome = match e { Expr::Nome(n) => n, Expr::Chamada(f, _) => match &**f { Expr::Nome(n) => n, _ => return None }, _ => return None };
                let r = self.p.achar(self.modulo, nome)?;
                let ret = self.p.proc(r).retorno.clone();
                self.classe(ret.as_deref()?)
            }),
        }
    }

    /// (JS, classe) de uma variável que guarda objeto de classe, ou de `Me` dentro da classe.
    pub fn var_objeto(&mut self, e: &Expr) -> Option<(String, usize)> {
        let Expr::Nome(n) = e else { return None };
        if n.eq_ignore_ascii_case("me") && self.p.e_classe(self.modulo) {
            return Some(("$eu".into(), self.modulo));
        }
        match self.var(n).or_else(|| self.global(n))? {
            v if !v.matriz => match v.tipo { Tipo::Objeto(c) => Some((v.js, c)), _ => None },
            _ => None,
        }
    }

    /// `obj.Metodo(args)`, `obj.Propriedade`, `obj.campo` como valor.
    pub fn membro_classe(&mut self, js: &str, c: usize, nome: &str, args: &[Arg]) -> R<V> {
        if let Some(r) = self.metodo_da_classe(c, nome, &[TipoProc::Funcao, TipoProc::PropriedadeLer, TipoProc::Sub]) {
            return self.chamar_proc_de(r, args, Some(js.to_string()));
        }
        let decl = self.p.modulos[c].ast.variaveis.iter().find(|d| d.nome.eq_ignore_ascii_case(nome)).cloned();
        let Some(d) = decl else {
            return Err(Motivo::novo(feathersheet_core::trf!("{nome} não é membro da classe {}", self.p.modulos[c].nome)));
        };
        let mut base = format!("{js}.{}", js_nome(&d.nome));
        // `obj.lista(i)`: item da matriz do campo.
        for a in self.valores(args)? {
            base.push_str(&format!("[{}]", if a.t == T::Num { a.js.clone() } else { self.como_num(&a) }));
        }
        let t = match self.tipo_de(d.tipo.as_deref()) {
            Tipo::Valor(t) if d.dims.is_none() || !args.is_empty() => t,
            Tipo::Valor(_) => T::Matriz,
            _ => T::Obj,
        };
        Ok(V::novo(base, t))
    }

    /// `obj.Metodo args` como instrução.
    pub fn chamar_classe(&mut self, js: &str, c: usize, nome: &str, args: &[Arg]) -> R<String> {
        Ok(format!("{};", self.membro_classe(js, c, nome, args)?.js))
    }

    /// `obj.Propriedade = v` (o `Property Let`/`Set`) ou `obj.campo = v`.
    pub fn atribuir_classe(&mut self, js: &str, c: usize, nome: &str, valor: &Expr) -> R<String> {
        if let Some(r) = self.metodo_da_classe(c, nome, &[TipoProc::PropriedadeEscrever]) {
            let mut args = vec![Arg { nome: None, valor: None }; self.p.proc(r).params.len().saturating_sub(1)];
            args.push(Arg { nome: None, valor: Some(valor.clone()) });
            return Ok(format!("{};", self.chamar_proc_de(r, &args, Some(js.to_string()))?.js));
        }
        if self.p.modulos[c].ast.variaveis.iter().any(|d| d.nome.eq_ignore_ascii_case(nome)) {
            let v = self.valor(valor)?;
            return Ok(format!("{js}.{} = {};", js_nome(nome), v.js));
        }
        Err(Motivo::novo(feathersheet_core::trf!("{nome} não é membro da classe {}", self.p.modulos[c].nome)))
    }
}

#[cfg(test)]
mod testes {
    /// Classe com campo, `Class_Initialize`, propriedade (Get/Let), método e `Me`, usada de um módulo comum.
    #[test]
    fn modulo_de_classe_vira_objeto_com_metodos() {
        let conta = "Attribute VB_Name = \"Conta\"\n\
            Private saldo_ As Double\n\
            Public dono As String\n\
            Private Sub Class_Initialize()\n saldo_ = 10\nEnd Sub\n\
            Public Property Get Saldo() As Double\n Saldo = saldo_\nEnd Property\n\
            Public Property Let Saldo(v As Double)\n saldo_ = v\nEnd Property\n\
            Public Sub Depositar(x As Double)\n Me.Saldo = Me.Saldo + x\nEnd Sub\n\
            Public Function Dobro() As Double\n Dobro = saldo_ * 2\nEnd Function\n";
        let modulo = "Sub T()\n\
            Dim c As Conta\n\
            Set c = New Conta\n\
            c.dono = \"Ana\"\n\
            c.Depositar 5\n\
            c.Saldo = c.Saldo + 1\n\
            Range(\"A1\").Value = c.Dobro() & c.dono\n\
            End Sub\n";
        let c = crate::converter_texto_com_classes(&[("Modulo1", modulo)], &[("Conta", conta)]);
        assert!(c.pendencias.iter().all(|p| p.aviso), "{:#?}", c.pendencias);
        let js = &c.macros.iter().find(|m| m.nome == "T").expect("macro T").codigo;
        for trecho in ["function novo_Conta()", "Conta_Class_Initialize($eu)", "c = novo_Conta();", "c.dono = \"Ana\"", "Conta_Depositar(c, 5)",
            "Conta_set_Saldo(c, Conta_get_Saldo(c) + 1)", "$eu.saldo_ = v", "Conta_set_Saldo($eu, Conta_get_Saldo($eu) + x)"] {
            assert!(js.contains(trecho), "falta {trecho:?} em:\n{js}");
        }
    }
}
