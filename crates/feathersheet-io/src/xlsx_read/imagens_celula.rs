//! Imagens "Colocar na célula" do Excel (16/10/2026). A célula vem com
//! `vm="N"` (metadado de valor) e o valor `#VALUE!`; a figura mora em:
//!
//! - `xl/metadata.xml`: o bloco N de `valueMetadata` aponta (pelo tipo
//!   `XLRICHVALUE`) para um bloco de `futureMetadata`, que traz o índice do
//!   valor rico (`<xlrd:rvb i="…"/>`);
//! - `xl/richData/rdrichvalue.xml` + `rdrichvaluestructure.xml`: o valor rico
//!   da estrutura `_localImage` traz `_rvRel:LocalImageIdentifier`, índice em
//!   `richValueRel.xml`;
//! - `xl/richData/richValueRel.xml` + `_rels/richValueRel.xml.rels`: a imagem
//!   em `xl/media/`.
//!
//! O resultado é o texto `data:image/…;base64,…` que a grade desenha (o mesmo
//! que a fórmula `IMAGEM` devolve).
use super::{entry_text, sheet, xml};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Teto por imagem (protege contra arquivo malicioso).
const MAX_BYTES_IMAGEM: u64 = 20 * 1024 * 1024;

/// `vm` (como vem no atributo, a partir de 1) → data URL da imagem.
pub(super) fn ler(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> HashMap<u32, String> {
    ler_ou_vazio(arquivo).unwrap_or_default()
}

fn ler_ou_vazio(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> Option<HashMap<u32, String>> {
    let meta = xml::parse_xml(&entry_text(arquivo, "xl/metadata.xml").ok()?).ok()?;
    // Posição (a partir de 1) do tipo XLRICHVALUE.
    let tipo_rico = meta.child("metadataTypes")?.children_named("metadataType")
        .position(|t| t.attr("name") == Some("XLRICHVALUE"))? + 1;
    // Bloco de futureMetadata → índice do valor rico.
    let futuros: Vec<Option<usize>> = meta.children_named("futureMetadata")
        .find(|f| f.attr("name") == Some("XLRICHVALUE"))?
        .children_named("bk")
        .map(|bk| achar(bk, "rvb").and_then(|rvb| rvb.attr("i")).and_then(|i| i.parse().ok()))
        .collect();
    // Bloco N de valueMetadata → bloco de futureMetadata.
    let valores: Vec<Option<usize>> = meta.child("valueMetadata")?.children_named("bk")
        .map(|bk| bk.children_named("rc")
            .find(|rc| rc.attr("t").and_then(|t| t.parse::<usize>().ok()) == Some(tipo_rico))
            .and_then(|rc| rc.attr("v")).and_then(|v| v.parse().ok()))
        .collect();

    // Estruturas: em que posição de cada uma está o identificador da imagem.
    let estruturas = xml::parse_xml(&entry_text(arquivo, "xl/richData/rdrichvaluestructure.xml").ok()?).ok()?;
    let posicao_imagem: Vec<Option<usize>> = estruturas.children_named("s")
        .map(|s| s.children_named("k").position(|k| k.attr("n") == Some("_rvRel:LocalImageIdentifier")))
        .collect();
    let ricos = xml::parse_xml(&entry_text(arquivo, "xl/richData/rdrichvalue.xml").ok()?).ok()?;
    // Valor rico → índice em richValueRel.
    let relacao_do_rico: Vec<Option<usize>> = ricos.children_named("rv")
        .map(|rv| {
            let s: usize = rv.attr("s")?.parse().ok()?;
            let pos = (*posicao_imagem.get(s)?)?;
            rv.children_named("v").nth(pos)?.text.trim().parse().ok()
        })
        .collect();
    let relacoes = xml::parse_xml(&entry_text(arquivo, "xl/richData/richValueRel.xml").ok()?).ok()?;
    let ids: Vec<Option<String>> = relacoes.children_named("rel")
        .map(|r| r.attr("r:id").or_else(|| r.attr("id")).map(str::to_owned))
        .collect();
    let rels = sheet::read_rels(&entry_text(arquivo, "xl/richData/_rels/richValueRel.xml.rels").ok()?);

    let mut por_alvo: HashMap<String, Option<String>> = HashMap::new();
    let mut saida = HashMap::new();
    for (k, futuro) in valores.iter().enumerate() {
        let Some(rico) = futuro.and_then(|f| futuros.get(f).copied().flatten()) else { continue };
        let Some(rel) = relacao_do_rico.get(rico).copied().flatten() else { continue };
        let Some(Some(id)) = ids.get(rel) else { continue };
        let Some(alvo) = rels.get(id) else { continue };
        let caminho = caminho_da_midia(alvo);
        let url = por_alvo.entry(caminho.clone()).or_insert_with(|| data_url(arquivo, &caminho)).clone();
        if let Some(url) = url {
            saida.insert(k as u32 + 1, url);
        }
    }
    Some(saida)
}

/// O primeiro descendente com esse nome (o `rvb` fica dentro de `extLst/ext`).
fn achar<'a>(no: &'a xml::XmlNode, nome: &str) -> Option<&'a xml::XmlNode> {
    no.children.iter().find_map(|c| if c.name == nome { Some(c) } else { achar(c, nome) })
}

/// `../media/image1.png` (relativo a `xl/richData/`) → `xl/media/image1.png`.
fn caminho_da_midia(alvo: &str) -> String {
    if let Some(abs) = alvo.strip_prefix('/') {
        return abs.to_owned();
    }
    let mut partes: Vec<&str> = vec!["xl", "richData"];
    for p in alvo.split('/') {
        match p {
            ".." => { partes.pop(); }
            "." | "" => {}
            outro => partes.push(outro),
        }
    }
    partes.join("/")
}

fn data_url(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, caminho: &str) -> Option<String> {
    let mut entrada = arquivo.by_name(caminho).ok()?;
    if entrada.size() > MAX_BYTES_IMAGEM {
        return None;
    }
    let mut bytes = Vec::with_capacity(entrada.size() as usize);
    // O tamanho do cabeçalho do zip é o que o arquivo *diz*: o teto vale para
    // o que sai de fato da descompressão (#53).
    (&mut entrada).take(MAX_BYTES_IMAGEM + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_BYTES_IMAGEM {
        return None;
    }
    let ext = caminho.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    let tipo = match ext.as_str() {
        "png" => "png",
        "jpg" | "jpeg" => "jpeg",
        "gif" => "gif",
        "bmp" => "bmp",
        "webp" => "webp",
        _ => return None,
    };
    Some(format!("data:image/{tipo};base64,{}", super::desenho_objetos::base64(&bytes)))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;


    fn zip_com(partes: &[(&str, Vec<u8>)]) -> ZipArchive<Cursor<std::sync::Arc<[u8]>>> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (nome, conteudo) in partes {
                z.start_file(*nome, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(conteudo).unwrap();
            }
            z.finish().unwrap();
        }
        ZipArchive::new(Cursor::new(buf.into_inner().into())).unwrap()
    }

    #[test]
    fn imagem_colocada_na_celula_vira_data_url() {
        // O leitor não decodifica a figura: qualquer conteúdo serve.
        let png = b"\x89PNG figura".to_vec();
        let esperado = format!("data:image/png;base64,{}", super::super::desenho_objetos::base64(&png));
        let mut z = zip_com(&[
            ("xl/metadata.xml", br#"<metadata xmlns="x" xmlns:xlrd="y"><metadataTypes count="1"><metadataType name="XLRICHVALUE" minSupportedVersion="120000"/></metadataTypes><futureMetadata name="XLRICHVALUE" count="1"><bk><extLst><ext uri="{3e2802c4}"><xlrd:rvb i="0"/></ext></extLst></bk></futureMetadata><valueMetadata count="1"><bk><rc t="1" v="0"/></bk></valueMetadata></metadata>"#.to_vec()),
            ("xl/richData/rdrichvaluestructure.xml", br#"<rvStructures count="1"><s t="_localImage"><k n="_rvRel:LocalImageIdentifier" t="i"/><k n="CalcOrigin" t="i"/></s></rvStructures>"#.to_vec()),
            ("xl/richData/rdrichvalue.xml", br#"<rvData count="1"><rv s="0"><v>0</v><v>5</v></rv></rvData>"#.to_vec()),
            ("xl/richData/richValueRel.xml", br#"<richValueRels xmlns:r="z"><rel r:id="rId1"/></richValueRels>"#.to_vec()),
            ("xl/richData/_rels/richValueRel.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="image" Target="../media/image1.png"/></Relationships>"#.to_vec()),
            ("xl/media/image1.png", png.clone()),
        ]);
        let mapa = ler(&mut z);
        assert_eq!(mapa.get(&1), Some(&esperado));
    }

    #[test]
    fn sem_metadados_nao_ha_imagens() {
        let mut z = zip_com(&[("xl/workbook.xml", b"<workbook/>".to_vec())]);
        assert!(ler(&mut z).is_empty());
    }

    #[test]
    fn caminho_relativo_da_midia() {
        assert_eq!(caminho_da_midia("../media/image1.png"), "xl/media/image1.png");
        assert_eq!(caminho_da_midia("/xl/media/x.jpeg"), "xl/media/x.jpeg");
    }
}
