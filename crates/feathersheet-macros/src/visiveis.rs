//! As linhas que a macro vê escondidas, para `specialCells("visible")` (o
//! `SpecialCells(xlCellTypeVisible)` do VBA). O AutoFiltro que a macro pede só é
//! aplicado pelo aplicativo no fim (com as setas e o Ctrl+Z), mas a macro lê o
//! resultado logo depois: aqui os pedidos são repetidos sobre as tabelas da aba,
//! sem mexer em nada.
use crate::pasta::{AcaoFiltro, PedidoFiltro};
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::operacoes::{filtro, filtro_data, ops};
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::filtro_json::criterio_de_json;
use serde_json::Value as Json;

struct Filtro {
    area: Rect,
    cabecalho: bool,
    so_filtro: bool,
    criterios: Vec<(u32, Json)>,
    /// As faixas que o filtro já escondia antes da macro.
    ocultas: Vec<(u32, u32)>,
    mexido: bool,
}

fn faixas(v: &Json) -> Vec<(u32, u32)> {
    v.as_array().into_iter().flatten().filter_map(|f| {
        Some((u32::try_from(f.get(0)?.as_u64()?).ok()?, u32::try_from(f.get(1)?.as_u64()?).ok()?))
    }).collect()
}

fn da_tabela(t: &Json) -> Option<Filtro> {
    let a = &t["area"];
    let n = |k: &str| a[k].as_u64().and_then(|x| u32::try_from(x).ok());
    let area = Rect::new(n("r0")?, n("c0")?, n("r1")?, n("c1")?);
    let criterios = t["filtros"].as_array().into_iter().flatten()
        .filter_map(|f| Some((u32::try_from(f["col"].as_u64()?).ok()?, f["criterio"].clone()))).collect();
    Some(Filtro { area, cabecalho: t["temCabecalho"] == true, so_filtro: t["soFiltro"] == true,
        criterios, ocultas: faixas(&t["_featherOcultas"]), mexido: false })
}

fn dentro(faixas: &[(u32, u32)], r: u32) -> bool {
    faixas.iter().any(|&(a, b)| a <= r && r <= b)
}

/// Para cada linha de `r0..=r1`, se está escondida: altura zero (fora as que um filtro
/// da aba escondia e a macro mudou) ou escondida pelos filtros como a macro os deixou.
pub(crate) fn linhas_escondidas(pasta: &Workbook, aba: usize, r0: u32, r1: u32, pedidos: &[PedidoFiltro]) -> Result<Vec<bool>, String> {
    let viva = pasta.aba(aba).ok_or("Aba inexistente")?;
    let sheet = &viva.sheet;
    let altura_zero = |r: u32| sheet.linha_oculta(r);
    let meus: Vec<&AcaoFiltro> = pedidos.iter().filter(|p| p.aba == viva.id).map(|p| &p.acao).collect();
    if meus.is_empty() {
        return Ok((r0..=r1).map(altura_zero).collect());
    }
    let mut tabelas: Vec<Filtro> = sheet.tb.as_deref()
        .and_then(|b| serde_json::from_str::<Vec<Json>>(b).ok()).unwrap_or_default()
        .iter().filter_map(da_tabela).collect();
    let antes: Vec<(u32, u32)> = tabelas.iter().flat_map(|t| t.ocultas.iter().copied()).collect();
    for acao in meus {
        match acao {
            AcaoFiltro::Aplicar { area, coluna, criterio } => {
                let i = match tabelas.iter().position(|t| t.area.contains(area.r0, area.c0)) {
                    Some(i) => i,
                    None => {
                        let a = if area.r0 == area.r1 && area.c0 == area.c1 { ops::regiao_atual(sheet, area.clone()) } else { area.clone() };
                        tabelas.push(Filtro { area: a, cabecalho: true, so_filtro: true, criterios: Vec::new(), ocultas: Vec::new(), mexido: true });
                        tabelas.len() - 1
                    }
                };
                let t = &mut tabelas[i];
                let Some(coluna) = coluna else { continue };
                // A coluna é relativa ao intervalo da macro; o filtro pode começar noutra.
                let Some(rel) = (area.c0 + coluna - 1).checked_sub(t.area.c0) else { continue };
                t.mexido = true;
                t.criterios.retain(|(c, _)| *c != rel);
                if let Some(c) = criterio { t.criterios.push((rel, c.clone())); }
            }
            AcaoFiltro::MostrarTudo => tabelas.iter_mut().filter(|t| t.so_filtro).for_each(|t| { t.criterios.clear(); t.mexido = true; }),
            AcaoFiltro::Remover => tabelas.retain(|t| !t.so_filtro),
        }
    }
    // shortcut: "hoje" em UTC (o app usa o fuso local); só pesa no filtro de data dinâmico, que a macro não pede.
    let hoje = (feathersheet_core::celulas::relogio::agora_ms() / 86_400_000.0).floor() as i64 + filtro_data::serial_de_data(1970, 1, 1);
    let valor = |r, c| pasta.valor(aba, r, c);
    let cores = |r, c| filtro::cores_da_celula(sheet, r, c);
    let ctx = filtro::Contexto { valor: &valor, cores: Some(&cores), hoje };
    let mut agora: Vec<(u32, u32)> = Vec::new();
    for t in &tabelas {
        if !t.mexido { agora.extend(&t.ocultas); continue; }
        let (primeira, ultima) = (t.area.r0 + u32::from(t.cabecalho), t.area.r1.min(sheet.max_row).min(r1));
        let criterios: Vec<(u32, filtro::Criterio)> = t.criterios.iter()
            .filter(|(c, _)| *c <= t.area.c1 - t.area.c0)
            .map(|(c, j)| (t.area.c0 + c, criterio_de_json(j))).collect();
        if criterios.is_empty() || primeira > ultima { continue; }
        agora.extend(filtro::linhas_escondidas_com(primeira, ultima, &criterios, &ctx));
    }
    Ok((r0..=r1).map(|r| (altura_zero(r) && !dentro(&antes, r)) || dentro(&agora, r)).collect())
}
