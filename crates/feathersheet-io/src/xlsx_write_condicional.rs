//! `<conditionalFormatting>` da aba, a partir do `Sheet.r` (JSON de
//! `Vec<ConditionalRule>`). Contraparte de `xlsx_read::condicional`.
//!
//! Operadores viram `cellIs`; texto, vazio, repetido/único e média viram os
//! tipos próprios do Excel; fórmula personalizada vira `expression`; escala de
//! cor e barra de dados vão como estão. O estilo da regra vai para
//! `<dxfs>` do `styles.xml` (a tabela quem guarda). A cor negativa da barra
//! de dados só existe no bloco x14 do Excel e não é gravada.
//!
//! Fórmula do Excel é relativa ao canto superior esquerdo do `sqref`; o motor
//! usa o canto de cada retângulo da regra. Por isso a regra de fórmula sai um
//! `<conditionalFormatting>` por retângulo, cada um com a fórmula como está.

use std::fmt::Write as _;

use feathersheet_core::celulas::input::parse_literal;
use feathersheet_core::celulas::number::to_js_string;
use feathersheet_core::celulas::refs::cell_name;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::rules::{
    CellCondition, CellOp, ColorScaleCondition, ConditionalRule, DataBarCondition, Faixa, ScalePoint, ScalePointType,
};
use feathersheet_core::planilha::sheet::Sheet;

use crate::xlsx_write::{escape_xml, formula_to_excel, para_argb_pub};

fn intervalo(r: &Faixa) -> String {
    // `A:A` vem até a linha 100 milhões: o Excel só tem 1.048.576 e recusa a regra.
    let (r1, c1) = (crate::xlsx_write::ate_o_excel_linha(r.r1), crate::xlsx_write::ate_o_excel_coluna(r.c1));
    if (r.r0, r.c0) == (r1, c1) {
        cell_name(r.r0, r.c0)
    } else {
        format!("{}:{}", cell_name(r.r0, r.c0), cell_name(r1, c1))
    }
}

fn aspas(texto: &str) -> String {
    format!("\"{}\"", texto.replace('"', "\"\""))
}

/// O valor digitado (pt-BR) como literal de fórmula do Excel.
fn literal(v: &str) -> Option<String> {
    match parse_literal(v).value {
        Value::Number(n) => Some(to_js_string(n)),
        Value::Bool(b) => Some(if b { "TRUE" } else { "FALSE" }.to_string()),
        Value::Empty | Value::Error(_) => None,
        _ => Some(aspas(v.trim())),
    }
}

fn ponto(p: &ScalePoint) -> Option<String> {
    let cor = para_argb_pub(&p.color)?;
    let _ = cor;
    Some(match p.point_type {
        ScalePointType::Min => "<cfvo type=\"min\"/>".to_string(),
        ScalePointType::Max => "<cfvo type=\"max\"/>".to_string(),
        t => {
            let nome = match t {
                ScalePointType::Percent => "percent",
                ScalePointType::Percentile => "percentile",
                _ => "num",
            };
            format!("<cfvo type=\"{nome}\" val=\"{}\"/>", to_js_string(p.value.unwrap_or(0.0)))
        }
    })
}

fn escala(e: &ColorScaleCondition) -> Option<String> {
    let mut pts: Vec<&ScalePoint> = vec![&e.min];
    pts.extend(e.mid.as_ref());
    pts.push(&e.max);
    let mut vos = String::new();
    let mut cores = String::new();
    for p in pts {
        vos.push_str(&ponto(p)?);
        let _ = write!(cores, "<color rgb=\"{}\"/>", para_argb_pub(&p.color)?);
    }
    Some(format!("<colorScale>{vos}{cores}</colorScale>"))
}

fn barra(d: &DataBarCondition) -> Option<String> {
    let cor = para_argb_pub(&d.color)?;
    let extremo = |p: &Option<ScalePoint>, padrao: &str| match p {
        Some(p) => ponto(&ScalePoint { color: d.color.clone(), ..p.clone() }),
        None => Some(format!("<cfvo type=\"{padrao}\"/>")),
    };
    let oculta = if d.hide_value { " showValue=\"0\"" } else { "" };
    Some(format!("<dataBar{oculta}>{}{}<color rgb=\"{cor}\"/></dataBar>", extremo(&d.min, "min")?, extremo(&d.max, "max")?))
}

/// Grava as regras da aba. `dxf` devolve o `dxfId` do estilo (ou `None` se o
/// estilo é vazio).
pub(crate) fn condicional_xml(
    sheet: &Sheet,
    dxf: &mut dyn FnMut(&feathersheet_core::operacoes::rules::CondStyle) -> Option<usize>,
) -> String {
    use feathersheet_core::operacoes::rules::Condition;

    let Some(json) = sheet.r.as_deref() else { return String::new() };
    let Ok(regras) = serde_json::from_str::<Vec<ConditionalRule>>(json) else { return String::new() };
    let mut saida = String::new();
    let mut prioridade = 0usize;
    for regra in &regras {
        if regra.ranges.is_empty() {
            continue;
        }
        let todos = regra.ranges.iter().map(intervalo).collect::<Vec<_>>().join(" ");
        let tl = cell_name(regra.ranges[0].r0, regra.ranges[0].c0);
        match &regra.condition {
            Condition::Cell(c) => {
                let id = dxf(&c.style);
                if c.op == CellOp::Formula {
                    let corpo = c.values.first().map(String::as_str).unwrap_or("");
                    if corpo.trim_start_matches('=').trim().is_empty() {
                        continue;
                    }
                    let f = escape_xml(&formula_to_excel(corpo));
                    for r in &regra.ranges {
                        prioridade += 1;
                        let _ = write!(
                            saida,
                            "<conditionalFormatting sqref=\"{}\"><cfRule type=\"expression\"{} priority=\"{prioridade}\"><formula>{f}</formula></cfRule></conditionalFormatting>",
                            intervalo(r), atributo_dxf(id)
                        );
                    }
                    continue;
                }
                let Some(corpo) = regra_de_celula(c, &tl, id) else { continue };
                prioridade += 1;
                let _ = write!(saida, "<conditionalFormatting sqref=\"{todos}\">{}</conditionalFormatting>", corpo.replacen("PRIO", &prioridade.to_string(), 1));
            }
            Condition::ColorScale(e) => {
                let Some(x) = escala(e) else { continue };
                prioridade += 1;
                let _ = write!(saida, "<conditionalFormatting sqref=\"{todos}\"><cfRule type=\"colorScale\" priority=\"{prioridade}\">{x}</cfRule></conditionalFormatting>");
            }
            Condition::DataBar(d) => {
                let Some(x) = barra(d) else { continue };
                prioridade += 1;
                let _ = write!(saida, "<conditionalFormatting sqref=\"{todos}\"><cfRule type=\"dataBar\" priority=\"{prioridade}\">{x}</cfRule></conditionalFormatting>");
            }
        }
    }
    saida
}

fn atributo_dxf(id: Option<usize>) -> String {
    id.map(|i| format!(" dxfId=\"{i}\"")).unwrap_or_default()
}

/// O `<cfRule>` de uma regra de célula (que não é fórmula). A prioridade vai
/// como `PRIO`, trocada por quem chama.
fn regra_de_celula(c: &CellCondition, tl: &str, id: Option<usize>) -> Option<String> {
    let d = atributo_dxf(id);
    let v = |i: usize| c.values.get(i).map(String::as_str).unwrap_or("");
    let comparacao = |operador: &str, dois: bool| -> Option<String> {
        let a = literal(v(0))?;
        let f2 = if dois { format!("<formula>{}</formula>", escape_xml(&literal(v(1))?)) } else { String::new() };
        Some(format!("<cfRule type=\"cellIs\"{d} priority=\"PRIO\" operator=\"{operador}\"><formula>{}</formula>{f2}</cfRule>", escape_xml(&a)))
    };
    let com_formula = |tipo: &str, extra: &str, f: String| {
        format!("<cfRule type=\"{tipo}\"{d} priority=\"PRIO\"{extra}><formula>{}</formula></cfRule>", escape_xml(&f))
    };
    Some(match c.op {
        CellOp::GreaterThan | CellOp::DateAfter => comparacao("greaterThan", false)?,
        CellOp::GreaterOrEqual => comparacao("greaterThanOrEqual", false)?,
        CellOp::LessThan | CellOp::DateBefore => comparacao("lessThan", false)?,
        CellOp::LessOrEqual => comparacao("lessThanOrEqual", false)?,
        CellOp::Equal | CellOp::DateOn => comparacao("equal", false)?,
        CellOp::NotEqual => comparacao("notEqual", false)?,
        CellOp::Between => comparacao("between", true)?,
        CellOp::NotBetween => comparacao("notBetween", true)?,
        CellOp::IsEmpty => com_formula("containsBlanks", "", format!("LEN(TRIM({tl}))=0")),
        CellOp::IsNotEmpty => com_formula("notContainsBlanks", "", format!("LEN(TRIM({tl}))>0")),
        CellOp::TextContains | CellOp::TextNotContains | CellOp::TextStartsWith | CellOp::TextEndsWith => {
            let texto = v(0);
            if texto.is_empty() {
                return None;
            }
            let q = aspas(texto);
            let attr = format!(" text=\"{}\"", escape_xml(texto));
            match c.op {
                CellOp::TextContains => com_formula("containsText", &format!(" operator=\"containsText\"{attr}"), format!("NOT(ISERROR(SEARCH({q},{tl})))")),
                CellOp::TextNotContains => com_formula("notContainsText", &format!(" operator=\"notContains\"{attr}"), format!("ISERROR(SEARCH({q},{tl}))")),
                CellOp::TextStartsWith => com_formula("beginsWith", &format!(" operator=\"beginsWith\"{attr}"), format!("LEFT({tl},LEN({q}))={q}")),
                _ => com_formula("endsWith", &format!(" operator=\"endsWith\"{attr}"), format!("RIGHT({tl},LEN({q}))={q}")),
            }
        }
        CellOp::Duplicate => format!("<cfRule type=\"duplicateValues\"{d} priority=\"PRIO\"/>"),
        CellOp::Unique => format!("<cfRule type=\"uniqueValues\"{d} priority=\"PRIO\"/>"),
        CellOp::AboveAverage => format!("<cfRule type=\"aboveAverage\"{d} priority=\"PRIO\"/>"),
        CellOp::BelowAverage => format!("<cfRule type=\"aboveAverage\"{d} priority=\"PRIO\" aboveAverage=\"0\"/>"),
        CellOp::Formula => return None,
    })
}

/// `<dxf>` do estilo da regra, ou `None` se ele não pinta nada.
pub(crate) fn dxf_xml(s: &feathersheet_core::operacoes::rules::CondStyle) -> Option<String> {
    let mut fonte = String::new();
    let mut liga = |tag: &str, v: Option<bool>| match v {
        Some(true) => { let _ = write!(fonte, "<{tag}/>"); }
        Some(false) => { let _ = write!(fonte, "<{tag} val=\"0\"/>"); }
        None => {}
    };
    liga("b", s.bold);
    liga("i", s.italic);
    liga("strike", s.strike);
    match s.underline {
        Some(true) => fonte.push_str("<u/>"),
        Some(false) => fonte.push_str("<u val=\"none\"/>"),
        None => {}
    }
    if let Some(c) = s.color.as_deref().and_then(para_argb_pub) {
        let _ = write!(fonte, "<color rgb=\"{c}\"/>");
    }
    let mut xml = String::new();
    if !fonte.is_empty() {
        let _ = write!(xml, "<font>{fonte}</font>");
    }
    if let Some(c) = s.bg.as_deref().and_then(para_argb_pub) {
        let _ = write!(xml, "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{c}\"/><bgColor rgb=\"{c}\"/></patternFill></fill>");
    }
    (!xml.is_empty()).then(|| format!("<dxf>{xml}</dxf>"))
}
