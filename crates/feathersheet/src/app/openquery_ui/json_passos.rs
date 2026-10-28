//! JSON das etapas: TypeScript usa valores primitivos, Rust usa Value tipado.
use super::*;
use serde_json::{json, Value as Json};

fn campos_com_valor(tipo: &str) -> &'static [&'static str] {
    match tipo {
        "filtrar" => &["criterio"],
        "substituir" => &["de", "para"],
        "substituirErros" => &["para"],
        _ => &[],
    }
}

fn valor_para_rust(valor: Json) -> Json {
    match valor {
        Json::Null => json!("Empty"),
        Json::Bool(v) => json!({"Bool": v}),
        Json::String(v) => json!({"Text": v}),
        Json::Number(v) => json!({"Number": v.as_f64().unwrap_or(f64::NAN)}),
        objeto => objeto,
    }
}

fn valor_para_ts(valor: &Json) -> Option<Json> {
    if valor.as_str() == Some("Empty") { return Some(Json::Null); }
    let objeto = valor.as_object()?;
    if objeto.len() != 1 { return None; }
    for campo in ["Text", "Number", "Bool"] {
        if let Some(v) = objeto.get(campo) {
            return Some(v.clone());
        }
    }
    None
}

/// Lê JSON TypeScript e também a representação tipada anterior do Rust.
pub(super) fn ler(texto: &str) -> Result<Vec<Passo>, String> {
    let mut json: Json = serde_json::from_str(texto)
        .map_err(|erro| trf!("JSON inválido: {erro}"))?;
    let lista = json.as_array_mut().ok_or(tr!("As etapas precisam ser um array JSON"))?;
    for passo in lista {
        let Some(objeto) = passo.as_object_mut() else { continue };
        let tipo = objeto.get("tipo").and_then(Json::as_str)
            .unwrap_or("").to_owned();
        for campo in campos_com_valor(&tipo) {
            if let Some(valor) = objeto.get_mut(*campo) {
                if valor.is_object() || valor.as_str() == Some("Empty") { continue; }
                *valor = valor_para_rust(std::mem::take(valor));
            }
        }
    }
    serde_json::from_value(json)
        .map_err(|erro| trf!("Passo de consulta inválido: {erro}"))
}

/// Produz a estrutura de passos JSON do TypeScript, sem tags do enum Value.
pub(super) fn escrever(passos: &[Passo]) -> Result<String, String> {
    let mut json = serde_json::to_value(passos)
        .map_err(|erro| erro.to_string())?;
    if let Some(lista) = json.as_array_mut() {
        for passo in lista {
            let Some(objeto) = passo.as_object_mut() else { continue };
            let tipo = objeto.get("tipo").and_then(Json::as_str)
                .unwrap_or("").to_owned();
            for campo in campos_com_valor(&tipo) {
                if let Some(valor) = objeto.get_mut(*campo) {
                    if let Some(ts) = valor_para_ts(valor) {
                        *valor = ts;
                    }
                }
            }
        }
    }
    serde_json::to_string_pretty(&json).map_err(|erro| erro.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn filtro_com_criterio_typescript_primitivo() {
        let passos = ler(r#"[{"tipo":"filtrar","col":0,"criterio":">=100"}]"#).unwrap();
        assert!(matches!(
            &passos[0],
            Passo::Filtrar { criterio: Value::Text(v), .. } if v == ">=100"
        ));
        assert!(escrever(&passos).unwrap().contains(r#""criterio": ">=100""#));
    }

    #[test]
    fn nulo_numero_e_booleano_roundtrip() {
        let entrada = r#"[
            {"tipo":"substituir","col":1,"de":null,"para":100.5},
            {"tipo":"substituir","col":2,"de":false,"para":"sim"}
        ]"#;
        let passos = ler(entrada).unwrap();
        let saida = escrever(&passos).unwrap();
        let json: Json = serde_json::from_str(&saida).unwrap();
        assert!(json[0]["de"].is_null());
        assert_eq!(json[0]["para"], 100.5);
        assert_eq!(json[1]["de"], false);
        assert_eq!(json[1]["para"], "sim");
    }

    #[test]
    fn aceita_formato_tipado_legado() {
        let passos = ler(r#"[{"tipo":"filtrar","col":0,"criterio":{"Text":"OK"}}]"#).unwrap();
        assert!(escrever(&passos).unwrap().contains(r#""criterio": "OK""#));
    }
}
