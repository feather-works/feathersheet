//! Menu de orientação da Ribbon: overlay real, não PickList.
use super::*;

const LARGURA: f32 = 306.0;
const ALTURA: f32 = 334.0;
pub(super) const OPCOES: &[(&str, &str)] = &[
    ("Horário", "Inclinar texto no sentido horário"),
    ("Anti-horário", "Inclinar texto no sentido anti-horário"),
    ("Vertical", "Texto vertical"),
    ("Para cima", "Girar texto para cima"),
    ("Para baixo", "Girar texto para baixo"),
    ("Normal", "Orientação normal (0°)"),
];

impl App {
    pub(super) fn escolher_orientacao(&mut self, opcao: &'static str) -> Task<Msg> {
        self.orientacao_menu_aberto = false;
        if opcao == "Formatar células…" {
            return self.abrir_formatar_celulas(formatar_celulas::Aba::Alinhamento);
        }
        let (rotation, vertical) = match opcao {
            "Horário" => (45, false),
            "Anti-horário" => (-45, false),
            "Vertical" => (0, true),
            "Para cima" => (-90, false),
            "Para baixo" => (90, false),
            "Normal" => (0, false),
            _ => return Task::none(),
        };
        self.confirmar();
        let area = self.area_selecionada();
        let mut patch = EstiloParcial::new();
        patch.insert("rotation".into(), if rotation == 0 {
            Campo::Nulo
        } else { Campo::Numero(rotation as f64) });
        patch.insert("textVertical".into(), Campo::Logico(vertical));
        self.workbook.editar(self.folha, |sheet| sheet.set_style(&area, &patch));
        self.alterado = true;
        self.status = trf!("Orientação aplicada: {opcao}");
        Task::none()
    }

    fn menu_orientacao(&self) -> Element<'_, Msg> {
        let mut items = pelicano::column![text(tr!("Orientação do texto"))
            .size(12).color(COR_TEXTO_MUTED())].spacing(2).padding(6);
        for &(acao, rotulo) in OPCOES {
            let icone = match acao {
                "Horário" => "arrow-down-right", "Anti-horário" => "arrow-up-right",
                "Vertical" => "arrows-vertical", "Para cima" => "arrow-up",
                "Para baixo" => "arrow-down", _ => "arrow-right",
            };
            // Traço Tabler sempre: `arrow-down` também é nome de um ícone
            // colorido da faixa, e aparecia um quadradinho no lugar da seta.
            items = items.push(button(pelicano::row![
                container(feather_ui::icones::icone_traco(icone, 17.0)).width(Length::Fixed(24.0)),
                text(tr!(rotulo)).size(12),
            ].align(pelicano::Align::Center).spacing(7))
                .width(Length::FILL).padding([7, 9])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::OrientacaoEscolher(acao)));
        }
        items = items.push(
            button(pelicano::row![
                feather_ui::icones::icone("settings", 17.0),
                text(tr!("Formatar células…")).size(12),
            ].spacing(10).align(pelicano::Align::Center))
                .width(Length::FILL).padding([9, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::OrientacaoEscolher("Formatar células…"))
        );
        container(items).width(Length::Fixed(LARGURA))
            .padding(5).style(estilo_popup_menu).into()
    }

    pub(super) fn sobrepor_menu_orientacao<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if !self.orientacao_menu_aberto { return base; }
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::FecharMenuOrientacao);
        let x = self.orientacao_popup_x.clamp(8., (self.tamanho_principal.width-LARGURA-8.).max(8.));
        let y = self.orientacao_popup_y.clamp(8., (self.tamanho_principal.height-ALTURA-8.).max(8.));
        stack![
            base, fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![Space::new(Length::Fixed(x), 0.0), self.menu_orientacao()],
            ]
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn menu_aplica_direcoes_e_fecha_sem_criar_dropdown_na_ribbon() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuOrientacao);
        assert!(app.orientacao_menu_aberto);
        app.update(Msg::OrientacaoEscolher("Anti-horário"));
        assert!(!app.orientacao_menu_aberto);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0,0).unwrap().rotation, Some(-45));
        app.update(Msg::OrientacaoEscolher("Vertical"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_mesclado(0,0).unwrap().get("textVertical"),
            Some(&Campo::Logico(true)));
    }

    #[test]
    fn icones_do_menu_existem_no_catalogo() {
        for i in ["arrow-down-right", "arrow-up-right", "arrows-vertical", "arrow-up", "arrow-down", "arrow-right", "settings"] {
            assert!(feather_ui::icones::existe(i), "{i}");
        }
    }
}
