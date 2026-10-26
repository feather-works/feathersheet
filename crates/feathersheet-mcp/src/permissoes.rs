//! Quais chamadas só leem a planilha. Serve ao modo "somente leitura" das
//! Configurações do aplicativo: com ele ligado, o resto é recusado antes de
//! chegar à planilha.
use serde_json::Value;

/// A chamada `nome` com `args` só lê (não muda a pasta, o arquivo nem a tela)?
pub fn so_le(nome: &str, args: &Value) -> bool {
    let acao = args.get("acao").and_then(Value::as_str).unwrap_or("");
    match nome {
        "sheet_resumo" | "sheet_ler" | "sheet_formula" | "sheet_estatistica" | "sheet_amostra"
        | "sheet_localizar" | "sheet_tela" | "sheet_ver" => true,
        "sheet_aba" | "sheet_nomes" => acao == "listar",
        "sheet_macro" => matches!(acao, "listar" | "ler"),
        "sheet_filtrar" | "sheet_dinamica" | "sheet_condicional" | "sheet_validacao" | "sheet_grafico" => acao == "listar",
        // Sem destino a consulta só devolve a prévia.
        "sheet_consulta" => args.get("destino").is_none_or(Value::is_null),
        _ => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn leitura_passa_e_escrita_nao() {
        assert!(so_le("sheet_ler", &json!({ "intervalo": "A1" })));
        assert!(so_le("sheet_aba", &json!({ "acao": "listar" })));
        assert!(!so_le("sheet_aba", &json!({ "acao": "excluir", "nome": "X" })));
        assert!(!so_le("sheet_escrever", &json!({})));
        assert!(so_le("sheet_consulta", &json!({ "fonte": "A1:B3" })));
        assert!(!so_le("sheet_consulta", &json!({ "fonte": "A1:B3", "destino": "R" })));
        assert!(!so_le("sheet_comando", &json!({ "comando": "listar" })));
        assert!(!so_le("ferramenta_desconhecida", &json!({})));
    }
}
