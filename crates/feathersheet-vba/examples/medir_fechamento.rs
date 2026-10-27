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
    let t = std::time::Instant::now();
    // FEATHERSHEET_JS=arquivo.js troca o código convertido (para medir uma versão instrumentada).
    let codigo = std::env::var("FEATHERSHEET_JS").map_or_else(|_| c.macros[0].codigo.clone(), |p| std::fs::read_to_string(p).unwrap());
    let r = feathersheet_macros::executar(&mut wb, 0, &codigo);
    for l in &r.saida { println!("{l}"); }
    println!("{n} linhas: {:?}  erro {:?}  abas {:?}", t.elapsed(), r.erro, wb.nomes());
    println!("{:?}", r.mensagens);
}
