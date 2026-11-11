//! GRI-3: abrir `.xlsx` protegido por senha. A leitura em segundo plano devolve o aviso
//! [`feathersheet_io::xlsx_senha::PEDE_SENHA`]; esta janela pede a senha e manda abrir de
//! novo. A senha vai à thread da leitura por [`SENHA`] e é tirada de lá ao usar: não fica
//! guardada no aplicativo nem sai da máquina.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// O aviso de senha errada, no mesmo canal de erro da leitura.
pub(super) const SENHA_ERRADA: &str = "Senha incorreta.";

static SENHA: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub enum Evento {
    /// A leitura pediu a senha (`true`: a que foi digitada estava errada).
    Pedir(PathBuf, bool),
    Texto(String),
    /// Revisão › Criptografar com senha: define ou tira a senha da pasta.
    Definir,
    Confirmar(String),
    Abrir,
    Fechar,
}

pub(super) struct Estado {
    caminho: PathBuf,
    senha: String,
    errou: bool,
    /// `Some`: definindo a senha da pasta (a confirmação); `None`: abrindo um arquivo.
    confirmar: Option<String>,
}

/// O arquivo em `caminho` é um `.xlsx` cifrado (lê só o começo).
pub(super) fn tem_senha(caminho: &Path) -> bool {
    use std::io::Read;
    let mut inicio = [0u8; 8];
    std::fs::File::open(caminho).and_then(|mut f| f.read_exact(&mut inicio)).is_ok()
        && feathersheet_io::xlsx_senha::eh_protegido(&inicio)
}

/// A senha digitada para `caminho`, tirada de [`SENHA`].
fn tirar(caminho: &Path) -> Option<String> {
    let mut guardada = SENHA.lock().ok()?;
    match guardada.take() {
        Some((p, s)) if p == caminho => Some(s),
        outra => { *guardada = outra; None }
    }
}

/// O `.xlsx` de `caminho`, decifrado se há senha digitada para ele; sem senha, `None`
/// (a leitura normal segue e devolve o aviso de senha).
pub(super) fn decifrado(caminho: &Path) -> Option<Result<Vec<u8>, String>> {
    let senha = tirar(caminho)?;
    // O mesmo teto do .xlsx sem senha, antes de ler: a decifração faz mais duas cópias.
    if let Err(e) = feathersheet_io::xlsx_read::conferir_tamanho_xlsx(caminho) { return Some(Err(e)); }
    let bytes = match std::fs::read(caminho) {
        Ok(b) => b,
        Err(e) => return Some(Err(trf!("Não consegui ler {}: {e}", caminho.display()))),
    };
    use feathersheet_io::xlsx_senha::{decifrar, ErroSenha};
    Some(decifrar(&bytes, &senha).map_err(|e| match e {
        ErroSenha::SenhaErrada => SENHA_ERRADA.to_string(),
        ErroSenha::NaoSuportado(como) => trf!("O arquivo usa uma proteção por senha que o FeatherSheet ainda não abre ({como})."),
        ErroSenha::Corrompido(o_que) => trf!("O arquivo com senha está danificado ({o_que})."),
    }))
}

/// A mensagem que a leitura em segundo plano manda de volta: a janela da senha quando o
/// erro é de senha; senão, o resultado de sempre.
pub(super) fn resposta(caminho: &Path, resultado: Result<Option<PastaLida>, String>) -> Msg {
    // A leitura pode ter parado antes de usar a senha (teto de memória): não fica guardada.
    let _ = tirar(caminho);
    match resultado {
        Err(e) if e == feathersheet_io::xlsx_senha::PEDE_SENHA => Msg::SenhaArquivo(Evento::Pedir(caminho.to_path_buf(), false)),
        Err(e) if e == SENHA_ERRADA => Msg::SenhaArquivo(Evento::Pedir(caminho.to_path_buf(), true)),
        outro => Msg::Aberto(outro),
    }
}

impl App {
    pub(super) fn evento_senha_arquivo(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Pedir(caminho, errou) => {
                self.carregando = false;
                self.em_tela_inicio = false;
                self.status = tr!("O arquivo está protegido por senha.").into();
                self.senha_arquivo = Some(Estado { caminho, senha: String::new(), errou, confirmar: None });
                return Task::focus(ID_SENHA);
            }
            Evento::Definir => {
                let senha = self.senha_documento.clone().unwrap_or_default();
                let caminho = self.caminho.clone().unwrap_or_default();
                self.senha_arquivo = Some(Estado { caminho, confirmar: Some(senha.clone()), senha, errou: false });
                return Task::focus(ID_SENHA);
            }
            Evento::Texto(t) => if let Some(e) = &mut self.senha_arquivo { e.senha = t },
            Evento::Confirmar(t) => if let Some(Estado { confirmar: Some(c), .. }) = &mut self.senha_arquivo { *c = t },
            Evento::Abrir => {
                let Some(mut e) = self.senha_arquivo.take() else { return Task::none() };
                if let Some(confirmacao) = &e.confirmar {
                    if *confirmacao != e.senha {
                        e.errou = true;
                        self.senha_arquivo = Some(e);
                        return Task::none();
                    }
                    let nova = (!e.senha.is_empty()).then_some(e.senha);
                    if nova != self.senha_documento { self.alterado = true; }
                    self.status = if nova.is_some() { tr!("Senha definida: o .xlsx sai cifrado ao salvar.") }
                        else { tr!("Senha removida: o próximo salvamento grava sem senha.") }.into();
                    self.senha_documento = nova;
                    return Task::none();
                }
                self.senha_tentada = Some(e.senha.clone());
                if let Ok(mut s) = SENHA.lock() { *s = Some((e.caminho.clone(), e.senha)); }
                self.carregando = true;
                self.status = trf!("Abrindo {}", e.caminho.display());
                return Self::tarefa_abrir_recente(e.caminho, self.config.separador_csv());
            }
            Evento::Fechar => {
                let abrindo = self.senha_arquivo.take().is_some_and(|e| e.confirmar.is_none());
                self.senha_tentada = None;
                if abrindo { self.status = tr!("Abertura cancelada; planilha preservada").into(); }
            }
        }
        Task::none()
    }

    pub(super) fn sobrepor_senha_arquivo<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(e) = &self.senha_arquivo else { return base };
        let nome = e.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut painel = pelicano::column(Vec::<Element<_>>::new()).spacing(10).push(
            pelicano::text_input(tr!("Senha"), &e.senha).secure(true).id(ID_SENHA)
                .on_input(|t| Msg::SenhaArquivo(Evento::Texto(t)))
                .on_submit(Msg::SenhaArquivo(Evento::Abrir)).padding(7).size(13));
        if let Some(c) = &e.confirmar {
            painel = painel.push(pelicano::text_input(tr!("Confirmar senha"), c).secure(true)
                .on_input(|t| Msg::SenhaArquivo(Evento::Confirmar(t)))
                .on_submit(Msg::SenhaArquivo(Evento::Abrir)).padding(7).size(13));
        }
        if e.errou {
            let aviso = if e.confirmar.is_some() { tr!("As senhas não conferem.") } else { tr!("Senha incorreta. Confira maiúsculas e minúsculas.") };
            painel = painel.push(row![feather_ui::icones::icone("info-circle", 15.0),
                text(aviso).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center));
        }
        let (titulo, icone, subtitulo) = if e.confirmar.is_some() {
            (tr!("Criptografar com senha"), "shield-lock", tr!("A senha protege o arquivo ao salvar em .xlsx. Deixe em branco para tirar.").to_string())
        } else {
            (tr!("Arquivo protegido por senha"), "lock", trf!("Digite a senha para abrir {nome}."))
        };
        self.dialogo(titulo, painel)
            .icone(icone)
            .subtitulo(subtitulo)
            .largura(420.0)
            .ao_fechar(Msg::SenhaArquivo(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::SenhaArquivo(Evento::Fechar)))
            .primario("OK", Some(Msg::SenhaArquivo(Evento::Abrir)))
            .sobre(base)
    }
}

const ID_SENHA: &str = "feathersheet.senha-arquivo";

#[cfg(test)]
mod testes {
    use super::*;

    const ARQUIVO: &[u8] = include_bytes!("../../../feathersheet-io/tests/arquivos/senha123_libreoffice.xlsx");
    /// Os dois testes passam a senha pela mesma [`SENHA`] global: um de cada vez.
    static UM_POR_VEZ: Mutex<()> = Mutex::new(());

    #[test]
    fn xlsx_com_senha_pede_a_senha_recusa_a_errada_e_abre_com_a_certa() {
        let _vez = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let pasta = std::env::temp_dir().join(format!("fs_senha_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("com senha.xlsx");
        std::fs::write(&caminho, ARQUIVO).unwrap();
        let ler = |c: &Path| resposta(c, App::decodificar_caminho(c, None).map(|_| None));
        assert!(matches!(ler(&caminho), Msg::SenhaArquivo(Evento::Pedir(_, false))));
        let mut app = App::default();
        let _ = app.evento_senha_arquivo(Evento::Pedir(caminho.clone(), false));
        let _ = app.evento_senha_arquivo(Evento::Texto("errada".into()));
        let _ = app.evento_senha_arquivo(Evento::Abrir);
        assert!(app.senha_arquivo.is_none());
        assert!(matches!(ler(&caminho), Msg::SenhaArquivo(Evento::Pedir(_, true))), "senha errada pede de novo");
        *SENHA.lock().unwrap() = Some((caminho.clone(), "senha123".into()));
        let (wb, _) = App::decodificar_caminho(&caminho, None).expect("abre com a senha certa");
        assert_eq!(wb.valor(0, 1, 0), feathersheet_core::celulas::values::Value::Text("Ana".into()));
        assert!(SENHA.lock().unwrap().is_none(), "a senha não fica guardada");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Aberto com senha, salva cifrado com a mesma senha e sem cópia de recuperação; a
    /// Revisão troca ou tira a senha.
    #[test]
    fn xlsx_com_senha_salva_cifrado_e_a_senha_troca_ou_sai() {
        let _vez = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        use feathersheet_io::xlsx_senha::{decifrar, ErroSenha};
        let pasta = std::env::temp_dir().join(format!("fs_senha_gravar_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let caminho = pasta.join("protegido.xlsx");
        std::fs::write(&caminho, ARQUIVO).unwrap();
        let mut app = App::default();
        let _ = app.evento_senha_arquivo(Evento::Pedir(caminho.clone(), false));
        let _ = app.evento_senha_arquivo(Evento::Texto("senha123".into()));
        let _ = app.evento_senha_arquivo(Evento::Abrir);
        let (wb, folha) = App::decodificar_caminho(&caminho, None).expect("abre com a senha digitada");
        app.instalar(caminho.clone(), wb, folha);
        assert_eq!(app.senha_documento.as_deref(), Some("senha123"), "sem cópia de recuperação");
        let salvar = |app: &mut App| {
            app.alterado = true;
            let _ = app.update(Msg::Salvar);
            assert!(app.salvando, "{}", app.status);
            app.concluir_gravacao_teste(caminho.clone(), false);
            std::fs::read(&caminho).unwrap()
        };
        let gravado = salvar(&mut app);
        let zip = decifrar(&gravado, "senha123").expect("salvo com a mesma senha");
        let (volta, _) = feathersheet_io::xlsx_read::ler_xlsx_workbook(zip).unwrap();
        assert_eq!(volta.valor(0, 2, 0), feathersheet_core::celulas::values::Value::Text("Bia".into()));

        // Trocar: as duas caixas têm de conferir.
        let _ = app.evento_senha_arquivo(Evento::Definir);
        let _ = app.evento_senha_arquivo(Evento::Texto("nova".into()));
        let _ = app.evento_senha_arquivo(Evento::Confirmar("nov".into()));
        let _ = app.evento_senha_arquivo(Evento::Abrir);
        assert!(app.senha_arquivo.as_ref().is_some_and(|e| e.errou), "não conferem");
        let _ = app.evento_senha_arquivo(Evento::Confirmar("nova".into()));
        let _ = app.evento_senha_arquivo(Evento::Abrir);
        assert!(app.senha_arquivo.is_none() && app.alterado);
        let gravado = salvar(&mut app);
        assert_eq!(decifrar(&gravado, "senha123"), Err(ErroSenha::SenhaErrada));
        assert!(decifrar(&gravado, "nova").is_ok());

        // Tirar: em branco nas duas, o próximo salvamento é um ZIP comum.
        let _ = app.evento_senha_arquivo(Evento::Definir);
        let _ = app.evento_senha_arquivo(Evento::Texto(String::new()));
        let _ = app.evento_senha_arquivo(Evento::Confirmar(String::new()));
        let _ = app.evento_senha_arquivo(Evento::Abrir);
        assert!(app.senha_documento.is_none());
        assert!(salvar(&mut app).starts_with(b"PK"));
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
