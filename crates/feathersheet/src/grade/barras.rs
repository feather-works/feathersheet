//! Barras de rolagem da grade, desenhadas no próprio Canvas (vertical à
//! direita, horizontal embaixo), como no Excel. O curso da barra segue a
//! área usada da aba mais uma página, não as 100 milhões de linhas: com o
//! teto inteiro a alça viraria um traço e um pixel saltaria milhares de linhas.
use super::*;

/// Espessura da barra em pixels de tela (não acompanha o zoom).
pub(crate) const ESPESSURA: f32 = 13.0;
/// Alça nunca menor que isto, para continuar fácil de pegar.
const ALCA_MINIMA: f32 = 28.0;

/// Arrasto da alça em andamento; o curso é congelado no começo para a alça
/// não fugir do cursor quando a área visível passa do fim dos dados.
#[derive(Clone, Copy, Debug)]
pub struct ArrastoBarra {
    vertical: bool,
    /// Distância do cursor até o começo da alça, no momento do clique.
    pega: f32,
    curso: Curso,
}

/// Geometria de uma barra, em pixels de tela relativos ao Canvas.
#[derive(Clone, Copy, Debug)]
struct Curso {
    /// Trilha: começo e comprimento no eixo da barra.
    inicio: f32,
    comprimento: f32,
    /// Primeiro índice rolável (depois das linhas/colunas congeladas).
    fixas: u32,
    /// Índices que cabem na tela e a origem atual.
    visiveis: u32,
    origem: u32,
    /// Fim do curso: último índice que pode ficar no topo/à esquerda.
    ultimo: u32,
}

impl Curso {
    fn tamanho_alca(&self) -> f32 {
        let total = (self.ultimo - self.fixas + self.visiveis).max(1) as f32;
        (self.comprimento * self.visiveis as f32 / total)
            .clamp(ALCA_MINIMA.min(self.comprimento), self.comprimento)
    }

    /// Começo da alça na trilha.
    fn posicao_alca(&self) -> f32 {
        let curso = self.ultimo.saturating_sub(self.fixas);
        let fracao = if curso == 0 { 0.0 }
            else { (self.origem.saturating_sub(self.fixas)) as f32 / curso as f32 };
        self.inicio + (self.comprimento - self.tamanho_alca()) * fracao.clamp(0.0, 1.0)
    }

    /// Origem correspondente à alça começando em `pos` (pixels de tela).
    fn origem_em(&self, pos: f32) -> u32 {
        let livre = (self.comprimento - self.tamanho_alca()).max(1.0);
        let fracao = ((pos - self.inicio) / livre).clamp(0.0, 1.0);
        self.fixas + (fracao * self.ultimo.saturating_sub(self.fixas) as f32).round() as u32
    }
}

impl Grade<'_> {
    /// Curso das duas barras (vertical, horizontal) para o tamanho do Canvas.
    fn cursos(&self, bounds: Size) -> (Curso, Curso) {
        let usada = self.workbook.aba(self.aba).map(|a| a.sheet.used_rect());
        let (fim_linhas, fim_colunas) = usada.map(|r| (r.r1 + 1, r.c1 + 1)).unwrap_or((1, 1));
        let vis_linhas = (self.linhas_visiveis(bounds.height / self.zoom).0.len() as u32)
            .saturating_sub(self.congelar_linhas).max(1);
        let vis_colunas = (self.colunas_visiveis(bounds.width / self.zoom).0.len() as u32)
            .saturating_sub(self.congelar_colunas).max(1);
        let origem_linha = self.primeira_linha_visivel(bounds.height);
        let topo = CABECALHO_COLUNA * self.zoom;
        let esquerda = CABECALHO_LINHA * self.zoom;
        // Dados + uma página de folga; se a pessoa já foi além, o curso a acompanha.
        let ultimo_linha = (fim_linhas.max(origem_linha) + vis_linhas / 2)
            .max(self.congelar_linhas).min(MAX_ROWS - 1);
        let origem_coluna = self.primeira_coluna_visivel(bounds.width);
        let ultima_coluna = (fim_colunas.max(origem_coluna) + vis_colunas / 2)
            .max(self.congelar_colunas).min(MAX_COLS - 1);
        (
            Curso {
                inicio: topo, comprimento: (bounds.height - topo - ESPESSURA).max(1.0),
                fixas: self.congelar_linhas, visiveis: vis_linhas,
                origem: origem_linha, ultimo: ultimo_linha,
            },
            Curso {
                inicio: esquerda, comprimento: (bounds.width - esquerda - ESPESSURA).max(1.0),
                fixas: self.congelar_colunas, visiveis: vis_colunas,
                origem: origem_coluna, ultimo: ultima_coluna,
            },
        )
    }

    /// Desenha trilha e alça das duas barras por cima da grade, sem zoom.
    pub(super) fn desenhar_barras(
        &self, renderer: &mut Renderer, bounds: Rectangle, arrasto: Option<ArrastoBarra>,
    ) {
        use feather_ui::tema::{COR_BORDA, COR_PAINEL_FAIXA, COR_TEXTO_MUTED, COR_AZUL_ACENTO};
        let mut f = Frame::new(renderer, bounds);
        let (v, h) = self.cursos(bounds.size());
        let (w, a) = (bounds.width, bounds.height);
        let trilha = COR_PAINEL_FAIXA();
        let contorno = COR_BORDA();
        // Trilhas e o quadradinho do canto, onde as duas se encontram.
        f.fill_rectangle(Point::new(w - ESPESSURA, 0.0), Size::new(ESPESSURA, a), trilha);
        f.fill_rectangle(Point::new(0.0, a - ESPESSURA), Size::new(w, ESPESSURA), trilha);
        f.fill_rectangle(Point::new(w - ESPESSURA, 0.0), Size::new(1.0, a), contorno);
        f.fill_rectangle(Point::new(0.0, a - ESPESSURA), Size::new(w, 1.0), contorno);
        let alca = |f: &mut Frame, vertical: bool, c: &Curso| {
            let ativa = arrasto.is_some_and(|x| x.vertical == vertical);
            let mut cor = if ativa { COR_AZUL_ACENTO() } else { COR_TEXTO_MUTED() };
            cor.a = if ativa { 0.85 } else { 0.55 };
            let (pos, tam) = (c.posicao_alca(), c.tamanho_alca());
            let caminho = if vertical {
                canvas::Path::rounded_rectangle(Point::new(w - ESPESSURA + 3.0, pos + 1.0),
                    Size::new(ESPESSURA - 6.0, (tam - 2.0).max(4.0)), 4.0)
            } else {
                canvas::Path::rounded_rectangle(Point::new(pos + 1.0, a - ESPESSURA + 3.0),
                    Size::new((tam - 2.0).max(4.0), ESPESSURA - 6.0), 4.0)
            };
            f.fill(&caminho, cor);
        };
        alca(&mut f, true, &v);
        alca(&mut f, false, &h);
        f.fim();
    }

    /// `Some(true)` = sobre a barra vertical, `Some(false)` = horizontal.
    pub(super) fn barra_no_ponto(&self, p: Point, bounds: Size) -> Option<bool> {
        if p.x >= bounds.width - ESPESSURA && p.y >= CABECALHO_COLUNA * self.zoom
            && p.y < bounds.height - ESPESSURA { return Some(true); }
        if p.y >= bounds.height - ESPESSURA && p.x >= CABECALHO_LINHA * self.zoom
            && p.x < bounds.width - ESPESSURA { return Some(false); }
        None
    }

    /// Clique na barra: na alça começa o arrasto; na trilha pula uma página
    /// na direção do clique (como no Excel).
    pub(super) fn pressionar_barra(
        &self, p: Point, bounds: Size,
    ) -> Option<(Option<ArrastoBarra>, Option<Msg>)> {
        let fora = p.x >= bounds.width - ESPESSURA || p.y >= bounds.height - ESPESSURA;
        let Some(vertical) = self.barra_no_ponto(p, bounds) else {
            // Canto/sobra das trilhas: engole o clique sem selecionar célula.
            return fora.then_some((None, None));
        };
        let (v, h) = self.cursos(bounds);
        let curso = if vertical { v } else { h };
        let eixo = if vertical { p.y } else { p.x };
        let (pos, tam) = (curso.posicao_alca(), curso.tamanho_alca());
        if eixo >= pos && eixo < pos + tam {
            return Some((Some(ArrastoBarra { vertical, pega: eixo - pos, curso }), None));
        }
        let pagina = curso.visiveis.max(1);
        let destino = if eixo < pos { curso.origem.saturating_sub(pagina).max(curso.fixas) }
            else { (curso.origem + pagina).min(curso.ultimo) };
        Some((None, Some(self.mensagem_rolar(vertical, destino))))
    }

    /// Cursor arrastando a alça: nova origem pela posição na trilha.
    pub(super) fn arrastar_barra(&self, arrasto: ArrastoBarra, p: Point) -> Msg {
        let eixo = if arrasto.vertical { p.y } else { p.x };
        let destino = arrasto.curso.origem_em(eixo - arrasto.pega);
        self.mensagem_rolar(arrasto.vertical, destino)
    }

    fn mensagem_rolar(&self, vertical: bool, destino: u32) -> Msg {
        if vertical { Msg::RolarPara(Some(destino), None) }
        else { Msg::RolarPara(None, Some(destino)) }
    }
}

#[cfg(test)]
mod testes {
    use super::Curso;

    fn curso() -> Curso {
        Curso { inicio: 30.0, comprimento: 400.0, fixas: 0, visiveis: 20, origem: 0, ultimo: 980 }
    }

    #[test]
    fn alca_proporcional_e_nos_extremos_da_trilha() {
        let c = curso();
        // 20 de 1000 dariam 8 px: vale o mínimo para continuar pegável.
        assert_eq!(c.tamanho_alca(), 28.0);
        let grande = Curso { visiveis: 200, ultimo: 800, ..c };
        assert!((grande.tamanho_alca() - 80.0).abs() < 0.01);
        assert_eq!(c.posicao_alca(), 30.0);
        let fim = Curso { origem: 980, ..c };
        assert!((fim.posicao_alca() + fim.tamanho_alca() - 430.0).abs() < 0.01);
    }

    #[test]
    fn arrastar_ate_o_fim_chega_ao_ultimo_e_nao_passa_do_comeco() {
        let c = curso();
        assert_eq!(c.origem_em(10_000.0), 980);
        assert_eq!(c.origem_em(-50.0), 0);
        let congelada = Curso { fixas: 3, ..c };
        assert_eq!(congelada.origem_em(0.0), 3);
    }
}
