//! Cálculo iterativo (referência circular com iterações, como no Excel). É do processo
//! inteiro, como o idioma: o app liga pelas Configurações. Desligado, o ciclo dá `#CIRCULAR!`.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static MAXIMO: AtomicU32 = AtomicU32::new(0);
static DIFERENCA: AtomicU64 = AtomicU64::new(0);

/// `Some((máximo de iterações, maior diferença aceita))` liga; `None` desliga.
pub fn definir(cfg: Option<(u32, f64)>) {
    let (max, dif) = cfg.unwrap_or((0, 0.0));
    DIFERENCA.store(dif.abs().to_bits(), Ordering::Relaxed);
    MAXIMO.store(max.min(32_767), Ordering::Relaxed);
}

pub fn config() -> Option<(u32, f64)> {
    match MAXIMO.load(Ordering::Relaxed) {
        0 => None,
        max => Some((max, f64::from_bits(DIFERENCA.load(Ordering::Relaxed)))),
    }
}
