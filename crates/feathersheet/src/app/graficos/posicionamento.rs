//! Geometria dos gráficos: coordenadas persistidas na folha a 100%.
//! Scroll e zoom transformam exclusivamente a apresentação no Iced.
use feathersheet_core::Sheet;
use crate::grade::ALTURA_PADRAO;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Quadro {
    pub x: f64,
    pub y: f64,
    pub largura: f64,
    pub altura: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alca {
    Noroeste, Norte, Nordeste, Oeste, Leste, Sudoeste, Sul, Sudeste,
}

impl Alca {
    fn eixos(self) -> (i8, i8) {
        match self {
            Self::Noroeste => (-1, -1),
            Self::Norte => (0, -1),
            Self::Nordeste => (1, -1),
            Self::Oeste => (-1, 0),
            Self::Leste => (1, 0),
            Self::Sudoeste => (-1, 1),
            Self::Sul => (0, 1),
            Self::Sudeste => (1, 1),
        }
    }
}

impl Quadro {
    pub(super) fn redimensionar(self, alca: Alca, dx: f64, dy: f64) -> Self {
        let (horizontal, vertical) = alca.eixos();
        let mut esquerda = self.x;
        let mut cima = self.y;
        let mut direita = self.x + self.largura;
        let mut baixo = self.y + self.altura;
        if horizontal < 0 { esquerda = (esquerda + dx).clamp(0., direita - 240.); }
        if horizontal > 0 { direita = (direita + dx).clamp(esquerda + 240., esquerda + 1600.); }
        if vertical < 0 { cima = (cima + dy).clamp(0., baixo - 200.); }
        if vertical > 0 { baixo = (baixo + dy).clamp(cima + 200., cima + 1600.); }
        Self { x: esquerda, y: cima, largura: direita - esquerda, altura: baixo - cima }
    }

    pub(super) fn da_posicao(valor: &serde_json::Value) -> Option<Self> {
        let n = |key: &str, fallback: f64| -> Option<f64> {
            let v = valor[key].as_f64().unwrap_or(fallback);
            v.is_finite().then_some(v)
        };
        Some(Self {
            x: n("x", 80.)?.max(0.),
            y: n("y", 80.)?.max(0.),
            largura: n("width", 480.)?.clamp(240., 1600.),
            altura: n("height", 300.)?.clamp(200., 1600.),
        })
    }
}

/// Somente sobre os índices personalizados; O(quantidade de ajustes),
/// independentemente de 10 milhões de linhas.
pub(super) fn scroll_em_pixels(sheet: &Sheet, linha: u32, coluna: u32) -> (f64, f64) {
    let padrao = crate::grade::largura_padrao_da(sheet);
    let x = f64::from(coluna) * f64::from(padrao)
        + sheet.col_widths.iter()
            .filter(|(idx, _)| **idx < coluna)
            .map(|(_, w)| f64::from(*w) - f64::from(padrao))
            .sum::<f64>();
    let y = f64::from(linha) * f64::from(ALTURA_PADRAO)
        + sheet.row_heights.iter()
            .filter(|(idx, _)| **idx < linha)
            .map(|(_, h)| f64::from(*h) - f64::from(ALTURA_PADRAO))
            .sum::<f64>();
    (x, y)
}

/// Origem relativa à região de CÉLULAS (sem cabeçalhos), em px lógicos.
pub(super) fn origem_tela(q: Quadro, scroll: (f64, f64), zoom: f64) -> (f64, f64) {
    ((q.x - scroll.0) * zoom, (q.y - scroll.1) * zoom)
}

/// O pedaço do gráfico que aparece na área de células (origem >= 0, como o
/// pelicano posiciona): `(x, y, largura, altura, corte_x, corte_y)`, onde o
/// corte é quanto ficou para fora à esquerda e em cima. `None` se nada aparece.
pub(super) fn parte_visivel(x: f64, y: f64, largura: f64, altura: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let (corte_x, corte_y) = ((-x).max(0.), (-y).max(0.));
    let (w, h) = (largura - corte_x, altura - corte_y);
    (w >= 1. && h >= 1. && x <= 100_000. && y <= 100_000.)
        .then_some((x.max(0.), y.max(0.), w, h, corte_x, corte_y))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn quadro() -> Quadro {
        Quadro { x: 100., y: 120., largura: 480., altura: 300. }
    }

    #[test]
    fn grafico_cortado_pela_borda_mostra_a_parte_visivel() {
        assert_eq!(parte_visivel(10., 20., 480., 300.), Some((10., 20., 480., 300., 0., 0.)));
        assert_eq!(parte_visivel(-100., -50., 480., 300.), Some((0., 0., 380., 250., 100., 50.)));
        assert_eq!(parte_visivel(-480., 0., 480., 300.), None);
        assert_eq!(parte_visivel(0., 200_000., 480., 300.), None);
    }

    #[test]
    fn grafico_se_desloca_com_linhas_colunas_e_zoom() {
        let folha = Sheet::default();
        let scroll = scroll_em_pixels(&folha, 3, 2);
        assert_eq!(scroll, (244., 84.));
        assert_eq!(origem_tela(quadro(), scroll, 1.), (-144., 36.));
        assert_eq!(origem_tela(quadro(), scroll, 1.5), (-216., 54.));
        assert_eq!(origem_tela(quadro(), (0., 0.), 1.), (100., 120.));
    }

    #[test]
    fn tamanhos_customizados_entram_no_deslocamento() {
        let mut folha = Sheet::default();
        folha.row_heights.insert(0, 50);
        folha.row_heights.insert(2, 80);
        folha.col_widths.insert(0, 200);
        folha.col_widths.insert(4, 400);
        assert_eq!(scroll_em_pixels(&folha, 3, 2), (322., 158.));
        assert_eq!(scroll_em_pixels(&folha, 4, 4), (566., 186.));
    }

    #[test]
    fn nao_percorre_todos_os_dez_milhoes_de_linhas() {
        let folha = Sheet::default();
        let (_, y) = scroll_em_pixels(&folha, 9_999_999, 0);
        assert_eq!(y, 9_999_999. * 28.);
    }

    #[test]
    fn oito_alcas_preservam_aresta_oposta() {
        let q = quadro();
        assert_eq!(q.redimensionar(Alca::Sudeste, 30., 10.),
                   Quadro { largura: 510., altura: 310., ..q });
        assert_eq!(q.redimensionar(Alca::Noroeste, 20., 10.),
                   Quadro { x: 120., y: 130., largura: 460., altura: 290. });
        assert_eq!(q.redimensionar(Alca::Oeste, 20., 0.),
                   Quadro { x: 120., largura: 460., ..q });
        assert_eq!(q.redimensionar(Alca::Norte, 0., 20.),
                   Quadro { y: 140., altura: 280., ..q });
        assert_eq!(q.redimensionar(Alca::Leste, 20., 0.),
                   Quadro { largura: 500., ..q });
        assert_eq!(q.redimensionar(Alca::Sul, 0., 20.),
                   Quadro { altura: 320., ..q });
        assert_eq!(q.redimensionar(Alca::Nordeste, 20., -10.),
                   Quadro { y: 110., largura: 500., altura: 310., ..q });
        assert_eq!(q.redimensionar(Alca::Sudoeste, -10., 20.),
                   Quadro { x: 90., largura: 490., altura: 320., ..q });
    }

    #[test]
    fn dimensoes_minimas_e_posicao_positiva() {
        let q = quadro().redimensionar(Alca::Noroeste, 20_000., 20_000.);
        assert_eq!((q.largura, q.altura), (240., 200.));
        let q = quadro().redimensionar(Alca::Noroeste, -20_000., -20_000.);
        assert_eq!((q.x, q.y), (0., 0.));
    }

    #[test]
    fn posicao_persistida_valida_e_rejeita_nan() {
        let pos = serde_json::json!({"x": 10, "y": 30, "width": 460, "height": 280});
        assert_eq!(Quadro::da_posicao(&pos).unwrap().largura, 460.);
        assert_eq!(Quadro::da_posicao(&serde_json::json!({})).unwrap(), Quadro {
            x: 80., y: 80., largura: 480., altura: 300.,
        });
    }
}
