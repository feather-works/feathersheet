//! Hit-test do arraste de referências coloridas durante a edição de fórmulas.
//! A identificação das referências e a transformação do texto já pertencem
//! ao motor Rust `formula::ref_drag`. Este módulo cuida só do pixel Iced.
use super::{Grade, Point, Size};
use feathersheet_core::{celulas::refs::Rect, formula::ref_drag::Pegada};

#[derive(Debug, Clone, Copy)]
pub(super) struct ReferenciaAtiva {
    pub start: usize,
    pub pegada: Pegada,
    pub inicio: (u32, u32),
    pub destino: (u32, u32),
}

/// Aceita a borda externa ou a alça inferior direita, não o interior.
fn reconhecer(
    area: &Rect, celula: (u32, u32), x: f32, y: f32, w: f32, h: f32,
    borda: f32, alca: f32,
) -> Option<Pegada> {
    let (r,c) = celula;
    if !area.contains(r,c) || w <= 0.0 || h <= 0.0 { return None; }
    let dx = x.clamp(0.0, w);
    let dy = y.clamp(0.0, h);
    if r == area.r1 && c == area.c1 && w-dx <= alca && h-dy <= alca {
        return Some(Pegada::Redimensionar);
    }
    if (r == area.r0 && dy <= borda) ||
       (r == area.r1 && h-dy <= borda) ||
       (c == area.c0 && dx <= borda) ||
       (c == area.c1 && w-dx <= borda) {
        return Some(Pegada::Mover);
    }
    None
}

impl Grade<'_> {
    pub(super) fn pegar_referencia(&self, ponto: Point, viewport: Size) -> Option<ReferenciaAtiva> {
        if self.refs_formula.is_empty() { return None; }
        let celula = self.celula_no_ponto(ponto, viewport)?;
        let logico = Point::new(ponto.x/self.zoom, ponto.y/self.zoom);
        let (x,y,w,h) = self.posicao_da_celula(
            celula.0, celula.1,
            Size::new(viewport.width/self.zoom, viewport.height/self.zoom),
        )?;
        let borda = (5.0/self.zoom).min(w/2.0).min(h/2.0);
        let alca = (11.0/self.zoom).min(w/2.0).min(h/2.0);
        // Referências que se sobrepõem: vale a última desenhada.
        for caixa in self.refs_formula.iter().rev() {
            if let Some(pegada) = reconhecer(
                &caixa.rect, celula, logico.x-x, logico.y-y, w, h, borda, alca,
            ) {
                return Some(ReferenciaAtiva {
                    start: caixa.start, pegada, inicio: celula, destino: celula,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn borda_arrasta_interior_seleciona_e_quina_estica() {
        let a = Rect::new(1, 1, 3, 2);
        assert_eq!(reconhecer(&a,(1,1),3.,15.,100.,28.,5.,10.),Some(Pegada::Mover));
        assert_eq!(reconhecer(&a,(2,1),50.,14.,100.,28.,5.,10.),None);
        assert_eq!(reconhecer(&a,(3,2),95.,25.,100.,28.,5.,10.),Some(Pegada::Redimensionar));
        assert_eq!(reconhecer(&a,(3,1),50.,27.,100.,28.,5.,10.),Some(Pegada::Mover));
        assert_eq!(reconhecer(&a,(4,2),95.,25.,100.,28.,5.,10.),None);
    }
    #[test]
    fn arraste_compartilha_motor_rust_e_preserva_travas() {
        use feathersheet_core::formula::ref_drag::{reference_boxes,drag_rect,rewrite_box};
        let t = "=SOMA($B$1:$B$8;C1)";
        let caixa = reference_boxes(t).remove(0);
        let destino = drag_rect(&caixa.rect,Pegada::Mover,(0,1),(2,2));
        assert_eq!(rewrite_box(t,&caixa,&destino),"=SOMA($C$3:$C$10;C1)");
    }
}
