//! Posições do erro da macro na numeração do editor (14/10/2026).
//!
//! O código do usuário roda dentro de um invólucro:
//! `(function() { <prelúdio> <código>\n})()`. O prelúdio está na mesma
//! linha que a 1ª linha do código, então **as linhas batem** com as do editor
//! (Alt+F11) e só a **coluna da linha 1** fica deslocada pelo tamanho do
//! prelúdio. O Boa ainda acrescenta à pilha a chamada do invólucro (a linha
//! depois da última do código) e as funções da API (`sheet.range`…), que têm
//! numeração própria. Sem nome de fonte, tudo saía como `unknown at :L:C`
//! e um código de 3 linhas citava a "linha 5". Agora a fonte do usuário se
//! chama [`ORIGEM`] e a da API `api`; aqui as posições viram
//! `linha L, coluna C` do editor, a chamada do invólucro some e a API aparece
//! como "API do FeatherSheet".

/// Nome da fonte do código do usuário na pilha do Boa.
pub(crate) const ORIGEM: &str = "macro";

/// Converte `macro:L:C` para a posição no editor. `None` quando a posição é
/// do invólucro (prelúdio na linha 1 ou a chamada final depois do código).
fn posicao_no_editor(linha: usize, coluna: usize, prefixo: usize, linhas: usize) -> Option<(usize, usize)> {
    if linha == 0 || linha > linhas { return None; }
    if linha == 1 {
        return coluna.checked_sub(prefixo).filter(|c| *c >= 1).map(|c| (1, c));
    }
    Some((linha, coluna))
}

/// Lê `L:C` (o que vem depois de `macro:`).
fn ler_linha_coluna(texto: &str) -> Option<(usize, usize)> {
    let (l, c) = texto.split_once(':')?;
    Some((l.trim().parse().ok()?, c.trim().parse().ok()?))
}

/// O que pôr no lugar de `(origem:L:C)`: `Some(texto)` troca, `None` tira.
fn traduzir_local(local: &str, prefixo: usize, linhas: usize) -> Option<String> {
    if let Some(resto) = local.strip_prefix(&format!("{ORIGEM}:")) {
        let (l, c) = ler_linha_coluna(resto)?;
        let (l, c) = posicao_no_editor(l, c, prefixo, linhas)?;
        return Some(format!("linha {l}, coluna {c}"));
    }
    if local.starts_with("api:") {
        return Some("API do FeatherSheet".into());
    }
    Some(local.to_owned())
}

/// Separa `texto (local)` no fim da linha.
fn separar_local(linha: &str) -> Option<(&str, &str)> {
    let sem = linha.strip_suffix(')')?;
    let i = sem.rfind(" (")?;
    Some((&sem[..i], &sem[i + 2..]))
}

/// Ajusta a mensagem de erro do Boa para a numeração do editor.
/// `prefixo`: caracteres do prelúdio antes da 1ª coluna do código.
pub(crate) fn ajustar_erro(erro: &str, prefixo: usize, codigo: &str) -> String {
    let linhas = codigo.split('\n').count();
    let mut saida: Vec<String> = Vec::new();
    // No Falcão o corpo da macro é a função anônima do invólucro: o último quadro anônimo
    // do código do usuário (o de fora de todos).
    let mut anonima: Option<usize> = None;
    for (i, linha) in erro.split('\n').enumerate() {
        let quadro = linha.trim_start().strip_prefix("at ");
        match (quadro, separar_local(linha)) {
            // Quadro da pilha: "    at nome (local)".
            (Some(_), Some((antes, local))) => {
                let Some(novo) = traduzir_local(local, prefixo, linhas) else { continue };
                let nome = antes.trim_start().trim_start_matches("at ");
                let nome = if nome == "<main>" { "corpo da macro" } else { nome };
                saida.push(format!("    em {nome} ({novo})"));
                if nome == "<anônima>" && local.starts_with(&format!("{ORIGEM}:")) { anonima = Some(saida.len() - 1); }
            }
            // Cabeçalho com local: tira o local do invólucro, traduz o resto.
            (None, Some((antes, local))) if i == 0 && (local.starts_with(ORIGEM) || local.starts_with("api:")) => {
                match traduzir_local(local, prefixo, linhas) {
                    Some(novo) => saida.push(format!("{antes} ({novo})")),
                    None => saida.push(antes.to_owned()),
                }
            }
            _ => saida.push(ajustar_sintaxe(linha, prefixo)),
        }
    }
    if let Some(i) = anonima {
        saida[i] = saida[i].replacen("<anônima>", "corpo da macro", 1);
    }
    saida.join("\n")
}

/// Erro de sintaxe: "... at line L, col C" — só a coluna da linha 1 anda.
fn ajustar_sintaxe(linha: &str, prefixo: usize) -> String {
    // Boa: "at line 1, col C"; Falcão: "(linha 1, coluna C)".
    let Some((i, marca)) = ["at line 1, col ", "(linha 1, coluna "].iter()
        .find_map(|m| linha.find(m).map(|i| (i, *m))) else { return linha.to_owned() };
    let inicio = i + marca.len();
    let fim = linha[inicio..].find(|c: char| !c.is_ascii_digit()).map_or(linha.len(), |f| inicio + f);
    let Ok(coluna) = linha[inicio..fim].parse::<usize>() else { return linha.to_owned() };
    match coluna.checked_sub(prefixo).filter(|c| *c >= 1) {
        Some(c) => format!("{}{c}{}", &linha[..inicio], &linha[fim..]),
        None => linha.to_owned(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::planilha::workbook::Workbook;

    fn erro(codigo: &str) -> String {
        let mut pasta = Workbook::default();
        crate::executar(&mut pasta, 0, codigo).erro.expect("a macro devia falhar")
    }

    #[test]
    fn linha_do_erro_bate_com_a_do_editor() {
        // Linha 3 do editor; a chamada do invólucro (linha 4) não aparece.
        let e = erro("log(1);\nlet o = null;\nlog(o.a);");
        // A coluna depende do motor: o Boa aponta a propriedade (`a`), o Falcão a expressão (`o.a`).
        let coluna = if cfg!(feature = "falcao") { 5 } else { 7 };
        assert!(e.contains(&format!("em corpo da macro (linha 3, coluna {coluna})")), "{e}");
        assert!(!e.contains("linha 4") && !e.contains("unknown at"), "{e}");
        let e = erro("log(1);\nfunction g(){\n  throw new Error('x');\n}\ng();");
        assert!(e.starts_with("Error: x"), "{e}");
        let coluna = if cfg!(feature = "falcao") { 1 } else { 2 };
        assert!(e.contains("em g (linha 3, coluna 9)") && e.contains(&format!("(linha 5, coluna {coluna})")), "{e}");
    }

    #[test]
    fn coluna_da_primeira_linha_desconta_o_preludio() {
        let e = erro("throw new Error('um');");
        assert!(e.contains("(linha 1, coluna 7)"), "{e}");
        // Boa: "at line L, col C"; Falcão: "(linha L, coluna C)".
        let e = erro("let a = ;");
        assert!(e.contains("at line 1, col 9") || e.contains("(linha 1, coluna 9)"), "{e}");
        let e = erro("log(1);\n\nlog(2)\nlet a = ;");
        assert!(e.contains("at line 4, col 9") || e.contains("(linha 4, coluna 9)"), "{e}");
    }

    #[test]
    fn quadro_da_api_nao_se_passa_por_linha_da_macro() {
        let e = erro("log(1);\n\nsheet.range('ZZZ').value = 1;\n");
        assert!(e.contains("em range (API do FeatherSheet)"), "{e}");
        // O Falcão aponta o começo do comando; o Boa, a chamada.
        let coluna = if cfg!(feature = "falcao") { 1 } else { 12 };
        assert!(e.contains(&format!("em corpo da macro (linha 3, coluna {coluna})")), "{e}");
    }

    #[test]
    fn traducao_pura_sem_executar() {
        let bruto = "ReferenceError: x is not defined (macro:5:3)\n    at <main> (macro:1:20)\n    at <main> (macro:5:3)";
        assert_eq!(ajustar_erro(bruto, 10, "a\nb\nc\n"), "ReferenceError: x is not defined\n    em corpo da macro (linha 1, coluna 10)");
    }
}
