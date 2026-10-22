//! Alça de preenchimento, como no Excel: o quadradinho no canto de baixo à
//! direita da seleção. Arrastar mostra o destino tracejado e, ao soltar, o
//! app preenche pelo motor (`Sheet::preencher`: série, fórmula relativa e
//! estilo, um Ctrl+Z). Duplo clique continua até o fim da coluna vizinha.
//! A grade só desenha e diz o destino; quem escreve é o app.
use super::*;
use super::congelado::Faixa;
use feathersheet_core::{celulas::refs::Rect, operacoes::fill::fill_target};

/// Lado do quadradinho, em pixels da tela (não cresce com o zoom).
const LADO: f32 = 7.0;
/// Folga do clique em volta do canto, em pixels da tela.
const FOLGA: f32 = 6.0;

/// Arrasto da alça em andamento: o retângulo que a prévia mostra.
#[derive(Debug, Clone)]
pub(super) struct ArrastoAlca {
    pub destino: Rect,
}

impl Grade<'_> {
    /// O canto de baixo à direita da seleção (unidades da grade, sem zoom),
    /// quando a alça aparece: fora da edição, com uma área só e com o canto
    /// à vista. Coluna ou linha inteira não tem alça (o canto não existe).
    fn canto_da_alca(&self, viewport: Size) -> Option<Point> {
        if self.rascunho.is_some() || !self.areas_extras.is_empty() {
            return None;
        }
        let a = self.area.normalized();
        if a.r1 >= MAX_ROWS - 1 || a.c1 >= MAX_COLS - 1 {
            return None;
        }
        let (x, y, w, h) = self.posicao_da_celula(a.r1, a.c1, viewport)?;
        Some(Point::new(x + w, y + h))
    }

    /// O ponto (da tela, relativo ao Canvas) está sobre a alça?
    pub(super) fn sobre_alca(&self, p: Point, tela: Size) -> bool {
        let viewport = Size::new(tela.width / self.zoom, tela.height / self.zoom);
        self.canto_da_alca(viewport).is_some_and(|c| {
            (p.x - c.x * self.zoom).abs() <= FOLGA && (p.y - c.y * self.zoom).abs() <= FOLGA
        })
    }

    /// Destino do arrasto com o ponteiro em `p`: a seleção estendida num eixo
    /// só, o de maior deslocamento (regra do motor, `fill_target`).
    pub(super) fn destino_da_alca(&self, p: Point, tela: Size) -> Option<Rect> {
        let (r, c) = self.celula_no_ponto(p, tela)?;
        Some(fill_target(&self.area.normalized(), r, c))
    }

    /// Borda de 2 px em volta da seleção de várias células, como no Excel
    /// (a célula ativa já tem a sua). Só os lados à vista: uma seleção que
    /// passa da tela não ganha uma borda falsa na beirada.
    fn contornar_selecao(&self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], cor: Color) {
        let a = self.area.normalized();
        if !self.areas_extras.is_empty() || (a.r0 == a.r1 && a.c0 == a.c1) {
            return;
        }
        let cols: Vec<_> = colunas.iter().filter(|c| c.0 >= a.c0 && c.0 <= a.c1).collect();
        let rows: Vec<_> = linhas.iter().filter(|r| r.0 >= a.r0 && r.0 <= a.r1).collect();
        let (Some(c0), Some(c1), Some(r0), Some(r1)) = (cols.first(), cols.last(), rows.first(), rows.last())
            else { return };
        let (x0, y0, x1, y1) = (c0.1, r0.1, c1.1 + c1.2, r1.1 + r1.2);
        let esp = 2.0;
        if r0.0 == a.r0 { f.fill_rectangle(Point::new(x0, y0), Size::new(x1 - x0, esp), cor); }
        if r1.0 == a.r1 { f.fill_rectangle(Point::new(x0, y1 - esp), Size::new(x1 - x0, esp), cor); }
        if c0.0 == a.c0 { f.fill_rectangle(Point::new(x0, y0), Size::new(esp, y1 - y0), cor); }
        if c1.0 == a.c1 { f.fill_rectangle(Point::new(x1 - esp, y0), Size::new(esp, y1 - y0), cor); }
    }

    /// A área do Ctrl+C: contorno tracejado andando, como o do Excel (o
    /// `CopiaPulso` redesenha a cada 100 ms). Só os lados à vista (a cópia que
    /// passa da tela não ganha borda na beirada).
    pub(super) fn desenhar_copiada(&self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], cor: Color) {
        let Some(a) = self.copiada.as_ref() else { return };
        let cols: Vec<_> = colunas.iter().filter(|c| c.0 >= a.c0 && c.0 <= a.c1).collect();
        let rows: Vec<_> = linhas.iter().filter(|r| r.0 >= a.r0 && r.0 <= a.r1).collect();
        let (Some(c0), Some(c1), Some(r0), Some(r1)) = (cols.first(), cols.last(), rows.first(), rows.last())
            else { return };
        let (x0, y0, x1, y1) = (c0.1, r0.1, c1.1 + c1.2, r1.1 + r1.2);
        let e = 1.0 / self.zoom.max(0.25);
        let traco = canvas::Stroke {
            line_dash: canvas::LineDash { segments: &[5.0, 3.0], offset: deslocamento_do_traco() },
            ..canvas::Stroke::default().with_color(cor).with_width(2.0 * e)
        };
        let lado = |f: &mut Frame, de: Point, ate: Point| f.stroke(&canvas::Path::line(de, ate), traco.clone());
        if r0.0 == a.r0 { lado(f, Point::new(x0, y0 + e), Point::new(x1, y0 + e)); }
        if r1.0 == a.r1 { lado(f, Point::new(x0, y1 - e), Point::new(x1, y1 - e)); }
        if c0.0 == a.c0 { lado(f, Point::new(x0 + e, y0), Point::new(x0 + e, y1)); }
        if c1.0 == a.c1 { lado(f, Point::new(x1 - e, y0), Point::new(x1 - e, y1)); }
    }

    /// O quadradinho e, durante o arrasto, o destino tracejado.
    pub(super) fn desenhar_alca(
        &self, f: &mut Frame, viewport: Size, arrasto: Option<&ArrastoAlca>,
        colunas: &[Faixa], linhas: &[Faixa], ativo: Color, fundo: Color,
    ) {
        let e = 1.0 / self.zoom.max(0.25);
        self.contornar_selecao(f, colunas, linhas, ativo);
        if let Some(a) = arrasto.filter(|a| a.destino != self.area.normalized()) {
            let d = &a.destino;
            let xs: Vec<_> = colunas.iter().filter(|(c, _, _)| *c >= d.c0 && *c <= d.c1).collect();
            let ys: Vec<_> = linhas.iter().filter(|(r, _, _)| *r >= d.r0 && *r <= d.r1).collect();
            if let (Some(c0), Some(c1), Some(r0), Some(r1)) = (xs.first(), xs.last(), ys.first(), ys.last()) {
                let (x0, y0) = (c0.1, r0.1);
                let (x1, y1) = (c1.1 + c1.2, r1.1 + r1.2);
                let caixa = canvas::Path::rectangle(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0));
                f.stroke(&caixa, canvas::Stroke {
                    line_dash: canvas::LineDash { segments: &[4.0, 3.0], offset: 0 },
                    ..canvas::Stroke::default().with_color(Color { a: 0.85, ..ativo }).with_width(1.5 * e)
                });
            }
        }
        let Some(canto) = self.canto_da_alca(viewport) else { return };
        let lado = LADO * e;
        let origem = Point::new(canto.x - lado / 2.0 - e, canto.y - lado / 2.0 - e);
        // Borda da cor do fundo em volta, para separar da grade e da borda azul.
        f.fill_rectangle(Point::new(origem.x - e, origem.y - e), Size::new(lado + 2.0 * e, lado + 2.0 * e), fundo);
        f.fill_rectangle(origem, Size::new(lado, lado), ativo);
    }
}

#[cfg(test)]
mod testes {
    use feathersheet_core::{celulas::refs::Rect, operacoes::fill::fill_target};

    #[test]
    fn destino_estende_um_eixo_so() {
        let sel = Rect::new(2, 1, 2, 1);
        // Mais para baixo que para o lado: estende as linhas.
        assert_eq!(fill_target(&sel, 6, 2), Rect::new(2, 1, 6, 1));
        // Mais para o lado: estende as colunas.
        assert_eq!(fill_target(&sel, 3, 5), Rect::new(2, 1, 2, 5));
        // Dentro da seleção: nada muda.
        assert_eq!(fill_target(&sel, 2, 1), sel);
    }
}

/// Quanto o tracejado do Ctrl+C andou: um passo a cada 100 ms, volta ao
/// começo a cada traço + espaço (8).
fn deslocamento_do_traco() -> usize {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    (ms / 100 % 8) as usize
}
