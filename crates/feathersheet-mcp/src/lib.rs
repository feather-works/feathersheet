//! O servidor MCP do FeatherSheet. A lógica das ferramentas fica aqui, sobre um
//! [`hospede::Hospede`]: a pasta aberta por arquivo ([`hospede::Avulso`]) ou o
//! próprio aplicativo, ao vivo (o `feathersheet-gui` implementa o hospedeiro e
//! atende o canal de [`ao_vivo`]).
//!
//! Regra central (ver `feathersheet/CLAUDE.md`, "MCP do FeatherSheet"): nunca
//! despejar a planilha para a IA. Ler tem teto de células; resumo, estatística
//! e consulta são calculados pelo motor, e a IA recebe só o resultado.
pub mod ao_vivo;
pub mod arquivo;
pub mod condicional;
pub mod validacao;
pub mod dados;
pub mod edicao;
pub mod esquemas;
pub mod ferramentas;
pub mod formato;
pub mod hospede;
pub mod macros;
pub mod permissoes;
pub mod rpc;
pub mod texto;

/// O servidor por stdio (uma mensagem JSON por linha): é o `feathersheet-mcp`
/// e também o `feathersheet --mcp`, para quem instalou só o aplicativo. Com o
/// FeatherSheet aberto (e o MCP ligado nas Configurações), as chamadas vão
/// para ele; sem ele, o servidor trabalha sobre arquivos.
pub fn rodar_stdio() {
    use std::io::{BufRead, Write};
    use serde_json::{json, Value};
    let mut avulso = hospede::Avulso::default();
    let mut vivo = ao_vivo::Cliente::default();
    let saida = std::io::stdout();
    for linha in std::io::stdin().lock().lines() {
        let Ok(linha) = linha else { break };
        if linha.trim().is_empty() {
            continue;
        }
        let resposta = match serde_json::from_str::<Value>(&linha) {
            Ok(msg) => responder(&mut avulso, &mut vivo, &linha, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("JSON inválido: {e}") } })),
        };
        if let Some(r) = resposta {
            let mut s = saida.lock();
            let _ = writeln!(s, "{r}");
            let _ = s.flush();
        }
    }
}

fn responder(avulso: &mut hospede::Avulso, vivo: &mut ao_vivo::Cliente, linha: &str, msg: &serde_json::Value) -> Option<serde_json::Value> {
    use serde_json::{json, Value};
    if msg.get("method").and_then(Value::as_str) == Some("tools/call") {
        match vivo.repassar(linha) {
            Some(Ok(r)) => return serde_json::from_str(&r).ok(),
            Some(Err(e)) => {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                return Some(json!({ "jsonrpc": "2.0", "id": id, "result": { "isError": true, "content": [{ "type": "text", "text": e }] } }));
            }
            None => {}
        }
    }
    rpc::atender(avulso, msg)
}
