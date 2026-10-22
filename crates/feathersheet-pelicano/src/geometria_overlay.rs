//! UI-009: posicionamento de overlays relativo ao viewport Iced.
//!
//! O domínio é pixel lógico do Canvas, sem converter células ou criar estado
//! do documento. O popup se desloca para a esquerda/acima quando necessário
//! e seu conteúdo ganha scroll em janelas estreitas/baixas.

/// Largura mínima do .menu-panel no TypeScript.
pub const LARGURA_CONTEXTUAL: f32 = feather_ui::tema::LARGURA_MENU;
pub const ALTURA_CONTEXTUAL: f32 = 340.0;
const MARGEM: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosicaoPopup {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

impl PosicaoPopup {
    /// Coordenadas relativas à grade Iced (`mouse::Cursor::position_in`).
    /// A função não cria deslocamentos negativos, dimensões maiores que o
    /// viewport nem valores não finitos, mesmo em janelas muito pequenas.
    pub fn contexto(
        cursor_x: f32,
        cursor_y: f32,
        viewport_largura: f32,
        viewport_altura: f32,
    ) -> Option<Self> {
        if ![cursor_x, cursor_y, viewport_largura, viewport_altura]
            .iter()
            .all(|v| v.is_finite())
            || viewport_largura <= 0.0
            || viewport_altura <= 0.0
            || cursor_x < 0.0
            || cursor_y < 0.0
            || cursor_x > viewport_largura
            || cursor_y > viewport_altura
        {
            return None;
        }
        let largura = LARGURA_CONTEXTUAL.min(viewport_largura);
        let altura = ALTURA_CONTEXTUAL.min(viewport_altura);
        let posicionar = |cursor: f32, espaco: f32, popup: f32| {
            if cursor + MARGEM + popup <= espaco {
                cursor + MARGEM
            } else if cursor >= MARGEM + popup {
                cursor - MARGEM - popup
            } else {
                cursor.min((espaco - popup).max(0.0))
            }
        };
        let x = posicionar(cursor_x, viewport_largura, largura);
        let y = posicionar(cursor_y, viewport_altura, altura);
        Some(Self { x, y, largura, altura })
    }

    /// Garantia adicional para qualquer alteração de tamanho da janela:
    /// x + largura e y + altura permanecem dentro da grade.
    pub fn contido(self, viewport_largura: f32, viewport_altura: f32) -> bool {
        self.x >= 0.0 && self.y >= 0.0 && self.largura > 0.0 && self.altura > 0.0
            && self.x + self.largura <= viewport_largura
            && self.y + self.altura <= viewport_altura
    }
}

/// Y da borda inferior do botão Gráfico na Ribbon compacta.
/// O botão SVG de 23px + legenda de 11px ocupa aproximadamente 48px
/// centralizados nos 68px de conteúdo; o restante são legenda do grupo
/// e gutter de rolagem, que NÃO devem separar botão e popup.
pub fn abaixo_botao_grafico(topo: f32, abas: f32, faixa_recolhida: bool) -> f32 {
    if faixa_recolhida {
        return topo + abas + 4.0;
    }
    // Ribbon: 2px de padding, 3px do grupo, botão centralizado em 68px.
    const BOTAO_ALTURA: f32 = 48.0;
    let altura_conteudo = feather_ui::componentes::ALTURA_CONTEUDO_GRUPO_COMPACTO;
    topo + abas + 2.0 + 3.0 + (altura_conteudo - BOTAO_ALTURA) / 2.0
        + BOTAO_ALTURA + 4.0
}

/// Galeria de gráficos ancorada horizontalmente ao botão clicado e
/// verticalmente abaixo do botão, sem esperar o rodapé da Ribbon.
///
/// Usa pixels lógicos do Iced: o zoom das células não altera o popover.
/// Reduz a altura disponível em janelas baixas (a galeria possui scroll).
pub fn galeria_graficos(
    clique_x: f32,
    janela_largura: f32,
    janela_altura: f32,
    abaixo_faixa_y: f32,
    largura_desejada: f32,
    altura_desejada: f32,
) -> Option<PosicaoPopup> {
    if ![clique_x, janela_largura, janela_altura, abaixo_faixa_y,
         largura_desejada, altura_desejada].iter().all(|n| n.is_finite())
        || janela_largura <= 0.0
        || janela_altura <= 0.0
        || largura_desejada <= 0.0
        || altura_desejada <= 0.0
    {
        return None;
    }
    // O popover fica à esquerda do centro do ícone, não da janela.
    // Se o clique estiver próximo à borda direita, recua sem sair da tela.
    let largura = largura_desejada.min((janela_largura - 2.0 * MARGEM).max(1.0));
    let y = abaixo_faixa_y.max(0.0).min((janela_altura - MARGEM).max(0.0));
    let altura = altura_desejada.min((janela_altura - y - MARGEM).max(1.0));
    let x = (clique_x - 32.0)
        .clamp(0.0, (janela_largura - largura).max(0.0));
    let p = PosicaoPopup { x, y, largura, altura };
    p.contido(janela_largura, janela_altura).then_some(p)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn galeria_nova_fica_rente_ao_botao_e_nao_ao_fim_da_ribbon() {
        let topo = 43.0;
        let abas = 52.0;
        let y = abaixo_botao_grafico(topo, abas, false);
        let y_antigo = topo + abas + 141.0 + 4.0;
        assert!(y > topo + abas + 50.0);
        assert!(y < topo + abas + 75.0);
        assert!(y_antigo - y >= 65.0, "o menu anterior estava afastado");
        let popup = galeria_graficos(90.0, 1280.0, 800.0, y, 360.0, 360.0)
            .expect("galeria deve ficar dentro da janela");
        assert_eq!(popup.y, y);
        assert!(popup.contido(1280.0, 800.0));
    }

    #[test]
    fn galeria_ancorada_ao_botao_respeita_borda_direita() {
        let y = abaixo_botao_grafico(43.0, 52.0, false);
        let p = galeria_graficos(1120.0, 1280.0, 800.0, y, 360.0, 360.0)
            .expect("popup deve caber recuado à esquerda");
        assert_eq!(p.y, y);
        assert!(p.x + p.largura <= 1280.0);
    }

    #[test]
    fn galeria_graficos_fica_abaixo_da_ribbon_e_nao_cobre_abas() {
        // Cromo 43 px + abas 52 px + Ribbon 127 px + margem 4 px.
        let abaixo = 43.0 + 52.0 + 127.0 + 4.0;
        let p = galeria_graficos(286.0, 1280.0, 800.0, abaixo, 360.0, 360.0)
            .expect("a galeria precisa caber na janela padrão");
        assert_eq!(p.y, 226.0);
        assert_eq!(p.x, 254.0); // horizontalmente alinhada ao botão, não fixa em 120 px
        assert_eq!((p.largura, p.altura), (360.0, 360.0));
        assert!(p.contido(1280.0, 800.0));
    }

    #[test]
    fn galeria_graficos_recuada_e_rolavel_em_janela_minima() {
        let abaixo = 226.0;
        let direita = galeria_graficos(730.0, 760.0, 520.0, abaixo, 360.0, 360.0).unwrap();
        assert_eq!(direita.y, abaixo);
        assert!(direita.x < 730.0); // recua perto da borda direita
        assert_eq!(direita.altura, 520.0 - abaixo - 6.0);
        assert!(direita.contido(760.0, 520.0));

        let pequena = galeria_graficos(120.0, 300.0, 340.0, abaixo, 360.0, 360.0).unwrap();
        assert!(pequena.contido(300.0, 340.0));
        assert!(pequena.largura < 360.0);
        assert!(pequena.altura < 360.0);
    }

    #[test]
    fn galeria_graficos_nao_aceita_medidas_invalidas() {
        for bad in [
            (f32::NAN, 1000.0, 800.0),
            (200.0, f32::INFINITY, 800.0),
            (200.0, 1000.0, -1.0),
        ] {
            assert!(galeria_graficos(bad.0, bad.1, bad.2, 226.0, 360.0, 360.0).is_none());
        }
    }

    #[test]
    fn clique_no_canto_superior_abre_abaixo_e_direita() {
        let r = PosicaoPopup::contexto(8.0, 12.0, 1200.0, 800.0).unwrap();
        assert_eq!((r.largura, r.altura), (272.0, 340.0));
        assert_eq!((r.x, r.y), (14.0, 18.0));
        assert!(r.contido(1200.0, 800.0));
    }

    #[test]
    fn clique_proximo_a_direita_e_rodape_abre_para_cima_esquerda() {
        let r = PosicaoPopup::contexto(1100.0, 690.0, 1200.0, 800.0).unwrap();
        assert_eq!((r.x, r.y), (822.0, 344.0));
        assert!(r.contido(1200.0, 800.0));
    }

    #[test]
    fn viewport_menor_que_popup_usa_scroll_interno_e_nao_extravasa() {
        // Popup de LARGURA_CONTEXTUAL (272) x ALTURA_CONTEXTUAL (340): cada
        // dimensão só encolhe quando o viewport é menor que ela.
        for (w, h) in [(300.0, 180.0), (200.0, 500.0), (130.0, 70.0), (1.0, 1.0)] {
            let r = PosicaoPopup::contexto(w - 0.5, h - 0.5, w, h).unwrap();
            assert_eq!(r.largura, LARGURA_CONTEXTUAL.min(w));
            assert_eq!(r.altura, ALTURA_CONTEXTUAL.min(h));
            assert!(r.contido(w, h));
        }
        // Viewport igual ao popup: origem em (0, 0).
        let r = PosicaoPopup::contexto(0.0, 0.0, 130.0, 70.0).unwrap();
        assert_eq!((r.largura, r.altura), (130.0, 70.0));
        assert_eq!((r.x, r.y), (0.0, 0.0));
    }

    #[test]
    fn zoom_e_rolagem_mudam_apenas_a_origem_do_canvas() {
        // Coordenadas aqui já são pixels lógicos que o Iced entrega em
        // position_in(bounds): zoom afeta a grade, não o menu contextual.
        let a = PosicaoPopup::contexto(90.0, 60.0, 640.0, 480.0).unwrap();
        let b = PosicaoPopup::contexto(90.0, 60.0, 640.0, 480.0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.largura, LARGURA_CONTEXTUAL);
        assert!(a.contido(640.0, 480.0));
    }

    #[test]
    fn parametros_nao_finitos_ou_fora_da_grade_sao_rejeitados() {
        for p in [
            (f32::NAN, 0.0, 900.0, 700.0),
            (0.0, f32::INFINITY, 900.0, 700.0),
            (0.0, 0.0, 0.0, 700.0),
            (0.0, 0.0, 900.0, -1.0),
            (-1.0, 0.0, 900.0, 700.0),
            (901.0, 0.0, 900.0, 700.0),
        ] {
            assert_eq!(PosicaoPopup::contexto(p.0, p.1, p.2, p.3), None);
        }
    }

    #[test]
    fn teste_por_grade_de_tamanhos_garante_confinamento() {
        for width in [1.0, 48.0, 200.0, 330.0, 331.0, 800.0, 2048.0] {
            for height in [1.0, 48.0, 200.0, 340.0, 341.0, 900.0, 2048.0] {
                for x in [0.0, width / 2.0, width] {
                    for y in [0.0, height / 2.0, height] {
                        let r = PosicaoPopup::contexto(x, y, width, height).unwrap();
                        assert!(r.contido(width, height), "viewport {width}x{height}; cursor {x},{y}: {r:?}");
                    }
                }
            }
        }
    }
}
