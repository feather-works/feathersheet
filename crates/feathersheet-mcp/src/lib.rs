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
pub mod grafico;
pub mod botao;
pub mod formulario;
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

/// Maior mensagem aceita (uma linha de JSON): acima disto a linha é descartada sem ir toda para a memória.
pub(crate) const MAX_MENSAGEM: usize = 16 << 20;

/// A próxima linha (sem o "\n"): `None` no fim, `Some(Err(()))` quando passou de [`MAX_MENSAGEM`]
/// (o resto dela é lido e jogado fora, sem guardar).
pub(crate) fn ler_linha(leitura: &mut impl std::io::BufRead) -> Option<Result<String, ()>> {
    use std::io::{BufRead, Read};
    let mut buf = Vec::new();
    let n = leitura.by_ref().take(MAX_MENSAGEM as u64 + 1).read_until(b'\n', &mut buf).ok()?;
    if n == 0 { return None; }
    if buf.len() > MAX_MENSAGEM && buf.last() != Some(&b'\n') {
        loop {
            let pedaco = leitura.fill_buf().ok()?;
            if pedaco.is_empty() { break; }
            match pedaco.iter().position(|&b| b == b'\n') {
                Some(i) => { leitura.consume(i + 1); break; }
                None => { let tam = pedaco.len(); leitura.consume(tam); }
            }
        }
        return Some(Err(()));
    }
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') { buf.pop(); }
    Some(String::from_utf8(buf).map_err(|_| ()))
}

/// O servidor por stdio (uma mensagem JSON por linha): é o `feathersheet-mcp`
/// e também o `feathersheet --mcp`, para quem instalou só o aplicativo. Com o
/// FeatherSheet aberto (e o MCP ligado nas Configurações), as chamadas vão
/// para ele; sem ele, o servidor trabalha sobre arquivos.
pub fn rodar_stdio() {
    use std::io::Write;
    use serde_json::{json, Value};
    let mut avulso = hospede::Avulso::default();
    let mut vivo = ao_vivo::Cliente::default();
    let saida = std::io::stdout();
    let mut entrada = std::io::stdin().lock();
    while let Some(linha) = ler_linha(&mut entrada) {
        let resposta = match linha {
            Err(()) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32600, "message": format!("Mensagem grande demais ou inválida (limite de {} MB)", MAX_MENSAGEM >> 20) } })),
            Ok(linha) if linha.trim().is_empty() => continue,
            Ok(linha) => match serde_json::from_str::<Value>(&linha) {
            Ok(msg) => responder(&mut avulso, &mut vivo, &linha, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("JSON inválido: {e}") } })),
            },
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

#[cfg(test)]
mod testes_linha {
    #[test]
    fn linha_grande_demais_e_jogada_fora_e_a_seguinte_vale() {
        let grande = "x".repeat(super::MAX_MENSAGEM + 10);
        let texto = format!("{{\"a\":1}}\r\n{grande}\n{{\"b\":2}}\n");
        let mut leitura = std::io::BufReader::new(texto.as_bytes());
        assert_eq!(super::ler_linha(&mut leitura), Some(Ok("{\"a\":1}".into())));
        assert_eq!(super::ler_linha(&mut leitura), Some(Err(())));
        assert_eq!(super::ler_linha(&mut leitura), Some(Ok("{\"b\":2}".into())));
        assert_eq!(super::ler_linha(&mut leitura), None);
    }
}
