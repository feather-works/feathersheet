//! Porta a API fn-planilha.ts de buscas de macro para Rust.
//! Não se usa uma segunda engine de fórmulas: critérios de MATCH
//! (coringas) vêm do fn_helpers::make_predicate canônico.
//! Matrizes são lidas por referência, sem transpor/copiar milhões de células.

use crate::funcoes_criterio::achatar;
use feathersheet_core::{
    celulas::values::{ErrorCode, Value},
    funcoes::fn_helpers::make_predicate,
};
use serde_json::{json, Value as Json};

const LIMITE: usize = 500_000;

fn erro(codigo: ErrorCode) -> Json {
    json!({"__ftCellError":codigo.as_str()})
}

fn indice(valor: &Json) -> Option<usize> {
    // Boa pode serializar o Number JavaScript 2 como JSON 2.0.
    // Aceitar inteiro em ponto flutuante evita rejeitar índices válidos.
    let n = valor.as_f64()?;
    (n.is_finite() && n >= 1.0 && n.fract() == 0.0
        && n <= LIMITE as f64).then_some(n as usize)
}

/// Sem coerção entre números e textos; igualdade textual ignora caixa,
/// preservando igualSolto de macros/fn-planilha.ts.
fn iguais(a: &Json, b: &Json) -> bool {
    match (a,b) {
        (Json::String(x), Json::String(y)) => x.to_lowercase()==y.to_lowercase(),
        (Json::Number(x), Json::Number(y)) => x.as_f64()==y.as_f64(),
        (Json::Bool(x), Json::Bool(y)) => x==y,
        (Json::Null, Json::Null) => true,
        _ => false,
    }
}

/// Modo aproximado de fn-planilha.ts: mesma classe e última linha
/// qualificada na ordem fornecida. Nunca ordena a tabela por conta própria.
fn comparacao_aproximada(a: &Json, b: &Json, tipo: i8) -> bool {
    let ordem = match (a,b) {
        (Json::Number(x),Json::Number(y)) => x.as_f64().zip(y.as_f64())
            .and_then(|(x,y)|x.partial_cmp(&y)),
        (Json::String(x),Json::String(y)) => Some(x.cmp(y)),
        (Json::Bool(x),Json::Bool(y)) => Some(x.cmp(y)),
        (Json::Null,Json::Null) => Some(std::cmp::Ordering::Equal),
        _ => None,
    };
    ordem.is_some_and(|o| if tipo >= 0 { o.is_le() } else { o.is_ge() })
}

/// Dimensões sem materialização; a primeira linha determina as colunas
/// aceitas por VLOOKUP, como a função TS original.
fn validar_matriz(tabela: &Json) -> Result<(usize,usize),String> {
    let linhas = tabela.as_array();
    let nlinhas = linhas.map_or(1,Vec::len);
    if nlinhas > LIMITE { return Err(feathersheet_core::tr!("WorksheetFunction: mais de 500.000 linhas").into()); }
    let mut total = 0usize;
    if let Some(linhas) = linhas {
        for linha in linhas {
            total = total.saturating_add(linha.as_array().map_or(1,Vec::len));
            if total > LIMITE {
                return Err(feathersheet_core::tr!("WorksheetFunction: limite de 500.000 valores").into());
            }
        }
    }
    let primeira = linhas.and_then(|l|l.first()).unwrap_or(tabela);
    let colunas = if linhas.is_some() && nlinhas == 0 { 0 }
        else { primeira.as_array().map_or(1,Vec::len) };
    Ok((nlinhas,colunas))
}

/// Equivalente a linhasDe(tabela)[r][c]; linhas escalares têm uma coluna.
fn obter(tabela: &Json, linha: usize, coluna: usize) -> Option<&Json> {
    let valor = match tabela {
        Json::Array(linhas) => linhas.get(linha)?,
        escalar if linha == 0 => escalar,
        _ => return None,
    };
    match valor {
        Json::Array(colunas) => colunas.get(coluna),
        escalar if coluna == 0 => Some(escalar),
        _ => None,
    }
}

/// VLOOKUP ou HLOOKUP. HLOOKUP varre a primeira linha diretamente,
/// ao contrário do TS que gerava uma matriz transposta temporária.
fn buscar(op: &str, dados: &Json) -> Result<Json,String> {
    let chave = dados.get(0).ok_or("WorksheetFunction: chave ausente")?;
    let tabela = dados.get(1).ok_or("WorksheetFunction: tabela ausente")?;
    let dimensao = dados.get(2).and_then(indice);
    let (nlinhas, ncolunas) = validar_matriz(tabela)?;
    let vertical = op == "worksheetVLookup";
    let limite_indice = if vertical { ncolunas } else { nlinhas };
    let Some(destino) = dimensao.filter(|n| *n <= limite_indice) else {
        return Ok(erro(ErrorCode::Ref));
    };
    let exato = dados.get(3).and_then(Json::as_bool).unwrap_or(true);
    let ntestes = if vertical { nlinhas } else { ncolunas };
    let mut encontrado = None;
    for i in 0..ntestes {
        let candidato = if vertical { obter(tabela,i,0) } else { obter(tabela,0,i) };
        let Some(candidato) = candidato else { continue };
        let combina = if exato { iguais(candidato,chave) }
            else { comparacao_aproximada(candidato,chave,1) };
        if combina {
            encontrado = Some(i);
            if exato { break; }
        }
    }
    Ok(match encontrado {
        None => erro(ErrorCode::Na),
        Some(i) => {
            let celula = if vertical { obter(tabela,i,destino-1) }
                else { obter(tabela,destino-1,i) };
            celula.cloned().unwrap_or(Json::Null)
        }
    })
}

fn localizar(dados: &Json) -> Result<Json,String> {
    let procurado = dados.get(0).ok_or("WorksheetFunction: valor ausente")?;
    let fonte = dados.get(1).ok_or("WorksheetFunction: lista ausente")?;
    let tipo = match dados.get(2) {
        None => 0.0,
        Some(v) => v.as_f64().filter(|n| n.is_finite())
            .ok_or_else(|| feathersheet_core::tr!("MATCH: tipo deve ser um número finito"))?,
    };
    let mut itens = Vec::new();
    achatar(fonte,&mut itens,0)?;
    let curinga = if tipo == 0.0 {
        procurado.as_str().filter(|s|s.contains('*')||s.contains('?'))
            .map(|s|make_predicate(&Value::Text(s.to_string())))
    } else { None };
    let mut posicao = None;
    for (idx,valor) in itens.into_iter().enumerate() {
        let combina = if tipo == 0.0 {
            if let Some(ref teste) = curinga {
                valor.as_str().is_some_and(|s|teste(&Value::Text(s.to_string())))
            } else { iguais(procurado,valor) }
        } else {
            comparacao_aproximada(valor,procurado,if tipo>0.0 {1} else {-1})
        };
        if combina {
            posicao=Some(idx+1);
            if tipo==0.0 {break}
        }
    }
    Ok(posicao.map_or_else(||erro(ErrorCode::Na),|n|json!(n)))
}

fn indexar(dados: &Json) -> Result<Json,String> {
    let tabela=dados.get(0).ok_or("WorksheetFunction: matriz ausente")?;
    let Some(linha)=dados.get(1).and_then(indice) else { return Ok(erro(ErrorCode::Ref)) };
    let coluna=dados.get(2).and_then(indice);
    let (nlinhas,_) = validar_matriz(tabela)?;
    let plano = !tabela.is_array()
        || tabela.as_array().and_then(|l|l.first()).is_none_or(|primeira|!primeira.is_array());
    // index(lista,n) usa o vetor plano, enquanto index(matriz,r,c)
    // usa linha/coluna (ambos índices baseados em 1).
    if plano && dados.get(2).is_none() {
        let mut itens=Vec::new();
        achatar(tabela,&mut itens,0)?;
        return Ok(itens.get(linha-1).map(|v|(*v).clone()).unwrap_or_else(||erro(ErrorCode::Ref)));
    }
    if linha>nlinhas {return Ok(erro(ErrorCode::Ref))}
    let Some(coluna)=coluna.or_else(||dados.get(2).is_none().then_some(1)) else {
        return Ok(erro(ErrorCode::Ref));
    };
    Ok(obter(tabela,linha-1,coluna-1).cloned().unwrap_or_else(||erro(ErrorCode::Ref)))
}

pub(crate) fn executar(op: &str, dados: &Json) -> Result<Json,String> {
    match op {
        "worksheetVLookup" | "worksheetHLookup" => buscar(op,dados),
        "worksheetMatch" => localizar(dados),
        "worksheetIndex" => indexar(dados),
        _ => Err(format!("WorksheetFunction desconhecida: {op}")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn buscas_exatas_e_aproximadas() {
        let tabela=json!([["A",10],["b",20],["C",30]]);
        assert_eq!(executar("worksheetVLookup",&json!(["B",tabela,2])).unwrap(),json!(20));
        assert_eq!(executar("worksheetVLookup",&json!(["Z",tabela,2])).unwrap(),erro(ErrorCode::Na));
        assert_eq!(executar("worksheetVLookup",&json!(["A",tabela,3])).unwrap(),erro(ErrorCode::Ref));
        let ordenada=json!([[2,20],[5,50],[10,100]]);
        assert_eq!(executar("worksheetVLookup",&json!([6,ordenada,2,false])).unwrap(),json!(50));
        assert_eq!(executar("worksheetHLookup",&json!([5,[[2,5,10],[20,50,100]],2])).unwrap(),json!(50));
    }

    #[test]
    fn corresponde_e_indice_preservam_a_base_um() {
        assert_eq!(executar("worksheetMatch",&json!(["ab*",["aba","ABACATE","uva"]])).unwrap(),json!(1));
        assert_eq!(executar("worksheetMatch",&json!([6,[2,5,10],1])).unwrap(),json!(2));
        assert_eq!(executar("worksheetMatch",&json!([7,[2,5,10],-1])).unwrap(),json!(3));
        assert_eq!(executar("worksheetMatch",&json!([99,[2,5,10]])).unwrap(),erro(ErrorCode::Na));
        assert_eq!(executar("worksheetIndex",&json!([[10,20,30],2])).unwrap(),json!(20));
        assert_eq!(executar("worksheetIndex",&json!([[[10,20],[30,40]],2,1])).unwrap(),json!(30));
        assert_eq!(executar("worksheetIndex",&json!([[10,20],99])).unwrap(),erro(ErrorCode::Ref));
        assert_eq!(executar("worksheetIndex",&json!([[10,20],2.0])).unwrap(),json!(20));
        assert_eq!(executar("worksheetIndex",&json!([[10,20],1.5])).unwrap(),erro(ErrorCode::Ref));
    }

    #[test]
    fn tipo_de_match_invalido_retorna_erro_sem_suprimir_rollback_da_macro() {
        assert!(executar("worksheetMatch",&json!(["a",["a"],{"tipo":0}])).is_err());
        assert!(executar("worksheetMatch",&json!(["a",["a"],"invalid"])).is_err());
    }

    #[test]
    fn evita_materializar_matriz_transposta_massiva() {
        let linhas=vec![json!([1]); LIMITE+1];
        assert!(executar("worksheetHLookup",&json!([1,linhas,1])).is_err());
    }
}
