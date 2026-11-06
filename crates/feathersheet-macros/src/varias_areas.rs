//! Intervalo de várias áreas (`"A2:E2,A4:E9"`), como o `SpecialCells` do VBA
//! devolve: a API das macros fala de uma área por vez, então aqui cada pedido
//! vira um por área. Como no Excel, ler (`value`, `rows`) olha a primeira área;
//! escrever, limpar e formatar valem em todas; copiar junta as áreas no destino
//! (as linhas visíveis de um filtro coladas uma embaixo da outra). O que não
//! tem sentido com várias áreas (ordenar, mover, mesclar…) dá erro.
use crate::operacoes_valores::endereco_a1;
use feathersheet_core::celulas::refs::{cell_name, parse_range_ref, Rect};
use serde_json::{json, Value as Json};

/// Repetidas em cada área.
const POR_AREA: &[&str] = &["set", "clear", "clearContents", "clearFormats", "setFormula", "setFormulaR1C1",
    "setNumberFormat", "setFontFamily", "setFontSize", "setBold", "setItalic", "setUnderline", "setStrikethrough",
    "setFontColor", "setBackground", "setAlignHorizontal", "setAlignVertical", "setWrapText", "setTextRotation",
    "setBorder", "setOutline", "setHyperlink", "setDataValidation", "clearDataValidation"];

/// Leem só a primeira área, como o Excel (`Range("A1:A3,C1").Value` é o A1:A3).
const PRIMEIRA: &[&str] = &["area", "get", "raw", "getText", "values", "formulas", "getFormat", "isMerged", "rowCount", "columnCount"];

/// Pedem uma área só.
const UMA_SO: &[&str] = &["move", "autoFill", "sort", "removeDuplicates", "textToColumns", "insertCells", "deleteCells",
    "merge", "unmerge", "replace", "addConditionalFormat", "addColorScale", "addDataBar", "clearConditionalFormats",
    "intersect", "union", "offset", "resize", "getRegion", "autoFitColumns"];

/// As áreas, se `dados[1]` é um endereço com vírgula numa operação de intervalo.
fn areas<'a>(op: &str, dados: &'a Json) -> Option<Vec<&'a str>> {
    let todas = [POR_AREA, PRIMEIRA, UMA_SO, &["copy", "specialCells", "areasEspeciais", "getAddress"]];
    if !todas.iter().any(|l| l.contains(&op)) || !dados[0].is_u64() { return None; }
    let end = dados[1].as_str().filter(|e| e.contains(','))?;
    Some(end.split(',').map(str::trim).collect())
}

fn com(dados: &Json, i: usize, v: Json) -> Json {
    let mut d = dados.clone();
    d[i] = v;
    d
}

/// `Some` quando o pedido é de várias áreas: o resultado, pedindo cada área a `chamar`.
pub(crate) fn executar(op: &str, dados: &Json, chamar: &dyn Fn(&str, &Json) -> Result<Json, String>) -> Option<Result<Json, String>> {
    let lista = areas(op, dados)?;
    Some((|| {
        if UMA_SO.contains(&op) {
            return Err(format!("{op} não vale para um intervalo de várias áreas ({})", dados[1].as_str().unwrap_or("")));
        }
        if PRIMEIRA.contains(&op) { return chamar(op, &com(dados, 1, json!(lista[0]))); }
        match op {
            "getAddress" => {
                let partes: Result<Vec<String>, String> = lista.iter().map(|a| chamar(op, &com(dados, 1, json!(a)))
                    .map(|v| v.as_str().unwrap_or_default().to_string())).collect();
                Ok(json!(partes?.join(",")))
            }
            "specialCells" | "areasEspeciais" => {
                let mut todas = Vec::new();
                for a in &lista {
                    match chamar(op, &com(dados, 1, json!(a)))? {
                        Json::Array(v) => todas.extend(v),
                        Json::String(s) if !s.is_empty() => todas.push(Json::String(s)),
                        _ => {}
                    }
                }
                Ok(if op == "specialCells" { Json::Array(todas) }
                    else { json!(todas.iter().filter_map(Json::as_str).collect::<Vec<_>>().join(",")) })
            }
            "copy" => copiar_juntando(&lista, dados, chamar),
            "set" if dados[2].is_array() => Err("Valor em matriz num intervalo de várias áreas: escreva um valor só".into()),
            _ => {
                let mut ultimo = Json::Null;
                for a in &lista { ultimo = chamar(op, &com(dados, 1, json!(a)))?; }
                Ok(ultimo)
            }
        }
    })())
}

/// `Copy` de várias áreas: as linhas e colunas que elas ocupam vão juntas para o destino,
/// sem os buracos entre elas. Como no Excel, só se as áreas formam uma grade (as mesmas
/// colunas em todas as linhas, como as linhas visíveis de um filtro).
fn copiar_juntando(lista: &[&str], dados: &Json, chamar: &dyn Fn(&str, &Json) -> Result<Json, String>) -> Result<Json, String> {
    let rects: Vec<Rect> = lista.iter().map(|a| parse_range_ref(a).map(|r| r.normalized()).ok_or_else(|| format!("Endereço inválido: {a}")))
        .collect::<Result<_, _>>()?;
    let mut linhas: Vec<u32> = rects.iter().flat_map(|r| r.r0..=r.r1).collect();
    let mut colunas: Vec<u32> = rects.iter().flat_map(|r| r.c0..=r.c1).collect();
    linhas.sort_unstable(); linhas.dedup();
    colunas.sort_unstable(); colunas.dedup();
    let celulas: u64 = rects.iter().map(|r| u64::from(r.r1 - r.r0 + 1) * u64::from(r.c1 - r.c0 + 1)).sum();
    if celulas != linhas.len() as u64 * colunas.len() as u64 {
        return Err("Copiar várias áreas só vale quando elas formam uma grade (as mesmas linhas ou as mesmas colunas)".into());
    }
    let destino = dados[2].as_str().ok_or("Destino da cópia precisa ser endereço A1")?;
    let (prefixo, end) = match destino.rsplit_once('!') { Some((a, b)) => (format!("{a}!"), b), None => (String::new(), destino) };
    let alvo = parse_range_ref(end).ok_or_else(|| format!("Destino inválido: {destino}"))?.normalized();
    let posicao = |v: &[u32], x: u32| v.binary_search(&x).unwrap_or(0) as u32;
    for (r, a) in rects.iter().zip(lista) {
        let canto = cell_name(alvo.r0 + posicao(&linhas, r.r0), alvo.c0 + posicao(&colunas, r.c0));
        chamar("copy", &com(&com(dados, 1, json!(a)), 2, json!(format!("{prefixo}{canto}"))))?;
    }
    Ok(Json::Null)
}

/// As células em áreas retangulares, em ordem de leitura: as faixas de colunas de cada
/// linha, e as linhas seguidas com as mesmas faixas numa área só.
pub(crate) fn juntar(celulas: &[(u32, u32)]) -> Vec<Rect> {
    let mut por_linha: Vec<(u32, Vec<(u32, u32)>)> = Vec::new();
    for &(r, c) in celulas {
        match por_linha.last_mut() {
            Some((l, faixas)) if *l == r => match faixas.last_mut() {
                Some(f) if f.1 + 1 == c => f.1 = c,
                _ => faixas.push((c, c)),
            },
            _ => por_linha.push((r, vec![(c, c)])),
        }
    }
    let mut abertas: Vec<Rect> = Vec::new();
    let mut prontas: Vec<Rect> = Vec::new();
    let mut anterior: Option<(u32, &Vec<(u32, u32)>)> = None;
    for (r, faixas) in &por_linha {
        if anterior.is_some_and(|(l, f)| l + 1 == *r && f == faixas) {
            abertas.iter_mut().for_each(|a| a.r1 = *r);
        } else {
            prontas.append(&mut abertas);
            abertas = faixas.iter().map(|&(c0, c1)| Rect::new(*r, c0, *r, c1)).collect();
        }
        anterior = Some((*r, faixas));
    }
    prontas.append(&mut abertas);
    prontas
}

/// [`juntar`] como endereço da API (`"A2:E2,A4:E9"`); vazio se não há célula.
pub(crate) fn endereco(celulas: &[(u32, u32)]) -> String {
    juntar(celulas).iter().map(endereco_a1).collect::<Vec<_>>().join(",")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn junta_as_linhas_visiveis_em_areas() {
        let celulas = [(1, 0), (1, 1), (3, 0), (3, 1), (4, 0), (4, 1), (6, 0), (6, 2)];
        assert_eq!(endereco(&celulas), "A2:B2,A4:B5,A7,C7");
        assert_eq!(endereco(&[]), "");
    }
}
