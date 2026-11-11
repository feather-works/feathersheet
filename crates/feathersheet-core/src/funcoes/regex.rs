//! Expressão regular escrita à mão, para `REGEXEXTRAIR`, `REGEXCORRESP` e
//! `REGEXSUBSTITUIR`.
//!
//! O TypeScript usava o `RegExp` do navegador. O `sheet-core` não ganha
//! dependência (regra 11), então o que se usa numa planilha veio para cá, com
//! a semântica do JavaScript sem bandeira nenhuma (o TypeScript não passava
//! `i` nem `m`):
//!
//! - literais, `.` (tudo menos quebra de linha), `^` e `$` do texto inteiro;
//! - classes `[a-z0-9_]`, `[^...]`, e os atalhos `\d \w \s \D \W \S`;
//! - `\b` e `\B`, com a "letra" do JavaScript (`[A-Za-z0-9_]`);
//! - grupos `( )`, `(?: )` e `(?<nome> )` — o nome é aceito e ignorado;
//! - alternativa `|`, quantificadores `* + ? {n} {n,} {n,m}` e as versões
//!   preguiçosas com `?`;
//! - referência a grupo `\1`…`\9` e lookahead `(?= )` e `(?! )`.
//!
//! **Não há lookbehind** (`(?<= )`, `(?<! )`): a expressão é recusada, e a
//! função dá `#VALUE!` — onde o navegador aceitaria. É a única diferença
//! conhecida, e está travada por teste.
//!
//! O casamento é por retrocesso, como o do JavaScript, então a resposta é a
//! mesma (o casamento mais à esquerda, e entre as alternativas, a primeira que
//! casa). Uma expressão patológica, que no navegador travaria a aba, aqui para
//! num teto de passos e vira erro — ver [`LIMITE_DE_PASSOS`].

use std::cell::Cell;

/// Quantos passos o casamento pode dar antes de desistir. Uma expressão como
/// `(a+)+b` contra trinta `a` levaria bilhões de passos: o navegador trava, e
/// aqui a função devolve erro.
const LIMITE_DE_PASSOS: u64 = 5_000_000;

#[derive(Clone, Debug)]
enum Item {
    Char(char),
    Range(char, char),
    Digito(bool),
    Palavra(bool),
    Espaco(bool),
}

#[derive(Clone, Debug)]
enum No {
    Vazio,
    Char(char),
    Qualquer,
    Classe { itens: Vec<Item>, negada: bool },
    Inicio,
    Fim,
    Borda(bool),
    Grupo { dentro: Box<No>, captura: Option<usize> },
    Seq(Vec<No>),
    Alt(Vec<No>),
    Repete { dentro: Box<No>, min: usize, max: Option<usize>, guloso: bool },
    Retorno(usize),
    Adiante { dentro: Box<No>, negado: bool },
}

/// Uma expressão já compilada.
#[derive(Clone, Debug)]
pub struct Regex {
    raiz: No,
    grupos: usize,
}

/// Um casamento: o trecho inteiro e os grupos, em posições de CARACTERE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Casamento {
    pub inicio: usize,
    pub fim: usize,
    pub grupos: Vec<Option<(usize, usize)>>,
}

/// Por que a expressão não serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroRegex {
    Sintaxe(String),
    /// Passou do [`LIMITE_DE_PASSOS`].
    Demorada,
}

// ------------------------------------------------------------------- análise

struct Analisador {
    c: Vec<char>,
    i: usize,
    grupos: usize,
}

fn erro<T>(msg: &str) -> Result<T, ErroRegex> {
    Err(ErroRegex::Sintaxe(crate::tr!(msg)))
}

impl Analisador {
    fn espia(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn alternativa(&mut self) -> Result<No, ErroRegex> {
        let mut ramos = vec![self.sequencia()?];
        while self.espia() == Some('|') {
            self.i += 1;
            ramos.push(self.sequencia()?);
        }
        Ok(if ramos.len() == 1 { ramos.pop().unwrap() } else { No::Alt(ramos) })
    }

    fn sequencia(&mut self) -> Result<No, ErroRegex> {
        let mut itens = Vec::new();
        while let Some(ch) = self.espia() {
            if ch == '|' || ch == ')' {
                break;
            }
            let atomo = self.atomo()?;
            let atomo = self.quantificador(atomo)?;
            itens.push(atomo);
        }
        Ok(match itens.len() {
            0 => No::Vazio,
            1 => itens.pop().unwrap(),
            _ => No::Seq(itens),
        })
    }

    /// Lê `{n}`, `{n,}` ou `{n,m}` a partir de `self.i` (no `{`). Devolve
    /// `None` sem mexer na posição quando não é quantificador — aí o `{` é um
    /// caractere comum, como no JavaScript sem a bandeira `u`.
    fn chaves(&mut self) -> Option<(usize, Option<usize>)> {
        let inicio = self.i;
        let mut j = self.i + 1;
        let numero = |j: &mut usize, c: &[char]| -> Option<usize> {
            let k = *j;
            while *j < c.len() && c[*j].is_ascii_digit() {
                *j += 1;
            }
            if *j == k {
                None
            } else {
                c[k..*j].iter().collect::<String>().parse().ok()
            }
        };
        let min = numero(&mut j, &self.c)?;
        let max = if self.c.get(j) == Some(&',') {
            j += 1;
            if self.c.get(j) == Some(&'}') { None } else { Some(numero(&mut j, &self.c)?) }
        } else {
            Some(min)
        };
        if self.c.get(j) != Some(&'}') {
            self.i = inicio;
            return None;
        }
        self.i = j + 1;
        Some((min, max))
    }

    fn quantificador(&mut self, atomo: No) -> Result<No, ErroRegex> {
        let (min, max) = match self.espia() {
            Some('*') => {
                self.i += 1;
                (0, None)
            }
            Some('+') => {
                self.i += 1;
                (1, None)
            }
            Some('?') => {
                self.i += 1;
                (0, Some(1))
            }
            Some('{') => match self.chaves() {
                Some(q) => q,
                None => return Ok(atomo),
            },
            _ => return Ok(atomo),
        };
        if let Some(m) = max {
            if m < min {
                return erro("números fora de ordem no quantificador");
            }
        }
        if matches!(atomo, No::Inicio | No::Fim | No::Borda(_) | No::Adiante { .. }) {
            return erro("nada para repetir");
        }
        let guloso = if self.espia() == Some('?') {
            self.i += 1;
            false
        } else {
            true
        };
        Ok(No::Repete { dentro: Box::new(atomo), min, max, guloso })
    }

    fn atomo(&mut self) -> Result<No, ErroRegex> {
        let ch = self.espia().unwrap();
        self.i += 1;
        match ch {
            '.' => Ok(No::Qualquer),
            '^' => Ok(No::Inicio),
            '$' => Ok(No::Fim),
            '*' | '+' | '?' => erro("nada para repetir"),
            '{' => {
                // `{2}` sozinho repetiria nada; `{a` é só um caractere.
                self.i -= 1;
                if self.chaves().is_some() {
                    return erro("nada para repetir");
                }
                self.i += 1;
                Ok(No::Char('{'))
            }
            '[' => self.classe(),
            '(' => self.grupo(),
            ')' => erro("parêntese fechado sem abrir"),
            '\\' => self.escape_fora(),
            c => Ok(No::Char(c)),
        }
    }

    fn grupo(&mut self) -> Result<No, ErroRegex> {
        let mut captura = true;
        if self.espia() == Some('?') {
            self.i += 1;
            match self.espia() {
                Some(':') => {
                    self.i += 1;
                    captura = false;
                }
                Some('=') | Some('!') => {
                    let negado = self.espia() == Some('!');
                    self.i += 1;
                    let dentro = self.alternativa()?;
                    self.fecha()?;
                    return Ok(No::Adiante { dentro: Box::new(dentro), negado });
                }
                Some('<') => {
                    self.i += 1;
                    if matches!(self.espia(), Some('=') | Some('!')) {
                        return erro("lookbehind não é aceito aqui");
                    }
                    // Grupo com nome: o nome é lido e ignorado.
                    while let Some(c) = self.espia() {
                        self.i += 1;
                        if c == '>' {
                            break;
                        }
                        if !(c.is_alphanumeric() || c == '_' || c == '$') {
                            return erro("nome de grupo inválido");
                        }
                    }
                }
                _ => return erro("grupo inválido"),
            }
        }
        let indice = if captura {
            self.grupos += 1;
            Some(self.grupos)
        } else {
            None
        };
        let dentro = self.alternativa()?;
        self.fecha()?;
        Ok(No::Grupo { dentro: Box::new(dentro), captura: indice })
    }

    fn fecha(&mut self) -> Result<(), ErroRegex> {
        if self.espia() != Some(')') {
            return erro("parêntese aberto sem fechar");
        }
        self.i += 1;
        Ok(())
    }

    /// `\x` fora de classe.
    fn escape_fora(&mut self) -> Result<No, ErroRegex> {
        let Some(c) = self.espia() else { return erro("barra no fim da expressão") };
        self.i += 1;
        Ok(match c {
            'd' => No::Classe { itens: vec![Item::Digito(true)], negada: false },
            'D' => No::Classe { itens: vec![Item::Digito(true)], negada: true },
            'w' => No::Classe { itens: vec![Item::Palavra(true)], negada: false },
            'W' => No::Classe { itens: vec![Item::Palavra(true)], negada: true },
            's' => No::Classe { itens: vec![Item::Espaco(true)], negada: false },
            'S' => No::Classe { itens: vec![Item::Espaco(true)], negada: true },
            'b' => No::Borda(true),
            'B' => No::Borda(false),
            '1'..='9' => No::Retorno(c as usize - '0' as usize),
            _ => No::Char(self.escape_literal(c)?),
        })
    }

    /// O caractere que um escape comum representa (`\n`, `\x41`, `\.`...).
    fn escape_literal(&mut self, c: char) -> Result<char, ErroRegex> {
        let hexa = |a: &mut Analisador, n: usize| -> Option<char> {
            let s: String = a.c.get(a.i..a.i + n)?.iter().collect();
            let v = u32::from_str_radix(&s, 16).ok()?;
            a.i += n;
            char::from_u32(v)
        };
        Ok(match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'f' => '\u{c}',
            'v' => '\u{b}',
            '0' => '\0',
            // Sem os dígitos, `\x` e `\u` são a própria letra (JavaScript
            // sem a bandeira `u`).
            'x' => hexa(self, 2).unwrap_or('x'),
            'u' => hexa(self, 4).unwrap_or('u'),
            outro => outro,
        })
    }

    fn classe(&mut self) -> Result<No, ErroRegex> {
        let negada = if self.espia() == Some('^') {
            self.i += 1;
            true
        } else {
            false
        };
        let mut itens = Vec::new();
        loop {
            let Some(c) = self.espia() else { return erro("colchete aberto sem fechar") };
            self.i += 1;
            if c == ']' {
                break;
            }
            let inicio = if c == '\\' {
                let Some(e) = self.espia() else { return erro("barra no fim da expressão") };
                self.i += 1;
                match e {
                    'd' => {
                        itens.push(Item::Digito(true));
                        continue;
                    }
                    'D' => {
                        itens.push(Item::Digito(false));
                        continue;
                    }
                    'w' => {
                        itens.push(Item::Palavra(true));
                        continue;
                    }
                    'W' => {
                        itens.push(Item::Palavra(false));
                        continue;
                    }
                    's' => {
                        itens.push(Item::Espaco(true));
                        continue;
                    }
                    'S' => {
                        itens.push(Item::Espaco(false));
                        continue;
                    }
                    // Dentro de classe, `\b` é o backspace.
                    'b' => '\u{8}',
                    outro => self.escape_literal(outro)?,
                }
            } else {
                c
            };
            // `a-z` é faixa; `-` no começo ou no fim é o próprio hífen.
            if self.espia() == Some('-') && self.c.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let mut fim = self.espia().unwrap();
                self.i += 1;
                if fim == '\\' {
                    let Some(e) = self.espia() else { return erro("barra no fim da expressão") };
                    self.i += 1;
                    fim = self.escape_literal(e)?;
                }
                if fim < inicio {
                    return erro("faixa fora de ordem na classe");
                }
                itens.push(Item::Range(inicio, fim));
            } else {
                itens.push(Item::Char(inicio));
            }
        }
        Ok(No::Classe { itens, negada })
    }
}

// ------------------------------------------------------------------ casamento

fn eh_palavra(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// O `\s` do JavaScript: os espaços Unicode mais o BOM.
fn eh_espaco(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn quebra_de_linha(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

impl Item {
    fn casa(&self, c: char) -> bool {
        match *self {
            Item::Char(x) => c == x,
            Item::Range(a, b) => a <= c && c <= b,
            Item::Digito(sim) => c.is_ascii_digit() == sim,
            Item::Palavra(sim) => eh_palavra(c) == sim,
            Item::Espaco(sim) => eh_espaco(c) == sim,
        }
    }
}

type Grupos = Vec<Option<(usize, usize)>>;

struct Maquina<'a> {
    texto: &'a [char],
    passos: Cell<u64>,
}

impl Maquina<'_> {
    fn passo(&self) -> Result<(), ErroRegex> {
        let n = self.passos.get() + 1;
        self.passos.set(n);
        if n > LIMITE_DE_PASSOS {
            Err(ErroRegex::Demorada)
        } else {
            Ok(())
        }
    }

    /// Casa `no` a partir de `i` e chama `k` com a posição de depois. O
    /// retrocesso é o `k` devolver `false`: aí a próxima possibilidade é
    /// tentada.
    fn casa(
        &self,
        no: &No,
        i: usize,
        g: &mut Grupos,
        k: &mut dyn FnMut(usize, &mut Grupos) -> Result<bool, ErroRegex>,
    ) -> Result<bool, ErroRegex> {
        self.passo()?;
        let t = self.texto;
        match no {
            No::Vazio => k(i, g),
            No::Char(c) => {
                if t.get(i) == Some(c) {
                    k(i + 1, g)
                } else {
                    Ok(false)
                }
            }
            No::Qualquer => match t.get(i) {
                Some(&c) if !quebra_de_linha(c) => k(i + 1, g),
                _ => Ok(false),
            },
            No::Classe { itens, negada } => match t.get(i) {
                Some(&c) if itens.iter().any(|it| it.casa(c)) != *negada => k(i + 1, g),
                _ => Ok(false),
            },
            No::Inicio => {
                if i == 0 {
                    k(i, g)
                } else {
                    Ok(false)
                }
            }
            No::Fim => {
                if i == t.len() {
                    k(i, g)
                } else {
                    Ok(false)
                }
            }
            No::Borda(sim) => {
                let antes = i > 0 && eh_palavra(t[i - 1]);
                let depois = i < t.len() && eh_palavra(t[i]);
                if (antes != depois) == *sim {
                    k(i, g)
                } else {
                    Ok(false)
                }
            }
            No::Grupo { dentro, captura } => {
                let captura = *captura;
                self.casa(dentro, i, g, &mut |j, g| {
                    let Some(n) = captura else { return k(j, g) };
                    let antes = g[n];
                    g[n] = Some((i, j));
                    if k(j, g)? {
                        return Ok(true);
                    }
                    g[n] = antes;
                    Ok(false)
                })
            }
            No::Seq(itens) => self.sequencia(itens, i, g, k),
            No::Alt(ramos) => {
                for ramo in ramos {
                    let salvo = g.clone();
                    if self.casa(ramo, i, g, k)? {
                        return Ok(true);
                    }
                    *g = salvo;
                }
                Ok(false)
            }
            No::Repete { dentro, min, max, guloso } => self.repete(dentro, *min, *max, *guloso, 0, i, g, k),
            No::Retorno(n) => {
                // Grupo que não casou (ou que não existe) casa com o vazio,
                // como no JavaScript.
                let Some(Some((a, b))) = g.get(*n).copied() else { return k(i, g) };
                let len = b - a;
                if i + len <= t.len() && t[a..b] == t[i..i + len] {
                    k(i + len, g)
                } else {
                    Ok(false)
                }
            }
            No::Adiante { dentro, negado } => {
                let mut dentro_g = g.clone();
                let achou = self.casa(dentro, i, &mut dentro_g, &mut |_, _| Ok(true))?;
                if achou == *negado {
                    return Ok(false);
                }
                // O lookahead positivo guarda os grupos que casou lá dentro.
                if !*negado {
                    *g = dentro_g;
                }
                k(i, g)
            }
        }
    }

    fn sequencia(
        &self,
        itens: &[No],
        i: usize,
        g: &mut Grupos,
        k: &mut dyn FnMut(usize, &mut Grupos) -> Result<bool, ErroRegex>,
    ) -> Result<bool, ErroRegex> {
        match itens.split_first() {
            None => k(i, g),
            Some((primeiro, resto)) => self.casa(primeiro, i, g, &mut |j, g| self.sequencia(resto, j, g, k)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repete(
        &self,
        dentro: &No,
        min: usize,
        max: Option<usize>,
        guloso: bool,
        feitas: usize,
        i: usize,
        g: &mut Grupos,
        k: &mut dyn FnMut(usize, &mut Grupos) -> Result<bool, ErroRegex>,
    ) -> Result<bool, ErroRegex> {
        let pode_parar = feitas >= min;
        if guloso {
            let salvo = g.clone();
            if self.mais_uma(dentro, min, max, guloso, feitas, i, g, k)? {
                return Ok(true);
            }
            *g = salvo;
            if pode_parar {
                return k(i, g);
            }
            Ok(false)
        } else {
            if pode_parar {
                let salvo = g.clone();
                if k(i, g)? {
                    return Ok(true);
                }
                *g = salvo;
            }
            self.mais_uma(dentro, min, max, guloso, feitas, i, g, k)
        }
    }

    /// Mais uma volta da repetição — desde que ela ande: repetir o vazio para
    /// sempre não leva a lugar nenhum (é a mesma regra do JavaScript).
    #[allow(clippy::too_many_arguments)]
    fn mais_uma(
        &self,
        dentro: &No,
        min: usize,
        max: Option<usize>,
        guloso: bool,
        feitas: usize,
        i: usize,
        g: &mut Grupos,
        k: &mut dyn FnMut(usize, &mut Grupos) -> Result<bool, ErroRegex>,
    ) -> Result<bool, ErroRegex> {
        if max.is_some_and(|m| feitas >= m) {
            return Ok(false);
        }
        let k_ref = &mut *k;
        self.casa(dentro, i, g, &mut |j, g| {
            if j == i && feitas >= min {
                return Ok(false);
            }
            self.repete(dentro, min, max, guloso, feitas + 1, j, g, k_ref)
        })
    }
}

// ---------------------------------------------------------------- público

impl Regex {
    /// Compila a expressão; erro de sintaxe onde o `new RegExp` lançaria.
    pub fn new(padrao: &str) -> Result<Regex, ErroRegex> {
        let mut a = Analisador { c: padrao.chars().collect(), i: 0, grupos: 0 };
        let raiz = a.alternativa()?;
        if a.i < a.c.len() {
            return erro("parêntese fechado sem abrir");
        }
        Ok(Regex { raiz, grupos: a.grupos })
    }

    /// Quantos grupos de captura a expressão tem.
    pub fn grupos(&self) -> usize {
        self.grupos
    }

    /// O primeiro casamento a partir da posição `desde` (em caracteres).
    pub fn procura_em(&self, texto: &[char], desde: usize) -> Result<Option<Casamento>, ErroRegex> {
        let m = Maquina { texto, passos: Cell::new(0) };
        for inicio in desde..=texto.len() {
            let mut g: Grupos = vec![None; self.grupos + 1];
            let mut fim = None;
            let achou = m.casa(&self.raiz, inicio, &mut g, &mut |j, _| {
                fim = Some(j);
                Ok(true)
            })?;
            if achou {
                return Ok(Some(Casamento { inicio, fim: fim.unwrap(), grupos: g }));
            }
        }
        Ok(None)
    }

    /// O `exec` do JavaScript: o primeiro casamento no texto.
    pub fn procura(&self, texto: &str) -> Result<Option<Casamento>, ErroRegex> {
        let chars: Vec<char> = texto.chars().collect();
        self.procura_em(&chars, 0)
    }

    /// O `test` do JavaScript.
    pub fn casa(&self, texto: &str) -> Result<bool, ErroRegex> {
        Ok(self.procura(texto)?.is_some())
    }

    /// O `text.replace(new RegExp(padrao, 'g'), troca)` do JavaScript, com os
    /// `$1`, `$&`, `` $` ``, `$'` e `$$` na troca.
    pub fn troca_todos(&self, texto: &str, troca: &str) -> Result<String, ErroRegex> {
        let chars: Vec<char> = texto.chars().collect();
        let troca: Vec<char> = troca.chars().collect();
        let mut saida = String::with_capacity(texto.len());
        let mut pos = 0;
        let mut copiado = 0;
        while pos <= chars.len() {
            let Some(c) = self.procura_em(&chars, pos)? else { break };
            saida.extend(&chars[copiado..c.inicio]);
            self.expande(&troca, &chars, &c, &mut saida);
            copiado = c.fim;
            // Casamento vazio: anda um caractere, senão casaria ali para sempre.
            pos = if c.fim == c.inicio { c.fim + 1 } else { c.fim };
        }
        if copiado < chars.len() {
            saida.extend(&chars[copiado..]);
        }
        Ok(saida)
    }

    fn expande(&self, troca: &[char], texto: &[char], c: &Casamento, saida: &mut String) {
        let mut i = 0;
        while i < troca.len() {
            let ch = troca[i];
            if ch != '$' || i + 1 >= troca.len() {
                saida.push(ch);
                i += 1;
                continue;
            }
            match troca[i + 1] {
                '$' => {
                    saida.push('$');
                    i += 2;
                }
                '&' => {
                    saida.extend(&texto[c.inicio..c.fim]);
                    i += 2;
                }
                '`' => {
                    saida.extend(&texto[..c.inicio]);
                    i += 2;
                }
                '\'' => {
                    saida.extend(&texto[c.fim..]);
                    i += 2;
                }
                d if d.is_ascii_digit() => {
                    // `$12` é o grupo 12 se ele existir; senão `$1` seguido
                    // de "2". `$0` e grupo que não existe ficam como estão.
                    let um = d as usize - '0' as usize;
                    let dois = troca
                        .get(i + 2)
                        .filter(|x| x.is_ascii_digit())
                        .map(|x| um * 10 + (*x as usize - '0' as usize));
                    let (n, largura) = match dois {
                        Some(n) if (1..=self.grupos).contains(&n) => (n, 3),
                        _ => (um, 2),
                    };
                    if (1..=self.grupos).contains(&n) {
                        if let Some(Some((a, b))) = c.grupos.get(n) {
                            saida.extend(&texto[*a..*b]);
                        }
                        i += largura;
                    } else {
                        saida.push('$');
                        i += 1;
                    }
                }
                _ => {
                    saida.push('$');
                    i += 1;
                }
            }
        }
    }
}

/// O trecho de `texto` entre duas posições de caractere.
pub fn trecho(texto: &str, (a, b): (usize, usize)) -> String {
    texto.chars().skip(a).take(b - a).collect()
}

use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, err_na, err_value};
use crate::celulas::values::{to_text, Computed, Value};

/// `=REGEXCORRESP(texto; padrao)` / `=REGEXMATCH(text; pattern)`
pub fn fn_regexmatch(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(e));
    }
    let texto_val = ev.eval(&args[0]);
    if let Value::Error(e) = texto_val {
        return Computed::Scalar(Value::Error(e));
    }
    let texto = to_text(&texto_val);

    let padrao_val = ev.eval(&args[1]);
    if let Value::Error(e) = padrao_val {
        return Computed::Scalar(Value::Error(e));
    }
    let padrao = to_text(&padrao_val);

    match Regex::new(&padrao) {
        Ok(re) => match re.casa(&texto) {
            Ok(b) => Computed::Scalar(Value::Bool(b)),
            Err(_) => err_value(),
        },
        Err(_) => err_value(),
    }
}

/// `=REGEXEXTRAIR(texto; padrao)` / `=REGEXEXTRACT(text; pattern)`
pub fn fn_regexextract(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(e));
    }
    let texto_val = ev.eval(&args[0]);
    if let Value::Error(e) = texto_val {
        return Computed::Scalar(Value::Error(e));
    }
    let texto = to_text(&texto_val);

    let padrao_val = ev.eval(&args[1]);
    if let Value::Error(e) = padrao_val {
        return Computed::Scalar(Value::Error(e));
    }
    let padrao = to_text(&padrao_val);

    match Regex::new(&padrao) {
        Ok(re) => match re.procura(&texto) {
            Ok(Some(casamento)) => {
                let g = casamento.grupos.get(1).copied().flatten().unwrap_or((casamento.inicio, casamento.fim));
                Computed::Scalar(Value::Text(trecho(&texto, g)))
            }
            Ok(None) => err_na(),
            Err(_) => err_value(),
        },
        Err(_) => err_value(),
    }
}

/// `=REGEXSUBSTITUIR(texto; padrao; substituto)` / `=REGEXREPLACE(text; pattern; replacement)`
pub fn fn_regexreplace(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 3, 3) {
        return Computed::Scalar(Value::Error(e));
    }
    let texto_val = ev.eval(&args[0]);
    if let Value::Error(e) = texto_val {
        return Computed::Scalar(Value::Error(e));
    }
    let texto = to_text(&texto_val);

    let padrao_val = ev.eval(&args[1]);
    if let Value::Error(e) = padrao_val {
        return Computed::Scalar(Value::Error(e));
    }
    let padrao = to_text(&padrao_val);

    let sub_val = ev.eval(&args[2]);
    if let Value::Error(e) = sub_val {
        return Computed::Scalar(Value::Error(e));
    }
    let sub = to_text(&sub_val);

    match Regex::new(&padrao) {
        Ok(re) => match re.troca_todos(&texto, &sub) {
            Ok(r) => Computed::Scalar(Value::Text(r)),
            Err(_) => err_value(),
        },
        Err(_) => err_value(),
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    fn extrai(p: &str, t: &str) -> Option<String> {
        let r = Regex::new(p).unwrap();
        r.procura(t).unwrap().map(|c| {
            let g = c.grupos.get(1).copied().flatten().unwrap_or((c.inicio, c.fim));
            trecho(t, g)
        })
    }

    #[test]
    fn literais_classes_e_atalhos() {
        assert_eq!(extrai(r"\d+", "pedido 1234 ok"), Some("1234".into()));
        assert_eq!(extrai(r"[A-Z]{2}\d{3}", "placa: ab ABC1234"), Some("BC123".into()));
        assert_eq!(extrai(r"[^a-z ]+", "abc DEF ghi"), Some("DEF".into()));
        assert_eq!(extrai(r"\w+@\w+\.com", "fale com ana@exemplo.com hoje"), Some("ana@exemplo.com".into()));
        assert_eq!(extrai(r"a.c", "a\nc abc"), Some("abc".into()), "o ponto não pega quebra de linha");
        assert_eq!(extrai(r"\s+", "a \t b"), Some(" \t ".into()));
        assert_eq!(extrai(r"[a\-z]+", "x-a-z"), Some("-a-z".into()));
        assert_eq!(extrai(r"[-.]+", "x.-.y"), Some(".-.".into()));
    }

    #[test]
    fn grupo_de_captura_e_o_que_extrai() {
        assert_eq!(extrai(r"R\$ ?([\d.,]+)", "Total: R$ 1.250,50"), Some("1.250,50".into()));
        assert_eq!(extrai(r"(?:ab)+(c)", "ababc"), Some("c".into()));
        assert_eq!(extrai(r"(?<ano>\d{4})-\d\d", "em 2026-09"), Some("2026".into()));
    }

    #[test]
    fn ganancioso_e_preguicoso() {
        assert_eq!(extrai(r"<.+>", "<a><b>"), Some("<a><b>".into()));
        assert_eq!(extrai(r"<.+?>", "<a><b>"), Some("<a>".into()));
        assert_eq!(extrai(r"a{2,3}", "aaaa"), Some("aaa".into()));
        assert_eq!(extrai(r"a{2,}?", "aaaa"), Some("aa".into()));
        assert_eq!(extrai(r"x{a", "x{a"), Some("x{a".into()), "chave que não é quantificador é caractere");
    }

    #[test]
    fn ancoras_bordas_alternativa() {
        let r = Regex::new(r"^(sim|não)$").unwrap();
        assert!(r.casa("sim").unwrap());
        assert!(!r.casa("simm").unwrap());
        assert_eq!(extrai(r"\bgato\b", "gatos gato"), Some("gato".into()));
        assert_eq!(Regex::new(r"\bgato\b").unwrap().procura("gatos gato").unwrap().unwrap().inicio, 6);
        // Alternativa: vale a PRIMEIRA que casa, não a mais longa.
        assert_eq!(extrai(r"a|ab", "ab"), Some("a".into()));
    }

    #[test]
    fn retorno_e_lookahead() {
        assert_eq!(extrai(r"(\w)\1", "abccd"), Some("c".into()));
        assert_eq!(extrai(r"\d+(?= reais)", "10 dólares, 20 reais"), Some("20".into()));
        assert_eq!(extrai(r"\d+(?! reais)\b", "20 reais 30 dólares"), Some("30".into()));
    }

    #[test]
    fn erros_de_sintaxe() {
        for p in ["(", "a)", "[a", "*a", "a**", r"\", "a{3,1}", "(?<=a)b", "(?<!a)b", "{2}"] {
            assert!(matches!(Regex::new(p), Err(ErroRegex::Sintaxe(_))), "{p} deveria ser recusada");
        }
    }

    #[test]
    fn expressao_patologica_para_no_teto() {
        let r = Regex::new(r"(a+)+b").unwrap();
        assert_eq!(r.procura(&"a".repeat(40)), Err(ErroRegex::Demorada));
    }

    #[test]
    fn repeticao_do_vazio_nao_trava() {
        // Conferido no Node: /(a*)*b/.exec("aab") dá ["aab", "aa"].
        assert_eq!(extrai(r"(a*)*b", "aab"), Some("aa".into()));
        assert!(Regex::new(r"(?:)*x").unwrap().casa("x").unwrap());
    }

    #[test]
    fn troca_como_o_replace_global() {
        let r = Regex::new(r"(\d+)").unwrap();
        assert_eq!(r.troca_todos("a1 b22", "[$1]").unwrap(), "a[1] b[22]");
        assert_eq!(r.troca_todos("a1", "$$-$&-$`-$'").unwrap(), "a$-1-a-");
        assert_eq!(r.troca_todos("a1", "$2$0").unwrap(), "a$2$0", "grupo que não existe fica literal");
        // Casamento vazio anda um caractere, como o JavaScript.
        assert_eq!(Regex::new("x*").unwrap().troca_todos("abc", "-").unwrap(), "-a-b-c-");
        assert_eq!(Regex::new(r"\s+").unwrap().troca_todos("  a  b ", " ").unwrap(), " a b ");
        // `$12` com um grupo só é o grupo 1 seguido de "2".
        assert_eq!(r.troca_todos("x5", "$12").unwrap(), "x52");
    }

    #[test]
    fn acentos_contam_como_um_caractere() {
        assert_eq!(extrai(r"ç(.)o", "ação"), Some("ã".into()));
        assert_eq!(extrai(r"a.ão", "ação"), Some("ação".into()));
        assert_eq!(Regex::new("ã").unwrap().troca_todos("ação e mão", "a").unwrap(), "açao e mao");
    }
}
