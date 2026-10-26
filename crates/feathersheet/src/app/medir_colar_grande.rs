//! Medição (ignorada): colar um valor da linha 2 até a última num CSV
//! grande, pelo mesmo caminho do Ctrl+V. `FEATHERSHEET_CSV=arquivo.csv
//! cargo test --release medir_colar -- --ignored --nocapture`.
use super::*;
use std::time::Instant;

#[test]
#[ignore]
fn medir_colar_coluna_inteira() {
    let caminho = PathBuf::from(std::env::var("FEATHERSHEET_CSV").expect("FEATHERSHEET_CSV"));
    let (workbook, folha) = App::decodificar_caminho(&caminho, Some(',')).expect("abrir");
    let mut app = App::default();
    app.instalar(caminho, workbook, folha);
    let ultima = app.workbook.aba(0).unwrap().sheet.max_row;
    let _ = app.update(Msg::Selecionar(1, 3));
    app.fim_selecao = (ultima, 3);
    let t = Instant::now();
    let _ = app.update(Msg::Colar("5".into()));
    eprintln!("[colar] update em {:.2} s · {}", t.elapsed().as_secs_f64(), app.status);
    let t = Instant::now();
    let _: Element<'_, Msg> = app.view(window::novo_id());
    eprintln!("[tela] view em {:.2} s", t.elapsed().as_secs_f64());
    for (nome, msg) in [("calcular", Msg::CalcularPulso), ("monitor", Msg::MonitorPulso), ("resumo", Msg::ResumoPulso)] {
        let t = Instant::now();
        let _ = app.update(msg);
        eprintln!("[{nome}] {:.2} s", t.elapsed().as_secs_f64());
    }
    let t = Instant::now();
    let _: Element<'_, Msg> = app.view(window::novo_id());
    eprintln!("[tela] view de novo em {:.2} s", t.elapsed().as_secs_f64());
    // A cópia da recuperação automática converte a pasta na thread da tela.
    let celulas = app.workbook.aba(0).unwrap().sheet.len();
    eprintln!("[recuperação] grande demais? {} ({celulas} células)",
        feathersheet_io::backup_policy::grande_demais_para_copia_automatica(celulas as u64));
    let t = Instant::now();
    let pasta = feathersheet_io::da_workbook::da_workbook(&app.workbook, 0);
    eprintln!("[recuperação] da_workbook na thread da tela em {:.2} s", t.elapsed().as_secs_f64());
    drop(pasta);
    // Valor copiado de dentro do app (Ctrl+C numa célula).
    let _ = app.update(Msg::Selecionar(1, 4));
    let _ = app.copiar_selecao();
    let _ = app.update(Msg::Selecionar(1, 5));
    app.fim_selecao = (ultima, 5);
    let texto = app.area_copiada.as_ref().map(|c| c.texto.clone()).unwrap_or_default();
    let t = Instant::now();
    let _ = app.update(Msg::Colar(texto));
    eprintln!("[colar interno] update em {:.2} s · {}", t.elapsed().as_secs_f64(), app.status);
    for linha in feathersheet_core::diario::recolher() {
        eprintln!("  {linha}");
    }
}

/// Planilha em branco, "100.000.000" da linha 1 até a última (1.048.576).
#[test]
#[ignore]
fn medir_colar_na_planilha_em_branco() {
    let mut app = App::default();
    let _ = app.update(Msg::Selecionar(0, 0));
    // FEATHERSHEET_ATE=linha (base 0); padrão: a última da planilha.
    let ate = std::env::var("FEATHERSHEET_ATE").ok().and_then(|v| v.parse().ok())
        .unwrap_or(feathersheet_core::celulas::refs::MAX_ROWS - 1);
    app.fim_selecao = (ate, 0);
    eprintln!("[seleção] até {ate} · coluna inteira? {}", app.selecao_e_colunas_inteiras());
    let t = Instant::now();
    let _ = app.update(Msg::Colar("100.000.000".into()));
    eprintln!("[colar em branco] update em {:.2} s · {}", t.elapsed().as_secs_f64(), app.status);
    let sheet = &app.workbook.aba(0).unwrap().sheet;
    eprintln!("[estilos] {} · valor {:?}", sheet.styles.len(), sheet.get_value(5, 0));
    let t = Instant::now();
    let _ = app.update(Msg::Desfazer);
    eprintln!("[desfazer] {:.2} s", t.elapsed().as_secs_f64());
}
