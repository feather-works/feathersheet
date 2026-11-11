//! O que um `.xlsx`/`.xlsm` tem e o FeatherSheet ainda não converte (ActiveX, segmentação de
//! dados…), para o F12 mostrar por aba. Só lê a lista do zip e os `.rels`; não monta
//! planilha nenhuma. Minigráficos saíram da lista: são convertidos (`minigraficos.rs`).
use super::{entry_text, sheet, xml};
use std::io::Cursor;
use zip::ZipArchive;

/// Um tipo de item não convertido: a aba (`None` = do arquivo inteiro), o nome em português
/// (a interface traduz) e quantos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaoConvertido {
    pub aba: Option<String>,
    pub o_que: &'static str,
    pub quantos: usize,
}

/// Pelo caminho do alvo de um `.rels` da aba.
fn pelo_alvo(alvo: &str) -> Option<&'static str> {
    let a = alvo.to_ascii_lowercase();
    Some(if a.contains("activex/") { "Controle ActiveX" }
    else if a.contains("slicers/") { "Segmentação de dados" }
    else if a.contains("timelines/") { "Linha do tempo (filtro de datas)" }
    else if a.contains("embeddings/") { "Objeto incorporado (OLE)" }
    else { return None })
}

/// Do arquivo inteiro, pelo nome da parte no zip.
fn pela_parte(nome: &str) -> Option<&'static str> {
    let n = nome.to_ascii_lowercase();
    Some(if n.starts_with("xl/chartsheets/sheet") && n.ends_with(".xml") { "Planilha de gráfico" }
    else if n.starts_with("xl/macrosheets/") && n.ends_with(".xml") { "Folha de macro do Excel 4 (XLM)" }
    else if n.starts_with("xl/dialogsheets/") && n.ends_with(".xml") { "Folha de diálogo do Excel 5" }
    else if n.starts_with("xl/model/") { "Modelo de dados (Power Pivot)" }
    else if n == "xl/connections.xml" { "Conexão de dados externos" }
    else if n.starts_with("xl/querytables/") && n.ends_with(".xml") { "Consulta de tabela (Power Query)" }
    else { return None })
}

pub fn nao_convertidos(bytes: Vec<u8>) -> Result<Vec<NaoConvertido>, String> {
    let mut z = ZipArchive::new(Cursor::new(std::sync::Arc::<[u8]>::from(bytes))).map_err(|e| feathersheet_core::trf!("Não parece um arquivo ZIP: {e}"))?;
    let mut saida: Vec<NaoConvertido> = Vec::new();
    let mut somar = |aba: Option<&str>, o_que: &'static str, n: usize| {
        if n == 0 { return; }
        match saida.iter_mut().find(|x| x.aba.as_deref() == aba && x.o_que == o_que) {
            Some(x) => x.quantos += n,
            None => saida.push(NaoConvertido { aba: aba.map(str::to_owned), o_que, quantos: n }),
        }
    };
    let livro = xml::parse_xml(&entry_text(&mut z, "xl/workbook.xml")?).map_err(|e| e.to_string())?;
    let rels = sheet::read_rels(&entry_text(&mut z, "xl/_rels/workbook.xml.rels")?);
    for no in livro.child("sheets").into_iter().flat_map(|s| s.children_named("sheet")) {
        let (Some(nome), Some(alvo)) = (no.attr("name"), no.attr("r:id").and_then(|id| rels.get(id))) else { continue };
        let caminho = match alvo.strip_prefix('/') { Some(a) => a.to_owned(), None => format!("xl/{}", alvo.trim_start_matches("./")) };
        if !caminho.starts_with("xl/worksheets/") { continue; } // gráfico/macro/diálogo: contados pela parte
        let (pasta, arquivo) = caminho.rsplit_once('/').unwrap_or(("", &caminho));
        for alvo in sheet::read_rels(&entry_text(&mut z, &format!("{pasta}/_rels/{arquivo}.rels")).unwrap_or_default()).values() {
            if let Some(o) = pelo_alvo(alvo) { somar(Some(nome), o, 1); }
        }
    }
    let partes: Vec<String> = z.file_names().map(str::to_owned).collect();
    for p in &partes {
        if let Some(o) = pela_parte(p) { somar(None, o, 1); }
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn acha_activex_e_partes_do_arquivo_e_nao_conta_minigrafico() {
        let mut bytes = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut bytes));
            let mut f = |n: &str, t: &str| { zw.start_file(n, SimpleFileOptions::default()).unwrap(); zw.write_all(t.as_bytes()).unwrap(); };
            f("xl/workbook.xml", r#"<workbook xmlns:r="r"><sheets><sheet name="Vendas" r:id="rId1"/><sheet name="Gráf" r:id="rId2"/></sheets></workbook>"#);
            f("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="chartsheets/sheet1.xml"/></Relationships>"#);
            f("xl/worksheets/sheet1.xml", "<worksheet><extLst><x14:sparkline></x14:sparkline><x14:sparkline></x14:sparkline></extLst></worksheet>");
            f("xl/worksheets/_rels/sheet1.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="../activeX/activeX1.xml"/><Relationship Id="rId2" Target="../activeX/activeX2.xml"/><Relationship Id="rId3" Target="../drawings/drawing1.xml"/></Relationships>"#);
            f("xl/chartsheets/sheet1.xml", "<chartsheet/>");
            f("xl/connections.xml", "<connections/>");
            zw.finish().unwrap();
        }
        let mut r = nao_convertidos(bytes).unwrap();
        r.sort_by_key(|x| x.o_que);
        let v: Vec<_> = r.iter().map(|x| (x.aba.as_deref(), x.o_que, x.quantos)).collect();
        assert_eq!(v, vec![
            (None, "Conexão de dados externos", 1),
            (Some("Vendas"), "Controle ActiveX", 2),
            (None, "Planilha de gráfico", 1),
        ]);
    }
}
