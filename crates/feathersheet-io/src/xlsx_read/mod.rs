//! Leitor nativo de `.xlsx`. Espelha `importXlsx` de `src/io/xlsx.ts:793-817`
//! — mesma abrangência (fórmula, estilo, formato numérico, string
//! compartilhada, fórmula compartilhada, nomes definidos, largura/altura).
//! O que já não atravessa em JS (gráfico, formatação condicional,
//! validação, célula mesclada, tabela) continua de fora aqui também.
//!
//! Diferente do `.fshz` (Parte 1), aqui a tradução inteira mora em Rust —
//! pedido explícito do usuário, aceitando o custo de manter `pt_name.rs`
//! sincronizado com `core/functions.ts::CANONICAL` (ver o aviso no topo
//! daquele arquivo).

mod format;
mod formas;
mod imagens_celula;
mod desenho_objetos;
mod graficos_ancora;
pub use formas::{ler_formas, Forma};
mod native;
mod dinamica_importada;
mod formula;
mod pt_name;
mod autofiltro;
mod condicional;
mod cores;
mod comentarios;
mod refs;
mod sheet;
mod styles;
mod validacoes;
mod vinculos;
mod vistas;
pub use vistas::{ler_vistas_arquivo, ler_vistas_bytes, VistaAba};
mod xml;

pub use native::{ler_xlsx_arquivo, ler_xlsx_workbook};

use serde::Serialize;
use std::io::{Cursor, Read};
use zip::ZipArchive;

const LIMITE_XLSX_BYTES: usize = 256 * 1024 * 1024;
// Protege XLSX pequeno no disco mas enorme depois de descomprimir XML.
const LIMITE_ENTRADA_XML: u64 = 32 * 1024 * 1024;

/// Roda `f` em cada item com até `threads` threads, cada uma com o seu
/// leitor do ZIP (o clone só copia o ponteiro dos bytes); devolve na ordem
/// dos itens. Descompactar e ler o XML é serial dentro de cada parte: as
/// abas e os gráficos em fila eram 2,3 s dos 2,4 s de um XLSX com 22 abas.
fn em_paralelo<T: Send, R: Send>(
    archive: &ZipArchive<Cursor<std::sync::Arc<[u8]>>>,
    threads: usize,
    itens: Vec<T>,
    f: impl Fn(&mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, T) -> R + Sync,
) -> Vec<R> {
    let n = itens.len();
    let fila = std::sync::Mutex::new(itens.into_iter().enumerate());
    let prontos = std::sync::Mutex::new(Vec::with_capacity(n));
    std::thread::scope(|escopo| {
        for _ in 0..threads.min(n) {
            escopo.spawn(|| {
                let mut zip = archive.clone();
                loop {
                    let Some((i, item)) = fila.lock().unwrap().next() else { break };
                    let r = f(&mut zip, item);
                    prontos.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut prontos = prontos.into_inner().unwrap();
    prontos.sort_by_key(|p| p.0);
    prontos.into_iter().map(|p| p.1).collect()
}

fn entry_text(archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, path: &str) -> Result<String, String> {
    entry_text_com_limite(archive, path, LIMITE_ENTRADA_XML)
}

fn entry_text_com_limite(
    archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>,
    path: &str,
    limite: u64,
) -> Result<String, String> {
    match archive.by_name(path) {
        Ok(mut file) => {
            if file.size() > limite {
                return Err(format!(
                    "XLSX: '{}' excede {} MiB de XML descomprimido.                      Remova formatacoes de areas vazias ou divida a planilha.",
                    path, limite / 1024 / 1024,
                ));
            }
            let mut buf = String::new();
            file.by_ref().take(limite + 1)
                .read_to_string(&mut buf)
                .map_err(|e| format!("XLSX: falha ao ler '{path}': {e}"))?;
            if buf.len() as u64 > limite {
                return Err(format!("XLSX: '{}' excede o limite de XML descomprimido.", path));
            }
            Ok(buf)
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(String::new()),
        Err(e) => Err(format!("XLSX: erro ao acessar '{path}': {e}")),
    }
}

#[derive(Serialize)]
struct WorkbookJson {
    version: u32,
    active: u32,
    names: Vec<NameJson>,
    sheets: Vec<sheet::SheetJson>,
}

#[derive(Serialize)]
struct NameJson {
    name: String,
    rect: RectJson,
}

#[derive(Serialize)]
struct RectJson {
    r0: i64,
    c0: i64,
    r1: i64,
    c1: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sheet: Option<String>,
}

/// Lê um `.xlsx` inteiro (os bytes do arquivo) e devolve o mesmo formato
/// que o `Workbook.loadJSON` entende, que a ponte monta como pasta
/// (`PastaAberta::de_json`). Morava no `src-tauri`; veio para cá para o
/// `.wasm` (e o navegador) abrir `.xlsx` sem o leitor em TypeScript.
pub fn ler_xlsx_bytes(bytes: Vec<u8>) -> Result<serde_json::Value, String> {
    if bytes.len() > LIMITE_XLSX_BYTES {
        return Err(format!("Arquivo XLSX excede o limite de {} MB", LIMITE_XLSX_BYTES / (1024 * 1024)));
    }
    let mut archive = ZipArchive::new(Cursor::new(std::sync::Arc::<[u8]>::from(bytes))).map_err(|e| format!("Não parece um arquivo ZIP: {e}"))?;

    let workbook_xml = entry_text(&mut archive, "xl/workbook.xml")?;
    let book = xml::parse_xml(&workbook_xml).map_err(|e| e.to_string())?;
    if book.name != "workbook" {
        return Err("Não parece uma planilha do Excel: falta o xl/workbook.xml".to_string());
    }

    let rels = sheet::read_rels(&entry_text(&mut archive, "xl/_rels/workbook.xml.rels")?);
    let strings = sheet::read_shared_strings(&entry_text(&mut archive, "xl/sharedStrings.xml")?);
    let estilos_xml = entry_text(&mut archive, "xl/styles.xml")?;
    let _paleta = cores::instalar(&entry_text(&mut archive, "xl/theme/theme1.xml").unwrap_or_default(), &estilos_xml);
    let xfs = styles::read_styles(&estilos_xml);

    let mut sheets_xml_por_indice: Vec<(String, String, bool)> = Vec::new();
    if let Some(sheets_node) = book.child("sheets") {
        for (i, node) in sheets_node.children_named("sheet").enumerate() {
            let target = node
                .attr("r:id")
                .and_then(|id| rels.get(id))
                .cloned()
                .unwrap_or_else(|| format!("worksheets/sheet{}.xml", i + 1));
            let path = if let Some(stripped) = target.strip_prefix('/') {
                stripped.to_string()
            } else {
                format!("xl/{}", target.strip_prefix("./").unwrap_or(&target))
            };
            let name = node.attr("name").filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| format!("Planilha{}", i + 1));
            let is_hidden = node.attr("state").map_or(false, |s| s == "hidden" || s == "veryHidden");
            sheets_xml_por_indice.push((name, path, is_hidden));
        }
    }

    let mut sheets: Vec<sheet::SheetJson> = sheets_xml_por_indice
        .iter()
        .map(|(name, path, is_hidden)| -> Result<sheet::SheetJson, String> {
            let xml = entry_text(&mut archive, path)?;
            let (pasta, arquivo) = path.rsplit_once('/').unwrap_or(("", path));
            let rels_da_aba = sheet::read_rels(&entry_text(
                &mut archive, &format!("{pasta}/_rels/{arquivo}.rels"),
            )?);
            let mut sheet_obj =
                sheet::read_sheet_com_links(name, &xml, &strings, &xfs, &rels_da_aba)?;
            sheet_obj.oculta = *is_hidden;
            Ok(sheet_obj)
        })
        .collect::<Result<Vec<_>, _>>()?;

    if sheets.is_empty() {
        sheets.push(sheet::SheetJson { name: "Planilha1".to_string(), ..Default::default() });
    }

    anexar_elementos(&mut archive, &sheets_xml_por_indice, &mut sheets)?;

    let names = sheet::read_names(&book)
        .into_iter()
        .map(|n| NameJson { name: n.name, rect: RectJson { r0: n.r0, c0: n.c0, r1: n.r1, c1: n.c1, sheet: n.sheet } })
        .collect();

    let out = WorkbookJson { version: 2, active: 0, names, sheets };
    serde_json::to_value(out).map_err(|e| format!("Não consegui montar o JSON: {e}"))
}

/// Metadados não celulares do Excel, compartilhados pelos caminhos JSON e nativo.
/// O caminho nativo mantém apenas o resumo de cada aba aqui, sem duplicar células.
fn anexar_elementos(
    archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>,
    sheets_xml_por_indice: &[(String, String, bool)],
    sheets: &mut Vec<sheet::SheetJson>,
) -> Result<(), String> {
    let zip_names: Vec<String> = (0..archive.len()).filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string())).collect();

    // Estilos diferenciais (`dxf`) que os filtros por cor apontam.
    let xfs_dxf = entry_text(archive, "xl/styles.xml").map(|t| styles::read_styles(&t)).unwrap_or_default();

    // Ler tabelas do Excel por aba
    for (i, sheet) in sheets.iter_mut().enumerate() {
        if i >= sheets_xml_por_indice.len() { break; }
        let path = &sheets_xml_por_indice[i].1;
        let (pasta, arquivo) = path.rsplit_once('/').unwrap_or(("", path));
        let rels_da_aba = sheet::read_rels(&entry_text(archive, &format!("{pasta}/_rels/{arquivo}.rels"))?);

        let mut table_targets: Vec<String> = Vec::new();
        for target in rels_da_aba.values() {
            if target.contains("table") && target.ends_with(".xml") {
                let clean = target.strip_prefix("../").unwrap_or(target);
                let full_path = if clean.starts_with("xl/") { clean.to_string() } else { format!("xl/{clean}") };
                table_targets.push(full_path);
            }
        }
        if table_targets.is_empty() && rels_da_aba.is_empty() && i == 0 {
            table_targets = zip_names.iter().filter(|n| n.to_lowercase().contains("tables/table") && n.ends_with(".xml")).cloned().collect();
        }

        for tpath in &table_targets {
            let xml_str = entry_text(archive, tpath)?;
            if let Ok(root) = xml::parse_xml(&xml_str) {
                let node_opt = if root.name == "table" { Some(&root) } else { root.child("table") };
                if let Some(node) = node_opt {
                    if let Some(ref_str_raw) = node.attr("ref") {
                        let t_name = node.attr("name").or_else(|| node.attr("displayName")).unwrap_or("Tabela1");
                        let ref_str = ref_str_raw.replace('$', "");
                        let mut partes = ref_str.split(':');
                        if let (Some(start), Some(end)) = (partes.next().and_then(refs::parse_cell_ref), partes.next().and_then(refs::parse_cell_ref)) {
                            let has_header = node.attr("headerRowCount").unwrap_or("1") != "0";
                            let style_info = node.child("tableStyleInfo");
                            let style_name = style_info.and_then(|s| s.attr("name")).unwrap_or("").to_lowercase();
                            let nome_original = style_info.and_then(|s| s.attr("name")).unwrap_or("");
                            let tema = if let Some(t) = cores::tema_de_tabela(nome_original) {
                                t
                            } else if style_name.contains("green") || style_name.contains("verde") {
                                "verde"
                            } else if style_name.contains("orange") || style_name.contains("laranja") {
                                "laranja"
                            } else if style_name.contains("red") || style_name.contains("vermelho") {
                                "vermelho"
                            } else if style_name.contains("purple") || style_name.contains("roxo") {
                                "roxo"
                            } else if style_name.contains("gray") || style_name.contains("cinza") {
                                "cinza"
                            } else {
                                "azul"
                            };
                            let faixas = style_info.and_then(|s| s.attr("showRowStripes")).map_or(true, |v| v != "0");
                            let has_totals = node.attr("totalsRowCount").map_or(false, |v| v != "0");
                            let r1 = if has_totals && end.row > start.row { end.row - 1 } else { end.row };

                            let (filtros, ordenacao) = match node.child("autoFilter") {
                                Some(auto) => autofiltro::ler(auto, &xfs_dxf, start.row as u32, start.col as u32),
                                None => (Vec::new(), None),
                            };
                            let mut tabela = serde_json::json!({
                                "id": format!("tb_{}", rand_id()),
                                "nome": t_name,
                                "area": { "r0": start.row, "c0": start.col, "r1": r1, "c1": end.col },
                                "temCabecalho": has_header,
                                "faixas": faixas,
                                "estilo": { "tema": tema },
                                "mostrarTotais": has_totals,
                                "filtros": filtros
                            });
                            if let Some(o) = ordenacao {
                                tabela["ordenacao"] = o;
                            }
                            sheet.tabelas.push(tabela);
                        }
                    }
                }
            }
        }
    }

    // Ler pivotCaches do Excel
    struct CacheInfo {
        aba: Option<String>,
        area: (i64, i64, i64, i64),
        /// `worksheetSource name="..."`: a origem é uma tabela (ou nome)
        /// em vez de `sheet` + `ref`; resolvida pelas tabelas já lidas.
        tabela: Option<String>,
        _fields: Vec<String>,
        /// O agrupamento de cada campo (`fieldGroup`), já no formato `Agrupamento` do core.
        grupos: Vec<Option<serde_json::Value>>,
        /// Os itens (`sharedItems`) de cada campo, como chave do filtro do
        /// painel: resolvem itens ocultos, recolhidos e o item base.
        itens: Vec<Vec<String>>,
    }
    let mut pivot_caches: std::collections::HashMap<String, CacheInfo> = std::collections::HashMap::new();
    // O `cacheId` da dinâmica é o do `<pivotCache cacheId r:id>` do
    // workbook.xml, não o número do arquivo. O Google Planilhas, por
    // exemplo, numera os arquivos a partir de 1 e os ids a partir de 0.
    let caminho_do_cache = caminhos_dos_caches(archive);
    for fname in &zip_names {
        if fname.to_lowercase().contains("pivotcache") && fname.ends_with(".xml") {
            let xml_str = entry_text(archive, fname)?;
            if let Ok(root) = xml::parse_xml(&xml_str) {
                let cache_node = if root.name == "pivotCacheDefinition" { Some(&root) } else { root.child("pivotCacheDefinition") };
                if let Some(c_node) = cache_node {
                    let mut cache_sheet = None;
                    let mut cache_area = (0, 0, 9, 1);
                    let mut cache_tabela = None;
                    if let Some(ws_source) = c_node.child("cacheSource").and_then(|cs| cs.child("worksheetSource")) {
                        cache_tabela = ws_source.attr("name").filter(|n| !n.is_empty()).map(str::to_string);
                        if let Some(s) = ws_source.attr("sheet") {
                            cache_sheet = Some(s.to_string());
                        }
                        if let Some(ref_str) = ws_source.attr("ref") {
                            let clean = ref_str.replace('$', "");
                            let mut p = clean.split(':');
                            if let (Some(start), Some(end)) = (p.next().and_then(refs::parse_cell_ref), p.next().and_then(refs::parse_cell_ref)) {
                                cache_area = (start.row, start.col, end.row, end.col);
                            }
                        }
                    }
                    let mut fields = Vec::new();
                    let mut grupos: Vec<Option<serde_json::Value>> = Vec::new();
                    let mut itens: Vec<Vec<String>> = Vec::new();
                    if let Some(cf_node) = c_node.child("cacheFields") {
                        for cf in cf_node.children_named("cacheField") {
                            let fname = cf.attr("name").map(str::to_string).unwrap_or_else(|| format!("Campo{}", fields.len() + 1));
                            fields.push(fname);
                            grupos.push(agrupamento_do_campo(cf));
                            itens.push(chaves_dos_itens(cf));
                        }
                    }
                    let num_str = fname.find("pivotCacheDefinition").and_then(|pos| {
                        let rest = &fname[pos + "pivotCacheDefinition".len()..];
                        rest.trim_end_matches(".xml").parse::<usize>().ok()
                    }).unwrap_or(1);
                    let info = || CacheInfo {
                        aba: cache_sheet.clone(), area: cache_area, tabela: cache_tabela.clone(),
                        _fields: fields.clone(), grupos: grupos.clone(), itens: itens.clone(),
                    };
                    let ids: Vec<String> = caminho_do_cache.iter()
                        .filter(|(_, caminho)| caminho.as_str() == fname.as_str())
                        .map(|(id, _)| id.clone()).collect();
                    if ids.is_empty() {
                        // Sem `<pivotCaches>` no workbook: o palpite antigo pelo número.
                        pivot_caches.entry(format!("{}", num_str.saturating_sub(1))).or_insert_with(info);
                        pivot_caches.entry(format!("{num_str}")).or_insert_with(info);
                    } else {
                        for id in ids { pivot_caches.insert(id, info()); }
                    }
                }
            }
        }
    }

    // Origem por nome de tabela: (nome, aba, área com cabeçalho e sem a linha de totais).
    type TabelaNomeada = (String, String, (i64, i64, i64, i64));
    let tabelas_por_nome: Vec<TabelaNomeada> = sheets.iter()
        .flat_map(|s| s.tabelas.iter().filter_map(move |t| {
            let a = &t["area"];
            Some((t["nome"].as_str()?.to_lowercase(), s.name.clone(),
                (a["r0"].as_i64()?, a["c0"].as_i64()?, a["r1"].as_i64()?, a["c1"].as_i64()?)))
        }))
        .collect();

    // Ler tabelas dinâmicas do Excel por aba
    for (i, sheet) in sheets.iter_mut().enumerate() {
        if i >= sheets_xml_por_indice.len() { break; }
        let path = &sheets_xml_por_indice[i].1;
        let (pasta, arquivo) = path.rsplit_once('/').unwrap_or(("", path));
        let rels_da_aba = sheet::read_rels(&entry_text(archive, &format!("{pasta}/_rels/{arquivo}.rels"))?);

        let mut pivot_targets: Vec<String> = Vec::new();
        for target in rels_da_aba.values() {
            if target.contains("pivotTable") && target.ends_with(".xml") {
                let clean = target.strip_prefix("../").unwrap_or(target);
                let full_path = if clean.starts_with("xl/") { clean.to_string() } else { format!("xl/{clean}") };
                pivot_targets.push(full_path);
            }
        }

        for ppath in &pivot_targets {
            let xml_str = entry_text(archive, ppath)?;
            if let Ok(root) = xml::parse_xml(&xml_str) {
                let pvt_node = if root.name == "pivotTableDefinition" { Some(&root) } else { root.child("pivotTableDefinition") };
                if let Some(p_node) = pvt_node {
                    let cache_id = p_node.attr("cacheId").unwrap_or("0");
                    let default_info = CacheInfo { aba: None, area: (0, 0, 9, 1), tabela: None, _fields: Vec::new(), grupos: Vec::new(), itens: Vec::new() };
                    let cache = pivot_caches.get(cache_id).or_else(|| pivot_caches.get("0")).unwrap_or(&default_info);
                    let (fonte_aba, fonte_area) = match cache.tabela.as_deref()
                        .map(str::to_lowercase)
                        .and_then(|n| tabelas_por_nome.iter().find(|(nome, _, _)| *nome == n)) {
                        Some((_, aba, area)) => (Some(aba.clone()), *area),
                        None => (cache.aba.clone(), cache.area),
                    };

                    // `location ref` é a área inteira que o Excel desenhou
                    // (sem os filtros de relatório, que ficam acima dela).
                    let mut dest_row = 0;
                    let mut dest_col = 0;
                    let mut area_resultado: Option<(i64, i64, i64, i64)> = None;
                    if let Some(loc) = p_node.child("location") {
                        if let Some(r_ref) = loc.attr("ref") {
                            let clean = r_ref.replace('$', "");
                            let mut partes = clean.split(':');
                            let inicio = partes.next().and_then(refs::parse_cell_ref);
                            let fim = partes.next().and_then(refs::parse_cell_ref);
                            if let Some(c) = &inicio {
                                dest_row = c.row;
                                dest_col = c.col;
                            }
                            if let (Some(a), Some(b)) = (&inicio, fim.as_ref().or(inicio.as_ref())) {
                                area_resultado = Some((a.row.min(b.row), a.col.min(b.col), a.row.max(b.row), a.col.max(b.col)));
                            }
                        }
                    }

                    // Os `pivotField` ficam dentro de `<pivotFields>`, não
                    // direto na definição (sem isso a ordenação e os
                    // subtotais de cada campo se perdiam).
                    let p_fields: Vec<&xml::XmlNode> = p_node.child("pivotFields")
                        .map(|pf| pf.children_named("pivotField").collect())
                        .unwrap_or_default();
                    let base_col = fonte_area.1;
                    let mut linhas: Vec<u32> = Vec::new();
                    let mut colunas: Vec<u32> = Vec::new();
                    let mut valores: Vec<serde_json::Value> = Vec::new();

                    if let Some(rf_node) = p_node.child("rowFields") {
                        for f in rf_node.children_named("field") {
                            if let Some(x_str) = f.attr("x") {
                                if let Ok(idx) = x_str.parse::<u32>() {
                                    linhas.push(idx);
                                }
                            }
                        }
                    }
                    if linhas.is_empty() {
                        for (idx, pf) in p_fields.iter().enumerate() {
                            if pf.attr("axis") == Some("axisRow") {
                                linhas.push(idx as u32);
                            }
                        }
                    }

                    if let Some(cf_node) = p_node.child("colFields") {
                        for f in cf_node.children_named("field") {
                            if let Some(x_str) = f.attr("x") {
                                if let Ok(idx) = x_str.parse::<u32>() {
                                    colunas.push(idx);
                                }
                            }
                        }
                    }
                    if colunas.is_empty() {
                        for (idx, pf) in p_fields.iter().enumerate() {
                            if pf.attr("axis") == Some("axisCol") {
                                colunas.push(idx as u32);
                            }
                        }
                    }

                    if let Some(df_node) = p_node.child("dataFields") {
                        for df in df_node.children_named("dataField") {
                            if let Some(fld_str) = df.attr("fld") {
                                if let Ok(fld_idx) = fld_str.parse::<u32>() {
                                    // `subtotal` ausente = soma (padrão do Excel).
                                    let como = match df.attr("subtotal").unwrap_or("sum") {
                                        "count" | "countNums" => "contagem",
                                        "average" => "media",
                                        "max" => "maximo",
                                        "min" => "minimo",
                                        "stdDev" | "stdDevp" => "desviopadrao",
                                        _ => "soma",
                                    };
                                    let mut medida = serde_json::json!({ "col": base_col + fld_idx as i64, "como": como });
                                    if let Some(nome) = df.attr("name").filter(|n| !n.is_empty()) {
                                        medida["nome"] = serde_json::json!(nome);
                                    }
                                    valores.push(medida);
                                }
                            }
                        }
                    }

                    let extras = extras_da_dinamica(p_node, &p_fields, &linhas, &colunas, &cache.grupos, &cache.itens);
                    // Layout do Excel: `compact="0"` é o de estrutura de
                    // tópicos ou o tabular (uma coluna por campo), que aqui
                    // é o tabular; o padrão é o compacto.
                    let desligado = |no: &xml::XmlNode, nome: &str| matches!(no.attr(nome), Some("0") | Some("false"));
                    let tabular = desligado(p_node, "compact");
                    // Subtotal do grupo de fora: `defaultSubtotal` ligado
                    // (o padrão) em algum campo de linha que não o último.
                    let subtotais = linhas.len() > 1 && linhas[..linhas.len() - 1].iter()
                        .any(|&x| p_fields.get(x as usize).is_none_or(|f| !desligado(f, "defaultSubtotal")));
                    // "Repetir rótulos dos itens" (x14 `fillDownLabels`) só
                    // existe no tabular.
                    let repetir = tabular && linhas.iter()
                        .any(|&x| p_fields.get(x as usize).is_some_and(|f| rotulos_repetidos(f)));
                    // Filtros no formato do painel (`{col, valores, excluir}`,
                    // valores = chaves que saem): os campos de `pageFields`
                    // primeiro, na ordem deles, e os itens ocultos (`h="1"`)
                    // de qualquer outro campo.
                    let paginas: Vec<(usize, Option<usize>)> = p_node.child("pageFields")
                        .map(|pf| pf.children_named("pageField").filter_map(|f| Some((
                            f.attr("fld")?.parse::<usize>().ok()?,
                            f.attr("item").and_then(|v| v.parse::<usize>().ok()),
                        ))).collect())
                        .unwrap_or_default();
                    let mut filtros: Vec<serde_json::Value> = Vec::new();
                    let demais = (0..p_fields.len()).filter(|i| !paginas.iter().any(|(f, _)| f == i)).map(|i| (i, None));
                    for (idx, escolhido) in paginas.iter().copied().chain(demais) {
                        let pagina = paginas.iter().any(|(f, _)| *f == idx);
                        let saem = itens_ocultos(p_fields.get(idx).copied(), cache.itens.get(idx), escolhido);
                        if pagina || !saem.is_empty() {
                            filtros.push(serde_json::json!({ "col": base_col + idx as i64, "valores": saem, "excluir": true }));
                        }
                    }
                    // Grupos recolhidos (`sd="0"`) do campo de fora: o caminho
                    // do painel nesse nível é a chave do item.
                    let recolhidos: Vec<String> = match linhas.first() {
                        Some(&x) if linhas.len() > 1 => p_fields.get(x as usize)
                            .and_then(|f| f.child("items"))
                            .map(|itens| itens.children_named("item")
                                .filter(|it| matches!(it.attr("sd"), Some("0") | Some("false")))
                                .filter_map(|it| cache.itens.get(x as usize)?.get(it.attr("x")?.parse::<usize>().ok()?).cloned())
                                .collect())
                            .unwrap_or_default(),
                        _ => Vec::new(),
                    };
                    // No `.fshz` os campos são colunas ABSOLUTAS da aba de
                    // origem (o painel da dinâmica trabalha assim); no
                    // `.xlsx` são posições dentro da origem.
                    let absolutas = |lista: &[u32]| -> Vec<i64> { lista.iter().map(|&x| base_col + x as i64).collect() };
                    let mut dinamica = serde_json::json!({
                        "id": format!("din_{}_{}", i, sheet.dinamicas.len()),
                        "nome": p_node.attr("name"),
                        "fonte": {
                            "aba": fonte_aba,
                            "area": { "r0": fonte_area.0, "c0": fonte_area.1, "r1": fonte_area.2, "c1": fonte_area.3 }
                        },
                        "destino": { "row": dest_row, "col": dest_col },
                        "linhas": absolutas(&linhas),
                        "colunas": absolutas(&colunas),
                        "valores": valores,
                        "datas": [],
                        "extras": extras,
                        "layout": if tabular { "tabular" } else { "compacto" },
                        "repetirRotulos": repetir,
                        "subtotais": subtotais,
                        "tema": "padrao",
                        "recolhidos": recolhidos,
                        "filtros": filtros,
                        "atualizarAoAbrir": matches!(p_node.attr("refreshOnLoad"), Some("1") | Some("true")),
                    });
                    if let Some((r0, c0, r1, c1)) = area_resultado {
                        dinamica["areaResultado"] = serde_json::json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 });
                    }
                    sheet.dinamicas.push(dinamica);
                }
            }
        }
    }

    // Ler gráficos do Excel. A aba e o retângulo vêm do desenho que aponta
    // para o gráfico (`graficos_ancora`); o intervalo dos dados pode morar em
    // outra aba.
    let lugares = graficos_ancora::lugares(archive, sheets_xml_por_indice);
    let nomes_ocultos: std::collections::HashMap<String, graficos_ancora::AreaNomeada> =
        entry_text(archive, "xl/workbook.xml").ok().and_then(|t| xml::parse_xml(&t).ok())
            .map(|b| sheet::read_names(&b).into_iter().map(|n| (n.name, (n.sheet, (n.r0, n.c0, n.r1, n.c1)))).collect())
            .unwrap_or_default();
    // O XML dos gráficos é lido em paralelo e sem os pontos guardados
    // (`numCache/pt`): ninguém os lê, e eram 99% dos nós (585 KB de XML
    // viravam 14 MB de árvore por gráfico).
    // shortcut: o `strCache/pt` (categoria em texto) ainda entra na árvore; pular também se pesar.
    let lidos = em_paralelo(archive, feathersheet_core::paralelismo::nucleos(), zip_names.iter().collect(), |zip, fname| {
        let grafico = fname.to_lowercase().contains("charts/chart") && fname.ends_with(".xml");
        (fname, grafico.then(|| entry_text(zip, fname)
            .map(|t| xml::parse_xml_items_reader_result(t.as_bytes(), "numCache", "pt", &mut |_| Ok(())))))
    });
    for (fname, lido) in lidos {
        if let Some(lido) = lido {
            let lugar = lugares.get(fname.as_str());
            if let Ok(root) = lido? {
                if let Some(ex) = graficos_ancora::ler_chartex(&root, &nomes_ocultos) {
                    let destino = match lugar {
                        Some(l) => sheets.get_mut(l.aba),
                        None => ex.aba.as_ref().and_then(|n| sheets.iter_mut().find(|s| s.name.eq_ignore_ascii_case(n))),
                    };
                    if let Some(sheet) = destino {
                        let (r0, c0, r1, c1) = ex.area;
                        let posicao = match lugar {
                            Some(l) => graficos_ancora::posicao(l, sheet),
                            None => serde_json::json!({ "x": 50, "y": 50, "width": 480, "height": 320 }),
                        };
                        let mut g = serde_json::json!({
                            "id": format!("chart_{}", rand_id()),
                            "type": ex.tipo,
                            "title": if ex.titulo.is_empty() { "Gráfico".to_string() } else { ex.titulo.clone() },
                            "range": { "r0": r0, "c0": c0, "r1": r1, "c1": c1 },
                            "sourceSheet": ex.aba,
                            "seriesInColumns": true,
                            "hasHeaders": false,
                            "hasCategoryLabels": c1 > c0,
                            "position": posicao,
                        });
                        if !ex.ocultas.is_empty() {
                            g["style"]["hiddenSeries"] = serde_json::json!(ex.ocultas);
                        }
                        sheet.charts.push(g);
                    }
                    continue;
                }
                if let Some(chart_node) = root.child("chart") {
                    if let Some(plot_area) = chart_node.child("plotArea") {
                        let mut chart_type = "column";
                        let mut type_node = plot_area.child("barChart").or_else(|| plot_area.child("bar3DChart"));
                        if let Some(n) = type_node {
                            let dir = n.child("barDir").and_then(|d| d.attr("val")).unwrap_or("col");
                            chart_type = if dir == "bar" { "bar" } else { "column" };
                        } else if let Some(n) = plot_area.child("lineChart").or_else(|| plot_area.child("line3DChart")) {
                            type_node = Some(n);
                            chart_type = "line";
                        } else if let Some(n) = plot_area.child("pieChart").or_else(|| plot_area.child("pie3DChart")) {
                            type_node = Some(n);
                            chart_type = "pie";
                        } else if let Some(n) = plot_area.child("areaChart").or_else(|| plot_area.child("area3DChart")) {
                            type_node = Some(n);
                            chart_type = "area";
                        } else if let Some(n) = plot_area.child("doughnutChart") {
                            type_node = Some(n);
                            chart_type = "doughnut";
                        } else if let Some(n) = plot_area.child("scatterChart") {
                            type_node = Some(n);
                            chart_type = "scatter";
                        } else if let Some(n) = plot_area.child("radarChart") {
                            type_node = Some(n);
                            chart_type = "radar";
                        }

                        let title_text = chart_node.child("title").map(extract_text_node).filter(|s| !s.is_empty()).unwrap_or_else(|| "Gráfico".to_string());

                        let mut found_range = false;
                        let mut min_r: i64 = i64::MAX;
                        let mut min_c: i64 = i64::MAX;
                        let mut max_r: i64 = 0;
                        let mut max_c: i64 = 0;
                        let mut target_sheet_name: Option<String> = None;
                        let mut has_headers = false;
                        let mut has_category_labels = false;
                        let mut in_rows_count = 0;
                        let mut in_cols_count = 0;

                        let mut ser_idx_count = 0;
                        let mut cat_ref = None;
                        let mut val_refs = Vec::new();
                        let mut series_point_shapes = std::collections::BTreeMap::<u32, String>::new();
                        let mut series_point_sizes = std::collections::BTreeMap::<u32, f64>::new();

                        if let Some(tn) = type_node {
                            for ser in tn.children_named("ser") {
                                let ser_idx = ser_idx_count;
                                ser_idx_count += 1;

                                if let Some(marker) = ser.child("marker") {
                                    if let Some(sym) = marker.child("symbol").and_then(|s| s.attr("val")) {
                                        let shape = match sym.to_lowercase().as_str() {
                                            "circle" => "circle",
                                            "square" => "square",
                                            "triangle" => "triangle",
                                            "diamond" => "diamond",
                                            "star" => "star",
                                            "x" => "x",
                                            "none" => "none",
                                            _ => "circle",
                                        };
                                        series_point_shapes.insert(ser_idx, shape.to_string());
                                    }
                                    if let Some(sz) = marker.child("size").and_then(|s| s.attr("val")).and_then(|v| v.parse::<f64>().ok()) {
                                        if sz > 0.0 {
                                            series_point_sizes.insert(ser_idx, sz);
                                        }
                                    }
                                }

                                for section in ["tx", "cat", "val"] {
                                    if let Some(sec_node) = ser.child(section) {
                                        if section == "tx" { has_headers = true; }
                                        if section == "cat" { has_category_labels = true; }
                                        let ref_node = sec_node.child("numRef").or_else(|| sec_node.child("strRef"));
                                        if let Some(f_node) = ref_node.and_then(|r| r.child("f")) {
                                            let f_val = f_node.text.as_str();
                                            if !f_val.is_empty() {
                                                let clean = f_val.replace('$', "");
                                                let parts: Vec<&str> = clean.split('!').collect();
                                                if parts.len() > 1 {
                                                    let s_name = parts[0].trim_matches('\'');
                                                    if !s_name.is_empty() {
                                                        target_sheet_name = Some(s_name.to_string());
                                                    }
                                                }
                                                let ref_part = parts.last().unwrap_or(&"");
                                                let mut p = ref_part.split(':');
                                                if let Some(start) = p.next().and_then(refs::parse_cell_ref) {
                                                    let end = p.next().and_then(refs::parse_cell_ref).unwrap_or(start);
                                                    found_range = true;
                                                    min_r = min_r.min(start.row);
                                                    max_r = max_r.max(end.row);
                                                    min_c = min_c.min(start.col);
                                                    max_c = max_c.max(end.col);

                                                    if section == "cat" { cat_ref = Some((start, end)); }
                                                    if section == "val" {
                                                        val_refs.push((start, end));
                                                        if start.row == end.row && start.col != end.col {
                                                            in_rows_count += 1;
                                                        } else if start.col == end.col && start.row != end.row {
                                                            in_cols_count += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let target_sheet = if let Some(l) = lugar {
                            sheets.get_mut(l.aba)
                        } else if let Some(ref sheet_name) = target_sheet_name {
                            sheets.iter_mut().find(|s| s.name.eq_ignore_ascii_case(sheet_name))
                        } else {
                            sheets.get_mut(0)
                        };

                        if let Some(sheet) = target_sheet {
                            let idx = sheet.charts.len();
                            let posicao = match lugar {
                                Some(l) => graficos_ancora::posicao(l, sheet),
                                None => serde_json::json!({ "x": 50 + idx * 40, "y": 50 + idx * 40, "width": 480, "height": 320 }),
                            };
                            let range_json = if found_range {
                                serde_json::json!({ "r0": min_r, "c0": min_c, "r1": max_r, "c1": max_c })
                            } else {
                                serde_json::json!({ "r0": 0, "c0": 0, "r1": 9, "c1": 1 })
                            };
                            let series_in_columns = in_rows_count <= in_cols_count;
                            let saltos = if found_range {
                                graficos_ancora::series_salteadas(series_in_columns, (min_r, min_c, max_r, max_c), cat_ref, &val_refs)
                            } else {
                                None
                            };

                            let mut chart_json = serde_json::json!({
                                "id": format!("chart_{}", rand_id()),
                                "type": chart_type,
                                "title": title_text,
                                "range": range_json,
                                "sourceSheet": target_sheet_name,
                                "seriesInColumns": series_in_columns,
                                "hasHeaders": has_headers,
                                "hasCategoryLabels": has_category_labels,
                                "position": posicao
                            });

                            if !series_point_shapes.is_empty() || !series_point_sizes.is_empty() {
                                chart_json["style"] = serde_json::json!({
                                    "seriesPointShapes": series_point_shapes,
                                    "seriesPointSizes": series_point_sizes,
                                });
                            }

                            // Cara do Excel: cores, rótulos, fundo, eixos (`graficos_ancora::estilo_excel`).
                            if let Some((nativa, mut excel)) = graficos_ancora::estilo_excel(&root) {
                                if let Some(l) = lugar { excel["z"] = serde_json::json!(l.z); }
                                chart_json["_featherNative"] = nativa;
                                chart_json["_excel"] = excel;
                            }
                            if let Some(s) = saltos {
                                chart_json["categoryColumnIndex"] = serde_json::json!(s.categoria);
                                // O `hiddenSeries` sai com a aba já lida (`ocultar_colunas_puladas`).
                                chart_json[graficos_ancora::COLUNAS_SER] = serde_json::json!(s.colunas);
                            }
                            sheet.charts.push(chart_json);
                        }
                    }
                }
            }
        }
    }


    Ok(())
}

/// `fieldGroup` de um `cacheField` no formato `Agrupamento` do core: faixas
/// numéricas (`rangePr` com `startNum`/`endNum`/`groupInterval`) ou parte de
/// data (`groupBy` = years/quarters/months/days).
fn agrupamento_do_campo(cf: &xml::XmlNode) -> Option<serde_json::Value> {
    let range = cf.child("fieldGroup")?.child("rangePr")?;
    match range.attr("groupBy").unwrap_or("range") {
        "years" => Some(serde_json::json!({"tipo": "data", "parte": "ano"})),
        "quarters" => Some(serde_json::json!({"tipo": "data", "parte": "trimestre"})),
        "months" => Some(serde_json::json!({"tipo": "data", "parte": "mes"})),
        "days" => Some(serde_json::json!({"tipo": "data", "parte": "dia"})),
        "range" => {
            let numero = |nome: &str| range.attr(nome).and_then(|v| v.parse::<f64>().ok());
            Some(serde_json::json!({
                "tipo": "faixas",
                "inicio": numero("startNum")?,
                "fim": numero("endNum")?,
                "intervalo": numero("groupInterval").unwrap_or(1.0),
            }))
        }
        _ => None,
    }
}

/// A chave do filtro do painel (a do `parse_date_key` do core, sem agrupar)
/// de cada item do `sharedItems` de um `cacheField`: o texto da célula, com
/// `(Vazio)` para a vazia.
fn chaves_dos_itens(cf: &xml::XmlNode) -> Vec<String> {
    use feathersheet_core::celulas::values::{to_text, Value};
    let Some(compartilhados) = cf.child("sharedItems") else { return Vec::new() };
    compartilhados.children.iter().map(|item| {
        let v = item.attr("v").unwrap_or("");
        let valor = match item.name.as_str() {
            "m" => Value::Empty,
            "n" => v.parse::<f64>().map(Value::Number).unwrap_or(Value::Empty),
            "b" => Value::Bool(v == "1" || v.eq_ignore_ascii_case("true")),
            _ => Value::Text(v.to_string()),
        };
        let texto = to_text(&valor);
        if texto.trim().is_empty() { "(Vazio)".to_string() } else { texto }
    }).collect()
}

/// As chaves dos itens que um campo da dinâmica deixa de fora: os ocultos
/// (`h="1"`) ou, com `escolhido` (`pageField item`, filtro de um item só),
/// todos menos ele. Ordenadas e sem repetição, como o painel grava.
fn itens_ocultos(campo: Option<&xml::XmlNode>, chaves: Option<&Vec<String>>, escolhido: Option<usize>) -> Vec<String> {
    let (Some(campo), Some(chaves)) = (campo, chaves) else { return Vec::new() };
    let itens: Vec<&xml::XmlNode> = campo.child("items").map(|i| i.children_named("item").collect()).unwrap_or_default();
    let chave = |item: &xml::XmlNode| -> Option<String> {
        if item.attr("t").is_some_and(|t| t != "data") {
            return None; // subtotal, total...
        }
        chaves.get(item.attr("x")?.parse::<usize>().ok()?).cloned()
    };
    let mut saem: Vec<String> = match escolhido.and_then(|i| itens.get(i)).and_then(|item| chave(item)) {
        Some(fica) => chaves.iter().filter(|c| **c != fica).cloned().collect(),
        None => itens.iter()
            .filter(|item| matches!(item.attr("h"), Some("1") | Some("true")))
            .filter_map(|item| chave(item))
            .collect(),
    };
    saem.sort();
    saem.dedup();
    saem
}

/// O campo repete os rótulos dos itens (x14 `fillDownLabels`)?
fn rotulos_repetidos(campo: &xml::XmlNode) -> bool {
    campo.child("extLst").is_some_and(|lista| lista.children_named("ext").any(|ext| {
        ext.child("pivotField").is_some_and(|p| matches!(p.attr("fillDownLabels"), Some("1") | Some("true")))
    }))
}

/// O nome do item base (`baseItem`, índice nos `items` do campo base).
fn nome_do_item(campo: Option<&xml::XmlNode>, chaves: Option<&Vec<String>>, indice: usize) -> Option<String> {
    let chaves = chaves?;
    let x = match campo.and_then(|c| c.child("items")) {
        Some(itens) => itens.children_named("item").nth(indice)?.attr("x")?.parse::<usize>().ok()?,
        None => indice,
    };
    chaves.get(x).cloned()
}

/// A medida da ordem "por valor" (`autoSortScope` apontando para o
/// pseudo-campo Valores, 4294967294 = −2).
fn medida_da_ordem(campo: &xml::XmlNode) -> Option<usize> {
    campo.child("autoSortScope")?.child("pivotArea")?.child("references")?
        .children_named("reference").find(|r| r.attr("field") == Some("4294967294"))?
        .child("x")?.attr("v")?.parse().ok()
}

/// O que a dinâmica do Excel tem além de linhas/colunas/valores, já no
/// formato `Extras` do core (`dinamica::exibicao`): `showDataAs` de cada
/// `dataField`, `rowGrandTotals`/`colGrandTotals`, `sortType` e `fieldGroup`
/// dos campos de linha e coluna. O item base nomeado (`baseItem` por índice)
/// sai pelos itens do cache (`itens`); a classificação e o % do total
/// acumulado vêm do x14 `pivotShowAs`; a ordem "por valor", do `autoSortScope`.
fn extras_da_dinamica(
    p_node: &xml::XmlNode,
    p_fields: &[&xml::XmlNode],
    linhas: &[u32],
    colunas: &[u32],
    grupos: &[Option<serde_json::Value>],
    itens: &[Vec<String>],
) -> serde_json::Value {
    let campo_base = |idx: u32| -> Option<serde_json::Value> {
        if let Some(p) = linhas.iter().position(|&c| c == idx) {
            return Some(serde_json::json!({"eixo": "linhas", "posicao": p}));
        }
        colunas.iter().position(|&c| c == idx).map(|p| serde_json::json!({"eixo": "colunas", "posicao": p}))
    };
    let mut valores = Vec::new();
    if let Some(df_node) = p_node.child("dataFields") {
        for df in df_node.children_named("dataField") {
            let base = df.attr("baseField").and_then(|v| v.parse::<u32>().ok());
            let campo = base.and_then(campo_base);
            let item = match df.attr("baseItem").and_then(|v| v.parse::<u32>().ok()) {
                Some(1048828) => Some(serde_json::json!({"tipo": "anterior"})),
                Some(1048829) => Some(serde_json::json!({"tipo": "seguinte"})),
                Some(i) => base
                    .and_then(|f| nome_do_item(p_fields.get(f as usize).copied(), itens.get(f as usize), i as usize))
                    .map(|nome| serde_json::json!({"tipo": "item", "nome": nome})),
                None => None,
            };
            // Excel 2010+: classificação e % do total acumulado moram no
            // `x14:dataField pivotShowAs` e valem mais que o `showDataAs`.
            let x14 = df.child("extLst")
                .and_then(|l| l.children_named("ext").find_map(|e| e.child("dataField")?.attr("pivotShowAs")));
            let modo = match x14.or(df.attr("showDataAs")).unwrap_or("normal") {
                "percentOfTotal" => serde_json::json!({"tipo": "perc_total_geral"}),
                "percentOfCol" => serde_json::json!({"tipo": "perc_total_coluna"}),
                "percentOfRow" => serde_json::json!({"tipo": "perc_total_linha"}),
                "index" => serde_json::json!({"tipo": "indice"}),
                "runTotal" => match campo {
                    Some(c) => serde_json::json!({"tipo": "total_acumulado_em", "campo": c}),
                    None => serde_json::json!({"tipo": "normal"}),
                },
                kind @ ("percentOfRunningTotal" | "rankAscending" | "rankDescending") => match campo {
                    Some(c) => {
                        let tipo = match kind {
                            "percentOfRunningTotal" => "perc_total_acumulado_em",
                            "rankAscending" => "classificar_menor_maior",
                            _ => "classificar_maior_menor",
                        };
                        serde_json::json!({"tipo": tipo, "campo": c})
                    }
                    None => serde_json::json!({"tipo": "normal"}),
                },
                kind @ ("percent" | "difference" | "percentDiff") => match (campo, item) {
                    (Some(c), Some(i)) => {
                        let tipo = match kind {
                            "percent" => "perc_de",
                            "difference" => "diferenca_de",
                            _ => "perc_diferenca_de",
                        };
                        serde_json::json!({"tipo": tipo, "campo": c, "item": i})
                    }
                    _ => serde_json::json!({"tipo": "normal"}),
                },
                _ => serde_json::json!({"tipo": "normal"}),
            };
            valores.push(modo);
        }
    }
    let n_valores = valores.len();
    let por_campo = |lista: &[u32]| -> Vec<serde_json::Value> {
        lista
            .iter()
            .map(|&idx| {
                let campo = p_fields.get(idx as usize);
                let tipo = campo.and_then(|f| f.attr("sortType"));
                let por_valor = campo.and_then(|f| medida_da_ordem(f)).filter(|&m| m < n_valores);
                let ordenacao = match (tipo, por_valor) {
                    (Some(t @ ("ascending" | "descending")), Some(m)) => {
                        serde_json::json!({"tipo": "por_valor", "medida": m, "crescente": t == "ascending"})
                    }
                    (Some("ascending"), _) => serde_json::json!({"tipo": "crescente"}),
                    (Some("descending"), _) => serde_json::json!({"tipo": "decrescente"}),
                    _ => serde_json::json!({"tipo": "padrao"}),
                };
                serde_json::json!({
                    "agrupamento": grupos.get(idx as usize).cloned().flatten(),
                    "ordenacao": ordenacao,
                })
            })
            .collect()
    };
    let ligado = |nome: &str| p_node.attr(nome).map(|v| v != "0" && v != "false").unwrap_or(true);
    serde_json::json!({
        "valores": valores,
        "linhas": por_campo(linhas),
        "colunas": por_campo(colunas),
        "totais_gerais_linhas": ligado("rowGrandTotals"),
        "totais_gerais_colunas": ligado("colGrandTotals"),
    })
}

/// `cacheId` → caminho do `pivotCacheDefinition` no zip, pelo
/// `<pivotCaches>` do workbook.xml e as relações do workbook.
fn caminhos_dos_caches(archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> std::collections::HashMap<String, String> {
    let mut saida = std::collections::HashMap::new();
    let Ok(book) = entry_text(archive, "xl/workbook.xml").and_then(|t| xml::parse_xml(&t).map_err(|e| e.to_string())) else {
        return saida;
    };
    let rels = sheet::read_rels(&entry_text(archive, "xl/_rels/workbook.xml.rels").unwrap_or_default());
    if let Some(caches) = book.child("pivotCaches") {
        for c in caches.children_named("pivotCache") {
            let (Some(id), Some(alvo)) = (c.attr("cacheId"), c.attr("r:id").and_then(|r| rels.get(r))) else { continue };
            let caminho = match alvo.strip_prefix('/') {
                Some(abs) => abs.to_string(),
                None => format!("xl/{}", alvo.strip_prefix("./").unwrap_or(alvo)),
            };
            saida.insert(id.to_string(), caminho);
        }
    }
    saida
}

fn rand_id() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0))
}

fn extract_text_node(node: &xml::XmlNode) -> String {
    if !node.text.trim().is_empty() {
        return node.text.trim().to_string();
    }
    let mut res = String::new();
    for c in &node.children {
        let t = extract_text_node(c);
        if !t.is_empty() {
            if !res.is_empty() {
                res.push(' ');
            }
            res.push_str(&t);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Os testes nasceram lendo do disco, como o comando do `src-tauri`.
    fn ler_xlsx_livro(caminho: &str) -> Result<serde_json::Value, String> {
        let bytes = std::fs::read(caminho).map_err(|e| format!("Não consegui ler \"{caminho}\": {e}"))?;
        ler_xlsx_bytes(bytes)
    }

    fn ler_xlsx_nativo(caminho: String) -> Result<serde_json::Value, String> {
        ler_xlsx_livro(&caminho)
    }

    fn montar_xlsx(partes: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            for (nome, conteudo) in partes {
                zip.start_file(*nome, opts).unwrap();
                zip.write_all(conteudo.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    const WORKBOOK_MIN: &str = r#"<?xml version="1.0"?><workbook><sheets><sheet name="Plan1" r:id="rId1"/></sheets></workbook>"#;
    const RELS_MIN: &str = r#"<?xml version="1.0"?><Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;

    #[test]
    fn zip_pequeno_com_xml_expandido_demais_retorna_erro_claro() {
        // Limite artificial pequeno permite testar a protecao sem gastar RAM.
        let xml = "x".repeat(8_000);
        let zip = montar_xlsx(&[("xl/worksheets/sheet1.xml", &xml)]);
        let mut arquivo = ZipArchive::new(Cursor::new(zip.into())).unwrap();
        let erro = entry_text_com_limite(
            &mut arquivo, "xl/worksheets/sheet1.xml", 512,
        ).unwrap_err();
        assert!(erro.contains("excede"), "{erro}");
        let faltante = entry_text_com_limite(&mut arquivo, "xl/styles.xml", 512).unwrap();
        assert!(faltante.is_empty());
    }

    #[test]
    fn tabela_com_filtro_por_cor_e_ordenacao_vem_no_json() {
        let sheet1 = r#"<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>n</v></c><c r="B1" t="str"><v>v</v></c></row><row r="2"><c r="A2" t="str"><v>a</v></c><c r="B2"><v>3</v></c></row></sheetData><tableParts count="1"><tablePart r:id="rId1"/></tableParts></worksheet>"#;
        let rels_aba = r#"<Relationships><Relationship Id="rId1" Target="../tables/table1.xml"/></Relationships>"#;
        let tabela = r#"<table name="T1" ref="A1:B2"><autoFilter ref="A1:B2"><filterColumn colId="0"><colorFilter dxfId="0"/></filterColumn><filterColumn colId="1"><top10 val="5"/></filterColumn><sortState ref="A2:B2"><sortCondition sortBy="cellColor" ref="A2:A2" dxfId="0"/></sortState></autoFilter><tableColumns count="2"/></table>"#;
        let estilos = r#"<styleSheet><dxfs count="1"><dxf><fill><patternFill><bgColor rgb="FFFF0000"/></patternFill></fill></dxf></dxfs></styleSheet>"#;
        let bytes = montar_xlsx(&[
            ("xl/workbook.xml", WORKBOOK_MIN),
            ("xl/_rels/workbook.xml.rels", RELS_MIN),
            ("xl/worksheets/sheet1.xml", sheet1),
            ("xl/worksheets/_rels/sheet1.xml.rels", rels_aba),
            ("xl/tables/table1.xml", tabela),
            ("xl/styles.xml", estilos),
        ]);
        let json = ler_xlsx_bytes(bytes).unwrap();
        let t = &json["sheets"][0]["tabelas"][0];
        assert_eq!(t["filtros"][0]["criterio"]["op"], "cor_fundo", "{json}");
        assert_eq!(t["filtros"][0]["criterio"]["cor"], "#ff0000");
        assert_eq!(t["filtros"][1]["criterio"]["op"], "top10");
        assert_eq!(t["filtros"][1]["criterio"]["n"], 5);
        assert_eq!(t["ordenacao"]["niveis"][0]["por"], "cor_fundo");
    }

    #[test]
    fn abre_planilha_minima_com_um_valor() {
        let sheet1 = r#"<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>oi</v></c></row></sheetData></worksheet>"#;
        let bytes = montar_xlsx(&[
            ("xl/workbook.xml", WORKBOOK_MIN),
            ("xl/_rels/workbook.xml.rels", RELS_MIN),
            ("xl/worksheets/sheet1.xml", sheet1),
        ]);

        let dir = std::env::temp_dir().join(format!("feathersheet-test-{}.xlsx", std::process::id()));
        std::fs::write(&dir, &bytes).unwrap();
        let json = ler_xlsx_nativo(dir.to_string_lossy().to_string()).unwrap();
        std::fs::remove_file(&dir).ok();

        assert_eq!(json["sheets"][0]["name"], "Plan1");
        assert_eq!(json["sheets"][0]["cells"]["0"]["raw"], "oi");
    }

    /// Checagem manual, não roda no CI: `FEATHERSHEET_XLSX_TESTE=/caminho.xlsx
    /// cargo test -- --ignored imprime_json_de_arquivo_real`. Serve pra
    /// comparar visualmente com o que `io/xlsx.ts::importXlsx` dá pro mesmo
    /// arquivo gerado pelo `exportXlsx` (verificação cruzada Rust vs JS).
    #[test]
    #[ignore]
    fn imprime_json_de_arquivo_real() {
        let caminho = std::env::var("FEATHERSHEET_XLSX_TESTE").expect("defina FEATHERSHEET_XLSX_TESTE");
        let json = ler_xlsx_nativo(caminho).unwrap();
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }

    #[test]
    fn formula_abre_com_o_valor_guardado_e_recalcula_quando_muda() {
        use feathersheet_core::celulas::values::Value;
        // O <v> de B1 é 999 de propósito (A1*10 daria 20): prova que a
        // abertura usa o valor do arquivo em vez de recalcular.
        let sheet1 = r#"<worksheet><sheetData>
            <row r="1"><c r="A1"><v>2</v></c><c r="B1"><f>A1*10</f><v>999</v></c>
              <c r="C1" t="str"><f>IF(A1&gt;1,"sim","não")</f><v>sim</v></c><c r="D1"><f>NOW()</f><v>1</v></c></row>
            <row r="2"><c r="A2"><v>3</v></c><c r="B2"><f>SUM(A1:A2)</f><v>5</v></c></row>
        </sheetData></worksheet>"#;
        let bytes = montar_xlsx(&[
            ("xl/workbook.xml", WORKBOOK_MIN),
            ("xl/_rels/workbook.xml.rels", RELS_MIN),
            ("xl/worksheets/sheet1.xml", sheet1),
        ]);
        let (mut w, _) = super::native::ler_xlsx_workbook(bytes).unwrap();
        assert!(!w.calculo_pendente(0, 0, 1), "nada a calcular na primeira tela");
        assert_eq!(w.valor(0, 0, 1), Value::Number(999.0));
        assert_eq!(w.valor(0, 0, 2), Value::Text("sim".into()));
        assert_eq!(w.valor(0, 1, 1), Value::Number(5.0));
        assert_ne!(w.valor(0, 0, 3), Value::Number(1.0), "AGORA() não usa o valor velho");
        // Mudou o que ela lê: recalcula pelo motor.
        w.escrever_celula(0, 0, 0, "5".into());
        assert_eq!(w.valor(0, 0, 1), Value::Number(50.0));
        assert_eq!(w.valor(0, 1, 1), Value::Number(8.0));
    }

    #[test]
    fn arquivo_sem_workbook_xml_da_erro_claro() {
        let bytes = montar_xlsx(&[("xl/qualquercoisa.xml", "<a/>")]);
        let dir = std::env::temp_dir().join(format!("feathersheet-test-bad-{}.xlsx", std::process::id()));
        std::fs::write(&dir, &bytes).unwrap();
        let err = ler_xlsx_nativo(dir.to_string_lossy().to_string()).unwrap_err();
        std::fs::remove_file(&dir).ok();
        assert!(err.contains("Não parece uma planilha do Excel") || err.contains("Não parece um arquivo ZIP"));
    }
}

#[cfg(test)]
mod extras_dinamica_testes {
    use super::*;
    use feathersheet_core::dinamica::exibicao::{Agrupamento, Eixo, Extras, ItemBase, MostrarComo, Ordenacao, ParteData};

    #[test]
    fn show_data_as_totais_gerais_e_grupos_do_excel_viram_extras_do_core() {
        let raiz = xml::parse_xml(
            r#"<pivotTableDefinition rowGrandTotals="0" colGrandTotals="1">
                 <pivotFields count="2"><pivotField axis="axisRow" sortType="descending"/><pivotField/></pivotFields>
                 <dataFields count="2">
                   <dataField fld="1" showDataAs="percentOfCol"/>
                   <dataField fld="1" showDataAs="difference" baseField="0" baseItem="1048828"/>
                 </dataFields>
               </pivotTableDefinition>"#,
        )
        .unwrap();
        let cache = xml::parse_xml(
            r#"<cacheField name="Data"><fieldGroup base="0"><rangePr groupBy="months"/></fieldGroup></cacheField>"#,
        )
        .unwrap();
        let grupos = vec![agrupamento_do_campo(&cache), None];
        let campos: Vec<&xml::XmlNode> = raiz.child("pivotFields").unwrap().children_named("pivotField").collect();
        let json = extras_da_dinamica(&raiz, &campos, &[0], &[], &grupos, &[]);
        let e: Extras = serde_json::from_value(json).unwrap();
        assert!(!e.totais_gerais_linhas && e.totais_gerais_colunas);
        assert_eq!(e.valores[0], MostrarComo::PercTotalColuna);
        match &e.valores[1] {
            MostrarComo::DiferencaDe { campo, item } => {
                assert_eq!((campo.eixo, campo.posicao), (Eixo::Linhas, 0));
                assert_eq!(*item, ItemBase::Anterior);
            }
            outro => panic!("{outro:?}"),
        }
        assert_eq!(e.linhas[0].ordenacao, Ordenacao::Decrescente);
        assert_eq!(e.linhas[0].agrupamento, Some(Agrupamento::Data { parte: ParteData::Mes }));
    }
}
