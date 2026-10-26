//! O protocolo MCP (JSON-RPC 2.0, uma mensagem por linha): `initialize`,
//! `tools/list`, `tools/call` e `ping`. Nada de rede.
use serde_json::{json, Value};

use crate::esquemas;
use crate::ferramentas::chamar;
use crate::hospede::Hospede;

const VERSOES: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const INSTRUCOES: &str = "FeatherSheet: planilha local. Comece por sheet_resumo. Com o aplicativo aberto (modo ao vivo) as ferramentas agem na planilha da tela e cada chamada é um Ctrl+Z; sem ele, abra um arquivo com sheet_abrir e grave com sheet_salvar. NUNCA leia a planilha inteira: sheet_ler tem teto de 2000 células; para muitas linhas use sheet_amostra, sheet_estatistica (perfil de coluna) e sheet_consulta (filtrar/agrupar/somar no motor). Fórmulas em pt-BR: =SOMA(A1:A3;10), =SE(A1>0;\"sim\";\"não\"), vírgula decimal (1,5); nomes das funções em português ou inglês. Para preencher uma coluna com fórmula use sheet_escrever com intervalo + formula (referências relativas andam). Depois de escrever, confira com sheet_ler modo \"ambos\". Para entender um erro ou de onde vem um valor: sheet_formula. Ao vivo: sheet_comando aciona a interface (comando=\"listar\"), sheet_tela descreve a tela e sheet_ver devolve a foto da janela; sheet_mouse move o mouse e clica na tela (cursor \"Claude\").";

fn resposta(id: &Value, resultado: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": resultado })
}

fn erro(id: &Value, codigo: i32, mensagem: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": codigo, "message": mensagem } })
}

fn inicializar(params: &Value) -> Value {
    let pedida = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSOES[0]);
    let versao = if VERSOES.contains(&pedida) { pedida } else { VERSOES[0] };
    json!({ "protocolVersion": versao, "capabilities": { "tools": {} },
        "serverInfo": { "name": "feathersheet", "version": env!("CARGO_PKG_VERSION") }, "instructions": INSTRUCOES })
}

/// Erro de ferramenta volta como resultado com `isError`. Um pânico não
/// derruba o servidor nem o aplicativo que hospeda o código ao vivo.
pub fn chamar_ferramenta(h: &mut dyn Hospede, params: &Value) -> Value {
    let nome = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| chamar(h, nome, &args))) {
        Ok(Ok(conteudo)) => json!({ "content": conteudo, "isError": false }),
        Ok(Err(e)) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
        Err(causa) => {
            let motivo = causa.downcast_ref::<&str>().map(|s| (*s).to_string())
                .or_else(|| causa.downcast_ref::<String>().cloned()).unwrap_or_else(|| "motivo desconhecido".into());
            json!({ "content": [{ "type": "text", "text": format!("erro interno na ferramenta `{nome}`: {motivo}. Confira a planilha (sheet_ler) antes de repetir.") }], "isError": true })
        }
    }
}

/// Atende uma mensagem. `None` para notificações (sem id).
pub fn atender(h: &mut dyn Hospede, msg: &Value) -> Option<Value> {
    let metodo = msg.get("method").and_then(Value::as_str)?;
    let id = msg.get("id")?;
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    Some(match metodo {
        "initialize" => resposta(id, inicializar(&params)),
        "ping" => resposta(id, json!({})),
        "tools/list" => resposta(id, json!({ "tools": esquemas::todas() })),
        "prompts/list" => resposta(id, json!({ "prompts": [] })),
        "tools/call" => resposta(id, chamar_ferramenta(h, &params)),
        outro => erro(id, -32601, &format!("método desconhecido: {outro}")),
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hospede::Avulso;

    #[test]
    fn todas_as_ferramentas_listadas_tem_tratamento() {
        let mut h = Avulso::default();
        for f in esquemas::todas() {
            let nome = f["name"].as_str().unwrap();
            let r = chamar_ferramenta(&mut h, &json!({ "name": nome, "arguments": {} }));
            let texto = r["content"][0]["text"].as_str().unwrap_or("");
            assert!(!texto.contains("Ferramenta desconhecida"), "{nome} sem tratamento");
        }
        let init = atender(&mut h, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "feathersheet");
    }
}
