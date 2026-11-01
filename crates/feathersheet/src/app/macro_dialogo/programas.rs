//! `app.shell(comando)` (o `Shell` do VBA). A macro só anota o comando; quando ela
//! termina bem, o app mostra cada comando e a pessoa decide se roda. Antes de
//! perguntar, procura programa escondido no comando e no código da macro
//! (`riscos::binario_escondido`: o vírus por macro monta um executável a partir de
//! Base64/bytes e roda pelo Shell); achou, o aviso fica em vermelho e o botão
//! em destaque passa a ser "Não rodar".
use super::*;

pub(in crate::app) struct ProgramasMacro {
    nome: String,
    comandos: Vec<String>,
    /// Sinais de programa escondido; vazio é o caso comum.
    suspeitas: Vec<String>,
}

/// Roda pelo interpretador do sistema, como o `Shell` do VBA, sem esperar o fim.
fn rodar(comando: &str, pasta: Option<&std::path::Path>) -> std::io::Result<u32> {
    let mut c = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(comando);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(comando);
        c
    };
    if let Some(p) = pasta.filter(|p| p.is_dir()) { c.current_dir(p); }
    let mut filho = c.stdin(std::process::Stdio::null()).spawn()?;
    let pid = filho.id();
    // Espera numa thread à parte só para o sistema não guardar o processo morto.
    std::thread::spawn(move || { let _ = filho.wait(); });
    Ok(pid)
}

impl App {
    pub(super) fn pedir_programas_da_macro(&mut self, nome: &str, codigo: &str, comandos: Vec<String>) {
        let mut suspeitas = feathersheet_macros::riscos::binario_escondido(&comandos.join("\n"));
        for s in feathersheet_macros::riscos::binario_escondido(codigo) {
            if !suspeitas.contains(&s) { suspeitas.push(s); }
        }
        self.macro_programas = Some(ProgramasMacro { nome: nome.to_owned(), comandos, suspeitas });
    }

    pub(in crate::app) fn responder_programas_da_macro(&mut self, rodar_sim: bool) {
        use feather_ui::monitor::Nivel;
        let Some(p) = self.macro_programas.take() else { return };
        if !rodar_sim {
            self.monitor.registrar(Nivel::Info, tr!("Macros"),
                &trf!("[macro] {}: a pessoa não deixou rodar {} programa(s)", p.nome, p.comandos.len()));
            return;
        }
        let pasta = self.caminho.as_deref().and_then(std::path::Path::parent);
        for comando in &p.comandos {
            match rodar(comando, pasta) {
                Ok(pid) => self.monitor.registrar(Nivel::Info, tr!("Macros"),
                    &trf!("[macro] {}: programa iniciado (processo {pid}): {comando}", p.nome)),
                Err(e) => {
                    self.monitor.registrar(Nivel::Erro, tr!("Macros"),
                        &trf!("[macro] {}: não foi possível rodar {comando}: {e}", p.nome));
                    self.macro_mensagens.push(trf!("Não foi possível rodar {comando}: {e}"));
                }
            }
        }
    }

    pub(super) fn dialogo_programas_da_macro<'a>(&'a self, p: &'a ProgramasMacro, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::super::dialogo_interno::dlg;
        let paragrafo = |t: String| text(t).size(12.5).line_height(1.4);
        let mut corpo = pelicano::column![paragrafo(trf!("A macro {} quer rodar no seu computador:", p.nome))].spacing(10);
        for c in &p.comandos {
            corpo = corpo.push(text(c.clone()).size(12.5).font(pelicano::Font::MONO));
        }
        let perigo = !p.suspeitas.is_empty();
        if perigo {
            let vermelho = COR_ERRO();
            corpo = corpo.push(text(tr!("Possível vírus: a macro esconde um programa")).size(13)
                .font(pelicano::Font { weight: 700, ..Default::default() }).color(vermelho));
            for s in &p.suspeitas {
                corpo = corpo.push(text(format!("• {s}")).size(12.5).line_height(1.4).color(vermelho));
            }
            corpo = corpo.push(paragrafo(tr!("Macros de vírus costumam trazer um programa disfarçado de texto, gravá-lo no disco e rodá-lo por aqui. Não rode se não sabe exatamente o que este comando faz.").into()));
        } else {
            corpo = corpo.push(paragrafo(tr!("O programa roda com as mesmas permissões que você. Só rode se confia na origem desta planilha.").into()));
        }
        let d = self.dialogo(p.nome.clone(), corpo)
            .icone("alert-triangle")
            .largura(540.0)
            .clique_fora_fecha(false);
        let d = if perigo {
            d.botao(dlg::botao_secundario(tr!("Rodar mesmo assim"), Some(Msg::MacroProgramas(true))))
                .primario(tr!("Não rodar"), Some(Msg::MacroProgramas(false)))
        } else {
            d.botao(dlg::botao_secundario(tr!("Não rodar"), Some(Msg::MacroProgramas(false))))
                .primario(tr!("Rodar"), Some(Msg::MacroProgramas(true)))
        };
        d.sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use historico_macros::GatilhoMacro;

    #[test]
    fn shell_espera_o_sim_e_acusa_programa_escondido() {
        let mut app = App::default();
        let r = app.rodar_macro(GatilhoMacro::manual("teste"), "M", "app.shell('true');", vec![]).unwrap();
        assert!(r.erro.is_none());
        let p = app.macro_programas.as_ref().expect("espera o Rodar?");
        assert_eq!(p.comandos, ["true"]);
        assert!(p.suspeitas.is_empty());
        app.update(Msg::MacroProgramas(false));
        assert!(app.macro_programas.is_none());

        let codigo = "const b = 'TVqQAAMAAAAEAAAA//8AALgAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'; app.shell('certutil -decode a.txt a.exe');";
        app.rodar_macro(GatilhoMacro::manual("teste"), "M", codigo, vec![]).unwrap();
        let p = app.macro_programas.as_ref().unwrap();
        assert!(p.suspeitas.len() >= 2, "{:?}", p.suspeitas);
    }
}
