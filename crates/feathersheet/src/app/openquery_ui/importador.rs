//! Importadores locais do FeatherQuery, isolados da thread do Iced.
//! O parser CSV/TSV e o XLSX são os mesmos do FeatherSheet; JSON e HTML
//! seguem o extrator Web do TypeScript sem executar scripts. Limites explícitos.
use super::*;
use std::path::Path;
use serde_json::Value as Json;
mod caminho_json;


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
    }
    // Listas de números/textos/booleanos viram coluna única (Valor).
    if cabecalho.is_empty() {
        cabecalho.push(tr!("Valor").into());
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

/// O que o "De arquivo" e o "Da pasta" leem.
pub(super) const EXTENSOES: [&str; 8] = ["csv", "tsv", "txt", "json", "xlsx", "xlsm", "fshz", "parquet"];
/// Os tipos do menu Obter dados ▸ Arquivo: (nome, extensões).
pub(super) const TIPOS: [(&str, &[&str]); 6] = [
    ("Pasta de trabalho do Excel", &["xlsx", "xlsm"]),
    ("Pasta de trabalho do FeatherSheet", &["fshz"]),
    ("Texto/CSV", &["csv", "tsv", "txt"]),
    ("JSON", &["json"]),
    ("Parquet", &["parquet"]),
    ("Consulta Web do Excel", &["iqy"]),
];

/// O `.iqy` do Excel (consulta Web): `WEB`, versão, endereço; uma linha logo depois
/// do endereço é o corpo do POST; depois da linha em branco, `Selection=` diz qual tabela
/// da página (o número, a partir de 1). Devolve (endereço, tabela).
pub(super) fn ler_iqy(texto: &str) -> Result<(String, Option<usize>), String> {
    let mut linhas = texto.lines().map(str::trim);
    let mut proxima = || linhas.by_ref().find(|l| !l.is_empty());
    if !proxima().is_some_and(|l| l.eq_ignore_ascii_case("WEB")) {
        return Err(tr!("O arquivo .iqy não é uma consulta Web do Excel").into());
    }
    let url = match proxima() {
        Some(v) if v.chars().all(|c| c.is_ascii_digit()) => proxima(),
        outra => outra,
    }.ok_or(tr!("O arquivo .iqy não tem o endereço da consulta"))?.to_owned();
    if url.contains("[\"") {
        return Err(tr!("Esta consulta .iqy pede parâmetros ao abrir; o FeatherQuery ainda não pergunta.").into());
    }
    let resto: Vec<&str> = texto.lines().map(str::trim).skip_while(|l| *l != url).skip(1).collect();
    if resto.first().is_some_and(|l| !l.is_empty()) {
        return Err(tr!("Esta consulta .iqy envia dados ao site (POST); o FeatherQuery só lê o endereço.").into());
    }
    let selecao = resto.iter().find_map(|l| l.strip_prefix("Selection="))
        .and_then(|v| v.split(',').next()?.trim().parse::<usize>().ok()).filter(|&n| n >= 1);
    Ok((url, selecao))
}
#[cfg(test)]
#[test]
fn iqy_le_endereco_tabela_e_recusa_post_e_parametros() {
    let iqy = "WEB\r\n1\r\nhttps://exemplo.org/cotacoes\r\n\r\nSelection=2,3\r\nFormatting=None\r\n";
    assert_eq!(ler_iqy(iqy).unwrap(), ("https://exemplo.org/cotacoes".into(), Some(2)));
    assert_eq!(ler_iqy("WEB\n1\nhttps://a.org/x\n\nSelection=EntirePage").unwrap().1, None);
    assert!(ler_iqy("WEB\n1\nhttps://a.org/x\nnome=1&b=2\n").is_err(), "POST");
    assert!(ler_iqy("WEB\n1\nhttps://a.org/?c=[\"Cidade\",\"Digite\"]\n").is_err(), "parâmetro");
    assert!(ler_iqy("<html>").is_err());
}

/// Teto do "Da pasta": arquivos e células somadas.
const MAX_ARQUIVOS_PASTA: usize = 500;
const MAX_CELULAS_PASTA: usize = 50_000_000;

/// Junta os arquivos da pasta (sem subpastas, em ordem de nome) numa tabela só: a
/// primeira coluna diz de que arquivo veio a linha, e as colunas casam pelo nome
/// (a que falta num arquivo fica vazia). Executar via spawn_blocking.
pub(super) fn ler_pasta(dir: &Path) -> Result<(Tabela, String, super::Formatos), String> {
    let mut arquivos: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| trf!("Falha ao ler a pasta: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSOES.contains(&e.to_ascii_lowercase().as_str())))
        // Arquivo temporário do Excel aberto (~$Vendas.xlsx) não é dado.
        .filter(|p| !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("~$")))
        .collect();
    arquivos.sort();
    if arquivos.is_empty() { return Err(tr!("A pasta não tem CSV, Excel, JSON, FSHZ nem Parquet").into()); }
    if arquivos.len() > MAX_ARQUIVOS_PASTA {
        return Err(trf!("A pasta tem {} arquivos; o limite é {MAX_ARQUIVOS_PASTA}", arquivos.len()));
    }
    let mut cabecalho = vec![tr!("Arquivo").to_string()];
    let mut linhas: Vec<Vec<Value>> = Vec::new();
    let mut formatos = super::Formatos::new();
    let mut celulas = 0usize;
    for caminho in &arquivos {
        let nome = caminho.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        let (t, _, f) = ler(caminho).map_err(|e| trf!("{nome}: {e}"))?;
        for (k, v) in f { formatos.entry(k).or_insert(v); }
        let destino: Vec<usize> = t.cabecalho.iter().map(|h| match cabecalho.iter().position(|c| c == h) {
            Some(i) => i,
            None => { cabecalho.push(h.clone()); cabecalho.len() - 1 }
        }).collect();
        celulas = celulas.saturating_add(t.linhas.len().saturating_mul(cabecalho.len()));
        if celulas > MAX_CELULAS_PASTA { return Err(tr!("Os arquivos da pasta passam de 50 milhões de células juntos").into()); }
        for l in t.linhas {
            let mut nova = vec![Value::Empty; cabecalho.len()];
            nova[0] = Value::Text(nome.clone());
            for (v, &d) in l.into_iter().zip(&destino) { nova[d] = v; }
            linhas.push(nova);
        }
    }
    for l in &mut linhas { l.resize(cabecalho.len(), Value::Empty); }
    Ok((Tabela { cabecalho, linhas }, format!("Pasta: {}", dir.display()), formatos))
}

/// Linhas de cada opção que o Navegador mostra antes da escolha.
const LINHAS_PREVIA: u32 = 8;

/// Uma opção do Navegador: o nome (planilha ou tabela), a prévia e de onde ler o resto.
/// A tabela inteira só é montada para a escolhida ([`Item::ler`]): montar todas antes
/// custava vários GB num `.fshz` de 1 milhão de linhas.
#[derive(Clone)]
pub struct Item {
    pub nome: String,
    pub previa: Tabela,
    /// Linhas de dados (pela área usada; linhas vazias no meio contam).
    pub linhas: usize,
    fonte: FonteItem,
}

#[derive(Clone)]
enum FonteItem {
    Pronta(Tabela, super::Formatos),
    /// A pasta lida fica guardada até a escolha (o `Workbook` não atravessa thread sem trava).
    Pasta(std::sync::Arc<std::sync::Mutex<Workbook>>, usize, Rect),
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Item({}, {} linhas)", self.nome, self.linhas)
    }
}

impl Item {
    pub(super) fn pronto(nome: String, t: Tabela, f: super::Formatos) -> Self {
        let previa = Tabela { cabecalho: t.cabecalho.clone(), linhas: t.linhas.iter().take(LINHAS_PREVIA as usize).cloned().collect() };
        Item { nome, linhas: t.linhas.len(), previa, fonte: FonteItem::Pronta(t, f) }
    }

    /// A tabela inteira e os formatos de número. Pesado: executar via spawn_blocking.
    pub(super) fn ler(self) -> Result<(Tabela, super::Formatos), String> {
        match self.fonte {
            FonteItem::Pronta(t, f) => Ok((t, f)),
            FonteItem::Pasta(wb, aba, area) => {
                let wb = wb.lock().map_err(|_| tr!("Arquivo indisponível").to_string())?;
                let t = ler_tabela(&wb, aba, area.clone())?;
                let f = super::formatos_da_fonte(&wb, aba, &area, &t.cabecalho);
                Ok((t, f))
            }
        }
    }
}

/// A primeira linha "de verdade" da área: pula o título acima da tabela ("Painel de
/// Vendas 2026" sozinho na linha 1) até a linha que preenche metade das colunas.
pub(super) fn sem_titulo(wb: &Workbook, aba: usize, area: Rect) -> Rect {
    let largura = (area.c1 - area.c0 + 1) as usize;
    let cheias = |r: u32| (area.c0..=area.c1).filter(|&c| {
        !feathersheet_core::celulas::values::to_text(&wb.valor(aba, r, c)).trim().is_empty()
    }).count();
    if largura < 2 || cheias(area.r0) * 2 >= largura { return area; }
    (area.r0 + 1..=area.r1.min(area.r0 + 10)).find(|&r| cheias(r) * 2 >= largura)
        .map_or(area.clone(), |r| Rect::new(r, area.c0, area.r1, area.c1))
}

/// O nome do item no idioma da tela (o guardado na origem fica em português, como o prefixo).
pub(super) fn nome_visivel(nome: &str) -> String {
    if let Some(n) = nome.strip_prefix("Planilha ") { trf!("Planilha {}", n) }
    else if let Some(n) = nome.strip_prefix("Tabela ") { trf!("Tabela {}", n) }
    else { nome.to_owned() }
}

/// As planilhas (a ativa primeiro) e as tabelas (Ctrl+T) da pasta, cada uma uma opção.
fn itens_da_pasta(wb: Workbook, ativa: usize) -> Vec<Item> {
    let mut ordem: Vec<usize> = vec![ativa];
    ordem.extend((0..wb.len()).filter(|i| *i != ativa));
    let mut opcoes: Vec<(String, usize, Rect)> = Vec::new();
    for i in ordem {
        let Some(aba) = wb.aba(i) else { continue };
        opcoes.push((format!("Planilha {}", aba.sheet.name), i, sem_titulo(&wb, i, aba.sheet.used_rect())));
    }
    opcoes.extend(tabelas_existentes(&wb).into_iter().map(|(i, nome, area)| (format!("Tabela {nome}"), i, area)));
    let mut itens = Vec::new();
    let mut previas = Vec::new();
    for (nome, i, area) in opcoes {
        let a = area.normalized();
        let corte = Rect::new(a.r0, a.c0, a.r1.min(a.r0 + LINHAS_PREVIA), a.c1);
        let Ok(previa) = ler_tabela(&wb, i, corte) else { continue };
        let planilha = nome.starts_with("Planilha ");
        if previa.cabecalho.is_empty() || (planilha && previa.linhas.is_empty() && previa.cabecalho.len() < 2) { continue; }
        previas.push((nome, previa, (a.r1 - a.r0) as usize, i, area));
    }
    let wb = std::sync::Arc::new(std::sync::Mutex::new(wb));
    for (nome, previa, linhas, i, area) in previas {
        itens.push(Item { nome, previa, linhas, fonte: FonteItem::Pasta(wb.clone(), i, area) });
    }
    itens
}

/// O arquivo como opções do Navegador: CSV/JSON dão uma; Excel e FSHZ, uma por planilha
/// e por tabela. Não modifica o Workbook aberto. Executar via spawn_blocking.
pub(super) fn ler_itens(path: &Path) -> Result<(Vec<Item>, String), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("")
        .to_ascii_lowercase();
    if !EXTENSOES.contains(&ext.as_str()) {
        return Err(tr!("Use CSV, TSV, TXT, JSON, XLSX, XLSM, FSHZ ou PARQUET neste importador").into());
    }
    let nome = path.file_name().and_then(|n| n.to_str()).unwrap_or(tr!("Arquivo"));
    let unico = |t: Tabela, f| vec![Item::pronto(nome.to_owned(), t, f)];
    let itens = match ext.as_str() {
        "json" => unico(json_tabela(&std::fs::read(path)
            .map_err(|e| trf!("Falha de leitura: {e}"))?)?, super::Formatos::new()),
        "csv" | "tsv" | "txt" => {
            let workbook = feathersheet_io::csv_workbook::importar_csv_tsv_arquivo(
                path, ext == "tsv", None,
            )?;
            let area = workbook.aba(0).ok_or(tr!("CSV sem aba"))?.sheet.used_rect();
            let t = ler_tabela(&workbook, 0, area.clone())?;
            let f = super::formatos_da_fonte(&workbook, 0, &area, &t.cabecalho);
            unico(t, f)
        }
        // Lido como o app abre o arquivo (fórmulas calculadas, sem teto próprio de tamanho:
        // o que o app abre, o FeatherQuery importa).
        _ => {
            let (workbook, folha) = crate::app::App::decodificar_caminho(path, None)?;
            itens_da_pasta(workbook, folha)
        }
    };
    if itens.is_empty() { return Err(tr!("Arquivo sem colunas").into()); }
    // A origem é dado (o prefixo diz como atualizar): não se traduz.
    Ok((itens, format!("Arquivo: {nome}")))
}

/// A primeira opção do arquivo (a planilha ativa), para o "Da pasta" e o reimportar.
pub(super) fn ler(path: &Path) -> Result<(Tabela, String, super::Formatos), String> {
    let (mut itens, origem) = ler_itens(path)?;
    let (t, f) = itens.swap_remove(0).ler()?;
    Ok((t, origem, f))
}


/// Lê as tabelas HTML da resposta uma única vez, sem executar JavaScript.
/// Aplica o limite de células ao CONJUNTO das tabelas, não a cada tabela
/// isoladamente, para impedir uma página com muitas grades de esgotar RAM.
fn html_tabelas(bytes: &[u8]) -> Result<Vec<Tabela>, String> {
    const MAX_TABELAS_HTML: usize = 32;
    // Site do governo ainda serve ISO-8859-1: vira UTF-8 como no CSV.
    let texto = feathersheet_io::csv_workbook::texto_de_bytes(bytes.to_vec());
    let documento = scraper::Html::parse_document(&texto);
    let seletor_tabela = scraper::Selector::parse("table").map_err(|e| e.to_string())?;
    let seletor_linha = scraper::Selector::parse("tr").map_err(|e| e.to_string())?;
    let seletor_celula = scraper::Selector::parse("th, td").map_err(|e| e.to_string())?;
    let mut tabelas = Vec::new();
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
                continue;
            }
            if celulas.len() > cabecalho.len() {
                return Err(trf!(
                    "Tabela HTML {} tem mais colunas do que o cabeçalho", indice + 1
                ));
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
/// O que a fonte Web aceita baixar (o servidor pode mandar qualquer coisa).
const TETO_WEB: u64 = 512 * 1024 * 1024;

/// Lê até `teto` bytes; passou disso, erro em vez de encher a memória.
fn ler_com_teto(leitor: &mut impl std::io::Read, teto: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    leitor.take(teto + 1).read_to_end(&mut bytes).map_err(|e| trf!("Falha ao baixar os dados: {e}"))?;
    if bytes.len() as u64 > teto {
        return Err(trf!("A resposta da Web passa de {} MB; baixe o arquivo e use Obter dados ▸ De arquivo.", teto >> 20));
    }
    Ok(bytes)
}

pub(super) fn ler_web(url: &str) -> Result<(Vec<Tabela>, String), String> {
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
    let mut resposta = client.get(endereco.clone()).send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|e| trf!("Não foi possível obter dados da Web: {e}"))?;
    let tipo = resposta.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let bytes = ler_com_teto(&mut resposta, TETO_WEB)?;
    let caminho = endereco.path().to_ascii_lowercase();
    let tsv = caminho.ends_with(".tsv") || tipo.contains("tab-separated");
    // APIs frequentemente respondem application/octet-stream ou text/plain.
    // O assistente TS tentava detectar JSON/HTML pelo próprio conteúdo.
    let primeiro = bytes.iter().copied().find(|byte| !byte.is_ascii_whitespace());
    let aparencia_json = matches!(primeiro, Some(b'{') | Some(b'['));
    let aparencia_html = {
            let inicio = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]);
            let inicio = inicio.trim_start().to_ascii_lowercase();
            inicio.starts_with("<!doctype html") || inicio.starts_with("<html")
                || inicio.starts_with("<table")
        };
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
    Ok((tabelas, format!("Web: {}", url.trim())))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resposta_da_web_acima_do_teto_vira_erro() {
        assert_eq!(ler_com_teto(&mut &b"abcd"[..], 4).unwrap(), b"abcd");
        assert!(ler_com_teto(&mut &b"abcde"[..], 4).unwrap_err().contains("passa de"));
    }

    #[test]
    fn da_pasta_junta_os_arquivos_pelo_nome_das_colunas() {
        let dir = std::env::temp_dir().join(format!("oq_pasta_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("vendas_01.csv"), "Cliente;Valor\nAna;10\nBia;20\n").unwrap();
        std::fs::write(dir.join("vendas_02.csv"), "Valor;Cliente;UF\n5;Caio;SP\n").unwrap();
        std::fs::write(dir.join("~$vendas_03.xlsx"), "lixo").unwrap();
        std::fs::write(dir.join("leia-me.md"), "não é dado").unwrap();
        let (t, origem, _) = ler_pasta(&dir).unwrap();
        assert!(origem.starts_with("Pasta: "));
        assert_eq!(t.cabecalho, ["Arquivo", "Cliente", "Valor", "UF"]);
        assert_eq!(t.linhas.len(), 3);
        assert_eq!(t.linhas[2], [Value::Text("vendas_02.csv".into()), Value::Text("Caio".into()), Value::Number(5.0), Value::Text("SP".into())]);
        assert_eq!(t.linhas[0][3], Value::Empty, "coluna que falta no arquivo fica vazia");
        std::fs::remove_dir_all(&dir).unwrap();
        let vazia = std::env::temp_dir().join(format!("oq_vazia_{}", std::process::id()));
        std::fs::create_dir_all(&vazia).unwrap();
        assert!(ler_pasta(&vazia).is_err());
        std::fs::remove_dir_all(&vazia).unwrap();
    }

    #[test]
    fn navegador_lista_as_planilhas_e_pula_o_titulo() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Painel de Vendas 2026".into());
        for (c, v) in ["Produto", "Valor", "UF"].iter().enumerate() { wb.escrever_celula(0, 2, c as u32, (*v).into()); }
        for (c, v) in ["Caneta", "3", "SP"].iter().enumerate() { wb.escrever_celula(0, 3, c as u32, (*v).into()); }
        let outra = wb.criar_aba("Clientes");
        wb.escrever_celula(outra, 0, 0, "Nome".into());
        wb.escrever_celula(outra, 1, 0, "Ana".into());
        let caminho = std::env::temp_dir().join(format!("oq_nav_{}.fshz", std::process::id()));
        std::fs::write(&caminho, feathersheet_io::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let (itens, origem) = ler_itens(&caminho).unwrap();
        std::fs::remove_file(&caminho).unwrap();
        assert!(origem.starts_with("Arquivo: "));
        assert_eq!(itens.len(), 2);
        assert_eq!(itens[0].previa.cabecalho, ["Produto", "Valor", "UF"], "o título acima da tabela sai");
        assert_eq!((itens[0].previa.linhas.len(), itens[0].linhas), (1, 1));
        assert!(itens[1].nome.ends_with("Clientes"));
        let (t, _) = itens.into_iter().next().unwrap().ler().unwrap();
        assert_eq!(t.cabecalho, ["Produto", "Valor", "UF"]);
    }

    #[test]
    fn de_arquivo_le_fshz_pela_aba_ativa() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Item".into());
        wb.escrever_celula(0, 1, 0, "Caneta".into());
        let caminho = std::env::temp_dir().join(format!("oq_fonte_{}.fshz", std::process::id()));
        std::fs::write(&caminho, feathersheet_io::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap()).unwrap();
        let (t, origem, _) = ler(&caminho).unwrap();
        std::fs::remove_file(&caminho).unwrap();
        assert_eq!((t.cabecalho.as_slice(), origem.starts_with("Arquivo: ")), (&["Item".to_string()][..], true));
        assert_eq!(t.linhas[0][0], Value::Text("Caneta".into()));
    }

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
    fn html_em_iso_8859_1_mantem_os_acentos() {
        let tabela = html_tabelas(b"<html><meta charset=iso-8859-1><table>\
            <tr><th>Munic\xedpio</th></tr><tr><td>S\xe3o Jo\xe3o</td></tr></table></html>").unwrap().remove(0);
        assert_eq!(tabela.cabecalho, ["Município"]);
        assert_eq!(tabela.linhas[0], [Value::Text("São João".into())]);
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
    fn mais_de_32_tabelas_html_pede_refinar_mas_tabela_grande_passa() {
        let html = format!("<html>{}</html>",
            "<table><tr><th>C</th></tr></table>".repeat(33));
        assert!(html_tabelas(html.as_bytes()).unwrap_err().contains("32 tabelas"));
        let html_grande = format!("<html>{}</html>",
            format!("<table><tr><th>x</th></tr>{}</table>",
                "<tr><td>1</td></tr>".repeat(9000)).repeat(24));
        assert_eq!(html_tabelas(html_grande.as_bytes()).unwrap().len(), 24);
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
    fn json_invalido_rejeitado_e_grande_aceito() {
        assert!(json_tabela(b"{isto-nao-e-json}").is_err());
        assert!(json_tabela(b"[]").is_err());
        let mut linhas = Vec::new();
        for _ in 0..1000 {
            linhas.push(serde_json::json!({"a":1,"b":2,"c":3}));
        }
        assert_eq!(json_tabela(&serde_json::to_vec(&linhas).unwrap()).unwrap().linhas.len(), 1000);
        let muitas = vec![serde_json::json!({"a":1,"b":2}); 100_000];
        assert_eq!(json_tabela(&serde_json::to_vec(&muitas).unwrap()).unwrap().linhas.len(), 100_000);
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
