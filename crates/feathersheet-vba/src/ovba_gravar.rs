//! O caminho de volta do [`crate::ovba`]: gravar o projeto VBA. Esta parte é a compressão do MS-OVBA
//! (§2.4.1.3): o casamento mais longo medido sem teto e, no empate, o mais próximo. O Excel só descomprime;
//! a conferência é a ida e volta pelo [`crate::ovba::descomprimir`].

const BLOCO: usize = 4096;

/// Comprime `dados` num contêiner do MS-OVBA (assinatura 0x01 e blocos de até 4 KiB descomprimidos).
pub fn comprimir(dados: &[u8]) -> Result<Vec<u8>, String> {
    let mut saida = vec![0x01];
    for bloco in dados.chunks(BLOCO) {
        let comprimido = comprimir_bloco(bloco);
        if comprimido.len() <= BLOCO + 2 {
            // Cabeçalho: tamanho − 3 nos 12 bits de baixo, assinatura 0b011, bit 15 = comprimido.
            let cabeca = (comprimido.len() as u16 + 2 - 3) | 0x3000 | 0x8000;
            saida.extend_from_slice(&cabeca.to_le_bytes());
            saida.extend_from_slice(&comprimido);
        } else if bloco.len() == BLOCO {
            // Não comprimiu: o bloco vai cru (sempre 4096 bytes, MS-OVBA §2.4.1.1.4).
            saida.extend_from_slice(&(0x3FFFu16).to_le_bytes());
            saida.extend_from_slice(bloco);
        } else {
            // Bloco cru menor que 4 KiB não existe no formato; texto de VBA nunca chega aqui.
            return Err(feathersheet_core::tr!("O código VBA não pôde ser comprimido.").into());
        }
    }
    Ok(saida)
}

/// Os bytes de um bloco comprimido (sem o cabeçalho de 2 bytes): grupos de um byte de sinais e até 8 fichas.
fn comprimir_bloco(bloco: &[u8]) -> Vec<u8> {
    let mut saida = Vec::with_capacity(bloco.len() + bloco.len() / 8 + 1);
    let mut pos = 0;
    while pos < bloco.len() {
        let onde_sinais = saida.len();
        saida.push(0);
        let mut sinais = 0u8;
        for bit in 0..8 {
            if pos >= bloco.len() {
                break;
            }
            let (distancia, comprimento) = casamento(bloco, pos);
            if comprimento >= 3 {
                let bits = bits_da_distancia(pos);
                let ficha = ((distancia - 1) << (16 - bits)) as u16 | (comprimento - 3) as u16;
                saida.extend_from_slice(&ficha.to_le_bytes());
                sinais |= 1 << bit;
                pos += comprimento;
            } else {
                saida.push(bloco[pos]);
                pos += 1;
            }
        }
        saida[onde_sinais] = sinais;
    }
    saida
}

/// Bits da distância numa ficha copiada na posição `pos` do bloco (mín. 4), como no descompressor.
fn bits_da_distancia(pos: usize) -> u32 {
    (usize::BITS - pos.saturating_sub(1).leading_zeros()).max(4)
}

/// O casamento para `pos` como o MS-OVBA §2.4.1.3.19.4: o mais longo medido sem teto (no empate, o mais
/// próximo), e só depois cortado no máximo que a ficha guarda. (distância, comprimento).
fn casamento(bloco: &[u8], pos: usize) -> (usize, usize) {
    let (mut melhor_dist, mut melhor) = (0, 0);
    for candidato in (0..pos).rev() {
        let mut n = 0;
        // Pode passar de `pos` (cópia que se sobrepõe), como no descompressor.
        while pos + n < bloco.len() && bloco[candidato + n] == bloco[pos + n] {
            n += 1;
        }
        if n > melhor {
            melhor = n;
            melhor_dist = pos - candidato;
        }
    }
    let maximo = (0xFFFFusize >> bits_da_distancia(pos)) + 3;
    (melhor_dist, melhor.min(maximo))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ovba::descomprimir;

    /// O exemplo do MS-OVBA §3.2.3 volta ao texto e com o mesmo tamanho. Byte a byte ele difere numa ficha: com três
    /// candidatos de mesmo comprimento, o exemplo usou outro critério de busca; o Excel só descomprime, e qualquer
    /// escolha válida volta igual.
    #[test]
    fn exemplo_da_especificacao_volta_igual_e_do_mesmo_tamanho() {
        let texto = b"#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa";
        let nosso = comprimir(texto).unwrap();
        assert_eq!(descomprimir(&nosso).unwrap(), texto);
        assert_eq!(nosso.len(), 51, "o exemplo da especificação também tem 51 bytes");
    }

    #[test]
    fn ida_e_volta_com_varios_blocos_e_dados_sem_repeticao() {
        let modulo = "Attribute VB_Name = \"Modulo1\"\r\nSub Ola()\r\n    Range(\"A1\").Value = \"Olá, R$ 1.234,50\"\r\nEnd Sub\r\n".repeat(200);
        assert!(modulo.len() > 3 * BLOCO);
        assert_eq!(descomprimir(&comprimir(modulo.as_bytes()).unwrap()).unwrap(), modulo.as_bytes());
        // Sem repetição: o bloco cheio vai cru e ainda volta igual.
        let mut x = 12345u32;
        let ruido: Vec<u8> = (0..BLOCO * 2).map(|_| { x = x.wrapping_mul(1_103_515_245).wrapping_add(12345); (x >> 16) as u8 }).collect();
        assert_eq!(descomprimir(&comprimir(&ruido).unwrap()).unwrap(), ruido);
        assert_eq!(descomprimir(&comprimir(b"").unwrap()).unwrap(), b"");
    }
}
