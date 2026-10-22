//! Miniatura real de um modelo da tela inicial: o motor monta a pasta do
//! modelo e o canto superior esquerdo da primeira aba é desenhado em escala,
//! com os mesmos valores formatados, estilos, mesclas e formatação condicional
//! que a grade mostraria. Montado uma vez por sessão; o tema (cores padrão)
//! é resolvido na hora de desenhar.
use crate::condicional;
use crate::grade::ALTURA_PADRAO;
use feather_ui::tema::{COR_BORDA, COR_BORDA_SUAVE, COR_PAINEL, COR_TEXTO};
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::style::{effective_align, format_value, Align, VAlign};
use feathersheet_core::modelos;
use feathersheet_core::planilha::workbook::Workbook;
use crate::compat::alignment; use crate::compat::canvas; use pelicano::Color; use pelicano::Point; use crate::compat::Rectangle; use pelicano::Size;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

/// Largura da planilha capturada, em pixels da grade.
const LARGURA_CAPTURA: f32 = 820.0;
const FONTE_PADRAO: f32 = 13.0;

struct Celula {
    x: f32, y: f32, w: f32, h: f32,
    fundo: Option<Color>,
    texto: String,
    /// `None` = cor do texto do tema.
    cor: Option<Color>,
    negrito: bool,
    tamanho: f32,
    alinhar: alignment::Horizontal,
    topo: bool,
    /// Barra de dados: fração inicial, final e cor.
    barra: Option<(f32, f32, Color)>,
    /// Topo, direita, base, esquerda.
    bordas: [bool; 4],
    cor_borda: Color,
}

/// O recorte já calculado, em pixels da grade.
pub(super) struct Quadro {
    largura: f32,
    altura: f32,
    xs: Vec<f32>,
    ys: Vec<f32>,
    celulas: Vec<Celula>,
}

fn cor_hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 { return None; }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color::rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Recorta a primeira aba de `w` numa área com a proporção `largura / altura`.
fn capturar(w: &Workbook, proporcao: f32) -> Quadro {
    let altura_captura = LARGURA_CAPTURA / proporcao;
    let Some(aba) = w.aba(0) else {
        return Quadro { largura: LARGURA_CAPTURA, altura: altura_captura, xs: vec![], ys: vec![], celulas: vec![] };
    };
    let s = &aba.sheet;
    let mut xs = vec![0.0f32];
    while *xs.last().unwrap() < LARGURA_CAPTURA {
        let c = xs.len() as u32 - 1;
        let largura = s.col_widths.get(&c).map(|&v| v as f32).unwrap_or_else(|| crate::grade::largura_padrao_da(s));
        xs.push(xs.last().unwrap() + largura);
    }
    let mut ys = vec![0.0f32];
    while *ys.last().unwrap() < altura_captura {
        let r = ys.len() as u32 - 1;
        let altura = s.row_heights.get(&r).map(|&v| v as f32).unwrap_or(ALTURA_PADRAO);
        ys.push(ys.last().unwrap() + altura);
    }
    let (colunas, linhas) = (xs.len() as u32 - 1, ys.len() as u32 - 1);
    let mesclas: Vec<[u32; 4]> = s.mc.as_deref()
        .and_then(|m| serde_json::from_str::<Vec<Rect>>(m).ok()).unwrap_or_default()
        .into_iter().map(|m| [m.r0, m.c0, m.r1, m.c1]).collect();
    let mut cobertas = HashSet::new();
    for [r0, c0, r1, c1] in &mesclas {
        for r in *r0..=*r1 {
            for c in *c0..=*c1 {
                if (r, c) != (*r0, *c0) { cobertas.insert((r, c)); }
            }
        }
    }
    let mut condicional = condicional::Estado::default();
    let mut celulas = Vec::new();
    for r in 0..linhas {
        for c in 0..colunas {
            if cobertas.contains(&(r, c)) { continue; }
            let (r1, c1) = mesclas.iter().find(|m| m[0] == r && m[1] == c)
                .map(|m| (m[2].min(linhas - 1), m[3].min(colunas - 1))).unwrap_or((r, c));
            let estilo = s.estilo_resolvido(r, c);
            let valor = w.valor(0, r, c);
            let texto = format_value(&valor, estilo.as_ref());
            let formato = condicional.formato(w, 0, r, c);
            let cond = formato.as_ref().and_then(|f| f.style.as_ref());
            let fundo = cond.and_then(|e| e.bg.as_deref()).and_then(cor_hex)
                .or_else(|| estilo.as_ref().and_then(|e| e.bg.as_deref()).and_then(cor_hex));
            let bordas = estilo.as_ref().and_then(|e| e.borders.as_ref());
            if texto.is_empty() && fundo.is_none() && bordas.is_none() { continue; }
            let cor = cond.and_then(|e| e.color.as_deref()).and_then(cor_hex)
                .or_else(|| estilo.as_ref().and_then(|e| e.color.as_deref()).and_then(cor_hex));
            let negrito = cond.and_then(|e| e.bold)
                .unwrap_or_else(|| estilo.as_ref().is_some_and(|e| e.bold));
            let barra = formato.as_ref().and_then(|f| f.bar.as_ref())
                .and_then(|b| Some((b.from as f32, b.to as f32, cor_hex(&b.color)?)));
            celulas.push(Celula {
                x: xs[c as usize], y: ys[r as usize],
                w: xs[c1 as usize + 1] - xs[c as usize], h: ys[r1 as usize + 1] - ys[r as usize],
                fundo, texto, cor, negrito,
                tamanho: estilo.as_ref().and_then(|e| e.font_size).unwrap_or(FONTE_PADRAO),
                alinhar: match effective_align(&valor, estilo.as_ref()) {
                    Align::Right => pelicano::Align::End,
                    Align::Center => pelicano::Align::Center,
                    _ => pelicano::Align::Start,
                },
                topo: estilo.as_ref().is_some_and(|e| e.valign == VAlign::Top),
                barra,
                bordas: bordas.map(|b| [b.top, b.right, b.bottom, b.left]).unwrap_or([false; 4]),
                cor_borda: bordas.and_then(|b| b.color.as_deref()).and_then(cor_hex)
                    .unwrap_or(Color::rgb8(0xD1, 0xD5, 0xDB)),
            });
        }
    }
    Quadro { largura: LARGURA_CAPTURA, altura: altura_captura, xs, ys, celulas }
}

/// O recorte de cada modelo (pelo `id`), montado na primeira vez que a tela inicial pede.
pub(super) fn quadro(id: &str, proporcao: f32) -> Option<Arc<Quadro>> {
    static QUADROS: OnceLock<Vec<(&'static str, Arc<Quadro>)>> = OnceLock::new();
    QUADROS.get_or_init(|| {
        modelos::lista().iter()
            .filter_map(|m| Some((m.id, Arc::new(capturar(&modelos::criar(m.id)?, proporcao)))))
            .collect()
    }).iter().find(|(i, _)| *i == id).map(|(_, q)| q.clone())
}

/// Programa de Canvas que pinta um [`Quadro`] na largura do cartão.
pub(super) struct Miniatura(pub(super) Arc<Quadro>);

impl<M> canvas::Program<M> for Miniatura {
    type State = ();
    fn draw(
        &self, _state: &(), renderer: &mut pelicano::Renderer, bounds: Rectangle,
        _theme: &pelicano::Theme, _cursor: Option<Point>,
    ) {
        let q = &self.0;
        let mut f = canvas::Frame::new(renderer, bounds);
        let k = (bounds.width / q.largura).min(bounds.height / q.altura);
        f.fill_rectangle(Point::ORIGIN, bounds.size(), COR_PAINEL());
        // Linhas de grade apagadas, como a planilha vazia mostra.
        let grade = canvas::Stroke::default().with_color(Color { a: 0.6, ..COR_BORDA_SUAVE() }).with_width(0.5);
        for x in &q.xs {
            f.stroke(&canvas::Path::line(Point::new(x * k, 0.0), Point::new(x * k, bounds.height)), grade.clone());
        }
        for y in &q.ys {
            f.stroke(&canvas::Path::line(Point::new(0.0, y * k), Point::new(bounds.width, y * k)), grade.clone());
        }
        for c in &q.celulas {
            let (x, y, w, h) = (c.x * k, c.y * k, c.w * k, c.h * k);
            if let Some(fundo) = c.fundo {
                f.fill_rectangle(Point::new(x, y), Size::new(w, h), fundo);
            }
            if let Some((de, ate, cor)) = c.barra {
                let m = 1.5 * k.max(0.5);
                f.fill_rectangle(Point::new(x + w * de, y + m), Size::new((w * (ate - de)).max(0.0), h - 2.0 * m),
                    Color { a: 0.75, ..cor });
            }
            let traco = canvas::Stroke::default().with_color(c.cor_borda).with_width(if c.bordas == [false, false, false, true] { 1.5 } else { 0.7 });
            let lados = [
                (Point::new(x, y), Point::new(x + w, y)),
                (Point::new(x + w, y), Point::new(x + w, y + h)),
                (Point::new(x, y + h), Point::new(x + w, y + h)),
                (Point::new(x, y), Point::new(x, y + h)),
            ];
            for (ligado, (a, b)) in c.bordas.iter().zip(lados) {
                if *ligado { f.stroke(&canvas::Path::line(a, b), traco.clone()); }
            }
            if c.texto.is_empty() { continue; }
            let tamanho = (c.tamanho * k).max(2.5);
            let folga = 4.0 * k;
            let px = match c.alinhar {
                pelicano::Align::End => x + w - folga,
                pelicano::Align::Center => x + w / 2.0,
                _ => x + folga,
            };
            let (py, vertical) = if c.topo { (y + folga, pelicano::Align::Start) }
                else { (y + h / 2.0, pelicano::Align::Center) };
            f.fill_text(canvas::Text {
                content: c.texto.clone(),
                position: Point::new(px, py),
                color: c.cor.unwrap_or_else(COR_TEXTO),
                size: tamanho.into(),
                font: if c.negrito {
                    pelicano::Font { weight: 700, ..pelicano::Font::DEFAULT }
                } else { pelicano::Font::DEFAULT },
                horizontal_alignment: c.alinhar,
                vertical_alignment: vertical,
                ..canvas::Text::default()
            });
        }
        f.stroke(&canvas::Path::rectangle(Point::new(0.5, 0.5), Size::new(bounds.width - 1.0, bounds.height - 1.0)),
            canvas::Stroke::default().with_color(COR_BORDA()).with_width(1.0));
        f.fim();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_modelo_tem_recorte_com_titulo_e_cores() {
        for m in modelos::lista().iter().filter(|m| m.id != "em-branco") {
            let q = capturar(&modelos::criar(m.id).unwrap(), 1.45);
            assert!(q.celulas.iter().any(|c| !c.texto.is_empty()), "{}: sem texto", m.id);
            assert!(q.celulas.iter().any(|c| c.fundo.is_some()), "{}: sem cor", m.id);
            assert!(q.xs.len() > 2 && q.ys.len() > 4, "{}: recorte pequeno", m.id);
        }
    }
}
