//! `sheet_grafico`: listar, criar, editar e remover gráficos — o mesmo JSON do
//! editor de gráficos (`ChartConfig` + `position`), gravado no `g` da aba com
//! Ctrl+Z. Funciona com e sem o aplicativo; ao vivo o gráfico aparece na grade.
use feathersheet_core::grafico::chart::{ChartConfig, ChartType};
use feathersheet_io::graficos_salvos;
use serde_json::{json, Value as Json};

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

/// O nome em português → o nome do motor.
const TIPOS: &[(&str, &str)] = &[
    ("colunas", "column"), ("colunas_empilhadas", "column-stacked"), ("colunas_100", "column-stacked-100"),
    ("barras", "bar"), ("barras_empilhadas", "bar-stacked"), ("barras_100", "bar-stacked-100"),
    ("linhas", "line"), ("degraus", "step-line"), ("area", "area"), ("pizza", "pie"), ("dispersao", "scatter"),
    ("bolhas", "bubble"), ("combinado", "combo"), ("mapa_calor", "heatmap"), ("cascata", "waterfall"),
    ("histograma", "histogram"), ("radar", "radar"), ("candle", "candlestick"), ("gantt", "gantt"),
];

/// As medidas da grade (`grade.rs`): é nelas que a posição do gráfico é contada.
const LARGURA_PADRAO: f64 = 122.0;
const ALTURA_PADRAO: f64 = 28.0;

fn tipo_do_motor(t: &str) -> Result<&'static str, String> {
    TIPOS.iter().find(|(pt, motor)| *pt == t || *motor == t).map(|(_, m)| *m)
        .ok_or_else(|| format!("Tipo \"{t}\" desconhecido. Tipos: {}.", TIPOS.iter().map(|(p, _)| *p).collect::<Vec<_>>().join(", ")))
}

/// O canto de cima à esquerda da célula, em px da folha (como `scroll_em_pixels` do app).
pub(crate) fn pixels(sheet: &feathersheet_core::Sheet, linha: u32, coluna: u32) -> (f64, f64) {
    let padrao = sheet.largura_padrao.map_or(LARGURA_PADRAO, f64::from);
    let x = f64::from(coluna) * padrao + sheet.col_widths.iter().filter(|(i, _)| **i < coluna).map(|(_, w)| f64::from(*w) - padrao).sum::<f64>();
    let y = f64::from(linha) * ALTURA_PADRAO + sheet.row_heights.iter().filter(|(i, _)| **i < linha).map(|(_, h)| f64::from(*h) - ALTURA_PADRAO).sum::<f64>();
    (x, y)
}

fn descrever(v: &Json) -> String {
    let r = &v["range"];
    let area = nome_area(&feathersheet_core::celulas::refs::Rect::new(
        r["r0"].as_u64().unwrap_or(0) as u32, r["c0"].as_u64().unwrap_or(0) as u32,
        r["r1"].as_u64().unwrap_or(0) as u32, r["c1"].as_u64().unwrap_or(0) as u32));
    let tipo = v["type"].as_str().unwrap_or("?");
    let tipo = TIPOS.iter().find(|(_, m)| *m == tipo).map_or(tipo, |(p, _)| *p);
    let origem = v["sourceSheet"].as_str().map(|a| format!("{a}!")).unwrap_or_default();
    let p = &v["position"];
    format!("{} \"{}\": {tipo} de {origem}{area} · em x {} y {}, {}×{} px", v["id"].as_str().unwrap_or("?"),
        v["title"].as_str().unwrap_or(""), p["x"], p["y"], p["width"], p["height"])
}

/// A atualização com o intervalo de dados: `range` e, se os dados são de outra aba, `sourceSheet`.
fn com_intervalo(h: &dyn Hospede, args: &Json, aba_grafico: usize, valor: &mut Json) -> Result<Option<(u32, u32)>, String> {
    if arg_texto(args, "intervalo").is_none() { return Ok(None); }
    let (aba_dados, area) = intervalo(h, args, "intervalo")?;
    let a = area.normalized();
    valor["range"] = json!({ "r0": a.r0, "c0": a.c0, "r1": a.r1, "c1": a.c1 });
    valor["sourceSheet"] = if aba_dados == aba_grafico { Json::Null } else { json!(h.pasta().nomes()[aba_dados]) };
    Ok(Some((a.r0, a.c1)))
}

fn gravar(h: &mut dyn Hospede, aba: usize, novo: Option<String>) {
    h.pasta_mut().editar(aba, |sheet| { sheet.alterar_graficos(novo, false); });
    h.marcar_alterado();
}

pub fn grafico(h: &mut dyn Hospede, args: &Json) -> Saida {
    // A aba onde o gráfico fica: `aba_grafico`, senão a do intervalo, senão a ativa.
    let aba = match arg_texto(args, "aba_grafico") {
        Some(nome) => h.pasta().indice_por_nome(nome).ok_or_else(|| format!("Aba \"{nome}\" não existe."))?,
        None if arg_texto(args, "intervalo").is_some() => intervalo(h, args, "intervalo")?.0,
        None => aba_do_arg(h, args)?,
    };
    let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
    let lista = graficos_salvos::listar(sheet)?;
    match arg_texto(args, "acao").unwrap_or("listar") {
        "listar" => {
            let nome = &h.pasta().nomes()[aba];
            Ok(conteudo(if lista.is_empty() { format!("Nenhum gráfico em {nome}.") } else {
                format!("{} gráfico(s) em {nome}:\n{}", lista.len(), lista.iter().map(descrever).collect::<Vec<_>>().join("\n"))
            }))
        }
        acao @ ("criar" | "editar") => {
            let criar = acao == "criar";
            let id = match (criar, arg_texto(args, "id")) {
                (false, None) => return Err("Falta `id` (veja com acao listar).".into()),
                (false, Some(id)) if !lista.iter().any(|v| v["id"] == id) => return Err(format!("Nenhum gráfico \"{id}\" nesta aba.")),
                (_, Some(id)) => id.to_string(),
                (true, None) => (1..).map(|n| format!("mcp-{n}")).find(|c| !lista.iter().any(|v| v["id"] == c.as_str())).expect("sempre há id livre"),
            };
            let mut valor = json!({ "id": id });
            if criar {
                if arg_texto(args, "intervalo").is_none() { return Err("Falta `intervalo` com os dados (cabeçalho na primeira linha, categorias na primeira coluna).".into()); }
                valor = json!({ "id": id, "type": "column", "title": "", "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true });
            }
            let canto = com_intervalo(h, args, aba, &mut valor)?;
            if let Some(t) = arg_texto(args, "tipo") { valor["type"] = json!(tipo_do_motor(t)?); }
            if let Some(t) = arg_texto(args, "titulo") { valor["title"] = json!(t); }
            for (arg, campo) in [("series_em_colunas", "seriesInColumns"), ("cabecalho", "hasHeaders"), ("categorias", "hasCategoryLabels")] {
                if let Some(b) = args[arg].as_bool() { valor[campo] = json!(b); }
            }
            let p = &args["posicao"];
            let medida = |k: &str| p[k].as_f64().filter(|v| *v >= 0.0);
            if criar || p.is_object() {
                let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
                // Sem posição: ao lado dos dados (duas colunas depois), na altura do cabeçalho.
                let antiga = lista.iter().find(|v| v["id"] == id.as_str()).map(|v| v["position"].clone()).unwrap_or(Json::Null);
                let (x0, y0) = match (criar, canto) {
                    (true, Some((linha, coluna))) if aba == intervalo(h, args, "intervalo")?.0 => pixels(sheet, linha, coluna + 2),
                    _ => (antiga["x"].as_f64().unwrap_or(80.0), antiga["y"].as_f64().unwrap_or(80.0)),
                };
                valor["position"] = json!({
                    "x": medida("x").unwrap_or(x0), "y": medida("y").unwrap_or(y0),
                    "width": medida("largura").or(antiga["width"].as_f64()).unwrap_or(480.0),
                    "height": medida("altura").or(antiga["height"].as_f64()).unwrap_or(300.0),
                });
            }
            let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
            let novo = graficos_salvos::mesclado(sheet, &id, valor)?;
            // O que vai para o arquivo precisa abrir no editor de gráficos.
            let lido: Vec<Json> = serde_json::from_str(&novo).map_err(|e| e.to_string())?;
            let salvo = lido.into_iter().find(|v| v["id"] == id.as_str()).expect("acabou de entrar");
            let config: ChartConfig = serde_json::from_value(salvo.clone()).map_err(|e| format!("Gráfico inválido: {e}"))?;
            if config.tipo == ChartType::Outro { return Err("Tipo de gráfico desconhecido.".into()); }
            gravar(h, aba, Some(novo));
            Ok(conteudo(format!("Gráfico {}: {}", if criar { "criado" } else { "alterado" }, descrever(&salvo))))
        }
        "remover" => {
            let id = arg_texto(args, "id").ok_or("Falta `id` (veja com acao listar).")?;
            let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
            let novo = graficos_salvos::sem(sheet, id)?.ok_or_else(|| format!("Nenhum gráfico \"{id}\" nesta aba."))?;
            let vazio = novo == "[]";
            gravar(h, aba, (!vazio).then_some(novo));
            Ok(conteudo(format!("Gráfico {id} removido.")))
        }
        outra => Err(format!("Ação \"{outra}\" desconhecida: listar, criar, editar ou remover.")),
    }
}

#[cfg(test)]
mod testes {
    use crate::ferramentas::chamar;
    use crate::hospede::{Avulso, Hospede};
    use serde_json::json;

    fn texto(r: crate::ferramentas::Saida) -> String {
        r.unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn criar_editar_listar_remover_e_desfazer() {
        let mut h = Avulso::default();
        for (r, linha) in [["Mês", "Vendas"], ["Jan", "10"], ["Fev", "30"]].iter().enumerate() {
            for (c, v) in linha.iter().enumerate() { h.pasta.escrever_celula(0, r as u32, c as u32, (*v).into()); }
        }
        let t = texto(chamar(&mut h, "sheet_grafico", &json!({ "acao": "criar", "intervalo": "A1:B3", "tipo": "pizza", "titulo": "Vendas" })));
        assert!(t.contains("mcp-1 \"Vendas\": pizza de A1:B3"), "{t}");
        // Ao lado dos dados: coluna D (2 colunas depois de B), linha 1.
        assert!(t.contains("x 366.0 y 0.0, 480.0×300.0"), "{t}");
        let t = texto(chamar(&mut h, "sheet_grafico", &json!({ "acao": "editar", "id": "mcp-1", "tipo": "linhas", "posicao": { "largura": 600 } })));
        assert!(t.contains("linhas") && t.contains("600.0×300.0") && t.contains("\"Vendas\""), "{t}");
        assert!(chamar(&mut h, "sheet_grafico", &json!({ "acao": "criar", "intervalo": "A1:B3", "tipo": "xyz" })).is_err());
        assert!(texto(chamar(&mut h, "sheet_grafico", &json!({}))).contains("1 gráfico"));
        // Os dados em outra aba: o gráfico fica na aba pedida e guarda a de origem.
        h.pasta.criar_aba("Painel");
        let t = texto(chamar(&mut h, "sheet_grafico", &json!({ "acao": "criar", "intervalo": "Planilha1!A1:B3", "aba_grafico": "Painel" })));
        assert!(t.contains("colunas de Planilha1!A1:B3"), "{t}");
        chamar(&mut h, "sheet_desfazer", &json!({ "aba": "Painel" })).unwrap();
        assert!(h.pasta().aba(1).unwrap().sheet.g.is_none());
        texto(chamar(&mut h, "sheet_grafico", &json!({ "acao": "remover", "id": "mcp-1" })));
        assert!(h.pasta().aba(0).unwrap().sheet.g.is_none());
    }
}
