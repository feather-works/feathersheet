//! Seleção de várias áreas, como no Excel: Ctrl+clique acrescenta uma área,
//! Ctrl+arrastar estende a nova, e clique comum (ou seta) volta a uma só.
//! A área ativa continua em `linha/coluna/fim_selecao`; as anteriores ficam
//! em `selecoes_extras`. Apagar, formatar e Ctrl+Enter valem para todas.
use super::*;

/// Acima disto as áreas extras não são guardadas: cada uma é desenhada e
/// percorrida a cada comando.
const TETO_AREAS: usize = 10_000;

impl App {
    /// Todas as áreas selecionadas, a ativa por último.
    pub(super) fn areas_selecionadas(&self) -> Vec<Rect> {
        let mut areas = self.selecoes_extras.clone();
        areas.push(self.area_selecionada());
        areas
    }

    pub(super) fn tem_varias_areas(&self) -> bool { !self.selecoes_extras.is_empty() }

    /// Ctrl+clique: a área atual vira extra e a célula clicada passa a ser
    /// a ativa (e o começo da nova área, se a pessoa arrastar).
    pub(super) fn selecionar_adicional(&mut self, r: u32, c: u32) {
        if !self.confirmar() { return; }
        let anterior = self.area_selecionada();
        let mut extras = std::mem::take(&mut self.selecoes_extras);
        if extras.len() < TETO_AREAS && !extras.contains(&anterior) { extras.push(anterior); }
        self.trocar_selecao(r, c);
        self.selecoes_extras = extras;
        self.status = trf!("{} áreas selecionadas", self.selecoes_extras.len() + 1);
    }

    /// Substitui a seleção por estas áreas (Ir para especial). A última
    /// vira a ativa.
    pub(super) fn selecionar_areas(&mut self, mut areas: Vec<Rect>) {
        let Some(ativa) = areas.pop() else { return };
        self.trocar_selecao(ativa.r0, ativa.c0);
        self.fim_selecao = (ativa.r1, ativa.c1);
        areas.truncate(TETO_AREAS);
        self.selecoes_extras = areas;
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    #[test]
    fn ctrl_clique_acumula_areas_e_clique_comum_volta_a_uma() {
        let mut app = App::default();
        let _ = app.update(Msg::IrPara("A1:B2".into()));
        let _ = app.update(Msg::SelecionarAdicional(4, 3));
        let _ = app.update(Msg::Estender(5, 3));
        let areas = app.areas_selecionadas();
        assert_eq!(areas, vec![Rect::new(0, 0, 1, 1), Rect::new(4, 3, 5, 3)]);
        let _ = app.update(Msg::Selecionar(9, 9));
        assert!(!app.tem_varias_areas());
    }

    #[test]
    fn apagar_e_formatar_valem_para_todas_as_areas_num_desfazer() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "a".into());
        app.workbook.escrever_celula(0, 5, 5, "b".into());
        app.workbook.escrever_celula(0, 3, 3, "fica".into());
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::SelecionarAdicional(5, 5));
        app.aplicar_estilo_menu("bold", Campo::Logico(true));
        let negrito = |app: &App, r, c| app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(r, c).is_some_and(|s| s.bold);
        assert!(negrito(&app, 0, 0) && negrito(&app, 5, 5) && !negrito(&app, 3, 3));
        let _ = app.update(Msg::Limpar);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Empty);
        assert_eq!(app.workbook.valor(0, 5, 5), Value::Empty);
        assert_eq!(app.workbook.valor(0, 3, 3), Value::Text("fica".into()));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("a".into()));
        assert_eq!(app.workbook.valor(0, 5, 5), Value::Text("b".into()));
    }

    #[test]
    fn copiar_com_varias_areas_avisa_e_nao_copia() {
        let mut app = App::default();
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::SelecionarAdicional(2, 2));
        assert!(app.copiar_selecao().is_none());
        assert!(app.status.contains("várias"));
    }
}
