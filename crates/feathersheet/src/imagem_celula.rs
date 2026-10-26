//! Imagem dentro da célula (16/10/2026): o valor é um data URL
//! (`data:image/png;base64,…`), como a fórmula `IMAGEM` devolve e como o
//! leitor de `.xlsx` guarda a imagem "Colocar na célula" do Excel. A grade
//! desenha a figura no lugar do texto. A decodificação fica guardada pelo
//! texto (a grade redesenha a cada quadro); figuras grandes são reduzidas.
use std::collections::HashMap;
use std::sync::Mutex;

use pelicano::Image;

/// Quantas figuras diferentes ficam decodificadas; acima, recomeça.
const LIMITE: usize = 256;
/// Lado máximo guardado: uma célula nunca mostra mais que isso.
const LADO_MAXIMO: u32 = 512;

type Decodificada = Option<(Image, u32, u32)>;
static CACHE: Mutex<Option<HashMap<u64, Decodificada>>> = Mutex::new(None);

/// A figura de um texto de célula, com largura e altura. `None` quando o
/// texto não é uma imagem (ou não decodifica).
pub fn imagem(texto: &str) -> Decodificada {
    if !texto.starts_with("data:image/") {
        return None;
    }
    let chave = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        texto.hash(&mut h);
        h.finish()
    };
    let mut guarda = CACHE.lock().ok()?;
    let mapa = guarda.get_or_insert_with(HashMap::new);
    if let Some(v) = mapa.get(&chave) {
        return v.clone();
    }
    let v = decodificar(texto);
    if mapa.len() >= LIMITE {
        mapa.clear();
    }
    mapa.insert(chave, v.clone());
    v
}

fn decodificar(texto: &str) -> Decodificada {
    use base64::Engine;
    let (cabeca, corpo) = texto.split_once(',')?;
    if !cabeca.ends_with(";base64") {
        return None;
    }
    let limpo: String = corpo.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(limpo).ok()?;
    let mut img = image::load_from_memory(&bytes).ok()?;
    if img.width() > LADO_MAXIMO || img.height() > LADO_MAXIMO {
        img = img.thumbnail(LADO_MAXIMO, LADO_MAXIMO);
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((Image::from_rgba(w, h, rgba.into_raw())?, w, h))
}

/// Onde a figura cabe na célula (x, y, largura, altura), sem distorcer e
/// com 2 px de margem.
pub fn encaixe(x: f32, y: f32, w: f32, h: f32, iw: u32, ih: u32) -> (f32, f32, f32, f32) {
    let (aw, ah) = ((w - 4.0).max(1.0), (h - 4.0).max(1.0));
    let escala = (aw / iw.max(1) as f32).min(ah / ih.max(1) as f32);
    let (lw, lh) = (iw as f32 * escala, ih as f32 * escala);
    (x + (w - lw) / 2.0, y + (h - lh) / 2.0, lw, lh)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// PNG 1×1 vermelho.
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

    #[test]
    fn decodifica_e_guarda_a_figura() {
        let (_, w, h) = imagem(PNG).expect("png válido");
        assert_eq!((w, h), (1, 1));
        assert!(imagem(PNG).is_some(), "do cache");
        assert!(imagem("texto comum").is_none());
        assert!(imagem("data:image/png;base64,@@@").is_none());
    }

    #[test]
    fn encaixe_mantem_a_proporcao() {
        let (x, y, w, h) = encaixe(0.0, 0.0, 104.0, 24.0, 200, 100);
        assert!((w / h - 2.0).abs() < 1e-4);
        assert!(h <= 20.0 + 1e-4 && x >= 0.0 && y >= 0.0);
    }
}
