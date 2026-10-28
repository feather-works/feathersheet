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

/// Trocar de aba: `update` e os primeiros `view` de cada aba, duas voltas
/// (a segunda já com os gráficos guardados).
/// `FEATHERSHEET_ARQUIVO=pasta.xlsx cargo test --profile medir --bin feathersheet medir_troca_de_aba -- --ignored --nocapture`.
#[test]
#[ignore]
fn medir_troca_de_aba() {
    let caminho = PathBuf::from(std::env::var("FEATHERSHEET_ARQUIVO").expect("FEATHERSHEET_ARQUIVO"));
    let (workbook, folha) = App::decodificar_caminho(&caminho, None).expect("abrir");
    let mut app = App::default();
    app.instalar(caminho, workbook, folha);
    let t = Instant::now();
    app.adiantar_graficos();
    eprintln!("[adiantar_graficos, na janela] {:.1} ms", ms(t));
    for volta in 0..2 {
        for i in 0..app.workbook.len() {
            let t = Instant::now();
            let _ = app.update(Msg::Aba(i));
            let atualizar = ms(t);
            let vistas: Vec<String> = (0..3).map(|_| {
                let t = Instant::now();
                let _: Element<'_, Msg> = app.view(window::novo_id());
                format!("{:.1}", ms(t))
            }).collect();
            eprintln!("[volta {volta}] aba {i} {:?}: update {atualizar:.1} ms · view {} ms",
                app.workbook.nomes()[i], vistas.join(" / "));
            let desenhos: Vec<String> = (0..2).map(|_| {
                use crate::compat::canvas::Program;
                let mut r = pelicano::Renderer::new();
                let t = Instant::now();
                app.grade().draw(&Default::default(), &mut r, crate::compat::Rectangle { x: 0.0, y: 0.0, width: 1280.0, height: 560.0 }, &pelicano::Theme::light(), None);
                format!("{:.1}", ms(t))
            }).collect();
            eprintln!("    grade: {} ms", desenhos.join(" / "));
            let t = Instant::now();
            let camada = app.graficos_da_camada();
            eprintln!("    graficos_da_camada: {:.1} ms ({} gráficos)", ms(t), camada.len());
            if volta == 0 {
                for (_, ret, svg) in camada {
                    let t = Instant::now();
                    let opcoes = resvg::usvg::Options { fontdb: fontes(), ..Default::default() };
                    let arvore = resvg::usvg::Tree::from_str(&svg, &opcoes).unwrap();
                    let analisar = ms(t);
                    let t = Instant::now();
                    let (w, h) = (ret.largura.max(1.0) as u32, ret.altura.max(1.0) as u32);
                    let mut img = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
                    let sz = arvore.size();
                    resvg::render(&arvore, resvg::tiny_skia::Transform::from_scale(w as f32 / sz.width(), h as f32 / sz.height()), &mut img.as_mut());
                    eprintln!("    gráfico {w}x{h} ({} kB de SVG): analisar {analisar:.1} ms · rasterizar {:.1} ms", svg.len() / 1024, ms(t));
                }
            }
        }
    }
}

fn fontes() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static F: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let t = Instant::now();
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        eprintln!("    [fontes do sistema] {:.1} ms", ms(t));
        std::sync::Arc::new(db)
    }).clone()
}
