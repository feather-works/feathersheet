//! `cargo run --example converter_bas -- Modulo.bas [Codinome=Aba ...]`: converte um módulo
//! exportado do editor do Excel e mostra o JavaScript e as pendências.

fn main() {
    let mut args = std::env::args().skip(1);
    let caminho = args.next().expect("passe o caminho do .bas");
    let codigo = std::fs::read_to_string(&caminho).expect("não consegui ler o arquivo");
    let abas: Vec<(String, String)> = args.filter_map(|a| a.split_once('=').map(|(c, n)| (c.into(), n.into()))).collect();
    let abas: Vec<(&str, &str)> = abas.iter().map(|(c, n)| (c.as_str(), n.as_str())).collect();
    let nome = std::path::Path::new(&caminho).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let c = feathersheet_vba::converter_texto(&[(&nome, &codigo)], &abas);
    for m in &c.macros {
        println!("// ===== {} =====\n{}", m.nome, m.codigo);
    }
    for p in &c.pendencias {
        println!("{} linha {}: {} — {}", if p.aviso { "aviso" } else { "PENDENTE" }, p.linha, p.motivo, p.vba);
    }
}
