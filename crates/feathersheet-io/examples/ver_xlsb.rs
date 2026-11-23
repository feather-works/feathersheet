//! `cargo run --example ver_xlsb -- arquivo.xlsb`: mostra o que o leitor de `.xlsb` entrega.
fn main() {
    for caminho in std::env::args().skip(1) {
        println!("=== {caminho}");
        let pasta = match feathersheet_io::xlsb_read::ler_xlsb(std::fs::read(&caminho).unwrap()) { Ok((p, perdidas)) => { println!("fórmulas não lidas: {perdidas}"); p } Err(e) => { println!("erro: {e}"); continue } };
        for i in 0..pasta.len() {
            let aba = pasta.aba(i).unwrap();
            println!("-- aba {i}");
            for r in 0..14 { for c in 0..5 {
                let raw = aba.sheet.get_raw(r, c);
                if !raw.is_empty() { println!("  {r},{c}: {raw:?} -> {:?}", pasta.valor(i, r, c)); }
            } }
        }
    }
}
