//! Testes da grade Canvas, preservados do módulo original.
use super::*;
use std::cell::RefCell;
use feathersheet_core::{celulas::refs::Rect, planilha::workbook::Workbook};
use crate::condicional::Estado as EstadoCondicional;

/// Cache de eixos novo para cada grade de teste (vive até o fim do processo).
fn eixos_teste() -> &'static RefCell<EstadoEixos> {
    Box::leak(Box::new(RefCell::new(EstadoEixos::default())))
}

    #[test]
    fn modo_mostrar_formulas_exibe_texto_bruto_sem_modificar_avaliacao() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "7".into());
        wb.escrever_celula(0, 0, 1, "=A1*2".into());
        wb.escrever_celula(0, 0, 2, "Texto".into());
        let condicional = RefCell::new(EstadoCondicional::default());
        let mut grade = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
            workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
            linha_inicio: 0, coluna_inicio: 0,
            selecionada: (0, 0), area: Rect::cell(0, 0),
            escuro: false, cabecalho_escuro: false, linhas_grade: true, realce_eixos: false,
            mostrar_formulas: false, zoom: 1.0, ctrl_pressionado: false,
            rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
        };
        // O desenho não calcula: até o app calcular, a fórmula mostra "…".
        assert_eq!(grade.texto(0, 1), "…");
        let _ = wb.valor(0, 0, 1);
        assert_eq!(grade.texto(0, 1), "14");
        assert_eq!(grade.alinhar(0, 1), pelicano::Align::End);
        grade.mostrar_formulas = true;
        assert_eq!(grade.texto(0, 1), "=A1*2");
        assert_eq!(grade.texto(0, 0), "7");
        assert_eq!(grade.texto(0, 2), "Texto");
        assert_eq!(grade.alinhar(0, 1), pelicano::Align::Start);
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 1), "=A1*2");
        assert_eq!(wb.valor(0, 0, 1), Value::Number(14.0));
        grade.mostrar_formulas = false;
        assert_eq!(grade.texto(0, 1), "14");
    }

    #[test]
    fn rascunho_de_formula_continua_prioritario_em_ambos_os_modos() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "=2+3".into());
        let condicional = RefCell::new(EstadoCondicional::default());
        let mut grade = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
            workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
            linha_inicio: 0, coluna_inicio: 0,
            selecionada: (0, 0), area: Rect::cell(0, 0),
            escuro: false, cabecalho_escuro: false, linhas_grade: true, realce_eixos: false,
            mostrar_formulas: false, zoom: 1.0, ctrl_pressionado: false,
            rascunho: Some("=2+7"), refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
        };
        assert_eq!(grade.texto(0, 0), "=2+7");
        grade.mostrar_formulas = true;
        assert_eq!(grade.texto(0, 0), "=2+7");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 0), "=2+3");
    }

    #[test]
    fn ctrl_roda_no_centro_da_celula_produz_zoom_sem_rolagem() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let mut grade = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
            workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
            linha_inicio: 0, coluna_inicio: 0,
            selecionada: (0, 0), area: Rect::cell(0, 0),
            escuro: false, cabecalho_escuro: false, linhas_grade: true, realce_eixos: false, mostrar_formulas: false,
            zoom: 1.0, ctrl_pressionado: true,
            rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
        };
        let area = Rectangle { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
        let cursor = Some(Point::new(151.0, 44.0));
        let roda = |y: f32| Event::Wheel { dx: 0.0, dy: y * 48.0, modifiers: Default::default() };
        let mut estado = super::EstadoMouse::default();
        let (status, evento) = grade.atualizar(&mut estado, &roda(1.0), area, cursor);
        assert_eq!(status, Captured::Yes);
        assert!(matches!(evento, Some(Msg::Roda(1))));
        let (_, evento) = grade.atualizar(&mut estado, &roda(-1.0), area, cursor);
        assert!(matches!(evento, Some(Msg::Roda(-1))));
        grade.ctrl_pressionado = false;
        let (_, evento) = grade.atualizar(&mut estado, &roda(-1.0), area, cursor);
        assert!(matches!(evento, Some(Msg::RolarNaGrade(_, _, _, _))));
    }

    #[test]
    fn arrastar_divisoria_do_cabecalho_muda_a_largura_com_zoom() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let grade = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
            workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
            linha_inicio: 0, coluna_inicio: 0,
            selecionada: (0, 0), area: Rect::cell(0, 0),
            escuro: false, cabecalho_escuro: false, linhas_grade: true, realce_eixos: false, mostrar_formulas: false,
            zoom: 2.0, ctrl_pressionado: false,
            rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
        };
        let area = Rectangle { x: 0.0, y: 0.0, width: 1600.0, height: 1000.0 };
        // Divisória A|B: 100 + 122 = 222 na grade, 444 na tela com zoom 2.
        let em = |x: f32| Some(Point::new(x, 20.0));
        let apertar = Event::MousePressed(pelicano::MouseButton::Left);
        let soltar = Event::MouseReleased(pelicano::MouseButton::Left);
        let mover = |x: f32| Event::CursorMoved(Point::new(x, 20.0));
        assert_eq!(pelicano::Program::<Msg>::mouse_interaction(&grade, &super::EstadoMouse::default(), area, em(445.0)),
            pelicano::CursorIcon::EwResize);
        let mut estado = super::EstadoMouse::default();
        let (status, msg) = grade.atualizar(&mut estado, &apertar, area, em(445.0));
        assert_eq!(status, Captured::Yes);
        assert!(msg.is_none(), "o primeiro clique não seleciona a coluna");
        grade.atualizar(&mut estado, &mover(544.0), area, em(544.0));
        let (_, msg) = grade.atualizar(&mut estado, &soltar, area, em(544.0));
        assert!(matches!(msg, Some(Msg::DimensaoArrastada(true, 0, 172))), "{msg:?}");
        // Clique sem arrastar não grava (e segue valendo o duplo clique).
        let mut estado = super::EstadoMouse::default();
        grade.atualizar(&mut estado, &apertar, area, em(444.0));
        let (_, msg) = grade.atualizar(&mut estado, &soltar, area, em(444.0));
        assert!(msg.is_none());
    }

    #[test]
    fn divisor_do_cabecalho_respeita_zoom_cortes_e_ocultacao() {
        for zoom in [0.5, 1.0, 2.0, 3.0] {
            let esp = 1.0 / zoom;
            // A primeira coluna de 122 px começa depois dos 90 px de cabeçalho.
            assert_eq!(divisoria_visivel(90.0, 122.0, 800.0, esp), Some(212.0 - esp));
            assert!(divisoria_visivel(90.0, 0.0, 800.0, esp).is_none());
            // A coluna seguinte existe, mas está parcialmente cortada.
            assert!(divisoria_visivel(212.0, 122.0, 250.0, esp).is_none());
            // A primeira linha de 28 px começa depois do cabeçalho de 30 px.
            assert_eq!(divisoria_visivel(30.0, 28.0, 580.0, esp), Some(58.0 - esp));
            assert!(divisoria_visivel(58.0, 28.0, 70.0, esp).is_none());
        }
    }

    #[test]
    fn divisorias_sao_independentes_do_estado_linhas_grade() {
        // Mesmo desabilitando linhas internas, a geometria do cabeçalho
        // continua sendo produzida. A pintura acontece fora do if linhas_grade.
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        for linhas_grade in [true, false] {
            let grade = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
                workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
                linha_inicio: 0, coluna_inicio: 0,
                selecionada: (11, 0), area: Rect::cell(11, 0),
                escuro: false, cabecalho_escuro: false, linhas_grade, realce_eixos: true, mostrar_formulas: false,
                zoom: 1.0, ctrl_pressionado: false, rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
            };
            let largura = largura(grade.workbook, grade.aba, 0);
            let altura = altura(grade.workbook, grade.aba, 0);
            assert_eq!(divisoria_visivel(CABECALHO_LINHA, largura, 800.0, 1.0), Some(221.0));
            assert_eq!(divisoria_visivel(CABECALHO_COLUNA, altura, 580.0, 1.0), Some(57.0));
            assert_eq!(grade.selecionada, (11, 0));
        }
    }
    #[test]
    fn cruz_da_celula_d4_destaca_linha_4_e_coluna_d() {
        // Indices base zero: D4 = (3, 3).
        let ativa = (3, 3);
        assert_eq!(realce_cruz((3, 3), ativa, true), RealceCruz::Cruzamento);
        assert_eq!(realce_cruz((3, 0), ativa, true), RealceCruz::Eixo);
        assert_eq!(realce_cruz((0, 3), ativa, true), RealceCruz::Eixo);
        assert_eq!(realce_cruz((4, 4), ativa, true), RealceCruz::Nenhum);
    }

    #[test]
    fn cruz_move_com_a_celula_ativa_e_desliga_sem_editar_planilha() {
        let antes = (3, 3);
        let depois = (8, 5); // F9
        assert_eq!(realce_cruz((3, 6), antes, true), RealceCruz::Eixo);
        assert_eq!(realce_cruz((3, 6), depois, true), RealceCruz::Nenhum);
        assert_eq!(realce_cruz((8, 6), depois, true), RealceCruz::Eixo);
        assert_eq!(realce_cruz((8, 5), depois, true), RealceCruz::Cruzamento);
        assert_eq!(realce_cruz((8, 5), depois, false), RealceCruz::Nenhum);
        assert_eq!(realce_cruz((8, 0), depois, false), RealceCruz::Nenhum);
    }

    #[test]
    fn parse_de_cor_para_estilos_do_motor() {
        let azul=cor_hex("#0b57d0").unwrap();
        assert_eq!(azul,Color::rgb8(11,87,208));
        assert!(cor_hex("invalida").is_none());
    }
    #[test]
    fn zoom_e_eixos_ocultos_mantem_clique_na_celula_visivel() {
        let mut wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        wb.aba_mut(0).unwrap().sheet.row_heights.insert(0,0);
        wb.aba_mut(0).unwrap().sheet.col_widths.insert(0,0);
        let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[], workbook:&wb, condicional:&condicional, eixos: eixos_teste(), aba:0,linha_inicio:0,coluna_inicio:0,
            selecionada:(1,1),area:Rect::cell(1,1),escuro:false, cabecalho_escuro: false,linhas_grade:true,realce_eixos:true, mostrar_formulas: false,zoom: 2.0, ctrl_pressionado: false, rascunho:None,refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0 };
        assert_eq!(g.celula_no_ponto(Point::new(200.,72.),Size::new(800.,500.)),Some((1,1)));
        assert_eq!(g.celula_no_ponto(Point::new(100.,72.),Size::new(800.,500.)),None);
    }
    #[test]
    fn ponto_da_grade_respeita_deslocamento_e_largura() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
            workbook:&wb, condicional:&condicional, eixos: eixos_teste(), aba:0,linha_inicio:200,coluna_inicio:26,
            selecionada:(200,26),area:Rect::cell(200,26),escuro:false, cabecalho_escuro: false,linhas_grade:true,realce_eixos:true, mostrar_formulas: false,zoom: 1.0, ctrl_pressionado: false, rascunho:None,refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0
        };
        assert_eq!(g.celula_no_ponto(Point::new(105.0,36.0),Size::new(800.0,500.0)),Some((200,26)));
        assert_eq!(g.celula_no_ponto(Point::new(230.0,65.0),Size::new(800.0,500.0)),Some((201,27)));
        assert_eq!(g.celula_no_ponto(Point::new(20.0,45.0),Size::new(800.0,500.0)),None);
    }

    #[test]
    fn autofiltro_claro_nao_desenha_texto_ou_funil_branco_sobre_fundo_claro() {
        let normal = cor_contraste(None, false);
        assert_eq!(normal, Color::rgb8(30, 35, 40));
        assert_eq!(cor_contraste(Some(Color::WHITE), false), normal);
        assert_eq!(cor_contraste(None, true), Color::WHITE);
        // Tabela com cabeçalho azul continua com texto/funil brancos.
        assert_eq!(cor_contraste(Some(Color::rgb8(26, 115, 232)), false), Color::WHITE);
        // Fundo personalizado claro tem prioridade, mesmo quando é uma tabela.
        assert_eq!(
            cor_contraste(Some(Color::rgb8(235, 240, 248)), false),
            normal,
        );
    }

    #[test]
    fn cabecalho_reserva_largura_para_nove_digitos() {
        assert!(CABECALHO_LINHA >= 96.0);
        assert_eq!(MAX_ROWS, 100_000_000);
    }

    #[test]
    fn xfd_encosta_na_borda_direita() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let largura_tela = 1260.0;
        for zoom in [0.5, 1.0, 2.0] {
            let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
                workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0, linha_inicio: 0, coluna_inicio: MAX_COLS - 3,
                selecionada: (0, MAX_COLS - 1),
                area: Rect::cell(0, MAX_COLS - 1), escuro: false, cabecalho_escuro: false,
                linhas_grade: true, realce_eixos: true, mostrar_formulas: false, zoom, ctrl_pressionado: false, rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
            };
            let inicio = g.limite_direito_rolagem(largura_tela);
            assert!(inicio < MAX_COLS - 3, "deve mostrar mais colunas que XFB:XFD no zoom {zoom}");
            assert_eq!(g.primeira_coluna_visivel(largura_tela), inicio);
            let (colunas, _) = g.colunas_visiveis(largura_tela / zoom);
            let ultima = colunas.last().expect("há colunas");
            assert_eq!(ultima.0, MAX_COLS - 1, "XFD é a última desenhada no zoom {zoom}");
            let fim = (ultima.1 + ultima.2) * zoom;
            let barra = super::barras::ESPESSURA;
            assert!((fim - (largura_tela - barra)).abs() < 1.0, "XFD encosta na barra no zoom {zoom}: {fim}");
            assert_eq!(ultima.2, LARGURA_PADRAO, "XFD inteira no zoom {zoom}");
            // Antes de XFD a origem guardada vale como está.
            let antes = Grade { coluna_inicio: 10, ..g };
            assert_eq!(antes.primeira_coluna_visivel(largura_tela), 10);
        }
    }

    #[test]
    fn limite_inferior_10_milhoes_respeita_zoom_e_cliques() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let altura_tela = 590.0;
        for zoom in [0.5, 1.0, 2.0] {
            let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
                workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0, linha_inicio: MAX_ROWS - 1, coluna_inicio: 0,
                selecionada: (MAX_ROWS - 1, 0),
                area: Rect::cell(MAX_ROWS - 1, 0), escuro: false, cabecalho_escuro: false,
                linhas_grade: true, realce_eixos: true, mostrar_formulas: false, zoom, ctrl_pressionado: false, rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
            };
            let inicio = g.limite_inferior_rolagem(altura_tela);
            assert!(inicio < MAX_ROWS - 1, "deve mostrar outras linhas no zoom {zoom}");
            assert_eq!(g.primeira_linha_visivel(altura_tela), inicio);
            let ultima = MAX_ROWS - 1 - inicio;
            let centro_ultima = (CABECALHO_COLUNA
                + ultima as f32 * ALTURA_PADRAO + ALTURA_PADRAO / 2.0) * zoom;
            // A barra horizontal tem faixa própria: a última linha para acima dela.
            let util = altura_tela - super::barras::ESPESSURA;
            assert!(centro_ultima < util, "última linha visível com zoom {zoom}");
            assert!(
                centro_ultima >= util - ALTURA_PADRAO * zoom * 1.5,
                "não pode deixar um espaço vazio grande no rodapé com zoom {zoom}",
            );
            let x = (CABECALHO_LINHA + 10.0) * zoom;
            assert_eq!(
                g.celula_no_ponto(Point::new(x, centro_ultima), Size::new(800.0, altura_tela)),
                Some((MAX_ROWS - 1, 0)),
            );
            // Já no limite, a roda para baixo não passa da última linha (MAX_ROWS);
            // um giro para cima deve responder imediatamente.
            assert_eq!(g.origem_apos_rolar(altura_tela, 12), inicio);
            assert_eq!(g.origem_apos_rolar(altura_tela, -3), inicio - 3);
        }
    }

    #[test]
    fn ultimo_viewport_considera_linhas_com_altura_personalizada_e_ocultas() {
        let mut wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let sheet = &mut wb.aba_mut(0).unwrap().sheet;
        sheet.row_heights.insert(MAX_ROWS - 1, 55);
        sheet.row_heights.insert(MAX_ROWS - 2, 0);
        sheet.row_heights.insert(MAX_ROWS - 3, 70);
        for zoom in [0.5, 1.0, 2.0] {
            let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
                workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0, linha_inicio: MAX_ROWS - 1, coluna_inicio: 0,
                selecionada: (MAX_ROWS - 1, 0),
                area: Rect::cell(MAX_ROWS - 1, 0), escuro: false, cabecalho_escuro: false,
                linhas_grade: true, realce_eixos: true, mostrar_formulas: false, zoom, ctrl_pressionado: false, rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
            };
            let tela = Size::new(800.0, 580.0);
            let inicio = g.primeira_linha_visivel(tela.height);
            assert!(inicio < MAX_ROWS - 3);
            let mut y = CABECALHO_COLUNA;
            for linha in inicio..MAX_ROWS - 1 {
                y += altura(&wb, 0, linha);
            }
            let centro_ultima = (y + 55.0 / 2.0) * zoom;
            assert!(centro_ultima < tela.height);
            assert_eq!(
                g.celula_no_ponto(
                    Point::new((CABECALHO_LINHA + 8.0) * zoom, centro_ultima), tela,
                ),
                Some((MAX_ROWS - 1, 0)),
            );
            assert_eq!(g.origem_apos_rolar(tela.height, 100), inicio);
        }
    }

    /// Pé da grade (14/10, planilha de informações .xlsm): a barra de rolagem
    /// horizontal cobria a última linha, e a linha cortada desenhava o número
    /// centrado no pedaço visível (espremido sobre a linha de cima).
    #[test]
    fn ultima_linha_para_na_barra_e_texto_usa_a_altura_inteira() {
        let wb = Workbook::new();
        let condicional = RefCell::new(EstadoCondicional::default());
        let altura_tela = 590.0;
        let largura_tela = 900.0;
        for zoom in [0.5, 1.0, 2.0] {
            for linha_inicio in [19, MAX_ROWS - 1] {
                let g = Grade { cache: cache_teste(), impressao: None, copiada: None, setas: &[], comentarios: Default::default(), circulos_validacao: &[], opcoes_coluna: None, areas_extras: &[],
                    workbook: &wb, condicional: &condicional, eixos: eixos_teste(), aba: 0,
                    linha_inicio, coluna_inicio: 0,
                    selecionada: (0, 0), area: Rect::cell(0, 0), escuro: false, cabecalho_escuro: false,
                    linhas_grade: true, realce_eixos: true, mostrar_formulas: false, zoom, ctrl_pressionado: false, rascunho: None, refs_formula: Vec::new(), lista_aberta: false, lista_pagina: 0, lista_destaque: 0, fonte_padrao: "sans", tamanho_fonte_padrao: 13, congelar_linhas: 0, congelar_colunas: 0,
                };
                let barra = super::barras::ESPESSURA;
                let tela = Size::new(largura_tela / zoom, altura_tela / zoom);
                let (linhas, _) = g.linhas_visiveis(tela.height);
                let (colunas, _) = g.colunas_visiveis(tela.width);
                let fim_linhas = linhas.last().map(|f| (f.1 + f.2) * zoom).unwrap();
                let fim_colunas = colunas.last().map(|f| (f.1 + f.2) * zoom).unwrap();
                assert!(fim_linhas <= altura_tela - barra + 0.01,
                    "linhas acabam antes da barra (zoom {zoom}, origem {linha_inicio}): {fim_linhas}");
                assert!(fim_colunas <= largura_tela - barra + 0.01,
                    "colunas acabam antes da barra (zoom {zoom}): {fim_colunas}");
                if linha_inicio == MAX_ROWS - 1 {
                    // No fim da planilha a última linha encosta na barra, inteira.
                    let ultima = linhas.last().unwrap();
                    assert_eq!(ultima.0, MAX_ROWS - 1);
                    assert_eq!(ultima.2, ALTURA_PADRAO);
                }
                let (alturas, larguras) = g.extensoes_inteiras(&linhas, &colunas, tela);
                assert_eq!(alturas.len(), linhas.len());
                assert_eq!(larguras.len(), colunas.len());
                for (i, f) in linhas.iter().enumerate() {
                    // Só a cortada no fim muda: volta à altura inteira.
                    assert_eq!(alturas[i], ALTURA_PADRAO.max(f.2),
                        "linha {} (zoom {zoom}) com altura inteira", f.0);
                }
            }
        }
    }

/// Cache de desenho para as grades dos testes (vive até o fim do processo).
fn cache_teste() -> &'static super::CacheGrade {
    Box::leak(Box::new(super::CacheGrade::default()))
}
