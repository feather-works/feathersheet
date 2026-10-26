//! Reorganização de abas na interface Iced, com identidade estável por ID.
//! Não modifica células: o `Workbook` já guarda IDs e ordem física.
use super::*;

impl App {
    /// Posiciona a aba na lista da pasta mantendo ativa a mesma identidade.
    /// Recebe índices físicos do momento do clique, inclusive com abas ocultas.
    pub(super) fn mover_aba_para(&mut self, origem: usize, destino: usize) -> Task<Msg> {
        let total = self.workbook.len();
        if origem >= total || destino >= total || origem == destino {
            return Task::none();
        }
        if self.editando_nome_aba {
            let tarefa = self.salvar_nome_aba();
            if self.editando_nome_aba { return tarefa; }
        }
        // O editor de fórmulas mantém coordenadas por índice de aba. Evitar
        // reordenar com uma referência ainda aberta para não alterá-la.
        if self.editor_formula.ativo {
            self.status = tr!("Conclua a edição da fórmula antes de mover a aba.").into();
            return Task::none();
        }
        // Consultas em edição podem armazenar índices físicos na origem.
        if self.openquery.is_some() {
            self.status = tr!("Conclua ou feche o OpenQuery antes de mover as abas.").into();
            return Task::none();
        }
        if !self.confirmar() { return Task::none(); }

        let ativa = self.workbook.aba(self.folha).map(|aba| aba.id);
        let nome = self.workbook.aba(origem)
            .map(|aba| aba.sheet.name.clone()).unwrap_or_default();
        self.workbook.mover_aba(origem, destino);
        // O índice da aba selecionada pode ter mudado mesmo ao mover outra.
        if let Some(id) = ativa {
            if let Some(nova) = self.workbook.indice_do_id(id) {
                self.folha = nova;
            }
        }
        self.nome_aba = self.workbook.aba(self.folha)
            .map(|aba| aba.sheet.name.clone()).unwrap_or_default();
        self.ultimo_clique_aba = None;
        self.contexto = None;
        self.paleta_aba_aberta = false;
        self.alterado = true;
        self.status = trf!("Aba \"{nome}\" movida para a posição {}.", destino + 1);
        Task::none()
    }

    /// Índices das guias que o usuário enxerga; as ocultas permanecem na pasta,
    /// mas não contam como vizinhas visuais do movimento.
    pub(super) fn vizinhas_visiveis(&self, origem: usize) -> (Option<usize>, Option<usize>) {
        let visiveis: Vec<_> = self.workbook.abas_visiveis().collect();
        let Some(pos) = visiveis.iter().position(|&i| i == origem) else {
            return (None, None);
        };
        (pos.checked_sub(1).map(|p| visiveis[p]), visiveis.get(pos + 1).copied())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn mover_aba_preserva_ativa_formula_cores_e_ordem() {
        let mut app = App::default();
        let dados = app.workbook.criar_aba("Dados");
        let resumo = app.workbook.criar_aba("Resumo");
        app.workbook.escrever_celula(dados, 0, 0, "27".into());
        app.workbook.escrever_celula(resumo, 0, 0, "=Dados!A1".into());
        app.workbook.aba_mut(dados).unwrap().sheet.tab_color = Some("#00aa44".into());
        let id_dados = app.workbook.aba(dados).unwrap().id;
        app.folha = resumo;
        let id_ativo = app.workbook.aba(app.folha).unwrap().id;
        let _ = app.update(Msg::MoverAba(dados, 0));
        assert_eq!(app.workbook.nomes(), vec!["Dados", "Planilha1", "Resumo"]);
        assert_eq!(app.workbook.aba(app.folha).unwrap().id, id_ativo);
        assert_eq!(app.workbook.indice_do_id(id_dados), Some(0));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.tab_color.as_deref(), Some("#00aa44"));
        assert_eq!(app.workbook.valor(app.folha, 0, 0), Value::Number(27.0));
        assert!(app.alterado);
    }

    #[test]
    fn movimentos_ignoram_abas_ocultas_e_arquivo_grava_a_nova_ordem() {
        let mut app = App::default();
        app.workbook.criar_aba("Oculta");
        app.workbook.criar_aba("Visível");
        app.workbook.ocultar_aba(1).unwrap();
        assert_eq!(app.vizinhas_visiveis(0), (None, Some(2)));
        let _ = app.update(Msg::MoverAba(2, 0));
        assert_eq!(app.workbook.nomes(), vec!["Visível", "Planilha1", "Oculta"]);
        assert!(app.workbook.aba(2).unwrap().sheet.oculta);
        let dados = feathersheet_io::da_workbook::pasta_gravar_bytes(
            &app.workbook, app.folha as u32).unwrap();
        let lido = feathersheet_io::fshz_read::ler_fshz(&dados).unwrap();
        let aberto = feathersheet_io::para_workbook::para_workbook(&lido);
        assert_eq!(aberto.nomes(), app.workbook.nomes());
        assert!(aberto.aba(2).unwrap().sheet.oculta);
    }

    #[test]
    fn indices_invalidos_ou_mesmo_lugar_nao_mudam_o_arquivo() {
        let mut app = App::default();
        let _ = app.update(Msg::MoverAba(0, 0));
        let _ = app.update(Msg::MoverAba(100, 0));
        assert!(!app.alterado);
        assert_eq!(app.workbook.nomes(), vec!["Planilha1"]);
    }
}
