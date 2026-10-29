//! Instruções, blocos e o módulo. Cada instrução que falha na análise é recuperada: o
//! cursor vai até o fim dela e ela vira `Desconhecida` com o texto original.

use super::expr::{Leitor, Res};
use super::{Caso, Decl, Expr, Instr, Modulo, Op, Param, Proc, TipoInstr, TipoProc};
use crate::lexico::{self, Ficha};

/// Instruções de arquivo e de E/S antigas: não têm como entrar no JavaScript da planilha.
const SEM_EQUIVALENTE: [&str; 15] = ["open", "close", "line", "print", "input", "get", "put", "kill", "mkdir", "rmdir", "chdir", "filecopy", "seek", "write", "reset"];

/// Onde um bloco termina.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bloco {
    Proc,
    Se,
    Escolha,
    Para,
    Faca,
    Enquanto,
    Com,
}

struct Analisador {
    l: Leitor,
    linhas: Vec<String>,
    /// Blocos abertos agora (`If` dentro de `For` dentro de `With`…).
    blocos: u32,
}

/// Teto de blocos aninhados (#129): o de dentro disso vira `Desconhecida`, sem estourar a pilha.
const MAX_BLOCOS: u32 = 100;

impl Analisador {
    fn pular_fins(&mut self) {
        while matches!(self.l.olha(), Ficha::Fim | Ficha::Sep) && !self.l.acabou() {
            self.l.i += 1;
        }
    }

    /// Vai até o fim da instrução atual (sem consumir o `Fim`).
    fn ate_o_fim(&mut self) {
        while !self.l.fim_da_instrucao() && !self.l.acabou() {
            self.l.i += 1;
        }
    }

    fn texto_das_linhas(&self, de: usize, ate: usize) -> String {
        let fim = ate.max(de).min(self.linhas.len());
        self.linhas.get(de.saturating_sub(1)..fim).map(|v| v.iter().map(|s| s.trim()).collect::<Vec<_>>().join(" ")).unwrap_or_default()
    }

    /// A linha da última ficha consumida.
    fn linha_anterior(&self) -> usize {
        self.l.t.get(self.l.i.saturating_sub(1)).map_or(1, |t| t.linha)
    }

    /// `End Xxx`: o fechamento de um bloco (e não o `End` sozinho, que termina o programa).
    fn e_fim_de(&self, p: &str) -> bool {
        self.l.e("end") && self.l.e_em(1, p)
    }

    fn fecha(&self, b: Bloco) -> bool {
        let fim_de_proc = self.e_fim_de("sub") || self.e_fim_de("function") || self.e_fim_de("property");
        fim_de_proc
            || match b {
                Bloco::Proc => false,
                Bloco::Se => self.l.e("else") || self.l.e("elseif") || self.e_fim_de("if"),
                Bloco::Escolha => self.l.e("case") || self.e_fim_de("select"),
                Bloco::Para => self.l.e("next"),
                Bloco::Faca => self.l.e("loop"),
                Bloco::Enquanto => self.l.e("wend"),
                Bloco::Com => self.e_fim_de("with"),
            }
    }

    fn bloco(&mut self, b: Bloco) -> Vec<Instr> {
        let mut v = Vec::new();
        loop {
            self.pular_fins();
            if self.l.acabou() || self.fecha(b) {
                return v;
            }
            v.push(self.instr());
        }
    }

    /// Uma instrução; se a análise falhar, vira `Desconhecida` e o cursor vai ao fim dela.
    fn instr(&mut self) -> Instr {
        let inicio = self.l.i;
        let linha = self.l.linha();
        self.blocos += 1;
        let tipo = if self.blocos > MAX_BLOCOS { Err(()) } else { self.tipo_instr() };
        self.blocos -= 1;
        match tipo {
            Ok(k) => Instr { k, linha, ate: self.linha_anterior().max(linha) },
            Err(()) => {
                self.l.i = inicio;
                self.ate_o_fim();
                let ate = self.linha_anterior().max(linha);
                Instr { k: TipoInstr::Desconhecida(self.texto_das_linhas(linha, ate)), linha, ate }
            }
        }
    }

    fn tipo_instr(&mut self) -> Res<TipoInstr> {
        let palavra = match self.l.olha() {
            Ficha::Nome(n) => n.to_lowercase(),
            Ficha::Op(".") => return self.simples(),
            _ => return Err(()),
        };
        let segue_atribuicao = matches!(self.l.olha_em(1), Ficha::Op("=" | "." | "("));
        match palavra.as_str() {
            "dim" | "static" | "private" | "public" | "global" => {
                self.l.i += 1;
                if self.l.aceita("const") {
                    return Ok(TipoInstr::Const(self.decls(true)?));
                }
                Ok(TipoInstr::Dim(self.decls(false)?))
            }
            "const" => {
                self.l.i += 1;
                Ok(TipoInstr::Const(self.decls(true)?))
            }
            "redim" => {
                self.l.i += 1;
                let preservar = self.l.aceita("preserve");
                Ok(TipoInstr::ReDim { preservar, decls: self.decls(false)? })
            }
            "set" | "let" if !segue_atribuicao => {
                self.l.i += 1;
                let alvo = self.l.posfixa()?;
                self.l.exige_op("=")?;
                Ok(TipoInstr::Atribuir { alvo, valor: self.l.expr()?, set: palavra == "set" })
            }
            // `Attribute Macro1.VB_ProcData…` que o editor do Excel deixa dentro da Sub.
            "attribute" => {
                self.ate_o_fim();
                Ok(TipoInstr::Dim(Vec::new()))
            }
            "call" => {
                self.l.i += 1;
                Ok(TipoInstr::Chamar(self.l.posfixa()?))
            }
            "if" => self.se(),
            "select" if self.l.e_em(1, "case") => self.escolha(),
            "for" if self.l.e_em(1, "each") => self.para_cada(),
            "for" => self.para(),
            "do" => self.faca(),
            "while" => self.enquanto(),
            "with" => self.com(),
            _ => self.palavra_solta(&palavra, segue_atribuicao),
        }
    }

    fn palavra_solta(&mut self, palavra: &str, segue_atribuicao: bool) -> Res<TipoInstr> {
        match palavra {
            "exit" => {
                self.l.i += 1;
                Ok(TipoInstr::Sair(self.l.nome()?.to_lowercase()))
            }
            "end" | "stop" if !segue_atribuicao && self.l.fim_da_instrucao_em(1) => {
                self.l.i += 1;
                Ok(TipoInstr::Terminar)
            }
            "goto" => {
                self.l.i += 1;
                Ok(TipoInstr::IrPara(self.rotulo_ou_numero()?))
            }
            "on" if self.l.e_em(1, "error") => {
                self.l.i += 2;
                Ok(TipoInstr::EmErro(self.resto_em_minusculas()))
            }
            "resume" => {
                self.l.i += 1;
                Ok(TipoInstr::EmErro(format!("resume {}", self.resto_em_minusculas())))
            }
            p if SEM_EQUIVALENTE.contains(&p) && !segue_atribuicao => Err(()),
            _ if matches!(self.l.olha_em(1), Ficha::Op(":")) => {
                let r = self.l.nome()?;
                self.l.i += 1;
                Ok(TipoInstr::Rotulo(r))
            }
            _ => self.simples(),
        }
    }

    fn rotulo_ou_numero(&mut self) -> Res<String> {
        match self.l.avanca() {
            Ficha::Nome(n) => Ok(n),
            Ficha::Num(n) => Ok(n.to_string()),
            _ => Err(()),
        }
    }

    fn resto_em_minusculas(&mut self) -> String {
        let mut partes = Vec::new();
        while !self.l.fim_da_instrucao() && !self.l.acabou() {
            match self.l.avanca() {
                Ficha::Nome(n) => partes.push(n.to_lowercase()),
                Ficha::Num(n) => partes.push(n.to_string()),
                _ => {}
            }
        }
        partes.join(" ")
    }

    /// Atribuição (`x = 1`, `Range("A1") = 2`) ou chamada (`Foo a, b`, `x.Select`).
    fn simples(&mut self) -> Res<TipoInstr> {
        let alvo = self.l.posfixa()?;
        if self.l.aceita_op("=") {
            return Ok(TipoInstr::Atribuir { alvo, valor: self.l.expr()?, set: false });
        }
        if self.l.fim_da_instrucao() {
            return Ok(TipoInstr::Chamar(alvo));
        }
        // `MsgBox ("a"), vbYesNo`: o parêntese era do primeiro argumento, não da chamada.
        if self.l.e_op(",") {
            if let Expr::Chamada(f, mut args) = alvo {
                if args.len() == 1 && args[0].nome.is_none() {
                    self.l.i += 1;
                    args.extend(self.l.argumentos("")?);
                    return Ok(TipoInstr::Chamar(Expr::Chamada(f, args)));
                }
            }
            return Err(());
        }
        let args = self.l.argumentos("")?;
        Ok(TipoInstr::Chamar(Expr::Chamada(Box::new(alvo), args)))
    }

    // ------------------------------------------------------------------ declarações

    /// O nome de um tipo, com ponto (`Word.Application`) e o `* 10` do texto de tamanho fixo.
    fn tipo(&mut self) -> Res<String> {
        let mut t = self.l.nome()?;
        while self.l.aceita_op(".") {
            t = format!("{t}.{}", self.l.nome()?);
        }
        if self.l.aceita_op("*") {
            self.l.avanca();
        }
        Ok(t.to_lowercase())
    }

    fn limites(&mut self) -> Res<Vec<(Option<Expr>, Expr)>> {
        let mut dims = Vec::new();
        if self.l.aceita_op(")") {
            return Ok(dims);
        }
        loop {
            let a = self.l.expr()?;
            if self.l.aceita("to") {
                dims.push((Some(a), self.l.expr()?));
            } else {
                dims.push((None, a));
            }
            if self.l.aceita_op(")") {
                return Ok(dims);
            }
            self.l.exige_op(",")?;
        }
    }

    /// `a As Long, b(10) As String, c As New Coll` (com `= valor` quando `constante`).
    fn decls(&mut self, constante: bool) -> Res<Vec<Decl>> {
        let mut v = Vec::new();
        loop {
            self.l.aceita("withevents");
            let nome = self.l.nome()?;
            let dims = if self.l.aceita_op("(") { Some(self.limites()?) } else { None };
            let (mut tipo, mut novo) = (None, false);
            if self.l.aceita("as") {
                novo = self.l.aceita("new");
                tipo = Some(self.tipo()?);
            }
            let valor = if constante && self.l.aceita_op("=") { Some(self.l.expr()?) } else { None };
            v.push(Decl { nome, tipo, dims, novo, valor });
            if !self.l.aceita_op(",") {
                return Ok(v);
            }
        }
    }

    // ------------------------------------------------------------------ blocos

    fn se(&mut self) -> Res<TipoInstr> {
        self.l.i += 1;
        let cond = self.l.expr()?;
        if !self.l.aceita("then") {
            return Err(());
        }
        if !matches!(self.l.olha(), Ficha::Fim) {
            return self.se_de_uma_linha(cond);
        }
        let mut ramos = vec![(cond, self.bloco(Bloco::Se))];
        let mut senao = None;
        loop {
            if self.l.aceita("elseif") {
                let c = self.l.expr()?;
                self.l.aceita("then");
                ramos.push((c, self.bloco(Bloco::Se)));
            } else if self.l.aceita("else") {
                // `Else If …` na mesma linha é um `If` aninhado (de uma linha ou bloco).
                senao = Some(self.bloco(Bloco::Se));
            } else if self.e_fim_de("if") {
                self.l.i += 2;
                return Ok(TipoInstr::Se { ramos, senao });
            } else {
                return Err(());
            }
        }
    }

    /// `If c Then a: b Else d` numa linha só.
    fn se_de_uma_linha(&mut self, cond: Expr) -> Res<TipoInstr> {
        let mut entao = Vec::new();
        let mut senao: Option<Vec<Instr>> = None;
        loop {
            if self.l.aceita("else") {
                senao.get_or_insert_with(Vec::new);
                continue;
            }
            if matches!(self.l.olha(), Ficha::Fim) || self.l.acabou() {
                break;
            }
            if self.l.aceita_op(":") || matches!(self.l.olha(), Ficha::Sep) {
                self.l.i += 1;
                continue;
            }
            let i = self.instr_da_linha()?;
            match senao.as_mut() {
                Some(s) => s.push(i),
                None => entao.push(i),
            }
        }
        Ok(TipoInstr::Se { ramos: vec![(cond, entao)], senao })
    }

    /// Uma instrução dentro do `If` de uma linha: para no `Else` também.
    fn instr_da_linha(&mut self) -> Res<Instr> {
        let linha = self.l.linha();
        let k = self.tipo_instr()?;
        Ok(Instr { k, linha, ate: linha })
    }

    fn escolha(&mut self) -> Res<TipoInstr> {
        self.l.i += 2;
        let expr = self.l.expr()?;
        let mut casos = Vec::new();
        let mut senao = None;
        loop {
            self.pular_fins();
            if self.e_fim_de("select") {
                self.l.i += 2;
                return Ok(TipoInstr::Escolha { expr, casos, senao });
            }
            if !self.l.aceita("case") {
                return Err(());
            }
            if self.l.aceita("else") {
                senao = Some(self.bloco(Bloco::Escolha));
                continue;
            }
            let lista = self.casos()?;
            casos.push((lista, self.bloco(Bloco::Escolha)));
        }
    }

    fn casos(&mut self) -> Res<Vec<Caso>> {
        let mut v = Vec::new();
        loop {
            if self.l.aceita("is") {
                let op = match self.l.avanca() {
                    Ficha::Op("=") => Op::Igual,
                    Ficha::Op("<>") => Op::Dif,
                    Ficha::Op("<") => Op::Menor,
                    Ficha::Op(">") => Op::Maior,
                    Ficha::Op("<=") => Op::MenorIg,
                    Ficha::Op(">=") => Op::MaiorIg,
                    _ => return Err(()),
                };
                v.push(Caso::Comparacao(op, self.l.expr()?));
            } else {
                let a = self.l.expr()?;
                if self.l.aceita("to") {
                    v.push(Caso::Faixa(a, self.l.expr()?));
                } else {
                    v.push(Caso::Valor(a));
                }
            }
            if !self.l.aceita_op(",") {
                return Ok(v);
            }
        }
    }

    /// Consome o `Next [var]` (ou `Next a, b`: só o primeiro fecha este laço).
    fn fecha_next(&mut self) -> Res<()> {
        if !self.l.aceita("next") {
            return Err(());
        }
        if matches!(self.l.olha(), Ficha::Nome(_)) {
            self.l.i += 1;
            if self.l.aceita_op(",") {
                // `Next j, i`: deixa um `Next i` para o laço de fora.
                self.l.i -= 1;
                let t = self.l.t[self.l.i].linha;
                self.l.t[self.l.i] = lexico::Tok { f: Ficha::Nome("Next".into()), linha: t };
            }
        }
        Ok(())
    }

    fn para(&mut self) -> Res<TipoInstr> {
        self.l.i += 1;
        let var = self.l.nome()?;
        self.l.exige_op("=")?;
        let de = self.l.expr()?;
        if !self.l.aceita("to") {
            return Err(());
        }
        let ate = self.l.expr()?;
        let passo = if self.l.aceita("step") { Some(self.l.expr()?) } else { None };
        let corpo = self.bloco(Bloco::Para);
        self.fecha_next()?;
        Ok(TipoInstr::Para { var, de, ate, passo, corpo })
    }

    fn para_cada(&mut self) -> Res<TipoInstr> {
        self.l.i += 2;
        let var = self.l.nome()?;
        if !self.l.aceita("in") {
            return Err(());
        }
        let em = self.l.expr()?;
        let corpo = self.bloco(Bloco::Para);
        self.fecha_next()?;
        Ok(TipoInstr::ParaCada { var, em, corpo })
    }

    fn condicao_de_laco(&mut self) -> Res<Option<(bool, Expr)>> {
        if self.l.aceita("while") {
            return Ok(Some((false, self.l.expr()?)));
        }
        if self.l.aceita("until") {
            return Ok(Some((true, self.l.expr()?)));
        }
        Ok(None)
    }

    fn faca(&mut self) -> Res<TipoInstr> {
        self.l.i += 1;
        let antes = self.condicao_de_laco()?;
        let corpo = self.bloco(Bloco::Faca);
        if !self.l.aceita("loop") {
            return Err(());
        }
        let depois = self.condicao_de_laco()?;
        Ok(TipoInstr::Faca { antes, depois, corpo })
    }

    fn enquanto(&mut self) -> Res<TipoInstr> {
        self.l.i += 1;
        let cond = self.l.expr()?;
        let corpo = self.bloco(Bloco::Enquanto);
        if !self.l.aceita("wend") {
            return Err(());
        }
        Ok(TipoInstr::Faca { antes: Some((false, cond)), depois: None, corpo })
    }

    fn com(&mut self) -> Res<TipoInstr> {
        self.l.i += 1;
        let obj = self.l.expr()?;
        let corpo = self.bloco(Bloco::Com);
        if !self.e_fim_de("with") {
            return Err(());
        }
        self.l.i += 2;
        Ok(TipoInstr::Com { obj, corpo })
    }

    // ------------------------------------------------------------------ módulo

    fn params(&mut self) -> Res<Vec<Param>> {
        let mut v = Vec::new();
        if !self.l.aceita_op("(") || self.l.aceita_op(")") {
            return Ok(v);
        }
        loop {
            let opcional = self.l.aceita("optional");
            let por_valor = self.l.aceita("byval");
            self.l.aceita("byref");
            self.l.aceita("paramarray");
            let nome = self.l.nome()?;
            let matriz = self.l.aceita_op("(") && self.l.aceita_op(")");
            let tipo = if self.l.aceita("as") { Some(self.tipo()?) } else { None };
            let padrao = if self.l.aceita_op("=") { Some(self.l.expr()?) } else { None };
            v.push(Param { nome, por_valor, opcional, padrao, tipo, matriz });
            if self.l.aceita_op(")") {
                return Ok(v);
            }
            self.l.exige_op(",")?;
        }
    }

    fn proc(&mut self, publico: bool) -> Res<Proc> {
        let linha = self.l.linha();
        let tipo = match self.l.nome()?.to_lowercase().as_str() {
            "sub" => TipoProc::Sub,
            "function" => TipoProc::Funcao,
            "property" if self.l.aceita("get") => TipoProc::PropriedadeLer,
            "property" if self.l.aceita("let") || self.l.aceita("set") => TipoProc::PropriedadeEscrever,
            _ => return Err(()),
        };
        let nome = self.l.nome()?;
        let params = self.params()?;
        let retorno = if self.l.aceita("as") { Some(self.tipo()?) } else { None };
        self.ate_o_fim();
        let corpo = self.bloco(Bloco::Proc);
        if self.l.aceita("end") {
            self.l.i += 1;
        }
        Ok(Proc { nome, tipo, publico, params, retorno, corpo, linha })
    }

    /// Pula `Type … End Type` / `Enum … End Enum`.
    fn pular_ate_fim_de(&mut self, p: &str) {
        while !self.l.acabou() && !self.e_fim_de(p) {
            self.l.i += 1;
        }
        self.l.i += 2;
    }

    fn item_do_modulo(&mut self, m: &mut Modulo) -> Res<()> {
        let linha = self.l.linha();
        let mut publico = true;
        while let Some(p) = ["public", "private", "friend", "global", "static"].into_iter().find(|p| self.l.e(p)) {
            publico = p != "private";
            self.l.i += 1;
        }
        if self.l.e("sub") || self.l.e("function") || self.l.e("property") {
            m.procs.push(self.proc(publico)?);
        } else if self.l.aceita("declare") {
            self.l.aceita("ptrsafe");
            self.l.nome()?;
            m.externas.push((self.l.nome()?, linha));
            self.ate_o_fim();
        } else if self.l.e("type") || self.l.e("enum") {
            let p = self.l.nome()?.to_lowercase();
            self.pular_ate_fim_de(&p);
            m.ignorados.push((linha, format!("{p} (bloco de módulo)")));
        } else if self.l.aceita("const") {
            m.constantes.extend(self.decls(true)?);
        } else if self.l.aceita("dim") || matches!(self.l.olha(), Ficha::Nome(_)) && self.linha_anterior() == linha && self.l.i > 0 {
            m.variaveis.extend(self.decls(false)?);
        } else if self.l.e("option") || self.l.e("attribute") || self.l.e("implements") {
            self.ate_o_fim();
        } else {
            return Err(());
        }
        Ok(())
    }
}

impl Leitor {
    fn fim_da_instrucao_em(&self, d: usize) -> bool {
        matches!(self.olha_em(d), Ficha::Fim | Ficha::Sep)
    }
}

/// Analisa o código de um módulo VBA.
pub fn analisar(fonte: &str) -> Modulo {
    let mut a = Analisador { l: Leitor { t: lexico::fichas(fonte), i: 0, profundidade: 0, nos: 0 }, linhas: fonte.lines().map(String::from).collect(), blocos: 0 };
    let mut m = Modulo::default();
    loop {
        a.pular_fins();
        if a.l.acabou() {
            return m;
        }
        let inicio = a.l.i;
        let linha = a.l.linha();
        if a.item_do_modulo(&mut m).is_err() {
            a.l.i = inicio;
            a.ate_o_fim();
            let ate = a.linha_anterior().max(linha);
            m.ignorados.push((linha, a.texto_das_linhas(linha, ate)));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn corpo(codigo: &str) -> Vec<Instr> {
        let m = analisar(&format!("Sub T()\n{codigo}\nEnd Sub\n"));
        assert_eq!(m.procs.len(), 1, "{m:#?}");
        m.procs[0].corpo.clone()
    }

    #[test]
    fn procedimentos_parametros_e_variaveis_do_modulo() {
        let m = analisar("Attribute VB_Name = \"M\"\nOption Explicit\nPublic total As Long\nPrivate Const PI = 3.14\nPublic Function Dobro(ByVal x As Double, Optional y = 2) As Double\n  Dobro = x * y\nEnd Function\nPrivate Sub Botao_Clique()\nEnd Sub\nPublic Declare PtrSafe Function ShellExecute _\n Lib \"shell32.dll\" (ByVal h As Long) As Long\n");
        assert_eq!(m.procs.len(), 2);
        assert_eq!(m.procs[0].tipo, TipoProc::Funcao);
        assert_eq!(m.procs[0].params[1].padrao, Some(Expr::Num(2.0)));
        assert!(!m.procs[1].publico);
        assert_eq!(m.variaveis[0].nome, "total");
        assert_eq!(m.constantes[0].nome, "PI");
        assert_eq!(m.externas[0].0, "ShellExecute");
        assert!(m.ignorados.is_empty(), "{:?}", m.ignorados);
    }

    #[test]
    fn se_de_bloco_e_de_uma_linha() {
        let c = corpo("If a = 1 Then\n  b = 2\nElseIf a = 2 Then\n  b = 3\nElse\n  b = 4\nEnd If\nIf x Then Exit Sub\nIf y Then z = 1: w = 2 Else z = 0");
        let TipoInstr::Se { ramos, senao } = &c[0].k else { panic!("{c:?}") };
        assert_eq!(ramos.len(), 2);
        assert!(senao.is_some());
        let TipoInstr::Se { ramos, .. } = &c[1].k else { panic!() };
        assert_eq!(ramos[0].1[0].k, TipoInstr::Sair("sub".into()));
        let TipoInstr::Se { ramos, senao } = &c[2].k else { panic!() };
        assert_eq!(ramos[0].1.len(), 2);
        assert_eq!(senao.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn chamadas_sem_parenteses_com_nomeados_e_parentese_no_primeiro() {
        let c = corpo("MsgBox (\"oi\"), vbYesNo\nSheet4.ExportAsFixedFormat Type:=xlTypePDF, Filename:=(d & \"x.pdf\")\nCall Foo(Target)\nRange(\"A1\").Select\n.Text = \"#NOME#\"");
        assert!(c.iter().all(|i| !matches!(i.k, TipoInstr::Desconhecida(_))), "{c:#?}");
        let TipoInstr::Chamar(Expr::Chamada(_, args)) = &c[0].k else { panic!() };
        assert_eq!(args.len(), 2);
        let TipoInstr::Chamar(Expr::Chamada(_, args)) = &c[1].k else { panic!() };
        assert_eq!(args[1].nome.as_deref(), Some("Filename"));
        assert!(matches!(c[4].k, TipoInstr::Atribuir { alvo: Expr::MembroDoCom(_), .. }));
    }

    #[test]
    fn lacos_com_e_escolha() {
        let c = corpo("For i = 1 To 10 Step 2\n x = x + i\nNext i\nFor Each c In Range(\"A1:A3\")\nNext\nDo While n < 5\n n = n + 1\nLoop\nWhile a\nWend\nWith Selection.Borders(xlEdgeBottom)\n .LineStyle = xlContinuous\nEnd With\nSelect Case v\nCase 1, 2\n  a = 1\nCase 3 To 5, Is > 10\n  a = 2\nCase Else\n  a = 3\nEnd Select");
        assert!(matches!(c[0].k, TipoInstr::Para { .. }));
        assert!(matches!(c[1].k, TipoInstr::ParaCada { .. }));
        assert!(matches!(c[2].k, TipoInstr::Faca { antes: Some((false, _)), .. }));
        assert!(matches!(c[3].k, TipoInstr::Faca { .. }));
        assert!(matches!(c[4].k, TipoInstr::Com { .. }));
        let TipoInstr::Escolha { casos, senao, .. } = &c[5].k else { panic!("{c:#?}") };
        assert_eq!(casos.len(), 2);
        assert_eq!(casos[1].0.len(), 2);
        assert!(senao.is_some());
    }

    #[test]
    fn else_com_dois_pontos_nao_e_rotulo() {
        let c = corpo("If a Then\n b = 1\nElse:\n b = 2\nEnd If");
        let TipoInstr::Se { senao, .. } = &c[0].k else { panic!("{c:#?}") };
        assert_eq!(senao.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn instrucao_ruim_vira_desconhecida_sem_perder_o_resto() {
        let c = corpo("Open f For Input As #1\nLine Input #1, s\nx = 1");
        assert!(matches!(&c[0].k, TipoInstr::Desconhecida(t) if t.starts_with("Open")));
        assert!(matches!(&c[1].k, TipoInstr::Desconhecida(_)));
        assert!(matches!(c[2].k, TipoInstr::Atribuir { .. }));
    }
}
