//! Quanto a tabela dinâmica leva com N linhas (metas da 0.4.0: 2 campos de linha + 1 de valor ≤ 200 ms; arrastar
//! um campo ≤ 100 ms): `cargo run --profile medir --example medir_dinamica [linhas]`. A leitura é por coluna, como
//! o app faz em fundo (`dinamica_ui/fundo.rs`); a mediana de 5 rodadas.
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::dinamica::dinamica_layout::{self, Layout, Opcoes};
use feathersheet_core::dinamica::exibicao::Extras;
use feathersheet_core::dinamica::pivot::{AgregacaoOp, DinamicaConfig, MedidaDinamica};
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
    let regioes: Vec<Value> = (0..8).map(|i| Value::Text(format!("Região {i}"))).collect();
    let produtos: Vec<Value> = (0..50).map(|i| Value::Text(format!("Produto {i:02}"))).collect();
    let mut cols: Vec<Vec<Value>> = vec![Vec::with_capacity(n); 4];
    let mut x: u64 = 7;
    for _ in 0..n {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        cols[0].push(regioes[(x >> 33) as usize % 8].clone());
        cols[1].push(produtos[(x >> 40) as usize % 50].clone());
        cols[2].push(Value::Number(2020.0 + ((x >> 20) % 5) as f64));
        cols[3].push(Value::Number(((x >> 11) % 10_000) as f64 / 100.0));
    }
    let cab = ["Região", "Produto", "Ano", "Valor"].map(|t| Value::Text(t.into()));
    let leitor = |r: usize, c: usize| if r == 0 { cab[c].clone() } else { cols[c][r - 1].clone() };
    let soma = vec![MedidaDinamica { coluna: 3, op: AgregacaoOp::Soma }];
    let cfg = |linhas: Vec<usize>, colunas: Vec<usize>| DinamicaConfig {
        origem: Rect::new(0, 0, n as u32, 3), linhas, colunas, valores: soma.clone(), agrupar_data: None,
    };
    let compacto = |subtotais: bool| Opcoes { layout: Layout::Compacto, subtotais, repetir_rotulos: false, ..Opcoes::antigas() };
    let casos: Vec<(&str, DinamicaConfig, Opcoes)> = vec![
        ("1 linha + valor (compacto)", cfg(vec![0], vec![]), compacto(false)),
        ("2 linhas + valor (compacto)", cfg(vec![0, 1], vec![]), compacto(false)),
        ("2 linhas + valor (compacto, subtotais)", cfg(vec![0, 1], vec![]), compacto(true)),
        ("2 linhas + valor (tabular)", cfg(vec![0, 1], vec![]), Opcoes::antigas()),
        ("2 linhas + 1 coluna + valor (compacto)", cfg(vec![0, 1], vec![2]), compacto(false)),
    ];
    println!("{n} linhas");
    for (nome, c, o) in &casos {
        let mut tempos: Vec<f64> = (0..5).map(|_| {
            let t = Instant::now();
            let r = dinamica_layout::montar_com_extras(n + 1, leitor, c, o, &Extras::default());
            std::hint::black_box(r.matriz.height());
            t.elapsed().as_secs_f64() * 1000.0
        }).collect();
        tempos.sort_by(f64::total_cmp);
        println!("{nome}: {:.0} ms (mín {:.0}, máx {:.0})", tempos[2], tempos[0], tempos[4]);
    }
}
