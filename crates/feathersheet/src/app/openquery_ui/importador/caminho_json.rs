//! Seleção de conteúdo JSON vindo de APIs HTTP(S).
//! Porta a regra de `seguirCaminho` do antigo core/fn-web.ts:
//! `dados.itens[0].linhas`, múltiplos índices e acesso por chave.
//! Não materializa tabelas nem duplica a resposta: devolve uma referência.

use serde_json::Value as Json;

/// Interpreta somente propriedades e índices de arrays, nunca executa código.
pub(super) fn seguir<'a>(raiz: &'a Json, caminho: &str) -> Option<&'a Json> {
    let caminho = caminho.trim();
    if caminho.is_empty() { return Some(raiz); }
    if caminho.len() > 1024 { return None; }
    let mut atual = raiz;
    for segmento in caminho.split('.') {
        let segmento = segmento.trim();
        if segmento.is_empty() { return None; }
        let bytes = segmento.as_bytes();
        let mut pos = 0usize;
        while pos < bytes.len() && bytes[pos] != b'[' && bytes[pos] != b']' { pos += 1; }
        if pos > 0 {
            atual = atual.as_object()?.get(&segmento[..pos])?;
        }
        if pos == 0 && (bytes.first() != Some(&b'[')) { return None; }
        while pos < bytes.len() {
            if bytes[pos] != b'[' { return None; }
            pos += 1;
            let inicio = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() { pos += 1; }
            if inicio == pos || bytes.get(pos) != Some(&b']') { return None; }
            let indice = segmento[inicio..pos].parse::<usize>().ok()?;
            atual = atual.as_array()?.get(indice)?;
            pos += 1;
        }
    }
    Some(atual)
}

/// `#json=dados.itens[0]` seleciona um nó sem alterar a URL enviada ao
/// servidor. O fragmento nunca é enviado pelo cliente HTTP. Os outros
/// fragmentos tradicionais são ignorados, preservando URLs existentes.
pub(super) fn extrair_fragmento(url: &mut reqwest::Url) -> Result<Option<String>, String> {
    let fragmento = url.fragment().map(str::to_owned);
    url.set_fragment(None);
    let Some(valor) = fragmento.as_deref().and_then(|f| f.strip_prefix("json=")) else {
        return Ok(None);
    };
    let entrada = valor.as_bytes();
    let mut bytes = Vec::with_capacity(entrada.len());
    let mut i = 0;
    while i < entrada.len() {
        if entrada[i] == b'%' {
            let (Some(&a), Some(&b)) = (entrada.get(i + 1), entrada.get(i + 2)) else {
                return Err(tr!("Caminho JSON contém escape percentual inválido").into());
            };
            let hex = |c: u8| (c as char).to_digit(16).map(|n| n as u8);
            let (Some(a), Some(b)) = (hex(a), hex(b)) else {
                return Err(tr!("Caminho JSON contém escape percentual inválido").into());
            };
            bytes.push(a * 16 + b);
            i += 3;
        } else {
            bytes.push(entrada[i]);
            i += 1;
        }
    }
    let caminho = String::from_utf8(bytes)
        .map_err(|_| tr!("Caminho JSON precisa estar em UTF-8").to_string())?;
    if caminho.trim().is_empty() || caminho.len() > 1024 {
        return Err(tr!("Informe um caminho JSON de até 1024 caracteres após #json=").into());
    }
    Ok(Some(caminho))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn percorre_objetos_arrays_e_multiplos_indices() {
        let dado = serde_json::json!({
            "dados": {"itens": [
                {"nome": "Ana", "matriz": [[2,3],[4,5]]},
                {"nome": "Bia"}
            ]}
        });
        assert_eq!(seguir(&dado, " dados.itens[0].nome "), Some(&Json::String("Ana".into())));
        assert_eq!(seguir(&dado, "dados.itens[0].matriz[1][0]"), Some(&serde_json::json!(4)));
        assert_eq!(seguir(&dado, ""), Some(&dado));
    }

    #[test]
    fn rejeita_ausencias_e_sintaxe_malformada() {
        let dado = serde_json::json!({"a":[1]});
        for caminho in ["a[1]", "a[-1]", "a[x]", "a[0", "a[0]x",
            "a..b", "a[0].x", "a[9999999999999999999999999]"] {
            assert!(seguir(&dado, caminho).is_none(), "caminho: {caminho}");
        }
    }

    #[test]
    fn fragmento_json_nao_vai_ao_servidor_e_decodifica_percentuais() {
        let mut url = reqwest::Url::parse(
            "https://api.exemplo.com/dados.json#json=dados.itens%5B0%5D.linhas"
        ).unwrap();
        assert_eq!(extrair_fragmento(&mut url).unwrap(),
            Some("dados.itens[0].linhas".into()));
        assert_eq!(url.as_str(), "https://api.exemplo.com/dados.json");
        let mut sem_path = reqwest::Url::parse("https://exemplo.com/arquivo.csv#secao").unwrap();
        assert_eq!(extrair_fragmento(&mut sem_path).unwrap(), None);
        assert_eq!(sem_path.as_str(), "https://exemplo.com/arquivo.csv");
    }
}
