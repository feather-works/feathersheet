//! O MCP **ao vivo**: o aplicativo hospeda o motor de ferramentas num canal local (só nesta máquina, nada de rede) e o
//! `feathersheet-mcp` que o Claude Code abre vira um proxy fino: com o aplicativo aberto, cada `tools/call` vai para lá e age
//! sobre o desenho que está na tela; sem ele, o servidor trabalha sozinho. Uma mensagem JSON por linha, as mesmas do stdio.
//! O mesmo canal garante **um aplicativo só**: quem abre o FeatherSheet de novo entrega os arquivos ao que já está aberto
//! (`pedir_foco`) e sai.
//!
//! Transporte: socket Unix (`$XDG_RUNTIME_DIR/feathersheet/mcp.sock`, pasta `0700`) no Linux/macOS; no Windows, que não tem
//! socket Unix na biblioteca padrão, TCP em `127.0.0.1` (porta sorteada) com um **token** guardado num arquivo da pasta do
//! usuário — a primeira linha de cada conexão é o token.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};

/// O método da mensagem "já existe um aplicativo aberto: mostre-se e abra estes arquivos".
pub const METODO_INSTANCIA: &str = "feathersheet/instancia";

/// Quanto se espera a resposta do aplicativo que já está aberto antes de supor que ele travou.
const ESPERA_DO_FOCO: Duration = Duration::from_secs(2);

/// Manda uma linha e lê a resposta (uma linha).
fn trocar(escrita: &mut impl Write, leitura: &mut impl BufRead, linha: &str) -> std::io::Result<String> {
    writeln!(escrita, "{linha}")?;
    escrita.flush()?;
    let mut resposta = String::new();
    match leitura.read_line(&mut resposta)? {
        0 => Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "o aplicativo fechou a conexão")),
        _ => Ok(resposta.trim_end().to_string()),
    }
}

/// A mensagem de [`pedir_foco`].
fn mensagem_de_foco(arquivos: &[String]) -> String {
    json!({ "jsonrpc": "2.0", "id": 1, "method": METODO_INSTANCIA, "params": { "arquivos": arquivos } }).to_string()
}

/// Atende cada linha de uma conexão com `tratar` (uma resposta por linha, quando há).
fn atender_conexao(leitura: impl BufRead, mut escrita: impl Write, tratar: &(impl Fn(Value) -> Option<Value> + ?Sized)) {
    for linha in leitura.lines() {
        let Ok(linha) = linha else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&linha) else { continue };
        if let Some(r) = tratar(msg) {
            if writeln!(escrita, "{r}").and_then(|()| escrita.flush()).is_err() {
                break;
            }
        }
    }
}

// ------------------------------------------------------------------ transporte TCP (Windows; testado em todos)
#[cfg_attr(unix, allow(dead_code))]
mod tcp {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::Value;

    use super::{atender_conexao, mensagem_de_foco, trocar, ESPERA_DO_FOCO};

    /// O arquivo com `porta token` do aplicativo aberto (na pasta de dados do usuário).
    pub fn arquivo_da_porta() -> PathBuf {
        let base = std::env::var_os("FEATHERSHEET_PORTA_DIR").or_else(|| std::env::var_os("LOCALAPPDATA")).or_else(|| std::env::var_os("XDG_RUNTIME_DIR")).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        base.join("feathersheet").join("mcp.porta")
    }

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

    fn ler_porta() -> Option<(u16, String)> {
        let t = std::fs::read_to_string(arquivo_da_porta()).ok()?;
        let (porta, token) = t.trim().split_once(' ')?;
        Some((porta.parse().ok()?, token.to_string()))
    }

    /// Conecta no aplicativo aberto e já se identifica com o token.
    fn conectar() -> Option<(TcpStream, BufReader<TcpStream>)> {
        let (porta, token) = ler_porta()?;
        let s = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, porta)), ESPERA_DO_FOCO).ok()?;
        let leitura = BufReader::new(s.try_clone().ok()?);
        let mut escrita = s.try_clone().ok()?;
        writeln!(escrita, "{token}").ok()?;
        Some((s, leitura))
    }

    #[derive(Default)]
    pub struct Cliente {
        conexao: Option<(TcpStream, BufReader<TcpStream>)>,
    }

    impl Cliente {
        pub fn repassar(&mut self, linha: &str) -> Option<Result<String, String>> {
            if self.conexao.is_none() {
                if std::env::var("FEATHERSHEET_VIVO").is_ok_and(|v| v == "0") {
                    return None;
                }
                self.conexao = conectar();
            }
            let (escrita, leitura) = self.conexao.as_mut()?;
            let mut escrita = escrita.try_clone().ok()?;
            match trocar(&mut escrita, leitura, linha) {
                Ok(r) => Some(Ok(r)),
                Err(e) => {
                    self.conexao = None;
                    Some(Err(format!("a conversa com o aplicativo FeatherSheet caiu ({e}); confira o planilha na tela antes de repetir")))
                }
            }
        }
    }

    pub fn pedir_foco(arquivos: &[String]) -> bool {
        let Some((s, mut leitura)) = conectar() else { return false };
        let _ = s.set_read_timeout(Some(ESPERA_DO_FOCO));
        let Ok(mut escrita) = s.try_clone() else { return false };
        trocar(&mut escrita, &mut leitura, &mensagem_de_foco(arquivos)).is_ok_and(|r| serde_json::from_str::<Value>(&r).is_ok_and(|v| v["result"]["ok"] == true))
    }

    pub fn servir(tratar: impl Fn(Value) -> Option<Value> + Send + Sync + 'static) -> std::io::Result<bool> {
        if conectar().is_some() {
            return Ok(false);
        }
        let ouvinte = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let porta = ouvinte.local_addr()?.port();
        let token = token_novo();
        let arquivo = arquivo_da_porta();
        if let Some(pasta) = arquivo.parent() {
            std::fs::create_dir_all(pasta)?;
        }
        std::fs::write(&arquivo, format!("{porta} {token}"))?;
        let tratar = Arc::new(tratar);
        std::thread::spawn(move || {
            for conexao in ouvinte.incoming().flatten() {
                let (tratar, token) = (tratar.clone(), token.clone());
                std::thread::spawn(move || {
                    let Ok(leitura) = conexao.try_clone() else { return };
                    let mut leitura = BufReader::new(leitura);
                    let mut primeira = String::new();
                    if leitura.read_line(&mut primeira).is_err() || primeira.trim() != token {
                        return; // quem não sabe o token (outro programa na máquina) é largado
                    }
                    atender_conexao(leitura, conexao, &*tratar);
                });
            }
        });
        Ok(true)
    }

    pub fn encerrar() {
        let _ = std::fs::remove_file(arquivo_da_porta());
    }
}

// ------------------------------------------------------------------ transporte Unix
#[cfg(unix)]
mod unix {
    use std::io::BufReader;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::Value;

    use super::{atender_conexao, mensagem_de_foco, trocar, ESPERA_DO_FOCO};

    /// Onde o aplicativo escuta: `$XDG_RUNTIME_DIR/feathersheet/mcp.sock` (ou uma pasta própria em `/tmp`).
    pub fn caminho_do_socket() -> PathBuf {
        // Testes com duas janelas: cada uma com o seu canal, e o registro de
        // vínculos (que usa o XDG_RUNTIME_DIR) continua o mesmo.
        if let Some(proprio) = std::env::var_os("FEATHERSHEET_MCP_SOCKET") {
            return PathBuf::from(proprio);
        }
        // Sem XDG_RUNTIME_DIR: `~/.cache` (o mesmo do app, em `vinculos/canal.rs`), não o /tmp de todos.
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).unwrap_or_else(|| {
            let usuario = std::env::var("USER").unwrap_or_else(|_| "usuario".into());
            std::env::temp_dir().join(format!("feathersheet-{usuario}"))
        });
        base.join("feathersheet").join("mcp.sock")
    }

    /// O lado do `feathersheet-mcp`: conecta por demanda e repassa uma linha de cada vez.
    #[derive(Default)]
    pub struct Cliente {
        conexao: Option<(UnixStream, BufReader<UnixStream>)>,
    }

    impl Cliente {
        fn conectar(&mut self) -> bool {
            if self.conexao.is_some() {
                return true;
            }
            if std::env::var("FEATHERSHEET_VIVO").is_ok_and(|v| v == "0") {
                return false;
            }
            let Ok(s) = UnixStream::connect(caminho_do_socket()) else { return false };
            let Ok(leitura) = s.try_clone() else { return false };
            self.conexao = Some((s, BufReader::new(leitura)));
            true
        }

        /// `None` = o aplicativo não está aberto (quem chamou atende sozinho). `Some(Err)` = a conversa caiu no meio
        /// (a ferramenta pode ou não ter rodado: não se repete sozinho). `Some(Ok(linha))` = a resposta do aplicativo.
        pub fn repassar(&mut self, linha: &str) -> Option<Result<String, String>> {
            if !self.conectar() {
                return None;
            }
            let (escrita, leitura) = self.conexao.as_mut()?;
            match trocar(escrita, leitura, linha) {
                Ok(r) => Some(Ok(r)),
                Err(e) => {
                    self.conexao = None;
                    Some(Err(format!("a conversa com o aplicativo FeatherSheet caiu ({e}); confira o planilha na tela antes de repetir")))
                }
            }
        }
    }

    /// Já há um aplicativo aberto? Então ele recebe `arquivos`, se mostra, e `true` (quem chamou deve sair).
    pub fn pedir_foco(arquivos: &[String]) -> bool {
        let Ok(s) = UnixStream::connect(caminho_do_socket()) else { return false };
        let _ = s.set_read_timeout(Some(ESPERA_DO_FOCO));
        let (Ok(leitura), Ok(mut escrita)) = (s.try_clone(), s.try_clone()) else { return false };
        trocar(&mut escrita, &mut BufReader::new(leitura), &mensagem_de_foco(arquivos)).is_ok_and(|r| serde_json::from_str::<Value>(&r).is_ok_and(|v| v["result"]["ok"] == true))
    }

    /// O lado do aplicativo: escuta no socket e chama `tratar` para cada mensagem (uma thread por conexão).
    /// Se já há outro aplicativo escutando, não faz nada (devolve `Ok(false)`).
    pub fn servir(tratar: impl Fn(Value) -> Option<Value> + Send + Sync + 'static) -> std::io::Result<bool> {
        let caminho = caminho_do_socket();
        if UnixStream::connect(&caminho).is_ok() {
            return Ok(false);
        }
        if let Some(pasta) = caminho.parent() {
            std::fs::create_dir_all(pasta)?;
            std::fs::set_permissions(pasta, std::fs::Permissions::from_mode(0o700))?;
        }
        let _ = std::fs::remove_file(&caminho);
        let ouvinte = UnixListener::bind(&caminho)?;
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o600))?;
        let tratar = Arc::new(tratar);
        std::thread::spawn(move || {
            for conexao in ouvinte.incoming().flatten() {
                let tratar = tratar.clone();
                std::thread::spawn(move || {
                    let Ok(leitura) = conexao.try_clone() else { return };
                    atender_conexao(BufReader::new(leitura), conexao, &*tratar);
                });
            }
        });
        Ok(true)
    }

    /// Apaga o arquivo do socket (ao fechar o aplicativo).
    pub fn encerrar() {
        let _ = std::fs::remove_file(caminho_do_socket());
    }
}

#[cfg(unix)]
pub use unix::{caminho_do_socket, encerrar, pedir_foco, servir, Cliente};
#[cfg(not(unix))]
pub use tcp::{encerrar, pedir_foco, servir, Cliente};

/// Onde o aplicativo está escutando (para a mensagem de diagnóstico).
#[cfg(unix)]
pub fn onde_escuta() -> PathBuf {
    caminho_do_socket()
}
#[cfg(not(unix))]
pub fn onde_escuta() -> PathBuf {
    tcp::arquivo_da_porta()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn cliente_e_servidor_conversam_por_linha_e_so_um_app_vale() {
        let pasta = std::env::temp_dir().join(format!("fcad_vivo_{}", std::process::id()));
        std::env::set_var("XDG_RUNTIME_DIR", &pasta);
        assert!(servir(|m| Some(json!({ "eco": m["id"], "result": { "ok": m["method"] == METODO_INSTANCIA } }))).unwrap());
        assert!(!servir(|_| None).unwrap(), "um segundo aplicativo não toma o lugar do primeiro");
        let mut c = Cliente::default();
        let r = c.repassar(r#"{"id":7}"#).unwrap().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&r).unwrap()["eco"], 7);
        assert!(pedir_foco(&["a.dxf".into()]), "o segundo processo entrega os arquivos ao primeiro e sai");
        encerrar();
        assert!(Cliente::default().repassar("{}").is_none(), "sem aplicativo escutando, quem chamou atende sozinho");
        assert!(!pedir_foco(&[]), "sem aplicativo aberto, ninguém recebe o foco");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// O transporte do Windows, exercitado aqui no Linux: token certo conversa, token errado é largado.
    #[test]
    fn transporte_tcp_exige_o_token() {
        let pasta = std::env::temp_dir().join(format!("fcad_tcp_{}", std::process::id()));
        std::env::set_var("FEATHERSHEET_PORTA_DIR", &pasta);
        assert!(tcp::servir(|m| Some(json!({ "eco": m["id"], "result": { "ok": true } }))).unwrap());
        assert!(!tcp::servir(|_| None).unwrap());
        let mut c = tcp::Cliente::default();
        let r = c.repassar(r#"{"id":3}"#).unwrap().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&r).unwrap()["eco"], 3);
        assert!(tcp::pedir_foco(&[]));
        // um intruso que não sabe o token não recebe resposta
        let (porta, _) = std::fs::read_to_string(tcp::arquivo_da_porta()).unwrap().trim().split_once(' ').map(|(p, t)| (p.parse::<u16>().unwrap(), t.to_string())).unwrap();
        let s = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
        s.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        let (mut e, mut l) = (s.try_clone().unwrap(), std::io::BufReader::new(s));
        assert!(trocar(&mut e, &mut l, "{\"id\":1}").is_err(), "token errado: conexão largada");
        tcp::encerrar();
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
