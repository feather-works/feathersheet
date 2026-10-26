//! Rolagem da grade ao trocar a célula ativa, com zoom diferente de 100%.
//! Bug de 14/10/2026: com zoom 75%, clicar perto da borda de baixo ou da
//! direita rolava a tela e jogava a célula para o topo, porque a conta usava
//! 28 linhas × 12 colunas fixas da configuração.
use super::*;
use crate::grade::Exigencia;

/// Canvas de 1280 × 700 px de tela, já "desenhado" uma vez.
fn app_com_zoom(zoom: u16) -> App {
    let mut app = App::default();
    app.config.zoom = zoom;
    app.eixos_grade.borrow_mut().tamanho_canvas = Some(pelicano::Size::new(1280.0, 700.0));
    app
}

/// A célula ativa cabe inteira na tela (nada mais a rolar).
fn inteira_a_vista(app: &App) -> bool {
    let tela = app.eixos_grade.borrow().tamanho_canvas.unwrap();
    app.grade().origem_para_mostrar(app.linha, app.coluna, tela, Exigencia::Inteira) == (None, None)
}

#[test]
fn zoom_75_clique_na_ultima_linha_e_coluna_visiveis_nao_rola() {
    let mut app = app_com_zoom(75);
    // (700 − 13) / 0,75 − 30 = 886 px lógicos → 31 linhas de 28 px;
    // (1280 − 13) / 0,75 − 90 = 1599 px → 13 colunas de 122 px.
    let (linhas, colunas) = app.pagina_da_grade();
    assert_eq!((linhas, colunas), (31, 13));
    assert!(linhas > app.config.linhas_visiveis && colunas > app.config.colunas_visiveis,
        "o teste precisa cair além dos 28 × 12 fixos de antes");
    app.update(Msg::Selecionar(linhas - 1, colunas - 1));
    assert_eq!((app.linha, app.coluna), (linhas - 1, colunas - 1));
    assert_eq!((app.linha_inicio, app.coluna_inicio), (0, 0), "o clique não pode rolar");
    // A linha logo abaixo aparece cortada pela metade: clicar nela também não rola.
    app.update(Msg::Selecionar(linhas, colunas));
    assert_eq!((app.linha_inicio, app.coluna_inicio), (0, 0));
}

#[test]
fn zoom_75_seta_na_ultima_visivel_rola_uma_linha_so() {
    let mut app = app_com_zoom(75);
    let (linhas, colunas) = app.pagina_da_grade();
    app.update(Msg::Selecionar(linhas - 1, colunas - 1));
    app.update(Msg::Mover(1, 0));
    assert_eq!(app.linha, linhas);
    assert_eq!(app.linha_inicio, 1, "seta para baixo rola uma linha, não joga a célula para o topo");
    assert!(inteira_a_vista(&app));
    app.update(Msg::Mover(0, 1));
    assert_eq!(app.coluna, colunas);
    assert_eq!(app.coluna_inicio, 1);
    assert!(inteira_a_vista(&app));
    // Dentro da tela a seta não rola.
    app.update(Msg::Mover(-1, -1));
    assert_eq!((app.linha_inicio, app.coluna_inicio), (1, 1));
}

#[test]
fn zoom_150_clique_nao_rola_e_seta_rola_uma_linha() {
    let mut app = app_com_zoom(150);
    // (700 − 13) / 1,5 − 30 = 428 px → 15 linhas; (1280 − 13) / 1,5 − 90 → 6 colunas.
    let (linhas, colunas) = app.pagina_da_grade();
    assert_eq!((linhas, colunas), (15, 6));
    app.update(Msg::Selecionar(linhas - 1, colunas - 1));
    assert_eq!((app.linha_inicio, app.coluna_inicio), (0, 0));
    app.update(Msg::Mover(1, 0));
    assert_eq!((app.linha, app.linha_inicio), (linhas, 1));
    app.update(Msg::Mover(0, 1));
    assert_eq!((app.coluna, app.coluna_inicio), (colunas, 1));
    assert!(inteira_a_vista(&app));
}

#[test]
fn seta_para_cima_acima_da_tela_poe_a_linha_no_topo() {
    let mut app = app_com_zoom(75);
    app.linha_inicio = 40;
    app.trocar_selecao(45, 0);
    assert_eq!(app.linha_inicio, 40);
    for _ in 0..6 { app.update(Msg::Mover(-1, 0)); }
    assert_eq!((app.linha, app.linha_inicio), (39, 39));
}

#[test]
fn salto_longe_deixa_a_celula_inteira_na_ultima_linha() {
    // Ctrl+End, Ir para…: a célula aparece inteira, colada embaixo/à direita.
    for zoom in [75, 100, 150] {
        let mut app = app_com_zoom(zoom);
        let (linhas, colunas) = app.pagina_da_grade();
        app.trocar_selecao(5_000, 300);
        assert!(inteira_a_vista(&app), "zoom {zoom}");
        assert_eq!(app.linha_inicio, 5_000 + 1 - linhas, "zoom {zoom}");
        assert_eq!(app.coluna_inicio, 300 + 1 - colunas, "zoom {zoom}");
    }
}

#[test]
fn alturas_proprias_e_paineis_congelados_entram_na_conta() {
    let mut app = app_com_zoom(75);
    let id = app.workbook.aba(0).unwrap().id;
    app.congelados.insert(id, (2, 1));
    // Linhas altas reduzem o que cabe; uma oculta não ocupa lugar.
    app.workbook.editar(0, |s| {
        s.row_heights.insert(5, 100);
        s.row_heights.insert(6, 0);
        s.col_widths.insert(3, 300);
    });
    let (linhas, colunas) = app.pagina_da_grade();
    // Roláveis: 886 − 2 × 28 = 830 px; a linha 5 (100) vale 3,57 linhas
    // padrão e a 6 some: cabem 27 roláveis inteiras.
    assert_eq!(linhas, 27);
    // 1599 − 122 da congelada = 1477 px; a coluna D (300) leva duas e meia.
    assert_eq!(colunas, 10);
    // Última inteira: congeladas (0,1) + roláveis 2..=29 menos a oculta 6.
    let ultima_linha = 2 + linhas; // conta a oculta no meio
    let ultima_coluna = colunas; // 1 congelada + roláveis 1..=10
    app.update(Msg::Selecionar(ultima_linha, ultima_coluna));
    assert_eq!((app.linha_inicio, app.coluna_inicio), (0, 0));
    assert!(inteira_a_vista(&app));
    app.update(Msg::Mover(1, 1));
    assert!(inteira_a_vista(&app));
    // A linha alta saiu de cena aos poucos: a origem anda o mínimo.
    assert!(app.linha_inicio > 2 && app.linha_inicio <= 6, "linha_inicio {}", app.linha_inicio);
    assert!(app.coluna_inicio > 1 && app.coluna_inicio <= 3, "coluna_inicio {}", app.coluna_inicio);
    // Célula congelada nunca rola.
    let antes = (app.linha_inicio, app.coluna_inicio);
    app.update(Msg::Selecionar(0, 0));
    app.trocar_selecao(1, 0);
    assert_eq!((app.linha_inicio, app.coluna_inicio), antes);
}

#[test]
fn page_down_anda_uma_tela_real_com_zoom() {
    for (zoom, pagina) in [(75, 31), (150, 15)] {
        let mut app = app_com_zoom(zoom);
        app.update(Msg::LinhaPagina(1));
        assert_eq!(app.linha_inicio, pagina, "zoom {zoom}");
        app.update(Msg::LinhaPagina(-1));
        assert_eq!(app.linha_inicio, 0, "zoom {zoom}");
    }
}

#[test]
fn sem_tamanho_do_canvas_usa_a_estimativa_antiga() {
    let mut app = App::default();
    app.trocar_selecao(100, 40);
    assert_eq!((app.linha_inicio, app.coluna_inicio), (98, 38));
}
