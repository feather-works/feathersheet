//! Comparação de desempenho com outra planilha (GridCraft, 19/11/2026): abre um `.xlsx` ou `.csv`,
//! calcula tudo, mostra G1:I1 (os totais da planilha de teste), e opcionalmente ordena e grava.
//!
//! cargo run --release --example comparar_desempenho -- ENTRADA [--ordenar COLUNA] [--salvar SAIDA.xlsx]
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::operacoes::ops;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entrada = std::path::PathBuf::from(&args[0]);
    let opcao = |nome: &str| args.iter().position(|a| a == nome).and_then(|i| args.get(i + 1)).cloned();
    let t = Instant::now();
    let mut w = if entrada.extension().is_some_and(|e| e == "csv") {
        feathersheet_io::csv_workbook::importar_csv_tsv_arquivo(&entrada, false, Some(',')).expect("ler csv")
    } else {
        feathersheet_io::xlsx_read::ler_xlsx_arquivo(&entrada).expect("ler").0
    };
    let abrir = t.elapsed();
    let t2 = Instant::now();
    w.calcular_em_paralelo();
    let calcular = t2.elapsed();
    let totais: Vec<String> = (6..9).map(|c| format!("{:?}", w.valor(0, 0, c))).collect();
    println!("abrir {:.3} s · calcular {:.3} s · G1:I1 = {}", abrir.as_secs_f64(), calcular.as_secs_f64(), totais.join("  "));
    if let Some(coluna) = opcao("--ordenar") {
        let indice = coluna.bytes().next().expect("letra") as u32 - b'A' as u32;
        // O caminho do botão A→Z do app (`App::ordenar`): planeja lendo os valores e permuta as linhas.
        let t3 = Instant::now();
        let usado = w.aba(0).unwrap().sheet.used_rect();
        let area = Rect::new(0, 0, usado.r1, usado.c1);
        let plano = ops::planejar_ordenacao(area, &[ops::SortKey { col: indice, ascending: true }], true, usado.r1, |r, c| w.valor(0, r, c));
        let planejar = t3.elapsed();
        let t4 = Instant::now();
        if let Some(p) = plano { w.editar(0, |s| ops::aplicar_ordenacao(s, 0, usado.c1, &p)); }
        println!("ordenar: planejar {:.3} s · mover {:.3} s · A2 = {:?} · último = {:?}", planejar.as_secs_f64(),
            t4.elapsed().as_secs_f64(), w.valor(0, 1, 0), w.valor(0, usado.r1, 0));
    }
    if let Some(saida) = opcao("--salvar") {
        let t4 = Instant::now();
        let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&w).expect("gerar");
        std::fs::write(&saida, bytes).expect("gravar");
        println!("gravar {:.3} s", t4.elapsed().as_secs_f64());
    }
    println!("total {:.3} s", t.elapsed().as_secs_f64());
}
