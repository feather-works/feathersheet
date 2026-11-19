//! Comparação de desempenho com outra planilha (GridCraft, 19/11/2026): abre um `.xlsx`,
//! calcula tudo, mostra G1:I1 (os totais da planilha de teste) e grava um `.xlsx`.
//!
//! cargo run --release --example comparar_desempenho -- ENTRADA.xlsx [SAIDA.xlsx]
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let entrada = std::path::PathBuf::from(args.next().expect("o .xlsx"));
    let t = Instant::now();
    let (mut w, _) = feathersheet_io::xlsx_read::ler_xlsx_arquivo(&entrada).expect("ler");
    let abrir = t.elapsed();
    let t2 = Instant::now();
    w.calcular_em_paralelo();
    let calcular = t2.elapsed();
    let totais: Vec<String> = (6..9).map(|c| format!("{:?}", w.valor(0, 0, c))).collect();
    println!("abrir {:.3} s · calcular {:.3} s · G1:I1 = {}", abrir.as_secs_f64(), calcular.as_secs_f64(), totais.join("  "));
    if let Some(saida) = args.next() {
        let t3 = Instant::now();
        let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&w).expect("gerar");
        std::fs::write(&saida, bytes).expect("gravar");
        println!("gravar {:.3} s", t3.elapsed().as_secs_f64());
    }
    println!("total {:.3} s", t.elapsed().as_secs_f64());
}
