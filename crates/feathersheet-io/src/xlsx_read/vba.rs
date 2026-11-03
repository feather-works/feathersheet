//! O projeto VBA de um `.xlsm` (`xl/vbaProject.bin`) e os `codeName` da pasta e das abas, lidos
//! para o `.xlsm` sair igual ao salvar (`xlsx_write`). O VBA em si não é interpretado aqui.
use std::io::{Cursor, Read};
use zip::ZipArchive;

type Zip = ZipArchive<Cursor<std::sync::Arc<[u8]>>>;

/// Teto do `vbaProject.bin` (os maiores de verdade têm poucos MB).
const MAX_VBA: u64 = 64 * 1024 * 1024;
/// O `<sheetPr>` é o primeiro filho de `<worksheet>`: basta o começo do XML da aba.
const CABECA_ABA: u64 = 8 * 1024;

pub(super) fn binario(zip: &mut Zip) -> Option<Vec<u8>> {
    let f = zip.by_name("xl/vbaProject.bin").ok()?;
    let mut v = Vec::new();
    f.take(MAX_VBA + 1).read_to_end(&mut v).ok()?;
    (v.len() as u64 <= MAX_VBA && !v.is_empty()).then_some(v)
}

/// O `codeName` do `<sheetPr>` da aba neste caminho do pacote.
pub(super) fn codinome_da_aba(zip: &mut Zip, caminho: &str) -> Option<String> {
    let f = zip.by_name(caminho).ok()?;
    let mut cabeca = Vec::new();
    f.take(CABECA_ABA).read_to_end(&mut cabeca).ok()?;
    codinome_no_xml(&String::from_utf8_lossy(&cabeca))
}

fn codinome_no_xml(xml: &str) -> Option<String> {
    // `<sheetPr` e não `<sheetProtection`, que começa igual.
    let inicio = xml.match_indices("<sheetPr").map(|(i, _)| i)
        .find(|&i| matches!(xml.as_bytes().get(i + 8), Some(b' ' | b'>' | b'/')))?;
    let marca = &xml[inicio..];
    let marca = &marca[..marca.find('>')?];
    let depois = &marca[marca.find("codeName=\"")? + 10..];
    Some(depois[..depois.find('"')?].to_string()).filter(|n| !n.is_empty())
}

#[cfg(test)]
mod testes {
    #[test]
    fn codinome_sai_do_sheetpr_mesmo_com_outros_atributos() {
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="x"><sheetPr filterMode="1" codeName="Planilha3"><tabColor rgb="FFFF0000"/></sheetPr><dimension ref="A1"/>"#;
        assert_eq!(super::codinome_no_xml(xml).as_deref(), Some("Planilha3"));
        assert_eq!(super::codinome_no_xml(r#"<worksheet><sheetPr><tabColor/></sheetPr>"#), None);
        assert_eq!(super::codinome_no_xml(r#"<worksheet><dimension codeName="x"/>"#), None);
        assert_eq!(super::codinome_no_xml(r#"<worksheet><sheetProtection sheet="1"/><sheetPr codeName="Plan1"/>"#).as_deref(), Some("Plan1"));
    }
}
