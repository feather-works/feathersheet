//! MEM-2 e MEM-4: o motor dentro do teto de memória ([`crate::teste_memoria::Teto`]).

use crate::teste_memoria::Teto;

/// MEM-2: levar uma fórmula até a última linha (100 milhões) numa coluna vazia vai em
/// blocos (`preencher_formula_faixa_vazia`), sem uma entrada por célula.
#[test]
fn formula_ate_o_fim_da_coluna_cabe_no_teto() {
    use crate::celulas::refs::MAX_ROWS;
    let mut s = crate::planilha::sheet::Sheet::new("Dados");
    s.set_cell(0, 0, "1");
    let teto = Teto::novo("formula_ate_o_fim_da_coluna", 8 << 20);
    assert!(s.preencher_formula_faixa_vazia(2, 0, MAX_ROWS - 1, "=A1*2"));
    eprintln!("MEM-2 coluna inteira: pico {} KB", teto.pico() >> 10);
}

/// MEM-2: a linha de fórmula levada até o fim em `n` colunas; ~43 KB por coluna depois
/// do bloco constante (antes, ~400 MB por coluna).
fn formula_em_colunas(n: u32, teto_mb: usize) -> std::time::Duration {
    use crate::celulas::refs::MAX_ROWS;
    let mut s = crate::planilha::sheet::Sheet::new("Dados");
    s.set_cell(0, 0, "1");
    let teto = Teto::novo("formula_em_colunas", teto_mb << 20);
    let t = std::time::Instant::now();
    // Numa transação, como o colar e o arrastar da tela (ROB-10: invalida uma vez ao fechar).
    s.abrir_transacao();
    for c in 1..=n {
        assert!(s.preencher_formula_faixa_vazia(c, 0, MAX_ROWS - 1, "=$A$1*2"), "coluna {c}");
    }
    s.fechar_transacao();
    eprintln!("MEM-2 {n} colunas: pico {} MB em {:?}", teto.pico() >> 20, t.elapsed());
    t.elapsed()
}

#[test]
fn formula_em_mil_colunas_ate_o_fim_cabe_no_teto() {
    formula_em_colunas(1024, 64);
}

/// A planilha inteira (16.384 colunas × 100 milhões de linhas), o caso que o usuário
/// testou com tudo selecionado: 711 MB medidos em 10/11/2026. Era lento (136 s em debug:
/// cada coluna refazia a invalidação olhando as outras) até o ROB-10:
/// `cargo test --release --lib formula_na_planilha_inteira -- --ignored --nocapture`.
#[test]
#[ignore]
fn formula_na_planilha_inteira_cabe_no_teto() {
    formula_em_colunas(crate::celulas::refs::MAX_COLS - 1, 800);
}

/// MEM-4: a dinâmica de 1 milhão de linhas (5 regiões × 12 meses) guarda por grupo, não
/// por linha. Os dois caminhos: o antigo (tabular sem subtotal) e a árvore de grupos.
#[test]
fn dinamica_de_um_milhao_de_linhas_cabe_no_teto() {
    use crate::celulas::{refs::Rect, values::Value};
    use crate::dinamica::dinamica_layout::{montar, Layout, Opcoes};
    use crate::dinamica::pivot::{AgregacaoOp, DinamicaConfig, MedidaDinamica};
    const N: usize = 1_000_000;
    const REGIOES: [&str; 5] = ["Norte", "Sul", "Leste", "Oeste", "Centro"];
    let valor = |r: usize, c: usize| match (r, c) {
        (0, 0) => Value::Text("Região".into()),
        (0, 1) => Value::Text("Mês".into()),
        (0, _) => Value::Text("Valor".into()),
        (_, 0) => Value::Text(REGIOES[r % 5].into()),
        (_, 1) => Value::Number((r % 12 + 1) as f64),
        _ => Value::Number((r % 97) as f64),
    };
    let config = DinamicaConfig {
        origem: Rect::new(0, 0, N as u32, 2), linhas: vec![0, 1], colunas: vec![],
        valores: vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }], agrupar_data: None,
    };
    let compacto = Opcoes { layout: Layout::Compacto, subtotais: true, ..Opcoes::antigas() };
    for (nome, opcoes) in [("antigo", Opcoes::antigas()), ("árvore", compacto)] {
        let teto = Teto::novo("dinamica_de_um_milhao_de_linhas", 1 << 20);
        let r = montar(N + 1, valor, &config, &opcoes);
        assert!(r.matriz.height() > 60, "{nome}: 5 regiões × 12 meses");
        eprintln!("MEM-4 dinâmica {nome}: pico {} KB", teto.pico() >> 10);
    }
}
