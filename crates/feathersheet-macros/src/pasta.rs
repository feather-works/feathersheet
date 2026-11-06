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

/// `sheet.protect(senha, permissões)` / `sheet.unprotect(senha)`: o aplicativo aplica
/// (com o hash da senha do Excel) se a macro terminar bem. A macro já escreve na aba
/// protegida, então a ordem desproteger → escrever → proteger dá o mesmo resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoProtecao {
    pub aba: u32,
    pub proteger: bool,
    pub senha: Option<String>,
    /// As permissões pelas chaves do `.xlsx` (`formatCells`, `sort`…), já conferidas.
    pub permitidas: Vec<feathersheet_core::planilha::sheet::protecao::Permissao>,
}

/// `sheet.autoFilter`/`showAllData`/`autoFilterOff`: o aplicativo filtra (com as setas do
/// AutoFiltro e um Ctrl+Z) se a macro terminar bem.
#[derive(Clone, Debug, PartialEq)]
pub struct PedidoFiltro {
    pub aba: u32,
    pub acao: AcaoFiltro,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AcaoFiltro {
    /// Liga o AutoFiltro em `area` (se ainda não há) e, com `coluna` (1 = primeira da área),
    /// troca o critério dela; critério `None` limpa a coluna. O critério é o JSON do filtro
    /// (`feathersheet_io::filtro_json`): lista, `{op, val}` ou o texto clássico (`">100"`).
    Aplicar { area: Rect, coluna: Option<u32>, criterio: Option<serde_json::Value> },
    /// `ShowAllData`: tira os critérios, o AutoFiltro fica.
    MostrarTudo,
    /// `AutoFilterMode = False`: tira o AutoFiltro da aba.
    Remover,
}

/// `sheet.pastePicture(origem, célula)` (o `CopyPicture` + `Paste` do VBA): o aplicativo desenha
/// `area` da aba `origem` como imagem e a cola com o canto em `celula` da aba `destino`, se a
/// macro terminar bem. `largura`/`altura` em pontos, como o `Selection.Width` do VBA.
#[derive(Clone, Debug, PartialEq)]
pub struct PedidoImagem {
    pub origem: u32,
    pub area: Rect,
    pub destino: u32,
    pub celula: (u32, u32),
    pub largura: Option<f64>,
    pub altura: Option<f64>,
}

/// `shape.left/top/width/height = …` (pontos, como no VBA): a posição depende das larguras da
/// grade, então o aplicativo move a forma se a macro terminar bem.
#[derive(Clone, Debug, PartialEq)]
pub struct PedidoForma {
    pub aba: u32,
    pub nome: String,
    pub esquerda: Option<f64>,
    pub topo: Option<f64>,
    pub largura: Option<f64>,
    pub altura: Option<f64>,
}

/// Pergunta de vários botões (`app.choose`): os códigos de botão do `MsgBox` do VBA (1 OK,
/// 2 Cancelar, 3 Anular, 4 Repetir, 5 Ignorar, 6 Sim, 7 Não), o ícone (16 crítico, 32
/// pergunta, 48 aviso, 64 informação, 0 nenhum) e o índice do botão padrão. O aplicativo
/// traduz os rótulos.
#[derive(Clone, Debug, PartialEq)]
pub struct Escolha {
    pub botoes: Vec<u32>,
    pub icone: u32,
    pub padrao: usize,
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
    /// Botões já escolhidos nas perguntas de vários botões (`app.choose`, o `MsgBox` com
    /// Abortar/Repetir/Ignorar…), pelo índice, na ordem em que a macro as faz.
    pub escolhas: Vec<u32>,
    /// Textos já digitados nas perguntas de `prompt` (o `InputBox`), na ordem; `None` = Cancelar.
    pub textos: Vec<Option<String>>,
    /// Separador decimal da língua do app; `None` é a vírgula (pt-BR, o padrão do app).
    pub separador_decimal: Option<char>,
    /// O que foi preenchido no formulário cujo botão rodou a macro (`valores`).
    pub valores: Option<serde_json::Map<String, serde_json::Value>>,
    /// Prazo e sinal de parar (o app roda a macro do botão noutra thread, com mais tempo).
    pub limites: feather_macros::Limites,
}

/// O que a execução acumula para entregar ao aplicativo no fim.
#[derive(Debug, Default)]
pub(crate) struct Acumulado {
    pub contexto: Contexto,
    pub perguntas_feitas: usize,
    pub pergunta: Option<String>,
    pub escolhas_feitas: usize,
    /// A pergunta pendente é de vários botões: os botões, o ícone e o botão padrão.
    pub escolha: Option<Escolha>,
    pub textos_feitos: usize,
    /// A pergunta pendente pede um texto: o valor que vem escrito.
    pub entrada: Option<String>,
    pub mensagens: Vec<String>,
    /// Aplicados só se a macro terminar bem (o desfazer das abas não os cobre).
    pub nomes: Vec<(String, Rect)>,
    pub ativou: Option<usize>,
    pub arquivos: Vec<PedidoArquivo>,
    /// `sheet.showComponent(nome, dados)`: o último pedido vale.
    pub formulario: Option<(String, serde_json::Value)>,
    /// `sheet.field(nome).value/enabled/visible = …`: (campo, propriedade, valor), na ordem.
    pub campos: Vec<(String, String, serde_json::Value)>,
    /// `sheet.hideComponent()`.
    pub fechar_formulario: bool,
    /// `app.shell(comando)`: o app pergunta à pessoa e só então roda.
    pub programas: Vec<String>,
    pub protecoes: Vec<PedidoProtecao>,
    /// `sheet.showGridlines(on)`: (aba, ligada), o último por aba vale.
    pub grades: Vec<(u32, bool)>,
    /// `app.openUrl(link)`: o app confere (só web/mailto) e abre no navegador.
    pub links: Vec<String>,
    pub filtros: Vec<PedidoFiltro>,
    pub formas: Vec<PedidoForma>,
    pub imagens: Vec<PedidoImagem>,
}

/// Mais que isto numa execução é laço descontrolado, não exportação.
const LIMITE_ARQUIVOS: usize = 200;

/// Cada comando é uma linha na pergunta "Rodar?": mais que isto a pessoa não lê.
const LIMITE_PROGRAMAS: usize = 20;

const LIMITE_TEXTO: usize = 4096;

/// As operações desta parte (consultadas antes de montar a lista de abas).
pub(crate) const OPERACOES: &[&str] = &["alerta", "confirmar", "ativar", "abaAtiva",
    "selecaoInicial", "definirNome", "arquivo", "exportarPdf", "salvarAbas", "programa", "protecao", "grade", "abrirLink", "filtro", "formaPosicao", "escolher", "perguntarTexto", "separadorDecimal", "colarImagem", "tamanhoImagem"];

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
        let aba = dados[0].as_u64().and_then(|id| ids.iter().position(|&x| u64::from(x) == id))
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
        "colarImagem" => {
            let id = |v: &Json| v.as_u64().and_then(|id| ids.iter().copied().find(|&x| u64::from(x) == id));
            let (Some(destino), Some(origem)) = (id(&dados[0]), id(&dados[1])) else { return Some(Err("Aba inexistente".into())) };
            let end = texto(dados, 2);
            let Some(area) = parse_range_ref(end.trim().trim_start_matches('=')).map(|r| r.normalized()).filter(|r| r.sheet.is_none()) else {
                return Some(Err(format!("pastePicture: intervalo inválido: {end}")));
            };
            let cel = texto(dados, 3);
            let Some(c) = parse_range_ref(cel.trim()).map(|r| r.normalized()) else { return Some(Err(format!("pastePicture: célula inválida: {cel}"))) };
            if a.imagens.len() >= LIMITE_ARQUIVOS { return Some(Err("Imagens demais numa macro".into())); }
            a.imagens.push(PedidoImagem { origem, area, destino, celula: (c.r0, c.c0), largura: None, altura: None });
            Ok(json!(a.imagens.len() - 1))
        }
        "tamanhoImagem" => {
            let Some(p) = dados[0].as_u64().and_then(|i| a.imagens.get_mut(i as usize)) else { return Some(Err("Imagem colada inexistente".into())) };
            let Some(v) = dados[2].as_f64().filter(|v| v.is_finite() && *v >= 1.0 && *v <= 1e5) else { return Some(Err("Tamanho da imagem: número de 1 a 100000 pontos".into())) };
            if texto(dados, 1) == "height" { p.altura = Some(v) } else { p.largura = Some(v) }
            Ok(Json::Null)
        }
        "separadorDecimal" => Ok(json!(a.contexto.separador_decimal.unwrap_or(',').to_string())),
        "perguntarTexto" => {
            let n = a.textos_feitos;
            a.textos_feitos += 1;
            match a.contexto.textos.get(n) {
                Some(t) => Ok(json!(t)),
                None => {
                    // Como no confirm: a macro segue como se fosse Cancelar e roda de novo com o texto.
                    if a.pergunta.is_none() {
                        a.pergunta = Some(texto(dados, 0));
                        a.entrada = Some(texto(dados, 1));
                    }
                    Ok(Json::Null)
                }
            }
        }
        "escolher" => {
            let botoes: Vec<u32> = dados[1].as_array().map(|b| b.iter().filter_map(|x| x.as_u64())
                .filter(|&x| (1..=7).contains(&x)).map(|x| x as u32).collect()).unwrap_or_default();
            if botoes.is_empty() || botoes.len() > 3 { return Some(Err("app.choose: de 1 a 3 botões (1 a 7, os do MsgBox)".into())); }
            let n = a.escolhas_feitas;
            a.escolhas_feitas += 1;
            match a.contexto.escolhas.get(n) {
                Some(&i) if (i as usize) < botoes.len() => Ok(json!(i)),
                Some(_) => Err("app.choose: resposta fora dos botões".into()),
                None => {
                    // Como no confirm: a macro segue sem efeito e roda de novo com a resposta.
                    if a.pergunta.is_none() {
                        a.pergunta = Some(texto(dados, 0));
                        let icone = dados[2].as_u64().filter(|i| [16, 32, 48, 64].contains(i)).unwrap_or(0) as u32;
                        let padrao = dados[3].as_u64().map_or(0, |p| (p as usize).min(botoes.len() - 1));
                        a.escolha = Some(Escolha { botoes: botoes.clone(), icone, padrao });
                    }
                    Ok(json!(botoes.len() - 1))
                }
            }
        }
        "ativar" => {
            // O JavaScript manda o id da aba.
            let i = dados[0].as_u64().and_then(|id| ids.iter().position(|&x| u64::from(x) == id));
            match i {
                Some(i) => { a.ativou = Some(i); Ok(Json::Null) }
                None => Err("Aba inexistente".into()),
            }
        }
        "programa" => {
            let comando = texto(dados, 0).trim().to_string();
            if comando.is_empty() { return Some(Err("Shell: comando vazio".into())); }
            if a.programas.len() >= LIMITE_PROGRAMAS {
                return Some(Err(format!("Shell: mais de {LIMITE_PROGRAMAS} programas numa execução")));
            }
            a.programas.push(comando);
            // O VBA devolve o número da tarefa; aqui o programa só roda depois da macro.
            Ok(json!(0))
        }
        "protecao" => {
            use feathersheet_core::planilha::sheet::protecao::Permissao;
            let Some(aba) = dados[0].as_u64().and_then(|id| ids.iter().copied().find(|&x| u64::from(x) == id)) else {
                return Some(Err("Aba inexistente".into()));
            };
            if a.protecoes.len() >= LIMITE_ARQUIVOS {
                return Some(Err(format!("Limite de {LIMITE_ARQUIVOS} proteções por execução")));
            }
            let mut permitidas = Vec::new();
            for c in dados[3].as_array().into_iter().flatten().filter_map(Json::as_str) {
                match Permissao::da_chave(c) {
                    Some(p) => permitidas.push(p),
                    None => return Some(Err(format!("Permissão de proteção desconhecida: {c}"))),
                }
            }
            let senha = dados[2].as_str().filter(|s| !s.is_empty()).map(|s| s.chars().take(255).collect());
            a.protecoes.push(PedidoProtecao { aba, proteger: dados[1].as_bool().unwrap_or(true), senha, permitidas });
            Ok(Json::Null)
        }
        "abrirLink" => {
            let link = texto(dados, 0).trim().to_string();
            if link.is_empty() { return Some(Err("openUrl: link vazio".into())); }
            if a.links.len() >= LIMITE_PROGRAMAS {
                return Some(Err(format!("openUrl: mais de {LIMITE_PROGRAMAS} links numa execução")));
            }
            a.links.push(link);
            Ok(json!(true))
        }
        "grade" => {
            let Some(aba) = dados[0].as_u64().and_then(|id| ids.iter().copied().find(|&x| u64::from(x) == id)) else {
                return Some(Err("Aba inexistente".into()));
            };
            a.grades.retain(|(x, _)| *x != aba);
            a.grades.push((aba, dados[1].as_bool().unwrap_or(true)));
            Ok(Json::Null)
        }
        "filtro" => {
            let Some(aba) = dados[0].as_u64().and_then(|id| ids.iter().copied().find(|&x| u64::from(x) == id)) else {
                return Some(Err("Aba inexistente".into()));
            };
            let acao = match texto(dados, 1).as_str() {
                "tudo" => AcaoFiltro::MostrarTudo,
                "remover" => AcaoFiltro::Remover,
                _ => {
                    let end = texto(dados, 2);
                    let Some(area) = parse_range_ref(end.trim()) else {
                        return Some(Err(format!("autoFilter: intervalo inválido: {end}")));
                    };
                    let coluna = match &dados[3] {
                        Json::Null => None,
                        c => match c.as_f64().filter(|n| *n >= 1.0 && n.fract() == 0.0 && *n <= f64::from(area.c1 - area.c0 + 1)) {
                            Some(n) => Some(n as u32),
                            None => return Some(Err(format!("autoFilter: coluna {c} fora de {end}"))),
                        },
                    };
                    AcaoFiltro::Aplicar { area, coluna, criterio: Some(dados[4].clone()).filter(|c| !c.is_null()) }
                }
            };
            if a.filtros.len() >= LIMITE_ARQUIVOS { return Some(Err("Filtros demais numa macro".into())); }
            a.filtros.push(PedidoFiltro { aba, acao });
            Ok(Json::Null)
        }
        "formaPosicao" => {
            let Some(aba) = dados[0].as_u64().and_then(|id| ids.iter().copied().find(|&x| u64::from(x) == id)) else {
                return Some(Err("Aba inexistente".into()));
            };
            let p = &dados[2];
            let mut erro = None;
            let mut campo = |k: &str, minimo: f64| p.get(k).filter(|v| !v.is_null()).and_then(|v| match v.as_f64() {
                Some(n) if n.is_finite() && n >= minimo && n <= 1e7 => Some(n),
                _ => { erro = Some(format!("shape.{k}: número de {minimo} a 10000000")); None }
            });
            let pedido = PedidoForma { aba, nome: texto(dados, 1), esquerda: campo("left", 0.0), topo: campo("top", 0.0),
                largura: campo("width", 1.0), altura: campo("height", 1.0) };
            if let Some(e) = erro { return Some(Err(e)); }
            if a.formas.len() >= LIMITE_ARQUIVOS { return Some(Err("Mudanças de forma demais numa macro".into())); }
            a.formas.push(pedido);
            Ok(Json::Null)
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
