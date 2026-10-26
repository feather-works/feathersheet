//! O canal local entre duas janelas do FeatherSheet para a referência viva
//! entre arquivos (`=[Vendas.fshz]Dados!A1`). Só nesta máquina, nada de rede:
//!
//! - **transporte**: no Linux/macOS, socket Unix por processo em
//!   `$XDG_RUNTIME_DIR/feathersheet/vinculos/<pid>.sock` (pasta `0700`,
//!   socket `0600`) — a mesma pasta-mãe do MCP ao vivo
//!   (`feathersheet_mcp::ao_vivo`); no Windows, que não tem socket Unix na
//!   biblioteca padrão, TCP em `127.0.0.1` (porta sorteada) com um **token**
//!   que a primeira linha de cada conexão precisa repetir (como o MCP faz lá);
//! - **descoberta**: quem tem um arquivo aberto e salvo escreve
//!   `abertos/<hash do caminho>` com o caminho, o endereço do canal, o token e
//!   o pid; ao fechar (ou trocar de arquivo) apaga. Registro de processo que
//!   morreu é ignorado e limpo;
//! - **conversa**: uma mensagem JSON por linha. Quem cita conecta na origem e
//!   manda `{"m":"assinar","caminho":…,"areas":[{"aba","r0","c0","r1","c1"}]}`
//!   (de novo, na mesma conexão, quando as áreas mudam); a origem responde
//!   `{"m":"retrato","abas":[nomes],"celulas":[{"aba","c":[[l,c,v]…]}]}` e,
//!   depois de cada recálculo, `{"m":"valores","aba":…,"c":[[l,c,v]…]}` só
//!   com as células que mudaram (vazia = `null`).
//!
//! As threads daqui não tocam na pasta: o que chega vira uma [`Chegada`]
//! numa fila que a assinatura do Iced entrega ao `update`.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_io::externos_guardados::{celulas_de_json, celulas_para_json};
use pelicano::futures::channel::mpsc as fila_iced;
use serde_json::{json, Value as Json};

#[cfg(unix)]
type Fluxo = std::os::unix::net::UnixStream;
#[cfg(not(unix))]
type Fluxo = std::net::TcpStream;

/// Uma área de outra pasta que quem cita quer receber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaPedida {
    pub aba: String,
    pub area: Rect,
}

/// O que chega pelo canal, para o `update`.
#[derive(Debug, Clone)]
pub enum Chegada {
    /// (origem) Uma janela que cita este arquivo quer os valores destas áreas.
    Assinar { conexao: u64, caminho: String, areas: Vec<AreaPedida> },
    /// (origem) A janela que assinava fechou.
    AssinanteSaiu(u64),
    /// (destino) A origem mandou os valores das áreas pedidas e as abas que tem.
    Retrato { conexao: u64, abas: Vec<String>, celulas: Vec<(String, Vec<(u32, u32, Value)>)> },
    /// (destino) Células que mudaram na origem, depois do recálculo dela.
    Valores { conexao: u64, aba: String, celulas: Vec<(u32, u32, Value)> },
    /// (destino) A origem fechou ou a conversa caiu.
    OrigemFechou(u64),
    /// (quem digita a fórmula) A pessoa clicou numa célula de outra janela:
    /// o arquivo (caminho e nome), a aba e a referência (`A1` ou `A1:B3`).
    /// `responder`: endereço e token de quem clicou, para a fórmula voltar.
    Apontar { caminho: String, arquivo: String, aba: String, referencia: String, responder: Option<(String, String)> },
    /// (quem clicou) A fórmula como está na outra janela, para a barra fx
    /// mostrar nas duas; `None` quando a edição lá acabou.
    FormulaRemota { arquivo: String, texto: Option<String> },
    /// (quem digita a fórmula) Enter (`true`) ou Esc (`false`) na outra janela.
    ApontarFim { confirmar: bool },
}

/// Quanto se espera para escrever numa conexão antes de desistir dela.
const ESPERA_ESCRITA: Duration = Duration::from_secs(2);

// ------------------------------------------------------------------ fila para o Iced

type Fila = (Mutex<fila_iced::UnboundedSender<Chegada>>, Mutex<Option<fila_iced::UnboundedReceiver<Chegada>>>);
static FILA: OnceLock<Fila> = OnceLock::new();

fn fila() -> &'static Fila {
    FILA.get_or_init(|| {
        let (tx, rx) = fila_iced::unbounded();
        (Mutex::new(tx), Mutex::new(Some(rx)))
    })
}

fn entregar(chegada: Chegada) {
    if let Ok(tx) = fila().0.lock() {
        let _ = tx.unbounded_send(chegada);
    }
}

/// O receptor da fila, uma vez só (a assinatura do Iced fica com ele).
pub fn tirar_receptor() -> Option<fila_iced::UnboundedReceiver<Chegada>> {
    fila().1.lock().ok()?.take()
}

/// Se a fila já existe (o canal foi ligado alguma vez).
pub fn fila_criada() -> bool {
    FILA.get().is_some()
}

// ------------------------------------------------------------------ disponibilidade

/// O canal só existe no aplicativo de verdade: nos testes, na captura de tela
/// e com `FEATHERSHEET_VINCULOS=0`, a janela não escuta nem conecta (os
/// valores guardados e o "Atualizar valores" continuam valendo).
pub fn disponivel() -> bool {
    !cfg!(test)
        && std::env::var_os("FEATHERSHEET_CAPTURAR").is_none()
        && !std::env::var("FEATHERSHEET_VINCULOS").is_ok_and(|v| v == "0")
}

// ------------------------------------------------------------------ pastas e registro

/// `$XDG_RUNTIME_DIR/feathersheet` (ou uma pasta própria do usuário na
/// temporária); no Windows, a pasta local de dados do usuário.
fn pasta_base() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let usuario = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "usuario".into());
            std::env::temp_dir().join(format!("feathersheet-{usuario}"))
        });
    base.join("feathersheet")
}

fn pasta_abertos() -> PathBuf {
    pasta_base().join("abertos")
}

/// Cria a pasta só para o usuário (`0700` no Unix).
fn criar_pasta_privada(pasta: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(pasta)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(pasta_base(), std::fs::Permissions::from_mode(0o700));
        std::fs::set_permissions(pasta, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// O caminho como chave: canônico quando o arquivo existe; no Windows sem
/// diferença de maiúscula nem de barra.
pub fn chave_caminho(caminho: &Path) -> String {
    let canonico = std::fs::canonicalize(caminho).unwrap_or_else(|_| caminho.to_path_buf());
    let texto = canonico.to_string_lossy().into_owned();
    if cfg!(windows) { texto.replace('\\', "/").to_lowercase() } else { texto }
}

/// FNV-1a de 64 bits: estável entre execuções e versões (o `DefaultHasher`
/// não promete isso), e o nome do registro não pode mudar de uma versão do
/// aplicativo para outra.
fn fnv64(texto: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn arquivo_do_registro(chave: &str) -> PathBuf {
    pasta_abertos().join(format!("{:016x}", fnv64(chave)))
}

/// Uma linha do registro de abertos.
#[derive(Debug, Clone, PartialEq)]
struct Registro {
    caminho: String,
    endereco: String,
    token: String,
    pid: u32,
}

fn ler_registro(chave: &str) -> Option<Registro> {
    let texto = std::fs::read_to_string(arquivo_do_registro(chave)).ok()?;
    let j: Json = serde_json::from_str(&texto).ok()?;
    let r = Registro {
        caminho: j["caminho"].as_str()?.to_string(),
        endereco: j["endereco"].as_str()?.to_string(),
        token: j["token"].as_str().unwrap_or("").to_string(),
        pid: u32::try_from(j["pid"].as_u64()?).ok()?,
    };
    (r.caminho == chave).then_some(r)
}

/// Se o processo ainda existe (só dá para saber barato no Linux; nos
/// outros, supõe que sim e deixa a conexão dizer).
fn processo_vivo(pid: u32) -> bool {
    if cfg!(target_os = "linux") { Path::new(&format!("/proc/{pid}")).exists() } else { true }
}

/// Os registros escritos por esta janela (para apagar ao fechar).
fn meus_registros() -> &'static Mutex<HashSet<String>> {
    static M: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

/// Anuncia que esta janela tem o arquivo `chave` aberto. Liga o canal se
/// ainda não estava. `false` quando não deu (canal indisponível).
pub fn registrar_aberto(chave: &str) -> bool {
    let Some(servidor) = ligar() else { return false };
    if criar_pasta_privada(&pasta_abertos()).is_err() {
        return false;
    }
    let corpo = json!({
        "caminho": chave, "endereco": servidor.endereco, "token": servidor.token, "pid": std::process::id(),
    });
    let destino = arquivo_do_registro(chave);
    if std::fs::write(&destino, corpo.to_string()).is_err() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(0o600));
    }
    if let Ok(mut meus) = meus_registros().lock() {
        meus.insert(chave.to_string());
    }
    true
}

/// Tira o anúncio (a janela fechou o arquivo ou salvou com outro nome). Só
/// apaga o registro se ele ainda é desta janela.
pub fn tirar_registro(chave: &str) {
    if ler_registro(chave).is_some_and(|r| r.pid == std::process::id()) {
        let _ = std::fs::remove_file(arquivo_do_registro(chave));
    }
    if let Ok(mut meus) = meus_registros().lock() {
        meus.remove(chave);
    }
}

// ------------------------------------------------------------------ servidor (lado da origem)

#[derive(Clone)]
struct Servidor {
    endereco: String,
    token: String,
}

static SERVIDOR: OnceLock<Mutex<Option<Servidor>>> = OnceLock::new();
static PROXIMA: AtomicU64 = AtomicU64::new(1);

/// As pontas de escrita de todas as conversas abertas: as dos assinantes
/// (lado da origem) e as das origens (lado de quem cita).
fn conexoes() -> &'static Mutex<HashMap<u64, Fluxo>> {
    static C: OnceLock<Mutex<HashMap<u64, Fluxo>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Liga o canal desta janela (uma vez). `None` se indisponível ou falhou.
fn ligar() -> Option<Servidor> {
    if !disponivel() {
        return None;
    }
    // A fila nasce antes da primeira conexão: a assinatura do Iced a pega.
    let _ = fila();
    let mut guarda = SERVIDOR.get_or_init(|| Mutex::new(None)).lock().ok()?;
    if let Some(s) = guarda.as_ref() {
        return Some(s.clone());
    }
    let servidor = escutar().ok()?;
    *guarda = Some(servidor.clone());
    Some(servidor)
}

#[cfg(unix)]
fn caminho_do_socket() -> PathBuf {
    pasta_base().join("vinculos").join(format!("{}.sock", std::process::id()))
}

#[cfg(unix)]
fn escutar() -> std::io::Result<Servidor> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    let caminho = caminho_do_socket();
    if let Some(pasta) = caminho.parent() {
        criar_pasta_privada(pasta)?;
    }
    let _ = std::fs::remove_file(&caminho);
    let ouvinte = UnixListener::bind(&caminho)?;
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new().name("vinculos-escuta".into()).spawn(move || {
        for conexao in ouvinte.incoming().flatten() {
            let _ = std::thread::Builder::new().name("vinculos-assinante".into()).spawn(move || {
                let Ok(leitura) = conexao.try_clone() else { return };
                atender_assinante(conexao, BufReader::new(leitura));
            });
        }
    })?;
    Ok(Servidor { endereco: caminho.to_string_lossy().into_owned(), token: String::new() })
}

#[cfg(not(unix))]
fn token_novo() -> String {
    use std::hash::{BuildHasher, Hasher};
    let parte = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.write_u32(std::process::id());
        h.finish()
    };
    format!("{:016x}{:016x}", parte(), parte())
}

#[cfg(not(unix))]
fn escutar() -> std::io::Result<Servidor> {
    use std::net::{Ipv4Addr, TcpListener};
    let ouvinte = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let porta = ouvinte.local_addr()?.port();
    let token = token_novo();
    let esperado = token.clone();
    std::thread::Builder::new().name("vinculos-escuta".into()).spawn(move || {
        for conexao in ouvinte.incoming().flatten() {
            let esperado = esperado.clone();
            let _ = std::thread::Builder::new().name("vinculos-assinante".into()).spawn(move || {
                let Ok(leitura) = conexao.try_clone() else { return };
                let mut leitura = BufReader::new(leitura);
                let mut primeira = String::new();
                if leitura.read_line(&mut primeira).is_err() || primeira.trim() != esperado {
                    return; // quem não sabe o token (outro programa na máquina) é largado
                }
                atender_assinante(conexao, leitura);
            });
        }
    })?;
    Ok(Servidor { endereco: format!("127.0.0.1:{porta}"), token })
}

/// Uma área na forma da conversa.
fn area_para_json(a: &AreaPedida) -> Json {
    json!({ "aba": a.aba, "r0": a.area.r0, "c0": a.area.c0, "r1": a.area.r1, "c1": a.area.c1 })
}

fn areas_de_json(j: &Json) -> Vec<AreaPedida> {
    j.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let n = |k: &str| a[k].as_u64().and_then(|v| u32::try_from(v).ok());
            Some(AreaPedida { aba: a["aba"].as_str()?.to_string(), area: Rect::new(n("r0")?, n("c0")?, n("r1")?, n("c1")?) })
        })
        .collect()
}

/// A mensagem de assinatura (quem cita → origem).
pub fn mensagem_assinar(caminho: &str, areas: &[AreaPedida]) -> Json {
    json!({ "m": "assinar", "caminho": caminho, "areas": areas.iter().map(area_para_json).collect::<Vec<_>>() })
}

/// A resposta à assinatura (origem → quem cita).
pub fn mensagem_retrato(abas: &[String], celulas: &[(String, Vec<(u32, u32, Value)>)]) -> Json {
    json!({
        "m": "retrato",
        "abas": abas,
        "celulas": celulas.iter().map(|(aba, c)| json!({ "aba": aba, "c": celulas_para_json(c) })).collect::<Vec<_>>(),
    })
}

/// As células que mudaram numa aba (origem → quem cita).
pub fn mensagem_valores(aba: &str, celulas: &[(u32, u32, Value)]) -> Json {
    json!({ "m": "valores", "aba": aba, "c": celulas_para_json(celulas) })
}

/// Lê uma linha da origem e diz o que ela é, para a conexão `conexao`.
pub fn interpretar_da_origem(conexao: u64, msg: &Json) -> Option<Chegada> {
    match msg["m"].as_str()? {
        "retrato" => Some(Chegada::Retrato {
            conexao,
            abas: msg["abas"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect(),
            celulas: msg["celulas"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|b| Some((b["aba"].as_str()?.to_string(), celulas_de_json(&b["c"]))))
                .collect(),
        }),
        "valores" => Some(Chegada::Valores { conexao, aba: msg["aba"].as_str()?.to_string(), celulas: celulas_de_json(&msg["c"]) }),
        _ => None,
    }
}

/// Lê uma linha de quem cita (ou de quem aponta) e diz o que ela é, para a
/// conexão `conexao`.
pub fn interpretar_do_assinante(conexao: u64, msg: &Json) -> Option<Chegada> {
    let texto = |k: &str| msg[k].as_str().unwrap_or("").to_string();
    match msg["m"].as_str()? {
        "assinar" => Some(Chegada::Assinar { conexao, caminho: texto("caminho"), areas: areas_de_json(&msg["areas"]) }),
        "apontar" => Some(Chegada::Apontar {
            caminho: texto("caminho"), arquivo: texto("arquivo"), aba: texto("aba"), referencia: texto("ref"),
            responder: msg["responder"]["endereco"].as_str()
                .map(|e| (e.to_string(), msg["responder"]["token"].as_str().unwrap_or("").to_string())),
        }),
        "formula_remota" => Some(Chegada::FormulaRemota {
            arquivo: texto("arquivo"), texto: msg["texto"].as_str().map(str::to_string),
        }),
        "apontar_fim" => Some(Chegada::ApontarFim { confirmar: msg["confirmar"].as_bool().unwrap_or(false) }),
        _ => None,
    }
}

/// A célula clicada noutra janela, para quem digita a fórmula.
pub fn mensagem_apontar(caminho: &str, arquivo: &str, aba: &str, referencia: &str) -> Json {
    let mut m = json!({ "m": "apontar", "caminho": caminho, "arquivo": arquivo, "aba": aba, "ref": referencia });
    if let Some(s) = ligar() {
        m["responder"] = json!({ "endereco": s.endereco, "token": s.token });
    }
    m
}

/// A fórmula em edição (ou `None`: acabou), de volta a quem clicou.
pub fn mensagem_formula_remota(arquivo: &str, texto: Option<&str>) -> Json {
    json!({ "m": "formula_remota", "arquivo": arquivo, "texto": texto })
}

/// Mensagem avulsa a um endereço do canal (conecta, escreve, fecha).
pub fn mandar_para(endereco: &str, token: &str, msg: &Json) -> bool {
    let r = Registro { caminho: String::new(), endereco: endereco.to_string(), token: token.to_string(), pid: 0 };
    let Some(mut fluxo) = abrir(&r) else { return false };
    let _ = fluxo.set_write_timeout(Some(ESPERA_ESCRITA));
    writeln!(fluxo, "{msg}").and_then(|()| fluxo.flush()).is_ok()
}

/// Enter (`confirmar`) ou Esc na janela clicada.
pub fn mensagem_apontar_fim(confirmar: bool) -> Json {
    json!({ "m": "apontar_fim", "confirmar": confirmar })
}

// ------------------------------------------------------------------ apontar em outra janela

fn arquivo_apontando() -> PathBuf {
    pasta_base().join("apontando")
}

/// Esta janela está digitando uma fórmula e aceita referência clicada noutra
/// janela (`true`), ou deixou de aceitar. Um aviso só na máquina: a última
/// janela que começou a apontar é a que recebe, como no Excel.
pub fn anunciar_apontar(ligado: bool) {
    if !ligado {
        if ler_apontando().is_some_and(|r| r.pid == std::process::id()) {
            let _ = std::fs::remove_file(arquivo_apontando());
        }
        return;
    }
    let Some(servidor) = ligar() else { return };
    if criar_pasta_privada(&pasta_base()).is_err() {
        return;
    }
    let corpo = json!({ "caminho": "", "endereco": servidor.endereco, "token": servidor.token, "pid": std::process::id() });
    let destino = arquivo_apontando();
    if std::fs::write(&destino, corpo.to_string()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(0o600));
        }
    }
}

fn ler_apontando() -> Option<Registro> {
    let texto = std::fs::read_to_string(arquivo_apontando()).ok()?;
    let j: Json = serde_json::from_str(&texto).ok()?;
    Some(Registro {
        caminho: String::new(),
        endereco: j["endereco"].as_str()?.to_string(),
        token: j["token"].as_str().unwrap_or("").to_string(),
        pid: u32::try_from(j["pid"].as_u64()?).ok()?,
    })
}

/// O pid da OUTRA janela que está digitando uma fórmula agora, se houver.
pub fn quem_aponta() -> Option<u32> {
    if !disponivel() {
        return None;
    }
    let r = ler_apontando()?;
    if r.pid == std::process::id() {
        return None;
    }
    if !processo_vivo(r.pid) {
        let _ = std::fs::remove_file(arquivo_apontando());
        return None;
    }
    Some(r.pid)
}

/// Manda uma mensagem avulsa à janela que está apontando (conecta, escreve,
/// fecha). `false` se ela não está mais lá.
pub fn mandar_para_quem_aponta(msg: &Json) -> bool {
    let Some(r) = ler_apontando().filter(|r| r.pid != std::process::id()) else { return false };
    let Some(mut fluxo) = abrir(&r) else { return false };
    let _ = fluxo.set_write_timeout(Some(ESPERA_ESCRITA));
    writeln!(fluxo, "{msg}").and_then(|()| fluxo.flush()).is_ok()
}

fn atender_assinante(escrita: Fluxo, leitura: impl BufRead) {
    let id = PROXIMA.fetch_add(1, Ordering::Relaxed);
    let _ = escrita.set_write_timeout(Some(ESPERA_ESCRITA));
    if let Ok(mut c) = conexoes().lock() {
        c.insert(id, escrita);
    }
    for linha in leitura.lines() {
        let Ok(linha) = linha else { break };
        let Ok(msg) = serde_json::from_str::<Json>(&linha) else { continue };
        if let Some(chegada) = interpretar_do_assinante(id, &msg) {
            entregar(chegada);
        }
    }
    if let Ok(mut c) = conexoes().lock() {
        c.remove(&id);
    }
    entregar(Chegada::AssinanteSaiu(id));
}

/// Manda uma mensagem numa conversa aberta. `false` (e a conversa é
/// fechada) se a outra ponta não recebe mais.
pub fn enviar(conexao: u64, msg: &Json) -> bool {
    let Ok(mut c) = conexoes().lock() else { return false };
    let Some(fluxo) = c.get_mut(&conexao) else { return false };
    let linha = format!("{msg}\n");
    if fluxo.write_all(linha.as_bytes()).and_then(|()| fluxo.flush()).is_ok() {
        return true;
    }
    if let Some(f) = c.remove(&conexao) {
        let _ = f.shutdown(std::net::Shutdown::Both);
    }
    false
}

/// Fecha uma conversa (a origem deixou de ser citada, ou a janela fecha).
pub fn desligar(conexao: u64) {
    let fluxo = conexoes().lock().ok().and_then(|mut c| c.remove(&conexao));
    if let Some(f) = fluxo {
        let _ = f.shutdown(std::net::Shutdown::Both);
    }
}

// ------------------------------------------------------------------ cliente (lado de quem cita)

#[cfg(unix)]
fn abrir(registro: &Registro) -> Option<Fluxo> {
    Fluxo::connect(&registro.endereco).ok()
}

#[cfg(not(unix))]
fn abrir(registro: &Registro) -> Option<Fluxo> {
    let endereco: std::net::SocketAddr = registro.endereco.parse().ok()?;
    if !endereco.ip().is_loopback() {
        return None; // o canal é só desta máquina
    }
    let mut s = Fluxo::connect_timeout(&endereco, ESPERA_ESCRITA).ok()?;
    writeln!(s, "{}", registro.token).ok()?;
    Some(s)
}

/// Conecta na janela que tem `chave` aberto e assina estas áreas. A resposta
/// (e o que mudar depois) chega pela fila. `None` se ninguém tem o arquivo
/// aberto ou a conexão falhou.
pub fn conectar_origem(chave: &str, areas: &[AreaPedida]) -> Option<u64> {
    if !disponivel() {
        return None;
    }
    let _ = fila();
    let registro = ler_registro(chave)?;
    if registro.pid == std::process::id() {
        return None; // a própria janela: a fórmula que cita o próprio arquivo não é vínculo
    }
    let Some(mut fluxo) = abrir(&registro) else {
        if !processo_vivo(registro.pid) {
            let _ = std::fs::remove_file(arquivo_do_registro(chave));
        }
        return None;
    };
    let _ = fluxo.set_write_timeout(Some(ESPERA_ESCRITA));
    let leitura = fluxo.try_clone().ok()?;
    writeln!(fluxo, "{}", mensagem_assinar(chave, areas)).ok()?;
    let id = PROXIMA.fetch_add(1, Ordering::Relaxed);
    conexoes().lock().ok()?.insert(id, fluxo);
    let lancou = std::thread::Builder::new().name("vinculos-origem".into()).spawn(move || {
        for linha in BufReader::new(leitura).lines() {
            let Ok(linha) = linha else { break };
            let Ok(msg) = serde_json::from_str::<Json>(&linha) else { continue };
            if let Some(chegada) = interpretar_da_origem(id, &msg) {
                entregar(chegada);
            }
        }
        if let Ok(mut c) = conexoes().lock() {
            c.remove(&id);
        }
        entregar(Chegada::OrigemFechou(id));
    });
    if lancou.is_err() {
        desligar(id);
        return None;
    }
    Some(id)
}

/// Fecha tudo ao sair: as conversas, os registros e o socket.
pub fn encerrar() {
    anunciar_apontar(false);
    let chaves: Vec<String> = meus_registros().lock().map(|m| m.iter().cloned().collect()).unwrap_or_default();
    for chave in chaves {
        tirar_registro(&chave);
    }
    let ids: Vec<u64> = conexoes().lock().map(|c| c.keys().copied().collect()).unwrap_or_default();
    for id in ids {
        desligar(id);
    }
    #[cfg(unix)]
    {
        if SERVIDOR.get().and_then(|m| m.lock().ok()).is_some_and(|g| g.is_some()) {
            let _ = std::fs::remove_file(caminho_do_socket());
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mensagens_vao_e_voltam_sem_socket() {
        let areas = vec![AreaPedida { aba: "Dados".into(), area: Rect::new(0, 0, 9, 1) }];
        let assinar = mensagem_assinar("/x/Vendas.fshz", &areas);
        match interpretar_do_assinante(3, &assinar) {
            Some(Chegada::Assinar { conexao, caminho, areas: lidas }) => {
                assert_eq!((conexao, caminho.as_str()), (3, "/x/Vendas.fshz"));
                assert_eq!(lidas, areas);
            }
            outra => panic!("{outra:?}"),
        }
        let retrato = mensagem_retrato(&["Dados".into(), "Resumo".into()], &[("Dados".into(), vec![(0, 0, Value::Number(2.0))])]);
        match interpretar_da_origem(4, &retrato) {
            Some(Chegada::Retrato { conexao: 4, abas, celulas }) => {
                assert_eq!(abas, vec!["Dados".to_string(), "Resumo".to_string()]);
                assert_eq!(celulas, vec![("Dados".to_string(), vec![(0, 0, Value::Number(2.0))])]);
            }
            outra => panic!("{outra:?}"),
        }
        let valores = mensagem_valores("Dados", &[(1, 0, Value::Empty), (2, 0, Value::Text("x".into()))]);
        match interpretar_da_origem(4, &valores) {
            Some(Chegada::Valores { aba, celulas, .. }) => {
                assert_eq!(aba, "Dados");
                assert_eq!(celulas, vec![(1, 0, Value::Empty), (2, 0, Value::Text("x".into()))]);
            }
            outra => panic!("{outra:?}"),
        }
        assert!(interpretar_da_origem(1, &json!({ "m": "outra" })).is_none());
    }

    #[test]
    fn registro_tem_nome_estavel_e_canal_fica_desligado_nos_testes() {
        assert_eq!(fnv64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(arquivo_do_registro("/x/a.fshz"), arquivo_do_registro("/x/a.fshz"));
        assert_ne!(arquivo_do_registro("/x/a.fshz"), arquivo_do_registro("/x/b.fshz"));
        assert!(!disponivel());
        assert!(!registrar_aberto("/x/a.fshz"));
        assert!(conectar_origem("/x/a.fshz", &[]).is_none());
    }
}
