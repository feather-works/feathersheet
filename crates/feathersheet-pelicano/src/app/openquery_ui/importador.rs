//! Importadores locais do OpenQuery, isolados da thread do Iced.
//! O parser CSV/TSV e o XLSX são os mesmos do FeatherSheet; JSON e HTML
//! seguem o extrator Web do TypeScript sem executar scripts. Limites explícitos.
use super::*;
use std::path::Path;
use serde_json::Value as Json;
mod caminho_json;

const MAX_ARQUIVO: u64 = 8 * 1024 * 1024;
const MAX_XML_XLSX: u64 = 32 * 1024 * 1024;

/// Valores primitivos preservam o tipo; objetos e listas aninhados viram
/// texto JSON, como o JSON.stringify do web-extractor.ts legado.
fn json_celula(v: &Json) -> Result<Value, String> {
    match v {
        Json::Null => Ok(Value::Empty),
        Json::Bool(v) => Ok(Value::Bool(*v)),
        Json::String(v) => Ok(Value::Text(v.clone())),
        Json::Number(v) => Ok(Value::Number(v.as_f64().ok_or(tr!("Número JSON inválido"))?)),
        Json::Array(_) | Json::Object(_) => serde_json::to_string(v)
            .map(Value::Text).map_err(|e| trf!("Não foi possível serializar campo JSON: {e}")),
    }
}

/// Compatibilidade com parseJSONWebData: array, objeto único ou primeiro
/// campo do objeto que contenha um array (respostas {data:[...]} / {results:[...]}).
/// Colunas são a união das chaves; linhas curtas são preenchidas com vazios.
/// Valida o tamanho ANTES de alocar a matriz tabular.
fn json_tabela(bytes: &[u8]) -> Result<Tabela, String> {
    json_tabela_caminho(bytes, "")
}

/// Permite selecionar `dados.itens[0].linhas` antes de tabular a resposta.
/// Sem caminho mantém a detecção antiga de arrays (data/results).
fn json_tabela_caminho(bytes: &[u8], caminho: &str) -> Result<Tabela, String> {
    let mut dado: Json = serde_json::from_slice(bytes)
        .map_err(|e| trf!("Não consegui interpretar JSON: {e}"))?;
    if !caminho.trim().is_empty() {
        dado = caminho_json::seguir(&dado, caminho)
            .ok_or_else(|| trf!("O caminho JSON '{caminho}' não existe ou é inválido"))?
            .clone();
    }
    let linhas = match dado {
        Json::Array(linhas) => linhas,
        Json::Object(mut obj) => {
            let array_chave = obj.iter()
                .find(|(_, valor)| valor.is_array())
                .map(|(chave, _)| chave.clone());
            if let Some(chave) = array_chave {
                match obj.remove(&chave) {
                    Some(Json::Array(linhas)) => linhas,
                    _ => unreachable!("chave verificada como array"),
                }
            } else {
                vec![Json::Object(obj)]
            }
        }
        _ => return Err(tr!("JSON precisa conter um objeto ou uma lista").into()),
    };
    if linhas.is_empty() { return Err(tr!("JSON não contém registros").into()); }

    let mut cabecalho = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    for registro in &linhas {
        match registro {
            Json::Object(obj) => {
                for chave in obj.keys() {
                    if vistos.insert(chave.clone()) { cabecalho.push(chave.clone()); }
                }
            }
            Json::Array(itens) => {
                // Object.keys([10,20]) no JavaScript devolve ["0","1"].
                for indice in 0..itens.len() {
                    let chave = indice.to_string();
                    if vistos.insert(chave.clone()) { cabecalho.push(chave); }
                }
            }
            _ => {}
        }
        if cabecalho.len() as u64 > MAX_CELULAS_FONTE {
            return Err(tr!("JSON possui colunas demais").into());
        }
    }
    // Listas de números/textos/booleanos viram coluna única (Valor).
    if cabecalho.is_empty() {
        cabecalho.push(tr!("Valor").into());
    }
    if (linhas.len() as u64 + 1).saturating_mul(cabecalho.len() as u64)
        > MAX_CELULAS_FONTE {
        return Err(trf!("JSON excede {} células; refine a fonte", MAX_CELULAS_FONTE));
    }

    let mut matriz = Vec::with_capacity(linhas.len());
    for registro in &linhas {
        let mut linha = Vec::with_capacity(cabecalho.len());
        match registro {
            Json::Object(obj) => for chave in &cabecalho {
                linha.push(json_celula(obj.get(chave).unwrap_or(&Json::Null))?);
            },
            Json::Array(itens) => for chave in &cabecalho {
                let valor = chave.parse::<usize>().ok()
                    .and_then(|i| itens.get(i)).unwrap_or(&Json::Null);
                linha.push(json_celula(valor)?);
            },
            // Mesmo contrato do TS: em dados mistos um escalar ocupa
            // apenas a primeira coluna, nunca é repetido nas demais.
            outro => {
                linha.push(json_celula(outro)?);
                linha.resize(cabecalho.len(), Value::Empty);
            }
        }
        matriz.push(linha);
    }
    Ok(Tabela { cabecalho, linhas: matriz })
}

fn validar_xlsx(path: &Path) -> Result<(), String> {
    let arquivo = std::fs::File::open(path)
        .map_err(|e| trf!("Não consegui abrir Excel: {e}"))?;
    let mut zip = zip::ZipArchive::new(arquivo)
        .map_err(|e| trf!("Arquivo Excel não é um ZIP válido: {e}"))?;
    if zip.len() > 512 {
        return Err(tr!("XLSX possui entradas demais para a importação interativa").into());
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| trf!("Entrada XLSX inválida: {e}"))?;
        total = total.saturating_add(entry.size());
        if total > MAX_XML_XLSX {
            return Err(tr!("XLSX descomprime mais de 32 MiB; importe-o pela planilha e selecione um intervalo").into());
        }
    }
    Ok(())
}

/// Não modifica o Workbook aberto. Executar via spawn_blocking.
pub(super) fn ler(path: &Path) -> Result<(Tabela, String), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("")
        .to_ascii_lowercase();
    if !["csv","tsv","json","xlsx","txt"].contains(&ext.as_str()) {
        return Err(tr!("Use CSV, TSV, TXT, JSON ou XLSX neste importador").into());
    }
    let tamanho = std::fs::metadata(path)
        .map_err(|e| trf!("Arquivo não encontrado: {e}"))?.len();
    if tamanho > MAX_ARQUIVO {
        return Err(tr!("Este arquivo excede 8 MiB. Abra pela planilha nativa, selecione a região e use OpenQuery").into());
    }
    let tabela = match ext.as_str() {
        "json" => json_tabela(&std::fs::read(path)
            .map_err(|e| trf!("Falha de leitura: {e}"))?)?,
        "csv" | "tsv" | "txt" => {
            let workbook = feathersheet_io::csv_workbook::importar_csv_tsv_arquivo(
                path, ext == "tsv", None,
            )?;
            let area = workbook.aba(0).ok_or(tr!("CSV sem aba"))?.sheet.used_rect();
            ler_tabela(&workbook, 0, area)?
        }
        "xlsx" => {
            validar_xlsx(path)?;
            let (workbook, _) = feathersheet_io::xlsx_read::ler_xlsx_arquivo(path)?;
            let area = workbook.aba(0).ok_or(tr!("Excel sem planilha"))?.sheet.used_rect();
            ler_tabela(&workbook, 0, area)?
        }
        _ => unreachable!(),
    };
    if tabela.cabecalho.is_empty() { return Err(tr!("Arquivo sem colunas").into()); }
    let nome = path.file_name().and_then(|n| n.to_str()).unwrap_or(tr!("Arquivo"));
    Ok((tabela, trf!("Arquivo: {nome}")))
}


/// Lê as tabelas HTML da resposta uma única vez, sem executar JavaScript.
/// Aplica o limite de células ao CONJUNTO das tabelas, não a cada tabela
/// isoladamente, para impedir uma página com muitas grades de esgotar RAM.
fn html_tabelas(bytes: &[u8]) -> Result<Vec<Tabela>, String> {
    const MAX_TABELAS_HTML: usize = 32;
    let texto = std::str::from_utf8(bytes)
        .map_err(|_| tr!("A página HTML precisa estar em UTF-8").to_string())?;
    let documento = scraper::Html::parse_document(texto);
    let seletor_tabela = scraper::Selector::parse("table").map_err(|e| e.to_string())?;
    let seletor_linha = scraper::Selector::parse("tr").map_err(|e| e.to_string())?;
    let seletor_celula = scraper::Selector::parse("th, td").map_err(|e| e.to_string())?;
    let mut tabelas = Vec::new();
    let mut total_celulas = 0u64;
    for (indice, tabela_html) in documento.select(&seletor_tabela).enumerate() {
        if indice >= MAX_TABELAS_HTML {
            return Err(tr!("A página contém mais de 32 tabelas HTML; refine a fonte").into());
        }
        let mut cabecalho = Vec::new();
        let mut linhas: Vec<Vec<Value>> = Vec::new();
        for linha in tabela_html.select(&seletor_linha) {
            let celulas = linha.select(&seletor_celula).map(|item| {
                item.text().collect::<Vec<_>>().join(" ").trim().to_owned()
            }).collect::<Vec<_>>();
            if celulas.is_empty() { continue; }
            if cabecalho.is_empty() {
                // Mesmo contrato do web-extractor.ts: primeira linha como
                // cabeçalho, seja composta por <th> ou por <td>.
                cabecalho = celulas.into_iter().enumerate()
                    .map(|(i, nome)| if nome.is_empty() {
                        trf!("Coluna {}", i + 1)
                    } else { nome })
                    .collect();
                if total_celulas.saturating_add(cabecalho.len() as u64) > MAX_CELULAS_FONTE {
                    return Err(tr!("As tabelas HTML excedem o limite de células da prévia").into());
                }
                continue;
            }
            if celulas.len() > cabecalho.len() {
                return Err(trf!(
                    "Tabela HTML {} tem mais colunas do que o cabeçalho", indice + 1
                ));
            }
            let previstas = total_celulas.saturating_add(
                (linhas.len() as u64 + 2).saturating_mul(cabecalho.len() as u64),
            );
            if previstas > MAX_CELULAS_FONTE {
                return Err(tr!("As tabelas HTML excedem o limite de células da prévia").into());
            }
            let mut valores: Vec<Value> = celulas.into_iter().map(|v| {
                if v.is_empty() { Value::Empty }
                else if let Ok(n) = v.parse::<f64>() {
                    if n.is_finite() { Value::Number(n) } else { Value::Text(v) }
                } else if v.eq_ignore_ascii_case("true") { Value::Bool(true) }
                else if v.eq_ignore_ascii_case("false") { Value::Bool(false) }
                else { Value::Text(v) }
            }).collect();
            valores.resize(cabecalho.len(), Value::Empty);
            linhas.push(valores);
        }
        if !cabecalho.is_empty() {
            total_celulas += (linhas.len() as u64 + 1) * cabecalho.len() as u64;
            tabelas.push(Tabela { cabecalho, linhas });
        }
    }
    if tabelas.is_empty() {
        return Err(tr!("Esta página não contém <table> HTML estática com cabeçalho").into());
    }
    Ok(tabelas)
}

/// Consulta HTTP(S) em tarefa bloqueante separada da UI. Aceita JSON,
/// CSV/TSV e páginas HTML estáticas. Não interpreta scripts da página.
/// Corpo e redirecionamentos têm limite para não consumir memória ilimitada.
pub(super) fn ler_web(url: &str) -> Result<(Vec<Tabela>, String), String> {
    use std::io::Read;
    let mut endereco = reqwest::Url::parse(url.trim())
        .map_err(|e| trf!("URL inválida: {e}"))?;
    let caminho_json = caminho_json::extrair_fragmento(&mut endereco)?;
    if !matches!(endereco.scheme(), "http" | "https") {
        return Err(tr!("A fonte Web precisa usar http:// ou https://").into());
    }
    if endereco.host_str().is_none() {
        return Err(tr!("Informe um endereço HTTP(S) com domínio ou IP").into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build().map_err(|e| trf!("Erro no cliente Web: {e}"))?;
    let resposta = client.get(endereco.clone()).send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|e| trf!("Não foi possível obter dados da Web: {e}"))?;
    if resposta.content_length().is_some_and(|tam| tam > MAX_ARQUIVO) {
        return Err(tr!("Resposta Web acima de 8 MiB; escolha uma fonte menor").into());
    }
    let tipo = resposta.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let mut bytes = Vec::new();
    resposta.take(MAX_ARQUIVO + 1).read_to_end(&mut bytes)
        .map_err(|e| trf!("Falha ao baixar os dados: {e}"))?;
    if bytes.len() as u64 > MAX_ARQUIVO {
        return Err(tr!("Resposta Web acima de 8 MiB; refine a URL da consulta").into());
    }
    let caminho = endereco.path().to_ascii_lowercase();
    let tsv = caminho.ends_with(".tsv") || tipo.contains("tab-separated");
    // APIs frequentemente respondem application/octet-stream ou text/plain.
    // O assistente TS tentava detectar JSON/HTML pelo próprio conteúdo.
    let primeiro = bytes.iter().copied().find(|byte| !byte.is_ascii_whitespace());
    let aparencia_json = matches!(primeiro, Some(b'{') | Some(b'['));
    let aparencia_html = std::str::from_utf8(&bytes[..bytes.len().min(256)])
        .ok().is_some_and(|inicio| {
            let inicio = inicio.trim_start().to_ascii_lowercase();
            inicio.starts_with("<!doctype html") || inicio.starts_with("<html")
                || inicio.starts_with("<table")
        });
    let json = caminho.ends_with(".json")
        || tipo.contains("application/json") || tipo.contains("+json")
        || (aparencia_json && !tipo.contains("text/html"));
    let html = caminho.ends_with(".html") || caminho.ends_with(".htm")
        || tipo.contains("text/html") || aparencia_html;
    let tabelas = if json {
        vec![json_tabela_caminho(&bytes, caminho_json.as_deref().unwrap_or(""))?]
    } else if html {
        html_tabelas(&bytes)?
    } else if tsv || caminho.ends_with(".csv") || tipo.contains("text/csv")
        || tipo.contains("text/plain") || tipo.contains("application/csv")
        || tipo.contains("application/octet-stream")
        || (tipo.is_empty() && !caminho.ends_with(".html"))
    {
        let workbook = feathersheet_io::csv_workbook::importar_csv_tsv(bytes, tsv)?;
        let area = workbook.aba(0).ok_or(tr!("Resposta Web sem linhas"))?.sheet.used_rect();
        vec![ler_tabela(&workbook, 0, area)?]
    } else {
        return Err(tr!("Formato Web não suportado. Use JSON, CSV, TSV ou páginas HTML com tabelas estáticas.").into());
    };
    if tabelas.is_empty() || tabelas.iter().any(|t| t.cabecalho.is_empty()) {
        return Err(tr!("A resposta Web não contém colunas").into());
    }
    Ok((tabelas, trf!("Web: {}", url.trim())))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn extrai_html_estatico_sem_executar_script() {
        let tabela = html_tabelas(br#"<html><table>
            <tr><th>Nome</th><th>Valor</th></tr>
            <tr><td>Ana</td><td>12</td></tr>
        </table><script>throw new Error('nao executar')</script></html>"#).unwrap().remove(0);
        assert_eq!(tabela.cabecalho, ["Nome", "Valor"]);
        assert_eq!(tabela.linhas[0], [Value::Text("Ana".into()), Value::Number(12.0)]);
    }

    #[test]
    fn html_com_varias_tabelas_permite_selecionar_sem_duplicar_download() {
        let tabelas = html_tabelas(r#"<html>
            <table><tr><th>Produto</th><th>Qtd</th></tr>
                <tr><td>A</td><td>3</td></tr></table>
            <table><tr><td>Cidade</td><td>Ativo</td></tr>
                <tr><td>Vitória</td><td>true</td></tr>
                <tr><td>Aracruz</td><td>false</td></tr></table>
        </html>"#.as_bytes()).unwrap();
        assert_eq!(tabelas.len(), 2);
        assert_eq!(tabelas[0].cabecalho, ["Produto", "Qtd"]);
        assert_eq!(tabelas[1].cabecalho, ["Cidade", "Ativo"]);
        assert_eq!(tabelas[1].linhas.len(), 2);
        assert_eq!(tabelas[1].linhas[0][0], Value::Text("Vitória".into()));
        assert_eq!(tabelas[1].linhas[0][1], Value::Bool(true));
        assert_eq!(tabelas[1].linhas[1][1], Value::Bool(false));
    }

    #[test]
    fn muitas_tabelas_html_nao_alocam_memoria_sem_limites() {
        let html = format!("<html>{}</html>",
            "<table><tr><th>C</th></tr></table>".repeat(33));
        assert!(html_tabelas(html.as_bytes()).unwrap_err().contains("32 tabelas"));
        let html_grande = format!("<html>{}</html>",
            format!("<table><tr><th>x</th></tr>{}</table>",
                "<tr><td>1</td></tr>".repeat(9000)).repeat(24));
        assert!(html_tabelas(html_grande.as_bytes()).is_err());
    }

    #[test]
    fn caminho_json_seleciona_array_aninhado_antes_da_tabulacao() {
        let t = json_tabela_caminho(
            br#"{"dados":{"itens":[{"linhas":[
                {"nome":"Ana","quantia":3},
                {"nome":"Bia","quantia":7}
            ]}]}}"#,
            "dados.itens[0].linhas"
        ).unwrap();
        assert_eq!(t.cabecalho, vec!["nome", "quantia"]);
        assert_eq!(t.linhas.len(), 2);
        assert_eq!(t.linhas[1][0], Value::Text("Bia".into()));
        assert_eq!(t.linhas[1][1], Value::Number(7.0));
        assert!(json_tabela_caminho(br#"{"dados":[]}"#, "dados[0]").is_err());
    }

    #[test]
    fn json_objetos_com_colunas_distintas_aceita_tipos_sem_strings_fake() {
        let t = json_tabela(br#"[{"a":1,"b":true},{"a":2.5,"c":null}]"#).unwrap();
        assert_eq!(t.cabecalho, vec!["a","b","c"]);
        assert_eq!(t.linhas[0][0], Value::Number(1.));
        assert_eq!(t.linhas[0][1], Value::Bool(true));
        assert_eq!(t.linhas[1][2], Value::Empty);
    }
    #[test]
    fn json_web_aceita_envelope_data_e_campos_aninhados() {
        let t = json_tabela(r#"{
            "status":"ok",
            "data":[
                {"nome":"Ana","tags":["a","b"],"detalhes":{"ativo":true}},
                {"nome":"João","detalhes":null}
            ]
        }"#.as_bytes()).unwrap();
        assert_eq!(t.linhas.len(), 2);
        let nome = t.cabecalho.iter().position(|v| v == "nome").unwrap();
        let tags = t.cabecalho.iter().position(|v| v == "tags").unwrap();
        let detalhes = t.cabecalho.iter().position(|v| v == "detalhes").unwrap();
        assert_eq!(t.linhas[0][nome], Value::Text("Ana".into()));
        assert_eq!(t.linhas[1][nome], Value::Text("João".into()));
        assert_eq!(t.linhas[0][tags], Value::Text(r#"["a","b"]"#.into()));
        assert_eq!(t.linhas[1][tags], Value::Empty);
        assert_eq!(t.linhas[0][detalhes],
            Value::Text(r#"{"ativo":true}"#.into()));
    }

    #[test]
    fn json_web_aceita_array_primitivo_ou_registros_como_listas() {
        let valores = json_tabela(br#"{"results":[12,true,"ok",null]}"#).unwrap();
        assert_eq!(valores.cabecalho, ["Valor"]);
        assert_eq!(valores.linhas, vec![
            vec![Value::Number(12.)], vec![Value::Bool(true)],
            vec![Value::Text("ok".into())], vec![Value::Empty],
        ]);
        let colunas = json_tabela(br#"[[1,"A"],[2,"B",false]]"#).unwrap();
        assert_eq!(colunas.cabecalho, ["0", "1", "2"]);
        assert_eq!(colunas.linhas[0], vec![
            Value::Number(1.), Value::Text("A".into()), Value::Empty
        ]);
        assert_eq!(colunas.linhas[1][2], Value::Bool(false));
    }

    #[test]
    fn html_sem_th_usa_primeira_linha_como_cabecalho_e_converte_booleanos() {
        let t = html_tabelas(br#"<table>
            <tr><td>Nome</td><td>Ativo</td></tr>
            <tr><td>Ana</td><td>TrUe</td></tr>
            <tr><td>Beto</td><td>false</td></tr>
        </table>"#).unwrap().remove(0);
        assert_eq!(t.cabecalho, ["Nome", "Ativo"]);
        assert_eq!(t.linhas.len(), 2);
        assert_eq!(t.linhas[0][1], Value::Bool(true));
        assert_eq!(t.linhas[1][1], Value::Bool(false));
    }

    #[test]
    fn json_invalido_e_expansivo_rejeitado() {
        assert!(json_tabela(b"{isto-nao-e-json}").is_err());
        assert!(json_tabela(b"[]").is_err());
        // Limite do editor é em células, não apenas nos bytes da resposta.
        let mut linhas = Vec::new();
        for _ in 0..1000 {
            linhas.push(serde_json::json!({"a":1,"b":2,"c":3}));
        }
        assert_eq!(json_tabela(&serde_json::to_vec(&linhas).unwrap()).unwrap().linhas.len(), 1000);
        let muitas = vec![serde_json::json!({"a":1,"b":2}); 100_000];
        assert!(json_tabela(&serde_json::to_vec(&muitas).unwrap()).is_err());
    }
    #[test]
    fn csv_real_lido_pelo_parser_nativo() {
        let p = std::env::temp_dir().join(format!(
            "feathersheet-openquery-{}-{}.csv",
            std::process::id(),
            std::thread::current().name().unwrap_or("t"),
        ));
        std::fs::write(&p, b"Nome;Consumo\nVitoria;42\n").unwrap();
        let tabela = ler(&p).unwrap().0;
        let _ = std::fs::remove_file(&p);
        assert_eq!(tabela.cabecalho, vec!["Nome","Consumo"]);
        assert_eq!(tabela.linhas.len(), 1);
    }
}
