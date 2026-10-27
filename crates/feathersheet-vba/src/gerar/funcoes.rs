//! As constantes (`vbYes`, `xlUp`, `vbCrLf`) e as funções embutidas do VBA (`Len`, `Mid`,
//! `Format`, `MsgBox`…), traduzidas para JavaScript ou para uma função `vba.*`.

use super::contexto::Gerador;
use super::tipos::{Motivo, R, T, V};

/// (nome em minúsculas, valor JS, tipo).
const CONSTANTES: &[(&str, &str, T)] = &[
    ("vbyes", "6", T::Num), ("vbno", "7", T::Num), ("vbok", "1", T::Num), ("vbcancel", "2", T::Num), ("vbabort", "3", T::Num), ("vbretry", "4", T::Num), ("vbignore", "5", T::Num),
    ("vbokonly", "0", T::Num), ("vbokcancel", "1", T::Num), ("vbabortretryignore", "2", T::Num), ("vbyesnocancel", "3", T::Num), ("vbyesno", "4", T::Num), ("vbretrycancel", "5", T::Num),
    ("vbcritical", "16", T::Num), ("vbquestion", "32", T::Num), ("vbexclamation", "48", T::Num), ("vbinformation", "64", T::Num), ("vbdefaultbutton1", "0", T::Num), ("vbdefaultbutton2", "256", T::Num),
    ("vbcrlf", "'\\n'", T::Texto), ("vbnewline", "'\\n'", T::Texto), ("vblf", "'\\n'", T::Texto), ("vbcr", "'\\r'", T::Texto), ("vbtab", "'\\t'", T::Texto), ("vbnullstring", "''", T::Texto), ("vbnullchar", "'\\0'", T::Texto),
    ("vbblack", "0", T::Num), ("vbwhite", "16777215", T::Num), ("vbred", "255", T::Num), ("vbgreen", "65280", T::Num), ("vbblue", "16711680", T::Num), ("vbyellow", "65535", T::Num), ("vbmagenta", "16711935", T::Num), ("vbcyan", "16776960", T::Num),
    ("vbbinarycompare", "0", T::Num), ("vbtextcompare", "1", T::Num), ("vbsunday", "1", T::Num), ("vbmonday", "2", T::Num), ("vbtuesday", "3", T::Num), ("vbwednesday", "4", T::Num), ("vbthursday", "5", T::Num), ("vbfriday", "6", T::Num), ("vbsaturday", "7", T::Num), ("vbusesystemdayofweek", "0", T::Num),
    ("vbuppercase", "1", T::Num), ("vblowercase", "2", T::Num), ("vbpropercase", "3", T::Num),
    ("xldelimited", "1", T::Num), ("xlfixedwidth", "2", T::Num), ("xldoublequote", "1", T::Num), ("xlsinglequote", "2", T::Num),
    ("xltextqualifierdoublequote", "1", T::Num), ("xltextqualifiersinglequote", "2", T::Num), ("xltextqualifiernone", "-4142", T::Num),
    ("xlup", "-4162", T::Num), ("xldown", "-4121", T::Num), ("xltoleft", "-4159", T::Num), ("xltoright", "-4161", T::Num),
    ("xlcenter", "-4108", T::Num), ("xlleft", "-4131", T::Num), ("xlright", "-4152", T::Num), ("xlgeneral", "1", T::Num), ("xltop", "-4160", T::Num), ("xlbottom", "-4107", T::Num), ("xljustify", "-4130", T::Num), ("xlcenteracrossselection", "7", T::Num),
    ("xlnone", "-4142", T::Num), ("xlautomatic", "-4105", T::Num), ("xlcontinuous", "1", T::Num), ("xldash", "-4115", T::Num), ("xldot", "-4118", T::Num), ("xldouble", "-4119", T::Num), ("xlhairline", "1", T::Num), ("xlthin", "2", T::Num), ("xlmedium", "-4138", T::Num), ("xlthick", "4", T::Num),
    ("xldiagonaldown", "5", T::Num), ("xldiagonalup", "6", T::Num), ("xledgeleft", "7", T::Num), ("xledgetop", "8", T::Num), ("xledgebottom", "9", T::Num), ("xledgeright", "10", T::Num), ("xlinsidevertical", "11", T::Num), ("xlinsidehorizontal", "12", T::Num),
    ("xlsolid", "1", T::Num), ("xlunderlinestylesingle", "2", T::Num), ("xlunderlinestylenone", "-4142", T::Num), ("xlunderlinestyledouble", "-4119", T::Num),
    ("xlpastevalues", "-4163", T::Num), ("xlpasteformats", "-4122", T::Num), ("xlpasteall", "-4104", T::Num), ("xlpasteformulas", "-4123", T::Num), ("xlpastevaluesandnumberformats", "12", T::Num), ("xlcellvalue", "1", T::Num), ("xlexpression", "2", T::Num),
    ("xlyes", "1", T::Num), ("xlno", "2", T::Num), ("xlguess", "0", T::Num), ("xlascending", "1", T::Num), ("xldescending", "2", T::Num),
    ("xlsheetvisible", "-1", T::Num), ("xlsheethidden", "0", T::Num), ("xlsheetveryhidden", "2", T::Num), ("xlcalculationautomatic", "-4105", T::Num), ("xlcalculationmanual", "-4135", T::Num),
    ("xlcelltypelastcell", "11", T::Num), ("xlcelltypeblanks", "4", T::Num), ("xlcelltypeconstants", "2", T::Num), ("xlcelltypeformulas", "-4123", T::Num), ("xlcelltypevisible", "12", T::Num),
    ("xlshiftup", "-4162", T::Num), ("xlshifttoleft", "-4159", T::Num), ("xlshiftdown", "-4121", T::Num), ("xlshifttoright", "-4161", T::Num),
    ("xlvalidateinputonly", "0", T::Num), ("xlvalidatewholenumber", "1", T::Num), ("xlvalidatedecimal", "2", T::Num), ("xlvalidatelist", "3", T::Num), ("xlvalidatedate", "4", T::Num), ("xlvalidatetime", "5", T::Num), ("xlvalidatetextlength", "6", T::Num), ("xlvalidatecustom", "7", T::Num),
    ("xlvalidalertstop", "1", T::Num), ("xlvalidalertwarning", "2", T::Num), ("xlvalidalertinformation", "3", T::Num),
    ("xlbetween", "1", T::Num), ("xlnotbetween", "2", T::Num), ("xlequal", "3", T::Num), ("xlnotequal", "4", T::Num), ("xlgreater", "5", T::Num), ("xlless", "6", T::Num), ("xlgreaterequal", "7", T::Num), ("xllessequal", "8", T::Num),
    ("xlsortonvalues", "0", T::Num), ("xlsortnormal", "0", T::Num), ("xlsorttextasnumbers", "1", T::Num), ("xltoptobottom", "1", T::Num), ("xllefttoright", "2", T::Num), ("xlpinyin", "1", T::Num), ("xlstroke", "2", T::Num),
    ("xltypepdf", "0", T::Num), ("xltypexps", "1", T::Num), ("xlqualitystandard", "0", T::Num), ("xlqualityminimum", "1", T::Num), ("xlopenxmlworkbook", "51", T::Num), ("xlworkbookdefault", "51", T::Num),
    ("xlthemecolordark1", "1", T::Num), ("xlthemecolorlight1", "2", T::Num), ("xlthemecoloraccent1", "5", T::Num),
];

/// Uma constante do VBA ou do Excel.
pub fn constante(nome: &str) -> Option<V> {
    let n = nome.to_lowercase();
    CONSTANTES.iter().find(|(c, _, _)| *c == n).map(|(_, js, t)| V::novo(*js, *t))
}

/// O número de uma constante (`xlEdgeBottom` → 9), para decidir na conversão.
pub fn valor_da_constante(nome: &str) -> Option<i64> {
    constante(nome).and_then(|v| v.js.parse().ok())
}

/// Número como JS (`3`, `0.5`).
pub fn num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Texto como literal JS.
pub fn texto(s: &str) -> String {
    format!("{s:?}")
}

impl Gerador<'_> {
    /// Força número (o argumento já é número, ou passa por `vba.n`).
    pub fn como_num(&mut self, v: &V) -> String {
        if v.t == T::Num {
            v.par()
        } else {
            format!("vba.{}({})", self.ajuda("n"), v.js)
        }
    }

    /// Força texto.
    pub fn como_texto(&mut self, v: &V) -> String {
        if v.t == T::Texto {
            v.par()
        } else {
            format!("vba.{}({})", self.ajuda("str"), v.js)
        }
    }

    /// Força verdadeiro/falso (condição de `If`, `While`).
    pub fn como_bool(&mut self, v: &V) -> String {
        if v.t == T::Bool {
            v.js.clone()
        } else {
            format!("vba.{}({})", self.ajuda("bool"), v.js)
        }
    }

    fn chamar_ajuda(&mut self, nome: &'static str, args: &[V], t: T) -> V {
        let a: Vec<&str> = args.iter().map(|v| v.js.as_str()).collect();
        V::novo(format!("vba.{}({})", self.ajuda(nome), a.join(", ")), t)
    }

    /// Uma função embutida do VBA sem argumento (`Now`, `Date`, `Timer`), ou `None`.
    pub fn embutida_sem_args(&mut self, nome: &str) -> Option<V> {
        Some(match nome.to_lowercase().as_str() {
            "now" => V::novo(format!("vba.{}()", self.ajuda("agora")), T::Num),
            "date" => V::novo(format!("vba.{}()", self.ajuda("hoje")), T::Num),
            "time" => V::composto(format!("vba.{}() % 1", self.ajuda("agora")), T::Num),
            "timer" => V::composto("(Date.now() % 86400000) / 1000", T::Num),
            "rnd" => V::novo("Math.random()", T::Num),
            "doevents" | "beep" => V::novo("undefined", T::Var),
            _ => return None,
        })
    }

    /// Uma função embutida com argumentos; `None` se não é embutida.
    pub fn embutida(&mut self, nome: &str, a: &[V]) -> Option<R<V>> {
        let n = nome.to_lowercase();
        if let Some(v) = self.embutida_de_texto(&n, a) {
            return Some(Ok(v));
        }
        if let Some(v) = self.embutida_de_numero(&n, a) {
            return Some(Ok(v));
        }
        self.embutida_outras(&n, a)
    }

    fn embutida_de_texto(&mut self, n: &str, a: &[V]) -> Option<V> {
        let t = |g: &mut Self, i: usize| a.get(i).map(|v| g.como_texto(v)).unwrap_or_default();
        Some(match n {
            "len" => self.chamar_ajuda("len", a, T::Num),
            "mid" => self.chamar_ajuda("mid", a, T::Texto),
            "left" => self.chamar_ajuda("left", a, T::Texto),
            "right" => self.chamar_ajuda("right", a, T::Texto),
            "instr" => self.chamar_ajuda("instr", a, T::Num),
            "instrrev" => self.chamar_ajuda("instrRev", a, T::Num),
            "replace" => self.chamar_ajuda("replace", a, T::Texto),
            "trim" => self.chamar_ajuda("trim", a, T::Texto),
            "ltrim" => self.chamar_ajuda("ltrim", a, T::Texto),
            "rtrim" => self.chamar_ajuda("rtrim", a, T::Texto),
            "ucase" => V::novo(format!("{}.toUpperCase()", t(self, 0)), T::Texto),
            "lcase" => V::novo(format!("{}.toLowerCase()", t(self, 0)), T::Texto),
            "cstr" | "str" => V::novo(t(self, 0), T::Texto),
            "split" => V::novo(format!("{}.split({})", t(self, 0), a.get(1).map_or("' '".to_string(), |v| v.js.clone())), T::Matriz),
            "join" => V::novo(format!("{}.join({})", a.first()?.par(), a.get(1).map_or("' '".to_string(), |v| v.js.clone())), T::Texto),
            "chr" | "chrw" => V::novo(format!("String.fromCharCode({})", a.first()?.js), T::Texto),
            "asc" | "ascw" => V::novo(format!("{}.charCodeAt(0)", t(self, 0)), T::Num),
            "space" => V::novo(format!("' '.repeat({})", a.first()?.js), T::Texto),
            "string" => V::novo(format!("{}.charAt(0).repeat({})", t(self, 1), a.first()?.js), T::Texto),
            "strreverse" => V::novo(format!("[...{}].reverse().join('')", t(self, 0)), T::Texto),
            "format" => self.chamar_ajuda("format", a, T::Texto),
            "isnumeric" => self.chamar_ajuda("isNumeric", a, T::Bool),
            "isempty" => self.chamar_ajuda("vazio", a, T::Bool),
            "isnull" => V::composto(format!("{} === null", a.first()?.par()), T::Bool),
            "isarray" => V::novo(format!("Array.isArray({})", a.first()?.js), T::Bool),
            "val" => self.chamar_ajuda("val", a, T::Num),
            _ => return None,
        })
    }

    fn embutida_de_numero(&mut self, n: &str, a: &[V]) -> Option<V> {
        let x = |g: &mut Self| a.first().map(|v| g.como_num(v)).unwrap_or_default();
        Some(match n {
            "cint" | "clng" | "clnglng" | "cbyte" => self.chamar_ajuda("cint", a, T::Num),
            "cdbl" | "csng" | "ccur" | "cdec" => V::novo(x(self), T::Num),
            "cbool" => {
                let b = self.como_bool(a.first()?);
                V::novo(b, T::Bool)
            }
            "int" => V::novo(format!("Math.floor({})", x(self)), T::Num),
            "fix" => V::novo(format!("Math.trunc({})", x(self)), T::Num),
            "abs" => V::novo(format!("Math.abs({})", x(self)), T::Num),
            "sqr" => V::novo(format!("Math.sqrt({})", x(self)), T::Num),
            "sgn" => V::novo(format!("Math.sign({})", x(self)), T::Num),
            "exp" => V::novo(format!("Math.exp({})", x(self)), T::Num),
            "log" => V::novo(format!("Math.log({})", x(self)), T::Num),
            "sin" | "cos" | "tan" | "atn" => V::novo(format!("Math.{}({})", if n == "atn" { "atan" } else { n }, x(self)), T::Num),
            "round" => self.chamar_ajuda("round", a, T::Num),
            "rgb" => V::novo(format!("app.rgb({})", a.iter().map(|v| v.js.as_str()).collect::<Vec<_>>().join(", ")), T::Num),
            "year" => self.chamar_ajuda("ano", a, T::Num),
            "month" => self.chamar_ajuda("mes", a, T::Num),
            "day" => self.chamar_ajuda("dia", a, T::Num),
            "ubound" => self.chamar_ajuda("ubound", a, T::Num),
            "lbound" => self.chamar_ajuda("lbound", a, T::Num),
            _ => return None,
        })
    }

    fn embutida_outras(&mut self, n: &str, a: &[V]) -> Option<R<V>> {
        Some(Ok(match n {
            "msgbox" => self.chamar_ajuda("msgBox", a, T::Num),
            "inputbox" => self.chamar_ajuda("inputBox", a, T::Texto),
            "array" => V::novo(format!("[{}]", a.iter().map(|v| v.js.as_str()).collect::<Vec<_>>().join(", ")), T::Matriz),
            "iif" if a.len() == 3 => {
                let c = self.como_bool(&a[0]);
                V::composto(format!("{c} ? {} : {}", a[1].par(), a[2].par()), T::Var)
            }
            "now" | "date" | "time" | "timer" => self.embutida_sem_args(n)?,
            "createobject" | "getobject" => return Some(Err(Motivo::novo("automação de outro programa (CreateObject)"))),
            "shell" => return Some(Err(Motivo::novo("rodar programa do sistema (Shell)"))),
            "dir" | "freefile" | "eof" | "filelen" | "filedatetime" | "curdir" => return Some(Err(Motivo::novo("acesso a arquivo"))),
            "environ" => return Some(Err(Motivo::novo("variável de ambiente do Windows"))),
            "evaluate" => return Some(Err(Motivo::novo("Evaluate (avaliar fórmula em texto)"))),
            "typename" | "vartype" => return Some(Err(Motivo::novo(format!("{n} (tipo do VBA)")))),
            "dateserial" => self.chamar_ajuda("serial", a, T::Num),
            "timeserial" => V::novo(format!("(({}) * 3600 + ({}) * 60 + ({})) / 86400", a.first()?.js, a.get(1)?.js, a.get(2)?.js), T::Num),
            "datevalue" => V::novo(format!("Math.floor(vba.{}({}))", self.ajuda("cdate"), a.first()?.js), T::Num),
            "timevalue" => V::composto(format!("vba.{}({}) % 1", self.ajuda("cdate"), a.first()?.js), T::Num),
            "cdate" => self.chamar_ajuda("cdate", a, T::Num),
            "weekday" => self.chamar_ajuda("weekday", a, T::Num),
            "hour" => self.chamar_ajuda("hora", a, T::Num),
            "minute" => self.chamar_ajuda("minuto", a, T::Num),
            "second" => self.chamar_ajuda("segundo", a, T::Num),
            "dateadd" => self.chamar_ajuda("dateadd", a, T::Num),
            "datediff" => self.chamar_ajuda("datediff", a, T::Num),
            "datepart" => self.chamar_ajuda("datepart", a, T::Num),
            "strconv" => self.chamar_ajuda("strconv", a, T::Texto),
            _ => return None,
        }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn constantes_e_literais() {
        assert_eq!(constante("vbYesNo").unwrap().js, "4");
        assert_eq!(valor_da_constante("xlEdgeBottom"), Some(9));
        assert_eq!(num(3.0), "3");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(texto("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
