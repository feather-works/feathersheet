//! Procurar atualizações — só quando o usuário clica (Configurações → Sobre).
//!
//! Privacidade: a consulta só **lê** do GitHub qual é a versão mais nova
//! (`api.github.com/repos/feather-works/feathersheet/releases/latest`); nada da
//! pessoa ou das planilhas é enviado. Quem consulta e baixa é o `curl` do próprio
//! sistema (Linux, macOS e Windows 10/11 já trazem), sem biblioteca de rede no app.
//!
//! Instalar acontece **à vista**: o app grava um roteiro comentado e abre o
//! terminal do sistema com ele. A pessoa vê cada passo (baixar, conferir o SHA-256
//! contra o `SHA256SUMS` publicado, instalar por cima) e, no `.deb`, digita a
//! senha no próprio terminal — o app nunca toca nela.

use std::path::Path;
use std::process::Command;

pub const REPOSITORIO: &str = "feather-works/feathersheet";
pub const PAGINA_VERSOES: &str = "https://github.com/feather-works/feathersheet/releases";

/// Versão deste executável.
pub fn versao_atual() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Uma versão publicada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lancamento {
    /// Sem o `v` da tag (`0.0.3`).
    pub versao: String,
    /// O que mudou (texto do release).
    pub notas: String,
    /// Página do release no GitHub.
    pub pagina: String,
}

/// Como o FeatherSheet foi instalado nesta máquina (decide como atualizar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instalacao {
    /// Pacote `.deb` (`/usr/bin/feathersheet`): `sudo apt install` por cima.
    Deb,
    /// `.tar.gz` com `instalar.sh` (`~/.local/bin`): troca sem senha.
    PastaDoUsuario,
    /// Instalador do Windows (`%LOCALAPPDATA%\Programs\FeatherSheet`).
    Windows,
    /// `FeatherSheet.app` no macOS.
    Mac,
    /// Compilado à mão ou copiado para outro lugar: só o link da página de versões.
    Desconhecida,
}

impl Instalacao {
    /// Nome do arquivo publicado no release para esta instalação.
    pub fn arquivo(self) -> Option<&'static str> {
        match self {
            Self::Deb => Some("feathersheet-linux-amd64.deb"),
            Self::PastaDoUsuario => Some("feathersheet-linux-x86_64.tar.gz"),
            Self::Windows => Some("FeatherSheet-windows-x64.exe"),
            Self::Mac => Some("FeatherSheet-macos.dmg"),
            Self::Desconhecida => None,
        }
    }

    /// Frase curta para a tela: como esta cópia foi instalada.
    pub fn descricao(self) -> &'static str {
        match self {
            Self::Deb => tr!("Instalado pelo pacote .deb"),
            Self::PastaDoUsuario => tr!("Instalado na pasta do usuário (.tar.gz)"),
            Self::Windows => tr!("Instalado pelo instalador do Windows"),
            Self::Mac => tr!("Instalado em Aplicativos (macOS)"),
            Self::Desconhecida => tr!("Cópia compilada ou fora da pasta de instalação"),
        }
    }

    /// Se a atualização pede a senha de administrador.
    pub fn pede_senha(self) -> bool {
        matches!(self, Self::Deb)
    }
}

/// Descobre a instalação pelo caminho do executável (`casa` = pasta do usuário).
/// Só compara texto, para valer igual em qualquer sistema (e nos testes).
pub fn instalacao_de(exe: &Path, casa: Option<&Path>) -> Instalacao {
    let texto = exe.to_string_lossy().replace('\\', "/");
    let minusculo = texto.to_lowercase();
    if minusculo.contains("/programs/feathersheet/") && minusculo.ends_with(".exe") {
        return Instalacao::Windows;
    }
    if texto.contains(".app/Contents/MacOS/") {
        return Instalacao::Mac;
    }
    if texto == "/usr/bin/feathersheet" {
        return Instalacao::Deb;
    }
    if let Some(casa) = casa {
        let local = casa.join(".local/bin/feathersheet");
        if exe == local {
            return Instalacao::PastaDoUsuario;
        }
    }
    Instalacao::Desconhecida
}

/// A instalação deste executável.
pub fn instalacao_atual() -> Instalacao {
    let exe = std::env::current_exe().ok();
    let casa = std::env::var_os("HOME").map(std::path::PathBuf::from);
    exe.map_or(Instalacao::Desconhecida, |e| instalacao_de(&e, casa.as_deref()))
}

/// `"v0.1.10"` → `(0, 1, 10)`; aceita com ou sem `v` e ignora sufixos (`-alpha`).
fn numeros(versao: &str) -> Option<(u32, u32, u32)> {
    let v = versao.trim().trim_start_matches(['v', 'V']);
    let v = v.split(['-', '+']).next()?;
    let mut p = v.split('.').map(|x| x.parse::<u32>().ok());
    Some((p.next()??, p.next().flatten().unwrap_or(0), p.next().flatten().unwrap_or(0)))
}

/// `outra` é mais nova que `atual`?
pub fn mais_nova(atual: &str, outra: &str) -> bool {
    matches!((numeros(atual), numeros(outra)), (Some(a), Some(b)) if b > a)
}

/// Lê a resposta de `releases/latest` da API do GitHub.
pub fn ler_lancamento(json: &str) -> Option<Lancamento> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    numeros(tag)?;
    Some(Lancamento {
        versao: tag.trim_start_matches(['v', 'V']).to_owned(),
        notas: v.get("body").and_then(|b| b.as_str()).unwrap_or_default().to_owned(),
        pagina: v.get("html_url").and_then(|b| b.as_str()).unwrap_or(PAGINA_VERSOES).to_owned(),
    })
}

fn curl() -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut c = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Sem piscar uma janela de console.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Consulta a versão mais nova publicada (bloqueia; chamar fora da thread da janela).
pub fn consultar() -> Result<Lancamento, String> {
    let url = format!("https://api.github.com/repos/{REPOSITORIO}/releases/latest");
    let saida = curl()
        .args(["-fsSL", "--max-time", "20", "-H", "Accept: application/vnd.github+json", "-H"])
        .arg(format!("User-Agent: FeatherSheet/{}", versao_atual()))
        .arg(&url)
        .output()
        .map_err(|_| tr!("Não encontrei o programa curl neste sistema para consultar o GitHub.").to_owned())?;
    if !saida.status.success() {
        return Err(tr!("Não consegui falar com o GitHub. Confira a conexão e tente de novo.").to_owned());
    }
    ler_lancamento(&String::from_utf8_lossy(&saida.stdout))
        .ok_or_else(|| tr!("O GitHub respondeu, mas não reconheci a versão publicada.").to_owned())
}

fn url_arquivo(versao: &str, arquivo: &str) -> String {
    format!("https://github.com/{REPOSITORIO}/releases/download/v{versao}/{arquivo}")
}

/// Roteiro comentado que o terminal executa: (nome do arquivo, conteúdo).
/// `pid` = este processo (Windows e macOS esperam o app fechar antes de trocar).
pub fn roteiro(inst: Instalacao, versao: &str, pid: u32) -> Option<(String, String)> {
    let arquivo = inst.arquivo()?;
    let url = url_arquivo(versao, arquivo);
    let somas = url_arquivo(versao, "SHA256SUMS");
    let cabecalho = |c: &str| {
        format!(
            "{c} Atualização do FeatherSheet para a versão {versao}.\n\
             {c} Tudo o que acontece está neste arquivo: só baixa do GitHub\n\
             {c} (github.com/{REPOSITORIO}), confere e instala por cima.\n\
             {c} Nada sobre você ou suas planilhas é enviado.\n"
        )
    };
    let unix = |passos: &str| {
        format!(
            "#!/bin/bash\n{}\npassos() {{\n  set -e\n  pasta=\"$(mktemp -d)\"\n  cd \"$pasta\"\n\
             \n  echo \"== Baixando a versão {versao} do GitHub\"\n  curl -fL --progress-bar -o {arquivo} {url}\n  curl -fsSL -o SHA256SUMS {somas}\n\
             \n  echo \"== Conferindo se o arquivo é exatamente o publicado\"\n  grep \" {arquivo}\\$\" SHA256SUMS | {soma} -c -\n\
             {passos}}}\n\
             \nif passos; then\n  echo\n  echo \"Pronto: FeatherSheet {versao} instalado.\"\nelse\n  echo\n  echo \"Algo deu errado; a versão instalada continua a mesma.\"\nfi\nread -r -p \"Pressione Enter para fechar esta janela. \" _\n",
            cabecalho("#"),
            soma = if inst == Instalacao::Mac { "shasum -a 256" } else { "sha256sum" },
        )
    };
    Some(match inst {
        Instalacao::Deb => ("feathersheet-atualizar.sh".into(), unix(
            "\n  echo \"== Instalando por cima da versão atual (o apt pede sua senha)\"\n  sudo apt install -y \"./{a}\"\n  echo \"Feche o FeatherSheet e abra de novo para usar a versão nova.\"\n"
                .replace("{a}", arquivo).as_str(),
        )),
        Instalacao::PastaDoUsuario => ("feathersheet-atualizar.sh".into(), unix(
            &format!("\n  echo \"== Trocando os arquivos em ~/.local (sem senha)\"\n  tar xzf {arquivo}\n  \"./feathersheet-{versao}/instalar.sh\"\n  echo \"Feche o FeatherSheet e abra de novo para usar a versão nova.\"\n"),
        )),
        Instalacao::Mac => ("feathersheet-atualizar.command".into(), unix(
            &format!(
                "\n  echo \"== Esperando o FeatherSheet fechar (feche a janela dele)\"\n  while kill -0 {pid} 2>/dev/null; do sleep 1; done\n\
                 \n  echo \"== Copiando para Aplicativos\"\n  ponto=\"$(mktemp -d)\"\n  hdiutil attach -nobrowse -quiet -mountpoint \"$ponto\" {arquivo}\n  rm -rf /Applications/FeatherSheet.app\n  cp -R \"$ponto/FeatherSheet.app\" /Applications/\n  hdiutil detach -quiet \"$ponto\"\n  open /Applications/FeatherSheet.app\n"
            ),
        )),
        Instalacao::Windows => ("feathersheet-atualizar.ps1".into(), format!(
            "{}\n$ErrorActionPreference = 'Stop'\ntry {{\n\
             \x20 $pasta = Join-Path $env:TEMP 'feathersheet-{versao}'\n  New-Item -ItemType Directory -Force $pasta | Out-Null\n\
             \x20 $instalador = Join-Path $pasta '{arquivo}'\n\
             \n  Write-Host '== Baixando a versão {versao} do GitHub'\n  Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile $instalador\n  Invoke-WebRequest -UseBasicParsing -Uri '{somas}' -OutFile (Join-Path $pasta 'SHA256SUMS')\n\
             \n  Write-Host '== Conferindo se o arquivo é exatamente o publicado'\n  $linha = Select-String -Path (Join-Path $pasta 'SHA256SUMS') -Pattern ' {arquivo}$' | Select-Object -First 1\n  $esperado = $linha.Line.Split(' ')[0].ToLower()\n  $obtido = (Get-FileHash $instalador -Algorithm SHA256).Hash.ToLower()\n  if ($obtido -ne $esperado) {{ throw 'O arquivo baixado não confere com o publicado.' }}\n\
             \n  Write-Host '== Esperando o FeatherSheet fechar (feche a janela dele)'\n  Wait-Process -Id {pid} -ErrorAction SilentlyContinue\n\
             \n  Write-Host '== Instalando por cima da versão atual'\n  Start-Process -FilePath $instalador -ArgumentList '/S' -Wait\n  Start-Process (Join-Path $env:LOCALAPPDATA 'Programs\\FeatherSheet\\feathersheet.exe')\n  Write-Host ''\n  Write-Host 'Pronto: FeatherSheet {versao} instalado.'\n\
             }} catch {{\n  Write-Host ''\n  Write-Host \"Algo deu errado; a versão instalada continua a mesma. ($_)\"\n}}\nRead-Host 'Pressione Enter para fechar esta janela'\n",
            cabecalho("#"),
        )),
        Instalacao::Desconhecida => return None,
    })
}

/// Grava o roteiro na pasta temporária e abre o terminal do sistema com ele.
pub fn abrir_no_terminal(inst: Instalacao, versao: &str) -> Result<(), String> {
    let (nome, texto) = roteiro(inst, versao, std::process::id())
        .ok_or_else(|| tr!("Esta cópia não foi instalada por um pacote: baixe pela página de versões.").to_owned())?;
    let caminho = std::env::temp_dir().join(nome);
    std::fs::write(&caminho, texto).map_err(|e| trf!("Não consegui gravar o roteiro: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755));
    }
    let arq = caminho.to_string_lossy().into_owned();
    match inst {
        Instalacao::Windows => {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &arq]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                // Console próprio e visível.
                c.creation_flags(0x0000_0010);
            }
            c.spawn().map(|_| ()).map_err(|e| trf!("Não consegui abrir o PowerShell: {e}"))
        }
        Instalacao::Mac => Command::new("open").args(["-a", "Terminal", &arq]).spawn().map(|_| ())
            .map_err(|e| trf!("Não consegui abrir o Terminal: {e}")),
        _ => {
            // O primeiro terminal que existir. Cada um tem o seu jeito de receber o comando.
            let tentativas: [(&str, &[&str]); 7] = [
                ("x-terminal-emulator", &["-e", "bash"]),
                ("gnome-terminal", &["--", "bash"]),
                ("kgx", &["--", "bash"]),
                ("konsole", &["-e", "bash"]),
                ("xfce4-terminal", &["-x", "bash"]),
                ("tilix", &["-e", "bash"]),
                ("xterm", &["-e", "bash"]),
            ];
            for (prog, args) in tentativas {
                if Command::new(prog).args(args).arg(&arq).spawn().is_ok() {
                    return Ok(());
                }
            }
            Err(trf!("Não achei um terminal para abrir. Rode à mão: bash {arq}"))
        }
    }
}

/// Abre um endereço no navegador padrão (página de versões).
pub fn abrir_endereco(url: &str) {
    // `rundll32` abre no programa padrão sem piscar um console (o `cmd /C
    // start` abria uma janela de cmd no Windows).
    let _ = if cfg!(windows) {
        Command::new("rundll32").arg("url.dll,FileProtocolHandler").arg(url).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
}

/// O que a seção Sobre mostra.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Estado {
    #[default]
    Parado,
    Procurando,
    EmDia,
    Disponivel(Lancamento),
    /// O terminal foi aberto com o roteiro.
    Instalando(String),
    Erro(String),
}

#[derive(Debug, Clone)]
pub enum Evento {
    Procurar,
    Resultado(Result<Lancamento, String>),
    Instalar,
    AbrirPagina,
}

use super::{App, Msg};
use pelicano::Task;

impl App {
    pub(super) fn evento_atualizacao(&mut self, e: Evento) -> Task<Msg> {
        match e {
            Evento::Procurar => {
                self.atualizacao = Estado::Procurando;
                // O curl roda fora da thread da janela.
                return Task::perform(
                    async { tokio::task::spawn_blocking(consultar).await.unwrap_or_else(|e| Err(e.to_string())) },
                    |r| Msg::Atualizacao(Evento::Resultado(r)),
                );
            }
            Evento::Resultado(Ok(l)) => {
                self.atualizacao = if mais_nova(versao_atual(), &l.versao) { Estado::Disponivel(l) } else { Estado::EmDia };
            }
            Evento::Resultado(Err(e)) => self.atualizacao = Estado::Erro(e),
            Evento::Instalar => {
                if let Estado::Disponivel(l) = &self.atualizacao {
                    let inst = instalacao_atual();
                    let versao = l.versao.clone();
                    self.atualizacao = match abrir_no_terminal(inst, &versao) {
                        Ok(()) if inst.pede_senha() => Estado::Instalando(trf!(
                            "O terminal abriu com cada passo à vista. Digite sua senha lá; depois feche e abra o FeatherSheet de novo.")),
                        Ok(()) if matches!(inst, Instalacao::Windows | Instalacao::Mac) => Estado::Instalando(
                            tr!("O terminal abriu e está baixando. Feche o FeatherSheet quando ele pedir: a versão nova abre sozinha.").to_owned()),
                        Ok(()) => Estado::Instalando(
                            tr!("O terminal abriu com cada passo à vista. Depois feche e abra o FeatherSheet de novo.").to_owned()),
                        Err(e) => Estado::Erro(e),
                    };
                }
            }
            Evento::AbrirPagina => {
                let url = match &self.atualizacao {
                    Estado::Disponivel(l) => l.pagina.clone(),
                    _ => PAGINA_VERSOES.to_owned(),
                };
                abrir_endereco(&url);
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reconhece_cada_instalacao() {
        let casa = PathBuf::from("/home/ana");
        let i = |p: &str| instalacao_de(Path::new(p), Some(&casa));
        assert_eq!(i("/usr/bin/feathersheet"), Instalacao::Deb);
        assert_eq!(i("/home/ana/.local/bin/feathersheet"), Instalacao::PastaDoUsuario);
        assert_eq!(i(r"C:\Users\Ana\AppData\Local\Programs\FeatherSheet\feathersheet.exe"), Instalacao::Windows);
        assert_eq!(i("/Applications/FeatherSheet.app/Contents/MacOS/feathersheet"), Instalacao::Mac);
        assert_eq!(i("/home/ana/GitHub/feathersheet/crates/feathersheet-gui/target/release/feathersheet"), Instalacao::Desconhecida);
    }

    #[test]
    fn compara_versoes() {
        assert!(mais_nova("0.0.2", "v0.0.3"));
        assert!(mais_nova("0.0.9", "0.1.0"));
        assert!(mais_nova("0.1.9", "0.1.10"));
        assert!(!mais_nova("0.0.3", "v0.0.3"));
        assert!(!mais_nova("0.1.0", "0.0.9"));
        assert!(!mais_nova("0.0.3", "lixo"));
        assert!(mais_nova("0.0.3", "0.0.4-alpha"));
    }

    #[test]
    fn le_a_resposta_do_github() {
        let json = r#"{"tag_name":"v0.0.3","body":"- botão de atualizar","html_url":"https://github.com/feather-works/feathersheet/releases/tag/v0.0.3","assets":[]}"#;
        let l = ler_lancamento(json).unwrap();
        assert_eq!(l.versao, "0.0.3");
        assert_eq!(l.notas, "- botão de atualizar");
        assert!(l.pagina.ends_with("/v0.0.3"));
        assert!(ler_lancamento(r#"{"message":"Not Found"}"#).is_none());
    }

    #[test]
    fn roteiros_mostram_e_conferem_antes_de_instalar() {
        let (nome, deb) = roteiro(Instalacao::Deb, "0.0.3", 42).unwrap();
        assert!(nome.ends_with(".sh"));
        let baixa = deb.find("releases/download/v0.0.3/feathersheet-linux-amd64.deb").unwrap();
        let confere = deb.find("sha256sum -c").unwrap();
        let instala = deb.find("sudo apt install").unwrap();
        assert!(baixa < confere && confere < instala, "baixar, conferir e só então instalar");
        assert!(deb.contains("Nada sobre você ou suas planilhas é enviado"));

        let (_, tar) = roteiro(Instalacao::PastaDoUsuario, "0.0.3", 42).unwrap();
        assert!(tar.contains("./feathersheet-0.0.3/instalar.sh") && !tar.contains("sudo"));

        let (nome, win) = roteiro(Instalacao::Windows, "0.0.3", 42).unwrap();
        assert!(nome.ends_with(".ps1"));
        assert!(win.contains("Get-FileHash") && win.contains("Wait-Process -Id 42") && win.contains("'/S'"));
        assert!(win.find("Get-FileHash").unwrap() < win.find("Start-Process -FilePath").unwrap());

        let (nome, mac) = roteiro(Instalacao::Mac, "0.0.3", 42).unwrap();
        assert!(nome.ends_with(".command"));
        assert!(mac.contains("shasum -a 256 -c") && mac.contains("kill -0 42") && mac.contains("/Applications/FeatherSheet.app"));

        assert!(roteiro(Instalacao::Desconhecida, "0.0.3", 42).is_none());
    }
}
