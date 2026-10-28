//! Mapa com chave de célula (`u64`) e hash barato.
//!
//! O `HashMap` padrão usa SipHash, feito para resistir a chaves escolhidas
//! por um atacante — aqui a chave é a posição da célula, e o custo dele
//! aparecia em cada uma das milhões de gravações de uma abertura de arquivo
//! (13/10/2026: o mapa de estilos de 11 milhões de células).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Guarda a chave e mistura no fim com o finalizador do SplitMix64: a
/// chave é `linha × 16384 + coluna`, e uma multiplicação só (FxHash) deixa
/// os bits baixos iguais — 7× mais lento que o padrão, de tanta colisão.
#[derive(Default, Clone, Copy)]
pub struct HasherChave(u64);

impl Hasher for HasherChave {
    fn finish(&self) -> u64 {
        let mut z = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.rotate_left(8) ^ b as u64;
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = self.0.rotate_left(29) ^ n;
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n as u64);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
}

/// Mapa por número de coluna, com o mesmo hash barato.
pub type MapaColuna<V> = HashMap<u32, V, BuildHasherDefault<HasherChave>>;
pub type MapaChave<V> = HashMap<u64, V, BuildHasherDefault<HasherChave>>;
/// O conjunto com a mesma chave e o mesmo hash barato.
pub type ConjuntoChave = std::collections::HashSet<u64, BuildHasherDefault<HasherChave>>;
