//! Abrir e calcular como o app faz (leitura + cálculo em paralelo), com o
//! diário do motor: `medir_abrir_calcular ARQUIVO.xlsx|.fshz`.
use std::time::Instant;

fn main() {
    let caminho = std::path::PathBuf::from(std::env::args().nth(1).expect("arquivo"));
    let t = Instant::now();
    let mut w = if caminho.extension().is_some_and(|e| e == "fshz") {
        let lida = feathersheet_io::fshz_read::ler_fshz_arquivo(&caminho).expect("ler FSHZ");
        println!("[lido] {:.2} s", t.elapsed().as_secs_f64());
        feathersheet_io::para_workbook::para_workbook_soltando(lida)
    } else {
        feathersheet_io::xlsx_read::ler_xlsx_arquivo(&caminho).expect("ler").0
    };
    println!("[aberto] {:.2} s", t.elapsed().as_secs_f64());
    let t2 = Instant::now();
    let n = w.calcular_em_paralelo();
    println!("[calculado] {n} células em {:.2} s · total {:.2} s", t2.elapsed().as_secs_f64(), t.elapsed().as_secs_f64());
    for linha in feathersheet_core::diario::recolher() {
        println!("  {linha}");
    }
    let ver = |aba: &str, r: u32, c: u32| {
        let i = w.indice_por_nome(aba).expect("aba");
        println!("  {aba}!({r},{c}) = {:?}", w.valor(i, r, c));
    };
    ver("Dados", 1_000_000, 10);
    ver("Resumo", 1, 1);
    ver("Resumo", 1, 13);
    ver("Produtos", 1, 4);
    ver("Produtos", 1, 5);
}
