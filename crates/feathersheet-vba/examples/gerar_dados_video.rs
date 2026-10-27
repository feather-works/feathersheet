//! `cargo run --release --example gerar_dados_video -- 200000 saida.xlsx`: a aba "Vendas" do
//! vídeo da v0.2.0 — exportação de sistema com nomes em caixa e espaços misturados,
//! regiões escritas de jeitos diferentes e ~2% de pedidos repetidos (a macro
//! `Fechamento.bas` limpa tudo isso).
fn main() {
    let mut args = std::env::args().skip(1);
    let n: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(200_000);
    let saida = args.next().unwrap_or_else(|| "vendas-outubro.xlsx".into());
    let nomes = ["ana souza", "bruno lima", "carla dias", "diego rocha", "elisa melo", "fábio nunes", "gabriela reis", "henrique alves",
        "isabela costa", "joão pereira", "karina lopes", "lucas martins", "marina freitas", "nicolas araújo", "olívia barros", "paulo teixeira",
        "renata moura", "sérgio cardoso", "tatiane ribeiro", "vinícius gomes"];
    let regioes = ["Norte", "Nordeste", "Centro-Oeste", "Sudeste", "Sul"];
    let produtos = [("Notebook", "Informática", 4200.0), ("Monitor 27\"", "Informática", 1600.0), ("Teclado", "Periféricos", 180.0),
        ("Mouse", "Periféricos", 90.0), ("Cadeira", "Móveis", 950.0), ("Mesa", "Móveis", 1300.0), ("Impressora", "Informática", 1100.0),
        ("Headset", "Periféricos", 320.0), ("Webcam", "Periféricos", 260.0), ("Nobreak", "Energia", 780.0)];
    let mut semente: u64 = 20261027;
    let mut sorteio = |max: u64| { semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (semente >> 33) % max };
    let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
    wb.renomear_aba(0, "Vendas");
    let s = &mut wb.aba_mut(0).unwrap().sheet;
    for (c, t) in ["Pedido", "Data", "Vendedor", "Região", "Produto", "Categoria", "Qtd", "Preço", "Desconto"].iter().enumerate() {
        s.set_cell(0, c as u32, t);
    }
    let mut pedido = 100_000u32;
    for l in 1..=n {
        // ~2% repetem o pedido anterior (a exportação mandou a linha duas vezes).
        if sorteio(100) >= 2 { pedido += 1; }
        let v = sorteio(nomes.len() as u64) as usize;
        let nome = match sorteio(4) { 0 => nomes[v].to_uppercase(), 1 => format!("  {}", nomes[v]), 2 => format!("{} ", nomes[v]), _ => nomes[v].to_string() };
        let regiao = regioes[v % regioes.len()];
        let regiao = match sorteio(3) { 0 => regiao.to_lowercase(), 1 => format!("{regiao} "), _ => regiao.to_string() };
        let (produto, categoria, preco) = produtos[sorteio(produtos.len() as u64) as usize];
        let preco = preco * (0.9 + sorteio(21) as f64 / 100.0);
        let campos = [pedido.to_string(), format!("{:02}/10/2026", 1 + sorteio(31)), nome, regiao, produto.into(), categoria.into(),
            (1 + sorteio(9)).to_string(), format!("{:.2}", preco).replace('.', ","), format!("{}", sorteio(26) as f64 / 100.0).replace('.', ",")];
        for (c, t) in campos.iter().enumerate() { s.set_cell(l, c as u32, t); }
    }
    let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&wb).expect("gerar .xlsx");
    std::fs::write(&saida, bytes).expect("gravar");
    println!("{n} linhas em {saida}");
}
