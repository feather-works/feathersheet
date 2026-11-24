//! `sheet.tableRange(nome, parte)`: o endereço de uma parte de uma tabela da aba (o `ListObject` do VBA), para o
//! conversor ligar `.Range`, `.DataBodyRange`, `.HeaderRowRange`, `.ListColumns(n).Range` e `.ListRows.Count` ao
//! que a API de intervalos já faz. A tabela é achada pelo nome (sem diferenciar maiúsculas) ou pela posição (1…).
use feathersheet_core::celulas::refs::{cell_name, Rect};
use feathersheet_core::planilha::sheet::Sheet;
use serde_json::Value as Json;

/// As tabelas da aba (sem os AutoFiltros soltos): nome, área, cabeçalho e linha de totais.
fn tabelas(sheet: &Sheet) -> Vec<(String, Rect, bool, bool)> {
    let lista: Vec<Json> = sheet.tb.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
    lista.iter().filter(|t| t["soFiltro"] != true).filter_map(|t| {
        let a = &t["area"];
        let n = |k: &str| a[k].as_u64().and_then(|x| u32::try_from(x).ok());
        Some((t["nome"].as_str().unwrap_or("").to_owned(), Rect::new(n("r0")?, n("c0")?, n("r1")?, n("c1")?),
            t["temCabecalho"] == true, t["mostrarTotais"] == true))
    }).collect()
}

/// O endereço (`A2:D10`) de `parte`: `range` (com cabeçalho e totais), `data`, `header`, `totals`,
/// `col:N`/`datacol:N` (coluna N a partir de 1), ou o número de linhas de dados em `rows`.
pub(crate) fn parte(sheet: &Sheet, quem: &Json, parte: &str) -> Result<Json, String> {
    let lista = tabelas(sheet);
    let achada = match quem {
        Json::String(nome) => lista.iter().find(|t| t.0.eq_ignore_ascii_case(nome)),
        outro => outro.as_f64().filter(|n| *n >= 1.0).and_then(|n| lista.get(n as usize - 1)),
    };
    let (_, a, cab, totais) = achada.ok_or_else(|| feathersheet_core::trf!("A aba não tem a tabela {quem}"))?;
    let dados_r0 = a.r0 + u32::from(*cab);
    let fim = a.r1 + u32::from(*totais);
    let end = |r0: u32, c0: u32, r1: u32, c1: u32| Json::String(format!("{}:{}", cell_name(r0, c0), cell_name(r1, c1)));
    let coluna = |p: &str| -> Result<u32, String> {
        let n: u32 = p.parse().map_err(|_| feathersheet_core::trf!("Coluna da tabela inválida: {p}"))?;
        if n == 0 || a.c0 + n - 1 > a.c1 { return Err(feathersheet_core::trf!("A tabela não tem a coluna {n}")); }
        Ok(a.c0 + n - 1)
    };
    Ok(match parte {
        "range" => end(a.r0, a.c0, fim, a.c1),
        "data" => end(dados_r0, a.c0, a.r1.max(dados_r0), a.c1),
        "header" if *cab => end(a.r0, a.c0, a.r0, a.c1),
        "totals" if *totais => end(fim, a.c0, fim, a.c1),
        "rows" => Json::from(a.r1 + 1 - dados_r0.min(a.r1 + 1)),
        p => match p.split_once(':') {
            Some(("col", n)) => { let c = coluna(n)?; end(a.r0, c, fim, c) }
            Some(("datacol", n)) => { let c = coluna(n)?; end(dados_r0, c, a.r1.max(dados_r0), c) }
            _ => return Err(feathersheet_core::trf!("Parte da tabela sem equivalente: {p}")),
        },
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn partes_da_tabela_pelo_nome_e_pela_posicao() {
        let mut s = Sheet::new("A");
        s.tb = Some(json!([
            {"nome": "Filtro", "soFiltro": true, "area": {"r0": 0, "c0": 9, "r1": 3, "c1": 9}},
            {"nome": "Vendas", "temCabecalho": true, "mostrarTotais": true, "area": {"r0": 1, "c0": 1, "r1": 5, "c1": 3}},
        ]).to_string());
        let p = |q: Json, x: &str| parte(&s, &q, x).unwrap();
        assert_eq!(p(json!("vendas"), "range"), json!("B2:D7"));
        assert_eq!(p(json!(1), "data"), json!("B3:D6"));
        assert_eq!(p(json!("Vendas"), "header"), json!("B2:D2"));
        assert_eq!(p(json!("Vendas"), "totals"), json!("B7:D7"));
        assert_eq!(p(json!("Vendas"), "datacol:2"), json!("C3:C6"));
        assert_eq!(p(json!("Vendas"), "rows"), json!(4));
        assert!(parte(&s, &json!("Outra"), "range").is_err());
        assert!(parte(&s, &json!("Vendas"), "col:9").is_err());
    }
}
