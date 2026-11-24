//! O que vale como hiperlink de célula: web (`http`, `https`, `www.`), `mailto:` e `#Aba!A1` (um lugar desta
//! pasta). Nada de esquema executável (`javascript:`, `file:`). Usado pelo app (diálogo, clique) e pelo MCP.

/// Mesma política de src/ui/grade/link.ts: esquema web/mailto ou www.,
/// sem espaços, tags nem esquemas executáveis. Não faz chamadas de rede.
pub fn normalizar_link(valor: &str) -> Option<String> {
    let valor = valor.trim();
    if !(8..=2048).contains(&valor.len()) || valor.chars().any(|c|
        c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\')
    ) {
        return None;
    }
    // Caminho quente do Canvas: não alocar lowercase para cada célula visível.
    let prefixo = |p: &str| valor.get(..p.len())
        .is_some_and(|inicio| inicio.eq_ignore_ascii_case(p));
    if prefixo("https://") || prefixo("http://") || prefixo("mailto:") {
        let apos = valor.split_once(':')?.1;
        if apos.is_empty() || (apos.starts_with("//") && apos.len() <= 2) {
            return None;
        }
        return Some(valor.to_owned());
    }
    if prefixo("www.") {
        let resto = &valor[4..];
        let (dominio, _) = resto.split_once('.').unwrap_or(("", ""));
        if !dominio.is_empty() && resto.split_once('.').is_some_and(|(_, t)| !t.is_empty()) {
            return Some(format!("https://{valor}"));
        }
    }
    None
}

/// O link guardado na célula (estilo ou diálogo): além dos da web, `#Aba!A1` é um lugar
/// desta pasta (o `location` do Excel). Texto digitado na célula não vira link interno.
pub fn link_do_estilo(valor: &str) -> Option<String> {
    match valor.trim().strip_prefix('#') {
        Some(lugar) if !lugar.trim().is_empty() && lugar.len() <= 255 && !lugar.chars().any(char::is_control) =>
            Some(format!("#{}", lugar.trim())),
        Some(_) => None,
        None => normalizar_link(valor),
    }
}


#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_web_mailto_e_lugar_da_pasta() {
        assert_eq!(link_do_estilo("www.exemplo.com.br").as_deref(), Some("https://www.exemplo.com.br"));
        assert_eq!(link_do_estilo("mailto:ana@ex.com").as_deref(), Some("mailto:ana@ex.com"));
        assert_eq!(link_do_estilo("#Vendas!A1").as_deref(), Some("#Vendas!A1"));
        for ruim in ["javascript:alert(1)", "file:///etc/passwd", "https://a b.com", "http://", "#", "C:\\x.exe"] {
            assert_eq!(link_do_estilo(ruim), None, "{ruim}");
        }
    }
}
