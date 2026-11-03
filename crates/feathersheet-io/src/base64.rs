//! Base64 padrão (RFC 4648, com `=`), sem dependência: imagens em data URL e o projeto VBA no `.fshz`.

const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn codificar(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for b in bytes.chunks(3) {
        let n = (u32::from(b[0]) << 16) | (u32::from(*b.get(1).unwrap_or(&0)) << 8) | u32::from(*b.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= b.len() { s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char) } else { s.push('=') }
        }
    }
    s
}

/// `None` se houver caractere fora do alfabeto (espaços e quebras de linha são ignorados).
pub fn decodificar(texto: &str) -> Option<Vec<u8>> {
    let mut saida = Vec::with_capacity(texto.len() / 4 * 3);
    let (mut acumulado, mut bits) = (0u32, 0u32);
    for c in texto.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acumulado = (acumulado << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            saida.push((acumulado >> bits) as u8);
            acumulado &= (1 << bits) - 1;
        }
    }
    Some(saida)
}

#[cfg(test)]
mod testes {
    #[test]
    fn ida_e_volta_em_todos_os_restos() {
        for n in 0..10usize {
            let bytes: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37)).collect();
            assert_eq!(super::decodificar(&super::codificar(&bytes)).unwrap(), bytes);
        }
        assert_eq!(super::codificar(b"Man"), "TWFu");
        assert!(super::decodificar("TW*u").is_none());
    }
}
