//! `Workbook::editar` suja só as fórmulas de fora que leem o que a ação
//! tocou: mexer numa coluna que ninguém lê não recalcula as outras abas.
use feathersheet_core::{planilha::workbook::Workbook, Value};

#[test]
fn editar_suja_so_quem_le_a_coluna_tocada() {
    let mut w = Workbook::new();
    w.criar_aba("Dados");
    for r in 0..3 {
        w.escrever_celula(1, r, 0, "1".into());
    }
    w.escrever_celula(0, 0, 0, "=SOMA(Dados!A1:A3)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(3.0));
    let versao = w.versao(0);
    // Coluna B da Dados: a Resumo não lê, não muda.
    w.editar(1, |s| s.set_cell(5, 1, "9"));
    assert_eq!(w.versao(0), versao);
    // Coluna A: suja e recalcula.
    w.editar(1, |s| s.limpar_conteudo(1, 0));
    assert_ne!(w.versao(0), versao);
    assert_eq!(w.valor(0, 0, 0), Value::Number(2.0));
    // O desfazer também passa pelo rastro.
    w.editar(1, |s| { s.undo(); });
    assert_eq!(w.valor(0, 0, 0), Value::Number(3.0));
}
