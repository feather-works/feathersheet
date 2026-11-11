//! Lista própria na alça (P, M, G, GG). Arquivo à parte porque as listas da pessoa são
//! globais no processo, como o idioma.
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::operacoes::fill::{plan_fill, SourceCell};
use feathersheet_core::operacoes::ordenacao_ext::{definir_listas_do_usuario, lista_de_texto};
use feathersheet_core::Value;

fn arrastar(origem: &str, ate: u32) -> Vec<String> {
    let ler = |r: u32, _c: u32| {
        let raw = if r == 0 { origem.to_string() } else { String::new() };
        SourceCell { value: Value::Text(raw.clone()), raw, style: None }
    };
    plan_fill(ler, &Rect::new(0, 0, 0, 0), &Rect::new(1, 0, ate, 0)).into_iter().map(|c| c.raw).collect()
}

#[test]
fn a_alca_continua_a_lista_da_pessoa_e_da_a_volta() {
    assert_eq!(arrastar("M", 2), ["M", "M"], "sem lista, copia");
    definir_listas_do_usuario(vec![lista_de_texto("P; M; G; GG"), lista_de_texto("só um")]);
    assert_eq!(arrastar("M", 3), ["G", "GG", "P"]);
    assert_eq!(arrastar("jan", 1), ["fev"], "as prontas continuam valendo");
    assert_eq!(arrastar("só um", 1), ["só um"], "lista de um item não conta");
    definir_listas_do_usuario(Vec::new());
    assert_eq!(arrastar("M", 1), ["M"]);
}
