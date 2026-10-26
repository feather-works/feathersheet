//! Menu flutuante de Colar especial, ancorado na seta do grupo Transferência.
//! O painel não participa do layout da Ribbon: ele flutua sobre a planilha.
use super::*;

const LARGURA: f32 = 278.0;
const ALTURA_PREVISTA: f32 = 280.0;
const X: f32 = 8.0;
// 43 px de cromo + 52 px das abas + botão Colar (ícone e seta).
const Y_ABAIXO_SETA: f32 = 43.0 + 52.0 + 64.0;

const OPCOES: &[(&str, ModoColagem)] = &[
    ("Transposta", ModoColagem::Transposta),
    ("Valores com formatação", ModoColagem::ValoresComFormato),
    ("Valores sem formatação", ModoColagem::SomenteValores),
    ("Fórmulas sem formatação", ModoColagem::FormulasSemFormato),
    ("Fórmulas com formatação (padrão)", ModoColagem::Completa),
    ("Somente formatação", ModoColagem::SomenteFormato),
];

impl App {
    fn menu_colagem(&self) -> Element<'_, Msg> {
        let mut itens = pelicano::column![
            text(tr!("Colar especial")).size(12).color(COR_TEXTO_MUTED()),
        ].padding(6).spacing(2);
        for &(titulo, modo) in OPCOES {
            let linha = pelicano::row![
                feather_ui::icones::icone("clipboard", 16.0),
                text(tr!(titulo)).size(12.5),
            ].spacing(9).align(pelicano::Align::Center);
            itens = itens.push(
                button(linha)
                    .width(Length::FILL)
                    .padding([8, 10])
                    .style(estilo_item_menu(false, true))
                    .on_press(Msg::ColarEspecialSolicitar(modo)),
            );
        }
        itens = itens.push(
            button(pelicano::row![
                feather_ui::icones::icone("clipboard", 16.0),
                text(tr!("Colar especial…")).size(12.5),
                Space::new(Length::FILL, 0.0),
                text("Ctrl+Alt+V").size(11).color(COR_TEXTO_MUTED()),
            ].spacing(9).align(pelicano::Align::Center))
                .width(Length::FILL).padding([8, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::ColarEspecialJanela(colar_especial::Evento::Abrir)),
        );
        container(itens)
            .padding(6)
            .width(Length::Fixed(LARGURA))
            .style(estilo_popup_menu)
            .into()
    }

    pub(super) fn sobrepor_menu_colagem<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if !self.colar_menu_aberto { return base; }
        // Fecha ao clicar fora, sem modificar células ou deslocar a Ribbon.
        let fora = pelicano::mouse_area(
            Space::new(Length::FILL, Length::FILL)
        ).on_press(Msg::FecharMenuColagem);
        let y = Y_ABAIXO_SETA.min(
            (self.tamanho_principal.height - ALTURA_PREVISTA - 8.0).max(8.0)
        );
        let x = X.min((self.tamanho_principal.width - LARGURA - 8.0).max(0.0));
        pelicano::stack![
            base,
            fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.menu_colagem(),
                ],
            ],
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn menu_tem_seis_opcoes_e_mantem_colagem_completa_como_padrao() {
        assert_eq!(OPCOES.len(), 6);
        assert!(OPCOES.iter().any(|(s,m)| *s == "Fórmulas com formatação (padrão)"
            && *m == ModoColagem::Completa));
        assert!(feather_ui::icones::existe("chevron-down"));
    }

    #[test]
    fn seta_abre_menu_sem_alterar_aba_e_escape_ou_selecao_o_fecham() {
        let mut app = App::default();
        let atual = app.faixa_ativa;
        assert!(!app.colar_menu_aberto);
        app.update(Msg::AlternarMenuColagem);
        assert!(app.colar_menu_aberto);
        let _: Element<'_, Msg> = app.menu_colagem();
        app.update(Msg::FecharMenuColagem);
        assert!(!app.colar_menu_aberto);
        app.update(Msg::AlternarMenuColagem);
        app.update(Msg::ColarEspecialSolicitar(ModoColagem::Transposta));
        assert!(!app.colar_menu_aberto);
        assert_eq!(app.faixa_ativa, atual);
    }
}
