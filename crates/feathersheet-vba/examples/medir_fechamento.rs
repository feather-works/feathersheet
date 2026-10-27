//! `cargo run --release --example medir_fechamento -- 20000`: converte a macro do vídeo da
//! v0.2.0 (`docs/planejamento/video-v0.2/Fechamento.bas`), roda numa aba "Vendas" sintética
//! com N linhas (2% de pedidos repetidos) e mede o tempo.
fn main() {
    let n: u32 = std::env::args().nth(1).unwrap().parse().unwrap();
    let vba = std::fs::read_to_string("../../docs/planejamento/video-v0.2/Fechamento.bas").unwrap();
    let c = feathersheet_vba::converter_texto(&[("Fechamento", &vba)], &[]);
    let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
    wb.renomear_aba(0, "Vendas");
    let s = &mut wb.aba_mut(0).unwrap().sheet;
    for (c, t) in ["Pedido", "Data", "Vendedor", "Região", "Produto", "Categoria", "Qtd", "Preço", "Desconto"].iter().enumerate() { s.set_cell(0, c as u32, t); }
    let vend = ["  ana souza", "BRUNO lima ", "carla dias", "diego rocha", "  elisa melo"];
    let reg = ["norte", "Nordeste ", "centro-oeste", "SUDESTE", "sul"];
    for i in 1..=n {
        let k = (i * 7919) % 1000;
        let v = [format!("{}", 100000 + i - (i % 50 == 0) as u32), format!("{:02}/10/2026", 1 + k % 28), vend[(k % 5) as usize].into(), reg[(k % 5) as usize].into(), format!("P{}", k % 40), "Cat".into(), format!("{}", 1 + k % 9), format!("{}", 50 + k), format!("{}", (k % 20) as f64 / 100.0)];
        for (c, t) in v.iter().enumerate() { s.set_cell(i, c as u32, t); }
    }
    // FEATHERSHEET_JS=arquivo.js troca o código convertido (para medir uma versão instrumentada).
    let codigo = std::env::var("FEATHERSHEET_JS").map_or_else(|_| c.macros[0].codigo.clone(), |p| std::fs::read_to_string(p).unwrap());
    // FEATHERSHEET_XLSX=arquivo.xlsx: roda nos dados desse arquivo (aba "Vendas") em vez dos sintéticos.
    if let Ok(x) = std::env::var("FEATHERSHEET_XLSX") {
        let t = std::time::Instant::now();
        wb = if x.ends_with(".fshz") {
            feathersheet_io::para_workbook::para_workbook_soltando(feathersheet_io::fshz_read::ler_fshz_arquivo(std::path::Path::new(&x)).expect("ler .fshz"))
        } else {
            feathersheet_io::xlsx_read::ler_xlsx_arquivo(std::path::Path::new(&x)).expect("ler .xlsx").0
        };
        println!("lido em {:?}", t.elapsed());
    }
    let t = std::time::Instant::now();
    // O prazo do app quando a macro roda pelo botão (noutra thread).
    let contexto = feathersheet_macros::Contexto { limites: feathersheet_macros::Limites { prazo: std::time::Duration::from_secs(600), parar: None }, ..Default::default() };
    let r = feathersheet_macros::executar_com(&mut wb, 0, &codigo, contexto);
    for l in &r.saida { println!("{l}"); }
    println!("{n} linhas: {:?}  erro {:?}  abas {:?}", t.elapsed(), r.erro, wb.nomes());
    println!("{:?}", r.mensagens);
    // O começo do relatório, para conferir o resultado.
    if let Some(f) = wb.nomes().iter().position(|n| n == "Fechamento") {
        for l in 0..30 {
            let linha: Vec<String> = (0..6).map(|c| wb.aba(f).unwrap().sheet.get_raw(l, c)).collect();
            if linha.iter().any(|x| !x.is_empty()) { println!("{:>3} {}", l + 1, linha.join(" | ")); }
        }
    }
}
