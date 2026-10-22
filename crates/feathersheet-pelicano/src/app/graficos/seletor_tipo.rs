//! Galeria visual de tipos do antigo chart-panel.ts, agora usando widgets do pelicano.
//! Não há DOM/JS, arquivos de ícones externos nem dependência gráfica no motor.
//! Os tipos sem renderizador comum ficam visíveis, mas desabilitados.
use super::{Evento, TipoUi};
use crate::app::Msg;
use crate::compat::button; use pelicano::column; use pelicano::container; use pelicano::row; use pelicano::scrollable; use pelicano::svg; use pelicano::text; use pelicano::Element; use pelicano::Length;

#[derive(Clone, Copy)]
struct Opcao {
    id: &'static str,
    rotulo: &'static str,
    disponivel: Option<TipoUi>,
}

const fn opcao(id: &'static str, rotulo: &'static str, disponivel: Option<TipoUi>) -> Opcao {
    Opcao {
        id,
        rotulo,
        disponivel,
    }
}

/// Mesmas seis famílias e os mesmos 25 tipos do seletor legado do FeatherSheet.
/// Exibir um tipo não implica que Charts já suporte a sua renderização.
const CATEGORIAS: &[(&str, &[Opcao])] = &[
    (
        "Colunas & Barras",
        &[
            opcao("column", "Colunas", Some(TipoUi::Colunas)),
            opcao("column-stacked", "Col. Empilhada", Some(TipoUi::ColunasEmpilhadas)),
            opcao("column-stacked-100", "Col. 100%", Some(TipoUi::Colunas100)),
            opcao("bar", "Barras", Some(TipoUi::Barras)),
            opcao("bar-stacked", "Bar. Empilhada", Some(TipoUi::BarrasEmpilhadas)),
            opcao("bar-stacked-100", "Bar. 100%", Some(TipoUi::Barras100)),
        ],
    ),
    (
        "Linhas & Degraus",
        &[
            opcao("line", "Linhas", Some(TipoUi::Linhas)),
            opcao("line-3d", "Linha 3D (XYZ)", Some(TipoUi::Linha3d)),
            opcao("step-line", "Degraus", Some(TipoUi::Degraus)),
            opcao("step-line-stacked", "Deg. Empilhado", Some(TipoUi::DegrausEmpilhados)),
            opcao("step-line-stacked-100", "Deg. 100%", Some(TipoUi::Degraus100)),
        ],
    ),
    (
        "Áreas & Degraus",
        &[
            opcao("area", "Áreas", Some(TipoUi::Area)),
            opcao("step-area", "Área Degrau", Some(TipoUi::AreaDegraus)),
            opcao("step-area-stacked", "Área Empilhada", Some(TipoUi::AreaDegrausEmpilhada)),
            opcao("step-area-stacked-100", "Área 100%", Some(TipoUi::AreaDegraus100)),
        ],
    ),
    (
        "Projetos & Cronograma",
        &[opcao("gantt", "Gantt (Cronograma)", Some(TipoUi::Gantt))],
    ),
    (
        "Análise & Estatística",
        &[
            opcao("combo", "Combinado", Some(TipoUi::Combinado)),
            opcao("histogram", "Histograma", Some(TipoUi::Histograma)),
            opcao("waterfall", "Cascata", Some(TipoUi::Cascata)),
            opcao("heatmap", "Heatmap", Some(TipoUi::MapaCalor)),
            opcao("radar", "Radar", Some(TipoUi::Radar)),
        ],
    ),
    (
        "Dispersão & Mercado",
        &[
            opcao("scatter", "Dispersão", Some(TipoUi::Dispersao)),
            opcao("bubble", "Bolhas", Some(TipoUi::Bolhas)),
            opcao("candlestick", "Candlestick", Some(TipoUi::Candlestick)),
            opcao("pie", "Pizza / Rosca", Some(TipoUi::Pizza)),
        ],
    ),
];

/// Miniaturas vetoriais embutidas (48×32) equivalentes às do seletor TS.
/// Nunca interpretamos dados de células como XML.
fn miniatura_svg(id: &str) -> String {
    let desenho = match id {
        "column" => {
            r##"<rect x="6" y="14" width="8" height="14" fill="#4285f4"/><rect x="20" y="6" width="8" height="22" fill="#ea4335"/><rect x="34" y="10" width="8" height="18" fill="#fbbc04"/>"##
        }
        "column-stacked" | "column-stacked-100" => {
            r##"<rect x="10" y="16" width="10" height="12" fill="#4285f4"/><rect x="10" y="8" width="10" height="8" fill="#ea4335"/><rect x="28" y="14" width="10" height="14" fill="#4285f4"/><rect x="28" y="4" width="10" height="10" fill="#ea4335"/>"##
        }
        "bar" => {
            r##"<rect x="4" y="6" width="26" height="6" fill="#4285f4"/><rect x="4" y="14" width="38" height="6" fill="#ea4335"/><rect x="4" y="22" width="18" height="6" fill="#fbbc04"/>"##
        }
        "bar-stacked" | "bar-stacked-100" => {
            r##"<rect x="4" y="8" width="18" height="7" fill="#4285f4"/><rect x="22" y="8" width="14" height="7" fill="#ea4335"/><rect x="4" y="18" width="12" height="7" fill="#4285f4"/><rect x="16" y="18" width="24" height="7" fill="#ea4335"/>"##
        }
        "line" => {
            r##"<path d="M4 22 L16 10 L28 20 L44 6" fill="none" stroke="#4285f4" stroke-width="2.5"/><path d="M4 26 L16 18 L28 24 L44 14" fill="none" stroke="#ea4335" stroke-width="2.5"/>"##
        }
        "step-line" | "step-line-stacked" | "step-line-stacked-100" => {
            r##"<path d="M4 24 L16 24 L16 12 L28 12 L28 6 L44 6" fill="none" stroke="#4285f4" stroke-width="2.5"/>"##
        }
        "area" => {
            r##"<path d="M4 26 L16 14 L28 22 L44 8 L44 28 L4 28 Z" fill="#4285f4" opacity=".4"/><path d="M4 26 L16 14 L28 22 L44 8" fill="none" stroke="#4285f4" stroke-width="2"/>"##
        }
        "step-area" | "step-area-stacked" | "step-area-stacked-100" => {
            r##"<path d="M4 24 L16 24 L16 12 L28 12 L28 6 L44 6 L44 28 L4 28 Z" fill="#4285f4" opacity=".4"/><path d="M4 24 L16 24 L16 12 L28 12 L28 6 L44 6" fill="none" stroke="#4285f4" stroke-width="2"/>"##
        }
        "pie" => {
            r##"<path d="M24 16 L24 4 A12 12 0 0 1 36 16 Z" fill="#4285f4"/><path d="M24 16 L36 16 A12 12 0 1 1 24 4 Z" fill="#ea4335"/>"##
        }
        "scatter" => {
            r##"<circle cx="8" cy="22" r="3" fill="#4285f4"/><circle cx="18" cy="10" r="3" fill="#ea4335"/><circle cx="28" cy="20" r="3" fill="#fbbc04"/><circle cx="38" cy="8" r="3" fill="#34a853"/>"##
        }
        "bubble" => {
            r##"<circle cx="12" cy="20" r="7" fill="#4285f4" opacity=".8"/><circle cx="28" cy="10" r="9" fill="#ea4335" opacity=".8"/><circle cx="36" cy="24" r="5" fill="#fbbc04" opacity=".8"/>"##
        }
        "combo" => {
            r##"<rect x="8" y="14" width="8" height="14" fill="#4285f4"/><rect x="24" y="8" width="8" height="20" fill="#4285f4"/><path d="M4 18 L20 8 L36 14 L44 4" fill="none" stroke="#ea4335" stroke-width="2.5"/>"##
        }
        "heatmap" => {
            r##"<rect x="4" y="4" width="18" height="10" fill="#3b82f4"/><rect x="24" y="4" width="18" height="10" fill="#06b6d4"/><rect x="4" y="16" width="18" height="10" fill="#eab308"/><rect x="24" y="16" width="18" height="10" fill="#ef4444"/>"##
        }
        "waterfall" => {
            r##"<rect x="4" y="18" width="8" height="10" fill="#4285f4"/><rect x="16" y="10" width="8" height="8" fill="#10b981"/><rect x="28" y="14" width="8" height="6" fill="#ef4444"/><rect x="38" y="6" width="6" height="22" fill="#6366f1"/>"##
        }
        "histogram" => {
            r##"<rect x="4" y="18" width="8" height="10" fill="#4285f4"/><rect x="12" y="6" width="8" height="22" fill="#4285f4"/><rect x="20" y="10" width="8" height="18" fill="#4285f4"/><rect x="28" y="20" width="8" height="8" fill="#4285f4"/>"##
        }
        "radar" => {
            r##"<path d="M24 4 L40 16 L32 28 L16 28 L8 16 Z" fill="#4285f4" opacity=".3" stroke="#4285f4" stroke-width="2"/>"##
        }
        "gantt" => {
            r##"<line x1="4" y1="28" x2="44" y2="28" stroke="#94a3b8"/><rect x="6" y="6" width="18" height="5" fill="#4285f4"/><rect x="16" y="14" width="22" height="5" fill="#10b981"/><rect x="24" y="21" width="16" height="5" fill="#f59e0b"/>"##
        }
        "candlestick" => {
            r##"<line x1="14" y1="4" x2="14" y2="28" stroke="#10b981"/><rect x="10" y="10" width="8" height="12" fill="#10b981"/><line x1="34" y1="4" x2="34" y2="28" stroke="#ef4444"/><rect x="30" y="8" width="8" height="14" fill="#ef4444"/>"##
        }
        "line-3d" => {
            r##"<path d="M4 24 L16 12 L28 18 L44 5" fill="none" stroke="#4285f4" stroke-width="2.5"/><path d="M4 28 L16 16 L28 22 L44 9" fill="none" stroke="#ea4335" stroke-width="1.5"/>"##
        }
        _ => "",
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="32" viewBox="0 0 48 32">{desenho}</svg>"##
    )
}

/// Medidas compactas da galeria, próximas às do chart-panel.ts legado.
/// O tamanho fixo evita que o comprimento dos rótulos estique os cartões.
pub(super) const LARGURA_POPOVER: f32 = 360.0;
pub(super) const ALTURA_POPOVER: f32 = 360.0;
const COLUNAS: usize = 3;
const LARGURA_CARTAO: f32 = 102.0;
const ALTURA_CARTAO: f32 = 52.0;
const ESPACO_CARTOES: u16 = 6;
const LARGURA_MINIATURA: f32 = 36.0;
const ALTURA_MINIATURA: f32 = 24.0;

/// Destaque discreto como no TypeScript: borda azul e fundo azul muito claro,
/// sem transformar o cartão inteiro em um bloco de cor saturada.
fn estilo_opcao(
    _tema: &pelicano::Theme,
    estado: pelicano::Status,
    ativo: bool,
    disponivel: bool,
) -> pelicano::Style {
    use feather_ui::tema::{
        COR_AZUL_ACENTO, COR_AZUL_FUNDO, COR_BORDA, COR_HOVER_AZUL, COR_PAINEL,
        COR_PAINEL_FAIXA, COR_TEXTO, COR_TEXTO_MUTED,
    };
    // O `button::secondary` do Iced: tudo o que ele pinta é trocado abaixo.
    let mut estilo = pelicano::Style::default();
    let hover = matches!(estado, pelicano::Status::Hovered | pelicano::Status::Pressed);
    let (fundo, borda, tinta) = if !disponivel {
        (COR_PAINEL_FAIXA(), COR_BORDA(), COR_TEXTO_MUTED())
    } else if ativo {
        (COR_AZUL_FUNDO(), COR_AZUL_ACENTO(), COR_TEXTO())
    } else if hover {
        (COR_HOVER_AZUL(), COR_AZUL_ACENTO(), COR_TEXTO())
    } else {
        (COR_PAINEL(), COR_BORDA(), COR_TEXTO())
    };
    estilo.background = Some(pelicano::Background::Color(fundo));
    estilo.text_color = Some(tinta);
    estilo.border = pelicano::Border {
        color: borda,
        width: 1.0,
        radius: [6.0; 4],
    };
    estilo
}

/// Popover de escolha: três cartões horizontais por linha, rolagem independente
/// e categorias compactas. Apenas tipos com renderizador recebem evento Tipo.
pub(super) fn painel(tipo_ativo: TipoUi) -> Element<'static, Msg> {
    let cabecalho = row![
        text(tr!("Tipo de gráfico")).size(13),
        pelicano::Space::new(Length::FILL, 0.0),
        pelicano::tooltip(
            button(
                container(feather_ui::icones::icone("x", 15.0)).center()
            )
            .width(Length::Fixed(26.0))
            .height(Length::Fixed(26.0))
            .padding(0)
            .style(feather_ui::tema::estilo_botao_pequeno(false))
            .on_press(Msg::Grafico(Evento::FecharSeletor)),
            tr!("Fechar"),
            pelicano::TooltipPosition::Bottom,
        )
        .padding([4, 8])
        .style(feather_ui::tema::estilo_dica),
    ]
    .align(pelicano::Align::Center);

    // O catálogo contém widgets visuais sem mensagens próprias: ancorar
    // o tipo para impedir inferência ambígua do Iced 0.13.
    let mut grupos: pelicano::Flex<'static, Msg> = column(Vec::new()).spacing(8);
    for &(categoria, opcoes) in CATEGORIAS {
        grupos = grupos.push(text(tr!(categoria)).size(11));
        for linha in opcoes.chunks(COLUNAS) {
            let mut cartoes = row(Vec::new()).spacing(ESPACO_CARTOES);
            for op in linha {
                let disponivel = op.disponivel.is_some();
                let ativo = disponivel && Some(tipo_ativo) == op.disponivel;
                // Tipos futuros continuam na lista, mas sem aparência de botão
                // acionável nem o sufixo longo "(em breve)" em cada legenda.
                let miniatura = miniatura_svg(op.id);
                let miniatura = if disponivel {
                    miniatura
                } else {
                    miniatura.replacen("<svg ", "<svg opacity=\"0.32\" ", 1)
                };
                let imagem: Element<'static, Msg> = match pelicano::Svg::from_bytes(miniatura.as_bytes()) {
                    Some(s) => svg(s)
                        .width(Length::Fixed(LARGURA_MINIATURA))
                        .height(Length::Fixed(ALTURA_MINIATURA))
                        .into(),
                    None => pelicano::Space::new(LARGURA_MINIATURA, ALTURA_MINIATURA).into(),
                };
                let item: Element<'static, Msg> = row![
                    imagem,
                    text(tr!(op.rotulo)).size(11).width(Length::FILL),
                ]
                .spacing(5)
                .align(pelicano::Align::Center)
                .into();
                let mut cartao = button(item)
                    .width(Length::Fixed(LARGURA_CARTAO))
                    .height(Length::Fixed(ALTURA_CARTAO))
                    .padding([5, 5])
                    .style(move |tema, estado| estilo_opcao(tema, estado, ativo, disponivel));
                if let Some(tipo) = op.disponivel {
                    cartao = cartao.on_press(Msg::Grafico(Evento::Tipo(tipo)));
                }
                cartoes = cartoes.push(cartao);
            }
            grupos = grupos.push(cartoes);
        }
    }

    container(
        column![
            cabecalho,
            scrollable(grupos).height(Length::FILL),
        ]
        .spacing(7),
    )
    .padding(8)
    .width(Length::FILL)
    .height(Length::FILL)
    .style(|_| pelicano::Style {
        text_color: Some(feather_ui::tema::COR_TEXTO()),
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn catalogo_reflete_exatamente_os_25_tipos_e_seis_familias_do_legado() {
        assert_eq!(CATEGORIAS.len(), 6);
        let mut ids = std::collections::HashSet::new();
        let mut ativos = Vec::new();
        for (_, opcoes) in CATEGORIAS {
            for op in *opcoes {
                assert!(ids.insert(op.id), "tipo repetido: {}", op.id);
                assert!(!op.rotulo.is_empty());
                assert!(miniatura_svg(op.id).contains("<svg "));
                if let Some(tipo) = op.disponivel {
                    ativos.push(tipo);
                }
            }
        }
        assert_eq!(ids.len(), 25);
        assert_eq!(ativos.len(), 25);
        for tipo in TipoUi::TODOS {
            assert!(ativos.contains(&tipo), "tipo disponível ausente: {tipo:?}");
        }
    }

    #[test]
    fn tamanho_compacto_preserva_tres_colunas_e_miniaturas() {
        assert!(LARGURA_POPOVER <= 380.0 && ALTURA_POPOVER <= 380.0);
        assert_eq!(COLUNAS, 3);
        assert_eq!((LARGURA_MINIATURA, ALTURA_MINIATURA), (36.0, 24.0));
        // Duas camadas de padding: painel (6) e conteúdo (8).
        let largura_necessaria =
            LARGURA_CARTAO * COLUNAS as f32
            + ESPACO_CARTOES as f32 * (COLUNAS - 1) as f32
            + 2.0 * (6.0 + 8.0);
        assert!(largura_necessaria <= LARGURA_POPOVER);
        assert!(ALTURA_CARTAO >= ALTURA_MINIATURA + 10.0);
    }

    #[test]
    fn todas_as_opcoes_tem_miniaturas_svg_distintas_de_vazias() {
        for (_, opcoes) in CATEGORIAS {
            for op in *opcoes {
                let svg = miniatura_svg(op.id);
                assert!(svg.contains("viewBox=\"0 0 48 32\""));
                assert!(
                    svg.contains("<path")
                        || svg.contains("<rect")
                        || svg.contains("<circle")
                        || svg.contains("<line"),
                    "sem desenho: {}",
                    op.id
                );
            }
        }
    }
}
