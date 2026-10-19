//! Vínculo com outro arquivo lido do `.xlsx` (`xl/externalLinks/`): a
//! fórmula chega como `[1]Dados!A1` e volta a citar o arquivo pelo nome, com
//! os últimos valores que o arquivo guardou. Par do `xlsx_write_vinculos`.

use std::io::Cursor;

use feathersheet_core::celulas::refs::parse_cell_ref;
use feathersheet_core::celulas::values::{ErrorCode, Value};
use feathersheet_core::planilha::workbook::vinculos::LivroGuardado;
use feathersheet_core::planilha::workbook::Workbook;
use zip::ZipArchive;

use super::{entry_text, sheet, xml};

/// `file:///home/x/Vendas.fshz` → (`Vendas.fshz`, `/home/x/Vendas.fshz`).
fn nome_e_caminho(alvo: &str) -> (String, Option<String>) {
    // shortcut: só o espaço (%20) é decodificado; outro escape fica como veio.
    let alvo = alvo.replace("%20", " ");
    let sem_esquema = alvo.strip_prefix("file:///").map(|r| if r.starts_with('/') || r.contains(':') { r.to_string() } else { format!("/{r}") });
    let nome = alvo.rsplit(['/', '\\']).next().unwrap_or(&alvo).to_string();
    (nome, sem_esquema)
}

fn valor(celula: &xml::XmlNode) -> Value {
    let texto = celula.child("v").map(|v| v.text.as_str()).unwrap_or("");
    match celula.attr("t") {
        Some("str") | Some("s") | Some("inlineStr") => Value::Text(texto.to_string()),
        Some("b") => Value::Bool(texto == "1"),
        Some("e") => ErrorCode::from_code(texto).map_or(Value::Empty, Value::erro),
        _ => texto.parse().map_or(Value::Empty, Value::Number),
    }
}

/// Os livros externos na ordem do `[N]`, com nome, caminho e valores guardados.
pub(super) fn ler(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, book: &xml::XmlNode, rels: &std::collections::HashMap<String, String>) -> Vec<LivroGuardado> {
    let Some(refs) = book.child("externalReferences") else { return Vec::new() };
    let mut livros = Vec::new();
    for r in refs.children_named("externalReference") {
        let Some(alvo) = r.attr("r:id").and_then(|id| rels.get(id)) else { continue };
        let caminho = format!("xl/{}", alvo.trim_start_matches('/').trim_start_matches("xl/"));
        let Ok(raiz) = xml::parse_xml(&entry_text(arquivo, &caminho).unwrap_or_default()) else { continue };
        let Some(externo) = raiz.child("externalBook") else { continue };
        let (pasta, base) = caminho.rsplit_once('/').unwrap_or(("", &caminho));
        let rels_livro = sheet::read_rels(&entry_text(arquivo, &format!("{pasta}/_rels/{base}.rels")).unwrap_or_default());
        let Some(origem) = externo.attr("r:id").and_then(|id| rels_livro.get(id)) else { continue };
        let (nome, caminho_origem) = nome_e_caminho(origem);
        let nomes: Vec<String> = externo.child("sheetNames").map(|n| n.children_named("sheetName").filter_map(|s| s.attr("val").map(str::to_owned)).collect()).unwrap_or_default();
        let mut abas: Vec<_> = nomes.iter().map(|n| (n.clone(), Vec::new())).collect();
        if let Some(dados) = externo.child("sheetDataSet") {
            for folha in dados.children_named("sheetData") {
                let Some(aba) = folha.attr("sheetId").and_then(|i| i.parse::<usize>().ok()).and_then(|i| abas.get_mut(i)) else { continue };
                for linha in folha.children_named("row") {
                    for celula in linha.children_named("cell") {
                        let Some((r, c)) = celula.attr("r").and_then(parse_cell_ref) else { continue };
                        let v = valor(celula);
                        if !matches!(v, Value::Empty) { aba.1.push((r, c, v)); }
                    }
                }
            }
        }
        livros.push(LivroGuardado { nome, caminho: caminho_origem, atualizado_em: None, abas });
    }
    livros
}

/// Troca `[N]` pelo nome do arquivo nas fórmulas e instala os valores.
pub(super) fn instalar(workbook: &mut Workbook, livros: Vec<LivroGuardado>) {
    // De trás para a frente: se o livro 1 se chama "2", na ordem direta a
    // troca seguinte (`2` → livro 2) levaria as fórmulas dele junto.
    for (i, livro) in livros.iter().enumerate().rev() {
        workbook.trocar_origem_externa(&(i + 1).to_string(), &livro.nome);
    }
    workbook.instalar_externos_guardados(livros);
}
