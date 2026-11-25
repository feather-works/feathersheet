//! Macros ▸ Gravar macro: o motor entrega o que cada transação fez (`transacoes_para_gravar`).
use feathersheet_core::planilha::sheet::edicao::Gravado;
use feathersheet_core::planilha::sheet::Sheet;

#[test]
fn ultimas_transacoes_vem_celula_a_celula_da_mais_velha_a_mais_nova() {
    let mut s = Sheet::new("A");
    s.abrir_transacao();
    s.set_cell(0, 0, "10");
    s.set_cell(0, 1, "=A1*2");
    s.fechar_transacao();
    s.abrir_transacao();
    s.set_cell(1, 0, "oi");
    s.fechar_transacao();
    assert_eq!(s.edicoes_novas(), 2);
    let t = s.transacoes_para_gravar(2);
    let resumo: Vec<Vec<(u32, u32, String)>> = t.iter().map(|passos| passos.iter().filter_map(|p| match p {
        Gravado::Celula { r, c, depois, .. } => Some((*r, *c, depois.as_ref().map(|d| d.raw.clone()).unwrap_or_default())),
        _ => None,
    }).collect()).collect();
    assert_eq!(resumo, vec![vec![(0, 0, "10".into()), (0, 1, "=A1*2".into())], vec![(1, 0, "oi".into())]]);
    assert_eq!(s.transacoes_para_gravar(1).len(), 1);
}
