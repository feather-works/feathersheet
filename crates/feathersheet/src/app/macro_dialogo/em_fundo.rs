//! A macro acionada pela pessoa (botão, forma, Executar macro) roda noutra thread: a janela
//! continua respondendo (o sistema não acusa "não está respondendo" numa macro longa), mostra
//! há quanto tempo ela roda e tem o botão Parar.
//!
//! A pasta vai junto para a thread e volta no fim; enquanto isso a tela usa um espelho vazio
//! com as mesmas abas, e o `update` só deixa passar o que não mexe na pasta.
use super::*;
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc};

/// Mais que o padrão de 30 s: aqui a janela não congela e a pessoa pode parar.
const PRAZO_EM_FUNDO: std::time::Duration = std::time::Duration::from_secs(600);

/// O que fazer com o resultado, conforme quem disparou a macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum DepoisDaMacro { Editor, Botao, Forma }

pub(in crate::app) struct MacroRodando {
    nome: String,
    codigo: String,
    gatilho: GatilhoMacro,
    depois: DepoisDaMacro,
    inicio: std::time::Instant,
    parar: Arc<AtomicBool>,
    recebe: mpsc::Receiver<(Workbook, feathersheet_macros::Resultado)>,
}

impl App {
    pub(in crate::app) fn rodar_macro_em_fundo(&mut self, gatilho: GatilhoMacro, nome: &str, codigo: &str, depois: DepoisDaMacro) {
        if self.macro_rodando.is_some() {
            self.status = tr!("Já há uma macro rodando.").into();
            return;
        }
        let parar = Arc::new(AtomicBool::new(false));
        let mut contexto = self.contexto_da_macro(Vec::new());
        contexto.limites = feather_macros::Limites { prazo: PRAZO_EM_FUNDO, parar: Some(parar.clone()) };
        let espelho = espelho_da_pasta(&self.workbook);
        let mut pasta = std::mem::replace(&mut self.workbook, espelho);
        let (folha, codigo_thread) = (self.folha, codigo.to_owned());
        let (envio, recebe) = mpsc::channel();
        std::thread::spawn(move || {
            // Pânico na thread não pode levar a pasta embora: ela volta sempre.
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                feathersheet_macros::executar_com(&mut pasta, folha, &codigo_thread, contexto)))
                .unwrap_or_else(|_| feathersheet_macros::Resultado {
                    erro: Some(tr!("a macro travou o interpretador").into()), ..Default::default()
                });
            let _ = envio.send((pasta, r));
        });
        self.status = trf!("Executando a macro {nome}…");
        self.macro_rodando = Some(MacroRodando {
            nome: nome.to_owned(), codigo: codigo.to_owned(), gatilho, depois,
            inicio: std::time::Instant::now(), parar, recebe,
        });
        // Nos testes a execução é a mesma, mas a resposta é esperada na hora.
        #[cfg(test)]
        if !testes::SEM_ESPERAR.with(|s| s.get()) {
            self.conferir_macro_em_fundo(true);
        }
    }

    /// `Msg::MacroPulso`: a macro terminou? `esperar` bloqueia até terminar (testes).
    pub(in crate::app) fn conferir_macro_em_fundo(&mut self, esperar: bool) {
        let Some(m) = self.macro_rodando.as_ref() else { return };
        let (pasta, r) = if esperar {
            match m.recebe.recv() { Ok(x) => x, Err(_) => return }
        } else {
            match m.recebe.try_recv() { Ok(x) => x, Err(_) => return }
        };
        let m = self.macro_rodando.take().expect("macro rodando");
        self.workbook = pasta;
        self.cache_grade.desenho.clear();
        let Some(r) = self.concluir_macro(m.gatilho, &m.nome, &m.codigo, Vec::new(), None, m.inicio, r) else {
            self.macro_saida = tr!("A macro fez uma pergunta: responda na janela.").into();
            return;
        };
        let nome = &m.nome;
        self.macro_saida = r.saida.join("\n");
        match (m.depois, r.erro) {
            (DepoisDaMacro::Editor, Some(e)) => {
                self.macro_saida.push_str(&trf!("\nErro: {e}\nAs escritas desta execução foram revertidas."));
            }
            (DepoisDaMacro::Editor, None) => {
                self.macro_saida.push_str(tr!("\nMacro concluída."));
                if let Some(mensagem) = r.status { self.status = mensagem; }
            }
            (_, Some(erro)) => self.status = trf!("Erro ao executar {nome}: {erro}. Escritas revertidas."),
            (DepoisDaMacro::Botao, None) => self.status = trf!("Macro {nome} executada pelo botão."),
            (DepoisDaMacro::Forma, None) => self.status = trf!("Macro {nome} executada pela forma (Ctrl+clique seleciona a forma)."),
        }
        self.trocar_selecao(self.linha, self.coluna);
    }

    pub(in crate::app) fn parar_macro_em_fundo(&mut self) {
        if let Some(m) = &self.macro_rodando {
            m.parar.store(true, Ordering::Relaxed);
            self.status = trf!("Parando a macro {}…", m.nome);
        }
    }

    /// Enquanto a macro roda, só passa o que não mexe na pasta.
    pub(in crate::app) fn mensagem_liberada_durante_macro(msg: &Msg) -> bool {
        matches!(msg, Msg::MacroPulso | Msg::MacroParar | Msg::MonitorPulso | Msg::ProgressoPulso
            | Msg::CapturarTela | Msg::JanelaRedimensionada(..) | Msg::RedimensionarJanela(..)
            | Msg::AuxAberta | Msg::AuxFechada(_))
    }

    pub(in crate::app) fn sobrepor_macro_rodando<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(m) = &self.macro_rodando else { return base };
        let segundos = m.inicio.elapsed().as_secs();
        let parando = m.parar.load(Ordering::Relaxed);
        let corpo = pelicano::column![
            text(trf!("{segundos} s")).size(28),
            text(if parando { tr!("Parando… as escritas desta execução serão desfeitas.") }
                else { tr!("A planilha volta assim que a macro terminar.") })
                .size(12).color(COR_TEXTO_MUTED()),
        ].spacing(6);
        self.dialogo(trf!("Executando {}", m.nome), corpo)
            .icone("player-play")
            .largura(380.0)
            .clique_fora_fecha(false)
            .primario(tr!("Parar"), (!parando).then_some(Msg::MacroParar))
            .sobre(base)
    }
}

/// A pasta que a tela usa enquanto a de verdade está com a macro: vazia, com as mesmas abas.
fn espelho_da_pasta(w: &Workbook) -> Workbook {
    let mut e = Workbook::new();
    for (i, nome) in w.nomes().iter().enumerate() {
        if i == 0 { e.renomear_aba(0, nome); } else { e.inserir_aba(nome.clone(), i); }
    }
    e
}

#[cfg(test)]
mod testes {
    use super::*;
    thread_local! { pub(super) static SEM_ESPERAR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

    #[test]
    fn macro_longa_roda_em_fundo_bloqueia_a_edicao_e_para() {
        SEM_ESPERAR.with(|s| s.set(true));
        let mut app = App::default();
        app.workbook.aba_mut(0).unwrap().sheet.set_cell(0, 0, "1");
        app.rodar_macro_em_fundo(GatilhoMacro::manual("teste"), "Longa",
            "sheet.set('B1', 2); for (let i = 0; i < 19e6; i++) {}", DepoisDaMacro::Botao);
        assert!(app.macro_rodando.is_some());
        // A pasta está com a thread: a tela mostra o espelho, e editar não passa.
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).is_empty());
        let _ = app.update(Msg::Desfazer);
        let _ = app.update(Msg::MacroParar);
        let inicio = std::time::Instant::now();
        while app.macro_rodando.is_some() && inicio.elapsed() < std::time::Duration::from_secs(10) {
            let _ = app.update(Msg::MacroPulso);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        SEM_ESPERAR.with(|s| s.set(false));
        assert!(app.macro_rodando.is_none(), "a macro não parou");
        assert!(app.status.contains("Erro ao executar Longa"), "{}", app.status);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "1");
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 1).is_empty(), "a escrita da macro parada volta");
    }
}
