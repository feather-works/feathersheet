//! Operações da pasta inteira pedidas pelas macros convertidas do VBA:
//! ativar aba, criar nome, nome e pasta do arquivo, mensagens e a
//! pergunta Sim/Não, e os pedidos de arquivo (PDF de uma aba, abas salvas
//! num arquivo novo). Nada aqui lê ou grava fora da pasta em memória: os
//! pedidos só ficam anotados para o aplicativo gravar depois.
use feathersheet_core::celulas::refs::{parse_range_ref, Rect};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};

/// Um arquivo que a macro pediu para gravar. O aplicativo grava só se a
/// macro terminar bem. As abas vão pelo `id` (não mudam se outra aba sair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedidoArquivo {
    /// `sheet.exportPdf(caminho)`: a área de impressão da aba (ou a parte usada).
    Pdf { aba: u32, caminho: PathBuf },
    /// `wb.saveSheetsAs(abas, caminho)`: uma pasta nova só com estas abas,
    /// em `.xlsx` ou `.fshz` (pela extensão).
    Abas { abas: Vec<u32>, caminho: PathBuf },
}

/// O que o aplicativo informa à macro antes de rodar.
#[derive(Debug, Default, Clone)]
pub struct Contexto {
    /// Arquivo aberto (`wb.name`, `wb.path`, `wb.fullName`); nenhum numa pasta nova.
    pub arquivo: Option<PathBuf>,
    /// Seleção da aba ativa, como "B3" ou "A1:C4".
    pub selecao: Option<String>,
    /// Respostas já dadas às perguntas Sim/Não, na ordem em que a macro as faz.
    /// A pergunta sem resposta interrompe a execução: o aplicativo pergunta à
    /// pessoa e roda a macro de novo com a resposta acrescentada.
    pub respostas: Vec<bool>,
}

/// O que a execução acumula para entregar ao aplicativo no fim.
#[derive(Debug, Default)]
pub(crate) struct Acumulado {
    pub contexto: Contexto,
    pub perguntas_feitas: usize,
    pub pergunta: Option<String>,
    pub mensagens: Vec<String>,
    /// Aplicados só se a macro terminar bem (o desfazer das abas não os cobre).
    pub nomes: Vec<(String, Rect)>,
    pub ativou: Option<usize>,
    pub arquivos: Vec<PedidoArquivo>,
}

/// Mais que isto numa execução é laço descontrolado, não exportação.
const LIMITE_ARQUIVOS: usize = 200;

const LIMITE_TEXTO: usize = 4096;

/// As operações desta parte (consultadas antes de montar a lista de abas).
pub(crate) const OPERACOES: &[&str] = &["alerta", "confirmar", "ativar", "abaAtiva",
    "selecaoInicial", "definirNome", "arquivo", "exportarPdf", "salvarAbas"];

/// O caminho pedido pela macro, absoluto. `\` do VBA vira `/` fora do
/// Windows; o relativo vale a partir da pasta do arquivo aberto. Sem
/// arquivo aberto, `wb.path` é vazio e `wb.path + "\x.pdf"` daria a raiz
/// do disco: isso também pede para salvar a pasta antes. A macro vem dentro
/// do documento, então só grava dentro da pasta dele (ou de uma subpasta):
/// absoluto fora dela e `..` que escapa são recusados.
fn resolver_caminho(pedido: &str, arquivo: Option<&Path>) -> Result<PathBuf, String> {
    let texto = pedido.trim();
    if texto.is_empty() { return Err("Caminho de arquivo vazio".into()); }
    let texto = if cfg!(windows) { texto.to_string() } else { texto.replace('\\', "/") };
    let caminho = PathBuf::from(&texto);
    let pasta = arquivo.and_then(Path::parent).filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| format!("Salve a pasta antes: o caminho {texto} depende da pasta do arquivo"))?;
    let pasta = normalizar(pasta).ok_or_else(|| format!("Pasta do arquivo inválida: {}", pasta.display()))?;
    let alvo = normalizar(&pasta.join(&caminho)).filter(|c| c.starts_with(&pasta) && *c != pasta);
    alvo.ok_or_else(|| format!("A macro só grava dentro da pasta do arquivo ({}): {texto}", pasta.display()))
}

/// Tira `.` e resolve `..` só pelo texto (sem tocar no disco); `None` se o
/// `..` passa da raiz.
fn normalizar(caminho: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut saida = PathBuf::new();
    for parte in caminho.components() {
        match parte {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(saida.components().next_back(), Some(Component::Normal(_))) { return None; }
                saida.pop();
            }
            outra => saida.push(outra),
        }
    }
    Some(saida)
}

/// Completa a extensão que falta (`relatorio` → `relatorio.pdf`) e recusa a
/// que não se sabe gravar.
fn com_extensao(mut caminho: PathBuf, aceitas: &[&str]) -> Result<PathBuf, String> {
    match caminho.extension().map(|e| e.to_string_lossy().to_lowercase()) {
        None => { caminho.set_extension(aceitas[0]); Ok(caminho) }
        Some(e) if aceitas.contains(&e.as_str()) => Ok(caminho),
        Some(e) => Err(format!("Formato .{e} não suportado pela macro (use {})",
            aceitas.iter().map(|a| format!(".{a}")).collect::<Vec<_>>().join(" ou "))),
    }
}

/// A aba pelo nome (sem diferenciar maiúsculas) ou pelo número (do 1).
fn indice_da_aba(v: &Json, nomes_abas: &[String]) -> Result<usize, String> {
    let achado = match v {
        Json::String(n) => nomes_abas.iter().position(|x| x.trim().eq_ignore_ascii_case(n.trim())),
        Json::Number(n) => n.as_u64().and_then(|n| (n as usize).checked_sub(1)).filter(|&i| i < nomes_abas.len()),
        _ => None,
    };
    achado.ok_or_else(|| format!("Aba inexistente: {}", texto_curto(v)))
}

fn texto_curto(v: &Json) -> String {
    match v { Json::String(s) => s.chars().take(80).collect(), x => x.to_string().chars().take(80).collect() }
}

fn pedido_de_arquivo(op: &str, dados: &Json, a: &mut Acumulado, nomes_abas: &[String], ids: &[u32])
    -> Result<Json, String>
{
    if a.arquivos.len() >= LIMITE_ARQUIVOS {
        return Err(format!("Limite de {LIMITE_ARQUIVOS} arquivos por execução"));
    }
    let caminho = resolver_caminho(&texto(dados, 1), a.contexto.arquivo.as_deref())?;
    let pedido = if op == "exportarPdf" {
        let aba = dados[0].as_u64().map(|i| i as usize).filter(|&i| i < ids.len())
            .ok_or("Aba inexistente")?;
        PedidoArquivo::Pdf { aba: ids[aba], caminho: com_extensao(caminho, &["pdf"])? }
    } else {
        let lista = dados[0].as_array().ok_or("Informe a lista de abas a salvar")?;
        let mut abas = Vec::new();
        for v in lista.iter().filter(|v| !v.is_null()) {
            let id = ids[indice_da_aba(v, nomes_abas)?];
            if !abas.contains(&id) { abas.push(id); }
        }
        if abas.is_empty() { return Err("Nenhuma aba para salvar no arquivo novo".into()); }
        PedidoArquivo::Abas { abas, caminho: com_extensao(caminho, &["xlsx", "fshz"])? }
    };
    a.arquivos.push(pedido);
    Ok(Json::Null)
}

fn texto(dados: &Json, i: usize) -> String {
    match dados.get(i) {
        Some(Json::String(s)) => s.chars().take(LIMITE_TEXTO).collect(),
        Some(Json::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

/// `None` quando a operação não é desta parte.
pub(crate) fn executar(
    op: &str, dados: &Json, a: &mut Acumulado, nomes_abas: &[String], ids: &[u32], aba_ativa: usize,
) -> Option<Result<Json, String>> {
    Some(match op {
        "exportarPdf" | "salvarAbas" => pedido_de_arquivo(op, dados, a, nomes_abas, ids),
        "alerta" => {
            if a.mensagens.len() < 200 { a.mensagens.push(texto(dados, 0)); }
            Ok(Json::Null)
        }
        "confirmar" => {
            let n = a.perguntas_feitas;
            a.perguntas_feitas += 1;
            match a.contexto.respostas.get(n) {
                Some(&r) => Ok(json!(r)),
                None => {
                    // Responde Não para a macro seguir sem efeito; o resultado
                    // é descartado e a pessoa responde de verdade.
                    a.pergunta.get_or_insert_with(|| texto(dados, 0));
                    Ok(json!(false))
                }
            }
        }
        "ativar" => {
            let i = dados[0].as_u64().map(|i| i as usize).filter(|&i| i < nomes_abas.len());
            match i {
                Some(i) => { a.ativou = Some(i); Ok(Json::Null) }
                None => Err("Aba inexistente".into()),
            }
        }
        "abaAtiva" => Ok(json!(a.ativou.unwrap_or(aba_ativa))),
        "selecaoInicial" => Ok(json!(a.contexto.selecao.clone().unwrap_or_else(|| "A1".into()))),
        "definirNome" => {
            let nome = texto(dados, 0).trim().to_string();
            let valido = nome.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && nome.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.'))
                && parse_range_ref(&nome).is_none();
            if !valido { return Some(Err(format!("Nome inválido: {nome}"))); }
            let refe = texto(dados, 1);
            let refe = refe.trim().trim_start_matches('=');
            // `Aba!A1:B2` ou `'Minha aba'!A1`: o parser só lê a parte do intervalo.
            let (aba_ref, refe) = match refe.rfind('!') {
                Some(i) => {
                    let p = refe[..i].trim();
                    let p = p.strip_prefix('\'').and_then(|x| x.strip_suffix('\''))
                        .map_or_else(|| p.to_string(), |x| x.replace("''", "'"));
                    (Some(p), refe[i + 1..].replace('$', ""))
                }
                None => (None, refe.replace('$', "")),
            };
            let refe = refe.as_str();
            let Some(mut area) = parse_range_ref(refe) else {
                return Some(Err(format!("Referência inválida para o nome {nome}: {refe}")));
            };
            area.sheet = aba_ref;
            if area.sheet.is_none() {
                let aba = dados[2].as_u64().map(|i| i as usize).unwrap_or(aba_ativa);
                area.sheet = nomes_abas.get(aba).cloned();
            }
            a.nomes.retain(|(n, _)| !n.eq_ignore_ascii_case(&nome));
            if a.nomes.len() < 10_000 { a.nomes.push((nome, area)); }
            Ok(Json::Null)
        }
        "arquivo" => {
            let c = &a.contexto.arquivo;
            let nome = c.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
            let base = c.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().into_owned());
            let pasta = c.as_ref().and_then(|p| p.parent()).map(|n| n.to_string_lossy().into_owned());
            let completo = c.as_ref().map(|p| p.to_string_lossy().into_owned());
            Ok(json!({"nome": nome.unwrap_or_else(|| "Pasta1".into()),
                "base": base.unwrap_or_else(|| "Pasta1".into()),
                "pasta": pasta.unwrap_or_default(), "completo": completo.unwrap_or_default()}))
        }
        _ => return None,
    })
}
