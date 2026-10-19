//! Comentários das abas do `.xlsx`.
//!
//! - **Clássicos** (anotações): `xl/commentsN.xml` (autor e texto) e, para
//!   saber quais ficam sempre à mostra, o desenho VML da aba
//!   (`<x:ClientData ObjectType="Note">` com `<x:Visible/>`).
//! - **Encadeados** (Excel 365, `xl/threadedComments/…`): lidos quando
//!   existem, como um comentário só — o primeiro com o autor, as respostas
//!   acrescentadas ao texto (`Nome: resposta`). O Excel também grava a
//!   versão clássica deles, com um aviso em inglês no texto; a encadeada
//!   tem prioridade na mesma célula.
use super::{entry_text, formas::caminho_relativo, refs::parse_cell_ref, xml::{parse_xml, XmlNode}};
use feathersheet_core::planilha::sheet::comentarios::{Comentario, Comentarios};
use std::collections::HashMap;
use std::io::Cursor;
use zip::ZipArchive;

fn junta_textos(no: &XmlNode, saida: &mut String) {
    // `rPh` é a leitura fonética (japonês): não faz parte do texto.
    if no.name == "rPh" { return; }
    if no.name == "t" { saida.push_str(&no.text); }
    for c in &no.children { junta_textos(c, saida); }
}

/// Tira o "Autor:\n" que o Excel põe em negrito no começo do texto.
fn sem_prefixo_do_autor(texto: String, autor: &str) -> String {
    if autor.is_empty() { return texto; }
    match texto.strip_prefix(autor).and_then(|t| t.strip_prefix(':')) {
        Some(resto) => resto.strip_prefix("\r\n").or_else(|| resto.strip_prefix('\n')).unwrap_or(resto).to_string(),
        None => texto,
    }
}

/// A cópia clássica de um comentário encadeado traz um aviso em inglês
/// ("[Threaded comment] Your version of Excel…") e o autor `tc={id}`. Se a
/// parte encadeada não puder ser lida, mostra só o corpo (`Comment:` e
/// `Reply:` com recuo de 4 espaços), sem o aviso nem o autor técnico.
fn sem_aviso_de_encadeado(autor: String, texto: String) -> (String, String) {
    let autor = if autor.starts_with("tc=") { String::new() } else { autor };
    if !texto.starts_with("[Threaded comment]") { return (autor, texto); }
    let corpo = texto.split_once("Comment:").map_or(texto.as_str(), |(_, r)| r);
    let limpo = corpo.replace("\r\n", "\n").lines()
        .map(|l| l.strip_prefix("    ").unwrap_or(l))
        .map(|l| if l == "Reply:" { "" } else { l })
        .collect::<Vec<_>>().join("\n");
    (autor, limpo.trim().to_string())
}

/// `(linha, coluna)` das notas sempre à mostra no VML da aba.
fn visiveis_no_vml(vml: &str) -> Vec<(u32, u32)> {
    let numero = |trecho: &str, marca: &str| -> Option<u32> {
        let abre = format!("<x:{marca}>");
        let ini = trecho.find(&abre)? + abre.len();
        let fim = trecho[ini..].find('<')? + ini;
        trecho[ini..fim].trim().parse().ok()
    };
    vml.split("<x:ClientData").skip(1)
        .filter_map(|t| {
            let t = &t[..t.find("</x:ClientData>").unwrap_or(t.len())];
            (t.contains("Note") && t.contains("<x:Visible")).then_some(())?;
            Some((numero(t, "Row")?, numero(t, "Column")?))
        })
        .collect()
}

fn ler_classicos(xml: &XmlNode, saida: &mut Comentarios) {
    let autores: Vec<String> = xml.child("authors").into_iter()
        .flat_map(|a| a.children_named("author")).map(|a| a.text.clone()).collect();
    for cm in xml.child("commentList").into_iter().flat_map(|l| l.children_named("comment")) {
        let Some(celula) = cm.attr("ref").and_then(parse_cell_ref) else { continue };
        if celula.row < 0 || celula.col < 0 { continue; }
        let autor = cm.attr("authorId").and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| autores.get(i)).cloned().unwrap_or_default();
        let mut texto = String::new();
        if let Some(t) = cm.child("text") { junta_textos(t, &mut texto); }
        let texto = sem_prefixo_do_autor(texto, &autor);
        // Encadeado sem a parte nova legível: fica o que vem depois do aviso.
        let (autor, texto) = sem_aviso_de_encadeado(autor, texto);
        saida.insert((celula.row as u32, celula.col as u32), Comentario { autor, texto, visivel: false });
    }
}

/// Os nomes das pessoas dos comentários encadeados (`xl/persons/person.xml`).
fn pessoas(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> HashMap<String, String> {
    let Ok(x) = entry_text(zip, "xl/persons/person.xml").and_then(|t| parse_xml(&t)) else { return HashMap::new() };
    x.children_named("person")
        .filter_map(|p| Some((p.attr("id")?.to_string(), p.attr("displayName").unwrap_or("").to_string())))
        .collect()
}

fn ler_encadeados(xml: &XmlNode, nomes: &HashMap<String, String>, saida: &mut Comentarios) {
    let nome = |t: &XmlNode| t.attr("personId").and_then(|p| nomes.get(p)).cloned().unwrap_or_default();
    let texto = |t: &XmlNode| t.child("text").map(|x| x.text.clone()).unwrap_or_default();
    let todos: Vec<&XmlNode> = xml.children_named("threadedComment").collect();
    for raiz in todos.iter().filter(|t| t.attr("parentId").is_none()) {
        let Some(celula) = raiz.attr("ref").and_then(parse_cell_ref) else { continue };
        if celula.row < 0 || celula.col < 0 { continue; }
        let mut corpo = texto(raiz);
        let id = raiz.attr("id");
        for resposta in todos.iter().filter(|t| t.attr("parentId").is_some() && t.attr("parentId") == id) {
            corpo.push_str(&format!("\n\n{}: {}", nome(resposta), texto(resposta)));
        }
        let chave = (celula.row as u32, celula.col as u32);
        let visivel = saida.get(&chave).is_some_and(|c| c.visivel);
        saida.insert(chave, Comentario { autor: nome(raiz), texto: corpo, visivel });
    }
}

/// Os comentários da aba em `caminho_aba` (`xl/worksheets/sheet1.xml`).
/// Parte ilegível fica de fora, sem impedir a abertura do arquivo.
pub(super) fn ler_comentarios(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, caminho_aba: &str) -> Comentarios {
    let mut saida = Comentarios::new();
    let (dir, arq) = caminho_aba.rsplit_once('/').unwrap_or(("", caminho_aba));
    let Ok(rels) = entry_text(zip, &format!("{dir}/_rels/{arq}.rels")).and_then(|t| parse_xml(&t)) else { return saida };
    let alvos = |sufixo: &str| -> Vec<String> {
        rels.children_named("Relationship")
            .filter(|r| r.attr("Type").is_some_and(|t| t.ends_with(sufixo)))
            .filter_map(|r| r.attr("Target")).map(|t| caminho_relativo(caminho_aba, t)).collect()
    };
    let (classicos, vmls, encadeados) = (alvos("/comments"), alvos("/vmlDrawing"), alvos("/threadedComment"));
    for p in classicos {
        if let Ok(x) = entry_text(zip, &p).and_then(|t| parse_xml(&t)) { ler_classicos(&x, &mut saida); }
    }
    for p in vmls {
        let Ok(vml) = entry_text(zip, &p) else { continue };
        for k in visiveis_no_vml(&vml) {
            if let Some(c) = saida.get_mut(&k) { c.visivel = true; }
        }
    }
    if !encadeados.is_empty() {
        let nomes = pessoas(zip);
        for p in encadeados {
            if let Ok(x) = entry_text(zip, &p).and_then(|t| parse_xml(&t)) { ler_encadeados(&x, &nomes, &mut saida); }
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn classico_tira_o_nome_do_autor_do_texto() {
        let x = parse_xml(r#"<comments xmlns="x"><authors><author>Ana Lima</author></authors><commentList>
            <comment ref="B3" authorId="0"><text><r><rPr><b/></rPr><t xml:space="preserve">Ana Lima:
</t></r><r><t xml:space="preserve">Conferir o total</t></r></text></comment>
            <comment ref="A1" authorId="9"><text><t>Sem autor</t></text></comment>
            </commentList></comments>"#).unwrap();
        let mut m = Comentarios::new();
        ler_classicos(&x, &mut m);
        let c = &m[&(2, 1)];
        assert_eq!((c.autor.as_str(), c.texto.as_str()), ("Ana Lima", "Conferir o total"));
        assert_eq!(m[&(0, 0)].texto, "Sem autor");
    }

    #[test]
    fn copia_classica_do_encadeado_sem_o_aviso() {
        let texto = "[Threaded comment]\n\nYour version of Excel allows you to read this threaded comment; however, any edits to it will get removed if the file is opened in a newer version of Excel. Learn more: https://go.microsoft.com/fwlink/?linkid=870924\n\nComment:\n    Pergunta?\nReply:\n    Resposta.";
        let (autor, t) = sem_aviso_de_encadeado("tc={T1}".into(), texto.into());
        assert_eq!((autor.as_str(), t.as_str()), ("", "Pergunta?\n\nResposta."));
        assert_eq!(sem_aviso_de_encadeado("Ana".into(), "oi".into()), ("Ana".into(), "oi".into()));
    }

    #[test]
    fn vml_diz_quais_ficam_a_mostra() {
        let vml = r#"<xml><v:shape><x:ClientData ObjectType="Note"><x:Row>2</x:Row><x:Column>1</x:Column><x:Visible/></x:ClientData></v:shape>
            <v:shape><x:ClientData ObjectType="Note"><x:Row>0</x:Row><x:Column>0</x:Column></x:ClientData></v:shape></xml>"#;
        assert_eq!(visiveis_no_vml(vml), vec![(2, 1)]);
    }

    #[test]
    fn encadeado_junta_as_respostas() {
        let x = parse_xml(r#"<ThreadedComments xmlns="x">
            <threadedComment ref="C2" personId="{P1}" id="{T1}"><text>Pergunta?</text></threadedComment>
            <threadedComment ref="C2" personId="{P2}" id="{T2}" parentId="{T1}"><text>Resposta.</text></threadedComment>
            </ThreadedComments>"#).unwrap();
        let nomes = HashMap::from([("{P1}".to_string(), "Ana".to_string()), ("{P2}".to_string(), "Bia".to_string())]);
        let mut m = Comentarios::new();
        m.insert((1, 2), Comentario { autor: "x".into(), texto: "[Threaded comment] …".into(), visivel: true });
        ler_encadeados(&x, &nomes, &mut m);
        let c = &m[&(1, 2)];
        assert_eq!((c.autor.as_str(), c.texto.as_str(), c.visivel), ("Ana", "Pergunta?\n\nBia: Resposta.", true));
    }

    /// Arquivo como o Excel 365 grava um comentário encadeado: a versão
    /// clássica com o aviso de compatibilidade em `comments1.xml` e a
    /// encadeada (com resposta) em `threadedComments1.xml` + `persons`.
    /// Uma anotação clássica comum na mesma aba continua sendo lida.
    #[test]
    fn xlsx_do_excel_365_le_encadeado_sem_o_aviso_e_a_anotacao_classica() {
        use feathersheet_core::planilha::workbook::Workbook;
        let mut wb = Workbook::new();
        wb.editar(0, |s| {
            s.definir_comentario(0, 0, Some(Comentario { autor: "Ana".into(), texto: "Nota clássica".into(), visivel: false }));
            s.definir_comentario(1, 2, Some(Comentario { autor: "tc={T1}".into(), visivel: false, texto:
                "[Threaded comment]\n\nYour version of Excel allows you to read this threaded comment; however, any edits to it will get removed if the file is opened in a newer version of Excel. Learn more: https://go.microsoft.com/fwlink/?linkid=870924\n\nComment:\n    Pergunta?".into() }));
        });
        let mut partes = crate::xlsx_write::montar_partes(&wb);
        for (nome, bytes) in partes.iter_mut() {
            if nome == "xl/worksheets/_rels/sheet1.xml.rels" {
                let t = String::from_utf8(bytes.clone()).unwrap().replace("</Relationships>",
                    "<Relationship Id=\"rIdTc\" Type=\"http://schemas.microsoft.com/office/2017/10/relationships/threadedComment\" Target=\"../threadedComments/threadedComment1.xml\"/></Relationships>");
                *bytes = t.into_bytes();
            }
        }
        partes.push(("xl/threadedComments/threadedComment1.xml".into(), r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<ThreadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments">
<threadedComment ref="C2" dT="2026-10-15T10:00:00.00" personId="{P1}" id="{T1}"><text>Pergunta?</text></threadedComment>
<threadedComment ref="C2" dT="2026-10-15T10:05:00.00" personId="{P2}" id="{T2}" parentId="{T1}"><text>Resposta da Bia.</text></threadedComment>
</ThreadedComments>"#.as_bytes().to_vec()));
        partes.push(("xl/persons/person.xml".into(), r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments">
<person displayName="Carla Souza" id="{P1}" userId="carla" providerId="None"/>
<person displayName="Bia" id="{P2}" userId="bia" providerId="None"/>
</personList>"#.as_bytes().to_vec()));
        let bytes = crate::zip_write::zipar(partes).unwrap();
        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let m = &lida.aba(0).unwrap().sheet.comentarios;
        assert_eq!(m.len(), 2);
        assert_eq!((m[&(0, 0)].autor.as_str(), m[&(0, 0)].texto.as_str()), ("Ana", "Nota clássica"));
        let c = &m[&(1, 2)];
        assert_eq!(c.autor, "Carla Souza");
        assert_eq!(c.texto, "Pergunta?\n\nBia: Resposta da Bia.");
        assert!(!c.texto.contains("Threaded comment") && !c.texto.contains("Your version of Excel"));
    }
}
