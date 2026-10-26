//! Registro OpenQuery no campo legado Sheet.oq, já persistido pelo FSHZ.
//! Compatível com ConsultaItem do TypeScript. O campo adicional _feather
//! registra proveniência e área gerada para atualização segura in-place.
use super::*;
use serde_json::{json, Value as Json};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub(super) struct Salva {
    pub dados: Json,
}

fn registros(w: &Workbook, aba: usize) -> Result<Vec<Json>, String> {
    let raw = w.aba(aba).ok_or(tr!("Aba de consultas não existe"))?
        .sheet.oq.as_deref();
    match raw {
        Some(raw) => serde_json::from_str::<Vec<Json>>(raw)
            .map_err(|_| trf!("Metadados OpenQuery da aba {} estão inválidos; nada foi alterado", aba + 1)),
        None => Ok(Vec::new()),
    }
}

/// A versão TS pode duplicar a lista inteira em diversas abas.
pub(super) fn listar(w: &Workbook) -> Result<Vec<Salva>, String> {
    let mut vistas = HashSet::new();
    let mut resultado = Vec::new();
    for i in 0..w.len() {
        for consulta in registros(w, i)? {
            if let Some(id) = consulta.get("id").and_then(Json::as_str) {
                if vistas.insert(id.to_string()) {
                    resultado.push(Salva { dados: consulta });
                }
            }
        }
    }
    Ok(resultado)
}

pub(super) fn buscar(w: &Workbook, id: &str) -> Result<Salva, String> {
    listar(w)?.into_iter().find(|e| e.dados["id"].as_str() == Some(id))
        .ok_or_else(|| trf!("Consulta '{id}' não encontrada"))
}

pub(super) fn proximo_id(w: &Workbook) -> Result<String, String> {
    let usados: HashSet<_> = listar(w)?.into_iter()
        .filter_map(|q| q.dados["id"].as_str().map(str::to_owned)).collect();
    for seq in 1.. {
        let id = format!("query-{seq}");
        if !usados.contains(&id) { return Ok(id) }
    }
    unreachable!()
}

fn agora() -> String {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis();
    ms.to_string()
}

fn valor_json(v: &Value) -> Json {
    match v {
        Value::Empty => Json::Null,
        Value::Number(n) => json!(n),
        Value::Bool(b) => json!(b),
        Value::Text(s) => json!(s),
        Value::Error(e) => json!(e.to_string()),
    }
}

fn json_valor(v: &Json) -> Value {
    match v {
        Json::Null => Value::Empty,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n.as_f64().map(Value::Number).unwrap_or(Value::Empty),
        Json::String(s) => Value::Text(s.clone()),
        _ => Value::Empty,
    }
}

fn serializar_tabela(t: &Tabela) -> Json {
    json!({
        "cabecalho": t.cabecalho,
        "linhas": t.linhas.iter().map(|linha|
            linha.iter().map(valor_json).collect::<Vec<_>>()
        ).collect::<Vec<_>>(),
    })
}

pub(super) fn fonte_snapshot(reg: &Json) -> Result<Tabela, String> {
    let dados = reg.get("fonte").ok_or(tr!("Consulta sem fonte salvo"))?;
    let cabecalho = dados["cabecalho"].as_array()
        .ok_or(tr!("Consulta sem cabeçalho"))?.iter().map(|v| v.as_str().unwrap_or("").to_owned()).collect::<Vec<_>>();
    let linhas = dados["linhas"].as_array()
        .ok_or(tr!("Consulta sem linhas"))?;
    if (linhas.len() as u64).saturating_mul(cabecalho.len() as u64) > MAX_CELULAS_FONTE {
        return Err(tr!("Fonte armazenada supera limite de células para esta etapa").into());
    }
    let linhas = linhas.iter().map(|linha| {
        linha.as_array().ok_or(tr!("Linha de fonte inválida")).map(|celulas|
            celulas.iter().map(json_valor).collect::<Vec<_>>()
        )
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(Tabela { cabecalho, linhas })
}

fn area_json(area: &Rect) -> Json {
    json!({"r0":area.r0,"r1":area.r1,"c0":area.c0,"c1":area.c1})
}
fn ler_area(obj: &Json) -> Option<Rect> {
    let r0 = u32::try_from(obj["r0"].as_u64()?).ok()?;
    let c0 = u32::try_from(obj["c0"].as_u64()?).ok()?;
    let r1 = u32::try_from(obj["r1"].as_u64()?).ok()?;
    let c1 = u32::try_from(obj["c1"].as_u64()?).ok()?;
    (r0 <= r1 && c0 <= c1 && r1 < MAX_ROWS && c1 < MAX_COLS)
        .then(|| Rect::new(r0, c0, r1, c1))
}
pub(super) fn origem(reg: &Json) -> Option<(String, Rect)> {
    let f = &reg["_feather"];
    Some((f["origemAba"].as_str()?.into(), ler_area(&f["origemArea"])?))
}
pub(super) fn area_saida(reg: &Json) -> Option<Rect> {
    ler_area(&reg["_feather"]["areaSaida"])
}
pub(super) fn fontes_extra(reg: &Json) -> Vec<String> {
    reg["_feather"]["fontesAdicionais"].as_array().into_iter()
        .flatten().filter_map(Json::as_str).map(str::to_owned).collect()
}
pub(super) fn destino(reg: &Json) -> Option<&str> {
    reg["destino"]["nomeAba"].as_str()
}

pub(super) fn montar(
    id: &str, nome: &str, fonte: &Tabela, passos: &[Passo],
    origem_aba: &str, origem_area: &Rect, aba_destino: &str,
    area_saida: &Rect, fontes_adicionais: &[String],
) -> Result<Json, String> {
    let passos_json: Json = serde_json::from_str(&json_passos::escrever(passos)?)
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "id":id,
        "nome":nome,
        "fonte":serializar_tabela(fonte),
        "passos":passos_json,
        "destino":{"modo":"nova_aba","nomeAba":aba_destino},
        "status":{"sucesso":true,"ultimaAtualizacao":agora(),
                  "linhasAfetadas":area_saida.r1.saturating_sub(area_saida.r0)},
        "_feather":{
            "origemAba":origem_aba,
            "origemArea":area_json(origem_area),
            "areaSaida":area_json(area_saida),
            "fontesAdicionais":fontes_adicionais,
        }
    }))
}

/// Valida os metadados *antes* de alterar as células.
pub(super) fn preparar_gravacao(
    w: &Workbook, aba_destino: usize, registro: &Json
) -> Result<Vec<(usize, String)>, String> {
    let id = registro["id"].as_str().ok_or(tr!("Consulta sem id"))?;
    let mut mudancas = Vec::new();
    for i in 0..w.len() {
        let mut itens = registros(w, i)?;
        let antes = itens.len();
        itens.retain(|v| v["id"].as_str() != Some(id));
        if i == aba_destino { itens.push(registro.clone()); }
        if antes != itens.len() || i == aba_destino {
            mudancas.push((i, serde_json::to_string(&itens)
                .map_err(|e| e.to_string())?));
        }
    }
    Ok(mudancas)
}

pub(super) fn aplicar_gravacao(w: &mut Workbook, mudancas: Vec<(usize, String)>) {
    for (aba, raw) in mudancas {
        w.editar(aba, move |sheet| sheet.oq = Some(raw));
    }
}
pub(super) fn excluir(w: &mut Workbook, id: &str) -> Result<(), String> {
    let mut mudancas = Vec::new();
    for i in 0..w.len() {
        let mut lista = registros(w, i)?;
        let anterior = lista.len();
        lista.retain(|v| v["id"].as_str() != Some(id));
        if anterior != lista.len() {
            mudancas.push((i, serde_json::to_string(&lista).map_err(|e| e.to_string())?));
        }
    }
    if mudancas.is_empty() { return Err(tr!("Consulta não encontrada").into()); }
    aplicar_gravacao(w, mudancas);
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn registrar_e_recuperar_consulta_compatibilidade_ts() {
        let mut wb = Workbook::new();
        let fonte = Tabela { cabecalho: vec!["Nome".into()], linhas: vec![vec![Value::Text("A".into())]] };
        let reg = montar("query-1", "Teste", &fonte, &[], "Planilha1",
            &Rect::new(0,0,1,0), "Consulta", &Rect::new(0,0,1,0), &[]).unwrap();
        let dest = wb.criar_aba("Consulta");
        let plano = preparar_gravacao(&wb, dest, &reg).unwrap();
        aplicar_gravacao(&mut wb, plano);
        assert_eq!(listar(&wb).unwrap().len(), 1);
        let achou = buscar(&wb, "query-1").unwrap();
        assert_eq!(achou.dados["destino"]["nomeAba"], "Consulta");
        assert_eq!(fonte_snapshot(&achou.dados).unwrap().linhas, fonte.linhas);
        assert!(origem(&achou.dados).is_some());
    }

    #[test]
    fn atualizar_sem_duplicar_e_preservar_consultas_legadas() {
        let mut wb = Workbook::new();
        wb.aba_mut(0).unwrap().sheet.oq = Some(
            r#"[{"id":"antigo","nome":"Legado","fonte":{"cabecalho":[],"linhas":[]}}]"#.into());
        let aba = wb.criar_aba("Resultado");
        let fonte = Tabela { cabecalho: vec!["C".into()], linhas: vec![] };
        let novo = montar("query-1","Novo", &fonte,&[],"Planilha1",
            &Rect::new(0,0,0,0),"Resultado",&Rect::new(0,0,0,0),&[]).unwrap();
        let plano = preparar_gravacao(&wb, aba, &novo).unwrap();
        aplicar_gravacao(&mut wb, plano);
        assert_eq!(listar(&wb).unwrap().len(),2);
        let mudado = montar("query-1","Mudou", &fonte,&[],"Planilha1",
            &Rect::new(0,0,0,0),"Resultado",&Rect::new(0,0,0,0),&[]).unwrap();
        let plano = preparar_gravacao(&wb, aba, &mudado).unwrap();
        aplicar_gravacao(&mut wb, plano);
        assert_eq!(listar(&wb).unwrap().len(),2);
        assert_eq!(buscar(&wb,"antigo").unwrap().dados["nome"],"Legado");
        assert_eq!(buscar(&wb,"query-1").unwrap().dados["nome"],"Mudou");
    }

    #[test]
    fn metadados_invalidos_nao_sao_sobrescritos() {
        let mut wb = Workbook::new();
        wb.aba_mut(0).unwrap().sheet.oq = Some("{quebrado".into());
        assert!(listar(&wb).is_err());
        assert!(preparar_gravacao(&wb, 0, &json!({"id":"x"})).is_err());
    }
}
