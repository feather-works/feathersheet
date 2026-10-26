//! Estrutura de tópicos na grade, como no Excel: os colchetes de cada nível
//! e o [+]/[−] no resumo do grupo à esquerda dos números das linhas (e no
//! alto das letras das colunas), e os botões de nível 1, 2, 3… no canto.
//! Só olha os índices visíveis; os níveis vêm do motor
//! (`Sheet::topicos`). Desenho e clique usam a mesma geometria.
use super::*;
use super::congelado::Faixa;
use crate::compat::Retangulo as _;
use crate::app::topicos_ui::Evento;
use feathersheet_core::planilha::sheet::topicos::EixoTopicos;

/// Largura de cada nível na faixa das linhas.
const PASSO_LINHA: f32 = 12.0;
/// Altura de cada nível no alto das colunas.
const PASSO_COLUNA: f32 = 9.0;
/// Lado do [+]/[−] e dos botões de nível.
const CAIXA: f32 = 9.0;
const BOTAO: f32 = 12.0;

/// Centro x do nível `n` (1, 2…) na faixa das linhas.
fn x_nivel(n: u8) -> f32 { 3.0 + (n - 1) as f32 * PASSO_LINHA + PASSO_LINHA / 2.0 }
/// Centro y do nível `n` no alto das colunas.
fn y_nivel(n: u8) -> f32 { 1.0 + (n - 1) as f32 * PASSO_COLUNA + PASSO_COLUNA / 2.0 }

/// O índice é o resumo de um grupo de nível `n`?
fn resumo(eixo: &EixoTopicos, i: u32, n: u8) -> bool {
    i > 0 && eixo.nivel(i) < n && eixo.nivel(i - 1) >= n
}

/// Os botões de nível do canto: `(eixo é coluna, nível, retângulo)`.
fn botoes(linhas: u8, colunas: u8) -> Vec<(bool, u8, Rectangle)> {
    let mut v = Vec::new();
    for (coluna, maior, y) in [(false, linhas, 2.0), (true, colunas, 16.0)] {
        if maior == 0 { continue; }
        for n in 1..=maior + 1 {
            let x = 3.0 + (n - 1) as f32 * (BOTAO + 1.0);
            if x + BOTAO > CABECALHO_LINHA - 2.0 { break; }
            v.push((coluna, n, Rectangle::new(Point::new(x, y), Size::new(BOTAO, BOTAO))));
        }
    }
    v
}

fn dentro(r: &Rectangle, p: Point) -> bool {
    p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
}

impl Grade<'_> {
    fn topicos(&self) -> Option<&feathersheet_core::planilha::sheet::topicos::Topicos> {
        self.workbook.aba(self.aba).map(|a| &a.sheet.topicos).filter(|t| !t.vazio())
    }

    pub(super) fn desenhar_topicos(&self, f: &mut Frame, colunas: &[Faixa], linhas: &[Faixa], cor: Color, fundo: Color) {
        let Some(t) = self.topicos() else { return };
        let traco = canvas::Stroke::default().with_color(cor).with_width(1.0);
        let caixa = |f: &mut Frame, centro: Point, aberto: bool| {
            let canto = Point::new(centro.x - CAIXA / 2.0, centro.y - CAIXA / 2.0);
            f.fill_rectangle(canto, Size::new(CAIXA, CAIXA), fundo);
            f.stroke(&canvas::Path::rectangle(canto, Size::new(CAIXA, CAIXA)), traco.clone());
            f.fill_rectangle(Point::new(canto.x + 2.0, centro.y - 0.5), Size::new(CAIXA - 4.0, 1.0), cor);
            if !aberto {
                f.fill_rectangle(Point::new(centro.x - 0.5, canto.y + 2.0), Size::new(1.0, CAIXA - 4.0), cor);
            }
        };
        let (ml, mc) = (t.linhas.nivel_maior(), t.colunas.nivel_maior());
        for &(r, y, h) in linhas {
            for n in 1..=ml {
                let x = x_nivel(n);
                if t.linhas.nivel(r) >= n {
                    f.fill_rectangle(Point::new(x - 0.5, y), Size::new(1.0, h), cor);
                    // Fim do colchete: um risco curto na última linha do grupo.
                    if t.linhas.nivel(r + 1) < n {
                        f.fill_rectangle(Point::new(x - 0.5, y + h - 1.0), Size::new(4.0, 1.0), cor);
                    }
                } else if resumo(&t.linhas, r, n) {
                    caixa(f, Point::new(x, y + h / 2.0), !t.linhas.recolhidas.contains(&r));
                }
            }
        }
        for &(c, x, w) in colunas {
            for n in 1..=mc {
                let y = y_nivel(n);
                if t.colunas.nivel(c) >= n {
                    f.fill_rectangle(Point::new(x, y - 0.5), Size::new(w, 1.0), cor);
                    if t.colunas.nivel(c + 1) < n {
                        f.fill_rectangle(Point::new(x + w - 1.0, y - 0.5), Size::new(1.0, 4.0), cor);
                    }
                } else if resumo(&t.colunas, c, n) {
                    caixa(f, Point::new(x + CAIXA / 2.0 + 3.0, y), !t.colunas.recolhidas.contains(&c));
                }
            }
        }
        for (_, n, r) in botoes(ml, mc) {
            f.fill_rectangle(r.position(), r.size(), fundo);
            f.stroke(&canvas::Path::rectangle(r.position(), r.size()), traco.clone());
            f.fill_text(Text {
                content: n.to_string(),
                position: Point::new(r.x + r.width / 2.0, r.y + r.height / 2.0),
                color: cor,
                size: crate::compat::Pixels(9.0),
                horizontal_alignment: pelicano::Align::Center,
                vertical_alignment: pelicano::Align::Center,
                ..Text::default()
            });
        }
    }

    /// O clique num botão de nível ou num [+]/[−]. `p` em unidades da grade.
    pub(super) fn clique_topicos(&self, p: Point, colunas: &[Faixa], linhas: &[Faixa]) -> Option<Msg> {
        let t = self.topicos()?;
        let (ml, mc) = (t.linhas.nivel_maior(), t.colunas.nivel_maior());
        if p.x < CABECALHO_LINHA && p.y < CABECALHO_COLUNA {
            return botoes(ml, mc).into_iter().find(|(_, _, r)| dentro(r, p))
                .map(|(coluna, n, _)| Msg::Topicos(Evento::Nivel(coluna, n)));
        }
        let perto = |a: f32, b: f32| (a - b).abs() <= CAIXA / 2.0 + 1.5;
        if p.x < CABECALHO_LINHA && ml > 0 {
            let &(r, y, h) = linhas.iter().find(|&&(_, y, h)| p.y >= y && p.y < y + h)?;
            return (1..=ml).find(|&n| resumo(&t.linhas, r, n) && perto(p.x, x_nivel(n)) && perto(p.y, y + h / 2.0))
                .map(|n| Msg::Topicos(Evento::Alternar(false, n, r)));
        }
        if p.y < CABECALHO_COLUNA && mc > 0 {
            let &(c, x, _) = colunas.iter().find(|&&(_, x, w)| p.x >= x && p.x < x + w)?;
            return (1..=mc).find(|&n| resumo(&t.colunas, c, n) && perto(p.y, y_nivel(n)) && perto(p.x, x + CAIXA / 2.0 + 3.0))
                .map(|n| Msg::Topicos(Evento::Alternar(true, n, c)));
        }
        None
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn botoes_cabem_no_canto_e_acertam_o_clique() {
        let b = botoes(2, 1);
        assert_eq!(b.iter().filter(|(c, ..)| !c).count(), 3, "níveis 1, 2 e 3 das linhas");
        assert_eq!(b.iter().filter(|(c, ..)| *c).count(), 2);
        assert!(b.iter().all(|(_, _, r)| r.x + r.width <= CABECALHO_LINHA && r.y + r.height <= CABECALHO_COLUNA));
        let mut e = EixoTopicos::default();
        for i in 2..=4 { e.niveis.insert(i, 1); }
        assert!(resumo(&e, 5, 1) && !resumo(&e, 4, 1) && !resumo(&e, 1, 1));
    }
}
