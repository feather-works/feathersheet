//! ZIP escrito à mão — tradução do `zip()` do `io/zip.ts`.
//!
//! Só o que o `.xlsx` precisa: cada entrada comprimida com deflate cru (ou
//! guardada como veio, se comprimir não diminuir), nome em UTF-8 e data fixa
//! (1980-01-01), para o mesmo conteúdo dar o mesmo arquivo. Sem ZIP64: uma
//! entrada ou o arquivo inteiro acima de 4 GB é recusado com erro, em vez de
//! sair um ZIP que ninguém abre.
//!
//! Comprime no nível rápido, como o `.fshz` (23/09/2026): no `.xlsx` da
//! planilha de 2 milhões de linhas o nível padrão levava 17 s para 700 MB
//! de XML.
//!
//! Não é a crate `zip` de propósito: o `flate2` já estava aqui (o `.fshz`
//! comprime com ele), o escritor tem sessenta linhas e o `sheet-ponte` que
//! depende deste crate também compila para `.wasm`.

use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};

const SIG_LOCAL: u32 = 0x0403_4b50;
const SIG_CENTRAL: u32 = 0x0201_4b50;
const SIG_EOCD: u32 = 0x0605_4b50;

/// Hora 0 e data 1980-01-01 no formato do DOS.
const HORA: u16 = 0;
const DATA: u16 = 0x0021;

fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn cabe(n: usize, o_que: &str) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| feathersheet_core::trf!("{o_que} passou de 4 GB, o limite do ZIP sem ZIP64"))
}

/// Monta o ZIP com as entradas na ordem dada. Consome as entradas: cada
/// uma é solta assim que foi comprimida, e o comprimido vai direto para o
/// arquivo — no `.xlsx` do `spotify_full` a aba sozinha tem 2,7 GB de XML.
pub fn zipar(entradas: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let quantas = u16::try_from(entradas.len()).map_err(|_| feathersheet_core::tr!("entradas demais para um ZIP"))?;

    for (nome, cru) in entradas {
        let nome = nome.as_bytes();
        let mut crc = Crc::new();
        crc.update(&cru);
        let soma = crc.sum();
        let deslocamento = cabe(out.len(), "o arquivo")?;
        let tam_cru = cabe(cru.len(), "uma parte")?;
        let tam_nome = u16::try_from(nome.len()).map_err(|_| feathersheet_core::tr!("nome de entrada comprido demais"))?;

        // O cabeçalho vai antes do corpo, mas o método e o tamanho
        // comprimido só se sabem depois: ficam em branco e são acertados.
        u32le(&mut out, SIG_LOCAL);
        u16le(&mut out, 20); // versão necessária
        u16le(&mut out, 0x0800); // nome em UTF-8
        let pos_metodo = out.len();
        u16le(&mut out, 0);
        u16le(&mut out, HORA);
        u16le(&mut out, DATA);
        u32le(&mut out, soma);
        let pos_tamanho = out.len();
        u32le(&mut out, 0);
        u32le(&mut out, tam_cru);
        u16le(&mut out, tam_nome);
        u16le(&mut out, 0);
        out.extend_from_slice(nome);

        let inicio = out.len();
        let mut enc = DeflateEncoder::new(&mut out, Compression::fast());
        enc.write_all(&cru).map_err(|e| e.to_string())?;
        enc.finish().map_err(|e| e.to_string())?;
        // Comprimir não diminuiu: guarda como veio.
        let metodo: u16 = if out.len() - inicio < cru.len() {
            8
        } else {
            out.truncate(inicio);
            out.extend_from_slice(&cru);
            0
        };
        drop(cru);
        let tam_corpo = cabe(out.len() - inicio, "uma parte")?;
        out[pos_metodo..pos_metodo + 2].copy_from_slice(&metodo.to_le_bytes());
        out[pos_tamanho..pos_tamanho + 4].copy_from_slice(&tam_corpo.to_le_bytes());

        u32le(&mut central, SIG_CENTRAL);
        u16le(&mut central, 20); // versão de quem gravou
        u16le(&mut central, 20);
        u16le(&mut central, 0x0800);
        u16le(&mut central, metodo);
        u16le(&mut central, HORA);
        u16le(&mut central, DATA);
        u32le(&mut central, soma);
        u32le(&mut central, tam_corpo);
        u32le(&mut central, tam_cru);
        u16le(&mut central, tam_nome);
        u16le(&mut central, 0); // extra
        u16le(&mut central, 0); // comentário
        u16le(&mut central, 0); // disco
        u16le(&mut central, 0); // atributos internos
        u32le(&mut central, 0); // atributos externos
        u32le(&mut central, deslocamento);
        central.extend_from_slice(nome);
    }

    let inicio_central = cabe(out.len(), "o arquivo")?;
    let tam_central = cabe(central.len(), "o diretório do ZIP")?;
    out.extend_from_slice(&central);
    u32le(&mut out, SIG_EOCD);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, quantas);
    u16le(&mut out, quantas);
    u32le(&mut out, tam_central);
    u32le(&mut out, inicio_central);
    u16le(&mut out, 0);
    Ok(out)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Lê de volta o que o [`zipar`] escreveu, pelo diretório central.
    fn ler(zip: &[u8]) -> Vec<(String, Vec<u8>)> {
        use std::io::Read;
        let u16a = |i: usize| u16::from_le_bytes([zip[i], zip[i + 1]]) as usize;
        let u32a = |i: usize| u32::from_le_bytes([zip[i], zip[i + 1], zip[i + 2], zip[i + 3]]) as usize;
        let fim = zip.len() - 22;
        assert_eq!(u32a(fim) as u32, SIG_EOCD);
        let (n, mut p) = (u16a(fim + 10), u32a(fim + 16));
        let mut saida = Vec::new();
        for _ in 0..n {
            assert_eq!(u32a(p) as u32, SIG_CENTRAL);
            let (metodo, tam, tam_nome, local) = (u16a(p + 10), u32a(p + 20), u16a(p + 28), u32a(p + 42));
            let nome = String::from_utf8(zip[p + 46..p + 46 + tam_nome].to_vec()).unwrap();
            let corpo_em = local + 30 + u16a(local + 26) + u16a(local + 28);
            let corpo = &zip[corpo_em..corpo_em + tam];
            let dados = if metodo == 8 {
                let mut v = Vec::new();
                flate2::read::DeflateDecoder::new(corpo).read_to_end(&mut v).unwrap();
                v
            } else {
                corpo.to_vec()
            };
            saida.push((nome, dados));
            p += 46 + tam_nome;
        }
        saida
    }

    #[test]
    fn o_que_entra_volta_igual() {
        let grande = "abc".repeat(1000).into_bytes();
        let entradas = vec![
            ("a.xml".to_string(), grande.clone()),
            ("pasta/ç.txt".to_string(), b"x".to_vec()),
            ("vazio".to_string(), Vec::new()),
        ];
        let zip = zipar(entradas.clone()).unwrap();
        assert_eq!(ler(&zip), entradas);
        // O repetido comprime; o de um byte fica guardado como veio.
        assert!(zip.len() < grande.len());
    }
}
