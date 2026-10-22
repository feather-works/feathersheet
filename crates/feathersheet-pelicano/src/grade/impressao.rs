//! Área de Impressão e quebras de página sobre a grade (paridade com
//! ui/grade/impressao.ts). Só desenha por cima; nunca altera células.
use super::*;
use feathersheet_core::celulas::refs::Rect;
use std::rc::Rc;

/// Dados visuais da aba ativa. A área e as quebras vivem na GUI, só em
/// memória (como `sheet.printArea` no TypeScript): nada é gravado no arquivo.
#[derive(Clone, Default)]
pub struct ImpressaoVisual {
    pub area: Option<Rect>,
    /// As outras áreas de impressão da aba (cada uma vai para outra página).
    pub outras: Vec<Rect>,
    /// Primeira linha/coluna de cada nova página (corte guloso do TS).
    pub quebras_linhas: Rc<Vec<u32>>,
    pub quebras_colunas: Rc<Vec<u32>>,
}

const VERDE: Color = Color { r: 0.063, g: 0.486, b: 0.255, a: 1.0 };
const AZUL: Color = Color { r: 0.102, g: 0.451, b: 0.910, a: 1.0 };

fn tracejado(f: &mut Frame, a: Point, b: Point, cor: Color, largura: f32,
    traco: [f32; 2]) {
    let segmentos = [traco[0], traco[1]];
    f.stroke(&canvas::Path::line(a, b), canvas::Stroke {
        color: cor, width: largura,
        line_dash: canvas::LineDash { segments: &segmentos, offset: 0 },
        ..canvas::Stroke::default()
    });
}

impl Grade<'_> {
    /// Moldura verde tracejada com a etiqueta "Área de Impressão" e as
    /// linhas azuis de quebra de página, recortadas ao que está visível.
    pub(super) fn desenhar_impressao(&self, f: &mut Frame, bounds: Rectangle,
        colunas: &[(u32, f32, f32)], linhas: &[(u32, f32, f32)]) {
        let Some(imp) = self.impressao.as_ref() else { return };
        if colunas.is_empty() || linhas.is_empty() { return; }
        let topo = CABECALHO_COLUNA;
        let esquerda = CABECALHO_LINHA;
        let base = bounds.height;
        let direita = bounds.width;
        for a in imp.area.iter().chain(imp.outras.iter()) {
            let cs: Vec<_> = colunas.iter().filter(|(c,_,_)| *c >= a.c0 && *c <= a.c1).collect();
            let rs: Vec<_> = linhas.iter().filter(|(r,_,_)| *r >= a.r0 && *r <= a.r1).collect();
            if let (Some(c_ini), Some(c_fim), Some(r_ini), Some(r_fim)) =
                (cs.first(), cs.last(), rs.first(), rs.last())
            {
                let x0 = c_ini.1; let x1 = c_fim.1 + c_fim.2;
                let y0 = r_ini.1; let y1 = r_fim.1 + r_fim.2;
                let tr = [8.0, 4.0];
                let l = 2.5;
                tracejado(f, Point::new(x0, y0), Point::new(x1, y0), VERDE, l, tr);
                tracejado(f, Point::new(x0, y1), Point::new(x1, y1), VERDE, l, tr);
                tracejado(f, Point::new(x0, y0), Point::new(x0, y1), VERDE, l, tr);
                tracejado(f, Point::new(x1, y0), Point::new(x1, y1), VERDE, l, tr);
                // Etiqueta no canto superior direito, acima da moldura.
                let texto = tr!("Área de Impressão");
                let larg = texto.chars().count() as f32 * 6.0 + 12.0;
                let ex = (x1 - larg).max(esquerda);
                let ey = (y0 - 18.0).max(topo);
                f.fill_rectangle(Point::new(ex, ey), Size::new(larg, 18.0), VERDE);
                f.fill_text(Text {
                    content: texto.into(),
                    position: Point::new(ex + larg / 2.0, ey + 9.0),
                    color: Color::WHITE, size: crate::compat::Pixels(10.0),
                    horizontal_alignment: pelicano::Align::Center,
                    vertical_alignment: pelicano::Align::Center,
                    ..Text::default()
                });
            }
        }
        for &c in imp.quebras_colunas.iter() {
            if let Some(&(_, x, _)) = colunas.iter().find(|(i,_,_)| *i == c) {
                if x > esquerda {
                    tracejado(f, Point::new(x, topo), Point::new(x, base), AZUL, 1.5, [6.0, 4.0]);
                }
            }
        }
        for &r in imp.quebras_linhas.iter() {
            if let Some(&(_, y, _)) = linhas.iter().find(|(i,_,_)| *i == r) {
                if y > topo {
                    tracejado(f, Point::new(esquerda, y), Point::new(direita, y), AZUL, 1.5, [6.0, 4.0]);
                }
            }
        }
    }
}

/// Mesmo algoritmo guloso do TS (`extendPrintBreaks`): acumula o tamanho
/// até estourar a página; aí o índice atual começa a próxima.
pub fn calcular_quebras(ate: u32, pagina: f32, mut tamanho: impl FnMut(u32) -> f32) -> Vec<u32> {
    let mut quebras = Vec::new();
    let mut acumulado = 0.0_f32;
    for i in 0..=ate {
        let t = tamanho(i);
        if acumulado > 0.0 && acumulado + t > pagina {
            quebras.push(i);
            acumulado = t;
        } else {
            acumulado += t;
        }
    }
    quebras
}

#[cfg(test)]
mod testes {
    use super::calcular_quebras;
    #[test]
    fn quebras_gulosas_como_no_typescript() {
        assert_eq!(calcular_quebras(9, 100.0, |_| 30.0), vec![3, 6, 9]);
        assert!(calcular_quebras(2, 100.0, |_| 30.0).is_empty());
    }
}
