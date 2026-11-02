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

/// Põe os livros na ordem de `[N]` e dá nome próprio aos que repetem o nome de outro: dois
/// vínculos com o mesmo nome de arquivo em pastas diferentes são arquivos diferentes (#136), e o
/// segundo passa a ser chamado pelo caminho. Devolve `N` → nome, para a leitura das abas trocar
/// `[N]` já no texto da fórmula.
pub(super) fn nomes_finais(livros: &mut [(usize, LivroGuardado)]) -> Vec<(usize, String)> {
    use feathersheet_core::planilha::vinculo::chave_arquivo;
    livros.sort_by_key(|(n, _)| *n);
    let mut vistos: Vec<String> = Vec::new();
    for (_, livro) in livros.iter_mut() {
        if vistos.contains(&chave_arquivo(&livro.nome)) {
            if let Some(c) = livro.caminho.clone().filter(|c| !vistos.contains(&chave_arquivo(c))) {
                livro.nome = c;
            }
        }
        vistos.push(chave_arquivo(&livro.nome));
    }
    livros.iter().map(|(n, l)| (*n, l.nome.clone())).collect()
}

/// `[1]Dados!A1` e `'[1]Minha Aba'!A1` → a aba do arquivo pelo nome (`[Vendas.xlsx]Dados!A1`).
/// Roda no texto da fórmula ao ler a aba: reescrever depois, fórmula por fórmula, levava ~30 s
/// numa aba com 780 mil fórmulas de vínculo (#136). Texto entre aspas duplas não é tocado;
/// `[N]` sem livro conhecido fica como veio.
pub(super) fn trocar_numeros<'a>(f: &'a str, livros: &[(usize, String)]) -> std::borrow::Cow<'a, str> {
    use feathersheet_core::celulas::refs::quote_sheet;
    use feathersheet_core::planilha::vinculo::nome_de_aba;
    if livros.is_empty() || !f.contains('[') { return f.into(); }
    let b = f.as_bytes();
    let mut saida = String::with_capacity(f.len() + 32);
    let (mut i, mut copiado, mut em_texto) = (0, 0, false);
    while i < b.len() {
        match b[i] {
            b'"' => em_texto = !em_texto,
            b'[' if !em_texto => {
                let digitos = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                let fecha = i + 1 + digitos;
                let livro = (digitos > 0 && b.get(fecha) == Some(&b']'))
                    .then(|| f[i + 1..fecha].parse::<usize>().ok()).flatten()
                    .and_then(|n| livros.iter().find(|(k, _)| *k == n)).map(|(_, nome)| nome);
                if let Some(nome) = livro {
                    let citada = i > 0 && b[i - 1] == b'\'';
                    // Fim do nome da aba: a aspa de fecho (`''` é aspa dentro) ou o `!`.
                    let mut j = fecha + 1;
                    let mut aba = String::new();
                    let mut achou = false;
                    while j < b.len() {
                        if citada && b[j] == b'\'' {
                            if b.get(j + 1) == Some(&b'\'') { aba.push('\''); j += 2; continue; }
                            achou = b.get(j + 1) == Some(&b'!');
                            break;
                        }
                        if !citada && b[j] == b'!' { achou = true; break; }
                        if !citada && matches!(b[j], b'(' | b')' | b';' | b',' | b' ' | b'"' | b'[') { break; }
                        let c = f[j..].chars().next().expect("char");
                        aba.push(c);
                        j += c.len_utf8();
                    }
                    if achou && !aba.is_empty() {
                        let inicio = if citada { i - 1 } else { i };
                        saida.push_str(&f[copiado..inicio]);
                        saida.push_str(&quote_sheet(&nome_de_aba(nome, &aba)));
                        // `j` está na aspa de fecho (citada) ou no `!`.
                        copiado = if citada { j + 1 } else { j };
                        i = copiado;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    if copiado == 0 { return f.into(); }
    saida.push_str(&f[copiado..]);
    saida.into()
}

/// Instala os valores guardados dos livros (o `[N]` já foi trocado na leitura das abas).
pub(super) fn instalar(workbook: &mut Workbook, livros: Vec<(usize, LivroGuardado)>) {
    if livros.is_empty() { return; }
    workbook.instalar_externos_na_leitura(juntar_mesmo_arquivo(livros.into_iter().map(|(_, l)| l).collect()));
}

#[cfg(test)]
mod testes_trocar {
    use super::trocar_numeros;
    #[test]
    fn troca_o_numero_pelo_arquivo() {
        let l = vec![(1, "LOFA.xlsx".to_string()), (2, "/H/Pasta X/Base.xlsx".to_string())];
        assert_eq!(trocar_numeros("SUM([1]Dados!A1:B2)", &l), "SUM([LOFA.xlsx]Dados!A1:B2)");
        assert_eq!(trocar_numeros("'[1]Minha Aba'!$A$1+[1]X!B2", &l), "'[LOFA.xlsx]Minha Aba'!$A$1+[LOFA.xlsx]X!B2");
        assert_eq!(trocar_numeros("'[2]D''a'!A1", &l), "'/H/Pasta X/[Base.xlsx]D''a'!A1");
        // Texto, livro desconhecido, nome definido externo e referência estruturada ficam.
        assert_eq!(trocar_numeros("\"[1]X!\"&[3]Y!A1&[1]!Nome&T[Col]", &l), "\"[1]X!\"&[3]Y!A1&[1]!Nome&T[Col]");
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
