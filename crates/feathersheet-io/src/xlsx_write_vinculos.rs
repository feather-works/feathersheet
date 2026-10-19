//! Vínculo com outro arquivo no `.xlsx` (`xl/externalLinks/externalLinkN.xml`).
//!
//! O Excel não aceita `[Vendas.fshz]Dados!A1` dentro de `<f>`: lá a fórmula
//! diz `[1]Dados!A1`, e o `1` é a ordem do livro em `<externalReferences>`.
//! Cada livro leva os nomes das abas citadas e os últimos valores lidos,
//! que o Excel mostra enquanto a origem não é aberta.

use std::fmt::Write as _;

use feathersheet_core::celulas::number::to_js_string;
use feathersheet_core::celulas::refs::cell_name;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::sheet::mesmo_nome;
use feathersheet_core::planilha::vinculo::chave_arquivo;
use feathersheet_core::planilha::workbook::Workbook;

use crate::xlsx_write::escape_xml;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Um livro citado: o nome como a primeira fórmula escreveu e as abas.
pub struct Livro {
    pub arquivo: String,
    pub abas: Vec<String>,
}

thread_local! {
    /// As chaves (`chave_arquivo`) dos livros citados, na ordem do `[N]`,
    /// só enquanto o `.xlsx` é montado.
    static ORDEM: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Os livros que as fórmulas citam, em ordem estável.
pub fn livros(pasta: &Workbook) -> Vec<Livro> {
    let mut saida: Vec<Livro> = Vec::new();
    for area in pasta.areas_citadas() {
        let chave = chave_arquivo(&area.arquivo);
        let i = match saida.iter().position(|l| chave_arquivo(&l.arquivo) == chave) {
            Some(i) => i,
            None => {
                saida.push(Livro { arquivo: area.arquivo.clone(), abas: Vec::new() });
                saida.len() - 1
            }
        };
        if !saida[i].abas.iter().any(|a| mesmo_nome(a, &area.aba)) {
            saida[i].abas.push(area.aba);
        }
    }
    saida
}

/// Liga (ou, com `&[]`, desliga) a troca `[arquivo]` → `[N]` nas fórmulas.
pub fn usar_ordem(livros: &[Livro]) {
    ORDEM.with(|o| *o.borrow_mut() = livros.iter().map(|l| chave_arquivo(&l.arquivo)).collect());
}

/// O número (1, 2, …) do livro `arquivo`, se ele está na lista.
pub fn numero(arquivo: &str) -> Option<usize> {
    let chave = chave_arquivo(arquivo);
    ORDEM.with(|o| o.borrow().iter().position(|c| *c == chave).map(|i| i + 1))
}

/// `<externalReferences>` do workbook.xml (vem depois de `<sheets>`).
pub fn referencias_do_livro(livros: &[Livro]) -> String {
    if livros.is_empty() {
        return String::new();
    }
    let itens: String = (1..=livros.len())
        .map(|i| format!("<externalReference r:id=\"rIdExt{i}\"/>"))
        .collect();
    format!("<externalReferences>{itens}</externalReferences>")
}

pub fn relacoes_do_livro(livros: &[Livro]) -> String {
    (1..=livros.len())
        .map(|i| format!("<Relationship Id=\"rIdExt{i}\" Type=\"{NS_REL}/externalLink\" Target=\"externalLinks/externalLink{i}.xml\"/>"))
        .collect()
}

pub fn tipos_de_conteudo(livros: &[Livro]) -> String {
    (1..=livros.len())
        .map(|i| format!("<Override PartName=\"/xl/externalLinks/externalLink{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml\"/>"))
        .collect()
}

/// O endereço do arquivo de origem como o Excel guarda: caminho absoluto
/// vira `file:///…`; nome solto fica relativo à pasta do `.xlsx`.
fn alvo(arquivo: &str, caminho: Option<&str>) -> String {
    let a = caminho.unwrap_or(arquivo).replace('\\', "/");
    if a.starts_with('/') { format!("file://{a}") } else { a }
}

/// As partes `externalLinkN.xml` e as relações de cada uma.
pub fn arquivos(pasta: &Workbook, livros: &[Livro]) -> Vec<(String, Vec<u8>)> {
    let guardados = pasta.externos_para_gravar();
    let mut partes = Vec::new();
    for (i, livro) in livros.iter().enumerate() {
        let n = i + 1;
        let guardado = guardados.iter().find(|g| chave_arquivo(&g.nome) == chave_arquivo(&livro.arquivo));
        let nomes: String = livro.abas.iter().map(|a| format!("<sheetName val=\"{}\"/>", escape_xml(a))).collect();
        let mut dados = String::new();
        for (id, aba) in livro.abas.iter().enumerate() {
            let celulas = guardado
                .and_then(|g| g.abas.iter().find(|(nome, _)| mesmo_nome(nome, aba)))
                .map(|(_, c)| c.as_slice())
                .unwrap_or(&[]);
            write!(dados, "<sheetData sheetId=\"{id}\"").ok();
            if celulas.is_empty() {
                dados.push_str("/>");
                continue;
            }
            dados.push('>');
            // `externos_para_gravar` entrega ordenado por (linha, coluna).
            let mut linha = None;
            for (r, c, v) in celulas {
                let (t, texto) = match v {
                    Value::Empty => continue,
                    Value::Number(x) => ("", to_js_string(*x)),
                    Value::Text(s) => (" t=\"str\"", escape_xml(s)),
                    Value::Bool(b) => (" t=\"b\"", if *b { "1" } else { "0" }.to_string()),
                    Value::Error(e) => (" t=\"e\"", escape_xml(e.code.as_str())),
                };
                if linha != Some(*r) {
                    if linha.is_some() {
                        dados.push_str("</row>");
                    }
                    write!(dados, "<row r=\"{}\">", r + 1).ok();
                    linha = Some(*r);
                }
                write!(dados, "<cell r=\"{}\"{t}><v>{texto}</v></cell>", cell_name(*r, *c)).ok();
            }
            if linha.is_some() {
                dados.push_str("</row>");
            }
            dados.push_str("</sheetData>");
        }
        partes.push((
            format!("xl/externalLinks/externalLink{n}.xml"),
            format!("{XML_DECL}<externalLink xmlns=\"{NS}\" xmlns:r=\"{NS_REL}\"><externalBook r:id=\"rId1\"><sheetNames>{nomes}</sheetNames><sheetDataSet>{dados}</sheetDataSet></externalBook></externalLink>").into_bytes(),
        ));
        let destino = escape_xml(&alvo(&livro.arquivo, guardado.and_then(|g| g.caminho.as_deref())));
        partes.push((
            format!("xl/externalLinks/_rels/externalLink{n}.xml.rels"),
            format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/externalLinkPath\" Target=\"{destino}\" TargetMode=\"External\"/></Relationships>").into_bytes(),
        ));
    }
    partes
}

#[cfg(test)]
mod testes {
    use feathersheet_core::celulas::values::Value;
    use feathersheet_core::planilha::vinculo::aba_de_valores;
    use feathersheet_core::planilha::workbook::Workbook;

    fn parte(bytes: &[u8], nome: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
        let mut texto = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(nome).unwrap(), &mut texto).unwrap();
        texto
    }

    #[test]
    fn vinculo_externo_vira_external_link_e_volta() {
        let mut pasta = Workbook::vazia();
        pasta.criar_aba("Planilha1");
        pasta.escrever_celula(0, 0, 0, "=[Vendas.fshz]Dados!A1+1".into());
        pasta.escrever_celula(0, 1, 0, "=SOMA('/home/x/[Base.fshz]Meu Plano'!B1:B2)".into());
        pasta.instalar_externo_com("Vendas.fshz", vec![aba_de_valores("Dados", &[(0, 0, Value::Number(41.0))])], None, None);
        pasta.instalar_externo_com("/home/x/Base.fshz", vec![aba_de_valores("Meu Plano", &[(0, 1, Value::Number(2.0)), (1, 1, Value::Text("a<b".into()))])], None, None);
        assert_eq!(pasta.valor(0, 0, 0), Value::Number(42.0));

        let bytes = crate::xlsx_write::gerar_xlsx(&pasta).unwrap();
        let aba = parte(&bytes, "xl/worksheets/sheet1.xml");
        assert!(aba.contains("[1]Dados") || aba.contains("[2]Dados"), "{aba}");
        assert!(!aba.contains(".fshz"), "o nome do arquivo não vai dentro do <f>: {aba}");
        assert!(parte(&bytes, "xl/workbook.xml").contains("<externalReferences><externalReference r:id=\"rIdExt1\"/><externalReference r:id=\"rIdExt2\"/></externalReferences>"));
        let livros: String = (1..=2).map(|n| parte(&bytes, &format!("xl/externalLinks/externalLink{n}.xml"))).collect();
        assert!(livros.contains("<sheetName val=\"Meu Plano\"/>") && livros.contains("a&lt;b") && livros.contains("<v>41</v>"), "{livros}");
        let rels: String = (1..=2).map(|n| parte(&bytes, &format!("xl/externalLinks/_rels/externalLink{n}.xml.rels"))).collect();
        assert!(rels.contains("Target=\"file:///home/x/Base.fshz\"") && rels.contains("Target=\"Vendas.fshz\""), "{rels}");

        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let s = &lida.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(0, 0), "=[Vendas.fshz]Dados!A1+1");
        assert!(s.get_raw(1, 0).contains("[Base.fshz]Meu Plano"), "{}", s.get_raw(1, 0));
        assert_eq!(lida.valor(0, 0, 0), Value::Number(42.0));
        assert_eq!(lida.valor(0, 1, 0), Value::Number(2.0));
    }
}
