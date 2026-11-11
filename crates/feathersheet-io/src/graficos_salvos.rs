//! Atualização dos gráficos no bloco `g` existente, sem perder campos do legado.
use feathersheet_core::Sheet;
use serde_json::Value;

pub fn listar(sheet: &Sheet) -> Result<Vec<Value>, String> {
    match &sheet.g {
        None => Ok(Vec::new()),
        Some(json) => serde_json::from_str::<Vec<Value>>(json)
            .map_err(|_| feathersheet_core::tr!("As configurações de gráficos desta aba não puderam ser lidas.").into()),
    }
}
/// Mescla somente os campos editados; tipos/estilos desconhecidos são preservados.
pub fn salvar(sheet: &mut Sheet, id: &str, atualizacao: Value) -> Result<(), String> {
    sheet.g = Some(mesclado(sheet, id, atualizacao)?);
    Ok(())
}
/// O texto novo do bloco `g` com a atualização mesclada, sem gravar: quem
/// chama grava com `Sheet::alterar_graficos` (um passo de Ctrl+Z).
pub fn mesclado(sheet: &Sheet, id: &str, atualizacao: Value) -> Result<String, String> {
    let mut graficos = listar(sheet)?;
    let objeto = atualizacao
        .as_object()
        .ok_or_else(|| feathersheet_core::tr!("Configuração de gráfico inválida."))?;
    if let Some(existente) = graficos.iter_mut().find(|g| g["id"].as_str() == Some(id)) {
        let destino = existente
            .as_object_mut()
            .ok_or_else(|| feathersheet_core::tr!("Configuração de gráfico inválida."))?;
        for (campo, valor) in objeto {
            if campo == "style" {
                if let (Some(antigo), Some(novo)) = (
                    destino.get_mut(campo).and_then(Value::as_object_mut),
                    valor.as_object(),
                ) {
                    antigo.extend(novo.clone());
                    continue;
                }
            }
            destino.insert(campo.clone(), valor.clone());
        }
    } else {
        graficos.push(atualizacao);
    }
    serde_json::to_string(&graficos).map_err(|e| e.to_string())
}
/// O texto novo do bloco `g` sem o gráfico `id` (`None` se ele não existe).
pub fn sem(sheet: &Sheet, id: &str) -> Result<Option<String>, String> {
    let mut graficos = listar(sheet)?;
    let antes = graficos.len();
    graficos.retain(|v| v["id"].as_str() != Some(id));
    if antes == graficos.len() {
        return Ok(None);
    }
    serde_json::to_string(&graficos).map(Some).map_err(|e| e.to_string())
}
/// Remove apenas o gráfico identificado, preservando todas as demais entradas.
pub fn remover(sheet: &mut Sheet, id: &str) -> Result<bool, String> {
    let mut graficos = listar(sheet)?;
    let antes = graficos.len();
    graficos.retain(|v| v["id"].as_str() != Some(id));
    if antes == graficos.len() {
        return Ok(false);
    }
    sheet.g = Some(serde_json::to_string(&graficos).map_err(|e| e.to_string())?);
    Ok(true)
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn preserva_campos_e_roundtrip_fshz() {
        let mut w = feathersheet_core::planilha::workbook::Workbook::new();
        let s = &mut w.aba_mut(0).unwrap().sheet;
        s.g =
            Some(r##"[{"id":"a","position":{"x":20},"style":{"future":true},"other":42}]"##.into());
        salvar(
            s,
            "a",
            serde_json::json!({"id":"a","title":"Novo","style":{"showTitle":false}}),
        )
        .unwrap();
        salvar(s, "b", serde_json::json!({"id":"b","title":"Segundo"})).unwrap();
        let bytes = crate::da_workbook::pasta_gravar_bytes(&w, 0).unwrap();
        let lida = crate::fshz_read::ler_fshz(&bytes).unwrap();
        let reaberta = crate::para_workbook::para_workbook(&lida);
        let gs = listar(&reaberta.aba(0).unwrap().sheet).unwrap();
        assert_eq!(gs.len(), 2);
        assert_eq!(gs[0]["other"], 42);
        assert_eq!(gs[0]["style"]["future"], true);
        assert_eq!(gs[0]["title"], "Novo");
    }
    #[test]
    fn redimensionamento_preserva_posicao_e_fshz_apos_reabertura() {
        let mut livro = feathersheet_core::planilha::workbook::Workbook::new();
        let folha = &mut livro.aba_mut(0).unwrap().sheet;
        salvar(folha, "grafico", serde_json::json!({
            "id": "grafico",
            "position": {"x": 45, "y": 90, "width": 480, "height": 300},
            "_featherNative": {"cor": "#4285f4"},
            "campoLegado": 123
        })).unwrap();
        salvar(folha, "grafico", serde_json::json!({
            "position": {"x": 45, "y": 90, "width": 520, "height": 324}
        })).unwrap();
        let binario = crate::da_workbook::pasta_gravar_bytes(&livro, 0).unwrap();
        let modelo = crate::fshz_read::ler_fshz(&binario).unwrap();
        let reaberto = crate::para_workbook::para_workbook(&modelo);
        let graficos = listar(&reaberto.aba(0).unwrap().sheet).unwrap();
        assert_eq!(graficos.len(), 1);
        assert_eq!(graficos[0]["position"]["x"], 45);
        assert_eq!(graficos[0]["position"]["y"], 90);
        assert_eq!(graficos[0]["position"]["width"], 520);
        assert_eq!(graficos[0]["position"]["height"], 324);
        assert_eq!(graficos[0]["campoLegado"], 123);
        assert_eq!(graficos[0]["_featherNative"]["cor"], "#4285f4");
    }

    #[test]
    fn nao_sobrescreve_json_invalido() {
        let mut s = Sheet::default();
        s.g = Some("quebrado".into());
        assert!(salvar(&mut s, "a", serde_json::json!({"id":"a"})).is_err());
        assert_eq!(s.g.as_deref(), Some("quebrado"));
    }
}
