//! Ponte de fn-planilha.ts::countIf/sumIf para a semântica de critérios
//! do FeatherSheet Rust. Não duplica regras de comparação nem curingas.
use feathersheet_core::{
    celulas::{values::{Value, ErrorCode}},
    funcoes::{fn_helpers::make_predicate, fn_stats::{media_numerica, mediana_numerica}},
};
use serde_json::{json, Value as Json};

const LIMITE: usize = 500_000;
const MAX_PROFUNDIDADE: usize = 64;

pub(crate) fn achatar<'a>(valor: &'a Json, saida: &mut Vec<&'a Json>, nivel: usize) -> Result<(), String> {
    if nivel > MAX_PROFUNDIDADE {
        return Err("WorksheetFunction: matriz aninhada demais".into());
    }
    if let Json::Array(itens) = valor {
        for item in itens { achatar(item, saida, nivel+1)?; }
    } else {
        if saida.len() >= LIMITE {
            return Err("WorksheetFunction: limite de 500.000 valores".into());
        }
        saida.push(valor);
    }
    Ok(())
}

fn valor(v: &Json) -> Result<Value, String> {
    Ok(match v {
        Json::Null => Value::Empty,
        Json::String(s) => Value::Text(s.clone()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Number(n.as_f64().ok_or("Número inválido")?),
        Json::Object(m) if m.get("__ftCellError").and_then(Json::as_str).is_some() => {
            use feathersheet_core::celulas::values::{CellError, ErrorCode};
            let code = m.get("__ftCellError").and_then(Json::as_str).unwrap();
            let code = ErrorCode::from_code(code).ok_or("Código de erro inválido")?;
            let e = match m.get("detail").and_then(Json::as_str) {
                Some(d) => CellError::com_motivo(code, d),
                None => CellError::new(code),
            };
            Value::Error(e)
        }
        _ => return Err("Critério deve ser número, texto, booleano, erro ou null".into()),
    })
}

fn numero(v: &Json) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

/// A API antiga de WorksheetFunction restringia critérios numéricos a
/// valores numéricos, enquanto o comparador de fórmulas tem ordenação
/// entre tipos. A comparação e os curingas continuam no core: aqui
/// somente preservamos a classe do valor comparado.
fn exige_numero(criterio: &Value) -> bool {
    if matches!(criterio, Value::Number(_)) { return true; }
    let Value::Text(s) = criterio else { return false };
    let restante = ["<=",">=","<>","<",">","="]
        .iter().find_map(|prefixo| s.strip_prefix(prefixo))
        .unwrap_or(s.as_str());
    let resto = restante.trim();
    !resto.is_empty()
        && feathersheet_core::celulas::number::from_js_string(
            &resto.replacen(',',".",1)
        ).is_some()
}

fn pode_comparar(valor: &Value, exige_numero: bool) -> bool {
    !exige_numero || matches!(valor, Value::Number(_))
}

/// Implementa só as funções que retornam sempre número. Outras
/// WorksheetFunction dependem do transporte de CellError no host.

/// Erros de planilha não são exceções JavaScript. O marcador JSON
/// vira instância de CellError na API Boa, preservando `isError`.
fn erro_de_planilha(code: ErrorCode) -> Json {
    json!({"__ftCellError": code.as_str()})
}

/// WorksheetFunction para agregações numéricas já existentes no core.
fn media_ou_mediana(op: &str, dados: &Json) -> Result<Json, String> {
    let mut refs = Vec::new();
    achatar(dados.get(0).ok_or("WorksheetFunction: valor ausente")?, &mut refs, 0)?;
    let mut nums = refs.into_iter().filter_map(numero).collect::<Vec<_>>();
    let res = if op == "worksheetAverage" {
        media_numerica(&nums)
    } else {
        mediana_numerica(&mut nums)
    };
    Ok(match res {
        Ok(n) if n.is_finite() => json!(n),
        Ok(_) => erro_de_planilha(ErrorCode::Num),
        Err(code) => erro_de_planilha(code),
    })
}

/// As cinco agregações sem erro de domínio (sum/count/countA/max/min).
/// average/median e averageIf retornam erro de célula quando necessário.
/// Usa as mesmas referências e limites de memória do COUNTIF/ SUMIF.
fn agregacao(op: &str, dados: &Json) -> Result<Json, String> {
    let entrada = dados.get(0).ok_or("WorksheetFunction: valor ausente")?;
    let mut valores = Vec::new();
    achatar(entrada, &mut valores, 0)?;
    let mut soma = 0.0_f64;
    let mut numericos = 0usize;
    let mut preenchidos = 0usize;
    let mut minimo: Option<f64> = None;
    let mut maximo: Option<f64> = None;
    for v in valores {
        // VBA CountA: zero, false e objetos contam; null e "" nao.
        if !v.is_null() && v.as_str() != Some("") { preenchidos += 1; }
        if let Some(n) = numero(v) {
            numericos += 1;
            if op == "worksheetSum" { soma += n; }
            minimo = Some(minimo.map_or(n, |m| m.min(n)));
            maximo = Some(maximo.map_or(n, |m| m.max(n)));
        }
    }
    match op {
        "worksheetSum" if soma.is_finite() => Ok(json!(soma)),
        "worksheetSum" => Err("WorksheetFunction: soma nao finita".into()),
        "worksheetCount" => Ok(json!(numericos)),
        "worksheetCountA" => Ok(json!(preenchidos)),
        "worksheetMax" => Ok(json!(maximo.unwrap_or(0.0))),
        "worksheetMin" => Ok(json!(minimo.unwrap_or(0.0))),
        _ => Err(format!("WorksheetFunction desconhecida: {op}")),
    }
}

pub(crate) fn executar(op: &str, dados: &Json) -> Result<Json, String> {
    if matches!(op, "worksheetAverage" | "worksheetMedian") {
        return media_ou_mediana(op, dados);
    }
    if matches!(op, "worksheetSum" | "worksheetCount" | "worksheetCountA"
        | "worksheetMax" | "worksheetMin") {
        return agregacao(op, dados);
    }
    let mut criterio = valor(dados.get(1).ok_or("WorksheetFunction: critério ausente")?)?;
    let so_numero = exige_numero(&criterio);
    // O critério <>5 também seleciona textos, booleanos e vazios,
    // pois a implementação TS usa a negação da igualdade estrita.
    let apenas_numeros = so_numero
        && !matches!(&criterio, Value::Text(s) if s.starts_with("<>"));
    // A string "5" no JS antigo é um critério numérico, não uma
    // comparação de textos. O motor do core reconhece "=5".
    if so_numero {
        if let Value::Text(ref texto) = criterio {
            if !["<=",">=","<>","<",">","="].iter().any(|p|texto.starts_with(p)) {
                criterio = Value::Text(format!("={texto}"));
            }
        }
    }
    let corresponde = make_predicate(&criterio);
    let mut lista = Vec::new();
    achatar(dados.get(0).ok_or("WorksheetFunction: intervalo ausente")?, &mut lista, 0)?;
    if op == "worksheetCountIf" {
        let mut n = 0usize;
        for item in lista {
            let v = valor(item)?;
            if pode_comparar(&v, apenas_numeros) && corresponde(&v) { n += 1; }
        }
        return Ok(json!(n));
    }
    if !matches!(op, "worksheetSumIf" | "worksheetAverageIf") {
        return Err(format!("WorksheetFunction desconhecida: {op}"));
    }
    let mut outra = Vec::new();
    let somas = if dados.get(2).is_none() {
        &lista
    } else {
        achatar(&dados[2], &mut outra, 0)?;
        &outra
    };
    let mut total = 0.0_f64;
    let mut selecionados = Vec::new();
    for (i, item) in lista.iter().enumerate() {
        let v = valor(item)?;
        if pode_comparar(&v, apenas_numeros) && corresponde(&v) {
            if let Some(n) = somas.get(i).and_then(|v| numero(v)) {
                if op == "worksheetAverageIf" { selecionados.push(n); }
                else { total += n; }
            }
        }
    }
    if op == "worksheetAverageIf" {
        return Ok(match media_numerica(&selecionados) {
            Ok(n) if n.is_finite() => json!(n),
            Ok(_) => erro_de_planilha(ErrorCode::Num),
            Err(code) => erro_de_planilha(code),
        });
    }
    if !total.is_finite() {
        return Err("Soma não finita".into());
    }
    Ok(json!(total))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn soma_e_conta_com_criterio_numerico() {
        assert_eq!(executar("worksheetCountIf",&json!([[[2,6],[9,null],["texto",20]],">5"]))
            .unwrap(),json!(3));
        assert_eq!(executar("worksheetSumIf",&json!([[[2,6],[9,null],["texto",20]],">5"]))
            .unwrap(),json!(35.0));
        assert_eq!(executar("worksheetSumIf",&json!([[2,6,9],">5",[100,200,300]]))
            .unwrap(),json!(500.0));
    }

    #[test]
    fn coringas_sao_do_motor_canonico_de_formulas() {
        assert_eq!(executar("worksheetCountIf",&json!([["Abacaxi","ABACATE","uva"],"aba*"]))
            .unwrap(),json!(2));
        assert_eq!(executar("worksheetCountIf",&json!([["abc","abd"],"ab?"]))
            .unwrap(),json!(2));
    }

    #[test]
    fn criterios_numericos_nao_contam_textos_com_ordem_de_tipo() {
        let valores=json!([[2,6],[9,null],["texto",20]]);
        assert_eq!(executar("worksheetCountIf",&json!([valores,">5"])).unwrap(),json!(3));
        assert_eq!(executar("worksheetCountIf",&json!([[5,"5",null],"5"])).unwrap(),json!(1));
        // "<>5" é o contrário de "=5", que no motor casa também o texto "5" (#57).
        assert_eq!(executar("worksheetCountIf",&json!([[5,"5",null,6],"<>5"])).unwrap(),json!(2));
        assert_eq!(executar("worksheetSumIf",&json!([[5,6],">5",null])).unwrap(),json!(0.0));
    }

    #[test]
    fn agregacoes_rust_equivalentes_ao_typescript() {
        let dados=json!([[[1,2],[3,"4"],[null,true]]]);
        assert_eq!(executar("worksheetSum",&dados).unwrap(),json!(6.0));
        assert_eq!(executar("worksheetCount",&dados).unwrap(),json!(3));
        assert_eq!(executar("worksheetCountA",&dados).unwrap(),json!(5));
        assert_eq!(executar("worksheetMax",&dados).unwrap(),json!(3.0));
        assert_eq!(executar("worksheetMin",&dados).unwrap(),json!(1.0));
        assert_eq!(executar("worksheetMin",&json!([["",null]])).unwrap(),json!(0.0));
        assert_eq!(executar("worksheetMax",&json!([[-20,-4,-8]])).unwrap(),json!(-4.0));
        assert_eq!(executar("worksheetCountA",&json!([[0,false,"",null,{},[]]])).unwrap(),json!(3));
    }

    #[test]
    fn media_e_mediana_reutilizam_helpers_estatisticos() {
        assert_eq!(executar("worksheetAverage", &json!([[[2,4],["x",8]]])).unwrap(),json!(14.0/3.0));
        assert_eq!(executar("worksheetMedian", &json!([[7,1,3,5]])).unwrap(),json!(4.0));
        assert_eq!(executar("worksheetAverageIf", &json!([[2,6,9],">5",[100,200,300]]))
            .unwrap(),json!(250.0));
    }

    #[test]
    fn agregacoes_sem_numeros_devolvem_erro_de_planilha() {
        assert_eq!(executar("worksheetAverage", &json!([["",null]])).unwrap(),
            json!({"__ftCellError":"#DIV/0!"}));
        assert_eq!(executar("worksheetMedian", &json!([["",null]])).unwrap(),
            json!({"__ftCellError":"#NUM!"}));
        assert_eq!(executar("worksheetAverageIf",&json!([[1,2,3],">100"])).unwrap(),
            json!({"__ftCellError":"#DIV/0!"}));
    }

    #[test]
    fn agregacao_respeita_limite_de_memoria() {
        let muitos=vec![Json::Null; LIMITE+1];
        assert!(executar("worksheetSum",&json!([muitos])).is_err());
    }

    #[test]
    fn limita_quantidade_de_valores() {
        let dados=vec![Json::Null; LIMITE+1];
        assert!(executar("worksheetCountIf",&json!([dados,"x"])).is_err());
    }

    #[test]
    fn argumentos_incompletos_retornam_erro_sem_panico() {
        assert!(executar("worksheetCountIf", &json!([])).is_err());
        assert!(executar("worksheetSumIf", &json!([[1, 2, 3]])).is_err());
        assert!(executar("worksheetAverageIf", &json!({})).is_err());
        assert!(executar("worksheetCountIf", &json!([[1, 2], ">1"])).is_ok());
    }
}
