//! Liga a saída OpenQuery a uma tabela real (Sheet.tb) e a um nome
//! estruturado. Compatível com os campos openQueryId/openQueryNome do TS.
use super::*;
use serde_json::{json, Value as Json};
use feathersheet_core::celulas::refs::Rect;

pub(super) struct Plano {
    pub nome: String,
    pub dados: String,
    pub area: Rect,
}

fn area_json(area: &Rect) -> Json {
    json!({"r0":area.r0, "r1":area.r1, "c0":area.c0, "c1":area.c1})
}

fn area_de(valor: &Json) -> Option<Rect> {
    let a = &valor["area"];
    let r0 = u32::try_from(a["r0"].as_u64()?).ok()?;
    let c0 = u32::try_from(a["c0"].as_u64()?).ok()?;
    let r1 = u32::try_from(a["r1"].as_u64()?).ok()?;
    let c1 = u32::try_from(a["c1"].as_u64()?).ok()?;
    if r0 > r1 || c0 > c1 { return None; }
    Some(Rect::new(r0,c0,r1,c1))
}
fn cruza(a: &Rect, b: &Rect) -> bool {
    a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1
}

fn tabelas(w: &Workbook, destino: Option<usize>) -> Result<Vec<Json>, String> {
    let Some(indice) = destino else { return Ok(vec![]) };
    let aba = w.aba(indice).ok_or(tr!("Destino da consulta indisponível"))?;
    match aba.sheet.tb.as_deref() {
        Some(raw) => serde_json::from_str::<Vec<Json>>(raw)
            .map_err(|e| trf!("Não alterei tabelas: metadados inválidos ({e})")),
        None => Ok(vec![]),
    }
}

/// Prepara e valida ANTES da carga. Se qualquer tabela estranha intercepta
/// a área nova, recusa sem alterar células.
pub(super) fn preparar(
    w: &Workbook, destino: Option<usize>, id: &str, nome_consulta: &str,
    area: Rect,
) -> Result<Plano, String> {
    if area.r0 > area.r1 || area.c0 > area.c1 || area.r1 >= MAX_ROWS || area.c1 >= MAX_COLS {
        return Err(tr!("Área inválida para vínculo OpenQuery").into());
    }
    let mut lista = tabelas(w, destino)?;
    let vinculada = lista.iter().position(|t|
        t["openQueryId"].as_str() == Some(id));
    for (i, tabela) in lista.iter().enumerate() {
        if Some(i) == vinculada { continue; }
        if let Some(outra) = area_de(tabela) {
            if cruza(&area, &outra) {
                return Err(tr!("A área da consulta atravessa outra tabela; não sobrescrevi o destino").into());
            }
        }
    }
    let nome = if let Some(i) = vinculada {
        lista[i]["nome"].as_str().ok_or(tr!("Tabela vinculada não tem nome"))?.to_owned()
    } else {
        let nomes = crate::app::tabelas::nomes_usados(w)?;
        let nomes: Vec<String> = nomes.into_iter().chain(w.nomeados().into_iter().map(|n|n.nome))
            .collect();
        let mut seq = 1u32;
        loop {
            let nome = format!("TabelaQuery{seq}");
            if !nomes.iter().any(|t|t.eq_ignore_ascii_case(&nome)) { break nome; }
            seq = seq.saturating_add(1);
            if seq > 1_000_000 {
                return Err(tr!("Não foi possível gerar um nome de tabela livre").into());
            }
        }
    };
    let item = if let Some(i) = vinculada {
        let mut antigo = lista.remove(i);
        antigo["area"] = area_json(&area);
        antigo["openQueryId"] = json!(id);
        antigo["openQueryNome"] = json!(nome_consulta);
        antigo
    } else {
        json!({
            "id": format!("tab_{nome}"),
            "nome": nome,
            "area": area_json(&area),
            "temCabecalho": true,
            "faixas": true,
            "estilo":{"tema":"azul","faixasColuna":false,"borda":true},
            "filtros":[],
            "openQueryId":id,
            "openQueryNome":nome_consulta,
        })
    };
    lista.push(item);
    let dados = serde_json::to_string(&lista).map_err(|e|e.to_string())?;
    Ok(Plano { nome, dados, area })
}

/// Desvincula sem apagar tabela, valores, estilos ou intervalo nomeado.
/// Todas as listas são validadas antes de modificar o workbook.
pub(super) fn preparar_desvinculacao(
    w: &Workbook, id: &str,
) -> Result<Vec<(usize, String)>, String> {
    let mut alteracoes = Vec::new();
    for indice in 0..w.len() {
        let Some(raw) = w.aba(indice).and_then(|a| a.sheet.tb.as_deref()) else {
            continue;
        };
        let mut itens: Vec<Json> = serde_json::from_str(raw)
            .map_err(|e| trf!("Metadados de tabela inválidos na aba {}: {e}", indice + 1))?;
        let mut alterado = false;
        for item in &mut itens {
            if item["openQueryId"].as_str() == Some(id) {
                if let Some(obj) = item.as_object_mut() {
                    obj.remove("openQueryId");
                    obj.remove("openQueryNome");
                    alterado = true;
                }
            }
        }
        if alterado {
            alteracoes.push((indice, serde_json::to_string(&itens)
                .map_err(|e| e.to_string())?));
        }
    }
    Ok(alteracoes)
}

pub(super) fn aplicar_desvinculacao(w: &mut Workbook, plano: Vec<(usize, String)>) {
    for (aba, json) in plano {
        w.editar(aba, move |sheet| sheet.tb = Some(json));
    }
}

pub(super) fn instalar(w: &mut Workbook, aba: usize, plano: Plano) {
    let nome = plano.nome;
    let area = plano.area;
    w.editar(aba, |sheet| sheet.tb = Some(plano.dados));
    let nome_aba = w.aba(aba).expect("destino validado").sheet.name.clone();
    w.definir_nome(&nome, Rect::na_aba(
        area.r0,area.c0,area.r1,area.c1,nome_aba,
    ));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cria_tabela_vinculada_e_atualiza_mesma_entrada() {
        let mut wb = Workbook::new();
        let aba = wb.criar_aba("Query");
        let area1 = Rect::new(0,0,2,2);
        let plano = preparar(&wb,Some(aba),"query-1","Vendas",area1).unwrap();
        let nome = plano.nome.clone();
        instalar(&mut wb,aba,plano);
        let area2 = Rect::new(0,0,3,2);
        let novo = preparar(&wb,Some(aba),"query-1","Vendas 2",area2).unwrap();
        assert_eq!(novo.nome, nome);
        instalar(&mut wb,aba,novo);
        let tabelas = tabelas(&wb,Some(aba)).unwrap();
        assert_eq!(tabelas.len(),1);
        assert_eq!(tabelas[0]["openQueryId"],"query-1");
        assert_eq!(tabelas[0]["area"]["r1"],3);
        assert_eq!(wb.nomeados().iter().find(|n|n.nome == nome).unwrap().area.r1,3);
    }
    #[test]
    fn desvincular_preserva_tabela_e_intervalo_nomeado() {
        let mut wb = Workbook::new();
        let destino = wb.criar_aba("Resultado");
        let p = preparar(&wb,Some(destino),"id-1","Vendas",Rect::new(0,0,3,1)).unwrap();
        let nome = p.nome.clone();
        instalar(&mut wb,destino,p);
        let plano = preparar_desvinculacao(&wb,"id-1").unwrap();
        aplicar_desvinculacao(&mut wb,plano);
        let tabelas = tabelas(&wb,Some(destino)).unwrap();
        assert_eq!(tabelas.len(),1);
        assert!(tabelas[0].get("openQueryId").is_none());
        assert_eq!(tabelas[0]["nome"],nome);
        assert!(wb.nomeados().iter().any(|n|n.nome == nome));
    }

    #[test]
    fn recusa_colisao_com_outra_tabela_antes_de_escrever() {
        let mut wb = Workbook::new();
        let aba = wb.criar_aba("Query");
        let primeiro = preparar(&wb,Some(aba),"query-1","A",Rect::new(0,0,2,2)).unwrap();
        instalar(&mut wb,aba,primeiro);
        assert!(preparar(&wb,Some(aba),"query-2","B",Rect::new(1,1,4,4)).is_err());
        assert_eq!(tabelas(&wb,Some(aba)).unwrap().len(),1);
    }
}
