//! Seletor visual interativo de orientação (-90° a +90°), como no Excel.
//! Canvas sem estado compartilhado: o ângulo e o modo vertical vêm do rascunho.
use super::*;
use crate::compat::canvas; use crate::compat::canvas::Frame; use crate::compat::canvas::Path; use crate::compat::canvas::Program; use crate::compat::canvas::Stroke;
use crate::compat::mouse; use pelicano::Captured; use pelicano::Point; use crate::compat::Rectangle; use pelicano::Renderer; use pelicano::Size; use pelicano::Theme; use crate::compat::Vector;
use std::f32::consts::PI;

const LARGURA: f32 = 220.0;
const ALTURA: f32 = 204.0;
const CENTRO: Point = Point { x: 112., y: 102. };
const RAIO: f32 = 74.;

#[derive(Clone, Copy)]
struct Orientacao {
    graus: i16,
    vertical: bool,
}

pub(super) fn controle(graus: i16, vertical: bool) -> Element<'static, Msg> {
    crate::compat::canvas(Orientacao { graus: graus.clamp(-90,90), vertical })
        .width(Length::Fixed(LARGURA))
        .height(Length::Fixed(ALTURA))
        .into()
}

/// O semicírculo à direita recebe cliques; a faixa vertical à esquerda
/// alterna o texto empilhado sem modificar a orientação armazenada.
fn acao_clique(p: Point) -> Option<Evento> {
    if (3.0..=41.0).contains(&p.x) && (28.0..=174.0).contains(&p.y) {
        return Some(Evento::Efeito("textVertical"));
    }
    let dx = p.x - CENTRO.x;
    let dy = p.y - CENTRO.y;
    let raio = (dx*dx + dy*dy).sqrt();
    if dx < -8. || dx > RAIO + 16. || raio < 18. || raio > RAIO + 21. {
        return None;
    }
    let graus = (dy.atan2(dx) * 180. / PI).round() as i16;
    Some(Evento::Grau(graus.clamp(-90, 90)))
}

impl Program<Msg> for Orientacao {
    type State = ();

    fn update(
        &self, _: &mut (), ev: &canvas::Event, bounds: Rectangle, cursor: Option<Point>,
        publish: &mut dyn FnMut(Msg),
    ) -> Captured {
        if matches!(ev, canvas::Event::MousePressed(mouse::Button::Left)) {
            if let Some(p) = cursor.filter(|p| bounds.contains(*p))
                .map(|p| Point::new(p.x - bounds.x, p.y - bounds.y)) {
                if let Some(acao) = acao_clique(p) {
                    publish(Msg::FormatoCelula(acao));
                    return Captured::Yes;
                }
            }
        }
        Captured::No
    }

    fn draw(
        &self, _: &(), renderer: &mut Renderer,
        bounds: Rectangle, _: &Theme, _: Option<Point>,
    ) {
        let mut f=Frame::new(renderer,bounds);
        let tinta=COR_TEXTO();
        let suave=COR_TEXTO_MUTED();
        let cor_ativa=COR_AZUL_ACENTO();
        let borda=COR_BORDA();
        // Faixa lateral com a palavra na vertical, também clicável.
        let vertical=Path::rectangle(Point::new(5., 30.), Size::new(34., 145.));
        f.stroke(&vertical, Stroke::default()
            .with_color(if self.vertical {cor_ativa} else {borda})
            .with_width(if self.vertical {2.} else {1.}));
        f.fill_text(canvas::Text {
            content: "T\ne\nx\nt\no".into(),
            position: Point::new(22., 102.),
            color: if self.vertical {cor_ativa} else {tinta},
            size: crate::compat::Pixels(14.),
            horizontal_alignment: pelicano::Align::Center,
            vertical_alignment: pelicano::Align::Center,
            ..Default::default()
        });
        // Ticks de -90° a 90° distribuídos na semicircunferência direita.
        for deg in (-90..=90).step_by(15) {
            let rad=(deg as f32).to_radians();
            let p=Point::new(
                CENTRO.x+RAIO*rad.cos(),
                CENTRO.y+RAIO*rad.sin(),
            );
            let tick=Path::circle(p, if deg%45==0 {3.1} else {2.1});
            f.fill(&tick, suave);
        }
        let origem=Path::circle(CENTRO,3.0);
        f.fill(&origem,cor_ativa);
        // Orientação atual — o ponto e a linha acompanham o ângulo.
        let rad=(self.graus as f32).to_radians();
        let destino=Point::new(
            CENTRO.x+(RAIO+2.)*rad.cos(),
            CENTRO.y+(RAIO+2.)*rad.sin(),
        );
        let caminho=Path::new(|builder|{
            builder.move_to(CENTRO);
            builder.line_to(destino);
        });
        f.stroke(&caminho, Stroke::default()
            .with_color(cor_ativa).with_width(2.0));
        f.fill(&Path::circle(destino,4.5),cor_ativa);
        // A prévia do texto acompanha a mesma rotação que será salva na célula.
        f.with_save(|f|{
            f.translate(Vector::new(CENTRO.x, CENTRO.y - 12.));
            f.rotate(crate::compat::Radians(rad));
            f.fill_text(canvas::Text {
                content: tr!("Texto").into(),
                position: Point::ORIGIN,
                color: tinta,
                size: crate::compat::Pixels(15.),
                horizontal_alignment: pelicano::Align::Center,
                vertical_alignment: pelicano::Align::Center,
                ..Default::default()
            });
        });
        f.fim();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn dial_converte_cliques_em_graus_e_modo_vertical() {
        assert!(matches!(acao_clique(Point::new(22.,100.)), Some(Evento::Efeito("textVertical"))));
        assert!(matches!(acao_clique(Point::new(186.,102.)), Some(Evento::Grau(0))));
        assert!(matches!(acao_clique(Point::new(112.,28.)), Some(Evento::Grau(-90))));
        assert!(matches!(acao_clique(Point::new(112.,176.)), Some(Evento::Grau(90))));
        assert!(acao_clique(Point::new(1.,1.)).is_none());
    }

    #[test]
    fn componente_renderiza_sem_depender_da_planilha() {
        let _: Element<'static,Msg> = controle(-45,false);
        let _: Element<'static,Msg> = controle(90,true);
    }
}
