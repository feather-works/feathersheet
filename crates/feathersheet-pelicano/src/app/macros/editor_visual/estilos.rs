//! Paleta e controles compactos, isolados da Ribbon global.
use super::*;


pub(in crate::app) fn cor_workspace(escuro: bool) -> Color {
    if escuro { Color::rgb8(24, 30, 40) } else { Color::rgb8(242, 246, 251) }
}
/// A mesma cor de destaque selecionada em Configurações > Aparência.
pub(in crate::app) fn cor_acento() -> Color {
    feather_ui::tema::COR_AZUL_ACENTO()
}

/// Mistura preservando a tonalidade do acento configurado (sem RGB azul fixo).
fn mesclar_cores(base: Color, destaque: Color, intensidade: f32) -> Color {
    let t = intensidade.clamp(0.0, 1.0);
    Color {
        r: base.r + (destaque.r - base.r) * t,
        g: base.g + (destaque.g - base.g) * t,
        b: base.b + (destaque.b - base.b) * t,
        a: 1.0,
    }
}

fn cor_selecao_para(escuro: bool, acento: Color) -> Color {
    mesclar_cores(cor_fundo(escuro), acento, if escuro { 0.28 } else { 0.13 })
}

/// Destaque das abas, módulos, menus, propriedades e console.
pub(in crate::app) fn cor_selecao(escuro: bool) -> Color {
    cor_selecao_para(escuro, cor_acento())
}

/// Fundo do botão primário: mantém a cor escolhida e um hover sutil.
fn cor_primaria_para(escuro: bool, acento: Color, status: pelicano::Status) -> Color {
    match status {
        pelicano::Status::Hovered =>
            mesclar_cores(acento, cor_texto(escuro), 0.13),
        pelicano::Status::Pressed =>
            mesclar_cores(acento, cor_texto(escuro), 0.22),
        pelicano::Status::Disabled =>
            mesclar_cores(acento, cor_chrome(escuro), 0.50),
        _ => acento,
    }
}
pub(in crate::app) fn estilo_fundo_editor(escuro: bool) -> impl Fn(&Theme) -> pelicano::Style {
    move |_| pelicano::Style {
        text_color: Some(cor_texto(escuro)),
        background: Some(Background::Color(cor_workspace(escuro))),
        ..pelicano::Style::default()
    }
}
pub(in crate::app) fn estilo_divisor(escuro: bool) -> impl Fn(&Theme) -> pelicano::Style {
    move |_| pelicano::Style {
        background: Some(Background::Color(cor_borda(escuro))),
        ..pelicano::Style::default()
    }
}
pub(in crate::app) fn estilo_celula_propriedade(escuro: bool, nome: bool) -> impl Fn(&Theme) -> pelicano::Style {
    move |_| pelicano::Style {
        text_color: Some(if nome { cor_secundaria(escuro) } else { cor_texto(escuro) }),
        background: Some(Background::Color(if nome { cor_chrome(escuro) } else { cor_fundo(escuro) })),
        border: Border { color: cor_borda(escuro), width: 0.5, ..Border::default() },
        ..pelicano::Style::default()
    }
}
pub(in crate::app) fn botao_menu<'a>(rotulo: &str, acao: Option<Msg>, ativo: bool) -> Element<'a, Msg> {
    let b = button(text(rotulo.to_owned()).size(12)).padding([8, 11]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(ativo)).into()
}
pub(in crate::app) fn botao_aba<'a>(rotulo: &str, icone: &str, acao: Option<Msg>, ativo: bool) -> Element<'a, Msg> {
    let conteudo = row![
        feather_ui::icones::icone(icone, 15.0),
        text(rotulo.to_owned()).size(12),
    ].spacing(6).align(pelicano::Align::Center);
    let b = button(conteudo).padding([8, 12]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(ativo)).into()
}

pub(in crate::app) fn cor_fundo(escuro: bool) -> Color {
    if escuro { Color::rgb8(30, 36, 46) } else { Color::rgb8(255, 255, 255) }
}
pub(in crate::app) fn cor_chrome(escuro: bool) -> Color {
    if escuro { Color::rgb8(37, 45, 57) } else { Color::rgb8(247, 249, 252) }
}
pub(in crate::app) fn cor_borda(escuro: bool) -> Color {
    if escuro { Color::rgb8(62, 73, 89) } else { Color::rgb8(219, 228, 239) }
}
pub(in crate::app) fn cor_texto(escuro: bool) -> Color {
    if escuro { Color::rgb8(231, 236, 244) } else { Color::rgb8(35, 49, 69) }
}
pub(in crate::app) fn cor_secundaria(escuro: bool) -> Color {
    if escuro { Color::rgb8(156, 172, 192) } else { Color::rgb8(111, 128, 152) }
}
pub(in crate::app) fn estilo_area(escuro: bool) -> impl Fn(&Theme) -> pelicano::Style {
    move |_| pelicano::Style {
        text_color: Some(cor_texto(escuro)),
        background: Some(Background::Color(cor_fundo(escuro))),
        border: Border {
            color: cor_borda(escuro), width: 1.0, radius: [9.0; 4],
        },
        ..Default::default()
    }
}
pub(in crate::app) fn estilo_cabecalho(escuro: bool) -> impl Fn(&Theme) -> pelicano::Style {
    move |_| pelicano::Style {
        text_color: Some(cor_texto(escuro)),
        background: Some(Background::Color(cor_chrome(escuro))),
        border: Border {
            color: cor_borda(escuro), width: 0.6, radius: [6.0; 4],
        },
        ..Default::default()
    }
}
// Estilo de IDE: itens planos para menus, abas e árvore; sem botões
// arredondados da Ribbon da planilha.
pub(in crate::app) fn estilo_botao_ide(ativo: bool) -> impl Fn(&Theme, pelicano::Status) -> pelicano::Style {
    move |_theme, status| {
        let escuro = feather_ui::tema::escuro();
        let desabilitado = status == pelicano::Status::Disabled;
        let fundo = if ativo {
            Some(Background::Color(cor_selecao(escuro)))
        } else if matches!(status, pelicano::Status::Hovered | pelicano::Status::Pressed) {
            Some(Background::Color(
                mesclar_cores(cor_chrome(escuro), cor_acento(),
                    if escuro { 0.18 } else { 0.10 })
            ))
        } else { None };
        pelicano::Style {
            background: fundo,
            text_color: Some(if desabilitado { cor_secundaria(escuro) }
                else if ativo {
                    feather_ui::tema::COR_ACENTO_TEXTO(cor_selecao(escuro))
                } else { cor_texto(escuro) }),
            border: Border { color: Color::TRANSPARENT, width: 0.0, radius: [6.0; 4] },
            ..Default::default()
        }
    }
}

pub(in crate::app) fn botao<'a>(rotulo: &str, acao: Option<Msg>, escolhido: bool) -> Element<'a, Msg> {
    let b = button(text(rotulo.to_owned()).size(12)).padding([5, 9]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(escolhido)).into()
}

pub(in crate::app) fn botao_executar<'a>(acao: Option<Msg>) -> Element<'a, Msg> {
    botao_primario(tr!("Executar  F5"), "player-play", acao)
}

/// Botão primário na cor de destaque configurada (Executar, Carregar…).
pub(in crate::app) fn botao_primario<'a>(
    rotulo: &str, icone: &str, acao: Option<Msg>,
) -> Element<'a, Msg> {
    let conteudo = row![
        feather_ui::icones::icone(icone, 16.0),
        text(rotulo.to_owned()).size(12),
    ].spacing(4).align(pelicano::Align::Center);
    let b = button(conteudo).padding([8, 12]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(|_theme, status| {
        let escuro = feather_ui::tema::escuro();
        let fundo = cor_primaria_para(escuro, cor_acento(), status);
        pelicano::Style {
            background: Some(Background::Color(fundo)),
            // Laranja/verde em tema escuro podem exigir texto preto.
            text_color: Some(feather_ui::acessibilidade::texto_sobre(fundo)),
            border: Border { radius: [7.0; 4], ..Border::default() },
            ..Default::default()
        }
    }).into()
}

/// Ícones SVG da suíte evitam os quadrados Unicode sem glifo no Linux.
pub(in crate::app) fn botao_icone<'a>(
    rotulo: &str, icone: &str, acao: Option<Msg>, ativo: bool,
) -> Element<'a, Msg> {
    let conteudo = row![
        feather_ui::icones::icone(icone, 15.0),
        text(rotulo.to_owned()).size(12),
    ].spacing(4).align(pelicano::Align::Center);
    let b = button(conteudo).padding([5, 8]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(ativo)).into()
}

/// Ícone e recuo do projeto são widgets reais, não caracteres Unicode
/// que podem renderizar como quadradinhos dependendo da fonte instalada.
pub(in crate::app) fn item_arvore<'a>(
    rotulo: &str, icone: &str, recuo: f32, acao: Option<Msg>, ativo: bool,
) -> Element<'a, Msg> {
    let conteudo = row![
        Space::new(Length::Fixed(recuo), 0.0),
        feather_ui::icones::icone(icone, 15.0),
        text(rotulo.to_owned()).size(12),
    ].spacing(5).align(pelicano::Align::Center);
    let b = button(conteudo).padding([5, 7]).width(Length::FILL);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(ativo)).into()
}

pub(in crate::app) fn grupo_arvore<'a>(rotulo: &str, icone: &str, escuro: bool) -> Element<'a, Msg> {
    container(row![
        Space::new(Length::Fixed(6.0), 0.0),
        feather_ui::icones::icone("chevron-down", 12.0),
        feather_ui::icones::icone(icone, 13.0),
        text(rotulo.to_owned()).size(12).color(cor_secundaria(escuro)),
    ].align(pelicano::Align::Center).spacing(5))
        .padding([6, 5]).width(Length::FILL).into()
}

/// Botão que só contém um SVG (fechar console, adicionar inspeção).
pub(in crate::app) fn botao_icone_simples<'a>(
    icone: &str, acao: Option<Msg>,
) -> Element<'a, Msg> {
    let b = button(feather_ui::icones::icone(icone, 15.0)).padding([5, 7]);
    let b = if let Some(acao) = acao { b.on_press(acao) } else { b };
    b.style(estilo_botao_ide(false)).into()
}

pub(in crate::app) fn botao_campo<'a>(rotulo: &str) -> Element<'a, Msg> {
    button(row![
        text(rotulo.to_owned()).size(12),
        feather_ui::icones::icone("chevron-down", 12.0),
    ].align(pelicano::Align::Center).spacing(5))
        .padding([6, 10])
        .style(|_theme, status| {
            let escuro = feather_ui::tema::escuro();
            pelicano::Style {
                background: Some(Background::Color(if escuro {
                    cor_chrome(true)
                } else { cor_fundo(false) })),
                text_color: Some(if status == pelicano::Status::Disabled {
                    cor_secundaria(escuro)
                } else { cor_texto(escuro) }),
                border: Border { color: cor_borda(escuro), width: 1.0, radius: [6.0; 4] },
                ..Default::default()
            }
        }).into()
}

pub(in crate::app) fn titulo<'a>(nome: &str, escuro: bool) -> Element<'a, Msg> {
    container(text(nome.to_owned()).size(12)).padding([9, 11])
        .width(Length::FILL).style(estilo_cabecalho(escuro)).into()
}

#[cfg(test)]
mod testes_acentos_editor {
    use super::*;

    fn colorir(hex: &str) -> Color {
        let valor = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
        Color::rgb8((valor >> 16) as u8, (valor >> 8) as u8, valor as u8)
    }

    #[test]
    fn seis_cores_de_destaque_valem_para_todo_o_editor() {
        use feather_ui::acessibilidade::{contraste, texto_acento, texto_sobre, CONTRASTE_TEXTO_AA};
        let mut fundos_claros = Vec::new();
        for (_, claro, escuro) in feather_ui::tema::ACENTOS {
            for (escuro_ativo, codigo) in [(false, claro), (true, escuro)] {
                let destaque = colorir(codigo);
                let selecao = cor_selecao_para(escuro_ativo, destaque);
                let texto = texto_acento(destaque, selecao);
                assert!(contraste(texto, selecao) + 0.001 >= CONTRASTE_TEXTO_AA);
                let primario = cor_primaria_para(
                    escuro_ativo, destaque, pelicano::Status::Idle
                );
                assert_eq!(primario, destaque);
                assert!(contraste(texto_sobre(primario), primario) >= CONTRASTE_TEXTO_AA);
                let sobre = cor_primaria_para(
                    escuro_ativo, destaque, pelicano::Status::Hovered
                );
                assert_ne!(sobre, primario);
                assert!(contraste(texto_sobre(sobre), sobre) >= CONTRASTE_TEXTO_AA);
                if !escuro_ativo { fundos_claros.push(selecao); }
            }
        }
        // O painel selecionado precisa mudar com a preferência de cor.
        assert!(fundos_claros.windows(2).all(|par| par[0] != par[1]));
    }
}
