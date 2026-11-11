//! Formatação condicional do `.xlsx`: `<conditionalFormatting>` da aba e o
//! bloco do Excel novo em `<extLst>` (`x14:conditionalFormatting`, onde ficam
//! as regras que leem outra aba). Vira o `Vec<ConditionalRule>` do motor
//! (JSON `Sheet.r`).
//!
//! O que o motor tem de regra de célula vira regra de célula (operadores,
//! texto, vazio, repetido/único, acima/abaixo da média), mais escala de cor e
//! barra de dados. O que o Excel faz e o motor não tem como regra própria é
//! reescrito como **fórmula personalizada** quando dá para dizer a mesma coisa
//! em fórmula: `cellIs` contra referência/fórmula, erros, 10 primeiros por
//! quantidade, acima da média com igualdade. Ficam de fora (anotados no
//! diário): conjuntos de ícones, período de datas (`timePeriod`), desvio
//! padrão, 10 primeiros por percentual, barra de dados do x14, e a borda e o
//! formato de número do `dxf` (o estilo da regra só tem fonte e fundo).
//!
//! Referência relativa na fórmula é relativa ao canto superior esquerdo do
//! `sqref`. O motor usa o canto de cada retângulo da regra; para `sqref` com
//! vários retângulos a regra é então repartida, uma por retângulo, com a
//! fórmula deslocada.

use super::formula::formula_from_excel;
use super::refs::shift_formula;
use super::styles::{cor_de, dxf_do_indice, ler_dxf, XfStyle};
use super::xml::XmlNode;
use feathersheet_core::celulas::refs::{cell_name, parse_range_ref};
use feathersheet_core::operacoes::rules::{
    CellCondition, CellOp, ColorScaleCondition, CondStyle, Condition, ConditionalRule, DataBarCondition, Faixa,
    ScalePoint, ScalePointType,
};

/// O resultado da leitura de uma aba.
#[derive(Default)]
pub(super) struct Lidas {
    pub regras: Vec<serde_json::Value>,
    /// Quantas `<cfRule>` o arquivo tinha (as do x14 que só repetem uma barra
    /// de dados clássica não entram na conta).
    pub total: usize,
    /// Uma linha por regra que não coube no motor: `tipo: motivo`.
    pub fora: Vec<String>,
}

struct Bruta<'a> {
    prioridade: i64,
    ranges: Vec<Faixa>,
    regra: &'a XmlNode,
    /// `dxf` embutido (x14).
    dxf: Option<&'a XmlNode>,
}

fn faixas(sqref: &str) -> Vec<Faixa> {
    sqref
        .split_whitespace()
        .filter_map(|t| parse_range_ref(&t.replace('$', "")))
        .map(|r| Faixa { r0: r.r0, c0: r.c0, r1: r.r1, c1: r.c1 })
        .collect()
}

fn formulas(r: &XmlNode) -> Vec<String> {
    r.children
        .iter()
        .filter(|c| c.name == "formula" || c.name == "f")
        .map(|c| c.text.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

pub(super) fn ler(root: &XmlNode, xfs: &[XfStyle]) -> Lidas {
    let mut brutas: Vec<Bruta> = Vec::new();
    for cf in root.children_named("conditionalFormatting") {
        let ranges = faixas(cf.attr("sqref").unwrap_or(""));
        for r in cf.children_named("cfRule") {
            brutas.push(Bruta { prioridade: prioridade(r), ranges: ranges.clone(), regra: r, dxf: None });
        }
    }
    let mut total = brutas.len();
    let mut fora = Vec::new();
    // `<extLst><ext><x14:conditionalFormattings>` (o leitor tira o prefixo).
    let novos = root
        .child("extLst")
        .into_iter()
        .flat_map(|e| e.children_named("ext"))
        .flat_map(|e| e.children_named("conditionalFormattings"))
        .flat_map(|c| c.children_named("conditionalFormatting"));
    for cf in novos {
        let ranges = faixas(&cf.child("sqref").map(|s| s.text.clone()).unwrap_or_default());
        for r in cf.children_named("cfRule") {
            // Barra de dados e ícones do x14 repetem (ou estendem) a regra
            // clássica de mesmo tipo; a clássica já foi lida.
            if matches!(r.attr("type"), Some("dataBar") | Some("iconSet")) {
                continue;
            }
            total += 1;
            brutas.push(Bruta { prioridade: prioridade(r), ranges: ranges.clone(), regra: r, dxf: r.child("dxf") });
        }
    }
    // Prioridade 1 manda primeiro; o motor também deixa a primeira que bate mandar.
    brutas.sort_by_key(|b| b.prioridade);

    let mut regras = Vec::new();
    let mut n = 0usize;
    for b in &brutas {
        if b.ranges.is_empty() {
            fora.push(feathersheet_core::trf!("{}: sem intervalo", b.regra.attr("type").unwrap_or("?")));
            continue;
        }
        let estilo = match (b.dxf, b.regra.attr("dxfId").and_then(|s| s.parse::<usize>().ok())) {
            (Some(d), _) => ler_dxf(d),
            (None, Some(id)) => dxf_do_indice(xfs, id).unwrap_or_default(),
            (None, None) => CondStyle::default(),
        };
        match converter(b, estilo) {
            Ok(itens) => {
                for (ranges, condition) in itens {
                    n += 1;
                    let regra = ConditionalRule { id: format!("cf_{n}"), ranges, condition };
                    if let Ok(v) = serde_json::to_value(&regra) {
                        regras.push(v);
                    }
                }
            }
            Err(motivo) => fora.push(format!("{}: {motivo}", b.regra.attr("type").unwrap_or("?"))),
        }
    }
    Lidas { regras, total, fora }
}

fn prioridade(r: &XmlNode) -> i64 {
    r.attr("priority").and_then(|p| p.parse().ok()).unwrap_or(i64::MAX)
}

type Itens = Vec<(Vec<Faixa>, Condition)>;

fn celula(ranges: Vec<Faixa>, op: CellOp, values: Vec<String>, style: CondStyle) -> Itens {
    vec![(ranges, Condition::Cell(CellCondition { op, values, style }))]
}

/// Fórmula do Excel (relativa ao canto superior esquerdo de `ranges`) vira
/// uma regra de fórmula por retângulo, deslocada para o canto dele.
fn por_faixa(ranges: &[Faixa], excel: &str, estilo: &CondStyle) -> Itens {
    let r0 = ranges.iter().map(|r| r.r0).min().unwrap_or(0);
    let c0 = ranges.iter().map(|r| r.c0).min().unwrap_or(0);
    ranges
        .iter()
        .map(|r| {
            let f = if (r.r0, r.c0) == (r0, c0) {
                format!("={excel}")
            } else {
                shift_formula(&format!("={excel}"), r.r0 as i64 - r0 as i64, r.c0 as i64 - c0 as i64)
            };
            (
                vec![*r],
                Condition::Cell(CellCondition { op: CellOp::Formula, values: vec![formula_from_excel(&f)], style: estilo.clone() }),
            )
        })
        .collect()
}

fn canto(ranges: &[Faixa]) -> String {
    cell_name(ranges.iter().map(|r| r.r0).min().unwrap_or(0), ranges.iter().map(|r| r.c0).min().unwrap_or(0))
}

/// O intervalo, absoluto, em sintaxe do Excel (só quando há um retângulo).
fn absoluto(ranges: &[Faixa]) -> Option<String> {
    let [r] = ranges else { return None };
    let (a, b) = (cell_name(r.r0, r.c0), cell_name(r.r1, r.c1));
    let abs = |c: &str| {
        let i = c.find(|ch: char| ch.is_ascii_digit()).unwrap_or(c.len());
        format!("${}${}", &c[..i], &c[i..])
    };
    Some(format!("{}:{}", abs(&a), abs(&b)))
}

/// Valor literal que o motor sabe comparar (`parse_literal`, pt-BR).
fn literal(f: &str) -> Option<String> {
    let t = f.trim();
    if let Some(dentro) = t.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        if !dentro.contains('"') || dentro.replace("\"\"", "").find('"').is_none() {
            return Some(dentro.replace("\"\"", "\""));
        }
        return None;
    }
    let (corpo, pct) = match t.strip_suffix('%') {
        Some(c) => (c, true),
        None => (t, false),
    };
    if corpo.parse::<f64>().is_ok() && !corpo.contains(['e', 'E', 'i', 'n']) {
        return Some(format!("{}{}", corpo.replace('.', ","), if pct { "%" } else { "" }));
    }
    if !pct && (t.eq_ignore_ascii_case("TRUE") || t.eq_ignore_ascii_case("FALSE")) {
        return Some(t.to_uppercase());
    }
    None
}

fn ponto(cfvo: &XmlNode, cor: String) -> Result<ScalePoint, String> {
    let valor = cfvo.attr("val").map(|v| v.trim().to_string());
    let numero = || valor.as_deref().and_then(|v| v.parse::<f64>().ok());
    let (tipo, v) = match cfvo.attr("type").unwrap_or("") {
        "min" | "autoMin" => (ScalePointType::Min, None),
        "max" | "autoMax" => (ScalePointType::Max, None),
        "num" => (ScalePointType::Number, Some(numero().ok_or_else(|| feathersheet_core::tr!("ponto de escala com fórmula"))?)),
        "percent" => (ScalePointType::Percent, Some(numero().ok_or_else(|| feathersheet_core::tr!("ponto de escala inválido"))?)),
        "percentile" => (ScalePointType::Percentile, Some(numero().ok_or_else(|| feathersheet_core::tr!("ponto de escala inválido"))?)),
        "formula" => (ScalePointType::Number, Some(numero().ok_or_else(|| feathersheet_core::tr!("ponto de escala com fórmula"))?)),
        outro => return Err(feathersheet_core::trf!("ponto de escala '{outro}'")),
    };
    Ok(ScalePoint { point_type: tipo, value: v, color: cor })
}

fn converter(b: &Bruta, estilo: CondStyle) -> Result<Itens, String> {
    let r = b.regra;
    let ranges = &b.ranges;
    let fs = formulas(r);
    let tipo = r.attr("type").unwrap_or("");
    let operador = r.attr("operator").unwrap_or("");
    let tl = canto(ranges);
    match tipo {
        "expression" => {
            let f = fs.first().ok_or_else(|| feathersheet_core::tr!("sem fórmula"))?;
            Ok(por_faixa(ranges, f, &estilo))
        }
        "cellIs" => {
            let op = match operador {
                "equal" => CellOp::Equal,
                "notEqual" => CellOp::NotEqual,
                "greaterThan" => CellOp::GreaterThan,
                "greaterThanOrEqual" => CellOp::GreaterOrEqual,
                "lessThan" => CellOp::LessThan,
                "lessThanOrEqual" => CellOp::LessOrEqual,
                "between" => CellOp::Between,
                "notBetween" => CellOp::NotBetween,
                o => return Err(format!("operador '{o}'")),
            };
            let dois = matches!(op, CellOp::Between | CellOp::NotBetween);
            let esperados = if dois { 2 } else { 1 };
            if fs.len() < esperados {
                return Err(feathersheet_core::tr!("sem valor de comparação").into());
            }
            let lits: Option<Vec<String>> = fs.iter().take(esperados).map(|f| literal(f)).collect();
            if let Some(values) = lits {
                return Ok(celula(ranges.clone(), op, values, estilo));
            }
            // Contra referência ou fórmula: vira fórmula personalizada.
            let sinal = |o: CellOp| match o {
                CellOp::Equal => "=",
                CellOp::NotEqual => "<>",
                CellOp::GreaterThan => ">",
                CellOp::GreaterOrEqual => ">=",
                CellOp::LessThan => "<",
                _ => "<=",
            };
            let expr = match op {
                CellOp::Between => format!("AND({tl}>=({}),{tl}<=({}))", fs[0], fs[1]),
                CellOp::NotBetween => format!("OR({tl}<({}),{tl}>({}))", fs[0], fs[1]),
                o => format!("{tl}{}({})", sinal(o), fs[0]),
            };
            Ok(por_faixa(ranges, &expr, &estilo))
        }
        "containsText" | "notContainsText" | "beginsWith" | "endsWith" => {
            let texto = r.attr("text").ok_or_else(|| feathersheet_core::tr!("sem texto"))?.to_string();
            let op = match tipo {
                "containsText" => CellOp::TextContains,
                "notContainsText" => CellOp::TextNotContains,
                "beginsWith" => CellOp::TextStartsWith,
                _ => CellOp::TextEndsWith,
            };
            Ok(celula(ranges.clone(), op, vec![texto], estilo))
        }
        "containsBlanks" => Ok(celula(ranges.clone(), CellOp::IsEmpty, vec![], estilo)),
        "notContainsBlanks" => Ok(celula(ranges.clone(), CellOp::IsNotEmpty, vec![], estilo)),
        "containsErrors" => Ok(por_faixa(ranges, &format!("ISERROR({tl})"), &estilo)),
        "notContainsErrors" => Ok(por_faixa(ranges, &format!("NOT(ISERROR({tl}))"), &estilo)),
        "duplicateValues" => Ok(celula(ranges.clone(), CellOp::Duplicate, vec![], estilo)),
        "uniqueValues" => Ok(celula(ranges.clone(), CellOp::Unique, vec![], estilo)),
        "aboveAverage" => {
            if r.attr("stdDev").is_some_and(|s| s != "0") {
                return Err(feathersheet_core::tr!("desvio padrão").into());
            }
            let acima = !matches!(r.attr("aboveAverage"), Some("0") | Some("false"));
            if matches!(r.attr("equalAverage"), Some("1") | Some("true")) {
                let abs = absoluto(ranges).ok_or_else(|| feathersheet_core::tr!("igual à média em vários intervalos"))?;
                let sinal = if acima { ">=" } else { "<=" };
                return Ok(por_faixa(ranges, &format!("{tl}{sinal}AVERAGE({abs})"), &estilo));
            }
            Ok(celula(ranges.clone(), if acima { CellOp::AboveAverage } else { CellOp::BelowAverage }, vec![], estilo))
        }
        "top10" => {
            if matches!(r.attr("percent"), Some("1") | Some("true")) {
                return Err(feathersheet_core::tr!("10 primeiros por percentual").into());
            }
            let n: u32 = r.attr("rank").and_then(|v| v.parse().ok()).unwrap_or(10);
            let abs = absoluto(ranges).ok_or_else(|| feathersheet_core::tr!("10 primeiros em vários intervalos"))?;
            let abaixo = matches!(r.attr("bottom"), Some("1") | Some("true"));
            let expr = if abaixo { format!("RANK({tl},{abs},1)<={n}") } else { format!("RANK({tl},{abs})<={n}") };
            Ok(por_faixa(ranges, &expr, &estilo))
        }
        "colorScale" => {
            let escala = r.child("colorScale").ok_or_else(|| feathersheet_core::tr!("sem escala"))?;
            let vos: Vec<&XmlNode> = escala.children_named("cfvo").collect();
            let cores: Vec<Option<String>> = escala.children_named("color").map(cor_de).collect();
            if vos.len() != cores.len() || !(2..=3).contains(&vos.len()) {
                return Err(feathersheet_core::tr!("escala com número de pontos inesperado").into());
            }
            let mut pts = Vec::new();
            for (vo, c) in vos.iter().zip(&cores) {
                pts.push(ponto(vo, c.clone().ok_or_else(|| feathersheet_core::tr!("cor de escala fora de RGB/tema"))?)?);
            }
            let max = pts.pop().unwrap();
            let min = pts.remove(0);
            let mid = pts.pop();
            Ok(vec![(ranges.clone(), Condition::ColorScale(ColorScaleCondition { min, mid, max }))])
        }
        "dataBar" => {
            let barra = r.child("dataBar").ok_or_else(|| feathersheet_core::tr!("sem barra"))?;
            let cor = barra.child("color").and_then(cor_de).ok_or_else(|| feathersheet_core::tr!("cor da barra fora de RGB/tema"))?;
            let vos: Vec<&XmlNode> = barra.children_named("cfvo").collect();
            let extremo = |vo: Option<&&XmlNode>, auto: ScalePointType| -> Option<ScalePoint> {
                let p = ponto(vo?, cor.clone()).ok()?;
                (p.point_type != auto).then_some(p)
            };
            Ok(vec![(
                ranges.clone(),
                Condition::DataBar(DataBarCondition {
                    color: cor.clone(),
                    negative_color: None,
                    hide_value: matches!(barra.attr("showValue"), Some("0") | Some("false")),
                    min: extremo(vos.first(), ScalePointType::Min),
                    max: extremo(vos.get(1), ScalePointType::Max),
                }),
            )])
        }
        "iconSet" => Err(feathersheet_core::tr!("conjunto de ícones").into()),
        "timePeriod" => Err(feathersheet_core::tr!("período de datas").into()),
        outro => Err(format!("tipo '{outro}'")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::xlsx_read::styles::read_styles;
    use crate::xlsx_read::xml::parse_xml;

    const ESTILOS: &str = r#"<styleSheet><dxfs count="2">
        <dxf><font><b/><color rgb="FF9C0006"/></font><fill><patternFill patternType="solid"><fgColor rgb="FFFFC7CE"/><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf>
        <dxf><font><i val="1"/></font></dxf></dxfs></styleSheet>"#;

    fn lidas(corpo: &str) -> Lidas {
        let xfs = read_styles(ESTILOS);
        let root = parse_xml(&format!("<worksheet><sheetData/>{corpo}</worksheet>")).unwrap();
        ler(&root, &xfs)
    }

    fn regra(v: &serde_json::Value) -> ConditionalRule {
        serde_json::from_value(v.clone()).unwrap()
    }

    #[test]
    fn expression_le_formula_e_estilo_do_dxf() {
        let l = lidas(r#"<conditionalFormatting sqref="A2:C9"><cfRule type="expression" dxfId="0" priority="1"><formula>AND(ISNUMBER($A2),$A2&gt;10.5)</formula></cfRule></conditionalFormatting>"#);
        assert_eq!((l.total, l.regras.len(), l.fora.len()), (1, 1, 0));
        let r = regra(&l.regras[0]);
        assert_eq!(r.ranges, vec![Faixa { r0: 1, c0: 0, r1: 8, c1: 2 }]);
        let Condition::Cell(c) = r.condition else { panic!() };
        assert_eq!(c.op, CellOp::Formula);
        assert_eq!(c.values, vec!["=E(ÉNÚM($A2);$A2>10,5)".to_string()]);
        assert_eq!(c.style.bg.as_deref(), Some("#ffc7ce"));
        assert_eq!(c.style.color.as_deref(), Some("#9c0006"));
        assert_eq!(c.style.bold, Some(true));
    }

    #[test]
    fn expression_em_varios_intervalos_desloca_pelo_canto_de_cada_um() {
        let l = lidas(r#"<conditionalFormatting sqref="A1:A3 C5:C7"><cfRule type="expression" dxfId="1" priority="1"><formula>A1&gt;0</formula></cfRule></conditionalFormatting>"#);
        assert_eq!(l.regras.len(), 2);
        let f = |i: usize| match regra(&l.regras[i]).condition { Condition::Cell(c) => c.values[0].clone(), _ => panic!() };
        assert_eq!(f(0), "=A1>0");
        assert_eq!(f(1), "=C5>0");
    }

    #[test]
    fn cell_is_literal_vira_operador_e_referencia_vira_formula() {
        let l = lidas(r#"<conditionalFormatting sqref="F24"><cfRule type="cellIs" dxfId="0" priority="2" operator="notEqual"><formula>100%</formula></cfRule></conditionalFormatting>
            <conditionalFormatting sqref="B2:B5"><cfRule type="cellIs" dxfId="0" priority="1" operator="between"><formula>1.5</formula><formula>"x"</formula></cfRule><cfRule type="cellIs" priority="3" operator="greaterThan"><formula>$D$1</formula></cfRule></conditionalFormatting>"#);
        assert_eq!(l.regras.len(), 3);
        let c = |i: usize| match regra(&l.regras[i]).condition { Condition::Cell(c) => c, _ => panic!() };
        // Ordem por prioridade.
        assert_eq!((c(0).op, c(0).values), (CellOp::Between, vec!["1,5".to_string(), "x".to_string()]));
        assert_eq!((c(1).op, c(1).values), (CellOp::NotEqual, vec!["100%".to_string()]));
        assert_eq!((c(2).op, c(2).values), (CellOp::Formula, vec!["=B2>($D$1)".to_string()]));
    }

    #[test]
    fn texto_vazio_repetido_e_media() {
        let l = lidas(r#"<conditionalFormatting sqref="A1:A9">
            <cfRule type="containsText" dxfId="0" priority="1" operator="containsText" text="abc"><formula>NOT(ISERROR(SEARCH("abc",A1)))</formula></cfRule>
            <cfRule type="beginsWith" dxfId="0" priority="2" operator="beginsWith" text="ab"><formula>LEFT(A1,2)="ab"</formula></cfRule>
            <cfRule type="containsBlanks" dxfId="0" priority="3"><formula>LEN(TRIM(A1))=0</formula></cfRule>
            <cfRule type="duplicateValues" dxfId="0" priority="4"/>
            <cfRule type="aboveAverage" dxfId="0" priority="5" aboveAverage="0"/>
            <cfRule type="top10" dxfId="0" priority="6" rank="3"/>
            <cfRule type="iconSet" priority="7"><iconSet iconSet="3Arrows"/></cfRule>
            </conditionalFormatting>"#);
        assert_eq!((l.total, l.regras.len()), (7, 6));
        assert_eq!(l.fora, vec!["iconSet: conjunto de ícones".to_string()]);
        let ops: Vec<CellOp> = l.regras.iter().map(|v| match regra(v).condition { Condition::Cell(c) => c.op, _ => panic!() }).collect();
        assert_eq!(ops, [CellOp::TextContains, CellOp::TextStartsWith, CellOp::IsEmpty, CellOp::Duplicate, CellOp::BelowAverage, CellOp::Formula]);
        let Condition::Cell(top) = regra(&l.regras[5]).condition else { panic!() };
        assert_eq!(top.values, vec!["=ORDEM(A1;$A$1:$A$9)<=3".to_string()]);
    }

    #[test]
    fn escala_de_cor_e_barra_de_dados() {
        let l = lidas(r#"<conditionalFormatting sqref="B1:B9"><cfRule type="colorScale" priority="1"><colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/></colorScale></cfRule>
            <cfRule type="dataBar" priority="2"><dataBar showValue="0"><cfvo type="num" val="0"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule></conditionalFormatting>"#);
        assert_eq!(l.regras.len(), 2);
        let Condition::ColorScale(e) = regra(&l.regras[0]).condition else { panic!() };
        assert_eq!(e.min.color, "#f8696b");
        assert_eq!(e.mid.as_ref().map(|m| (m.point_type, m.value)), Some((ScalePointType::Percentile, Some(50.0))));
        let Condition::DataBar(d) = regra(&l.regras[1]).condition else { panic!() };
        assert!(d.hide_value);
        assert_eq!(d.color, "#638ec6");
        assert_eq!(d.min.map(|p| (p.point_type, p.value)), Some((ScalePointType::Number, Some(0.0))));
        assert!(d.max.is_none());
    }

    #[test]
    fn x14_com_dxf_embutido_e_formula_em_f() {
        let l = lidas(r#"<extLst><ext uri="{78C0D931}"><x14:conditionalFormattings><x14:conditionalFormatting><x14:cfRule type="expression" priority="1"><xm:f>Dados!$A$1&gt;0</xm:f><x14:dxf><fill><patternFill><bgColor rgb="FF00FF00"/></patternFill></fill></x14:dxf></x14:cfRule><xm:sqref>D1:D4</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst>"#);
        assert_eq!(l.regras.len(), 1);
        let Condition::Cell(c) = regra(&l.regras[0]).condition else { panic!() };
        assert_eq!(c.values, vec!["=Dados!$A$1>0".to_string()]);
        assert_eq!(c.style.bg.as_deref(), Some("#00ff00"));
    }

    /// Conta o que a planilha real tem e o que o leitor entendeu. Rodar com
    /// `--ignored --nocapture`. Só lê o arquivo.
    #[test]
    #[ignore]
    fn planilha_real_v6_0a() {
        use std::io::Read;
        let caminho = format!("{}/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm", std::env::var("HOME").unwrap());
        let bytes = std::fs::read(&caminho).expect("planilha do usuário");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut texto = |nome: &str| {
            let mut s = String::new();
            zip.by_name(nome).ok().map(|mut f| f.read_to_string(&mut s));
            s
        };
        let xfs = read_styles(&texto("xl/styles.xml"));
        let (mut no_arquivo, mut lidas_n) = (0, 0);
        for i in 1..=10 {
            let xml = texto(&format!("xl/worksheets/sheet{i}.xml"));
            let no_xml = xml.matches("<cfRule").count() + xml.matches("<x14:cfRule").count();
            let root = parse_xml(&xml).unwrap();
            let l = ler(&root, &xfs);
            println!("sheet{i}: no arquivo {no_xml}, lidas {}, fora {:?}", l.regras.len(), l.fora);
            no_arquivo += no_xml;
            lidas_n += l.regras.len();
        }
        println!("TOTAL: no arquivo {no_arquivo}, lidas {lidas_n}");
        assert!(lidas_n > 0);
    }
}
