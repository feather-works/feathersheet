//! Medição (ignorada) das travadas da v0.1.2 num arquivo do usuário:
//! selecionar a área usada inteira, a fotografia da recuperação automática
//! e apagar uma fórmula + Ctrl+Z.
//! `FEATHERSHEET_ARQUIVO=arquivo.fshz cargo test --release medir_travadas -- --ignored --nocapture`.
use super::*;
use std::time::Instant;

fn ms(t: Instant) -> f64 { t.elapsed().as_secs_f64() * 1000.0 }

#[test]
#[ignore]
fn medir_travadas() {
    let caminho = PathBuf::from(std::env::var("FEATHERSHEET_ARQUIVO").expect("FEATHERSHEET_ARQUIVO"));
    let t = Instant::now();
    let (workbook, folha) = App::decodificar_caminho(&caminho, None).expect("abrir");
    eprintln!("[abrir] {:.0} ms", ms(t));
    let mut app = App::default();
    app.instalar(caminho, workbook, folha);
    let usado = app.workbook.aba(app.folha).unwrap().sheet.used_rect();
    eprintln!("[área usada] {}x{} = {} células",
        usado.r1 + 1, usado.c1 + 1, (usado.r1 as u64 + 1) * (usado.c1 as u64 + 1));

    // 1. Selecionar tudo o que está usado.
    let _ = app.update(Msg::Selecionar(0, 0));
    let t = Instant::now();
    let _ = app.update(Msg::Estender(usado.r1, usado.c1));
    eprintln!("[seleção] update {:.1} ms", ms(t));
    for i in 0..3 {
        let t = Instant::now();
        let _: Element<'_, Msg> = app.view(window::novo_id());
        eprintln!("[seleção] view {i} {:.1} ms", ms(t));
    }
    let (mut pulsos, mut pior, inicio) = (0, 0.0f64, Instant::now());
    while app.resumo_selecao.borrow().pendente && pulsos < 10_000 {
        let t = Instant::now();
        let _ = app.update(Msg::ResumoPulso);
        pior = pior.max(ms(t));
        pulsos += 1;
    }
    eprintln!("[seleção] resumo: {pulsos} pulsos, pior {pior:.1} ms, total {:.0} ms", ms(inicio));

    // 2. A fotografia da pasta (recuperação automática e salvar).
    let celulas: u64 = (0..app.workbook.len()).filter_map(|i| app.workbook.aba(i)).map(|a| a.sheet.len() as u64).sum();
    eprintln!("[recuperação] {celulas} células guardadas; pulada por ser grande? {}",
        feathersheet_io::backup_policy::grande_demais_para_copia_automatica(celulas));
    let t = Instant::now();
    let pasta = feathersheet_io::da_workbook::da_workbook(&app.workbook, 0);
    eprintln!("[recuperação] da_workbook {:.0} ms", ms(t));
    drop(pasta);

    // 3. Apagar a primeira fórmula empacotada e desfazer.
    let sheet = &app.workbook.aba(app.folha).unwrap().sheet;
    let coluna = sheet.formulas.colunas().first().copied();
    if let Some(c) = coluna {
        let r = (0..=usado.r1).find(|&r| sheet.get_raw(r, c).starts_with('=')).unwrap_or(1);
        let _ = app.update(Msg::Selecionar(r, c));
        for (nome, msg) in [("apagar", Msg::Limpar), ("ctrl+z", Msg::Desfazer)] {
            let t = Instant::now();
            let _ = app.update(msg);
            eprintln!("[{nome}] update {:.1} ms", ms(t));
            let t = Instant::now();
            let _: Element<'_, Msg> = app.view(window::novo_id());
            eprintln!("[{nome}] view {:.1} ms", ms(t));
        }
    } else {
        eprintln!("[apagar] sem fórmula empacotada");
    }
    for linha in feathersheet_core::diario::recolher() {
        eprintln!("  {linha}");
    }
}
