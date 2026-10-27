//! Abrir o `.xlsm` (ou `.xlsb`/`.xlam`, que guardam o VBA do mesmo jeito): o projeto
//! VBA e o nome de cada aba. O código VBA chama a aba pelo *codename* (`Sheet8`,
//! `Planilha1`) — que a pessoa não vê —, e o JavaScript precisa do nome da aba (`"MAIN"`).
//! O codename mora em `<sheetPr codeName="…"/>` no XML de cada aba.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::ovba::{self, ModuloVba};

/// O que o conversor precisa do `.xlsm`.
#[derive(Clone, Debug, Default)]
pub struct ProjetoVba {
    pub modulos: Vec<ModuloVba>,
    /// Codename (como escrito, `Sheet8`) → nome da aba.
    pub abas: HashMap<String, String>,
    /// Os nomes das abas, na ordem da pasta.
    pub nomes_das_abas: Vec<String>,
    /// O desenho de cada formulário (UserForm), com o nome do módulo dele.
    pub formularios: Vec<crate::formulario::FormularioVba>,
}

fn ler_parte(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, nome: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(nome).ok()?;
    let mut v = Vec::new();
    // Parte maior que o teto é tratada como ausente: o tamanho declarado no zip não é confiável.
    f.take(ovba::LIMITE_DE_BYTES as u64 + 1).read_to_end(&mut v).ok()?;
    (v.len() <= ovba::LIMITE_DE_BYTES).then_some(v)
}

/// Os atributos de um elemento como pares (nome local, valor).
fn atributos(e: &quick_xml::events::BytesStart) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .map(|a| {
            let chave = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
            let valor = a.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
            (chave, valor)
        })
        .collect()
}

/// Percorre o XML chamando `f` em cada elemento (nome local, atributos); `f` devolve
/// `false` para parar (o XML da aba pode ter megabytes; o `sheetPr` vem no começo).
fn cada_elemento(xml: &[u8], mut f: impl FnMut(&str, &[(String, String)]) -> bool) {
    let mut r = Reader::from_reader(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let nome = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if !f(&nome, &atributos(&e)) {
                    return;
                }
            }
            Ok(Event::Eof) | Err(_) => return,
            _ => {}
        }
    }
}

fn attr<'a>(atributos: &'a [(String, String)], nome: &str) -> Option<&'a str> {
    atributos.iter().find(|(k, _)| k == nome).map(|(_, v)| v.as_str())
}

/// As abas da pasta: (nome, caminho da parte), na ordem do `workbook.xml`.
fn abas_da_pasta(zip: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Vec<(String, String)> {
    let mut alvo_de = HashMap::new();
    if let Some(rels) = ler_parte(zip, "xl/_rels/workbook.xml.rels") {
        cada_elemento(&rels, |nome, a| {
            if nome == "Relationship" {
                if let (Some(id), Some(alvo)) = (attr(a, "Id"), attr(a, "Target")) {
                    let caminho = alvo.trim_start_matches('/').trim_start_matches("xl/");
                    alvo_de.insert(id.to_string(), format!("xl/{caminho}"));
                }
            }
            true
        });
    }
    let mut abas = Vec::new();
    if let Some(wb) = ler_parte(zip, "xl/workbook.xml") {
        cada_elemento(&wb, |nome, a| {
            if nome == "sheet" {
                if let (Some(n), Some(caminho)) = (attr(a, "name"), attr(a, "id").and_then(|id| alvo_de.get(id))) {
                    abas.push((n.to_string(), caminho.clone()));
                }
            }
            true
        });
    }
    abas
}

/// O codename de uma aba (`<sheetPr codeName>`), se houver.
fn codename(xml: &[u8]) -> Option<String> {
    let mut achado = None;
    cada_elemento(xml, |nome, a| {
        if nome == "sheetPr" {
            achado = attr(a, "codeName").map(String::from);
            return false;
        }
        // O `sheetPr` é o primeiro filho de `worksheet`; passou dele, não há.
        !matches!(nome, "dimension" | "sheetViews" | "sheetData")
    });
    achado
}

/// Lê o projeto VBA de um `.xlsm`. Sem macros, erro em português.
pub fn ler(bytes: &[u8]) -> Result<ProjetoVba, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Este arquivo não é um .xlsm (não é um ZIP).".to_string())?;
    let bin = ler_parte(&mut zip, "xl/vbaProject.bin").ok_or_else(|| "Este arquivo não tem macros VBA (falta xl/vbaProject.bin). Só o .xlsm, o .xlsb e o .xlam guardam macros.".to_string())?;
    let modulos = ovba::ler_projeto(&bin)?;
    let formularios = crate::formulario::ler_do_projeto(&bin, &modulos);
    let mut projeto = ProjetoVba { modulos, formularios, ..ProjetoVba::default() };
    for (nome, caminho) in abas_da_pasta(&mut zip) {
        if let Some(c) = ler_parte(&mut zip, &caminho).and_then(|xml| codename(&xml)) {
            projeto.abas.insert(c, nome.clone());
        }
        projeto.nomes_das_abas.push(nome);
    }
    Ok(projeto)
}
