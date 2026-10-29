//! Abre um `.fshz` ou `.parquet`, compacta as colunas e confere que nenhuma célula mudou
//! (impressão digital de cada coluna antes e depois), com a RAM e o tempo.
//!
//! cargo run --release --example medir_compactar -- arquivo.fshz
use feathersheet_core::planilha::data_table::Cru;

fn rss() -> f64 {
    std::fs::read_to_string("/proc/self/status").unwrap_or_default().lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|r| r.trim().trim_end_matches(" kB").trim().parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

fn digitais(wb: &feathersheet_core::planilha::workbook::Workbook) -> Vec<u64> {
    let mut v = Vec::new();
    for i in 0..wb.len() {
        let dados = &wb.aba(i).unwrap().sheet.data;
        for col in dados.colunas() {
            let mut h = 0xcbf2_9ce4_8422_2325u64;
            dados.for_each_cru_da_coluna(col, &mut |r, c| {
                let x = match c { Cru::Numero(n) => n.to_bits(), Cru::Texto(id) => 0x8000_0000_0000_0000 | id as u32 as u64 };
                h = (h ^ u64::from(r)).wrapping_mul(0x100_0000_01b3);
                h = (h ^ x).wrapping_mul(0x100_0000_01b3);
            });
            v.push(h);
        }
    }
    v
}

fn main() {
    let caminho = std::path::PathBuf::from(std::env::args().nth(1).expect("o arquivo"));
    feathersheet_core::paralelismo::ajustar_alocador();
    let t = std::time::Instant::now();
    let mut wb = if caminho.extension().is_some_and(|e| e == "parquet") {
        feathersheet_io::parquet_read::ler_parquet_arquivo(&caminho).unwrap()
    } else {
        feathersheet_io::para_workbook::para_workbook_soltando(feathersheet_io::fshz_read::ler_fshz_arquivo(&caminho).unwrap())
    };
    feathersheet_core::paralelismo::devolver_memoria_ao_sistema();
    println!("abrir: {:.1} s · RAM {:.0} MB", t.elapsed().as_secs_f64(), rss());
    let t = std::time::Instant::now();
    let antes = digitais(&wb);
    println!("ler tudo cru: {:.1} s", t.elapsed().as_secs_f64());
    let t = std::time::Instant::now();
    let nomes: Vec<String> = { let s = &wb.aba(0).unwrap().sheet; (0..60).map(|c| s.get_raw(0, c)).collect() };
    let mut colunas = wb.aba_mut(0).unwrap().sheet.data.compactar();
    colunas.sort_by_key(|c| c.0);
    let mb = |b: usize| b as f64 / 1048576.0;
    for (c, n, t) in &colunas {
        println!("  {:>2} {:<14} números {:>5.0} → {:>5.0} MB · textos {:>5.0} → {:>5.0} MB", c + 1,
            nomes.get(*c as usize).map_or("", String::as_str), mb(n.0), mb(n.1), mb(t.0), mb(t.1));
    }
    let (a, d) = colunas.iter().fold((0, 0), |(x, y), (_, n, t)| (x + n.0 + t.0, y + n.1 + t.1));
    feathersheet_core::paralelismo::devolver_memoria_ao_sistema();
    println!("compactar: {:.1} s · valores {:.0} → {:.0} MB · RAM {:.0} MB", t.elapsed().as_secs_f64(),
        a as f64 / 1048576.0, d as f64 / 1048576.0, rss());
    let t = std::time::Instant::now();
    let depois = digitais(&wb);
    println!("ler tudo compactado: {:.1} s · {} colunas · iguais: {}", t.elapsed().as_secs_f64(), antes.len(), antes == depois);
    assert_eq!(antes, depois, "alguma célula mudou");
}
