//! Várias funções num módulo, como as `Sub` de um módulo do VBA: `Módulo.função`
//! (ex. `macro_1.gravar`) roda o módulo e chama só aquela função.
use feather_macros::Catalogo;

/// Funções declaradas num módulo: (nome, linha a partir de 0). Reconhece
/// `function f(`, `async function f(`, `const f = (…) =>`, `let f = function`.
pub fn funcoes_do_modulo(codigo: &str) -> Vec<(String, usize)> {
    let mut saida = Vec::new();
    for (i, linha) in codigo.lines().enumerate() {
        let mut t = linha.trim_start();
        for prefixo in ["export ", "async "] {
            t = t.strip_prefix(prefixo).map(str::trim_start).unwrap_or(t);
        }
        let identificador = |s: &str| -> String {
            s.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect()
        };
        if let Some(resto) = t.strip_prefix("function") {
            let nome = identificador(resto.trim_start_matches('*').trim_start());
            if !nome.is_empty() { saida.push((nome, i)); }
            continue;
        }
        for decl in ["const ", "let ", "var "] {
            let Some(resto) = t.strip_prefix(decl) else { continue };
            let nome = identificador(resto.trim_start());
            let depois = resto.trim_start()[nome.len()..].trim_start();
            let Some(valor) = depois.strip_prefix('=').filter(|v| !v.starts_with('=')) else { break };
            let valor = valor.trim_start();
            if !nome.is_empty() && (valor.starts_with("function") || valor.starts_with("async")
                || valor.contains("=>")) {
                saida.push((nome, i));
            }
            break;
        }
    }
    saida
}

/// O código a rodar para `referencia`: o módulo inteiro pelo nome, ou `Módulo.função`
/// (o código solto do módulo roda antes, como as declarações do VBA). Devolve o nome
/// como fica no histórico e o código.
pub fn codigo_da_macro(catalogo: &Catalogo, referencia: &str) -> Option<(String, String)> {
    let referencia = referencia.trim();
    if let Some(m) = catalogo.obter(referencia) { return Some((m.name.clone(), m.code.clone())); }
    let (modulo, funcao) = referencia.rsplit_once('.')?;
    let m = catalogo.obter(modulo.trim())?;
    let (f, _) = funcoes_do_modulo(&m.code).into_iter().find(|(n, _)| n.eq_ignore_ascii_case(funcao.trim()))?;
    Some((format!("{}.{f}", m.name), format!("{}\n;{f}();\n", m.code)))
}

/// O que um botão ou formulário pode chamar: cada módulo e cada função dele (`Módulo.função`).
pub fn executaveis(catalogo: &Catalogo) -> Vec<String> {
    catalogo.macros.iter().flat_map(|m| std::iter::once(m.name.clone())
        .chain(funcoes_do_modulo(&m.code).into_iter().map(|(f, _)| format!("{}.{f}", m.name)))).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn modulo_ponto_funcao_chama_so_a_funcao() {
        let cat = Catalogo::importar_json(r#"[{"name":"macro_1","code":"function gravar(){ sheet.set('A1', 1); }\nfunction abrir(){ sheet.set('A2', 2); }"}]"#).unwrap();
        assert_eq!(executaveis(&cat), vec!["macro_1", "macro_1.gravar", "macro_1.abrir"]);
        let (nome, codigo) = codigo_da_macro(&cat, "MACRO_1.Abrir").unwrap();
        assert_eq!(nome, "macro_1.abrir");
        assert!(codigo.ends_with(";abrir();\n"));
        assert!(codigo_da_macro(&cat, "macro_1.nada").is_none());
        let mut wb = crate::Workbook::new();
        let r = crate::executar(&mut wb, 0, &codigo);
        assert!(r.erro.is_none(), "{:?}", r.erro);
        assert_eq!((wb.valor(0, 0, 0), wb.valor(0, 1, 0)), (feathersheet_core::Value::Empty, feathersheet_core::Value::Number(2.)));
    }
}
