//! WordArt na camada de objetos: as peças comuns da suíte montadas para a planilha.
//!
//! - Contorno das letras pela fonte do pelicano: `feather_ui::texto_contorno` (miolos preservados).
//! - Deformação (Reto, Arco, Onda, Inclinado) e estilos prontos: `feather_shapes::wordart`.
//! - Modelo no JSON `ob`: `tipo: "wordart"`, `forma` = deformação, `ajustes[0]` = intensidade,
//!   `texto`, cores em `aparencia`, `estilo` e `sombra` em chaves próprias (ver
//!   `feathersheet_io::objetos_aba`).
//!
//! Os polígonos saem num tamanho de referência e ficam em cache por (texto, deformação,
//! intensidade); o desenho só escala e centra na caixa do objeto, então arrastar não refaz
//! a fonte a cada quadro.
use super::camada::cor_hex;
use feather_shapes::wordart::{self as comum, EstiloWordArt, Transformacao};
use feathersheet_io::objetos_aba::{Aparencia, ObjetoSolto};
use crate::compat::canvas::Frame; use crate::compat::canvas::Path; use crate::compat::canvas::Stroke;
use pelicano::Color; use pelicano::Point;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// Corpo da fonte de referência dos contornos, em px.
const CORPO: f32 = 64.0;
/// Folga entre o texto e a moldura, fração do lado.
const MARGEM: f32 = 0.06;
/// Texto do WordArt novo (o mesmo do FeatherSlide).
pub(super) const TEXTO_PADRAO: &str = "Seu texto";
/// Intensidade padrão da deformação.
pub(super) const INTENSIDADE_PADRAO: f32 = 0.5;
/// Caixa do WordArt novo, em px lógicos.
pub(super) const TAMANHO_NOVO: (f32, f32) = (320.0, 80.0);

/// Polígonos deformados no tamanho de referência e a caixa que os envolve.
type Contornos = Arc<(Vec<Vec<[f32; 2]>>, [f32; 4])>;

thread_local! {
    static CACHE: RefCell<HashMap<(String, Transformacao, u32), Contornos>> = RefCell::new(HashMap::new());
}

/// Sombra pronta para a tela: cor e deslocamento em px lógicos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SombraTela {
    pub cor: Color,
    pub dx: f32,
    pub dy: f32,
}

/// O que a camada precisa de um WordArt além do que todo objeto tem.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WordArtTela {
    pub modo: Transformacao,
    pub intensidade: f32,
    pub sombra: Option<SombraTela>,
}

impl WordArtTela {
    pub(super) fn de(o: &ObjetoSolto) -> Self {
        Self { modo: Transformacao::ler(&o.forma), intensidade: intensidade(o), sombra: sombra(o) }
    }
}

pub(super) fn intensidade(o: &ObjetoSolto) -> f32 {
    o.ajustes.first().copied().filter(|v| v.is_finite()).unwrap_or(INTENSIDADE_PADRAO).clamp(-1.0, 1.0)
}

/// A chave do estilo pronto gravada no objeto (`None` se não houver).
pub(super) fn chave_estilo(o: &ObjetoSolto) -> Option<&str> {
    o.extras.get("estilo").and_then(Value::as_str)
}

fn sombra(o: &ObjetoSolto) -> Option<SombraTela> {
    let s = o.extras.get("sombra")?;
    let cor = s.get("cor")?.as_str()?;
    let num = |k: &str| s.get(k).and_then(Value::as_f64).map_or(3.0, |v| v as f32);
    Some(SombraTela { cor: cor_hex(cor, o.aparencia.transparencia), dx: num("dx"), dy: num("dy") })
}

/// Copia o estilo pronto para o objeto: cores em `aparencia`, a chave e a sombra nas chaves próprias.
pub(super) fn aplicar_estilo(o: &mut ObjetoSolto, estilo: &EstiloWordArt) {
    let transparencia = o.aparencia.transparencia;
    o.aparencia = Aparencia {
        preenchimento: Some(estilo.preenchimento.to_owned()),
        contorno: (!estilo.contorno.is_empty()).then(|| estilo.contorno.to_owned()),
        espessura: if estilo.espessura > 0.0 { estilo.espessura } else { 1.0 },
        transparencia,
    };
    o.extras.insert("estilo".into(), Value::String(estilo.chave.to_owned()));
    match estilo.sombra {
        Some(s) => {
            o.extras.insert("sombra".into(), json!({ "cor": s.cor, "dx": s.dx, "dy": s.dy }));
        }
        None => {
            o.extras.remove("sombra");
        }
    }
}

/// Liga a sombra padrão da suíte ou tira a sombra.
pub(super) fn definir_sombra(o: &mut ObjetoSolto, ligada: bool) {
    if ligada {
        let s = comum::SOMBRA_PADRAO;
        o.extras.insert("sombra".into(), json!({ "cor": s.cor, "dx": s.dx, "dy": s.dy }));
    } else {
        o.extras.remove("sombra");
    }
}

/// Os contornos deformados do texto no tamanho de referência (em cache).
pub(super) fn contornos(texto: &str, modo: Transformacao, intensidade: f32) -> Contornos {
    let chave = (texto.to_owned(), modo, intensidade.to_bits());
    if let Some(c) = CACHE.with(|c| c.borrow().get(&chave).cloned()) {
        return c;
    }
    // O contorno da fonte sai do `Renderer` (um só por thread, como no FeatherSlide).
    thread_local! {
        static MEDIDOR: RefCell<pelicano::Renderer> = RefCell::new(pelicano::Renderer::new());
    }
    let fonte = pelicano::Font { weight: 700, ..pelicano::Font::DEFAULT };
    let retos = MEDIDOR.with(|r| feather_ui::texto_contorno::contornos(&mut r.borrow_mut(), texto, CORPO, fonte, Point::ORIGIN));
    let base = feather_ui::texto_contorno::caixa(&retos).unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let deformados: Vec<Vec<[f32; 2]>> = retos
        .into_iter()
        .map(|p| p.into_iter().map(|q| comum::transformar(q, base, modo, intensidade)).collect())
        .collect();
    let caixa = feather_ui::texto_contorno::caixa(&deformados).unwrap_or(base);
    let pronto = Arc::new((deformados, caixa));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= 128 {
            c.clear();
        }
        c.insert(chave, Arc::clone(&pronto));
    });
    pronto
}

/// O caminho do texto escalado (mesma proporção) e centrado na origem dentro de
/// `largura` × `altura` px de tela, com espelhamento.
pub(super) fn caminho(texto: &str, wa: &WordArtTela, largura: f32, altura: f32, espelhar: (bool, bool)) -> Option<Path> {
    let c = contornos(texto, wa.modo, wa.intensidade);
    let (poligonos, [bx, by, bw, bh]) = (&c.0, c.1);
    if poligonos.is_empty() || bw <= 0.0 || bh <= 0.0 {
        return None;
    }
    let escala = ((largura * (1.0 - 2.0 * MARGEM)) / bw).min((altura * (1.0 - 2.0 * MARGEM)) / bh);
    let (cx, cy) = (bx + bw / 2.0, by + bh / 2.0);
    let (sx, sy) = (if espelhar.0 { -escala } else { escala }, if espelhar.1 { -escala } else { escala });
    Some(Path::new(|b| {
        for p in poligonos {
            for (i, &[x, y]) in p.iter().enumerate() {
                let pt = Point::new((x - cx) * sx, (y - cy) * sy);
                if i == 0 { b.move_to(pt) } else { b.line_to(pt) }
            }
            b.close();
        }
    }))
}

/// Desenha o WordArt centrado na origem do `frame` (já transladado e girado pela camada).
#[allow(clippy::too_many_arguments)]
pub(super) fn desenhar(
    f: &mut Frame,
    texto: &str,
    wa: &WordArtTela,
    largura: f32,
    altura: f32,
    espelhar: (bool, bool),
    preenchimento: Option<Color>,
    contorno: Option<Color>,
    espessura: f32,
    z: f32,
) {
    let Some(caminho) = caminho(texto, wa, largura, altura, espelhar) else { return };
    if let Some(s) = wa.sombra {
        f.with_save(|f| {
            f.translate(crate::compat::Vector::new(s.dx * z, s.dy * z));
            f.fill(&caminho, s.cor);
        });
    }
    if let Some(cor) = preenchimento {
        f.fill(&caminho, cor);
    }
    if let Some(cor) = contorno {
        f.stroke(&caminho, Stroke::default().with_color(cor).with_width((espessura * z).max(0.5)).with_line_join(crate::compat::canvas::LineJoin::Round));
    }
}
