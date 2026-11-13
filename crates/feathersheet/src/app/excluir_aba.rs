//! Confirmação de "Excluir aba" quando a aba tem dados: a exclusão não entra no Ctrl+Z.
use super::*;

impl App {
    pub(super) fn sobrepor_excluir_aba<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::dialogo_interno::dlg;
        if !self.excluir_aba_pedida { return base; }
        let nome = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
        let corpo = text(trf!("A aba \"{}\" tem dados. Excluir a aba apaga esses dados e não dá para desfazer.", nome))
            .size(12.5).line_height(1.4);
        self.dialogo(tr!("Excluir aba"), corpo)
            .icone("alert-triangle")
            .largura(440.0)
            .secundario(tr!("Cancelar"), Some(Msg::RemoverAbaCancelar))
            .botao(dlg::botao_primario(tr!("Excluir"), Some(Msg::RemoverAbaConfirmada)))
            .ao_fechar(Msg::RemoverAbaCancelar)
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn excluir_aba_com_dados_pergunta_antes() {
        let mut app = App::default();
        app.workbook.criar_aba("Outra");
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.folha = 0;
        let _ = app.update(Msg::RemoverAba);
        assert!(app.excluir_aba_pedida, "aba com dado pediu confirmação");
        assert_eq!(app.workbook.len(), 2, "nada foi excluído ainda");
        let _ = app.update(Msg::RemoverAbaCancelar);
        assert_eq!(app.workbook.len(), 2);
        let _ = app.update(Msg::RemoverAba);
        let _ = app.update(Msg::RemoverAbaConfirmada);
        assert_eq!(app.workbook.len(), 1, "confirmado, excluiu");
    }
}
