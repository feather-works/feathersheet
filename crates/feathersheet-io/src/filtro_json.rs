//! O critério de filtro e a ordenação da tabela em JSON (como moram em
//! `tabelas[].filtros` e `tabelas[].ordenacao`), de e para os tipos do motor.
//!
//! Formas do critério (`filtros[].criterio`):
//!
//! - `["a", 10]` — lista de caixinhas;
//! - `{"op": "not_in", "val": [...]}` — fora da lista;
//! - `{"op": "contains", "val": "x"}` (e as outras regras de texto/número);
//! - `{"op": "cor_fundo", "cor": "#ff0000"}` / `{"op": "cor_fonte", "cor": null}`
//!   — por cor (`null` = sem preenchimento / automático);
//! - `{"op": "e", "val": [">=10", "<=20"]}` (ou `"ou"`) — duas condições do `CONT.SE`;
//! - `{"op": "top10", "topo": true, "n": 10, "percentual": false}`;
//! - `{"op": "dinamico", "tipo": "thisMonth"}` — nome do `dynamicFilter` do
//!   `.xlsx` (`today`, `lastWeek`, `Q3`, `M12`, `aboveAverage`…);
//! - qualquer outra coisa — critério clássico do `CONT.SE`.
//!
//! Ordenação (`ordenacao`):
//! `{"por_colunas": false, "niveis": [{"indice": 0, "descendente": false,
//! "por": "valor"|"cor_fundo"|"cor_fonte", "cor": "#ff0000", "lista": ["seg"]}]}`.
//! `indice` é relativo à tabela (coluna; linha se `por_colunas`). Como no
//! `.xlsx`, em ordenação por cor `descendente: false` = cor em cima.

use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::filtro::{AlvoCor, Criterio};
use feathersheet_core::operacoes::filtro_data::Dinamico;
use feathersheet_core::operacoes::ordenacao_ext::{ChaveOrdem, CriterioOrdem};
use serde_json::{json, Value as Json};

fn de_json(v: &Json) -> Value {
    match v {
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n.as_f64().map(Value::Number).unwrap_or(Value::Empty),
        Json::String(s) => Value::Text(s.clone()),
        _ => Value::Empty,
    }
}

fn para_json(v: &Value) -> Json {
    match v {
        Value::Number(n) => json!(n),
        Value::Bool(b) => json!(b),
        Value::Text(t) => json!(t),
        _ => Json::Null,
    }
}

/// O critério do JSON da tabela.
pub fn criterio_de_json(v: &Json) -> Criterio {
    if let Some(lista) = v.as_array() {
        return Criterio::Lista(lista.iter().map(de_json).collect());
    }
    let cor = || v["cor"].as_str().map(str::to_string);
    match v["op"].as_str() {
        Some("not_in") => Criterio::ForaDaLista(v["val"].as_array().map(|a| a.iter().map(de_json).collect()).unwrap_or_default()),
        Some("cor_fundo") => Criterio::Cor { alvo: AlvoCor::Fundo, cor: cor() },
        Some("cor_fonte") => Criterio::Cor { alvo: AlvoCor::Fonte, cor: cor() },
        Some("top10") => Criterio::Top10 {
            topo: v["topo"].as_bool().unwrap_or(true),
            n: v["n"].as_u64().unwrap_or(10).min(u32::MAX as u64) as u32,
            percentual: v["percentual"].as_bool().unwrap_or(false),
        },
        Some("dinamico") => match v["tipo"].as_str().and_then(Dinamico::de_nome_xlsx) {
            Some(d) => Criterio::Dinamico(d),
            // Tipo desconhecido: não esconder nada por engano.
            None => Criterio::Regra { op: "is_not_blank".into(), val: Value::Empty },
        },
        Some(op @ ("e" | "ou")) => {
            let par = |i: usize| de_json(&v["val"][i]);
            Criterio::Duas { e: op == "e", a: par(0), b: par(1) }
        }
        Some(op) => Criterio::Regra { op: op.to_owned(), val: de_json(&v["val"]) },
        None => Criterio::Classico(de_json(v)),
    }
}

/// O inverso de [`criterio_de_json`].
pub fn criterio_para_json(c: &Criterio) -> Json {
    match c {
        Criterio::Lista(l) => Json::Array(l.iter().map(para_json).collect()),
        Criterio::ForaDaLista(l) => json!({"op": "not_in", "val": l.iter().map(para_json).collect::<Vec<_>>()}),
        Criterio::Regra { op, val } => json!({"op": op, "val": para_json(val)}),
        Criterio::Classico(v) => para_json(v),
        Criterio::Duas { e, a, b } => json!({"op": if *e { "e" } else { "ou" }, "val": [para_json(a), para_json(b)]}),
        Criterio::Cor { alvo, cor } => {
            json!({"op": if *alvo == AlvoCor::Fundo { "cor_fundo" } else { "cor_fonte" }, "cor": cor})
        }
        Criterio::Top10 { topo, n, percentual } => json!({"op": "top10", "topo": topo, "n": n, "percentual": percentual}),
        Criterio::Dinamico(d) => json!({"op": "dinamico", "tipo": d.nome_xlsx()}),
    }
}

/// Os níveis de ordenação do JSON da tabela, com índices absolutos a partir
/// de `origem` (a coluna `c0` da tabela, ou a linha `r0` se `por_colunas`).
/// Devolve também `por_colunas`.
pub fn ordenacao_de_json(v: &Json, c0: u32, r0: u32) -> (Vec<ChaveOrdem>, bool) {
    let por_colunas = v["por_colunas"].as_bool().unwrap_or(false);
    let origem = if por_colunas { r0 } else { c0 };
    let chaves = v["niveis"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|n| {
                    let indice = origem + u32::try_from(n["indice"].as_u64()?).ok()?;
                    let descendente = n["descendente"].as_bool().unwrap_or(false);
                    let cor = n["cor"].as_str().map(str::to_string);
                    let criterio = match n["por"].as_str().unwrap_or("valor") {
                        "cor_fundo" => CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor, em_cima: !descendente },
                        "cor_fonte" => CriterioOrdem::Cor { alvo: AlvoCor::Fonte, cor, em_cima: !descendente },
                        _ => match n["lista"].as_array() {
                            Some(l) => CriterioOrdem::Lista(l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()),
                            None => CriterioOrdem::Valor,
                        },
                    };
                    Some(ChaveOrdem { indice, ascendente: !descendente, criterio })
                })
                .collect()
        })
        .unwrap_or_default();
    (chaves, por_colunas)
}

/// O inverso de [`ordenacao_de_json`].
pub fn ordenacao_para_json(chaves: &[ChaveOrdem], por_colunas: bool, c0: u32, r0: u32) -> Json {
    let origem = if por_colunas { r0 } else { c0 };
    let niveis: Vec<Json> = chaves
        .iter()
        .map(|k| {
            let indice = k.indice.saturating_sub(origem);
            match &k.criterio {
                CriterioOrdem::Valor => json!({"indice": indice, "descendente": !k.ascendente, "por": "valor"}),
                CriterioOrdem::Lista(l) => json!({"indice": indice, "descendente": !k.ascendente, "por": "valor", "lista": l}),
                CriterioOrdem::Cor { alvo, cor, em_cima } => json!({
                    "indice": indice,
                    "descendente": !em_cima,
                    "por": if *alvo == AlvoCor::Fundo { "cor_fundo" } else { "cor_fonte" },
                    "cor": cor,
                }),
            }
        })
        .collect();
    json!({"por_colunas": por_colunas, "niveis": niveis})
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn criterios_ida_e_volta() {
        let todos = vec![
            Criterio::Lista(vec![Value::Text("a".into()), Value::Number(2.0)]),
            Criterio::ForaDaLista(vec![Value::Text("a".into())]),
            Criterio::Regra { op: "contains".into(), val: Value::Text("x".into()) },
            Criterio::Cor { alvo: AlvoCor::Fundo, cor: Some("#ff0000".into()) },
            Criterio::Cor { alvo: AlvoCor::Fonte, cor: None },
            Criterio::Top10 { topo: false, n: 5, percentual: true },
            Criterio::Dinamico(Dinamico::Trimestre(3)),
            Criterio::Dinamico(Dinamico::AnoAteHoje),
            Criterio::Duas { e: false, a: Value::Text(">10".into()), b: Value::Number(3.0) },
        ];
        for c in todos {
            assert_eq!(criterio_de_json(&criterio_para_json(&c)), c);
        }
        // Compatível com o que já estava gravado.
        assert_eq!(criterio_de_json(&json!([ "a" ])), Criterio::Lista(vec![Value::Text("a".into())]));
        assert_eq!(criterio_de_json(&json!(">5")), Criterio::Classico(Value::Text(">5".into())));
    }

    #[test]
    fn ordenacao_ida_e_volta() {
        let chaves = vec![
            ChaveOrdem { indice: 3, ascendente: true, criterio: CriterioOrdem::Cor { alvo: AlvoCor::Fonte, cor: Some("#00f".into()), em_cima: true } },
            ChaveOrdem { indice: 4, ascendente: false, criterio: CriterioOrdem::Lista(vec!["seg".into(), "ter".into()]) },
            ChaveOrdem { indice: 5, ascendente: true, criterio: CriterioOrdem::Valor },
        ];
        let j = ordenacao_para_json(&chaves, false, 2, 0);
        assert_eq!(j["niveis"][0]["indice"], 1);
        let (volta, por_colunas) = ordenacao_de_json(&j, 2, 0);
        assert!(!por_colunas);
        assert_eq!(volta, chaves);
    }
}
