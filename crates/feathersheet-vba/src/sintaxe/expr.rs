//! Expressões do VBA, por precedência (do mais forte ao mais fraco): `^`, `-` unário,
//! `* /`, `\`, `Mod`, `+ -`, `&`, comparações (`= <> < > <= >= Like Is`), `Not`, `And`,
//! `Or`, `Xor`, `Eqv`, `Imp`.

use super::{Arg, Expr, Op};
use crate::lexico::{Ficha, Tok};

/// O cursor sobre as fichas, compartilhado com a análise das instruções.
pub(super) struct Leitor {
    pub t: Vec<Tok>,
    pub i: usize,
    /// Expressões abertas uma dentro da outra agora (parênteses, `-`, `Not`).
    pub profundidade: u32,
    /// Nós da expressão de fora; zera a cada expressão nova.
    pub nos: u32,
}

/// Teto da profundidade da árvore de uma expressão (#129), que o gerador percorre
/// recursivamente: cada `(`, `-`, `Not`, operador encadeado (`a + b + c`) e membro
/// encadeado (`a.b.c`) conta um nível.
const MAX_PROFUNDIDADE: u32 = 512;
/// Teto de nós de uma expressão. Larga e rasa é macro de verdade: a linha lógica do VBA vai a
/// ~25 mil caracteres com `_`, como uma matriz de 17 `Array(…)` de 31 números (bateria #143).
const MAX_NOS: u32 = 20_000;

/// Erro de análise: a instrução vira `Desconhecida`.
pub(super) type Res<T> = Result<T, ()>;

impl Leitor {
    pub fn olha(&self) -> &Ficha {
        self.t.get(self.i).map_or(&Ficha::Fim, |t| &t.f)
    }

    pub fn olha_em(&self, d: usize) -> &Ficha {
        self.t.get(self.i + d).map_or(&Ficha::Fim, |t| &t.f)
    }

    pub fn linha(&self) -> usize {
        self.t.get(self.i).or_else(|| self.t.last()).map_or(1, |t| t.linha)
    }

    pub fn acabou(&self) -> bool {
        self.i >= self.t.len()
    }

    pub fn avanca(&mut self) -> Ficha {
        let f = self.olha().clone();
        self.i += 1;
        f
    }

    /// A ficha atual é a palavra `p` (sem diferenciar maiúsculas)?
    pub fn e(&self, p: &str) -> bool {
        matches!(self.olha(), Ficha::Nome(n) if n.eq_ignore_ascii_case(p))
    }

    pub fn e_em(&self, d: usize, p: &str) -> bool {
        matches!(self.olha_em(d), Ficha::Nome(n) if n.eq_ignore_ascii_case(p))
    }

    /// Consome a palavra `p`, se for ela.
    pub fn aceita(&mut self, p: &str) -> bool {
        let sim = self.e(p);
        if sim {
            self.i += 1;
        }
        sim
    }

    pub fn e_op(&self, op: &str) -> bool {
        matches!(self.olha(), Ficha::Op(o) if *o == op)
    }

    pub fn aceita_op(&mut self, op: &str) -> bool {
        let sim = self.e_op(op);
        if sim {
            self.i += 1;
        }
        sim
    }

    pub fn exige_op(&mut self, op: &str) -> Res<()> {
        if self.aceita_op(op) {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Fim da instrução: quebra de linha, `:` ou o fim do código.
    pub fn fim_da_instrucao(&self) -> bool {
        matches!(self.olha(), Ficha::Fim | Ficha::Sep)
    }

    pub fn nome(&mut self) -> Res<String> {
        match self.avanca() {
            Ficha::Nome(n) => Ok(n),
            _ => Err(()),
        }
    }

    // ------------------------------------------------------------------ expressões

    /// Uma expressão completa.
    pub fn expr(&mut self) -> Res<Expr> {
        if self.profundidade == 0 {
            self.nos = 0;
        }
        self.binaria(0)
    }

    /// Conta um nó; passou do teto, a instrução vira `Desconhecida`.
    fn no(&mut self) -> Res<()> {
        self.nos += 1;
        if self.nos > MAX_NOS || self.profundidade > MAX_PROFUNDIDADE { Err(()) } else { Ok(()) }
    }

    /// O operador binário na posição atual e a força dele (maior = liga mais).
    fn operador(&self) -> Option<(Op, u8)> {
        let palavra = |p: &str| self.e(p);
        let op = match self.olha() {
            Ficha::Op("^") => (Op::Pot, 12),
            Ficha::Op("*") => (Op::Mul, 10),
            Ficha::Op("/") => (Op::Div, 10),
            Ficha::Op("\\") => (Op::DivInt, 9),
            _ if palavra("mod") => (Op::Mod, 8),
            Ficha::Op("+") => (Op::Soma, 7),
            Ficha::Op("-") => (Op::Sub, 7),
            Ficha::Op("&") => (Op::Concat, 6),
            Ficha::Op("=") => (Op::Igual, 5),
            Ficha::Op("<>") => (Op::Dif, 5),
            Ficha::Op("<") => (Op::Menor, 5),
            Ficha::Op(">") => (Op::Maior, 5),
            Ficha::Op("<=") => (Op::MenorIg, 5),
            Ficha::Op(">=") => (Op::MaiorIg, 5),
            _ if palavra("like") => (Op::Like, 5),
            _ if palavra("is") => (Op::Is, 5),
            _ if palavra("and") => (Op::E, 3),
            _ if palavra("or") => (Op::Ou, 2),
            _ if palavra("xor") => (Op::Xor, 1),
            _ if palavra("eqv") => (Op::Eqv, 1),
            _ if palavra("imp") => (Op::Imp, 0),
            _ => return None,
        };
        Some(op)
    }

    fn binaria(&mut self, minimo: u8) -> Res<Expr> {
        let mut esq = self.unaria()?;
        // Cada operador encadeado aprofunda a árvore à esquerda.
        let mut niveis = 0;
        let r = loop {
            let Some((op, forca)) = self.operador() else { break Ok(()) };
            if forca < minimo {
                break Ok(());
            }
            self.i += 1;
            niveis += 1;
            self.profundidade += 1;
            // `^` é associativo à esquerda no VBA, como os outros.
            match self.binaria(forca + 1) {
                Ok(dir) => esq = Expr::Bin(op, Box::new(esq), Box::new(dir)),
                Err(()) => break Err(()),
            }
        };
        self.profundidade -= niveis;
        r.map(|()| esq)
    }

    fn unaria(&mut self) -> Res<Expr> {
        self.no()?;
        self.profundidade += 1;
        let e = self.unaria_dentro();
        self.profundidade -= 1;
        e
    }

    fn unaria_dentro(&mut self) -> Res<Expr> {
        if self.aceita("not") {
            // `Not` liga mais fraco que as comparações: `Not a = b` é `Not (a = b)`.
            return Ok(Expr::Nao(Box::new(self.binaria(4)?)));
        }
        if self.aceita_op("-") {
            // O `-` unário liga mais fraco que `^`: `-2 ^ 2` = -4.
            return Ok(Expr::Neg(Box::new(self.binaria(11)?)));
        }
        if self.aceita_op("+") {
            return self.binaria(11);
        }
        self.posfixa()
    }

    /// O primário e a corrente de `.membro`, `(argumentos)` e `!nome`.
    pub fn posfixa(&mut self) -> Res<Expr> {
        if self.profundidade == 0 {
            self.nos = 0;
        }
        let mut e = self.primaria()?;
        // Cada membro/chamada encadeado aprofunda a árvore.
        let mut niveis = 0;
        let r: Res<()> = loop {
            if self.no().is_err() { break Err(()); }
            niveis += 1;
            self.profundidade += 1;
            let passo = if self.aceita_op(".") {
                self.nome().map(|n| e = Expr::Membro(Box::new(std::mem::replace(&mut e, Expr::Nada)), n))
            } else if self.e_op("(") {
                self.i += 1;
                self.argumentos(")").map(|a| e = Expr::Chamada(Box::new(std::mem::replace(&mut e, Expr::Nada)), a))
            } else if self.e_op("!") && matches!(self.olha_em(1), Ficha::Nome(_)) {
                self.i += 1;
                self.nome().map(|n| e = Expr::Bang(Box::new(std::mem::replace(&mut e, Expr::Nada)), n))
            } else {
                break Ok(());
            };
            if passo.is_err() { break Err(()); }
        };
        self.profundidade -= niveis;
        r.map(|()| e)
    }

    fn primaria(&mut self) -> Res<Expr> {
        match self.avanca() {
            Ficha::Num(n) => Ok(Expr::Num(n)),
            Ficha::Texto(s) => Ok(Expr::Texto(s)),
            Ficha::Data(d) => Ok(Expr::Data(d)),
            Ficha::Op("(") => {
                let e = self.expr()?;
                self.exige_op(")")?;
                Ok(Expr::Paren(Box::new(e)))
            }
            // `.Membro` dentro de `With`.
            Ficha::Op(".") => Ok(Expr::MembroDoCom(self.nome()?)),
            Ficha::Nome(n) => self.palavra(n),
            _ => Err(()),
        }
    }

    fn palavra(&mut self, n: String) -> Res<Expr> {
        Ok(match n.to_lowercase().as_str() {
            "true" => Expr::Bool(true),
            "false" => Expr::Bool(false),
            "nothing" => Expr::Nada,
            "empty" => Expr::Vazio,
            "null" => Expr::Nulo,
            "new" => {
                let mut classe = self.nome()?;
                while self.aceita_op(".") {
                    classe = format!("{classe}.{}", self.nome()?);
                }
                Expr::Novo(classe)
            }
            "typeof" => {
                let e = self.posfixa()?;
                if !self.aceita("is") {
                    return Err(());
                }
                let mut tipo = self.nome()?;
                while self.aceita_op(".") {
                    tipo = format!("{tipo}.{}", self.nome()?);
                }
                Expr::TipoDe(Box::new(e), tipo)
            }
            _ => Expr::Nome(n),
        })
    }

    /// Argumentos até `fecha` (`)`) ou até o fim da instrução (`fecha` = ""), aceitando
    /// nomeados (`Nome:=valor`) e omitidos (`f(1, , 3)`).
    pub fn argumentos(&mut self, fecha: &str) -> Res<Vec<Arg>> {
        let mut args = Vec::new();
        let terminou = |l: &Leitor| if fecha.is_empty() { l.fim_da_instrucao() } else { l.e_op(fecha) };
        if terminou(self) {
            if !fecha.is_empty() {
                self.i += 1;
            }
            return Ok(args);
        }
        loop {
            if self.e_op(",") || terminou(self) {
                args.push(Arg { nome: None, valor: None });
            } else if matches!(self.olha(), Ficha::Nome(_)) && matches!(self.olha_em(1), Ficha::Op(":=")) {
                let nome = self.nome()?;
                self.i += 1;
                args.push(Arg { nome: Some(nome), valor: Some(self.expr()?) });
            } else {
                // `ByVal` explícito na chamada (raro) não muda nada aqui.
                self.aceita("byval");
                args.push(Arg { nome: None, valor: Some(self.expr()?) });
            }
            if self.aceita_op(",") {
                continue;
            }
            if terminou(self) {
                if !fecha.is_empty() {
                    self.i += 1;
                }
                return Ok(args);
            }
            return Err(());
        }
    }
}
