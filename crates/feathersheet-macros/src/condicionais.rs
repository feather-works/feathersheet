//! API de formatação condicional das macros JavaScript.
//! O avaliador é feathersheet-core::operacoes::rules; apenas o JSON r é alterado.
use feathersheet_core::{
    celulas::refs::Rect,
    operacoes::rules::{
        CellCondition, CellOp, ColorScaleCondition, CondStyle, Condition,
        ConditionalRule, DataBarCondition, Faixa, ScalePoint, ScalePointType,
    },
    planilha::sheet::Sheet,
};
use serde_json::Value as Json;
use crate::enderecos;

fn js(v: Option<&Json>) -> String {
    match v {
        None => "undefined".into(),
        Some(Json::Null) => "null".into(),
        Some(Json::String(s)) => s.clone(),
        Some(Json::Bool(b)) => b.to_string(),
        Some(Json::Number(n)) => n.to_string(),
        Some(Json::Array(a)) => a.iter().map(|v| {
            if v.is_null() { String::new() } else { js(Some(v)) }
        }).collect::<Vec<_>>().join(","),
        Some(Json::Object(_)) => "[object Object]".into(),
    }
}
fn truthy(v: &Json) -> bool {
    match v {
        Json::Null | Json::Bool(false) => false,
        Json::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Json::String(s) => !s.is_empty(),
        _ => true,
    }
}
fn cor(v: Option<&Json>, campo: &str) -> Result<String,String> {
    match v {
        Some(Json::String(s)) => enderecos::cor_hex(s),
        Some(Json::Number(n)) => {
            let n = n.as_i64().ok_or_else(||format!("{campo}: cor BGR deve ser inteira"))?;
            Ok(enderecos::vba_cor_hex(n as u32))
        }
        _ => Err(feathersheet_core::trf!("{campo}: informe uma cor #RGB, #RRGGBB ou BGR numérica")),
    }
}
fn ponto(t: ScalePointType, color: String, value: Option<f64>) -> ScalePoint {
    ScalePoint { point_type:t, color, value }
}
fn condicao(op: &str, o: &Json) -> Result<Condition,String> {
    if !o.is_object() {return Err(feathersheet_core::tr!("Formatação condicional exige objeto de opções").into());}
    match op {
        "addConditionalFormat" => {
            let nome = if o.get("type").and_then(Json::as_str)==Some("formula") {
                "formula"
            } else { o.get("operator").and_then(Json::as_str)
                .ok_or_else(|| feathersheet_core::tr!("Informe operator ou type:'formula'"))? };
            let operator: CellOp = serde_json::from_value(Json::String(nome.to_string()))
                .map_err(|_|feathersheet_core::trf!("Operador inválido: {nome}"))?;
            let n = match operator {
                CellOp::Between | CellOp::NotBetween => 2,
                CellOp::IsEmpty | CellOp::IsNotEmpty | CellOp::Duplicate
                | CellOp::Unique | CellOp::AboveAverage | CellOp::BelowAverage => 0,
                _ => 1,
            };
            let values = if operator==CellOp::Formula {
                vec![js(o.get("formula"))]
            } else if n==2 {
                vec![js(o.get("value")),js(o.get("value2"))]
            } else if n==1 {vec![js(o.get("value"))]} else {Vec::new()};
            let style=CondStyle {
                bg:o.get("fill").map(|v|cor(Some(v),"fill")).transpose()?,
                color:o.get("color").map(|v|cor(Some(v),"color")).transpose()?,
                bold:o.get("bold").map(truthy),
                italic:o.get("italic").map(truthy),
                ..CondStyle::default()
            };
            Ok(Condition::Cell(CellCondition { op:operator, values, style }))
        }
        "addColorScale" => Ok(Condition::ColorScale(ColorScaleCondition {
            min:ponto(ScalePointType::Min,cor(o.get("min"),"min")?,None),
            max:ponto(ScalePointType::Max,cor(o.get("max"),"max")?,None),
            mid:o.get("mid").filter(|v|truthy(v)).map(|v|
                cor(Some(v),"mid").map(|c|ponto(ScalePointType::Percentile,c,Some(50.0)))
            ).transpose()?,
        })),
        "addDataBar" => Ok(Condition::DataBar(DataBarCondition {
            color: match o.get("color") {
                Some(v)=>cor(Some(v),"color")?,
                None=>"#3b82f6".into(),
            },
            negative_color:None,
            hide_value:o.get("hideValue").is_some_and(truthy),
            min:None,max:None,
        })),
        _=>Err(feathersheet_core::trf!("Operação desconhecida: {op}")),
    }
}
fn cruza(a: &Faixa, b: &Rect) -> bool {
    a.r0<=b.r1 && b.r0<=a.r1 && a.c0<=b.c1 && b.c0<=a.c1
}

/// Não varre células: valida tudo e monta o JSON antes de abrir transação.
pub(crate) fn preparar(s: &Sheet, area: &Rect, op: &str, args: &Json) -> Result<Option<String>,String> {
    let mut regras: Vec<ConditionalRule> = match s.r.as_deref() {
        Some(r)=>serde_json::from_str(r).map_err(|e|feathersheet_core::trf!("JSON condicional inválido: {e}"))?,
        None=>Vec::new(),
    };
    if op=="clearConditionalFormats" {
        let antes=regras.len();
        regras.retain(|r|!r.ranges.iter().any(|v|cruza(v,area)));
        if regras.len()==antes {return Ok(s.r.clone());}
    } else {
        if regras.len()>=256 {return Err(feathersheet_core::tr!("Limite de 256 regras condicionais").into());}
        let condition=condicao(op,args)?;
        let mut i=regras.len();
        let id=loop {
            let id=format!("cf_macro_{i}");
            if regras.iter().all(|r|r.id!=id) {break id;}
            i+=1;
        };
        regras.push(ConditionalRule {
            id,
            ranges:vec![Faixa{r0:area.r0,c0:area.c0,r1:area.r1,c1:area.c1}],
            condition,
        });
    }
    if regras.is_empty() {Ok(None)}
    else {serde_json::to_string(&regras).map(Some).map_err(|e|e.to_string())}
}
