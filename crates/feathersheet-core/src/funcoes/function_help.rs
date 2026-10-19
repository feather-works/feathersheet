//! Assinatura, descrição e categoria de cada função.
//!
//! Tradução de `src/core/function-help.ts`. É tabela constante: nada aqui
//! aloca, e nada aqui calcula.

use crate::funcoes::function_help_data::HELP;

/// Os argumentos e o que a função faz. Os opcionais vêm entre colchetes, e o
/// `…` indica que repete.
#[derive(Clone, Copy, Debug)]
pub struct FunctionHelp {
    pub args: &'static [&'static str],
    pub doc: &'static str,
}

/// O agrupamento que o catálogo de funções mostra. Transcrito inteiro do
/// `function-help.ts` — é ele que define quantas funções a `HELP` precisa
/// cobrir.
pub static CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Matemática",
        &[
            "SUM", "SUBTOTAL", "AGGREGATE", "PRODUCT", "ABS", "SQRT", "POWER", "EXP", "LN", "LOG", "LOG10", "SIGN", "INT",
            "ROUND", "ROUNDUP", "ROUNDDOWN", "TRUNC", "MOD", "CEILING", "FLOOR", "PI", "RAND",
            "RANDBETWEEN", "SUMPRODUCT", "FACT", "EVEN", "ODD", "GCD", "LCM", "MROUND", "QUOTIENT", "SUMSQ", "SQRTPI", "FACTDOUBLE", "ROMAN", "COMBIN", "PERMUT", "ISEVEN", "ISODD", "SIN",
            "COS", "TAN", "ASIN", "ACOS", "ATAN", "ATAN2", "SINH", "COSH", "TANH", "RADIANS",
            "ACOSH", "ASINH", "ATANH", "ACOT", "ACOTH", "COT", "COTH", "CSC", "CSCH", "SEC", "SECH",
            "BASE", "DECIMAL", "ARABIC", "CEILING.MATH", "CEILING.PRECISE", "ISO.CEILING",
            "FLOOR.MATH", "FLOOR.PRECISE", "COMBINA", "MULTINOMIAL", "MUNIT", "SERIESSUM",
            "SUMX2MY2", "SUMX2PY2", "SUMXMY2",
            "DEGREES", "PERCENTOF",
        ],
    ),
    (
        "Estatística",
        &[
            "BETA.DIST", "BETA.INV", "BINOM.DIST", "BINOM.DIST.RANGE", "BINOM.INV", "CHISQ.DIST.RT", "CHISQ.INV", "CHISQ.INV.RT", "EXPON.DIST", "F.DIST", "F.DIST.RT", "F.INV", "F.INV.RT", "GAMMA", "GAMMA.DIST", "GAMMA.INV", "GAMMALN", "GAMMALN.PRECISE", "GAUSS", "HYPGEOM.DIST", "LOGNORM.DIST", "LOGNORM.INV", "NEGBINOM.DIST", "NORM.S.INV", "PHI", "POISSON.DIST", "T.DIST.2T", "T.DIST.RT", "T.INV", "T.INV.2T", "WEIBULL.DIST", "CONFIDENCE.NORM", "CONFIDENCE.T", "PERMUTATIONA", "BETADIST", "HYPGEOMDIST", "LOGNORMDIST", "NEGBINOMDIST", "NORMSDIST", "TDIST", "BINOMDIST", "CHIDIST", "CHIINV", "CRITBINOM", "EXPONDIST", "FDIST", "FINV", "GAMMADIST", "GAMMAINV", "LOGINV", "NORMSINV", "POISSON", "TINV", "WEIBULL", "BETAINV", "CONFIDENCE", "CHITEST", "TTEST",
            "AVERAGE", "MEDIAN", "MODE", "MIN", "MAX", "LARGE", "SMALL", "COUNT", "COUNTA",
            "COUNTBLANK", "COUNTUNIQUE", "SUMBYCOLOR", "COUNTBYCOLOR", "STDEV", "STDEV.P", "VAR", "VAR.P", "AVEDEV", "DEVSQ",
            "GEOMEAN", "HARMEAN", "SKEW", "KURT", "PERCENTILE", "QUARTILE", "RANK", "SUMIF",
            "SUMIFS", "COUNTIF", "COUNTIFS", "AVERAGEIF", "AVERAGEIFS", "MAXIFS", "MINIFS",
            "CORREL", "COVAR", "SLOPE", "INTERCEPT", "RSQ", "FORECAST",
            "AVERAGEA", "MAXA", "MINA", "STDEVA", "STDEVPA", "VARA", "VARPA", "RANK.EQ",
            "RANK.AVG", "PERCENTILE.EXC", "QUARTILE.EXC", "PERCENTRANK.INC", "PERCENTRANK.EXC",
            "COVARIANCE.P", "COVARIANCE.S", "PEARSON", "STANDARDIZE", "TRIMMEAN",
            "FORECAST.LINEAR", "SKEW.P", "MODE.MULT", "FISHER", "FISHERINV",
            "STEYX", "PROB", "F.TEST", "Z.TEST", "TREND", "GROWTH",
            "LINEST", "LOGEST", "FORECAST.ETS", "FORECAST.ETS.CONFINT", "FORECAST.ETS.SEASONALITY", "FORECAST.ETS.STAT",
            "NORM.DIST", "NORM.INV", "NORM.S.DIST", "NORMDIST", "NORMINV", "CHISQ.DIST", "CHISQ.TEST", "T.DIST", "T.TEST", "MODE.SNGL", "PERCENTILE.INC", "PERCENTRANK", "QUARTILE.INC", "STDEV.S", "STDEVP", "VAR.S", "VARP",
        ],
    ),
    (
        "Banco de dados",
        &["DSUM", "DAVERAGE", "DCOUNT", "DCOUNTA", "DGET", "DMAX", "DMIN", "DPRODUCT", "DSTDEV", "DSTDEVP", "DVAR", "DVARP"],
    ),
    (
        "Matriz dinâmica",
        &[
            "FILTER", "UNIQUE", "SORT", "TRANSPOSE", "SEQUENCE", "FREQUENCY", "MMULT", "MINVERSE",
            "MDETERM", "VSTACK", "HSTACK", "TAKE", "DROP", "CHOOSECOLS", "CHOOSEROWS", "TOCOL", "TOROW",
            "WRAPROWS", "WRAPCOLS", "EXPAND", "SORTBY", "RANDARRAY",
            "GROUPBY", "PIVOTBY",
        ],
    ),
    (
        "Lógica",
        &[
            "IF", "IFS", "IFERROR", "AND", "OR", "NOT", "XOR", "TRUE", "FALSE", "ISERROR",
            "ISBLANK", "ISNUMBER", "ISTEXT", "ISERR", "ISNA", "ISLOGICAL", "ISNONTEXT",
            "ERROR.TYPE", "TYPE", "N",
            "BYROW", "BYCOL", "SCAN", "MAKEARRAY", "ISOMITTED",
            "SWITCH", "IFNA", "NA", "LET", "LAMBDA", "MAP", "REDUCE",
        ],
    ),
    (
        "Texto",
        &[
            "IMAGE", "IMAGE.BASE64", "CONCAT", "LEN", "UPPER", "LOWER", "PROPER", "TRIM", "LEFT", "RIGHT", "MID", "REPT",
            "SUBSTITUTE", "FIND", "SEARCH", "VALUE", "TEXT",
            "CHAR", "CODE", "CLEAN", "EXACT", "T", "DOLLAR", "FIXED", "NUMBERVALUE", "UNICHAR", "UNICODE", "REPLACE",
            "TEXTAFTER", "TEXTBEFORE", "ARRAYTOTEXT", "VALUETOTEXT", "REGEXTEST",
            "LENB", "LEFTB", "RIGHTB", "MIDB", "FINDB", "SEARCHB", "REPLACEB", "ENCODEURL",
            "ASC", "DBCS", "BAHTTEXT", "PHONETIC",
            "CONCATENATE", "TEXTJOIN", "TEXTSPLIT", "JOIN", "REGEXEXTRACT", "REGEXMATCH", "REGEXREPLACE",
        ],
    ),
    (
        "Pesquisa e referência",
        &[
            "VLOOKUP", "HLOOKUP", "XLOOKUP", "INDEX", "MATCH", "CHOOSE", "ROW", "COLUMN", "ROWS",
            "COLUMNS", "LOOKUP", "XMATCH", "ADDRESS", "HYPERLINK", "ISREF",
            "OFFSET", "INDIRECT", "TRIMRANGE", "AREAS", "FORMULATEXT", "ISFORMULA", "SHEET", "SHEETS", "CELL", "INFO", "GETPIVOTDATA",
        ],
    ),
    (
        "Data e hora",
        &["TODAY", "NOW", "DATE", "DATEDIF", "YEAR", "MONTH", "DAY", "WEEKDAY", "HOUR", "MINUTE", "SECOND", "TIME", "DATEVALUE", "TIMEVALUE", "DAYS", "DAYS360", "ISOWEEKNUM", "YEARFRAC", "NETWORKDAYS.INTL", "WORKDAY.INTL", "EDATE", "EOMONTH", "WORKDAY", "NETWORKDAYS", "WEEKNUM"],
    ),
    (
        "Engenharia",
        &[
            "ERF", "ERF.PRECISE", "ERFC", "ERFC.PRECISE",
            "BIN2DEC", "BIN2HEX", "BIN2OCT", "DEC2BIN", "DEC2HEX", "DEC2OCT", "HEX2BIN",
            "HEX2DEC", "HEX2OCT", "OCT2BIN", "OCT2DEC", "OCT2HEX", "BITAND", "BITOR", "BITXOR",
            "BITLSHIFT", "BITRSHIFT", "DELTA", "GESTEP",
            "COMPLEX", "IMABS", "IMAGINARY", "IMARGUMENT", "IMCONJUGATE", "IMCOS", "IMCOSH",
            "IMCOT", "IMCSC", "IMCSCH", "IMDIV", "IMEXP", "IMLN", "IMLOG10", "IMLOG2", "IMPOWER",
            "IMPRODUCT", "IMREAL", "IMSEC", "IMSECH", "IMSIN", "IMSINH", "IMSQRT", "IMSUB", "IMSUM",
            "IMTAN", "CONVERT", "BESSELJ", "BESSELI",
            "BESSELK", "BESSELY",
        ],
    ),
    ("Fonte de dados", &["IMPORTDATA", "IMPORTJSON", "FETCHTEXT", "FILTERXML", "WEBSERVICE", "CUBEKPIMEMBER", "CUBEMEMBER", "CUBEMEMBERPROPERTY", "CUBERANKEDMEMBER", "CUBESET", "CUBESETCOUNT", "CUBEVALUE", "RTD", "CALL", "REGISTER.ID", "STOCKHISTORY", "TRANSLATE", "DETECTLANGUAGE"]),
    (
        "Financeiras",
        &[
            "PMT", "FV", "PV", "NPER", "IPMT", "PPMT", "ISPMT", "SLN", "SYD", "DB", "DDB", "VDB",
            "EFFECT", "NOMINAL", "DOLLARDE", "DOLLARFR", "RRI", "PDURATION", "NPV",
            "RATE", "CUMIPMT", "CUMPRINC", "XNPV", "XIRR", "IRR", "MIRR", "FVSCHEDULE",
            "DISC", "PRICEDISC", "YIELDDISC", "INTRATE", "RECEIVED", "TBILLPRICE", "TBILLYIELD", "TBILLEQ",
            "PRICEMAT", "YIELDMAT", "ACCRINTM",
            "ACCRINT", "AMORDEGRC", "AMORLINC", "COUPDAYBS", "COUPDAYS", "COUPDAYSNC", "COUPNCD", "COUPNUM", "COUPPCD", "DURATION", "MDURATION", "ODDFPRICE", "ODDFYIELD", "ODDLPRICE", "ODDLYIELD", "PRICE", "YIELD", "EUROCONVERT",
        ],
    ),
];

/// A ajuda de uma função pelo **nome canônico em inglês**, sem ligar para
/// maiúscula. Quem tem um apelido em português em mãos resolve antes, no
/// registro de funções.
pub fn help_for(canonical: &str) -> Option<&'static FunctionHelp> {
    let alvo = canonical.to_uppercase();
    HELP.iter()
        .find(|(nome, _)| *nome == alvo)
        .map(|(_, ajuda)| ajuda)
}

/// Assinatura pronta para mostrar: `SOMA(núm1; [núm2]; …)`.
///
/// O nome vem de fora justamente para a dica aparecer no idioma que a pessoa
/// escreveu — a mesma ajuda serve para `SOMA` e para `SUM`.
pub fn signature(name: &str, help: &FunctionHelp) -> String {
    format!("{}({})", name.to_uppercase(), help.args.join("; "))
}

pub fn all() -> &'static [(&'static str, FunctionHelp)] {
    HELP
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nao_ha_verbete_repetido() {
        let mut vistos = std::collections::HashSet::new();
        for (nome, _) in HELP {
            assert!(vistos.insert(*nome), "verbete repetido: {nome}");
        }
    }

    #[test]
    fn funcao_nao_aparece_em_duas_categorias() {
        let mut vistos = std::collections::HashSet::new();
        for (categoria, nomes) in CATEGORIES {
            for nome in *nomes {
                assert!(
                    vistos.insert(*nome),
                    "{nome} aparece em mais de uma categoria (a última foi {categoria})"
                );
            }
        }
    }

    #[test]
    fn acha_sem_ligar_para_maiuscula() {
        assert!(help_for("sum").is_some());
        assert!(help_for("SUM").is_some());
        assert!(help_for("NÃO.EXISTE").is_none());
    }

    #[test]
    fn monta_a_assinatura() {
        let ajuda = help_for("SUM").unwrap();
        assert_eq!(signature("soma", ajuda), "SOMA(núm1; [núm2]; …)");
        assert_eq!(signature("SUM", ajuda), "SUM(núm1; [núm2]; …)");
    }

    #[test]
    fn tabela_completa() {
        // Piso, não igualdade: cada lote de funções portadas acrescenta verbetes.
        assert!(HELP.len() >= 161, "a tabela perdeu verbetes");
        for (i, (nome, _)) in HELP.iter().enumerate() {
            assert!(
                !HELP[..i].iter().any(|(outro, _)| outro == nome),
                "verbete duplicado: {nome}"
            );
        }
        for (categoria, nomes) in CATEGORIES {
            for nome in *nomes {
                assert!(
                    help_for(nome).is_some(),
                    "{nome} está na categoria {categoria} e não tem ajuda"
                );
            }
        }
    }
}
