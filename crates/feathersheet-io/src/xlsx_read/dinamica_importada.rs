//! Dinâmicas vindas do `.xlsx`: completa a definição lida para o formato do
//! painel nativo e, quando o arquivo não trouxe o resultado nas células,
//! calcula a dinâmica no motor.
//!
//! O Excel grava o resultado da dinâmica nas células; o Google Planilhas
//! (e outros exportadores) gravam só a definição, com `refreshOnLoad="1"` e
//! sem `pivotCacheRecords` — as células da `location` ficam vazias. Sem este
//! passo a dinâmica simplesmente não aparecia ao abrir o arquivo.
//!
//! Regra: se a área do resultado já tem conteúdo (Excel), as células ficam
//! como vieram — os números são os que o Excel calculou. Se está vazia, o
//! motor calcula a partir da origem e grava sem passar pelo Ctrl+Z.

use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::dinamica::dinamica_layout::{self, Filtro, InfoLinha, Layout, Opcoes, TipoLinha};
use feathersheet_core::dinamica::exibicao::Extras;
use feathersheet_core::dinamica::pivot::{AgregacaoOp, DinamicaConfig, MedidaDinamica};
use feathersheet_core::planilha::workbook::Workbook;
use serde_json::{json, Value as Json};

fn retangulo(v: &Json) -> Option<Rect> {
    let n = |k: &str| v[k].as_u64().and_then(|x| u32::try_from(x).ok());
    let (r0, c0, r1, c1) = (n("r0")?, n("c0")?, n("r1")?, n("c1")?);
    (r0 <= r1 && c0 <= c1).then(|| Rect::new(r0, c0, r1, c1))
}

fn colunas(v: &Json) -> Option<Vec<usize>> {
    v.as_array()?.iter().map(|x| x.as_u64().map(|c| c as usize)).collect()
}

/// O mesmo mapa que o painel grava (`mapa_json` da GUI): o tipo de cada
/// linha, o nível e os botões +/−. A grade desenha a dinâmica por ele.
fn mapa(linhas: &[InfoLinha], rotulos: usize) -> Json {
    json!({
        "rotulos": rotulos,
        "linhas": linhas.iter().map(|linha| {
            let tipo = match linha.tipo {
                TipoLinha::Cabecalho => "c",
                TipoLinha::Grupo => "g",
                TipoLinha::Item => "i",
                TipoLinha::Subtotal => "s",
                TipoLinha::Total => "t",
            };
            json!([tipo, linha.nivel, linha.marcas.iter()
                .map(|m| json!([m.coluna, m.caminho, m.recolhido]))
                .collect::<Vec<_>>()])
        }).collect::<Vec<_>>(),
    })
}

/// O que gravar numa aba depois de calcular: células, formatos e a nova definição.
struct Gravacao {
    destino: usize,
    indice: usize,
    canto: (u32, u32),
    celulas: Vec<Vec<Value>>,
    /// (linha relativa, coluna relativa, formato) dos "mostrar como" em %.
    formatos: Vec<(u32, u32, &'static str)>,
    meta: Json,
}

/// Calcula uma dinâmica lida do `.xlsx`. `None` = definição que o motor não
/// entende (fica guardada como veio, sem resultado novo).
fn calcular(wb: &Workbook, destino: usize, indice: usize, meta: &Json) -> Option<Gravacao> {
    let origem = retangulo(&meta["fonte"]["area"])?;
    if origem.r1 <= origem.r0 {
        return None;
    }
    let nomes = wb.nomes();
    let folha_origem = match meta["fonte"]["aba"].as_str() {
        Some(nome) => nomes.iter().position(|n| n == nome)?,
        None => destino,
    };
    let linhas = colunas(&meta["linhas"])?;
    let cols = colunas(&meta["colunas"])?;
    let valores: Vec<MedidaDinamica> = meta["valores"].as_array()?.iter().map(|v| {
        Some(MedidaDinamica {
            coluna: v["col"].as_u64()? as usize,
            op: AgregacaoOp::do_str(v.get("como").or_else(|| v.get("op"))?.as_str()?),
        })
    }).collect::<Option<_>>()?;
    let fora = |c: &usize| *c < origem.c0 as usize || *c > origem.c1 as usize;
    if (linhas.is_empty() && cols.is_empty()) || valores.is_empty()
        || linhas.iter().chain(cols.iter()).chain(valores.iter().map(|m| &m.coluna)).any(fora) {
        return None;
    }
    let config = DinamicaConfig {
        origem: origem.clone(), linhas: linhas.clone(), colunas: cols.clone(),
        valores: valores.clone(), agrupar_data: None,
    };
    // Filtros e grupos recolhidos que o leitor achou (itens ocultos,
    // `pageFields`, `sd="0"`). Lista vazia = "(Tudo)", como no painel.
    let filtros = meta["filtros"].as_array().into_iter().flatten().filter_map(|f| {
        let coluna = f["col"].as_u64()? as usize;
        let valores: std::collections::HashSet<String> = f["valores"].as_array()?.iter()
            .filter_map(|v| v.as_str().map(str::to_string)).collect();
        (!valores.is_empty() && !fora(&coluna))
            .then(|| Filtro { coluna, valores, excluir: f["excluir"].as_bool().unwrap_or(false) })
    }).collect();
    let opcoes = Opcoes {
        layout: if meta["layout"] == "compacto" { Layout::Compacto } else { Layout::Tabular },
        repetir_rotulos: meta["repetirRotulos"].as_bool().unwrap_or(false),
        subtotais: meta["subtotais"].as_bool().unwrap_or(false),
        subtotal_embaixo: meta["subtotaisEmbaixo"].as_bool().unwrap_or(false),
        recolhidos: meta["recolhidos"].as_array().into_iter().flatten()
            .filter_map(|v| v.as_str().map(str::to_string)).collect(),
        filtros,
    };
    let extras: Extras = serde_json::from_value(meta["extras"].clone()).unwrap_or_default();
    let (r0, c0) = (origem.r0, origem.c0);
    let resultado = dinamica_layout::montar_com_extras(
        (origem.r1 - origem.r0 + 1) as usize,
        |r, c| wb.valor(folha_origem, r0 + r as u32, c0 + c as u32),
        &config, &opcoes, &extras,
    );
    let mut celulas: Vec<Vec<Value>> = resultado.matriz.rows().to_vec();
    let altura = celulas.len();
    let largura = celulas.iter().map(Vec::len).max().unwrap_or(0);
    if altura == 0 || largura == 0 {
        return None;
    }
    let declarada = retangulo(&meta["areaResultado"]);
    let canto = declarada.as_ref().map(|a| (a.r0, a.c0))
        .or_else(|| Some((meta["destino"]["row"].as_u64()? as u32, meta["destino"]["col"].as_u64()? as u32)))?;
    let aba = &wb.aba(destino)?.sheet;
    // O Excel já gravou o resultado: manter as células dele.
    let ja_desenhada = declarada.as_ref().is_some_and(|a| (a.r0..=a.r1)
        .any(|r| (a.c0..=a.c1).any(|c| !aba.get_raw(r, c).is_empty())));

    let mut formatos = Vec::new();
    if cols.is_empty() {
        // Sem campo em Colunas, a medida k é a coluna `rótulos + k`.
        let rotulos = resultado.colunas_de_rotulo;
        for (k, medida) in meta["valores"].as_array()?.iter().enumerate() {
            let c = rotulos + k;
            if let (Some(nome), Some(cab)) = (medida["nome"].as_str(), celulas[0].get_mut(c)) {
                *cab = Value::Text(nome.to_string());
            }
            if let Some(f) = extras.valores.get(k).and_then(|m| m.formato_sugerido()) {
                formatos.extend((1..altura).map(|r| (r as u32, c as u32, f)));
            }
        }
    }
    let mut nova = meta.clone();
    let area = if ja_desenhada {
        declarada.clone()?
    } else {
        Rect::new(canto.0, canto.1, canto.0 + altura as u32 - 1, canto.1 + largura as u32 - 1)
    };
    nova["areaResultado"] = json!({ "r0": area.r0, "c0": area.c0, "r1": area.r1, "c1": area.c1 });
    // O mapa só vale se as linhas desenhadas são as do motor.
    if !ja_desenhada || area.r1 - area.r0 + 1 == altura as u32 {
        nova["mapa"] = mapa(&resultado.linhas, resultado.colunas_de_rotulo);
    }
    if ja_desenhada {
        celulas.clear();
        formatos.clear();
    }
    Some(Gravacao { destino, indice, canto, celulas, formatos, meta: nova })
}

/// Completa e, quando faltar o resultado, calcula as dinâmicas de todas as abas.
pub(super) fn materializar(wb: &mut Workbook) {
    let mut gravacoes = Vec::new();
    let mut listas: Vec<(usize, Vec<Json>)> = Vec::new();
    for destino in 0..wb.len() {
        let Some(raw) = wb.aba(destino).and_then(|a| a.sheet.dn.as_deref()) else { continue };
        let Ok(lista) = serde_json::from_str::<Vec<Json>>(raw) else { continue };
        for (indice, meta) in lista.iter().enumerate() {
            if let Some(g) = calcular(wb, destino, indice, meta) {
                gravacoes.push(g);
            }
        }
        listas.push((destino, lista));
    }
    for g in gravacoes {
        let Some(viva) = wb.aba_mut(g.destino) else { continue };
        let folha = &mut viva.sheet;
        for (i, linha) in g.celulas.iter().enumerate() {
            for (j, v) in linha.iter().enumerate() {
                if !matches!(v, Value::Empty) {
                    folha.set_cell_valor(g.canto.0 + i as u32, g.canto.1 + j as u32, v.clone());
                }
            }
        }
        for (r, c, f) in g.formatos {
            let mut estilo = EstiloParcial::new();
            estilo.insert("format".into(), Campo::Texto(f.into()));
            if let Some(idx) = folha.registrar_parcial(estilo) {
                folha.set_style_idx(g.canto.0 + r, g.canto.1 + c, idx);
            }
        }
        if let Some((_, lista)) = listas.iter_mut().find(|(d, _)| *d == g.destino) {
            lista[g.indice] = g.meta;
        }
    }
    for (destino, lista) in listas {
        if let (Some(viva), Ok(texto)) = (wb.aba_mut(destino), serde_json::to_string(&lista)) {
            viva.sheet.dn = Some(texto);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::super::ler_xlsx_workbook;
    use feathersheet_core::celulas::values::Value;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// O que o Google Planilhas exporta: definição da dinâmica sem
    /// `pivotCacheRecords`, `refreshOnLoad="1"`, relação quebrada no cache,
    /// ids de cache a partir de 0 e a área da `location` vazia na aba.
    fn xlsx(celulas_resumo: &str) -> Vec<u8> {
        let mut dados = String::from(r#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Eixo</t></is></c><c r="B1" t="inlineStr"><is><t>Tipo</t></is></c><c r="C1" t="inlineStr"><is><t>Valor Total</t></is></c></row>"#);
        let linhas = [("e1", "Serviço", 100.0), ("e2", "Produto", 30.0), ("e3", "Serviço", 50.0), ("e4", "Alimentação", 20.0)];
        for (i, (e, t, v)) in linhas.iter().enumerate() {
            let r = i + 2;
            dados.push_str(&format!(r#"<row r="{r}"><c r="A{r}" t="inlineStr"><is><t>{e}</t></is></c><c r="B{r}" t="inlineStr"><is><t>{t}</t></is></c><c r="C{r}"><v>{v}</v></c></row>"#));
        }
        dados.push_str("</sheetData></worksheet>");
        let resumo = format!(r#"<worksheet><sheetData>{celulas_resumo}<row r="3"><c r="D3"><f>ROUNDUP(B3/B$6,2)</f><v>0.15</v></c></row></sheetData></worksheet>"#);
        let partes: Vec<(&str, String)> = vec![
            ("xl/workbook.xml", r#"<workbook xmlns:r="r"><sheets><sheet name="Resumo" r:id="rId4"/><sheet name="Mãos com o Futuro" r:id="rId5"/></sheets><pivotCaches><pivotCache cacheId="0" r:id="rId9"/></pivotCaches></workbook>"#.into()),
            ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId4" Target="worksheets/sheet1.xml"/><Relationship Id="rId5" Target="worksheets/sheet2.xml"/><Relationship Id="rId9" Target="pivotCache/pivotCacheDefinition1.xml"/></Relationships>"#.into()),
            ("xl/worksheets/sheet1.xml", resumo),
            ("xl/worksheets/_rels/sheet1.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="../pivotTables/pivotTable1.xml"/></Relationships>"#.into()),
            ("xl/worksheets/sheet2.xml", dados),
            ("xl/pivotCache/pivotCacheDefinition1.xml", r#"<pivotCacheDefinition invalid="1" refreshOnLoad="1"><cacheSource type="worksheet"><worksheetSource ref="A1:C5" sheet="Mãos com o Futuro"/></cacheSource><cacheFields><cacheField name="Eixo"/><cacheField name="Tipo"><sharedItems><s v="Serviço"/><s v="Produto"/><s v="Alimentação"/></sharedItems></cacheField><cacheField name="Valor Total"/></cacheFields></pivotCacheDefinition>"#.into()),
            ("xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels", r#"<Relationships><Relationship Type="externalLinkPath" TargetMode="External"/></Relationships>"#.into()),
            ("xl/pivotTables/pivotTable1.xml", r#"<pivotTableDefinition name="Resumo" cacheId="0" dataCaption="" compact="0" compactData="0"><location ref="A2:B6" firstHeaderRow="0" firstDataRow="1" firstDataCol="0"/><pivotFields><pivotField name="Eixo" compact="0" outline="0" showAll="0"/><pivotField name="Tipo" axis="axisRow" compact="0" outline="0" showAll="0" sortType="ascending"><items><item x="2"/><item x="1"/><item x="0"/><item t="default"/></items></pivotField><pivotField name="Valor Total" dataField="1" compact="0" outline="0" showAll="0"/></pivotFields><rowFields><field x="1"/></rowFields><dataFields><dataField name="Valor" fld="2" baseField="0"/></dataFields></pivotTableDefinition>"#.into()),
        ];
        let mut saida = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut saida);
            for (caminho, texto) in partes {
                zip.start_file(caminho, SimpleFileOptions::default()).unwrap();
                zip.write_all(texto.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        saida.into_inner()
    }

    fn definicao(wb: &feathersheet_core::planilha::workbook::Workbook) -> serde_json::Value {
        let dn = wb.aba(0).unwrap().sheet.dn.as_deref().expect("a dinâmica foi lida");
        serde_json::from_str::<Vec<serde_json::Value>>(dn).unwrap().remove(0)
    }

    #[test]
    fn dinamica_do_google_sem_resultado_e_calculada_ao_abrir() {
        let (wb, _) = ler_xlsx_workbook(xlsx("")).unwrap();
        let v = |r, c| wb.valor(0, r, c);
        // A2:B6 = cabeçalho, 3 itens em ordem crescente, total geral.
        assert_eq!(v(1, 1), Value::Text("Valor".into()));
        assert_eq!((v(2, 0), v(2, 1)), (Value::Text("Alimentação".into()), Value::Number(20.0)));
        assert_eq!((v(3, 0), v(3, 1)), (Value::Text("Produto".into()), Value::Number(30.0)));
        assert_eq!((v(4, 0), v(4, 1)), (Value::Text("Serviço".into()), Value::Number(150.0)));
        assert_eq!(v(5, 1), Value::Number(200.0));
        let d = definicao(&wb);
        assert_eq!(d["areaResultado"], serde_json::json!({"r0": 1, "c0": 0, "r1": 5, "c1": 1}));
        assert_eq!(d["fonte"]["aba"], "Mãos com o Futuro");
        assert_eq!(d["linhas"], serde_json::json!([1]));
        assert_eq!(d["valores"][0]["col"], 2);
        assert_eq!(d["valores"][0]["como"], "soma");
        assert_eq!(d["layout"], "tabular");
        assert_eq!(d["extras"]["linhas"][0]["ordenacao"]["tipo"], "crescente");
        assert_eq!(d["mapa"]["linhas"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn resultado_gravado_pelo_excel_fica_como_veio() {
        let celulas = r#"<row r="2"><c r="A2" t="inlineStr"><is><t>Tipo</t></is></c><c r="B2" t="inlineStr"><is><t>Valor</t></is></c></row><row r="4"><c r="B4"><v>999</v></c></row>"#;
        let (wb, _) = ler_xlsx_workbook(xlsx(celulas)).unwrap();
        assert_eq!(wb.valor(0, 3, 1), Value::Number(999.0));
        assert_eq!(wb.valor(0, 2, 0), Value::Empty);
        let d = definicao(&wb);
        assert_eq!(d["areaResultado"], serde_json::json!({"r0": 1, "c0": 0, "r1": 5, "c1": 1}));
    }
}
