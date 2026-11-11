//! GRI-3: `.xlsx` protegido por senha ("Criptografar com senha" do Excel, "Salvar com
//! senha" do LibreOffice). O arquivo deixa de ser ZIP: é um arquivo composto OLE com o
//! `EncryptionInfo` (como derivar a chave) e o `EncryptedPackage` (o ZIP cifrado em AES).
//!
//! As duas criptografias AES do MS-OFFCRYPTO: a "Agile" (versão 4.4, Excel 2010 em diante) e
//! a "Standard" (3.2/4.2, Excel 2007 e o LibreOffice). RC4 e a "Extensible" saem como não
//! suportadas.
//! Tudo em Rust puro (`cfb`, `aes`, `cbc`, `sha1`, `sha2`); a senha não sai da máquina.

use aes::cipher::{block_padding::NoPadding, BlockDecrypt, BlockDecryptMut, KeyInit, KeyIvInit};
use sha2::Digest;
use std::io::{Cursor, Read};

/// O que `ler_xlsx_workbook` devolve quando o arquivo pede senha; a interface pergunta.
pub const PEDE_SENHA: &str = "O arquivo está protegido por senha.";

#[derive(Debug, PartialEq, Eq)]
pub enum ErroSenha {
    SenhaErrada,
    /// Criptografia que não é a Agile (Excel 2007, RC4…).
    NaoSuportado(String),
    /// Arquivo composto sem as partes esperadas, XML ou tamanhos inválidos.
    Corrompido(String),
}

/// Começa como arquivo composto OLE (um `.xlsx` comum começa com `PK`).
pub fn eh_protegido(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

#[derive(Clone, Copy)]
enum Hash { Sha1, Sha256, Sha384, Sha512 }

impl Hash {
    fn de(nome: &str) -> Result<Hash, ErroSenha> {
        match nome {
            "SHA1" | "SHA-1" => Ok(Hash::Sha1),
            "SHA256" | "SHA-256" => Ok(Hash::Sha256),
            "SHA384" | "SHA-384" => Ok(Hash::Sha384),
            "SHA512" | "SHA-512" => Ok(Hash::Sha512),
            outro => Err(ErroSenha::NaoSuportado(format!("hash {outro}"))),
        }
    }

    fn resumo(&self, partes: &[&[u8]]) -> Vec<u8> {
        fn com<D: Digest>(partes: &[&[u8]]) -> Vec<u8> {
            let mut d = D::new();
            for p in partes { d.update(p); }
            d.finalize().to_vec()
        }
        match self {
            Hash::Sha1 => com::<sha1::Sha1>(partes),
            Hash::Sha256 => com::<sha2::Sha256>(partes),
            Hash::Sha384 => com::<sha2::Sha384>(partes),
            Hash::Sha512 => com::<sha2::Sha512>(partes),
        }
    }
}

/// O valor do atributo `nome` na primeira tag `<tag ...>` do XML.
fn atributo<'a>(xml: &'a str, tag: &str, nome: &str) -> Option<&'a str> {
    let ini = xml.find(&format!("<{tag} "))?;
    let fim = ini + xml[ini..].find('>')?;
    let dentro = &xml[ini..fim];
    let chave = format!(" {nome}=\"");
    let a = dentro.find(&chave)? + chave.len();
    let b = a + dentro[a..].find('"')?;
    Some(&dentro[a..b])
}

/// Os parâmetros de um elemento (`keyData` ou `p:encryptedKey`).
struct Parametros {
    sal: Vec<u8>,
    bloco: usize,
    bits: usize,
    hash: Hash,
}

fn parametros(xml: &str, tag: &str) -> Result<Parametros, ErroSenha> {
    let a = |n: &str| atributo(xml, tag, n).ok_or_else(|| ErroSenha::Corrompido(format!("{tag} sem {n}")));
    let numero = |n: &str| a(n)?.parse::<usize>().map_err(|_| ErroSenha::Corrompido(format!("{tag} {n}")));
    let cifra = a("cipherAlgorithm")?;
    let encadeamento = a("cipherChaining")?;
    if cifra != "AES" || encadeamento != "ChainingModeCBC" {
        return Err(ErroSenha::NaoSuportado(format!("{cifra} {encadeamento}")));
    }
    let p = Parametros {
        sal: crate::base64::decodificar(a("saltValue")?).ok_or_else(|| ErroSenha::Corrompido("saltValue".into()))?,
        bloco: numero("blockSize")?,
        bits: numero("keyBits")?,
        hash: Hash::de(a("hashAlgorithm")?)?,
    };
    if p.bloco != 16 || !matches!(p.bits, 128 | 192 | 256) || p.sal.len() < p.bloco {
        return Err(ErroSenha::NaoSuportado(format!("AES de {} bits, bloco {}", p.bits, p.bloco)));
    }
    Ok(p)
}

/// AES-CBC sem preenchimento; `dados` em múltiplos de 16.
fn decifrar_cbc(chave: &[u8], iv: &[u8], dados: &[u8]) -> Result<Vec<u8>, ErroSenha> {
    if dados.len() % 16 != 0 {
        return Err(ErroSenha::Corrompido("bloco cifrado incompleto".into()));
    }
    let mut buf = dados.to_vec();
    let iv = &iv[..16];
    let errado = |_| ErroSenha::Corrompido("chave AES".into());
    let fora = |_| ErroSenha::Corrompido("AES".into());
    match chave.len() {
        16 => cbc::Decryptor::<aes::Aes128>::new_from_slices(chave, iv).map_err(errado)?.decrypt_padded_mut::<NoPadding>(&mut buf).map_err(fora)?,
        24 => cbc::Decryptor::<aes::Aes192>::new_from_slices(chave, iv).map_err(errado)?.decrypt_padded_mut::<NoPadding>(&mut buf).map_err(fora)?,
        _ => cbc::Decryptor::<aes::Aes256>::new_from_slices(chave, iv).map_err(errado)?.decrypt_padded_mut::<NoPadding>(&mut buf).map_err(fora)?,
    };
    Ok(buf)
}

/// Ajusta ao tamanho: corta, ou completa com 0x36 (como manda a especificação).
fn ajustar(mut v: Vec<u8>, n: usize) -> Vec<u8> {
    v.resize(n, 0x36);
    v
}

/// O ZIP do `.xlsx` decifrado com a `senha`.
pub fn decifrar(bytes: &[u8], senha: &str) -> Result<Vec<u8>, ErroSenha> {
    let mut composto = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|_| ErroSenha::Corrompido("não é um arquivo composto".into()))?;
    let mut ler = |nome: &str| -> Result<Vec<u8>, ErroSenha> {
        let mut v = Vec::new();
        composto.open_stream(nome).and_then(|mut f| f.read_to_end(&mut v))
            .map_err(|_| ErroSenha::Corrompido(format!("sem {nome}")))?;
        Ok(v)
    };
    let info = ler("EncryptionInfo")?;
    let pacote = ler("EncryptedPackage")?;
    if info.len() < 8 {
        return Err(ErroSenha::Corrompido("EncryptionInfo curto".into()));
    }
    let (maior, menor) = (u16::from_le_bytes([info[0], info[1]]), u16::from_le_bytes([info[2], info[3]]));
    match (maior, menor) {
        (4, 4) => decifrar_agile(&info, &pacote, senha),
        (2..=4, 2) => decifrar_standard(&info, &pacote, senha),
        _ => Err(ErroSenha::NaoSuportado(format!("criptografia {maior}.{menor}"))),
    }
}

/// O tamanho declarado nos 8 primeiros bytes do `EncryptedPackage` e o resto cifrado.
fn tamanho_e_cifrado(pacote: &[u8]) -> Result<(usize, &[u8]), ErroSenha> {
    if pacote.len() < 8 {
        return Err(ErroSenha::Corrompido("EncryptedPackage curto".into()));
    }
    let tamanho = u64::from_le_bytes(pacote[..8].try_into().expect("8 bytes")) as usize;
    if tamanho > pacote.len() - 8 {
        return Err(ErroSenha::Corrompido("EncryptedPackage menor que o tamanho declarado".into()));
    }
    Ok((tamanho, &pacote[8..]))
}

fn u32_em(v: &[u8], i: usize) -> Result<u32, ErroSenha> {
    v.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().expect("4 bytes")))
        .ok_or_else(|| ErroSenha::Corrompido("EncryptionInfo curto".into()))
}

/// AES-ECB (a Standard cifra o pacote inteiro assim).
fn decifrar_ecb(chave: &[u8], dados: &[u8]) -> Result<Vec<u8>, ErroSenha> {
    fn com<C: BlockDecrypt + KeyInit>(chave: &[u8], buf: &mut [u8]) -> Result<(), ErroSenha> {
        let c = C::new_from_slice(chave).map_err(|_| ErroSenha::Corrompido("chave AES".into()))?;
        for bloco in buf.chunks_exact_mut(16) {
            c.decrypt_block(aes::cipher::generic_array::GenericArray::from_mut_slice(bloco));
        }
        Ok(())
    }
    let mut buf = dados[..dados.len() - dados.len() % 16].to_vec();
    match chave.len() {
        16 => com::<aes::Aes128>(chave, &mut buf)?,
        24 => com::<aes::Aes192>(chave, &mut buf)?,
        _ => com::<aes::Aes256>(chave, &mut buf)?,
    }
    Ok(buf)
}

/// Standard (MS-OFFCRYPTO 2.3.4.5–2.3.4.9): SHA-1 com 50 000 voltas, chave de 128 a 256
/// bits e AES-ECB.
fn decifrar_standard(info: &[u8], pacote: &[u8], senha: &str) -> Result<Vec<u8>, ErroSenha> {
    let tamanho_cab = u32_em(info, 8)? as usize;
    let cab = 12;
    let alg = u32_em(info, cab + 8)?;
    let alg_hash = u32_em(info, cab + 12)?;
    let bits = u32_em(info, cab + 16)? as usize;
    if !matches!(alg, 0x660E | 0x660F | 0x6610) || !matches!(alg_hash, 0 | 0x8004) || !matches!(bits, 128 | 192 | 256) {
        return Err(ErroSenha::NaoSuportado(format!("Standard com algoritmo {alg:#x}, hash {alg_hash:#x}, {bits} bits")));
    }
    let ver = cab + tamanho_cab;
    let tam_sal = u32_em(info, ver)? as usize;
    let fatia = |a: usize, n: usize| info.get(a..a + n).ok_or_else(|| ErroSenha::Corrompido("EncryptionVerifier curto".into()));
    let sal = fatia(ver + 4, tam_sal)?;
    let verificador = fatia(ver + 4 + tam_sal, 16)?;
    let tam_hash = u32_em(info, ver + 20 + tam_sal)? as usize;
    let hash_cifrado = fatia(ver + 24 + tam_sal, 32)?;

    let sha1 = |partes: &[&[u8]]| Hash::Sha1.resumo(partes);
    let utf16: Vec<u8> = senha.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut h = sha1(&[sal, &utf16]);
    for i in 0..50_000u32 {
        h = sha1(&[&i.to_le_bytes(), &h]);
    }
    let h = sha1(&[&h, &0u32.to_le_bytes()]);
    let mut x = Vec::with_capacity(40);
    for preenche in [0x36u8, 0x5c] {
        let mut buf = [preenche; 64];
        for (b, v) in buf.iter_mut().zip(&h) { *b ^= v; }
        x.extend(sha1(&[&buf]));
    }
    let chave = &x[..bits / 8];

    let verificador = decifrar_ecb(chave, verificador)?;
    let hash = decifrar_ecb(chave, hash_cifrado)?;
    if tam_hash > hash.len() || sha1(&[&verificador])[..] != hash[..tam_hash.min(20)] {
        return Err(ErroSenha::SenhaErrada);
    }
    let (tamanho, cifrado) = tamanho_e_cifrado(pacote)?;
    let mut zip = decifrar_ecb(chave, cifrado)?;
    if zip.len() < tamanho {
        return Err(ErroSenha::Corrompido("pacote incompleto".into()));
    }
    zip.truncate(tamanho);
    Ok(zip)
}

/// Agile (MS-OFFCRYPTO 2.3.4.10–2.3.4.15): XML com o hash, as voltas e a chave cifrada;
/// AES-CBC em blocos de 4096 bytes.
fn decifrar_agile(info: &[u8], pacote: &[u8], senha: &str) -> Result<Vec<u8>, ErroSenha> {
    let xml = String::from_utf8_lossy(&info[8..]);
    let dados = parametros(&xml, "keyData")?;
    let chave = parametros(&xml, "p:encryptedKey")?;
    let b64 = |n: &str| atributo(&xml, "p:encryptedKey", n).and_then(crate::base64::decodificar)
        .ok_or_else(|| ErroSenha::Corrompido(n.into()));
    let voltas: u32 = atributo(&xml, "p:encryptedKey", "spinCount").and_then(|s| s.parse().ok())
        .filter(|&n| n <= 10_000_000)
        .ok_or_else(|| ErroSenha::Corrompido("spinCount".into()))?;

    // A senha em UTF-16LE, com o sal, passada `voltas` vezes pelo hash.
    let utf16: Vec<u8> = senha.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut h = chave.hash.resumo(&[&chave.sal, &utf16]);
    for i in 0..voltas {
        h = chave.hash.resumo(&[&i.to_le_bytes(), &h]);
    }
    let derivar = |bloco: [u8; 8]| ajustar(chave.hash.resumo(&[&h, &bloco]), chave.bits / 8);
    let iv = &chave.sal[..chave.bloco];

    // Conferir a senha: o hash da entrada tem de ser o valor guardado.
    let entrada = decifrar_cbc(&derivar([0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79]), iv, &b64("encryptedVerifierHashInput")?)?;
    let valor = decifrar_cbc(&derivar([0xd7, 0xaa, 0x0f, 0x6d, 0x30, 0x61, 0x34, 0x4e]), iv, &b64("encryptedVerifierHashValue")?)?;
    let calculado = chave.hash.resumo(&[&entrada[..chave.sal.len().min(entrada.len())]]);
    if valor.len() < calculado.len() || valor[..calculado.len()] != calculado[..] {
        return Err(ErroSenha::SenhaErrada);
    }
    let segredo = decifrar_cbc(&derivar([0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6]), iv, &b64("encryptedKeyValue")?)?;
    if segredo.len() < dados.bits / 8 {
        return Err(ErroSenha::Corrompido("encryptedKeyValue".into()));
    }
    let segredo = &segredo[..dados.bits / 8];

    // O HMAC do pacote inteiro (`dataIntegrity`), quando há e é SHA-512: pega arquivo adulterado.
    let integridade = |n: &str| atributo(&xml, "dataIntegrity", n).and_then(crate::base64::decodificar);
    if let (Hash::Sha512, Some(ch), Some(va)) = (&dados.hash, integridade("encryptedHmacKey"), integridade("encryptedHmacValue")) {
        let iv = |b: [u8; 8]| ajustar(dados.hash.resumo(&[&dados.sal, &b]), dados.bloco);
        let ch = decifrar_cbc(segredo, &iv(BLOCO_CHAVE_HMAC), &ch)?;
        let va = decifrar_cbc(segredo, &iv(BLOCO_VALOR_HMAC), &va)?;
        if ch.len() < 64 || va.len() < 64 || hmac_sha512(&ch[..64], pacote) != va[..64] {
            return Err(ErroSenha::Corrompido("o arquivo foi alterado depois de cifrado".into()));
        }
    }

    // O pacote: blocos de 4096 bytes, cada um com o IV do seu número.
    let (tamanho, cifrado) = tamanho_e_cifrado(pacote)?;
    let mut zip = Vec::with_capacity(cifrado.len());
    for (i, parte) in cifrado.chunks(4096).enumerate() {
        let iv = ajustar(dados.hash.resumo(&[&dados.sal, &(i as u32).to_le_bytes()]), dados.bloco);
        // O último bloco pode ter sobra depois do múltiplo de 16.
        let parte = &parte[..parte.len() - parte.len() % 16];
        zip.extend(decifrar_cbc(segredo, &iv, parte)?);
        if zip.len() >= tamanho { break; }
    }
    if zip.len() < tamanho {
        return Err(ErroSenha::Corrompido("pacote incompleto".into()));
    }
    zip.truncate(tamanho);
    Ok(zip)
}

/// Os blocos fixos do MS-OFFCRYPTO 2.3.4.14 que derivam o IV do HMAC.
const BLOCO_CHAVE_HMAC: [u8; 8] = [0x5f, 0xb2, 0xad, 0x01, 0x0c, 0xb9, 0xe1, 0xf6];
const BLOCO_VALOR_HMAC: [u8; 8] = [0xa0, 0x67, 0x7f, 0x02, 0xb2, 0x2c, 0x84, 0x33];

/// HMAC-SHA512 (RFC 2104), bloco de 128 bytes.
fn hmac_sha512(chave: &[u8], dados: &[u8]) -> Vec<u8> {
    let mut k = if chave.len() > 128 { Hash::Sha512.resumo(&[chave]) } else { chave.to_vec() };
    k.resize(128, 0);
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    Hash::Sha512.resumo(&[&opad, &Hash::Sha512.resumo(&[&ipad, dados])])
}

/// AES-256-CBC sem preenchimento; completa com zeros até o múltiplo de 16.
fn cifrar_cbc(chave: &[u8], iv: &[u8], dados: &[u8]) -> Vec<u8> {
    use aes::cipher::BlockEncryptMut;
    let mut buf = dados.to_vec();
    buf.resize(dados.len().div_ceil(16) * 16, 0);
    let n = buf.len();
    cbc::Encryptor::<aes::Aes256>::new_from_slices(chave, &iv[..16]).expect("chave de 32 e IV de 16")
        .encrypt_padded_mut::<NoPadding>(&mut buf, n).expect("múltiplo de 16");
    buf
}

/// Cifra o ZIP de um `.xlsx` como o Excel 2010+ (Agile, SHA-512, AES-256, 100 mil voltas,
/// com o HMAC do `dataIntegrity`). Sal, chave e verificador vêm do gerador do sistema.
pub fn cifrar(zip: &[u8], senha: &str) -> Result<Vec<u8>, String> {
    let b64 = crate::base64::codificar;
    let h = |p: &[&[u8]]| Hash::Sha512.resumo(p);
    let mut acaso = [0u8; 16 + 16 + 32 + 16 + 64];
    getrandom::getrandom(&mut acaso).map_err(|e| format!("gerador aleatório: {e}"))?;
    let (sal_dados, resto) = acaso.split_at(16);
    let (sal_chave, resto) = resto.split_at(16);
    let (segredo, resto) = resto.split_at(32);
    let (entrada, chave_hmac) = resto.split_at(16);
    let voltas = 100_000u32;
    let utf16: Vec<u8> = senha.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut hh = h(&[sal_chave, &utf16]);
    for i in 0..voltas { hh = h(&[&i.to_le_bytes(), &hh]); }
    let derivar = |b: [u8; 8]| ajustar(h(&[&hh, &b]), 32);
    let v_entrada = cifrar_cbc(&derivar([0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79]), sal_chave, entrada);
    let v_valor = cifrar_cbc(&derivar([0xd7, 0xaa, 0x0f, 0x6d, 0x30, 0x61, 0x34, 0x4e]), sal_chave, &h(&[entrada]));
    let v_chave = cifrar_cbc(&derivar([0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6]), sal_chave, segredo);

    let mut pacote = (zip.len() as u64).to_le_bytes().to_vec();
    for (i, parte) in zip.chunks(4096).enumerate() {
        pacote.extend(cifrar_cbc(segredo, &h(&[sal_dados, &(i as u32).to_le_bytes()]), parte));
    }
    let hmac_chave = cifrar_cbc(segredo, &h(&[sal_dados, &BLOCO_CHAVE_HMAC]), chave_hmac);
    let hmac_valor = cifrar_cbc(segredo, &h(&[sal_dados, &BLOCO_VALOR_HMAC]), &hmac_sha512(chave_hmac, &pacote));

    let comum = |sal: &[u8]| format!(r#"saltSize="16" blockSize="16" keyBits="256" hashSize="64" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="SHA512" saltValue="{}""#, b64(sal));
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<encryption xmlns="http://schemas.microsoft.com/office/2006/encryption" xmlns:p="http://schemas.microsoft.com/office/2006/keyEncryptor/password"><keyData {}/><dataIntegrity encryptedHmacKey="{}" encryptedHmacValue="{}"/><keyEncryptors><keyEncryptor uri="http://schemas.microsoft.com/office/2006/keyEncryptor/password"><p:encryptedKey spinCount="{voltas}" {} encryptedVerifierHashInput="{}" encryptedVerifierHashValue="{}" encryptedKeyValue="{}"/></keyEncryptor></keyEncryptors></encryption>"#,
        comum(sal_dados), b64(&hmac_chave), b64(&hmac_valor), comum(sal_chave), b64(&v_entrada), b64(&v_valor), b64(&v_chave));
    let mut info = vec![4, 0, 4, 0, 0x40, 0, 0, 0];
    info.extend(xml.as_bytes());

    let falha = |e: std::io::Error| format!("arquivo composto: {e}");
    let mut composto = cfb::CompoundFile::create(Cursor::new(Vec::new())).map_err(falha)?;
    std::io::Write::write_all(&mut composto.create_stream("EncryptionInfo").map_err(falha)?, &info).map_err(falha)?;
    std::io::Write::write_all(&mut composto.create_stream("EncryptedPackage").map_err(falha)?, &pacote).map_err(falha)?;
    composto.flush().map_err(falha)?;
    Ok(composto.into_inner().into_inner())
}

#[cfg(test)]
mod testes {
    use super::*;

    const ARQUIVO: &[u8] = include_bytes!("../tests/arquivos/senha123_libreoffice.xlsx");

    #[test]
    fn senha_certa_abre_o_zip_e_errada_nao() {
        assert!(eh_protegido(ARQUIVO));
        let zip = decifrar(ARQUIVO, "senha123").expect("senha certa");
        assert!(zip.starts_with(b"PK"), "virou o ZIP do .xlsx");
        let (wb, _) = crate::xlsx_read::ler_xlsx_workbook(zip).expect("o .xlsx decifrado abre");
        assert_eq!(wb.valor(0, 2, 0), feathersheet_core::celulas::values::Value::Text("Bia".into()));
        assert_eq!(wb.valor(0, 2, 1), feathersheet_core::celulas::values::Value::Number(32.0));
        assert_eq!(crate::xlsx_read::ler_xlsx_workbook(ARQUIVO.to_vec()).err().as_deref(), Some(PEDE_SENHA));
        assert_eq!(decifrar(ARQUIVO, "senha124"), Err(ErroSenha::SenhaErrada));
        assert_eq!(decifrar(ARQUIVO, ""), Err(ErroSenha::SenhaErrada));
    }

    #[test]
    fn lixo_nao_entra_em_panico() {
        assert!(decifrar(b"PK\x03\x04", "x").is_err());
        assert!(!eh_protegido(b"PK\x03\x04"));
        let mut cortado = ARQUIVO.to_vec();
        cortado.truncate(3000);
        assert!(decifrar(&cortado, "senha123").is_err());
    }

    #[test]
    fn atributo_le_a_tag_certa() {
        let xml = r#"<a><keyData saltValue="AAA=" blockSize="16"/><p:encryptedKey spinCount="100000" saltValue="BBB="/></a>"#;
        assert_eq!(atributo(xml, "keyData", "saltValue"), Some("AAA="));
        assert_eq!(atributo(xml, "p:encryptedKey", "saltValue"), Some("BBB="));
        assert_eq!(atributo(xml, "p:encryptedKey", "blockSize"), None);
    }

    #[test]
    fn agile_do_excel_abre_com_a_senha() {
        let zip = decifrar(ARQUIVO, "senha123").unwrap();
        let cifrado = cifrar(&zip, "Açaí 2026").unwrap();
        assert!(eh_protegido(&cifrado));
        assert_eq!(decifrar(&cifrado, "Açaí 2026").unwrap(), zip, "dois blocos de 4096, o último cortado");
        assert_eq!(decifrar(&cifrado, "acai 2026"), Err(ErroSenha::SenhaErrada));
        assert_ne!(cifrar(&zip, "Açaí 2026").unwrap(), cifrado, "sal novo a cada vez");
        // Um byte trocado no pacote cifrado: o HMAC recusa.
        let mut composto = cfb::CompoundFile::open(Cursor::new(cifrado)).unwrap();
        let mut pacote = Vec::new();
        composto.open_stream("EncryptedPackage").unwrap().read_to_end(&mut pacote).unwrap();
        pacote[100] ^= 1;
        std::io::Write::write_all(&mut composto.create_stream("EncryptedPackage").unwrap(), &pacote).unwrap();
        composto.flush().unwrap();
        let adulterado = composto.into_inner().into_inner();
        assert!(matches!(decifrar(&adulterado, "Açaí 2026"), Err(ErroSenha::Corrompido(_))));
    }
}
