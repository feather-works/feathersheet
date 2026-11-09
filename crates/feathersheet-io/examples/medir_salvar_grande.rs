//! Quanto de RAM o Ctrl+S de um `.fshz` grande gasta além da pasta aberta:
//! abre, zera o pico e grava pelo mesmo caminho do app (`da_workbook` +
//! `gravar_fshz_para`). Caso de referência (17/10/2026): `2026_03.fshz`,
//! 13,7 milhões de linhas × 46 colunas, que batia 20 GB ao salvar.
//!
//! ```bash
//! cargo run --release --example medir_salvar_grande -- ~/Documentos/FeatherSheet/2026_03.fshz /tmp/saida.fshz
//! ```

use std::time::Instant;

fn mb(campo: &str) -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|l| l.strip_prefix(campo))
        .and_then(|r| r.trim().trim_end_matches(" kB").trim().parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let entrada = args.next().expect("passe o .fshz de entrada");
    let saida = args.next().expect("passe o caminho de saída");

    let t = Instant::now();
    let lida = feathersheet_io::fshz_read::ler_fshz_arquivo_compacto(std::path::Path::new(&entrada)).expect("não abri o arquivo");
    let pasta = feathersheet_io::para_workbook::para_workbook_soltando(lida);
    println!("abrir: {:?} · RSS {:.0} MB · pico {:.0} MB", t.elapsed(), mb("VmRSS:"), mb("VmHWM:"));

    // Zera o pico: daqui em diante o VmHWM é só o do salvar.
    let _ = std::fs::write("/proc/self/clear_refs", "5");
    let base = mb("VmRSS:");

    let t = Instant::now();
    let bruta = feathersheet_io::da_workbook::da_workbook(&pasta, 0);
    println!("cópia para gravar: {:?} · RSS {:.0} MB", t.elapsed(), mb("VmRSS:"));
    let t = Instant::now();
    let arquivo = std::io::BufWriter::new(std::fs::File::create(&saida).expect("não criei a saída"));
    feathersheet_io::fshz_write::gravar_fshz_para(arquivo, &bruta).expect("gravação falhou");
    let pico = mb("VmHWM:");
    println!(
        "gravar: {:?} · pico {:.0} MB · além da pasta aberta {:.0} MB · {:.0} MB no disco",
        t.elapsed(), pico, pico - base,
        std::fs::metadata(&saida).map_or(0.0, |m| m.len() as f64 / 1048576.0)
    );
}
