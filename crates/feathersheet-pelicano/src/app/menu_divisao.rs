//! Menu flutuante do ícone "Dividir texto". Nenhum pick_list na Ribbon.
use super::*;

const LARGURA: f32 = 238.0;
const ALTURA_NORMAL: f32 = 300.0;
const ALTURA_OUTRO: f32 = 360.0;

impl App {
    fn menu_divisao(&self) -> Element<'_, Msg> {
        let mut itens = pelicano::column![
            text(tr!("Dividir texto por")).size(12).color(COR_TEXTO_MUTED()),
        ].spacing(3).padding(6);
        itens = itens.push(button(row![
            feather_ui::icones::icone("columns", 17.0),
            text(tr!("Assistente completo · 4 etapas")).size(12.5),
        ].spacing(10).align(pelicano::Align::Center))
            .width(Length::FILL).padding([10, 10])
            .style(estilo_item_menu(false, true))
            .on_press(Msg::AbrirAssistenteDivisao));
        for (rotulo, separador) in [
            (tr!("Ponto e vírgula  ;"), ";"),
            (tr!("Vírgula  ,"), ","),
            (tr!("Ponto  ."), "."),
            (tr!("Espaço"), tr!("Espaço")),
        ] {
            itens = itens.push(button(row![
                feather_ui::icones::icone("columns", 16.0),
                text(rotulo).size(12.5),
            ].spacing(10).align(pelicano::Align::Center))
                .width(Length::FILL).padding([7, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::FaixaOpcao("dividir_texto", separador)));
        }
        itens = itens.push(button(row![
            feather_ui::icones::icone("edit", 16.0),
            text(tr!("Outro…")).size(12.5),
        ].spacing(10).align(pelicano::Align::Center))
            .width(Length::FILL).padding([7, 10])
            .style(estilo_item_menu(self.divisao_personalizada, true))
            .on_press(Msg::FaixaOpcao("dividir_texto", "Outro…")));
        if self.divisao_personalizada {
            itens = itens.push(row![
                text_input(tr!("Digite o separador"), &self.divisao_outro)
                    .id("feathersheet.dividir.outro")
                    .on_input(Msg::DividirTextoOutro)
                    .on_submit(Msg::DividirTextoAplicar)
                    .width(Length::FILL).size(12).padding([5, 7]),
                button(tr!("Aplicar"))
                    .on_press(Msg::DividirTextoAplicar)
                    .style(estilo_botao_primario)
                    .padding([5, 8]),
            ].spacing(5).align(pelicano::Align::Center));
        }
        container(itens)
            .width(Length::Fixed(LARGURA)).padding(6)
            .style(estilo_popup_menu).into()
    }

    pub(super) fn sobrepor_menu_divisao<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if !self.divisao_menu_aberto { return base; }
        let fora = pelicano::mouse_area(
            Space::new(Length::FILL, Length::FILL)
        ).on_press(Msg::FecharMenuDivisao);
        let altura = if self.divisao_personalizada { ALTURA_OUTRO } else { ALTURA_NORMAL };
        let x = self.divisao_popup_x.clamp(
            8.0, (self.tamanho_principal.width - LARGURA - 8.0).max(8.0));
        let y = self.divisao_popup_y.clamp(
            8.0, (self.tamanho_principal.height - altura - 8.0).max(8.0));
        stack![
            base, fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.menu_divisao(),
                ],
            ],
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    #[test]
    fn clique_no_icone_abre_o_menu_flutuante() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Dados;
        app.ponteiro_x = 160.0;
        app.ponteiro_y = 120.0;
        app.update(Msg::AlternarMenuDivisao);
        assert!(app.divisao_menu_aberto);
        assert_eq!(app.divisao_popup_x, 120.0);
        assert_eq!(app.divisao_popup_y, 144.0);
        let _: Element<'_, Msg> = app.menu_divisao();
        app.update(Msg::AlternarMenuDivisao);
        assert!(!app.divisao_menu_aberto);
    }

    #[test]
    fn outro_aparece_no_menu_e_aplica_separador() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "a||b||c".into());
        app.update(Msg::AlternarMenuDivisao);
        app.update(Msg::FaixaOpcao("dividir_texto", "Outro…"));
        assert!(app.divisao_menu_aberto && app.divisao_personalizada);
        app.update(Msg::DividirTextoOutro("||".into()));
        app.update(Msg::DividirTextoAplicar);
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("c".into()));
        assert!(!app.divisao_menu_aberto);
    }

    #[test]
    fn opcao_direta_fecha_menu_e_trocar_aba_tambem() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "João;25;Vitória".into());
        app.update(Msg::AlternarMenuDivisao);
        app.update(Msg::FaixaOpcao("dividir_texto", ";"));
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("Vitória".into()));
        assert!(!app.divisao_menu_aberto);
        app.update(Msg::AlternarMenuDivisao);
        app.update(Msg::AbaFaixa(AbaFaixa::Inicio));
        assert!(!app.divisao_menu_aberto);
    }
}
