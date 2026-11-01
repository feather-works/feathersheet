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
    let sem_esquema = alvo.strip_prefix("file:///").map(|r| if r.starts_with('/') || r.contains(':') { r.to_string() } else { format!("/{r}") })
        // O Excel também grava o caminho sem esquema (`/PASTA/arquivo.xlsx`, a partir da raiz do disco).
        .or_else(|| alvo.starts_with('/').then(|| alvo.clone()));
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

/// Os livros externos com o `N` de `[N]` (a posição em `<externalReferences>`,
/// contando os vínculos DDE/OLE e os ilegíveis que ficam de fora), nome,
/// caminho e valores guardados.
pub(super) fn ler(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, book: &xml::XmlNode, rels: &std::collections::HashMap<String, String>) -> Vec<(usize, LivroGuardado)> {
    let Some(refs) = book.child("externalReferences") else { return Vec::new() };
    let mut livros = Vec::new();
    for (i, r) in refs.children_named("externalReference").enumerate() {
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
        livros.push((i + 1, LivroGuardado { nome, caminho: caminho_origem, atualizado_em: None, abas }));
    }
    livros
}

/// Dois vínculos para pastas de mesmo nome (`../LOFA.xlsx` e `H:\\…\\LOFA.xlsx`) viram um arquivo
/// só depois da troca de `[N]`: instalados um depois do outro, o segundo (às vezes vazio, com
/// `refreshError`) apagava os valores do primeiro (#136). Juntos, cada aba fica com a versão que
/// tem mais células.
fn juntar_mesmo_arquivo(livros: Vec<LivroGuardado>) -> Vec<LivroGuardado> {
    use feathersheet_core::planilha::vinculo::chave_arquivo;
    let mut juntos: Vec<LivroGuardado> = Vec::new();
    for livro in livros {
        let Some(dono) = juntos.iter_mut().find(|j| chave_arquivo(&j.nome) == chave_arquivo(&livro.nome)) else {
            juntos.push(livro);
            continue;
        };
        for (aba, celulas) in livro.abas {
            match dono.abas.iter_mut().find(|(a, _)| a.eq_ignore_ascii_case(&aba)) {
                Some((_, ja)) if ja.len() >= celulas.len() => {}
                Some((_, ja)) => *ja = celulas,
                None => dono.abas.push((aba, celulas)),
            }
        }
    }
    juntos
}

/// Troca `[N]` pelo nome do arquivo nas fórmulas e instala os valores.
pub(super) fn instalar(workbook: &mut Workbook, livros: Vec<(usize, LivroGuardado)>) {
    if livros.is_empty() { return; }
    // A troca é parte da leitura: o Ctrl+Z logo depois de abrir não pode
    // devolver `[1]Dados!B2`, que aponta para um arquivo chamado "1".
    let antes: Vec<usize> = (0..workbook.len()).map(|i| workbook.aba_mut(i).map_or(0, |a| a.sheet.profundidade_do_historico())).collect();
    // De trás para a frente: se o livro 1 se chama "2", na ordem direta a
    // troca seguinte (`2` → livro 2) levaria as fórmulas dele junto.
    let mut livros = livros;
    livros.sort_by_key(|(n, _)| *n);
    // Dois vínculos com o mesmo nome de arquivo em pastas diferentes são arquivos diferentes
    // (#136): o segundo passa a ser chamado pelo caminho, para não se misturar com o primeiro.
    {
        use feathersheet_core::planilha::vinculo::chave_arquivo;
        let mut vistos: Vec<String> = Vec::new();
        for (_, livro) in livros.iter_mut() {
            if vistos.contains(&chave_arquivo(&livro.nome)) {
                if let Some(c) = livro.caminho.clone().filter(|c| !vistos.contains(&chave_arquivo(c))) {
                    livro.nome = c;
                }
            }
            vistos.push(chave_arquivo(&livro.nome));
        }
    }
    // Uma cascata de "sujo" por aba para a troca e para os valores instalados: eram três
    // passadas por milhões de fórmulas (#136).
    for i in 0..workbook.len() {
        if let Some(aba) = workbook.aba_mut(i) { aba.sheet.adiar_cascata(); }
    }
    for (n, livro) in livros.iter().rev() {
        workbook.trocar_origem_externa_na_leitura(&n.to_string(), &livro.nome);
    }
    for (i, profundidade) in antes.into_iter().enumerate() {
        if let Some(aba) = workbook.aba_mut(i) { aba.sheet.truncar_historico(profundidade); }
    }
    workbook.instalar_externos_guardados(juntar_mesmo_arquivo(livros.into_iter().map(|(_, l)| l).collect()));
    for i in 0..workbook.len() {
        if let Some(aba) = workbook.aba_mut(i) { aba.sheet.soltar_cascata(); }
    }
}

#[cfg(test)]
mod testes_juntar {
    use super::*;

    #[test]
    fn mesmo_arquivo_fica_com_a_aba_mais_cheia() {
        let livro = |abas: Vec<(&str, usize)>| LivroGuardado {
            nome: "LOFA.xlsx".into(), caminho: None, atualizado_em: None,
            abas: abas.into_iter().map(|(a, n)| (a.to_string(), (0..n as u32).map(|r| (r, 0, Value::Number(1.0))).collect())).collect(),
        };
        let juntos = juntar_mesmo_arquivo(vec![livro(vec![("Sorteios", 50), ("FILTROS", 9)]), livro(vec![("FILTROS", 3), ("Outra", 2)])]);
        assert_eq!(juntos.len(), 1);
        let n = |a: &str| juntos[0].abas.iter().find(|(x, _)| x == a).map(|(_, c)| c.len());
        assert_eq!((n("Sorteios"), n("FILTROS"), n("Outra")), (Some(50), Some(9), Some(2)));
    }
}
