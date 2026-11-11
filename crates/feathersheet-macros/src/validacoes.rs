//! Porta setDataValidation / clearDataValidation de api-dados.ts.
//! Avaliar valores continua em core::operacoes::validation::validar.
//! Aqui só há adaptação da API JSON e alteração da lista de regras.
use feathersheet_core::{
    celulas::refs::{parse_range_ref, Rect},
    operacoes::validation::{DataOp, NumeroOp, Regra, TextoOp, Validacao, QuandoValidacao, MensagensValidacao},
    planilha::sheet::Sheet,
};
use serde_json::Value as Json;

fn texto<'a>(r: &'a Json, chave: &str) -> Result<&'a str,String> {
    r.get(chave).and_then(Json::as_str)
        .ok_or_else(||feathersheet_core::trf!("Validação: campo {chave} precisa ser texto"))
}
fn numero(r:&Json,chave:&str) -> Result<f64,String> {
    r.get(chave).and_then(Json::as_f64)
        .filter(|n|n.is_finite())
        .ok_or_else(||feathersheet_core::trf!("Validação: {chave} precisa ser número finito"))
}
fn string_js(v:&Json)->String {
    match v {
        Json::Null => "null".into(),
        Json::String(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        // JavaScript String([1,2]) -> "1,2", e não "[object Array]".
        Json::Array(items) => items.iter().map(|x|{
            if x.is_null() {String::new()} else {string_js(x)}
        }).collect::<Vec<_>>().join(","),
        Json::Object(_) => "[object Object]".into(),
    }
}
fn campo_texto_js(r:&Json,chave:&str) -> Result<String,String> {
    r.get(chave).map(string_js)
        .ok_or_else(||feathersheet_core::trf!("Validação: campo {chave} obrigatório"))
}

/// Executada antes de abrir a transação; nenhuma entrada malformada
/// deixa parte de uma validação gravada.
fn regra(r:&Json) -> Result<Regra,String> {
    let t=texto(r,"type")?;
    match t {
        "list" => {
            if let Some(origem)=r.get("source").and_then(Json::as_str) {
                // Intervalo da própria aba; outra aba, nome ou INDIRETO(…)
                // viram lista por fórmula, resolvida pelo motor (como no .xlsx).
                let corpo=origem.trim().trim_start_matches('=').trim();
                if corpo.is_empty() {
                    return Err(feathersheet_core::tr!("Origem da validação vazia").into());
                }
                match parse_range_ref(corpo).map(|r|r.normalized()) {
                    Some(intervalo) if intervalo.sheet.is_none() => {
                        if intervalo.area() > 10_000 {
                            return Err(feathersheet_core::tr!("Origem da lista limitada a 10.000 células").into());
                        }
                        Ok(Regra::Intervalo{origem:intervalo})
                    }
                    _ => Ok(Regra::Formula{formula:format!("={corpo}")}),
                }
            } else {
                let itens=r.get("values").and_then(Json::as_array)
                    .filter(|v|!v.is_empty() && v.len()<=10_000)
                    .ok_or_else(|| feathersheet_core::tr!("Validação de lista: informe values com 1 a 10.000 itens"))?;
                Ok(Regra::Lista{valores:itens.iter().map(string_js).collect()})
            }
        }
        "number" => {
            let op=r.get("operator").and_then(Json::as_str).unwrap_or("between");
            let (op,a,b)=match op {
                "between" => (NumeroOp::Entre, numero(r,"min")?,Some(numero(r,"max")?)),
                "greaterThan" => (NumeroOp::Maior, numero(r,"value")?,None),
                "lessThan" => (NumeroOp::Menor, numero(r,"value")?,None),
                "equal" => (NumeroOp::Igual, numero(r,"value")?,None),
                "notBetween" => (NumeroOp::Fora, numero(r,"min")?,Some(numero(r,"max")?)),
                "notEqual" => (NumeroOp::Diferente, numero(r,"value")?,None),
                "greaterThanOrEqual" => (NumeroOp::MaiorIgual, numero(r,"value")?,None),
                "lessThanOrEqual" => (NumeroOp::MenorIgual, numero(r,"value")?,None),
                _ => return Err(feathersheet_core::trf!("Operador numérico inválido: {op}")),
            };
            let inteiro=r.get("integer").and_then(Json::as_bool).unwrap_or(false);
            Ok(Regra::Numero{op,a,b,inteiro})
        }
        "date" => {
            let op=r.get("operator").and_then(Json::as_str).unwrap_or("between");
            let (op,a,b)=match op {
                "between" => (DataOp::Entre,campo_texto_js(r,"min")?,
                    Some(campo_texto_js(r,"max")?)),
                "before" => (DataOp::Antes,campo_texto_js(r,"value")?,None),
                "after" => (DataOp::Depois,campo_texto_js(r,"value")?,None),
                _ => return Err(feathersheet_core::trf!("Operador de data inválido: {op}")),
            };
            Ok(Regra::Data{op,a,b})
        }
        "text" => {
            let op=r.get("operator").and_then(Json::as_str).unwrap_or("contains");
            let op=match op {
                "contains"=>TextoOp::Contem,
                "startsWith"=>TextoOp::Comeca,
                "email"=>TextoOp::Email,
                "url"=>TextoOp::Url,
                _=>return Err(feathersheet_core::trf!("Operador de texto inválido: {op}")),
            };
            let valor=r.get("value").map(string_js);
            Ok(Regra::Texto{op,valor})
        }
        "checkbox" => Ok(Regra::Caixa{
            marcado:r.get("checked").map(string_js).unwrap_or_else(||"Sim".into()),
            desmarcado:r.get("unchecked").map(string_js).unwrap_or_else(||"Não".into()),
        }),
        _ => Err(feathersheet_core::trf!("Validação desconhecida: {t}")),
    }
}
fn intersecta(a:&Rect,b:&Rect)->bool {
    a.r0<=b.r1 && b.r0<=a.r1 && a.c0<=b.c1 && b.c0<=a.c1
}

/// Congela a nova lista de regras ANTES de alterar o workbook;
/// apenas o JSON final é enviado ao histórico do core.
pub(crate) fn preparar(
    sheet:&Sheet,area:&Rect, op:&str,opcoes:&Json,
) -> Result<Option<String>,String> {
    let mut existentes:Vec<Validacao> = match sheet.va.as_deref() {
        None => Vec::new(),
        Some(s) => serde_json::from_str(s)
            .map_err(|e|feathersheet_core::trf!("Metadados de validação inválidos: {e}"))?,
    };
    existentes.retain(|v|!v.ranges.iter().any(|r|intersecta(r,area)));
    if op=="setDataValidation" {
        if !opcoes.is_object() {
            return Err(feathersheet_core::tr!("setDataValidation exige um objeto de regra").into());
        }
        let regra=regra(opcoes)?;
        if opcoes.get("errorMessage").is_some_and(|v|!v.is_string()) {
            return Err("errorMessage deve ser texto".into());
        }
        existentes.push(Validacao{
            ranges:vec![area.clone()],regra,
            quando:if opcoes.get("mode").and_then(Json::as_str)==Some("warn"){
                QuandoValidacao::Avisar
            }else{QuandoValidacao::Recusar},
            mensagem:opcoes.get("errorMessage").and_then(Json::as_str)
                .map(str::to_owned),
            mensagens:{
                let texto=|k:&str|opcoes.get(k).and_then(Json::as_str).map(str::to_owned);
                MensagensValidacao{
                    entrada_titulo:texto("inputTitle"),entrada_texto:texto("inputMessage"),
                    erro_titulo:texto("errorTitle"),
                }
            },
        });
    }else if op!="clearDataValidation" {
        return Err(format!("Comando desconhecido: {op}"));
    }
    if existentes.is_empty() {Ok(None)}
    else {serde_json::to_string(&existentes).map(Some)
        .map_err(|e|feathersheet_core::trf!("Erro ao serializar validação: {e}"))}
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;
    use feathersheet_core::operacoes::validation::validar;
    use serde_json::json;

    #[test]
    fn serializacao_compativel_com_avaliador_rust_e_undo() {
        let mut s=Sheet::new("Dados");
        let area=Rect::new(0,0,9,0);
        let novo=preparar(&s,&area,"setDataValidation",
            &json!({"type":"number","operator":"between","min":10,"max":20,
                "mode":"warn","errorMessage":"Valor fora do intervalo"}))
            .unwrap();
        s.abrir_transacao();
        assert!(s.alterar_validacoes(novo,&area));
        s.fechar_transacao();
        let rules:Vec<Validacao>=serde_json::from_str(s.va.as_ref().unwrap()).unwrap();
        assert_eq!(rules.len(),1);
        assert_eq!(rules[0].quando,QuandoValidacao::Avisar);
        assert!(!validar(&Value::Number(7.0),&rules[0].regra,
            None::<fn(u32,u32)->Value>).ok);
        assert!(s.undo()); assert!(s.va.is_none());
        assert!(s.redo()); assert!(s.va.is_some());
    }
    #[test]
    fn limpar_so_regras_que_cruzam_a_area() {
        let mut s=Sheet::new("Dados");
        for (r0,c0) in [(0,0),(30,2)] {
            let area=Rect::cell(r0,c0);
            let novo=preparar(&s,&area,"setDataValidation",
                &json!({"type":"checkbox"})).unwrap();
            assert!(s.alterar_validacoes(novo,&area));
        }
        let a=Rect::cell(0,0);
        let novo=preparar(&s,&a,"clearDataValidation",&Json::Null).unwrap();
        assert!(s.alterar_validacoes(novo,&a));
        let rules:Vec<Validacao>=serde_json::from_str(s.va.as_ref().unwrap()).unwrap();
        assert_eq!(rules.len(),1);
        assert_eq!(rules[0].ranges,vec![Rect::cell(30,2)]);
    }
    #[test]
    fn lista_mantem_coercao_de_string_do_typescript() {
        let a=Rect::cell(0,0);
        let sheet=Sheet::new("Dados");
        let novo=preparar(&sheet,&a,"setDataValidation",
            &json!({"type":"list","values":[true,12,null,["A","B"]]}))
            .unwrap().unwrap();
        let regras:Vec<Validacao>=serde_json::from_str(&novo).unwrap();
        assert!(matches!(&regras[0].regra,Regra::Lista{valores}
            if valores==&vec!["true".to_string(),"12".to_string(),
                "null".to_string(),"A,B".to_string()]));
    }

    #[test]
    fn valida_regra_antes_de_alterar() {
        let sheet=Sheet::new("Dados");
        let a=Rect::cell(0,0);
        assert!(preparar(&sheet,&a,"setDataValidation",
            &json!({"type":"list","values":[]})).is_err());
        assert!(preparar(&sheet,&a,"setDataValidation",
            &json!({"type":"number","operator":"greaterThan"})).is_err());
        assert!(preparar(&sheet,&a,"setDataValidation",
            &json!({"type":"text","operator":"outro"})).is_err());
        assert!(sheet.va.is_none());
    }
}
