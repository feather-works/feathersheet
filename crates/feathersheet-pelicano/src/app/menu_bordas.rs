//! Menu suspenso de bordas da Ribbon: botão único e prévias como no Excel.
//! Não utiliza pick_list: cada tipo apresenta a própria geometria em SVG.
use super::*;

const LARGURA: f32 = 240.0;
/// Opções de borda (346 px) + caneta de borda (desenhar, cor e estilo).
const ALTURA_PREVISTA: f32 = 346.0 + 240.0;

pub(super) const OPCOES: &[&str] = &[
    "Todas", "Externas", "Horizontais internas", "Verticais internas",
    "Superior", "Inferior", "Esquerda", "Direita", "Nenhuma",
];

/// Contorno discreto da célula + traços fortes somente nas bordas selecionadas.
/// Evita reutilizar um mesmo pictograma para ações visualmente diferentes.
fn desenho_borda(opcao: &str) -> String {
    let tracos = match opcao {
        "Todas" => "M4 4H20V20H4Z M12 4V20 M4 12H20",
        "Externas" => "M4 4H20V20H4Z",
        "Horizontais internas" => "M4 12H20",
        "Verticais internas" => "M12 4V20",
        "Superior" => "M4 4H20",
        "Inferior" => "M4 20H20",
        "Esquerda" => "M4 4V20",
        "Direita" => "M20 4V20",
        "Nenhuma" => "",
        _ => "",
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke-linecap="square">
        <path d="M4 4H20V20H4Z M12 4V20 M4 12H20" stroke="#89929d" stroke-width="0.9" stroke-dasharray="2 2" opacity="0.65"/>
        <path d="{tracos}" stroke="#808080" stroke-width="2.2"/>
        </svg>"##
    )
}

pub(super) fn icone_borda(opcao: &str, tamanho: f32) -> pelicano::SvgView {
    // O SVG é montado aqui: falhar é erro de programação.
    pelicano::svg(pelicano::Svg::from_bytes(
        desenho_borda(opcao).as_bytes(),
    ).expect("SVG da prévia de borda"))
    .width(tamanho)
    .height(tamanho)
    .color(COR_TEXTO())
}

/// Nome do estilo da linha na tela. "Média" aqui é espessura (Medium), não a
/// conta (Average) que a mesma palavra traduz no resto da interface.
pub(crate) fn nome_estilo_linha(estilo: &str) -> &str {
    if estilo == "Média" && crate::idioma::lingua() != crate::idioma::Lingua::Pt {
        return tr!("Média (espessura de linha)");
    }
    tr!(estilo)
}

/// Amostra do estilo da linha da caneta de borda (Fina, Média, Grossa, Tracejada).
fn desenho_estilo_linha(estilo: &str) -> String {
    let (largura, traco) = match estilo {
        "Média" => (2.5, ""),
        "Grossa" => (4.0, ""),
        "Tracejada" => (1.6, "stroke-dasharray=\"4 3\""),
        _ => (1.2, ""),
    };
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="16" viewBox="0 0 40 16" fill="none">
        <path d="M3 8H37" stroke="#808080" stroke-width="{largura}" {traco}/></svg>"##)
}

fn icone_estilo_linha(estilo: &str) -> pelicano::SvgView {
    pelicano::svg(pelicano::Svg::from_bytes(
        desenho_estilo_linha(estilo).as_bytes(),
    ).expect("SVG do estilo de linha"))
    .width(40.0)
    .height(16.0)
    .color(COR_TEXTO())
}

impl App {
    /// Seção "Desenhar bordas" do menu: os três modos da caneta, a cor e o
    /// estilo da linha, como no Excel.
    fn menu_bordas_caneta(&self) -> Element<'_, Msg> {
        use feathersheet_core::operacoes::desenho_borda::{ModoBorda, ESTILOS};
        use desenho_borda_ui::{Evento, CORES};
        let estado = &self.borda_desenho;
        let mut coluna = pelicano::column![
            container(pelicano::horizontal_rule(1.0)).padding([4, 0]),
            container(text(tr!("Desenhar bordas")).size(12).color(COR_TEXTO_MUTED())).padding([5, 10]),
        ].spacing(1);
        for (modo, icone, dica) in [
            (ModoBorda::Contorno, "pencil", tr!("Desenhar borda")),
            (ModoBorda::Grade, "layout-grid", tr!("Desenhar grade de borda")),
            (ModoBorda::Apagar, "eraser", tr!("Apagar borda")),
        ] {
            let ativo = estado.modo == Some(modo);
            coluna = coluna.push(
                button(row![
                    feather_ui::icones::icone(icone, 18.0),
                    text(dica).size(12.5),
                ].spacing(11).align(pelicano::Align::Center))
                    .width(Length::FILL)
                    .padding([7, 10])
                    .style(estilo_item_menu(ativo, true))
                    .on_press(Msg::BordaDesenho(Evento::Modo(modo))),
            );
        }
        let atual = estado.caneta.cor.as_deref();
        let mut cores = row(Vec::<pelicano::Element<_>>::new()).spacing(5).align(pelicano::Align::Center);
        for &(nome, cor) in CORES {
            let escolhida = atual == cor;
            let cor_iced = cor.filter(|c| seletor_cor::hex_valido(c)).map(|c| {
                let [r, g, b, a] = seletor_cor::rgba_de(c);
                pelicano::Color::rgb8(r, g, b).alpha(a as f32 / 255.0)
            });
            // A automática é a amostra sem cor com "A" (segue o tema, como no Excel).
            let rotulo: Element<'_, Msg> = if cor.is_none() {
                text("A").size(11).color(COR_TEXTO()).into()
            } else {
                Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
            };
            let amostra = container(rotulo)
                .width(Length::Fixed(16.0)).align_x(pelicano::Align::Center).height(Length::Fixed(16.0)).align_y(pelicano::Align::Center)
                .style(move |_| pelicano::Style {
                    background: cor_iced.map(pelicano::Background::Color),
                    border: pelicano::Border {
                        color: if escolhida { COR_AZUL_ACENTO() } else { COR_BORDA() },
                        width: if escolhida { 2.0 } else { 1.0 },
                        radius: [3.0; 4],
                    },
                    ..Default::default()
                });
            cores = cores.push(feather_ui::formatacao::dica(
                button(amostra).padding(2).style(estilo_botao_pequeno(escolhida))
                    .on_press(Msg::BordaDesenho(Evento::Cor(cor))),
                tr!(nome),
            ));
        }
        coluna = coluna.push(container(text(tr!("Cor da linha")).size(11.5).color(COR_TEXTO_MUTED())).padding([4, 10]));
        coluna = coluna.push(container(cores).padding([0, 8]));
        let estilo_atual = estado.caneta.estilo.as_deref().unwrap_or("Fina");
        let mut estilos = row(Vec::<pelicano::Element<_>>::new()).spacing(4).align(pelicano::Align::Center);
        for estilo in ESTILOS {
            estilos = estilos.push(feather_ui::formatacao::dica(
                button(icone_estilo_linha(estilo)).padding([4, 3])
                    .style(estilo_botao_pequeno(estilo_atual == estilo))
                    .on_press(Msg::BordaDesenho(Evento::Estilo(estilo))),
                nome_estilo_linha(estilo),
            ));
        }
        coluna = coluna.push(container(text(tr!("Estilo da linha")).size(11.5).color(COR_TEXTO_MUTED())).padding([6, 10]));
        coluna = coluna.push(container(estilos).padding([0, 8]));
        coluna.into()
    }

    fn menu_bordas(&self) -> Element<'_, Msg> {
        let mut itens = pelicano::column![
            container(text(tr!("Bordas")).size(12).color(COR_TEXTO_MUTED()))
                .padding([7, 10]),
        ]
        .spacing(1);

        for &opcao in OPCOES {
            itens = itens.push(
                button(row![
                    icone_borda(opcao, 20.0),
                    text(tr!(opcao)).size(12.5),
                ]
                .spacing(11)
                .align(pelicano::Align::Center))
                .width(Length::FILL)
                .padding([7, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::FaixaOpcao("borda", opcao)),
            );
        }

        itens = itens.push(self.menu_bordas_caneta());

        let altura = ALTURA_PREVISTA.min((self.tamanho_principal.height - 32.0).max(100.0));
        container(
            pelicano::scrollable(itens)
                .height(Length::Fixed(altura)),
        )
        .width(Length::Fixed(LARGURA))
        .padding(5)
        .style(estilo_popup_menu)
        .into()
    }

    pub(super) fn sobrepor_menu_bordas<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if !self.bordas_menu_aberto { return base; }

        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::FecharMenuBordas);
        let altura = ALTURA_PREVISTA.min((self.tamanho_principal.height - 32.0).max(100.0)) + 10.0;
        let x = self.bordas_popup_x.clamp(
            8.0, (self.tamanho_principal.width - LARGURA - 8.0).max(8.0),
        );
        let y = self.bordas_popup_y.clamp(
            8.0, (self.tamanho_principal.height - altura - 8.0).max(8.0),
        );
        stack![
            base,
            fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.menu_bordas(),
                ],
            ],
        ]
        .height(Length::FILL)
        .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cada_tipo_de_borda_tem_icone_proprio() {
        assert_eq!(OPCOES.len(), 9);
        let desenhos: Vec<String> = OPCOES.iter().map(|opcao| desenho_borda(opcao)).collect();
        for (i, desenho) in desenhos.iter().enumerate() {
            assert!(desenho.contains("<svg"), "SVG ausente na opção {}", OPCOES[i]);
            assert!(desenho.contains("stroke-width=\"2.2\""));
            assert!(!desenho.contains("href="));
            for outro in desenhos.iter().skip(i + 1) {
                assert_ne!(desenho, outro, "duas bordas com o mesmo desenho");
            }
        }
        assert!(desenho_borda("Nenhuma").contains("d=\"\""));
    }

    #[test]
    fn botao_abre_menu_e_opcao_aplica_sem_deixar_menu_aberto() {
        let mut app = App::default();
        app.ponteiro_x = 345.0;
        app.ponteiro_y = 155.0;
        app.update(Msg::AlternarMenuBordas);
        assert!(app.bordas_menu_aberto);
        assert_eq!(app.bordas_popup_x, 325.0);
        assert_eq!(app.bordas_popup_y, 179.0);
        let _: Element<'_, Msg> = app.menu_bordas();
        app.update(Msg::FaixaOpcao("borda", "Inferior"));
        assert!(!app.bordas_menu_aberto);
        app.update(Msg::AlternarMenuBordas);
        app.update(Msg::FecharMenuBordas);
        assert!(!app.bordas_menu_aberto);
    }
}
