use super::*;

pub(in crate::app) mod editor_visual;
pub(super) mod atalhos;
pub(in crate::app) mod codigo;
pub(in crate::app) mod organizacao;
pub(in crate::app) mod designer_formulario;
pub use codigo::EventoCodigo;

/// Estado exclusivamente visual do editor de macros, inspirado na UI TS/VBE.
/// Independente do interpretador geral e da serialização dos documentos.
#[derive(Debug, Clone)]
pub(super) struct EstadoVisual {
    pub(super) menu: Option<u8>,
    pub(super) documento: String,
    pub(super) painel: u8,
    /// O formulário em desenho (`designer_formulario.rs`).
    pub(super) designer: designer_formulario::Designer,
    pub(super) propriedades_categoria: bool,
    pub(super) explorador: bool,
    pub(super) propriedades: bool,
    pub(super) console: bool,
    /// Busca no projeto, Localizar/Substituir, desfazer e rolagem do código.
    pub(super) codigo: codigo::EstadoCodigo,
    /// Evento escolhido no código de eventos da planilha ("aoMudar"…).
    pub(super) evento_planilha: &'static str,
    /// Nome em edição no Renomear das Propriedades.
    pub(super) renomeando: Option<String>,
    /// Alturas escolhidas nas divisórias (None = proporção padrão).
    pub(super) altura_explorador: Option<f32>,
    pub(super) altura_console: Option<f32>,
    /// Largura escolhida para a lateral (None = 280 px).
    pub(super) largura_lateral: Option<f32>,
    /// Divisória sendo arrastada: qual, e (y do início, altura no início)
    /// depois do primeiro movimento.
    pub(super) arrasto: Option<(Divisoria, Option<(f32, f32)>)>,
    /// Alturas desenhadas no último quadro (base do arraste).
    pub(super) desenhado_explorador: std::cell::Cell<f32>,
    pub(super) desenhado_console: std::cell::Cell<f32>,
    pub(super) desenhado_largura: std::cell::Cell<f32>,
}

/// As divisórias arrastáveis do editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divisoria {
    /// Entre o Explorador de projeto e as Propriedades.
    Lateral,
    /// Entre o código e a Janela Imediata.
    Console,
    /// Entre a lateral (Explorador/Propriedades) e o código: arrasta na horizontal.
    Largura,
}

impl Default for EstadoVisual {
    fn default() -> Self {
        Self {
            menu: None,
            documento: "modulo:Macro1".into(),
            painel: 0,
            designer: Default::default(),
            propriedades_categoria: false,
            explorador: true,
            propriedades: true,
            console: true,
            codigo: Default::default(),
            evento_planilha: "aoMudar",
            renomeando: None,
            altura_explorador: None,
            altura_console: None,
            largura_lateral: None,
            arrasto: None,
            desenhado_explorador: std::cell::Cell::new(0.),
            desenhado_console: std::cell::Cell::new(0.),
            desenhado_largura: std::cell::Cell::new(0.),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EventoVisual {
    Menu(u8),
    FecharMenu,
    SelecionarMenu(u8, usize),
    Documento(String),
    Painel(u8),
    PropriedadesCategoria(bool),
    Explorador,
    Propriedades,
    Console,
    LimparSaida,
    /// Histórico de macros (painel 4 do console do editor).
    HistoricoFiltroMacro(String),
    HistoricoFiltroGatilho(Option<historico_macros::TipoGatilho>),
    HistoricoSoErros,
    HistoricoSelecionar(u64),
    /// Abre o módulo da execução escolhida no editor.
    HistoricoIrPara(String),
    HistoricoLimpar,
    /// Busca, Localizar/Substituir e desfazer do código (`codigo.rs`).
    Codigo(EventoCodigo),
    /// Código de eventos da planilha: qual evento editar.
    EventoPlanilha(&'static str),
    /// Pastas e Renomear (organizacao.rs).
    NovaPasta,
    RenomearInicio,
    RenomearTexto(String),
    RenomearConfirmar,
    RenomearCancelar,
    /// Arrastar uma divisória: apertar, mover (y na janela) e soltar.
    DivisoriaInicio(Divisoria),
    DivisoriaMover(f32),
    DivisoriaFim,
    /// O desenho dos formulários (`designer_formulario.rs`).
    Designer(designer_formulario::EventoDesigner),
}

impl App {
    pub(super) fn executar_macro(&mut self) {
        let doc = &self.macro_visual.documento;
        if !doc.starts_with("modulo:") && !doc.starts_with("planilha:") {
            self.macro_saida = tr!("Abra um módulo JavaScript para executar.").into();
            return;
        }
        self.confirmar();
        if !self.macros_liberadas() {
            self.macro_saida = tr!("Macros desativadas neste arquivo: ative no aviso da janela principal.").into();
            return;
        }
        let codigo = self.macro_codigo.text();
        // Módulo: o nome do documento; evento da planilha: Eventos/<aba>/<evento>.
        let nome = match self.macro_visual.documento.strip_prefix("modulo:") {
            Some(n) => n.to_owned(),
            None => self.macro_nome.clone(),
        };
        self.rodar_macro_em_fundo(historico_macros::GatilhoMacro::manual(tr!("Executar macro")), &nome, &codigo,
            macro_dialogo::em_fundo::DepoisDaMacro::Editor);
    }

    pub(super) fn abrir_macro() -> Task<Msg> {
        Task::perform(async {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao()).add_filter(tr!("Macro JavaScript"), &["js"])
                .pick_file().await.map(|f|std::fs::read_to_string(f.path())
                    .map_err(|e|trf!("Não consegui abrir a macro: {e}"))).transpose()
        },Msg::MacroLido)
    }

    pub(super) fn salvar_macro() -> Task<Msg> {
        Task::perform(async {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao()).add_filter(tr!("Macro JavaScript"), &["js"])
                .set_file_name("macro.js").save_file().await.map(|f|f.path().to_path_buf())
        },Msg::MacroDestino)
    }

    /// As macros moram na pasta de trabalho (gravadas no .fshz), como no
    /// .xlsm do Excel: cada arquivo leva as suas e Novo começa sem nenhuma.
    pub(super) fn persistir_modulos(&mut self, catalogo: feather_macros::Catalogo, status: &str) -> bool {
        let resultado = catalogo.exportar_json()
            .and_then(|json| feather_macros::Catalogo::importar_json(&json).map(|v| (json, v)));
        match resultado {
            Ok((json, validado)) => {
                self.workbook.macros = (!validado.macros.is_empty()).then_some(json);
                self.macro_catalogo = validado;
                self.sincronizar_eventos();
                self.alterado = true;
                self.macro_saida = trf!("{status}. Guardado nesta planilha (salve o arquivo para gravar).");
                true
            }
            Err(e) => {
                self.macro_saida = trf!("Falha ao guardar as macros: {e}");
                false
            }
        }
    }

    /// O catálogo da pasta aberta (vazio se ela não tem macros). Chamado
    /// sempre que a pasta troca: abrir, Novo, modelo.
    pub(super) fn recarregar_macros(&mut self) {
        self.macro_catalogo = self.workbook.macros.as_deref()
            .and_then(|t| feather_macros::Catalogo::importar_json(t).ok())
            .unwrap_or_default();
        self.sincronizar_eventos();
    }

    /// Gera um nome não utilizado, sem depender do código aberto no editor.
    fn proximo_nome_modulo(catalogo: &feather_macros::Catalogo) -> String {
        let mut numero = 1usize;
        loop {
            let nome = format!("Modulo{numero}");
            if catalogo.obter(&nome).is_none() { return nome; }
            numero += 1;
        }
    }

    /// Novo Módulo do antigo editor TypeScript, integrado à biblioteca JS
    /// compartilhada do FeatherWorks. Não executa a macro automaticamente.
    pub(super) fn criar_modulo_macro(&mut self) {
        let codigo = self.macro_codigo.text();
        let guardado = self.macro_catalogo.obter(self.macro_nome.trim());
        // Não substituir o texto do editor se ainda houver mudanças não
        // salvas. Para um rascunho novo a falta de módulo também conta.
        if guardado.is_none_or(|m| m.code != codigo) {
            self.macro_saida = trf!(
                "Guarde primeiro as alterações de '{}' antes de criar outro módulo.",
                self.macro_nome
            );
            self.macro_visual.console = true;
            return;
        }
        if self.macro_catalogo.macros.len() >= 1_000 {
            self.macro_saida = tr!("Limite de 1.000 módulos da biblioteca atingido.").into();
            return;
        }
        let nome = Self::proximo_nome_modulo(&self.macro_catalogo);
        let codigo = format!("// {nome} · JavaScript do FeatherSheet\n");
        let mut catalogo = self.macro_catalogo.clone();
        catalogo.macros.push(feather_macros::Macro {
            name: nome.clone(), code: codigo.clone(), auto: false,
        });
        // Só muda a UI depois do arquivo ser gravado com sucesso.
        if self.persistir_modulos(catalogo, &trf!("Módulo {nome} criado")) {
            self.macro_nome = nome.clone();
            self.macro_codigo = pelicano::Content::with_text(&codigo);
            self.macro_visual.documento = format!("modulo:{nome}");
            self.macro_visual.menu = None;
            self.macro_visual.explorador = true;
            self.macro_visual.painel = 0;
        }
    }

    pub(super) fn guardar_modulo_macro(&mut self) {
        if self.macro_visual.documento.starts_with("planilha:") {
            self.guardar_evento_planilha();
            return;
        }
        let nome = self.macro_nome.trim().to_owned();
        if nome.is_empty() {
            self.macro_saida = tr!("Informe o nome do módulo.").into();
            return;
        }
        let mut c = self.macro_catalogo.clone();
        if let Some(m) = c.macros.iter_mut().find(|m| m.name.eq_ignore_ascii_case(&nome)) {
            m.code = self.macro_codigo.text();
        } else {
            c.macros.push(feather_macros::Macro {
                name: nome.clone(), code: self.macro_codigo.text(), auto: false,
            });
        }
        self.persistir_modulos(c, &trf!("Módulo {nome} salvo"));
        self.macro_visual.documento = format!("modulo:{nome}");
    }

    pub(super) fn abrir_modulo_macro(&mut self, nome: &str) {
        if let Some(m) = self.macro_catalogo.obter(nome) {
            self.macro_codigo = pelicano::Content::with_text(&m.code);
            self.macro_nome = m.name.clone();
            self.macro_visual.documento = format!("modulo:{}", m.name);
            self.macro_saida = trf!("Módulo {} carregado; execute manualmente.", m.name);
        }
    }

    pub(super) fn excluir_modulo_macro(&mut self) {
        let mut c = self.macro_catalogo.clone();
        let antes = c.macros.len();
        c.macros.retain(|m| !m.name.eq_ignore_ascii_case(self.macro_nome.trim()));
        if antes == c.macros.len() {
            self.macro_saida = tr!("Selecione um módulo existente.").into();
            return;
        }
        self.persistir_modulos(c, tr!("Módulo excluído"));
    }

    /// Importa o array de office.macros.v1 (TS), preservando outros módulos.
    pub(super) fn mesclar_catalogo_macro(&mut self, texto: &str) {
        // Primeiro tenta o catálogo nativo estrito; para exports antigos de
        // office.macros.v1, recupera as entradas válidas como fazia o TS.
        let importado = feather_macros::Catalogo::importar_json(texto)
            .or_else(|_| feather_macros::Catalogo::importar_json_legado(texto));
        match importado {
            Ok(importado) => {
                let mut c = self.macro_catalogo.clone();
                for novo in importado.macros {
                    if let Some(m) = c.macros.iter_mut().find(|m|
                        m.name.eq_ignore_ascii_case(&novo.name)
                    ) {
                        *m = novo;
                    } else {
                        c.macros.push(novo);
                    }
                }
                self.persistir_modulos(c, tr!("Catálogo de macros importado"));
            }
            Err(e) => self.macro_saida = trf!("Catálogo inválido: {e}"),
        }
    }

    pub(super) fn importar_catalogo_macro() -> Task<Msg> {
        Task::perform(async {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao()).add_filter(tr!("Biblioteca JSON"), &["json"])
                .pick_file().await.map(|f| std::fs::read_to_string(f.path())
                    .map_err(|e| e.to_string())).transpose()
        }, Msg::MacroCatalogoLido)
    }

    pub(super) fn exportar_catalogo_macro() -> Task<Msg> {
        Task::perform(async {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao()).add_filter(tr!("Biblioteca JSON"), &["json"])
                .set_file_name("featheroffice-macros.json").save_file()
                .await.map(|f| f.path().to_path_buf())
        }, Msg::MacroDestinoCatalogo)
    }

    /// Um item de menu fecha o painel antes de executar o comando ativo.
    /// Itens sem implementação nunca despacham ação fictícia.
    pub(super) fn selecionar_item_menu_macro(&mut self, id: u8, indice: usize) -> Task<Msg> {
        self.macro_visual.menu = None;
        editor_visual::acao_menu_visual(id, indice)
            .map(Task::done).unwrap_or_else(Task::none)
    }

    pub(super) fn evento_macro_visual(&mut self, evento: EventoVisual) {
        match evento {
            EventoVisual::Menu(id) => {
                self.macro_visual.menu = (self.macro_visual.menu != Some(id)).then_some(id);
            }
            EventoVisual::FecharMenu => self.macro_visual.menu = None,
            EventoVisual::Documento(nome) => {
                self.macro_visual.menu = None;
                if let Some(modulo) = nome.strip_prefix("modulo:") {
                    // Só módulos existentes carregam outro código na GUI.
                    if self.macro_catalogo.obter(modulo).is_some() {
                        self.abrir_modulo_macro(modulo);
                    } else if modulo != self.macro_nome {
                        return;
                    }
                } else if let Some(aba) = nome.strip_prefix("planilha:") {
                    if !self.workbook.nomes().iter().any(|n| n == aba) { return; }
                    let aba = aba.to_owned();
                    self.macro_visual.renomeando = None;
                    self.abrir_evento_planilha(&aba);
                    return;
                } else if nome != "pasta" && nome != "componente:previsao" {
                    return;
                }
                self.macro_visual.renomeando = None;
                self.macro_visual.documento = nome;
            }
            EventoVisual::Painel(p) if p <= editor_visual::PAINEL_HISTORICO => {
                self.macro_visual.painel = p;
                self.macro_visual.console = true;
            }
            EventoVisual::Designer(e) => self.evento_designer(e),
            EventoVisual::PropriedadesCategoria(v) => self.macro_visual.propriedades_categoria = v,
            EventoVisual::Explorador => self.macro_visual.explorador = !self.macro_visual.explorador,
            EventoVisual::Propriedades => self.macro_visual.propriedades = !self.macro_visual.propriedades,
            EventoVisual::Console => self.macro_visual.console = !self.macro_visual.console,
            EventoVisual::LimparSaida => self.macro_saida.clear(),
            EventoVisual::HistoricoFiltroMacro(t) => self.historico_macros.filtro_macro = t,
            EventoVisual::HistoricoFiltroGatilho(g) => self.historico_macros.filtro_gatilho = g,
            EventoVisual::HistoricoSoErros => self.historico_macros.so_erros = !self.historico_macros.so_erros,
            EventoVisual::HistoricoSelecionar(id) => {
                let h = &mut self.historico_macros;
                h.selecionada = (h.selecionada != Some(id)).then_some(id);
            }
            EventoVisual::HistoricoIrPara(nome) => {
                self.evento_macro_visual(EventoVisual::Documento(format!("modulo:{nome}")));
            }
            EventoVisual::HistoricoLimpar => self.historico_macros.limpar(),
            EventoVisual::EventoPlanilha(e) => {
                self.macro_visual.evento_planilha = e;
                if let Some(aba) = self.macro_visual.documento.strip_prefix("planilha:").map(str::to_owned) {
                    self.abrir_evento_planilha(&aba);
                }
            }
            EventoVisual::NovaPasta => self.nova_pasta_macro(),
            EventoVisual::RenomearInicio => {
                if self.macro_visual.documento.starts_with("modulo:") {
                    self.macro_visual.renomeando = Some(self.macro_nome.clone());
                }
            }
            EventoVisual::RenomearTexto(t) => {
                if self.macro_visual.renomeando.is_some() { self.macro_visual.renomeando = Some(t); }
            }
            EventoVisual::RenomearCancelar => self.macro_visual.renomeando = None,
            EventoVisual::RenomearConfirmar => {
                if let Some(novo) = self.macro_visual.renomeando.clone() {
                    match self.renomear_modulo(&novo) {
                        Ok(()) => self.macro_visual.renomeando = None,
                        Err(e) => { self.macro_saida = e; self.macro_visual.console = true; }
                    }
                }
            }
            EventoVisual::DivisoriaInicio(d) => self.macro_visual.arrasto = Some((d, None)),
            EventoVisual::DivisoriaFim => self.macro_visual.arrasto = None,
            EventoVisual::DivisoriaMover(y) => {
                let v = &mut self.macro_visual;
                match v.arrasto {
                    Some((d, None)) => {
                        let atual = match d {
                            Divisoria::Lateral => v.desenhado_explorador.get(),
                            Divisoria::Console => v.desenhado_console.get(),
                            Divisoria::Largura => v.desenhado_largura.get(),
                        };
                        v.arrasto = Some((d, Some((y, atual))));
                    }
                    // O limite de cada lado é aplicado no desenho (editor_visual).
                    Some((Divisoria::Lateral, Some((y0, h0)))) => v.altura_explorador = Some(h0 + y - y0),
                    Some((Divisoria::Console, Some((y0, h0)))) => v.altura_console = Some(h0 - (y - y0)),
                    // Na Largura a coordenada é o x do mouse.
                    Some((Divisoria::Largura, Some((x0, l0)))) => v.largura_lateral = Some(l0 + y - x0),
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// Processa atalhos somente quando o evento pertence à janela auxiliar.
    /// Escape fecha primeiro o menu do VBE, preservando o documento aberto.
    pub(super) fn atalho_macro(
        &mut self, key: &keyboard::Key, mods: keyboard::Modifiers,
    ) -> Option<Task<Msg>> {
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
            && mods == keyboard::Modifiers::default()
        {
            if self.macro_visual.menu.take().is_some() {
                return Some(Task::none());
            }
            if self.macro_visual.renomeando.take().is_some() {
                return Some(Task::none());
            }
            // Depois do menu: a barra Localizar, depois a busca do Explorador.
            if let Some(tarefa) = self.escape_codigo() {
                return Some(tarefa);
            }
            return Some(self.janelas.fechar_tipo(Auxiliar::Macros));
        }
        let acao = atalhos::comando(key, mods)?;
        // Ctrl+Z/Ctrl+Y com o foco num campo da barra Localizar não mexem no código.
        if matches!(acao, Msg::MacroVisual(EventoVisual::Codigo(EventoCodigo::Desfazer | EventoCodigo::Refazer)))
            && self.macro_visual.codigo.campo_focado
        {
            return Some(Task::none());
        }
        // Não salvar/rodar código de outro documento enquanto uma folha ou
        // formulário de prévia estiver selecionado.
        if matches!(acao, Msg::MacroExecutar | Msg::MacroGuardarModulo | Msg::MacroSalvar)
            && !self.macro_visual.documento.starts_with("modulo:")
            && !self.macro_visual.documento.starts_with("planilha:")
        {
            self.macro_saida = tr!("Abra um módulo JavaScript para executar ou guardar o código.").into();
            return Some(Task::none());
        }
        Some(self.update(acao))
    }

    pub(super) fn view_macros(&self) -> Element<'_, Msg> {
        editor_visual::janela(self)
    }

}

#[cfg(test)]
mod testes_visuais {
    use super::*;

    #[test]
    fn proximo_modulo_usa_nome_livre_sem_colidir_com_maiúsculas() {
        let catalogo = feather_macros::Catalogo {
            macros: vec![
                feather_macros::Macro {
                    name: "Modulo1".into(), code: "log('um')".into(), auto: false,
                },
                feather_macros::Macro {
                    name: "MODULO2".into(), code: "log('dois')".into(), auto: true,
                },
            ],
        };
        assert_eq!(App::proximo_nome_modulo(&catalogo), "Modulo3");
        assert_eq!(catalogo.macros[1].auto, true);
    }

    #[test]
    fn novo_modulo_nao_apaga_rascunho_sem_guardar() {
        // Impede escrita em configuração real: a criação deve ser recusada.
        let mut app = App::default();
        app.macro_catalogo = feather_macros::Catalogo::default();
        app.macro_nome = "Rascunho".into();
        app.macro_codigo = pelicano::Content::with_text(
            "sheet.range('A1').value = 33;",
        );
        let antes = app.macro_codigo.text();
        app.criar_modulo_macro();
        assert_eq!(app.macro_codigo.text(), antes);
        assert_eq!(app.macro_nome, "Rascunho");
        assert!(app.macro_catalogo.macros.is_empty());
        assert!(app.macro_saida.contains("Guarde primeiro"));
    }

    #[test]
    fn menus_de_macro_oferecem_criacao_real() {
        let itens_arquivo = editor_visual::acao_menu_visual(0, 0);
        let itens_inserir = editor_visual::acao_menu_visual(3, 0);
        assert!(matches!(itens_arquivo, Some(Msg::MacroNovoModulo)));
        assert!(matches!(itens_inserir, Some(Msg::MacroNovoModulo)));
    }

    #[test]
    fn painel_editor_ts_alterna_sem_mudar_documento() {
        let mut app = App::default();
        let original = app.workbook.aba(0).unwrap().sheet.get_raw(3, 3);
        assert!(app.macro_visual.explorador);
        assert!(app.macro_visual.console);
        assert!(app.macro_visual.propriedades);
        app.evento_macro_visual(EventoVisual::Menu(3));
        assert_eq!(app.macro_visual.menu, Some(3));
        app.evento_macro_visual(EventoVisual::Menu(3));
        assert!(app.macro_visual.menu.is_none());
        app.evento_macro_visual(EventoVisual::Explorador);
        app.evento_macro_visual(EventoVisual::Propriedades);
        assert!(!app.macro_visual.explorador);
        assert!(!app.macro_visual.propriedades);
        app.evento_macro_visual(EventoVisual::Painel(2));
        assert_eq!(app.macro_visual.painel, 2);
        app.evento_macro_visual(EventoVisual::Documento("componente:previsao".into()));
        assert_eq!(app.macro_visual.documento, "componente:previsao");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 3), original);
        assert!(!app.alterado);
    }

    #[test]
    fn atalhos_do_editor_nao_afetam_a_planilha_e_abrem_a_saida() {
        use crate::compat::keyboard::key::Named; use crate::compat::keyboard::Key; use crate::compat::keyboard::Modifiers;
        let mut app = App::default();
        let principal = crate::compat::window::novo_id();
        let macro_id = crate::compat::window::novo_id();
        app.janelas.principal = Some(principal);
        app.janelas.registrar(macro_id, Auxiliar::Macros);
        app.macro_visual.console = false;
        app.macro_visual.painel = 2;
        app.update(Msg::MacroTeclado(principal, Key::Named(Named::F5), Modifiers::default()));
        assert!(!app.macro_visual.console);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");

        // O atalho funciona quando o text_editor consumiu o evento.
        app.update(Msg::MacroTeclado(macro_id, Key::Character("g".into()), Modifiers::CTRL));
        assert!(app.macro_visual.console);
        assert_eq!(app.macro_visual.painel, 0);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");

        // Na guia visual, executar não deve rodar o conteúdo da última macro.
        app.macro_visual.documento = "componente:previsao".into();
        app.update(Msg::Teclado(macro_id, Key::Named(Named::F5), Modifiers::default()));
        assert!(app.macro_saida.contains("Abra um módulo"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");
    }

    #[test]
    fn limpar_console_nao_altera_codigo_da_macro() {
        let mut app = App::default();
        let codigo = app.macro_codigo.text();
        app.macro_saida = "teste de saída".into();
        app.evento_macro_visual(EventoVisual::LimparSaida);
        assert!(app.macro_saida.is_empty());
        assert_eq!(app.macro_codigo.text(), codigo);
    }

    #[test]
    fn escape_fecha_primeiro_menu_e_alt_q_fecha_apenas_editor() {
        use crate::compat::keyboard::key::Named; use crate::compat::keyboard::Key; use crate::compat::keyboard::Modifiers;
        let mut app = App::default();
        let principal = crate::compat::window::novo_id();
        let macro_id = crate::compat::window::novo_id();
        app.janelas.principal = Some(principal);
        app.janelas.registrar(macro_id, Auxiliar::Macros);
        app.evento_macro_visual(EventoVisual::Menu(1));
        assert_eq!(app.macro_visual.menu, Some(1));

        app.update(Msg::Teclado(macro_id, Key::Named(Named::Escape), Modifiers::default()));
        assert!(app.macro_visual.menu.is_none());
        assert_eq!(app.janelas.id(Auxiliar::Macros), Some(macro_id));

        app.update(Msg::MacroTeclado(macro_id, Key::Character("q".into()), Modifiers::ALT));
        assert!(!app.janelas.aberta(Auxiliar::Macros));
        assert_eq!(app.janelas.principal, Some(principal));
    }

    #[test]
    fn documentos_estranhos_nao_sao_aceitos_no_explorador() {
        let mut app = App::default();
        let base = app.macro_visual.documento.clone();
        app.evento_macro_visual(EventoVisual::Documento("modulo:Inexistente".into()));
        assert_eq!(app.macro_visual.documento, base);
        app.evento_macro_visual(EventoVisual::Documento("planilha:Falsa".into()));
        assert_eq!(app.macro_visual.documento, base);
        let aba = app.workbook.nomes()[0].clone();
        app.evento_macro_visual(EventoVisual::Documento(format!("planilha:{aba}")));
        assert!(app.macro_visual.documento.starts_with("planilha:"));
        app.evento_macro_visual(EventoVisual::Documento("pasta".into()));
        assert_eq!(app.macro_visual.documento, "pasta");
    }
}

#[cfg(test)]
mod testes_macros_da_pasta {
    use super::*;

    #[test]
    fn macros_moram_na_pasta_novo_comeca_vazio_e_voltam_ao_reabrir() {
        let mut app = App::default();
        assert!(app.macro_catalogo.macros.is_empty(), "nada global ao abrir");
        let mut c = app.macro_catalogo.clone();
        c.macros.push(feather_macros::Macro { name: "Colorir".into(), code: "sheet.set('A1',1);".into(), auto: false });
        assert!(app.persistir_modulos(c, "Criada"));
        assert!(app.alterado && app.workbook.macros.is_some());
        // O arquivo leva a macro: grava e lê de volta noutra pasta.
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook, 0).unwrap();
        let lida = feathersheet_io::para_workbook::para_workbook(&feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        // Novo: pasta sem macros.
        let _ = app.executar_pendente(Pendente::Novo);
        assert!(app.macro_catalogo.macros.is_empty() && app.workbook.macros.is_none());
        // Reabrir o arquivo traz o catálogo dele.
        app.workbook = lida;
        app.recarregar_macros();
        assert_eq!(app.macro_catalogo.macros.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Colorir"]);
    }
}
