//! Prefixos `_xlfn.` / `_xlfn._xlws.` das "future functions" do Excel.
//!
//! O arquivo `.xlsx` guarda as funções posteriores ao Excel 2007 com prefixo
//! (`_xlfn.XLOOKUP`); sem ele o Excel mostra `#NAME?`. `FILTER` e `SORT`
//! levam `_xlfn._xlws.`. Parâmetros de `LET`/`LAMBDA` levam `_xlpm.`.

/// Funções que levam `_xlfn.` (lista da documentação "Issue: An _xlfn.
/// prefix is displayed in front of a formula" + Excel 365).
const XLFN: &[&str] = &[
    "ACOT", "ACOTH", "ARABIC", "ARRAYTOTEXT", "BASE", "BETA.DIST", "BETA.INV", "BINOM.DIST",
    "BINOM.DIST.RANGE", "BINOM.INV", "BITAND", "BITLSHIFT", "BITOR", "BITRSHIFT", "BITXOR",
    "BYCOL", "BYROW", "CEILING.MATH", "CEILING.PRECISE", "CHISQ.DIST", "CHISQ.DIST.RT",
    "CHISQ.INV", "CHISQ.INV.RT", "CHISQ.TEST", "CHOOSECOLS", "CHOOSEROWS", "COMBINA", "CONCAT",
    "CONFIDENCE.NORM", "CONFIDENCE.T", "COT", "COTH", "COVARIANCE.P", "COVARIANCE.S", "CSC",
    "CSCH", "DAYS", "DECIMAL", "DROP", "ERF.PRECISE", "ERFC.PRECISE", "EXPAND", "EXPON.DIST",
    "F.DIST", "F.DIST.RT", "F.INV", "F.INV.RT", "F.TEST", "FILTERXML", "FLOOR.MATH",
    "FLOOR.PRECISE", "FORECAST.ETS", "FORECAST.ETS.CONFINT", "FORECAST.ETS.SEASONALITY",
    "FORECAST.ETS.STAT", "FORECAST.LINEAR", "FORMULATEXT", "GAMMA", "GAMMA.DIST", "GAMMA.INV",
    "GAMMALN.PRECISE", "GAUSS", "HSTACK", "HYPGEOM.DIST", "IFNA", "IFS", "IMCOSH", "IMCOT",
    "IMCSCH", "IMCSC", "IMSEC", "IMSECH", "IMSINH", "IMTAN", "ISFORMULA", "ISOMITTED",
    "ISOWEEKNUM", "LAMBDA", "LET", "LOGNORM.DIST", "LOGNORM.INV", "MAKEARRAY", "MAP", "MAXIFS",
    "MINIFS", "MODE.MULT", "MODE.SNGL", "NEGBINOM.DIST", "NETWORKDAYS.INTL", "NORM.DIST",
    "NORM.INV", "NORM.S.DIST", "NORM.S.INV", "NUMBERVALUE", "PDURATION", "PERCENTILE.EXC",
    "PERCENTILE.INC", "PERCENTRANK.EXC", "PERCENTRANK.INC", "PERMUTATIONA", "PHI",
    "POISSON.DIST", "QUARTILE.EXC", "QUARTILE.INC", "RANDARRAY", "RANK.AVG", "RANK.EQ",
    "REDUCE", "RRI", "SCAN", "SEC", "SECH", "SEQUENCE", "SHEET", "SHEETS", "SKEW.P", "SORTBY",
    "STDEV.P", "STDEV.S", "SWITCH", "T.DIST", "T.DIST.2T", "T.DIST.RT", "T.INV", "T.INV.2T",
    "T.TEST", "TAKE", "TEXTAFTER", "TEXTBEFORE", "TEXTJOIN", "TEXTSPLIT", "TOCOL", "TOROW",
    "UNICHAR", "UNICODE", "UNIQUE", "VALUETOTEXT", "VAR.P", "VAR.S", "VSTACK", "WEBSERVICE",
    "WEIBULL.DIST", "WORKDAY.INTL", "WRAPCOLS", "WRAPROWS", "XLOOKUP", "XMATCH", "XOR", "Z.TEST",
    // Faltavam (19/10/2026, amostra aberta no Excel deu #NOME?): 2010/2013 e Excel 365.
    "AGGREGATE", "ENCODEURL", "IMAGE", "MUNIT", "PERCENTOF", "REGEXEXTRACT", "REGEXREPLACE", "REGEXTEST",
    "GROUPBY", "PIVOTBY", "TRIMRANGE", "DETECTLANGUAGE", "TRANSLATE", "STOCKHISTORY",
];

/// O prefixo que a função leva no arquivo, se levar. `nome` em qualquer caixa.
pub fn prefixo(nome: &str) -> Option<&'static str> {
    let up = nome.to_ascii_uppercase();
    match up.as_str() {
        "FILTER" | "SORT" => Some("_xlfn._xlws."),
        n if XLFN.contains(&n) => Some("_xlfn."),
        _ => None,
    }
}

/// `LET(x;1;x+1)` -> os nomes `x`; `LAMBDA(a;b;a+b)` -> `a`, `b`.
/// Estado do varredor de tokens de `formula_to_excel`.
#[derive(Default)]
pub struct Parametros {
    /// Pilha de chamadas abertas: (nome em maiúscula, índice do argumento, aridade conhecida só ao fechar).
    pilha: Vec<(String, usize)>,
    nomes: Vec<String>,
}

impl Parametros {
    pub fn abre(&mut self, funcao: Option<&str>) {
        self.pilha.push((funcao.unwrap_or("").to_ascii_uppercase(), 0));
    }
    pub fn fecha(&mut self) {
        self.pilha.pop();
    }
    pub fn virgula(&mut self) {
        if let Some(t) = self.pilha.last_mut() {
            t.1 += 1;
        }
    }
    /// O identificador `nome` (sem parêntese depois) está no lugar de um
    /// parâmetro novo? Registra e devolve `true`.
    pub fn declara(&mut self, nome: &str, vai_virgula_ou_fecha: bool) -> bool {
        let Some((f, i)) = self.pilha.last() else { return false };
        let eh_param = match f.as_str() {
            "LET" => i % 2 == 0 && vai_virgula_ou_fecha,
            "LAMBDA" => vai_virgula_ou_fecha,
            _ => false,
        };
        // No LET, o último argumento (corpo) também cai em índice par, mas
        // quem o segue é `)`: só é parâmetro se vier vírgula depois.
        if eh_param && !self.nomes.iter().any(|n| n.eq_ignore_ascii_case(nome)) {
            self.nomes.push(nome.to_string());
        }
        eh_param
    }
    pub fn conhecido(&self, nome: &str) -> bool {
        self.nomes.iter().any(|n| n.eq_ignore_ascii_case(nome))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn prefixos() {
        assert_eq!(prefixo("xlookup"), Some("_xlfn."));
        assert_eq!(prefixo("FILTER"), Some("_xlfn._xlws."));
        assert_eq!(prefixo("SUM"), None);
        assert_eq!(prefixo("STDEV.S"), Some("_xlfn."));
        assert_eq!(prefixo("STDEV"), None);
        assert_eq!(prefixo("REGEXTEST"), Some("_xlfn."));
        assert_eq!(prefixo("AGGREGATE"), Some("_xlfn."));
    }
}
