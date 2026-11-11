//! As consultas do Power Query guardadas dentro do `.xlsx` (o "DataMashup", MS-QDEFF).
//!
//! O Excel põe num `customXml/itemN.xml` um `<DataMashup>` com base64. Dentro: versão (u32),
//! o tamanho (u32) e um zip (OPC) com `Formulas/Section1.m`, o documento
//! `section Section1; shared Nome = let … in …;` com todas as consultas. Aqui só sai esse texto;
//! quem lê o M é o FeatherQuery.
use std::io::{Cursor, Read};

/// Teto de cada parte lida (o arquivo pode vir de qualquer lugar).
const LIMITE: u64 = 32 * 1024 * 1024;

fn ler_parte<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, nome: &str) -> Result<Vec<u8>, String> {
    let parte = zip.by_name(nome).map_err(|e| e.to_string())?;
    if parte.size() > LIMITE { return Err(format!("{nome} passa de 32 MiB")); }
    let mut bytes = Vec::new();
    parte.take(LIMITE).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Texto do XML (UTF-8 ou UTF-16, como o Excel grava esse item).
fn texto(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let u: Vec<u16> = bytes[2..].chunks_exact(2)
            .map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&u)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        [0xEF, 0xBB, 0xBF, resto @ ..] => String::from_utf8_lossy(resto).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// O `Section1.m` de dentro do base64 do `<DataMashup>`.
fn secao_do_mashup(base64: &str) -> Result<String, String> {
    let limpo: String = base64.chars().filter(|c| !c.is_whitespace()).collect();
    let bin = crate::base64::decodificar(&limpo).ok_or("DataMashup com base64 inválido")?;
    let tamanho = bin.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or("DataMashup curto demais")?;
    let pacote = bin.get(8..8 + tamanho).ok_or("DataMashup com tamanho errado")?;
    let mut zip = zip::ZipArchive::new(Cursor::new(pacote)).map_err(|e| e.to_string())?;
    let m = ler_parte(&mut zip, "Formulas/Section1.m")?;
    Ok(texto(&m))
}

/// O documento M das consultas do `.xlsx` (`None`: o arquivo não tem Power Query).
/// Lê só as partes `customXml/item*.xml`, não o arquivo inteiro.
pub fn secao_do_xlsx<R: Read + std::io::Seek>(leitor: R) -> Result<Option<String>, String> {
    let mut zip = zip::ZipArchive::new(leitor).map_err(|e| e.to_string())?;
    let itens: Vec<String> = zip.file_names()
        .filter(|n| n.starts_with("customXml/item") && n.ends_with(".xml") && !n.contains("Props"))
        .map(str::to_owned).collect();
    for nome in itens {
        let xml = texto(&ler_parte(&mut zip, &nome)?);
        let Some(ini) = xml.find("<DataMashup") else { continue };
        let Some(abre) = xml[ini..].find('>').map(|k| ini + k + 1) else { continue };
        let Some(fecha) = xml[abre..].find("</DataMashup>").map(|k| abre + k) else { continue };
        return secao_do_mashup(&xml[abre..fecha]).map(Some);
    }
    Ok(None)
}

/// O mesmo, do arquivo no disco.
pub fn secao_do_arquivo(caminho: &std::path::Path) -> Result<Option<String>, String> {
    let arquivo = std::fs::File::open(caminho).map_err(|e| e.to_string())?;
    secao_do_xlsx(std::io::BufReader::new(arquivo))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;

    fn zip_com(partes: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (nome, bytes) in partes {
            w.start_file(*nome, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn le_a_secao_do_datamashup_em_utf16() {
        let m = "section Section1;\r\nshared Vendas = let Fonte = 1 in Fonte;";
        let pacote = zip_com(&[("Formulas/Section1.m", m.as_bytes()), ("[Content_Types].xml", b"<x/>")]);
        let mut bin = 0u32.to_le_bytes().to_vec();
        bin.extend((pacote.len() as u32).to_le_bytes());
        bin.extend(&pacote);
        bin.extend(0u32.to_le_bytes());
        let xml = format!("<?xml version=\"1.0\" encoding=\"utf-16\"?><DataMashup xmlns=\"http://schemas.microsoft.com/DataMashup\">{}</DataMashup>",
            crate::base64::codificar(&bin));
        let mut utf16 = vec![0xFF, 0xFE];
        for u in xml.encode_utf16() { utf16.extend(u.to_le_bytes()); }
        let xlsx = zip_com(&[
            ("xl/workbook.xml", b"<workbook/>"),
            ("customXml/item1.xml", b"<b:Sources xmlns:b=\"x\"/>"),
            ("customXml/itemProps2.xml", b"<p/>"),
            ("customXml/item2.xml", &utf16),
        ]);
        assert_eq!(secao_do_xlsx(Cursor::new(&xlsx)).unwrap().as_deref(), Some(m));
        assert_eq!(secao_do_xlsx(Cursor::new(zip_com(&[("xl/workbook.xml", b"<w/>")]))).unwrap(), None);
        let caminho = std::env::temp_dir().join(format!("pq-{}.xlsx", std::process::id()));
        std::fs::write(&caminho, &xlsx).unwrap();
        assert_eq!(secao_do_arquivo(&caminho).unwrap().as_deref(), Some(m));
        let _ = std::fs::remove_file(caminho);
    }

    #[test]
    fn datamashup_estragado_da_erro_e_nao_panico() {
        assert!(secao_do_mashup("!!!").is_err());
        assert!(secao_do_mashup(&crate::base64::codificar(&[0, 0, 0, 0, 255, 0, 0, 0])).is_err());
    }
}
