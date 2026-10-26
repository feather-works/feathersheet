//! Histórico de macros: cada execução deixa uma linha (hora, macro, o que a
//! disparou, resultado, duração, arquivos gravados), para a pessoa ver no
//! editor Alt+F11 quais macros estão rodando e por quê.
//!
//! Fica só na memória enquanto o aplicativo está aberto: as últimas
//! [`LIMITE`] execuções, num anel. Não vai para o arquivo.
//!
//! Quase todo caminho passa por `App::rodar_macro` (`macro_dialogo.rs`), que
//! recebe o [`GatilhoMacro`] e registra aqui. A exceção é o MCP: a ferramenta
//! `sheet_macro` roda a macro dentro do `feathersheet-mcp`, sem passar pelo
//! app; o `mcp_vivo.rs` mede a chamada e registra com [`GatilhoMacro::Mcp`].
//!
//! Uma macro que pergunta Sim/Não roda duas (ou mais) vezes: a linha é uma
//! só, que fica "esperando resposta" e depois recebe o resultado final.
use super::*;
use std::collections::VecDeque;
use std::time::SystemTime;

/// Quantas execuções o histórico guarda; a mais antiga sai primeiro.
pub(super) const LIMITE: usize = 500;

/// O que fez a macro rodar.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum GatilhoMacro {
    /// Pela pessoa: Executar macro (F5 no editor, botão da faixa) ou o modo
    /// de captura de tela; `origem` diz qual.
    Manual { origem: String },
    /// Botão de macro desenhado na planilha, com o rótulo dele.
    Botao { rotulo: String },
    /// Evento convertido do VBA (`aoMudar`, `aoAtivar`), com a aba e, no
    /// `aoMudar`, o intervalo mudado.
    Evento { evento: String, aba: String, intervalo: Option<String> },
    /// Ferramenta `sheet_macro` do MCP (a IA).
    Mcp,
}

/// Grupo do gatilho, para o filtro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoGatilho { Manual, Botao, Evento, Mcp }

impl TipoGatilho {
    pub(super) const TODOS: [TipoGatilho; 4] =
        [TipoGatilho::Manual, TipoGatilho::Botao, TipoGatilho::Evento, TipoGatilho::Mcp];

    pub(super) fn rotulo(self) -> &'static str {
        match self {
            TipoGatilho::Manual => tr!("À mão"),
            TipoGatilho::Botao => tr!("Botão"),
            TipoGatilho::Evento => tr!("Evento"),
            TipoGatilho::Mcp => tr!("MCP / IA"),
        }
    }

    pub(super) fn icone(self) -> &'static str {
        match self {
            TipoGatilho::Manual => "player-play",
            TipoGatilho::Botao => "pointer",
            TipoGatilho::Evento => "bolt",
            TipoGatilho::Mcp => "message-circle",
        }
    }
}

impl GatilhoMacro {
    pub(super) fn manual(origem: &str) -> Self { GatilhoMacro::Manual { origem: origem.to_owned() } }

    pub(super) fn tipo(&self) -> TipoGatilho {
        match self {
            GatilhoMacro::Manual { .. } => TipoGatilho::Manual,
            GatilhoMacro::Botao { .. } => TipoGatilho::Botao,
            GatilhoMacro::Evento { .. } => TipoGatilho::Evento,
            GatilhoMacro::Mcp => TipoGatilho::Mcp,
        }
    }

    /// Frase curta: "Botão “Gerar”", "Ao mudar células · Planilha1!B19".
    pub(super) fn descricao(&self) -> String {
        match self {
            GatilhoMacro::Manual { origem } => trf!("À mão · {origem}"),
            GatilhoMacro::Botao { rotulo } => trf!("Botão “{rotulo}”"),
            GatilhoMacro::Evento { evento, aba, intervalo } => {
                let nome = match evento.as_str() {
                    "aoMudar" => tr!("Ao mudar células"),
                    "aoAtivar" => tr!("Ao ativar aba"),
                    outro => outro,
                };
                match intervalo {
                    Some(i) => trf!("{nome} · {aba}!{i}"),
                    None => trf!("{nome} · {aba}"),
                }
            }
            GatilhoMacro::Mcp => tr!("MCP / IA (sheet_macro)").into(),
        }
    }
}

/// Como a execução terminou.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ResultadoExecucao {
    Ok,
    Erro(String),
    /// A pessoa respondeu Não e a macro terminou sem mudar nada.
    Cancelada,
    /// Parada numa pergunta Sim/Não ainda sem resposta (o texto dela).
    Esperando(String),
}

impl ResultadoExecucao {
    pub(super) fn rotulo(&self) -> &'static str {
        match self {
            ResultadoExecucao::Ok => tr!("Concluída"),
            ResultadoExecucao::Erro(_) => tr!("Erro"),
            ResultadoExecucao::Cancelada => tr!("Cancelada (Não)"),
            ResultadoExecucao::Esperando(_) => tr!("Esperando Sim/Não"),
        }
    }

    pub(super) fn icone(&self) -> &'static str {
        match self {
            ResultadoExecucao::Ok => "circle-check",
            ResultadoExecucao::Erro(_) => "bug",
            ResultadoExecucao::Cancelada => "player-stop",
            ResultadoExecucao::Esperando(_) => "help",
        }
    }

    fn nivel(&self) -> Nivel {
        match self {
            ResultadoExecucao::Ok => Nivel::Info,
            ResultadoExecucao::Erro(_) => Nivel::Erro,
            _ => Nivel::Aviso,
        }
    }
}

/// Uma execução de macro.
#[derive(Debug, Clone)]
pub(super) struct Execucao {
    pub id: u64,
    /// Hora local da execução (HH:MM:SS).
    pub hora: String,
    /// Módulo/função rodado (nome do módulo no catálogo).
    pub macro_: String,
    pub gatilho: GatilhoMacro,
    pub resultado: ResultadoExecucao,
    /// Tempo de execução da macro, somando as reexecuções do Sim/Não (sem o
    /// tempo que a pessoa levou para responder).
    pub ms: u128,
    /// A macro mudou a planilha (o `Resultado` só diz sim/não, não quantas células).
    pub alterou: bool,
    /// Respostas dadas às perguntas Sim/Não, na ordem.
    pub respostas: Vec<bool>,
    /// Arquivos gravados (PDF, abas) e as falhas ao gravar.
    pub arquivos: Vec<String>,
}

impl Execucao {
    pub(super) fn erro(&self) -> bool { matches!(self.resultado, ResultadoExecucao::Erro(_)) }

    /// A linha do Console F12.
    fn linha_console(&self) -> String {
        let mut s = format!("[histórico] {} · {} · {}", self.macro_, self.gatilho.descricao(), self.resultado.rotulo());
        if let ResultadoExecucao::Erro(e) = &self.resultado {
            s.push_str(&format!(": {}", e.lines().next().unwrap_or("")));
        }
        if !self.respostas.is_empty() {
            s.push_str(&trf!(" · respostas {}", respostas_texto(&self.respostas)));
        }
        s.push_str(&format!(" · {} ms", self.ms));
        s
    }
}

pub(super) fn respostas_texto(respostas: &[bool]) -> String {
    respostas.iter().map(|&r| if r { tr!("Sim") } else { tr!("Não") }).collect::<Vec<_>>().join(", ")
}

/// O anel de execuções e o estado do painel (filtros, linha escolhida).
#[derive(Debug, Default)]
pub(super) struct Historico {
    execucoes: VecDeque<Execucao>,
    proximo_id: u64,
    pub filtro_macro: String,
    pub filtro_gatilho: Option<TipoGatilho>,
    pub so_erros: bool,
    pub selecionada: Option<u64>,
    /// Linhas do Console esperando o fim do `update` (ver `App::update`):
    /// registrar no meio dele apagaria a saída da macro no Console.
    console_pendente: Vec<(Nivel, String)>,
}

impl Historico {
    /// Acrescenta uma execução; devolve o id dela.
    pub(super) fn registrar(&mut self, mut e: Execucao) -> u64 {
        self.proximo_id += 1;
        e.id = self.proximo_id;
        if e.hora.is_empty() { e.hora = hora_local(SystemTime::now()); }
        self.console_pendente.push((e.resultado.nivel(), e.linha_console()));
        self.execucoes.push_back(e);
        while self.execucoes.len() > LIMITE {
            let saiu = self.execucoes.pop_front();
            if saiu.is_some_and(|s| Some(s.id) == self.selecionada) { self.selecionada = None; }
        }
        self.proximo_id
    }

    /// Atualiza a execução (a continuação depois do Sim/Não). Se ela já saiu
    /// do anel, entra de novo como execução nova.
    pub(super) fn atualizar(&mut self, id: u64, f: impl FnOnce(&mut Execucao)) -> u64 {
        if let Some(e) = self.execucoes.iter_mut().find(|e| e.id == id) {
            f(e);
            let (nivel, linha) = (e.resultado.nivel(), e.linha_console());
            self.console_pendente.push((nivel, linha));
            return id;
        }
        let mut nova = Execucao {
            id: 0, hora: String::new(), macro_: String::new(), gatilho: GatilhoMacro::manual("?"),
            resultado: ResultadoExecucao::Ok, ms: 0, alterou: false, respostas: Vec::new(), arquivos: Vec::new(),
        };
        f(&mut nova);
        self.registrar(nova)
    }

    pub(super) fn total(&self) -> usize { self.execucoes.len() }

    pub(super) fn obter(&self, id: u64) -> Option<&Execucao> {
        self.execucoes.iter().find(|e| e.id == id)
    }

    /// As execuções que passam nos filtros, a mais recente primeiro.
    pub(super) fn filtradas(&self) -> impl Iterator<Item = &Execucao> {
        let busca = feather_ui::ajuda::normalizar(self.filtro_macro.trim());
        self.execucoes.iter().rev().filter(move |e| {
            (!self.so_erros || e.erro())
                && self.filtro_gatilho.is_none_or(|t| e.gatilho.tipo() == t)
                && (busca.is_empty() || feather_ui::ajuda::normalizar(&e.macro_).contains(&busca))
        })
    }

    pub(super) fn limpar(&mut self) {
        self.execucoes.clear();
        self.selecionada = None;
    }

    /// As linhas do Console ainda não entregues (esvazia a fila).
    pub(super) fn tirar_console(&mut self) -> Vec<(Nivel, String)> {
        std::mem::take(&mut self.console_pendente)
    }
}

/// Hora local "HH:MM:SS". No Linux usa o fuso do sistema (`localtime_r`);
/// nos outros sistemas, por ora, a hora UTC com o sufixo "UTC".
pub(super) fn hora_local(t: SystemTime) -> String {
    let segundos = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let s = segundos as libc::time_t;
        // SAFETY: `localtime_r` só escreve na `tm` que passamos; nada é guardado.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&s, &mut tm) }.is_null() {
            return format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec);
        }
    }
    format!("{:02}:{:02}:{:02} UTC", segundos / 3600 % 24, segundos / 60 % 60, segundos % 60)
}

impl App {
    /// `FEATHERSHEET_CAPTURAR_JANELA=historico_macros`: roda algumas macros
    /// de exemplo, cada uma por um gatilho, escolhe a que deu erro e abre o
    /// editor (Alt+F11) com o Histórico à vista, para a foto.
    pub(super) fn preparar_captura_historico_macros(&mut self) -> Task<Msg> {
        let _ = self.rodar_macro(GatilhoMacro::manual(tr!("Executar macro")), tr!("Preencher"),
            "sheet.set('A1', 10); sheet.set('A2', 20);", vec![]);
        let _ = self.rodar_macro(GatilhoMacro::Botao { rotulo: tr!("Gerar relatório").into() }, "Relatorio",
            "log('relatório');", vec![]);
        let _ = self.rodar_macro(GatilhoMacro::Evento {
            evento: "aoMudar".into(), aba: "Planilha1".into(), intervalo: Some("B19".into()) },
            "Worksheet_Change", "sheet.set('C1', 'mudou');", vec![]);
        let _ = self.rodar_macro(GatilhoMacro::Evento {
            evento: "aoAtivar".into(), aba: "Planilha1".into(), intervalo: None },
            "Worksheet_Activate", "log('ativou');", vec![]);
        let _ = self.rodar_macro(GatilhoMacro::manual(tr!("Executar macro")), tr!("Importar"),
            "sheet.set('D1', 1);\nthrow new Error('Coluna \"Valor\" não encontrada na aba Dados');", vec![]);
        let _ = self.rodar_macro(GatilhoMacro::Botao { rotulo: tr!("Limpar").into() }, "LimparTudo",
            "if (confirm('Apagar tudo?')) sheet.clear('A1:D10');", vec![]);
        self.responder_macro(false);
        self.macro_mensagens.clear();
        let com_erro = self.historico_macros.filtradas().find(|e| e.erro()).map(|e| e.id);
        self.historico_macros.selecionada = com_erro;
        self.macro_visual.painel = crate::app::macros::editor_visual::PAINEL_HISTORICO;
        self.macro_visual.console = true;
        self.alternar(Auxiliar::Macros)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_macro(nome: &str, codigo: &str) -> App {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: nome.into(), code: codigo.into(), auto: false });
        app
    }

    #[test]
    fn gatilhos_diferentes_viram_linhas_com_o_gatilho_certo() {
        let mut app = app_com_macro("Dobrar", "sheet.set('Z1', 2);");
        // À mão, pelo editor (F5).
        app.macro_visual.documento = "modulo:Dobrar".into();
        app.macro_codigo = pelicano::Content::with_text("sheet.set('Z1', 2);");
        let _ = app.update(Msg::MacroExecutar);
        // Botão na planilha, com rótulo.
        app.novo_botao_macro();
        app.botao_macro_modal.as_mut().unwrap().rotulo = "Gerar relatório".into();
        app.guardar_botao_macro();
        let bt = app.workbook.aba(0).unwrap().sheet.bt.clone().unwrap();
        let itens: Vec<serde_json::Value> = serde_json::from_str(&bt).unwrap();
        app.executar_botao_macro(itens[0]["id"].as_str().unwrap().to_string());
        // Evento aoMudar.
        app.eventos_vba = vec![crate::app::eventos_vba::EventoVba {
            aba: "Planilha1".into(), evento: "aoMudar".into(), modulo: "Dobrar".into() }];
        app.update(Msg::Selecionar(1, 1));
        app.update(Msg::Editar("7".into()));
        app.update(Msg::Confirmar);

        let linhas: Vec<_> = app.historico_macros.filtradas().collect();
        assert_eq!(linhas.len(), 3, "{linhas:?}");
        // A mais recente primeiro.
        assert_eq!(linhas[0].gatilho, GatilhoMacro::Evento {
            evento: "aoMudar".into(), aba: "Planilha1".into(), intervalo: Some("B2".into()) });
        assert_eq!(linhas[0].gatilho.descricao(), "Ao mudar células · Planilha1!B2");
        assert_eq!(linhas[1].gatilho, GatilhoMacro::Botao { rotulo: "Gerar relatório".into() });
        assert!(matches!(linhas[2].gatilho, GatilhoMacro::Manual { .. }));
        assert!(linhas.iter().all(|e| e.macro_ == "Dobrar" && e.resultado == ResultadoExecucao::Ok && e.alterou));
        assert!(linhas.iter().all(|e| e.hora.len() >= 8));

        // Uma linha no Console F12 por execução (entregue no fim do update).
        let console: Vec<_> = app.monitor.console.registros()
            .filter(|r| r.origem == "Macros" && r.mensagem.starts_with("[histórico]")).collect();
        assert_eq!(console.len(), 3, "{:?}", console.iter().map(|r| &r.mensagem).collect::<Vec<_>>());
        assert!(console[1].mensagem.contains("Botão “Gerar relatório”") && console[1].mensagem.contains(" ms"));

        // Filtro por gatilho e por nome (sem acento/maiúscula).
        app.historico_macros.filtro_gatilho = Some(TipoGatilho::Botao);
        assert_eq!(app.historico_macros.filtradas().count(), 1);
        app.historico_macros.filtro_gatilho = None;
        app.historico_macros.filtro_macro = "dobr".into();
        assert_eq!(app.historico_macros.filtradas().count(), 3);
        app.historico_macros.filtro_macro = "outra".into();
        assert_eq!(app.historico_macros.filtradas().count(), 0);
    }

    #[test]
    fn erro_aparece_como_erro_e_filtro_so_erros() {
        let mut app = App::default();
        app.rodar_macro(GatilhoMacro::manual("teste"), "Boa", "sheet.set('A1', 1);", vec![]).unwrap();
        app.rodar_macro(GatilhoMacro::manual("teste"), "Ruim", "throw new Error('quebrou de vez');", vec![]).unwrap();
        let ultima = app.historico_macros.filtradas().next().unwrap();
        assert_eq!(ultima.macro_, "Ruim");
        assert!(matches!(&ultima.resultado, ResultadoExecucao::Erro(e) if e.contains("quebrou de vez")), "{:?}", ultima.resultado);
        app.historico_macros.so_erros = true;
        let so: Vec<_> = app.historico_macros.filtradas().map(|e| e.macro_.clone()).collect();
        assert_eq!(so, vec!["Ruim".to_string()]);
        let (nivel, linha) = app.historico_macros.tirar_console().pop().unwrap();
        assert_eq!(nivel, Nivel::Erro);
        assert!(linha.contains("Erro: ") && linha.contains("quebrou de vez"), "{linha}");
    }

    #[test]
    fn guarda_so_as_ultimas_500() {
        let mut h = Historico::default();
        for i in 0..(LIMITE + 37) {
            h.registrar(Execucao {
                id: 0, hora: "00:00:00".into(), macro_: format!("M{i}"), gatilho: GatilhoMacro::Mcp,
                resultado: ResultadoExecucao::Ok, ms: 1, alterou: false, respostas: vec![], arquivos: vec![],
            });
        }
        assert_eq!(h.total(), LIMITE);
        assert_eq!(h.filtradas().next().unwrap().macro_, format!("M{}", LIMITE + 36));
        assert_eq!(h.filtradas().last().unwrap().macro_, "M37");
        h.limpar();
        assert_eq!(h.total(), 0);
    }

    #[test]
    fn sim_nao_fica_numa_linha_so() {
        let mut app = App::default();
        let codigo = "if (confirm('Gravar?')) sheet.set('A1', 5);";
        assert!(app.rodar_macro(GatilhoMacro::manual("teste"), "Pergunta", codigo, vec![]).is_none());
        let e = app.historico_macros.filtradas().next().unwrap();
        assert_eq!(e.resultado, ResultadoExecucao::Esperando("Gravar?".into()));
        app.update(Msg::MacroResponder(false));
        assert_eq!(app.historico_macros.total(), 1);
        let e = app.historico_macros.filtradas().next().unwrap();
        assert_eq!(e.resultado, ResultadoExecucao::Cancelada);
        assert_eq!(e.respostas, vec![false]);

        app.rodar_macro(GatilhoMacro::manual("teste"), "Pergunta", codigo, vec![]);
        app.update(Msg::MacroResponder(true));
        assert_eq!(app.historico_macros.total(), 2);
        let e = app.historico_macros.filtradas().next().unwrap();
        assert_eq!(e.resultado, ResultadoExecucao::Ok);
        assert!(e.alterou && e.respostas == vec![true]);
    }

    #[test]
    fn arquivos_gravados_entram_na_linha() {
        let dir = std::env::temp_dir().join(format!("feathersheet-historico-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::default();
        app.caminho = Some(dir.join("Origem.fshz"));
        app.workbook.escrever_celula(0, 0, 0, "1".into());
        app.rodar_macro(GatilhoMacro::manual("teste"), "Pdf", "sheet.exportPdf('saida.pdf');", vec![]).unwrap();
        let e = app.historico_macros.filtradas().next().unwrap();
        assert!(e.arquivos.iter().any(|a| a.contains("saida.pdf")), "{:?}", e.arquivos);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hora_tem_formato_de_relogio() {
        let h = hora_local(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3 * 3600 + 4 * 60 + 5));
        let partes: Vec<_> = h[..8].split(':').collect();
        assert_eq!(partes.len(), 3);
        assert!(partes.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_digit())));
        assert!(h.ends_with(":04:05") || h.contains(":04:05"), "{h}");
    }
}
