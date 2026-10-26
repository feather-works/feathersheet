//! O projeto VBA por dentro (`xl/vbaProject.bin`): um arquivo composto OLE com a pasta
//! `VBA/`. O fluxo `VBA/dir` (comprimido) lista os módulos — nome, fluxo e onde, dentro do
//! fluxo, começa o código-fonte comprimido. A compressão é a do MS-OVBA §2.4.1: blocos de
//! 4 KiB, cada um com sinais de "byte literal" ou "cópia de trás" de tamanho variável.

use std::io::{Cursor, Read};

/// Um módulo do projeto, com o código-fonte já descomprimido e decodificado.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuloVba {
    pub nome: String,
    pub tipo: TipoDeModulo,
    pub codigo: String,
}

/// O tipo do módulo: os de documento são "Esta pasta de trabalho" e uma por aba.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeModulo {
    /// Módulo comum (`Módulo1`): as macros que os botões chamam.
    Padrao,
    /// O módulo da pasta de trabalho (`EstaPasta_de_trabalho`, `ThisWorkbook`).
    Pasta,
    /// O módulo de uma aba (`Sheet8`, `Planilha1`): os eventos dela.
    Aba,
    /// Módulo de classe ou formulário (UserForm).
    Classe,
}

/// Teto do que se lê ou descomprime de um fluxo: um projeto VBA de verdade tem poucos MiB,
/// e um arquivo malicioso não pode pedir memória sem fim.
pub(crate) const LIMITE_DE_BYTES: usize = 64 * 1024 * 1024;

/// Descomprime um contêiner comprimido do MS-OVBA (§2.4.1). Erro em português, nunca pânico.
pub fn descomprimir(dados: &[u8]) -> Result<Vec<u8>, String> {
    let erro = || "O código VBA está corrompido (compressão inválida).".to_string();
    if dados.first() != Some(&1) {
        return Err(erro());
    }
    let mut saida = Vec::with_capacity(dados.len() * 3);
    let mut i = 1;
    while i + 2 <= dados.len() {
        let cabeca = u16::from_le_bytes([dados[i], dados[i + 1]]);
        let tamanho = usize::from(cabeca & 0x0fff) + 3;
        let comprimido = cabeca & 0x8000 != 0;
        let fim = (i + tamanho).min(dados.len());
        i += 2;
        let inicio_do_bloco = saida.len();
        if !comprimido {
            let ate = (i + 4096).min(dados.len());
            saida.extend_from_slice(&dados[i..ate]);
            i = ate;
            continue;
        }
        while i < fim {
            let sinais = dados[i];
            i += 1;
            for bit in 0..8 {
                if i >= fim {
                    break;
                }
                if sinais & (1 << bit) == 0 {
                    // Bloco descomprimido tem no máximo 4 KiB (MS-OVBA §2.4.1.1.3).
                    if saida.len() - inicio_do_bloco >= 4096 {
                        return Err(erro());
                    }
                    saida.push(dados[i]);
                    i += 1;
                    continue;
                }
                if i + 2 > fim {
                    return Err(erro());
                }
                let ficha = u16::from_le_bytes([dados[i], dados[i + 1]]);
                i += 2;
                let posicao = saida.len() - inicio_do_bloco;
                // Quantos bits a distância usa depende de quanto do bloco já saiu (mín. 4).
                let bits_da_distancia = (usize::BITS - posicao.saturating_sub(1).leading_zeros()).max(4);
                let mascara = 0xffffu16 >> bits_da_distancia;
                let distancia = usize::from(ficha >> (16 - bits_da_distancia)) + 1;
                let comprimento = usize::from(ficha & mascara) + 3;
                if distancia > posicao || posicao + comprimento > 4096 {
                    return Err(erro());
                }
                for _ in 0..comprimento {
                    let b = saida[saida.len() - distancia];
                    saida.push(b);
                }
            }
        }
        if saida.len() > LIMITE_DE_BYTES {
            return Err(erro());
        }
    }
    Ok(saida)
}

/// Bytes na página de código do projeto → texto. 1252 (o padrão do Excel em pt-BR e en-US)
/// difere do Latin-1 só em 0x80–0x9F; 65001 é UTF-8.
pub fn decodificar(bytes: &[u8], pagina: u16) -> String {
    if pagina == 65001 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    const DE_0X80: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    bytes.iter().map(|&b| if (0x80..0xa0).contains(&b) { DE_0X80[usize::from(b - 0x80)] } else { char::from(b) }).collect()
}

/// O que o fluxo `dir` diz de um módulo.
#[derive(Default)]
struct Entrada {
    nome: String,
    fluxo: String,
    deslocamento: usize,
    padrao: bool,
}

/// Lê o fluxo `dir` já descomprimido: a página de código e os módulos.
fn ler_dir(dir: &[u8]) -> Result<(u16, Vec<Entrada>), String> {
    let mut pagina = 1252;
    let mut modulos = Vec::new();
    let mut atual = Entrada::default();
    let mut i = 0;
    while i + 6 <= dir.len() {
        let id = u16::from_le_bytes([dir[i], dir[i + 1]]);
        let mut tamanho = u32::from_le_bytes([dir[i + 2], dir[i + 3], dir[i + 4], dir[i + 5]]) as usize;
        i += 6;
        // PROJECTVERSION diz 4 no tamanho, mas tem 6 bytes (MS-OVBA §2.3.4.2.1.11).
        if id == 0x0009 {
            tamanho = 6;
        }
        let Some(dado) = dir.get(i..i + tamanho) else { return Err("O índice do projeto VBA está corrompido.".into()) };
        i += tamanho;
        match id {
            0x0003 if tamanho >= 2 => pagina = u16::from_le_bytes([dado[0], dado[1]]),
            0x0019 => atual = Entrada { nome: decodificar(dado, pagina), ..Entrada::default() },
            0x001A => atual.fluxo = decodificar(dado, pagina),
            0x0031 if tamanho >= 4 => atual.deslocamento = u32::from_le_bytes([dado[0], dado[1], dado[2], dado[3]]) as usize,
            0x0021 => atual.padrao = true,
            0x002B => modulos.push(std::mem::take(&mut atual)),
            _ => {}
        }
    }
    Ok((pagina, modulos))
}

/// O tipo de um módulo que não é padrão, pelo `Attribute VB_Base` do código.
fn tipo_pelo_codigo(codigo: &str) -> TipoDeModulo {
    let base = codigo.lines().find(|l| l.starts_with("Attribute VB_Base")).unwrap_or("");
    if base.contains("00020819") {
        TipoDeModulo::Pasta
    } else if base.contains("00020820") {
        TipoDeModulo::Aba
    } else {
        TipoDeModulo::Classe
    }
}

/// Lê os módulos de um `vbaProject.bin`.
pub fn ler_projeto(bin: &[u8]) -> Result<Vec<ModuloVba>, String> {
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bin)).map_err(|_| "O projeto VBA (vbaProject.bin) não é um arquivo OLE válido.".to_string())?;
    let mut fluxo = |nome: &str| -> Result<Vec<u8>, String> {
        let mut f = cfb.open_stream(nome).map_err(|_| format!("O projeto VBA não tem o fluxo {nome}."))?;
        let mut v = Vec::new();
        f.take(LIMITE_DE_BYTES as u64 + 1).read_to_end(&mut v).map_err(|e| format!("Não consegui ler {nome}: {e}"))?;
        if v.len() > LIMITE_DE_BYTES {
            return Err(format!("O fluxo {nome} do projeto VBA é grande demais."));
        }
        Ok(v)
    };
    let (pagina, entradas) = ler_dir(&descomprimir(&fluxo("/VBA/dir")?)?)?;
    let mut modulos = Vec::new();
    for e in entradas {
        let bruto = fluxo(&format!("/VBA/{}", e.fluxo))?;
        let comprimido = bruto.get(e.deslocamento..).ok_or_else(|| format!("O módulo {} está corrompido.", e.nome))?;
        let codigo = decodificar(&descomprimir(comprimido)?, pagina).replace("\r\n", "\n").replace('\r', "\n");
        let tipo = if e.padrao { TipoDeModulo::Padrao } else { tipo_pelo_codigo(&codigo) };
        modulos.push(ModuloVba { nome: e.nome, tipo, codigo });
    }
    Ok(modulos)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn descomprime_o_exemplo_da_especificacao() {
        // MS-OVBA §3.2.3: "#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa".
        let comprimido = [
            0x01, 0x2F, 0xB0, 0x00, 0x23, 0x61, 0x61, 0x61, 0x62, 0x63, 0x64, 0x65, 0x82, 0x66, 0x00, 0x70, 0x61, 0x67, 0x68, 0x69, 0x6A, 0x01, 0x38, 0x08, 0x61, 0x6B, 0x6C, 0x00, 0x30, 0x6D, 0x6E, 0x6F, 0x70, 0x06,
            0x71, 0x02, 0x70, 0x04, 0x10, 0x72, 0x73, 0x74, 0x75, 0x76, 0x10, 0x77, 0x78, 0x79, 0x7A, 0x00, 0x3C,
        ];
        let texto = String::from_utf8(descomprimir(&comprimido).unwrap()).unwrap();
        assert_eq!(texto, "#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa");
    }

    #[test]
    fn lixo_da_erro_nao_panico() {
        assert!(descomprimir(&[]).is_err());
        assert!(descomprimir(&[2, 0, 0]).is_err());
        let _ = descomprimir(&[1, 0xff, 0xbf, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn bloco_que_passa_de_4_kib_da_erro() {
        // Um literal e uma cópia de 4098 bytes: o bloco sairia com 4099. Sem o teto, blocos
        // assim cresciam sem limite e a distância passava de 16 bits (pânico).
        let c = [0x01, 0xff, 0xbf, 0xfe, 0x61, 0xff, 0x0f];
        assert!(descomprimir(&c).is_err());
    }

    #[test]
    fn pagina_1252_tem_acento_e_aspas_curvas() {
        assert_eq!(decodificar(&[0x42, 0x6F, 0x74, 0xE3, 0x6F, 0x93, 0x94], 1252), "Botão“”");
    }
}
