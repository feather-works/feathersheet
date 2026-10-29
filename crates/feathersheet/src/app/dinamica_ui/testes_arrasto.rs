//! Arrastar campos no painel: reordenar a hierarquia e tirar o último campo.
use super::*;

/// Região, Produto e Valor; dinâmica criada e painel de campos aberto.
fn app_com_dinamica() -> App {
    let mut app = App::default();
    app.workbook.editar(0, |s| {
        for (r, (a, b, c)) in [("Região", "Produto", "Valor"), ("Norte", "A", "10"), ("Sul", "B", "30"),
            ("Norte", "B", "20")].into_iter().enumerate() {
            s.set_cell(r as u32, 0, a); s.set_cell(r as u32, 1, b); s.set_cell(r as u32, 2, c);
        }
    });
    app.fim_selecao = (3, 2);
    app.evento_dinamica(Evento::Abrir);
    crate::app::dinamica_ui::campos_de_antes(&mut app);
    app.evento_dinamica(Evento::Criar);
    assert!(app.dinamica.as_ref().is_some_and(|e| e.editando.is_some()), "painel de campos aberto");
    app
}

/// Arrasta o campo `i` de `de` e solta sobre o campo `j` de `para`
/// (`None`: no vazio da área; área `None`: fora do painel).
fn arrastar(app: &mut App, de: (AreaDinamica, usize), para: Option<(AreaDinamica, Option<usize>)>) {
    let col = app.dinamica.as_ref().unwrap().coluna_em(de.0, de.1).unwrap();
    app.evento_dinamica(Evento::ArrastoInicio(col, Some(de)));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(0.0, 0.0)));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(40.0, 40.0)));
    app.evento_dinamica(Evento::ArrastoSobre(para.map(|p| p.0)));
    if let Some((area, Some(j))) = para { app.evento_dinamica(Evento::ArrastoSobreItem(Some((area, j)))); }
    app.evento_dinamica(Evento::ArrastoSoltar);
}

fn destino(app: &App) -> usize {
    (0..app.workbook.len()).find(|&i| app.workbook.aba(i).is_some_and(|a| a.sheet.dn.is_some())).unwrap()
}

#[test]
fn arrastar_dentro_da_area_reordena_a_hierarquia() {
    let mut app = app_com_dinamica();
    app.dinamica.as_mut().unwrap().linhas = vec![0, 1];
    arrastar(&mut app, (AreaDinamica::Linhas, 1), Some((AreaDinamica::Linhas, Some(0))));
    assert_eq!(app.dinamica.as_ref().unwrap().linhas, vec![1, 0], "Produto passa para cima");
    // Soltar no vazio da área leva para o fim.
    arrastar(&mut app, (AreaDinamica::Linhas, 0), Some((AreaDinamica::Linhas, None)));
    assert_eq!(app.dinamica.as_ref().unwrap().linhas, vec![0, 1]);
    // De outra área, entra na posição em que foi solto.
    app.dinamica.as_mut().unwrap().colunas = vec![];
    arrastar(&mut app, (AreaDinamica::Linhas, 1), Some((AreaDinamica::Colunas, None)));
    arrastar(&mut app, (AreaDinamica::Colunas, 0), Some((AreaDinamica::Linhas, Some(0))));
    assert_eq!(app.dinamica.as_ref().unwrap().linhas, vec![1, 0]);
}

#[test]
fn tirar_o_ultimo_campo_de_linhas_atualiza_a_grade() {
    let mut app = app_com_dinamica();
    let d = destino(&app);
    let tem = |app: &App, texto: &str| {
        let s = &app.workbook.aba(d).unwrap().sheet;
        (0..6).any(|r| (0..4).any(|c| s.get_raw(r, c) == texto))
    };
    assert!(tem(&app, "Norte"));
    arrastar(&mut app, (AreaDinamica::Linhas, 0), None);
    assert!(app.dinamica.as_ref().unwrap().linhas.is_empty());
    assert!(!tem(&app, "Norte"), "a grade não pode ficar com o resultado antigo");
    assert!(tem(&app, "60"), "fica o total geral");
    // Sem campo nenhum: o aviso no lugar da tabela.
    arrastar(&mut app, (AreaDinamica::Valores, 0), None);
    assert!(!tem(&app, "60"));
}

/// Mede criar, trocar campo e detalhar no arquivo pesado do usuário:
/// `FEATHERSHEET_PESADO=/caminho.fshz cargo test --release medir_dinamica_pesada -- --ignored --nocapture`.
#[test]
#[ignore]
fn medir_dinamica_pesada() {
    let Ok(caminho) = std::env::var("FEATHERSHEET_PESADO") else { return };
    let bytes = std::fs::read(&caminho).unwrap();
    let lida = feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap();
    let mut app = App::default();
    app.workbook = feathersheet_io::para_workbook::para_workbook(&lida);
    app.folha = 0;
    let usado = app.workbook.aba(0).unwrap().sheet.used_rect();
    eprintln!("origem: {} linhas × {} colunas", usado.r1 + 1, usado.c1 + 1);
    app.fim_selecao = (usado.r1, usado.c1);
    let t = std::time::Instant::now();
    app.evento_dinamica(Evento::Abrir);
    crate::app::dinamica_ui::campos_de_antes(&mut app);
    eprintln!("Abrir: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    app.evento_dinamica(Evento::Criar);
    eprintln!("Criar: {:?} · status: {}", t.elapsed(), app.status);
    let nomes = app.dinamica.as_ref().unwrap().nomes.clone();
    eprintln!("campos: {nomes:?}");
    // Segundo campo em Linhas (clique na lista).
    let t = std::time::Instant::now();
    app.evento_dinamica(Evento::Campo(1));
    eprintln!("Campo(1): {:?}", t.elapsed());
    // Arrasta o primeiro de Linhas para Colunas.
    let t = std::time::Instant::now();
    arrastar(&mut app, (AreaDinamica::Linhas, 0), Some((AreaDinamica::Colunas, None)));
    eprintln!("Arrastar para Colunas: {:?}", t.elapsed());
    let d = destino(&app);
    app.dinamica = None;
    app.folha = d;
    let s = &app.workbook.aba(d).unwrap().sheet;
    let area = s.used_rect();
    let (r, c) = (area.r0 + 2, area.c1);
    let t = std::time::Instant::now();
    let tratado = app.detalhar_dinamica(r, c);
    eprintln!("Detalhar ({r},{c}) = {tratado}: {:?} · {}", t.elapsed(), app.status);
}

#[test]
fn aba_da_faixa_aparece_na_dinamica_e_muda_sem_o_painel() {
    let mut app = app_com_dinamica();
    let d = destino(&app);
    let _ = app.evento_dinamica(Evento::Fechar);
    app.folha = d;
    app.trocar_selecao(1, 0);
    assert!(app.dinamica_na_faixa(), "célula na dinâmica: a aba aparece");
    let subtotais = |app: &App| {
        let dn = app.workbook.aba(d).unwrap().sheet.dn.clone().unwrap();
        serde_json::from_str::<Vec<serde_json::Value>>(&dn).unwrap()[0]["subtotais"].as_bool()
    };
    let antes = subtotais(&app);
    let _ = app.evento_dinamica(Evento::NaFaixa(Box::new(Evento::Subtotais)));
    assert!(app.dinamica.is_none(), "o painel continua fechado");
    assert_ne!(subtotais(&app), antes, "a opção foi gravada na dinâmica");
    assert_eq!(app.grupos_faixa_dinamica().len(), 6);
    // Fora da dinâmica a aba some.
    app.trocar_selecao(40, 10);
    assert!(!app.dinamica_na_faixa());
}

#[test]
fn filtro_de_relatorio_aparece_na_grade_e_abre_a_lista() {
    let mut app = app_com_dinamica();
    let d = destino(&app);
    // Produto (coluna 1) para Filtros, arrastando da lista.
    app.evento_dinamica(Evento::ArrastoInicio(1, None));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(0.0, 0.0)));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(40.0, 40.0)));
    app.evento_dinamica(Evento::ArrastoSobre(Some(AreaDinamica::Filtros)));
    app.evento_dinamica(Evento::ArrastoSoltar);
    assert_eq!(app.dinamica.as_ref().unwrap().filtros.len(), 1);
    let dn = app.workbook.aba(d).unwrap().sheet.dn.clone().unwrap();
    let meta = &serde_json::from_str::<Vec<serde_json::Value>>(&dn).unwrap()[0];
    let af = retangulo_json(&meta["areaFiltros"]).expect("área dos filtros gravada");
    let area = retangulo_json(&meta["areaResultado"]).unwrap();
    assert!(af.r1 + 1 < area.r0, "uma linha em branco entre o filtro e a tabela");
    let s = &app.workbook.aba(d).unwrap().sheet;
    assert_eq!((s.get_raw(af.r0, af.c0), s.get_raw(af.r0, af.c0 + 1)), ("Produto".into(), "(Tudo)".into()));
    // O funil na grade abre a lista de itens do campo.
    let _ = app.evento_dinamica(Evento::Fechar);
    app.folha = d;
    let _ = app.evento_dinamica(Evento::FiltroNaGrade(af.r0, af.c0 + 1, 100.0, 40.0));
    assert_eq!(app.dinamica.as_ref().and_then(|e| e.campo_filtro), Some(1));
    // Tirar o filtro limpa as células de cima.
    let col = app.dinamica.as_ref().unwrap().filtros[0].coluna;
    app.evento_dinamica(Evento::RemoverFiltro(col));
    let s = &app.workbook.aba(d).unwrap().sheet;
    assert!(s.get_raw(af.r0, af.c0).is_empty());
}

/// Põe o campo `col` em Filtros arrastando da lista.
fn para_filtros(app: &mut App, col: usize) {
    app.evento_dinamica(Evento::ArrastoInicio(col, None));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(0.0, 0.0)));
    app.evento_dinamica(Evento::ArrastoMover(pelicano::Point::new(40.0, 40.0)));
    app.evento_dinamica(Evento::ArrastoSobre(Some(AreaDinamica::Filtros)));
    app.evento_dinamica(Evento::ArrastoSoltar);
}

fn area_dos_filtros(app: &App, d: usize) -> Option<Rect> {
    let dn = app.workbook.aba(d).unwrap().sheet.dn.clone().unwrap();
    retangulo_json(&serde_json::from_str::<Vec<serde_json::Value>>(&dn).unwrap()[0]["areaFiltros"])
}

/// #106: STATUS em Filtros e, marcado na lista, também em Linhas: as células
/// do filtro (nome e "(Tudo)") continuam em cima da tabela.
#[test]
fn campo_em_filtros_e_em_linhas_mantem_o_filtro_em_cima() {
    let mut app = app_com_dinamica();
    let d = destino(&app);
    para_filtros(&mut app, 1);
    app.evento_dinamica(Evento::Campo(1));
    let e = app.dinamica.as_ref().unwrap();
    assert!(e.linhas.contains(&1) && e.filtros.iter().any(|f| f.coluna == 1), "Produto nas duas áreas");
    let af = area_dos_filtros(&app, d).expect("o filtro de relatório continua");
    let s = &app.workbook.aba(d).unwrap().sheet;
    assert_eq!((s.get_raw(af.r0, af.c0), s.get_raw(af.r0, af.c0 + 1)), ("Produto".into(), "(Tudo)".into()));
    // Reaberto do arquivo, continua igual.
    let _ = app.evento_dinamica(Evento::Fechar);
    app.folha = d;
    app.trocar_selecao(af.r1 + 2, af.c0);
    app.abrir_edicao_dinamica();
    assert_eq!(app.dinamica.as_ref().unwrap().filtros_de_relatorio().len(), 1);
    // Desmarcado na lista, sai das linhas e fica só o filtro.
    app.evento_dinamica(Evento::Campo(1));
    assert!(area_dos_filtros(&app, d).is_some());
    assert!(app.dinamica.as_ref().unwrap().pagina_tambem.is_empty());
}

/// #106: o funil da grade abre a lista embaixo dele e não mexe na seleção.
#[test]
fn filtrar_pelo_funil_nao_tira_a_selecao_do_lugar() {
    let mut app = app_com_dinamica();
    let d = destino(&app);
    para_filtros(&mut app, 1);
    let af = area_dos_filtros(&app, d).unwrap();
    let _ = app.evento_dinamica(Evento::Fechar);
    app.folha = d;
    app.trocar_selecao(30, 6);
    let _ = app.evento_dinamica(Evento::FiltroNaGrade(af.r0, af.c0 + 1, 120.0, 44.0));
    let e = app.dinamica.as_ref().unwrap();
    assert_eq!(e.campo_filtro, Some(1));
    assert_eq!(e.ui.filtro_ancora, Some((120.0, 44.0)), "a lista abre embaixo do funil");
    assert_eq!((app.linha, app.coluna, app.fim_selecao), (30, 6, (30, 6)));
    app.evento_dinamica(Evento::FiltroValor("A".into()));
    let _ = app.evento_dinamica(Evento::FiltroOk);
    assert_eq!((app.linha, app.coluna), (30, 6), "filtrar não leva o foco para a tabela");
    let s = &app.workbook.aba(d).unwrap().sheet;
    assert_eq!(s.get_raw(af.r0, af.c0 + 1), "(Vários itens)", "o filtro valeu");
    // Aberta pelo painel, a lista volta para o lado dele.
    app.evento_dinamica(Evento::FiltroCampo(0));
    assert_eq!(app.dinamica.as_ref().unwrap().ui.filtro_ancora, None);
}

/// #107: a setinha do campo tem "Mover para ›" com submenu, e "Resumir por ›" nos valores.
#[test]
fn menu_da_setinha_agrupa_mover_para_num_submenu() {
    use crate::app::menu_contexto::ItemMenu;
    let mut app = app_com_dinamica();
    let e = app.dinamica.as_ref().unwrap();
    let rotulos = |itens: &[ItemMenu]| itens.iter().filter_map(|i| match i {
        ItemMenu::Acao { rotulo, msg, .. } => Some(format!("{rotulo}{}", if msg.is_some() { "" } else { " (off)" })),
        ItemMenu::Sub { rotulo, .. } => Some(format!("{rotulo} ›")),
        ItemMenu::Separador => None,
    }).collect::<Vec<_>>();
    let menu = App::menu_do_item(e, AreaDinamica::Linhas, 0);
    assert_eq!(rotulos(&menu), ["Mover para ›", "Classificar e agrupar…", "Remover campo"]);
    let ItemMenu::Sub { itens, .. } = &menu[0] else { panic!("submenu") };
    assert_eq!(rotulos(itens), ["Para cima (off)", "Para baixo (off)", "Filtros", "Colunas", "Valores"]);
    let menu = App::menu_do_item(e, AreaDinamica::Valores, 0);
    let ItemMenu::Sub { rotulo, itens, .. } = &menu[1] else { panic!("Resumir por") };
    assert_eq!(rotulo, "Resumir por");
    assert_eq!(itens.len(), 7);
    // Escolher no submenu move o campo e fecha o menu.
    app.evento_dinamica(Evento::MenuItem(Some((AreaDinamica::Linhas, 0))));
    app.evento_dinamica(Evento::MenuSub(Some(0)));
    assert_eq!(app.dinamica.as_ref().unwrap().ui.submenu, Some(0));
    app.evento_dinamica(Evento::MoverPara(AreaDinamica::Linhas, 0, AreaDinamica::Colunas));
    let e = app.dinamica.as_ref().unwrap();
    assert!(e.linhas.is_empty() && e.colunas.len() == 1);
    assert_eq!((e.ui.menu_item, e.ui.submenu), (None, None));
}

/// Quebra o tempo de uma troca de campo por etapa (arquivo pesado).
#[test]
#[ignore]
fn medir_etapas_da_troca() {
    let Ok(caminho) = std::env::var("FEATHERSHEET_PESADO") else { return };
    let lida = feathersheet_io::fshz_read::ler_fshz(&std::fs::read(&caminho).unwrap()).unwrap();
    let mut app = App::default();
    app.workbook = feathersheet_io::para_workbook::para_workbook(&lida);
    for i in 0..app.workbook.len() {
        let u = app.workbook.aba(i).unwrap().sheet.used_rect();
        eprintln!("aba {i} {:?}: {} × {}", app.workbook.nomes()[i], u.r1 + 1, u.c1 + 1);
    }
    let folha = std::env::var("FEATHERSHEET_ABA").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    app.folha = folha;
    let u = app.workbook.aba(folha).unwrap().sheet.used_rect();
    app.fim_selecao = (u.r1, u.c1);
    app.evento_dinamica(Evento::Abrir);
    crate::app::dinamica_ui::campos_de_antes(&mut app);
    let cab: Vec<String> = (0..=u.c1).map(|c| texto_celula(&app.workbook.valor(folha, 0, c))).collect();
    eprintln!("campos: {cab:?}");
    // FEATHERSHEET_CAMPOS="linhas;valores" (colunas da origem, por vírgula):
    // o caso típico; sem ela, o de sempre (o 2º campo em Linhas).
    let lista = |t: &str| -> Vec<usize> { t.split(',').filter_map(|x| x.trim().parse().ok()).collect() };
    let campos = std::env::var("FEATHERSHEET_CAMPOS").ok();
    if let (Some(campos), Some(e)) = (&campos, app.dinamica.as_mut()) {
        let (l, v) = campos.split_once(';').unwrap_or((campos, ""));
        e.linhas = lista(l).into_iter().take(1).collect();
        e.colunas.clear();
        e.valores = lista(v).into_iter().map(|coluna| MedidaDinamica { coluna, op: AgregacaoOp::Soma }).collect();
    }
    let t = std::time::Instant::now();
    app.evento_dinamica(Evento::Criar);
    eprintln!("Criar: {:?}", t.elapsed());
    let id = app.dinamica.as_ref().and_then(|e| e.editando.clone()).unwrap_or_default();
    let d = (0..app.workbook.len()).rev().find(|&i| app.workbook.aba(i)
        .and_then(|a| a.sheet.dn.as_deref()).is_some_and(|t| t.contains(&id))).unwrap();
    app.folha = d;
    let mut e = app.dinamica.clone().unwrap();
    match &campos {
        // A troca: o 2º campo de Linhas entra.
        Some(campos) => e.linhas = lista(campos.split(';').next().unwrap_or("")),
        None => e.linhas.push(e.origem.c0 as usize + 1),
    }
    // FEATHERSHEET_COMPACTO=1: o caminho completo (árvore de grupos).
    if std::env::var("FEATHERSHEET_COMPACTO").is_ok() {
        e.layout = Layout::Compacto;
        e.subtotais = true;
    }
    // Só a leitura das células que a conta usa (o piso do cálculo).
    let t = std::time::Instant::now();
    let mut n = 0usize;
    for r in e.origem.r0 + 1..=e.origem.r1 {
        for &c in e.linhas.iter().chain(e.valores.iter().map(|m| &m.coluna)) {
            if !matches!(app.workbook.valor(e.folha_origem, r, c as u32), Value::Empty) { n += 1; }
        }
    }
    eprintln!("só ler as células: {:?} ({n})", t.elapsed());
    let t = std::time::Instant::now();
    let r = avancado::calcular(&app, &e);
    eprintln!("calcular: {:?} ({})", t.elapsed(), r.is_ok());
    let t = std::time::Instant::now();
    let r = app.aplicar_estado_na_dinamica(d, &e);
    eprintln!("aplicar_estado_na_dinamica: {:?} ({r:?}) · {}", t.elapsed(), app.status);
    let versao = |app: &App| app.workbook.aba(e.folha_origem).map_or(0, |a| a.versao);
    eprintln!("versão da origem depois de aplicar: {}", versao(&app));
    let t = std::time::Instant::now();
    let meta = edicao::localizar_selecionada(&app).map(|x| x.2);
    eprintln!("localizar_selecionada: {:?}", t.elapsed());
    if let Ok(meta) = meta {
        let t = std::time::Instant::now();
        let lido = edicao::ler_estado(&app, d, &meta).unwrap();
        eprintln!("ler_estado: {:?}", t.elapsed());
        let (a, b) = (avancado::impressao_digital(&app, &e), avancado::impressao_digital(&app, &lido));
        if a != b { eprintln!("IMPRESSÃO DIFERE:\n painel: {a}\n lido:   {b}"); }
    }
    let t = std::time::Instant::now();
    let _ = app.campo_numerico(e.origem.c0 as usize + 2);
    eprintln!("campo_numerico: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = app.update(Msg::Mover(1, 0));
    eprintln!("um update qualquer: {:?}", t.elapsed());
    eprintln!("versão da origem antes do duplo clique: {}", versao(&app));
    // Duplo clique num valor (Detalhes).
    let area = edicao::localizar_selecionada(&app).ok()
        .and_then(|(_, _, m)| retangulo_json(&m["areaResultado"]));
    if let Some(area) = area {
        let t = std::time::Instant::now();
        let ok = app.detalhar_dinamica(area.r0 + 2, area.c1);
        eprintln!("Detalhar: {:?} ({ok}) · {}", t.elapsed(), app.status);
        // O segundo também pelo atalho (a aba de Detalhes sobe a versão).
        app.folha = d;
        let t = std::time::Instant::now();
        let ok = app.detalhar_dinamica(area.r0 + 1, area.c1);
        eprintln!("Detalhar de novo: {:?} ({ok}) · {}", t.elapsed(), app.status);
        // Sem o atalho (arquivo recém-aberto): a dinâmica é montada de novo.
        (app.folha, app.linha, app.coluna) = (d, area.r0, area.c0);
        if let Ok((_, _, meta)) = edicao::localizar_selecionada(&app) {
            let lido = edicao::ler_estado(&app, d, &meta).unwrap();
            let t = std::time::Instant::now();
            let r = avancado::detalhar(&app, &lido, 2, area.c1 as usize - area.c0 as usize);
            eprintln!("Detalhar sem atalho: {:?} ({:?} linhas)", t.elapsed(), r.map(|x| x.map(|(_, l)| l.len())));
        }
    }
}

/// O travamento de 28/10/2026: escrever só a linha 2 das colunas de bloco no
/// `2026_03.fshz` deixou o app a 710% de CPU. Mede cada pulso que o app roda
/// depois de uma escrita.
#[test]
#[ignore]
fn medir_pulsos_depois_de_escrever() {
    let Ok(caminho) = std::env::var("FEATHERSHEET_PESADO") else { return };
    let rss = || std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines()
        .find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())).unwrap_or(0) / 1024;
    let t = std::time::Instant::now();
    let lida = feathersheet_io::fshz_read::ler_fshz(&std::fs::read(&caminho).unwrap()).unwrap();
    let mut app = App::default();
    app.workbook = feathersheet_io::para_workbook::para_workbook(&lida);
    drop(lida);
    app.em_tela_inicio = false;
    eprintln!("abrir: {:?} · {} MB", t.elapsed(), rss());
    let linha2 = ["=AP2&\"|\"&A2&\"|\"&C2", "=MÍNIMOSES(AO:AO;AU:AU;AU2)", "=MÁXIMOSES(AO:AO;AU:AU;AU2)",
        "=(MÁXIMO(TETO.MAT(AW2;200);ARREDMULTB.MAT(AV2;200)+200)-ARREDMULTB.MAT(AV2;200))/200",
        "=MÁXIMO(TETO.MAT(AO2;200);ARREDMULTB.MAT(AV2;200)+200)", "=(AY2-ARREDMULTB.MAT(AV2;200))/200",
        "=MÁXIMO(AV2;AY2-200)&\" - \"&MÍNIMO(AW2;AY2)"];
    for (i, f) in linha2.iter().enumerate() {
        let t = std::time::Instant::now();
        app.workbook.escrever_celula(0, 1, 46 + i as u32, f.to_string());
        eprintln!("escrever {}: {:?} · {} MB", 46 + i, t.elapsed(), rss());
    }
    app.alterado = true;
    app.linha = 1; app.coluna = 46; app.fim_selecao = (1, 46);
    app.coluna_inicio = 40;
    for (nome, msg) in [("calcular visíveis", Msg::CalcularPulso), ("monitor", Msg::MonitorPulso),
        ("resumo", Msg::ResumoPulso), ("calcular visíveis 2", Msg::CalcularPulso), ("monitor 2", Msg::MonitorPulso)] {
        let t = std::time::Instant::now();
        let _ = app.update(msg);
        eprintln!("{nome}: {:?} · {} MB", t.elapsed(), rss());
    }
    for c in 46..53 { eprintln!("  {}: {:?}", c, app.workbook.valor(0, 1, c)); }
}

/// Quantas chaves NOMENCLATURA|RAMAL|LINHA distintas o arquivo tem (o índice do
/// MÍNIMOSES desiste acima de `MAX_GRUPOS`).
#[test]
#[ignore]
fn contar_chaves_do_bloco() {
    let Ok(caminho) = std::env::var("FEATHERSHEET_PESADO") else { return };
    let lida = feathersheet_io::fshz_read::ler_fshz(&std::fs::read(&caminho).unwrap()).unwrap();
    let wb = feathersheet_io::para_workbook::para_workbook(&lida);
    drop(lida);
    let n = wb.aba(0).unwrap().sheet.used_rect().r1;
    let mut chaves = std::collections::HashSet::new();
    for r in 1..=n {
        chaves.insert((texto_celula(&wb.valor(0, r, 41)), texto_celula(&wb.valor(0, r, 0)), texto_celula(&wb.valor(0, r, 2))));
    }
    eprintln!("{} chaves distintas em {n} linhas", chaves.len());
}

/// Monta a dinâmica de blocos de 200 m pedida em 28/10/2026 sobre um `.fshz`
/// e grava numa cópia (`FEATHERSHEET_SAIDA`): colunas de bloco por
/// NOMENCLATURA|RAMAL|LINHA sobre o _KM (AO), e a dinâmica DATA, RAMAL, EH,
/// LINHA, BLOCO × Máx de NIV_MAX_5M.
#[test]
#[ignore]
fn montar_dinamica_de_blocos() {
    let (Ok(caminho), Ok(saida)) = (std::env::var("FEATHERSHEET_PESADO"), std::env::var("FEATHERSHEET_SAIDA")) else { return };
    let t = std::time::Instant::now();
    let lida = feathersheet_io::fshz_read::ler_fshz(&std::fs::read(&caminho).unwrap()).unwrap();
    let mut app = App::default();
    app.workbook = feathersheet_io::para_workbook::para_workbook(&lida);
    drop(lida);
    eprintln!("abrir: {:?}", t.elapsed());
    let u = app.workbook.aba(0).unwrap().sheet.used_rect();
    assert_eq!(u.c1, 45, "a origem mudou de forma");
    let colunas = [
        ("CHAVE", "=AP2&\"|\"&A2&\"|\"&C2"),
        ("KM_MIN", "=MÍNIMOSES(AO:AO;AU:AU;AU2)"),
        ("KM_MAX", "=MÁXIMOSES(AO:AO;AU:AU;AU2)"),
        ("QTD_BLOCOS", "=(MÁXIMO(TETO.MAT(AW2;200);ARREDMULTB.MAT(AV2;200)+200)-ARREDMULTB.MAT(AV2;200))/200"),
        ("TETO_BLOCO", "=MÁXIMO(TETO.MAT(AO2;200);ARREDMULTB.MAT(AV2;200)+200)"),
        ("BLOCO_N", "=(AY2-ARREDMULTB.MAT(AV2;200))/200"),
        ("BLOCO", "=MÁXIMO(AV2;AY2-200)&\" - \"&MÍNIMO(AW2;AY2)"),
    ];
    let t = std::time::Instant::now();
    for (i, (titulo, formula)) in colunas.iter().enumerate() {
        let c = 46 + i as u32;
        app.workbook.escrever_celula(0, 0, c, titulo.to_string());
        app.workbook.escrever_celula(0, 1, c, formula.to_string());
        let sheet = &mut app.workbook.aba_mut(0).unwrap().sheet;
        let primeira = feathersheet_core::operacoes::fill::shift_formula(formula, 1, 0);
        assert!(sheet.formula_cabe_em_bloco(c, 2, &primeira), "não cabe no bloco: {formula}");
        sheet.preencher_formula_faixa_vazia(c, 2, u.r1, &primeira);
    }
    app.workbook.calcular_em_paralelo();
    eprintln!("colunas do bloco: {:?}", t.elapsed());
    for r in [1u32, 2, 1000, u.r1] {
        let linha: Vec<String> = (46..53).map(|c| texto_celula(&app.workbook.valor(0, r, c))).collect();
        eprintln!("linha {}: _KM {} {linha:?}", r + 1, texto_celula(&app.workbook.valor(0, r, 40)));
    }
    app.folha = 0;
    app.fim_selecao = (u.r1, 52);
    app.evento_dinamica(Evento::Abrir);
    let e = app.dinamica.as_mut().unwrap();
    e.linhas = vec![4, 0, 44, 2, 52];
    e.colunas.clear();
    e.valores = vec![MedidaDinamica { coluna: 19, op: AgregacaoOp::Max }];
    let t = std::time::Instant::now();
    app.evento_dinamica(Evento::Criar);
    eprintln!("criar a dinâmica: {:?} · {}", t.elapsed(), app.status);
    let d = app.workbook.len() - 1;
    let ud = app.workbook.aba(d).unwrap().sheet.used_rect();
    eprintln!("aba {:?}: {} linhas × {}", app.workbook.nomes()[d], ud.r1 + 1, ud.c1 + 1);
    for r in 0..8u32.min(ud.r1 + 1) {
        let linha: Vec<String> = (0..=ud.c1).map(|c| texto_celula(&app.workbook.valor(d, r, c))).collect();
        eprintln!("  {linha:?}");
    }
    let t = std::time::Instant::now();
    let pasta = feathersheet_io::da_workbook::da_workbook(&app.workbook, d as u32);
    feathersheet_io::arquivo::gravar_atomico_com(std::path::Path::new(&saida), |arquivo| {
        feathersheet_io::fshz_write::gravar_fshz_para(arquivo, &pasta).map_err(std::io::Error::other)
    }).unwrap();
    eprintln!("gravar {saida}: {:?}", t.elapsed());
}
