//! Renomeação inline na própria guia, sem sobrepor a planilha.
use super::*;

impl App {
    pub(super) fn abrir_renomeacao(&mut self) -> Task<Msg> {
        let Some(aba) = self.workbook.aba(self.folha) else {
            return Task::none();
        };
        self.nome_aba = aba.sheet.name.clone();
        self.aba_em_renomeacao = Some(aba.id);
        self.erro_nome_aba = None;
        self.editando_nome_aba = true;
        self.menu_aberto = false;
        self.contexto = None;
        // shortcut: o pelicano não seleciona o texto do campo por Task; foca e o cursor fica no fim (digitar não substitui o nome), trocar quando houver selecionar tudo.
        Task::focus("feathersheet.renomear-aba")
    }

    pub(super) fn cancelar_renomeacao(&mut self) {
        self.editando_nome_aba = false;
        self.aba_em_renomeacao = None;
        self.erro_nome_aba = None;
        self.nome_aba = self
            .workbook
            .nomes()
            .get(self.folha)
            .cloned()
            .unwrap_or_default();
    }

    pub(super) fn salvar_nome_aba(&mut self) -> Task<Msg> {
        if !self.editando_nome_aba {
            return Task::none();
        }
        let alvo = (0..self.workbook.len()).find(|&i| {
            self.workbook
                .aba(i)
                .is_some_and(|a| Some(a.id) == self.aba_em_renomeacao)
        });
        let Some(alvo) = alvo else {
            self.erro_nome_aba = Some(tr!("A aba não está mais disponível.").into());
            return Task::none();
        };
        let nome = self.nome_aba.trim().to_owned();
        let erro = if nome.is_empty() {
            Some(tr!("O nome não pode ficar vazio."))
        } else if nome.contains('!') {
            Some(tr!("O nome não pode conter \"!\"."))
        } else if self
            .workbook
            .nomes()
            .iter()
            .enumerate()
            .any(|(i, n)| i != alvo && n.to_lowercase() == nome.to_lowercase())
        {
            Some(tr!("Já existe uma aba com esse nome."))
        } else {
            None
        };
        if let Some(erro) = erro {
            self.erro_nome_aba = Some(erro.into());
            self.status = erro.into();
            // Permanecer editando na própria guia para permitir correção.
            return Task::focus("feathersheet.renomear-aba");
        }
        let antigo = self.workbook.nomes()[alvo].clone();
        if self.workbook.renomear_aba(alvo, &nome) {
            self.cancelar_renomeacao();
            if antigo != nome {
                self.alterado = true;
                self.status = tr!("Aba renomeada").into();
            }
        } else {
            self.erro_nome_aba = Some(tr!("Não foi possível renomear a aba.").into());
            self.status = tr!("Não foi possível renomear a aba.").into();
        }
        Task::none()
    }

    pub(super) fn clicar_aba(&mut self, indice: usize) -> Task<Msg> {
        let agora = Instant::now();
        let duplo = self.ultimo_clique_aba.is_some_and(|(i, t)| {
            i == indice && agora.duration_since(t) <= std::time::Duration::from_millis(400)
        });
        self.ultimo_clique_aba = if duplo { None } else { Some((indice, agora)) };
        let selecionar = self.update(Msg::Aba(indice));
        if duplo {
            Task::batch([selecionar, self.abrir_renomeacao()])
        } else {
            selecionar
        }
    }


}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn valida_inline_sem_renomear_ate_salvar() {
        let mut a = App::default();
        let _ = a.abrir_renomeacao();
        // Montar a barra de abas: o editor agora reside na própria guia.
        let _: Element<'_, Msg> = a.view_abas();
        for (nome, erro) in [("   ", "vazio"), ("A!B", "conter")] {
            let _ = a.update(Msg::NomearAba(nome.into()));
            let _ = a.salvar_nome_aba();
            assert!(a.editando_nome_aba);
            assert!(a.erro_nome_aba.as_ref().unwrap().contains(erro));
            assert_eq!(a.workbook.nomes()[0], "Planilha1");
        }
        a.cancelar_renomeacao();
        assert!(!a.editando_nome_aba);
        assert!(!a.alterado);
    }
    #[test]
    fn menu_ocultar_e_reexibir_nao_apaga_dados() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "42".into());
        app.workbook.criar_aba("Visível");
        let opcoes = app.opcoes_contexto(Contexto::Aba(0));
        assert!(opcoes.iter().any(|o| o.as_ref()
            .is_some_and(|(rotulo, _)| rotulo == "Ocultar aba")));
        let _ = app.update(Msg::OcultarAba(0));
        assert!(app.workbook.aba(0).unwrap().sheet.oculta);
        assert_eq!(app.folha, 1);
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::Value::Number(42.0));
        assert!(app.opcoes_contexto(Contexto::Aba(1)).iter().any(|o| o.as_ref()
            .is_some_and(|(rotulo, _)| rotulo == "Reexibir abas…")));
        let _ = app.update(Msg::MostrarAbasOcultas);
        assert!(app.reexibir_abas_aberto);
        let _ = app.update(Msg::ReexibirAba(0));
        assert!(!app.workbook.aba(0).unwrap().sheet.oculta);
        assert_eq!(app.folha, 0);
        assert!(app.alterado);
    }

    #[test]
    fn menu_de_contexto_renomeia_aba_selecionada_no_proprio_rodape() {
        let mut app = App::default();
        app.workbook.criar_aba("Outra");
        app.update(Msg::AbrirContexto(Contexto::Aba(1), 0.0, 0.0));
        let indice = app.opcoes_contexto(Contexto::Aba(1)).iter()
            .position(|op| op.as_ref().is_some_and(|(rotulo, _)| rotulo == "Renomear aba"))
            .unwrap();
        app.update(Msg::ContextoEscolher(indice));
        assert_eq!(app.folha, 1);
        assert!(app.editando_nome_aba);
        assert_eq!(app.nome_aba, "Outra");
        let alvo = app.workbook.aba(1).unwrap().id;
        assert_eq!(app.aba_em_renomeacao, Some(alvo));
        let _: Element<'_, Msg> = app.view_abas();
        app.update(Msg::NomearAba("Relatório 2026".into()));
        app.update(Msg::RenomearAba);
        assert_eq!(app.workbook.nomes(), vec!["Planilha1", "Relatório 2026"]);
        assert!(!app.editando_nome_aba);
    }

    #[test]
    fn salvar_enter_cancelar_esc_e_nome_invalido_preservam_fluxo() {
        let mut app = App::default();
        let id = window::novo_id();
        app.janelas.principal = Some(id);
        app.update(Msg::EditarNomeAba);
        app.update(Msg::NomearAba(" ".into()));
        app.update(Msg::RenomearAba);
        assert!(app.editando_nome_aba);
        assert_eq!(app.workbook.nomes()[0], "Planilha1");
        app.update(Msg::NomearAba("Financeiro".into()));
        app.update(Msg::Teclado(id,
            keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::default(),
        ));
        assert_eq!(app.workbook.nomes()[0], "Financeiro");
        assert!(!app.editando_nome_aba);
        app.update(Msg::EditarNomeAba);
        app.update(Msg::NomearAba("Não salvar".into()));
        app.update(Msg::Teclado(id,
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::default(),
        ));
        assert_eq!(app.workbook.nomes()[0], "Financeiro");
        assert!(!app.editando_nome_aba);
    }

    #[test]
    fn mudar_de_guia_confirma_nome_sem_dialogo_e_nome_invalido_impede_troca() {
        let mut app = App::default();
        app.workbook.criar_aba("Dados");
        app.update(Msg::EditarNomeAba);
        app.update(Msg::NomearAba("Relatório".into()));
        app.update(Msg::Aba(1));
        assert_eq!(app.workbook.nomes(), vec!["Relatório", "Dados"]);
        assert_eq!(app.folha, 1);
        assert!(!app.editando_nome_aba);
        app.update(Msg::EditarNomeAba);
        app.update(Msg::NomearAba("relatório".into()));
        app.update(Msg::Aba(0));
        assert!(app.editando_nome_aba);
        assert_eq!(app.folha, 1);
        assert!(app.erro_nome_aba.as_ref().unwrap().contains("Já existe"));
        assert_eq!(app.workbook.nomes(), vec!["Relatório", "Dados"]);
        // Escape descarta o rascunho e restaura a guia.
        app.update(Msg::CancelarNomeAba);
        assert_eq!(app.nome_aba, "Dados");
        assert!(!app.editando_nome_aba);
    }

    #[test]
    fn clique_na_grade_confirma_renomeacao_inline_sem_popup() {
        let mut app = App::default();
        app.update(Msg::EditarNomeAba);
        app.update(Msg::NomearAba("Resumo anual".into()));
        let _: Element<'_, Msg> = app.view_abas();
        app.update(Msg::Selecionar(2, 3));
        assert_eq!(app.workbook.nomes()[0], "Resumo anual");
        assert_eq!((app.linha, app.coluna), (2, 3));
        assert!(!app.editando_nome_aba);
    }

    #[test]
    fn duplo_clique_e_nome_repetido_preservam_motor() {
        let mut a = App::default();
        let _ = a.clicar_aba(0);
        assert!(!a.editando_nome_aba);
        let _ = a.clicar_aba(0);
        assert!(a.editando_nome_aba);
        a.workbook.criar_aba("Resultados");
        let _ = a.update(Msg::NomearAba("resultados".into()));
        let _ = a.salvar_nome_aba();
        assert!(a.erro_nome_aba.as_ref().unwrap().contains("Já existe"));
        let _ = a.update(Msg::NomearAba("  Resumo  ".into()));
        let _ = a.salvar_nome_aba();
        assert_eq!(a.workbook.nomes()[0], "Resumo");
        assert!(!a.editando_nome_aba);
        assert!(a.alterado);
    }
}
