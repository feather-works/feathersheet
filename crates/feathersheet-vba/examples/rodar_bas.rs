//! `cargo run --release --example rodar_bas -- pasta-com-bas [filtro]`: converte cada `.bas` da
//! pasta sozinho, roda a `Sub Teste` numa pasta nova e mostra o que a macro escreveu em Z1
//! (as macros de teste conferem a si mesmas e escrevem "OK" ou "FALHOU: …"), o erro de execução
//! e o que ficou sem converter.
use feathersheet_core::planilha::workbook::Workbook;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let pasta = std::path::PathBuf::from(args.next().expect("passe a pasta dos .bas"));
    let filtro = args.next().unwrap_or_default().to_lowercase();
    let mut arquivos: Vec<_> = std::fs::read_dir(&pasta).expect("ler a pasta").filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bas")))
        .filter(|p| p.to_string_lossy().to_lowercase().contains(&filtro)).collect();
    arquivos.sort();
    let (mut ok, mut total) = (0, 0);
    for arq in arquivos {
        total += 1;
        let nome = arq.file_stem().unwrap().to_string_lossy().into_owned();
        let codigo = std::fs::read_to_string(&arq).expect("ler o .bas");
        let c = feathersheet_vba::converter_texto(&[(nome.as_str(), codigo.as_str())], &[]);
        let pend: Vec<String> = c.pendencias.iter().filter(|p| !p.aviso).map(|p| format!("{}: {}", p.linha, p.motivo)).collect();
        let Some(m) = c.macros.iter().find(|m| m.nome.eq_ignore_ascii_case("teste") || m.nome.to_lowercase().ends_with(".teste")) else {
            println!("SEM   {nome}: a Sub Teste não converteu ({} pendências)", pend.len());
            continue;
        };
        let mut wb = Workbook::new();
        let mut respostas = Vec::new();
        let r = loop {
            let ctx = feathersheet_macros::Contexto { respostas: respostas.clone(),
                limites: feathersheet_macros::Limites { prazo: Duration::from_secs(20), parar: None }, ..Default::default() };
            let r = feathersheet_macros::executar_com(&mut wb, 0, &m.codigo, ctx);
            if r.pergunta.is_some() && respostas.len() < 20 { respostas.push(true); continue; }
            break r;
        };
        let z1 = wb.aba(0).map(|a| a.sheet.get_raw(0, 25)).unwrap_or_default();
        let estado = match (&r.erro, z1.as_str()) {
            (None, "OK") => { ok += 1; "OK   ".to_string() }
            (Some(e), _) => format!("ERRO  {}", e.lines().next().unwrap_or("")),
            (None, z) => format!("Z1    {z}"),
        };
        println!("{nome:<28} {estado}");
        for p in pend.iter().take(6) { println!("        pendente {p}"); }
        if std::env::var("MOSTRAR_JS").is_ok() { println!("{}", m.codigo); }
    }
    println!("\n{ok} de {total} OK");
}
