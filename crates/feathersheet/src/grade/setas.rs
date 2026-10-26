//! Setas de Fórmulas → Rastrear precedentes/dependentes, como no Excel:
//! um ponto na origem, a ponta na fórmula, o intervalo lido contornado e,
//! quando a origem é de outra aba, linha tracejada saindo de um ícone de
//! planilha. Origem com erro sai em vermelho.
use super::*;
use super::congelado::Faixa;

/// Vermelho das setas de erro: o mesmo da paleta das referências da fórmula.
const COR_ERRO: Color = Color { r: 220.0 / 255.0, g: 38.0 / 255.0, b: 38.0 / 255.0, a: 1.0 };

/// Centro de um índice no eixo desenhado. Fora da tela, a borda do lado em
/// que ele está, para a seta sair da área visível na direção certa.
fn centro(indice: u32, faixas: &[Faixa], inicio: f32, fim: f32) -> f32 {
    if let Some(&(_, p, t)) = faixas.iter().find(|f| f.0 == indice) {
        return p + t / 2.0;
    }
    match faixas.last() {
        Some(&(ultimo, _, _)) if indice > ultimo => fim,
        _ => inicio,
    }
}

impl Grade<'_> {
    pub(super) fn desenhar_setas(
        &self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], tamanho: Size, cor: Color,
    ) {
        if self.setas.is_empty() { return; }
        let ponto = |r: u32, c: u32| Point::new(
            centro(c, colunas, CABECALHO_LINHA, tamanho.width),
            centro(r, linhas, CABECALHO_COLUNA, tamanho.height),
        );
        for seta in self.setas {
            let cor = if seta.erro { COR_ERRO } else { cor };
            let destino = ponto(seta.destino.0, seta.destino.1);
            let traco = canvas::Stroke::default().with_color(cor).with_width(1.5);
            let origem = if seta.externa {
                // Ícone de planilha acima e à esquerda da fórmula.
                let canto = Point::new((destino.x - 46.0).max(CABECALHO_LINHA + 2.0),
                    (destino.y - 30.0).max(CABECALHO_COLUNA + 2.0));
                let icone = Size::new(14.0, 11.0);
                f.fill_rectangle(canto, icone, if self.escuro {
                    Color::rgb8(45, 49, 55) } else { Color::WHITE });
                f.stroke(&canvas::Path::rectangle(canto, icone), traco.clone());
                f.fill_rectangle(Point::new(canto.x, canto.y + 4.0), Size::new(14.0, 1.0), cor);
                f.fill_rectangle(Point::new(canto.x + 5.0, canto.y), Size::new(1.0, 11.0), cor);
                Point::new(canto.x + 7.0, canto.y + 11.0)
            } else {
                let area = &seta.origem;
                if area.r0 != area.r1 || area.c0 != area.c1 {
                    self.contornar(f, area, colunas, linhas, cor);
                }
                ponto(area.r0, area.c0)
            };
            let linha = canvas::Path::line(origem, destino);
            if seta.externa {
                f.stroke(&linha, canvas::Stroke {
                    line_dash: canvas::LineDash { segments: &[4.0, 3.0], offset: 0 },
                    ..traco
                });
            } else {
                f.stroke(&linha, traco);
                f.fill(&canvas::Path::circle(origem, 3.0), cor);
            }
            let (dx, dy) = (destino.x - origem.x, destino.y - origem.y);
            let comprimento = (dx * dx + dy * dy).sqrt();
            if comprimento < 1.0 { continue; }
            let (ux, uy) = (dx / comprimento, dy / comprimento);
            let ponta = canvas::Path::new(|p| {
                p.move_to(destino);
                p.line_to(Point::new(destino.x - ux * 9.0 - uy * 4.0, destino.y - uy * 9.0 + ux * 4.0));
                p.line_to(Point::new(destino.x - ux * 9.0 + uy * 4.0, destino.y - uy * 9.0 - ux * 4.0));
                p.close();
            });
            f.fill(&ponta, cor);
        }
    }

    /// Contorno do intervalo precedente, só na parte visível.
    fn contornar(&self, f: &mut Frame, area: &feathersheet_core::celulas::refs::Rect,
        colunas: &[Faixa], linhas: &[Faixa], cor: Color) {
        let cols: Vec<_> = colunas.iter().filter(|c| c.0 >= area.c0 && c.0 <= area.c1).collect();
        let rows: Vec<_> = linhas.iter().filter(|r| r.0 >= area.r0 && r.0 <= area.r1).collect();
        let (Some(c0), Some(c1), Some(r0), Some(r1)) = (cols.first(), cols.last(), rows.first(), rows.last())
            else { return };
        let canto = Point::new(c0.1, r0.1);
        let tamanho = Size::new(c1.1 + c1.2 - c0.1, r1.1 + r1.2 - r0.1);
        f.stroke(&canvas::Path::rectangle(canto, tamanho),
            canvas::Stroke::default().with_color(cor).with_width(1.5));
    }
}

#[cfg(test)]
mod testes {
    use super::centro;

    #[test]
    fn centro_dentro_e_fora_da_tela() {
        let faixas = [(0, 90.0, 100.0), (5, 190.0, 100.0), (6, 290.0, 100.0)];
        assert_eq!(centro(5, &faixas, 90.0, 800.0), 240.0);
        assert_eq!(centro(50, &faixas, 90.0, 800.0), 800.0);
        assert_eq!(centro(2, &faixas, 90.0, 800.0), 90.0);
    }
}
