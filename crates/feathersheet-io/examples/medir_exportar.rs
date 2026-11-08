//! Mede a exportação como o app faz (#100): o retrato na thread da janela
//! (`da_workbook`) e o resto (remontar e gerar o .xlsx), que vai para outra thread.
//!
//! cargo run --release --example medir_exportar -- pasta.fshz saida.xlsx
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let entrada = args.next().expect("o .fshz");
    let saida = args.next().expect("o .xlsx de saída");
    let t = Instant::now();
    let wb = feathersheet_io::para_workbook::para_workbook_soltando(
        feathersheet_io::fshz_read::ler_fshz_arquivo(std::path::Path::new(&entrada)).expect("não abri"));
    println!("abrir: {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let pasta = feathersheet_io::da_workbook::da_workbook(&wb, 0);
    println!("retrato (thread da janela): {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let bytes = feathersheet_io::fshz_write::gerar_fshz2(&pasta).unwrap();
    let copia = feathersheet_io::para_workbook::para_workbook_soltando(feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
    drop(bytes);
    println!("remontar (fundo): {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let partes = feathersheet_io::xlsx_write::montar_partes(&copia);
    let cru: usize = partes.iter().map(|(_, b)| b.len()).sum();
    println!("  montar o XML: {} ms, {} MB sem compactar", t.elapsed().as_millis(), cru >> 20);
    let t2 = Instant::now();
    let z = feathersheet_io::zip_write::zipar(partes).unwrap();
    println!("  compactar: {} ms, {} MB", t2.elapsed().as_millis(), z.len() >> 20);
    drop(z);
    let t = Instant::now();
    let (xlsx, _) = feathersheet_io::xlsx_write::gerar_xlsx_com_funcoes(&copia).unwrap();
    feathersheet_io::arquivo::gravar_atomico(std::path::Path::new(&saida), &xlsx).unwrap();
    println!("gerar e gravar .xlsx (fundo): {} ms, {} MB", t.elapsed().as_millis(), xlsx.len() >> 20);
}
