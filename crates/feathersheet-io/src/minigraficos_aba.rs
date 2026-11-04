//! O JSON dos minigráficos da aba (`Sheet::mg`). O modelo e as contas ficam
//! em `feathersheet_core::operacoes::minigraficos`; o motor não lê JSON.
use feathersheet_core::operacoes::minigraficos::Grupo;

/// Lê a lista de grupos guardada na aba (vazio ou inválido = nenhum).
pub fn ler(json: Option<&str>) -> Vec<Grupo> {
    json.and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default()
}

/// O texto que vai para `Sheet::mg` (`None` quando não sobrou grupo).
pub fn gravar(grupos: &[Grupo]) -> Option<String> {
    if grupos.is_empty() { None } else { serde_json::to_string(grupos).ok() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feathersheet_core::celulas::refs::parse_range_ref;
    use feathersheet_core::operacoes::minigraficos::{criar, Tipo};

    #[test]
    fn json_vai_e_volta_e_campos_faltando_usam_o_padrao() {
        let r = |a| parse_range_ref(a).unwrap();
        let g = criar(Tipo::GanhosPerdas, r("A1:E1"), None, r("F1")).unwrap();
        assert_eq!(ler(gravar(std::slice::from_ref(&g)).as_deref()), vec![g]);
        let curto = ler(Some(r#"[{"itens":[{"linha":0,"coluna":5,"dados":"A1:E1"}]}]"#));
        assert_eq!((curto[0].tipo, curto[0].cor.as_str()), (Tipo::Linha, "#376092"));
        assert!(ler(Some("lixo")).is_empty());
        assert_eq!(gravar(&[]), None);
    }
}
