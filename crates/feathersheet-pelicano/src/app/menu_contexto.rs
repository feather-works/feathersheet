//! Menu suspenso com submenus (▸ abrindo ao lado), reaproveitado pelo menu
//! de contexto da célula, pelo menu da forma e pelos menus ▾ da aba
//! Editar forma. Mesmo visual do menu de célula: ícone Tabler 16 + texto
//! 13,5, `estilo_item_menu`, `estilo_popup_menu`. Item sem motor fica
//! desabilitado com a dica dizendo o que fará ("Em breve: …"), nunca some.
use super::*;
use pelicano::column; use pelicano::mouse_area; use pelicano::opaque; use pelicano::tooltip;

/// Altura de um item (padding 7 + texto 13,5 + respiro) e de um separador.
pub(super) const ALTURA_ITEM: f32 = 33.0;
pub(super) const ALTURA_SEPARADOR: f32 = 3.0;
pub(super) const LARGURA: f32 = 250.0;

#[derive(Debug, Clone)]
pub(super) enum ItemMenu {
    Acao {
        rotulo: String,
        icone: &'static str,
        /// `None` = desabilitado (com `dica`).
        msg: Option<Msg>,
        dica: Option<&'static str>,
        atalho: Option<&'static str>,
    },
    Sub { rotulo: String, icone: &'static str, itens: Vec<ItemMenu> },
    Separador,
}

impl ItemMenu {
    pub(super) fn acao(rotulo: impl Into<String>, icone: &'static str, msg: Msg) -> Self {
        Self::Acao { rotulo: rotulo.into(), icone, msg: Some(msg), dica: None, atalho: None }
    }
    pub(super) fn com_atalho(mut self, tecla: &'static str) -> Self {
        if let Self::Acao { atalho, .. } = &mut self {
            *atalho = Some(tecla);
        }
        self
    }
    /// Comando ainda sem motor: desabilitado, com a dica.
    pub(super) fn em_breve(rotulo: impl Into<String>, icone: &'static str, dica: &'static str) -> Self {
        Self::Acao { rotulo: rotulo.into(), icone, msg: None, dica: Some(dica), atalho: None }
    }
    /// Habilitado só quando `msg` existe; senão mostra `dica`.
    pub(super) fn talvez(rotulo: impl Into<String>, icone: &'static str, msg: Option<Msg>, dica: &'static str) -> Self {
        Self::Acao { rotulo: rotulo.into(), icone, msg, dica: Some(dica), atalho: None }
    }
    pub(super) fn sub(rotulo: impl Into<String>, icone: &'static str, itens: Vec<ItemMenu>) -> Self {
        Self::Sub { rotulo: rotulo.into(), icone, itens }
    }
    fn altura(&self) -> f32 {
        if matches!(self, Self::Separador) { ALTURA_SEPARADOR } else { ALTURA_ITEM }
    }
}

/// Altura total da lista (com o padding de 5 + 5).
pub(super) fn altura(itens: &[ItemMenu]) -> f32 {
    itens.iter().map(ItemMenu::altura).sum::<f32>() + 12.0
}

fn linha<'a>(icone: &'static str, rotulo: String, atalho: Option<&'static str>, sub: bool, habilitado: bool) -> Element<'a, Msg> {
    let cor = if habilitado { COR_TEXTO() } else { COR_TEXTO_MUTED() };
    let mut r = row![
        feather_ui::icones::icone(icone, 16.0),
        text(rotulo).size(13.5).color(cor),
        Space::new(Length::FILL, 0.0),
    ]
    .spacing(8)
    .align(pelicano::Align::Center);
    if let Some(a) = atalho {
        r = r.push(text(a).size(11.5).color(COR_TEXTO_MUTED()));
    }
    if sub {
        r = r.push(feather_ui::icones::icone("chevron-right", 14.0));
    }
    r.into()
}

/// A lista de itens. `aberto` = índice do submenu aberto; `abrir(i)` é a
/// mensagem que abre (Some) ou fecha (None) um submenu — ao passar o mouse
/// ou clicar no item com ▸.
pub(super) fn lista<'a>(
    itens: &[ItemMenu],
    aberto: Option<usize>,
    abrir: impl Fn(Option<usize>) -> Msg,
    largura: f32,
) -> Element<'a, Msg> {
    let mut col = column(Vec::<pelicano::Element<_>>::new()).padding(5).spacing(0);
    for (i, item) in itens.iter().enumerate() {
        match item {
            ItemMenu::Separador => {
                col = col.push(
                    container(container(Space::new(0.0, 1.0)).width(Length::FILL).style(estilo_linha))
                        .padding([1, 4]),
                );
            }
            ItemMenu::Acao { rotulo, icone, msg, dica, atalho } => {
                let habilitado = msg.is_some();
                let botao = button(linha(icone, rotulo.clone(), *atalho, false, habilitado))
                    .padding([7, 10])
                    .width(Length::FILL)
                    .style(estilo_item_menu(false, habilitado))
                    .on_press_maybe(msg.clone());
                let el: Element<'a, Msg> = mouse_area(botao).on_enter(abrir(None)).into();
                col = col.push(match dica.filter(|_| !habilitado) {
                    Some(d) => tooltip(el, d, pelicano::TooltipPosition::Right).padding([4, 8]).style(estilo_dica).into(),
                    None => el,
                });
            }
            ItemMenu::Sub { rotulo, icone, .. } => {
                let botao = button(linha(icone, rotulo.clone(), None, true, true))
                    .padding([7, 10])
                    .width(Length::FILL)
                    .style(estilo_item_menu(aberto == Some(i), true))
                    .on_press(abrir(Some(i)));
                col = col.push(mouse_area(botao).on_enter(abrir(Some(i))));
            }
        }
    }
    container(col).width(Length::Fixed(largura)).style(estilo_popup_menu).into()
}

/// Menu inteiro flutuando em (`x`, `y`) da janela, com o submenu aberto ao
/// lado (à direita, ou à esquerda se não couber). `fechar` = clique fora.
#[allow(clippy::too_many_arguments)]
pub(super) fn sobrepor<'a>(
    base: Element<'a, Msg>,
    itens: &[ItemMenu],
    aberto: Option<usize>,
    abrir: impl Fn(Option<usize>) -> Msg + Copy,
    fechar: Msg,
    (x, y): (f32, f32),
    janela: pelicano::Size,
) -> Element<'a, Msg> {
    let alto = altura(itens).min(janela.height - 16.0);
    let x = x.min(janela.width - LARGURA - 8.0).max(8.0);
    let y = y.min(janela.height - alto - 8.0).max(8.0);
    let fora = mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(fechar);
    let principal = column![Space::new(0.0, y), row![Space::new(x, 0.0), opaque(lista(itens, aberto, abrir, LARGURA))]];
    let mut camadas: Vec<Element<'a, Msg>> = vec![base, fora.into(), principal.into()];
    if let Some((i, ItemMenu::Sub { itens: filhos, .. })) = aberto.and_then(|i| itens.get(i).map(|it| (i, it))) {
        let topo: f32 = itens[..i].iter().map(ItemMenu::altura).sum::<f32>();
        let alto_sub = altura(filhos);
        let sx = if x + 2.0 * LARGURA + 4.0 <= janela.width { x + LARGURA - 2.0 } else { (x - LARGURA + 2.0).max(0.0) };
        let sy = (y + topo).min(janela.height - alto_sub - 8.0).max(8.0);
        // Dentro do submenu, passar o mouse não fecha o próprio submenu.
        let sub = lista(filhos, None, move |_| abrir(Some(i)), LARGURA);
        camadas.push(column![Space::new(0.0, sy), row![Space::new(sx, 0.0), opaque(sub)]].into());
    }
    stack(camadas).width(Length::FILL).height(Length::FILL).into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn menu_com_submenu_monta_nos_dois_lados_e_altura_conta_separador() {
        let itens = vec![
            ItemMenu::acao("Copiar", "copy", Msg::Copiar).com_atalho("Ctrl+C"),
            ItemMenu::Separador,
            ItemMenu::sub("Colar especial", "clipboard", vec![ItemMenu::acao("Colar", "clipboard", Msg::ColarSolicitar)]),
            ItemMenu::em_breve("Hiperlink…", "link", "Em breve"),
        ];
        assert_eq!(altura(&itens), 3.0 * ALTURA_ITEM + ALTURA_SEPARADOR + 12.0);
        for x in [10.0, 1200.0] {
            let _: Element<'_, Msg> = sobrepor(text("b").into(), &itens, Some(2), |_| Msg::FecharContexto, Msg::FecharContexto,
                (x, 100.0), pelicano::Size::new(1280.0, 800.0));
        }
    }
}
