//! Testes do editor de gráficos, preservados do módulo original.
use super::*;
use super::editor::svg_do_grafico_salvo;

    #[test]
    fn colorpicker_compartilhado_atualiza_series_e_fundo_sem_modificar_celulas() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar(
            "Grupo\tReceita\tCusto\nA\t12\t3\nB\t45\t11".into(),
        ));
        app.graficos = Estado {
            intervalo: "A1:C3".into(),
            titulo: "Cores".into(),
            tipo: TipoUi::Colunas,
            cabecalho: true,
            categorias: true,
            colunas: true,
            aba_id: app.workbook.aba(0).map(|a| a.id),
            ..Default::default()
        };
        app.atualizar_grafico();
        assert!(app.graficos.svg.is_some());

        let anterior = app.workbook.aba(0).unwrap().sheet.get_raw(1, 1).to_owned();
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Principal));
        assert_eq!(app.graficos.cor_ativa, Some(AlvoCorGrafico::Principal));
        {
            // A árvore Iced mantém empréstimos da App até ser descartada.
            let _painel: Element<'_, Msg> = app.view_graficos();
        }
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida("#ff1122".into()),
        ));
        assert_eq!(app.graficos.aparencia.cor, "#ff1122");
        assert!(app.graficos.svg.as_ref().unwrap().contains("#ff1122"));

        // Clicar em outro alvo alterna o mesmo painel e carrega sua cor.
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Fundo));
        assert_eq!(app.graficos.cor_hex, "#ffffff");
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida("#10203080".into()),
        ));
        assert_eq!(app.graficos.aparencia.fundo, "#10203080");
        assert!(app.graficos.svg.as_ref().unwrap().contains("fill-opacity=\"0.502\""));

        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Serie(1)));
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida("#3366aa80".into()),
        ));
        assert_eq!(app.graficos.aparencia.cores.get(&1).unwrap(), "#3366aa80");
        assert!(app.graficos.svg.as_ref().unwrap().contains("#3366aa"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 1), anterior);

        // Serialização real passa pelo contrato preexistente do arquivo FSHZ.
        let previa = app.graficos.svg.clone().unwrap();
        let _ = app.evento_grafico(Evento::Inserir);
        let aba = app.workbook.aba(0).unwrap();
        let gravados = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
        assert_eq!(gravados[0]["_featherNative"]["fundo"], "#10203080");
        assert_eq!(gravados[0]["_featherNative"]["cores"]["1"], "#3366aa80");
        assert_eq!(svg_do_grafico_salvo(&aba.sheet, &gravados[0]).unwrap(), previa);
    }

    #[test]
    fn colorpicker_hex_hsv_e_sem_cor_preservam_os_comandos_do_editor() {
        let mut app = App::default();
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Principal));
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::TextoHex("#00ff80".into()),
        ));
        assert_eq!(app.graficos.aparencia.cor, "#00ff80");
        let _ = app.evento_grafico(Evento::SeletorCor(EventoCor::Alfa(0.5)));
        assert!(app.graficos.aparencia.cor.ends_with("80"));

        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::TextoHex("errado".into()),
        ));
        // Hex incorreto fica editável, sem invalidar a cor persistida.
        assert_eq!(app.graficos.cor_hex, "errado");
        assert_eq!(app.graficos.aparencia.cor.len(), 9);

        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida(String::new()),
        ));
        assert_eq!(app.graficos.aparencia.cor, "#4285f4");
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Serie(2)));
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida("#112233".into()),
        ));
        assert_eq!(app.graficos.aparencia.cores.get(&2).unwrap(), "#112233");
        let _ = app.evento_grafico(Evento::SeletorCor(
            EventoCor::Escolhida(String::new()),
        ));
        assert!(!app.graficos.aparencia.cores.contains_key(&2));

        let _ = app.evento_grafico(Evento::SeletorCor(EventoCor::Fechar));
        assert!(app.graficos.cor_ativa.is_none());
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Fundo));
        let _ = app.evento_grafico(Evento::AbrirSeletorCor(AlvoCorGrafico::Fundo));
        assert!(app.graficos.cor_ativa.is_none());
    }

    #[test]
    fn grafico_aceita_rgb_e_rgba_sem_mudar_cores_opacas() {
        assert_eq!(cor_rgb("#4285f4"), Some(feather_draw::Cor::rgb(66, 133, 244)));
        assert_eq!(cor_rgb("#4285f480"),
            Some(feather_draw::Cor::rgba(66, 133, 244, 128)));
        assert_eq!(cor_rgb("#gggggg"), None);
        assert_eq!(cor_rgb("invalido"), None);
        assert_eq!(cor_rgb("#1234"), None);
    }

    #[test]
    fn app009_grafico_salvo_reproduz_previa_com_cores_e_nomes() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar(
            "Grupo\tReceita\tCusto\nA\t12\t3\nB\t45\t11".into(),
        ));
        app.graficos = Estado {
            intervalo: "A1:C3".into(),
            titulo: "Receita & Custo".into(),
            tipo: TipoUi::Colunas,
            cabecalho: true, categorias: true, colunas: true,
            aba_id: app.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        let _ = app.evento_grafico(Evento::Estilo(
            CampoEstilo::Cor, "#bd2455".into()
        ));
        let _ = app.evento_grafico(Evento::CorSerie(1, "#08a5b2".into()));
        let _ = app.evento_grafico(Evento::NomeSerie(0, "Valor realizado".into()));
        let _ = app.evento_grafico(Evento::NomeSerie(1, "Orçamento".into()));
        let _ = app.evento_grafico(Evento::Estilo(
            CampoEstilo::Subtitulo, "Ano 2026".into()
        ));
        let previa = app.graficos.svg.clone().unwrap();
        assert!(previa.contains("#bd2455"));
        assert!(previa.contains("#08a5b2"));
        assert!(previa.contains("Valor realizado"));
        assert!(previa.contains("Orçamento"));
        assert!(previa.contains("Receita &amp; Custo"));

        let celula_antes = app.workbook.aba(0).unwrap().sheet.get_raw(1, 1).to_owned();
        let _ = app.evento_grafico(Evento::Inserir);
        let aba = app.workbook.aba(0).unwrap();
        let graficos = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
        assert_eq!(graficos.len(), 1);
        assert!(graficos[0]["_featherNative"].is_object());
        assert_eq!(svg_do_grafico_salvo(&aba.sheet, &graficos[0]).unwrap(), previa);
        assert_eq!(aba.sheet.get_raw(1, 1), celula_antes);
    }

    #[test]
    fn app009_pizza_salva_respeita_serie_selecionada_e_cor_customizada() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar(
            "Categoria\tReal\tMeta\nA\t10\t30\nB\t20\t40".into(),
        ));
        app.graficos = Estado {
            intervalo: "A1:C3".into(),
            titulo: "Distribuição".into(),
            tipo: TipoUi::Pizza, cabecalho: true, categorias: true,
            colunas: true, aba_id: app.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        app.atualizar_grafico();
        // Valor da pizza = a segunda série (escolha única na aba Configurar).
        let _ = app.evento_grafico(Evento::SerieVisivel(1, true));
        assert_eq!(app.origem_do_valor(), Some(1));
        let _ = app.evento_grafico(Evento::Estilo(
            CampoEstilo::Cor, "#34ab78".into()
        ));
        let previa = app.graficos.svg.clone().unwrap();
        let _ = app.evento_grafico(Evento::Inserir);
        let aba = app.workbook.aba(0).unwrap();
        let salvos = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
        assert_eq!(salvos.len(), 1);
        assert_eq!(salvos[0]["type"], "pie");
        assert_eq!(svg_do_grafico_salvo(&aba.sheet, &salvos[0]).unwrap(), previa);
    }

    #[test]
    fn app009_grafico_antigo_sem_feathernative_preserva_cores_de_originais() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar(
            "Categoria\tVendas\nA\t10\nB\t20".into(),
        ));
        let mut config = ChartConfig {
            tipo: ChartType::Column,
            title: "Antigo".into(),
            range: ChartRange { r0: 0, c0: 0, r1: 2, c1: 1 },
            series_in_columns: true, has_headers: true,
            has_category_labels: true, ..Default::default()
        };
        config.style.series_colors.insert(0, "#0066cc".into());
        let antigo = serde_json::to_value(config).unwrap();
        assert!(antigo.get("_featherNative").is_none());
        let aba = app.workbook.aba(0).unwrap();
        let antes = aba.sheet.get_raw(1, 1).to_owned();
        let svg = svg_do_grafico_salvo(&aba.sheet, &antigo).unwrap();
        assert!(svg.contains("#0066cc"), "mantém a paleta salva no estilo legado");
        assert_eq!(aba.sheet.get_raw(1, 1), antes);
    }

    #[test]
    fn app009_entrada_hostil_rejeita_intervalo_sem_alocar_grafico() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("A\tValor\nB\t4".into()));
        let aba = app.workbook.aba(0).unwrap();
        let mut c = ChartConfig {
            tipo: ChartType::Column, title: "Grande".into(),
            range: ChartRange { r0: 0, c0: 0, r1: 1_048_575, c1: 16_383 },
            ..Default::default()
        };
        let too_big = serde_json::to_value(&c).unwrap();
        assert!(svg_do_grafico_salvo(&aba.sheet, &too_big).is_none());
        c.range = ChartRange { r0: 2, c0: 2, r1: 0, c1: 1 };
        assert!(svg_do_grafico_salvo(
            &aba.sheet, &serde_json::to_value(c).unwrap()
        ).is_none());
    }

    #[test]
    fn dropdown_respeita_selecao_e_grafico_move_na_grade() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar(
            "Grupo\tValor\tMeta\nA\t10\t30\nB\t20\t40".into(),
        ));
        a.linha = 0;
        a.coluna = 0;
        a.fim_selecao = (2, 1);
        let _ = a.evento_grafico(Evento::Abrir);
        assert_eq!(a.graficos.intervalo, "A1:B3");
        assert!(!a.graficos.aberto);
        let _ = a.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        assert!(a.graficos.aberto);
        let id = a.graficos.id.clone().unwrap();
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(100., 100.)));
        let _ = a.evento_grafico(Evento::Arrastar(id.clone()));
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(130., 120.)));
        let _ = a.evento_grafico(Evento::Soltar);
        let lista =
            feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap();
        assert_eq!(lista[0]["position"]["x"], 110.);
        assert_eq!(lista[0]["position"]["y"], 100.);
        let _ = a.evento_grafico(Evento::Fechar);
        assert!(!a.graficos.aberto);
        let _ = a.evento_grafico(Evento::Remover(id));
        assert!(
            feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn grafico_duas_linhas_sao_dados_nao_cabecalho() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("a\t1\nb\t2".into()));
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        let _ = app.evento_grafico(Evento::Abrir);
        assert_eq!(app.graficos.intervalo, "A1:B2");
        assert!(!app.graficos.cabecalho, "primeira linha a=1 não é título");
        let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        assert!(app.graficos.svg.is_some());
        let aba = app.workbook.aba(0).unwrap();
        let charts = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0]["hasHeaders"], false);
        assert_eq!(charts[0]["position"]["width"], 480.);
        assert_eq!(charts[0]["position"]["height"], 300.);
        assert!(app.graficos.mensagem.contains("2 categorias"));
    }

    #[test]
    fn alca_redimensiona_preserva_posicao_e_reabre_dimensoes_editadas() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Categoria\tValor\na\t1\nb\t2".into()));
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        let _ = app.evento_grafico(Evento::Abrir);
        let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let id = app.graficos.id.clone().unwrap();

        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(100., 100.)));
        let _ = app.evento_grafico(Evento::Redimensionar(id.clone(), Alca::Sudeste));
        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(140., 124.)));
        let _ = app.evento_grafico(Evento::Soltar);
        let salvos = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet,
        ).unwrap();
        assert_eq!(salvos[0]["position"]["x"], 80.);
        assert_eq!(salvos[0]["position"]["y"], 80.);
        assert_eq!(salvos[0]["position"]["width"], 520.);
        assert_eq!(salvos[0]["position"]["height"], 324.);

        let _ = app.evento_grafico(Evento::Editar(id.clone()));
        assert_eq!(app.graficos.aparencia.largura, "520");
        assert_eq!(app.graficos.aparencia.altura, "324");
        let _ = app.evento_grafico(Evento::Inserir);
        let salvos = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet,
        ).unwrap();
        assert_eq!(salvos.len(), 1);
        assert_eq!(salvos[0]["position"]["width"], 520.);
        assert_eq!(salvos[0]["position"]["height"], 324.);
    }

    #[test]
    fn scroll_e_zoom_nao_modificam_posicao_salva_na_folha() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Categoria\tValor\na\t1\nb\t2".into()));
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        let _ = app.evento_grafico(Evento::Abrir);
        let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let antes = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet
        ).unwrap()[0]["position"].clone();
        app.linha_inicio = 10;
        app.coluna_inicio = 4;
        app.config.zoom = 150;
        let deslocamento = posicionamento::scroll_em_pixels(
            &app.workbook.aba(0).unwrap().sheet,
            app.linha_inicio, app.coluna_inicio,
        );
        assert_eq!(deslocamento, (488., 280.));
        let quadro = Quadro::da_posicao(&antes).unwrap();
        assert_eq!(posicionamento::origem_tela(quadro, deslocamento, 1.5), (-612., -300.));
        let depois = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet
        ).unwrap()[0]["position"].clone();
        assert_eq!(depois, antes, "rolagem/zoom afetam tela, nunca .fshz");
    }

    #[test]
    fn grafico_novo_nasce_na_parte_visivel_da_folha() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Categoria\tValor\na\t1\nb\t2".into()));
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        // Rolado para longe dos dados, como numa planilha de 900 mil linhas.
        app.linha_inicio = 900_000;
        app.coluna_inicio = 3;
        let _ = app.evento_grafico(Evento::Abrir);
        let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let posicao = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet
        ).unwrap()[0]["position"].clone();
        let rolagem = posicionamento::scroll_em_pixels(
            &app.workbook.aba(0).unwrap().sheet, app.linha_inicio, app.coluna_inicio);
        let quadro = Quadro::da_posicao(&posicao).unwrap();
        assert_eq!(posicionamento::origem_tela(quadro, rolagem, 1.0), (80., 80.));
    }

    #[test]
    fn alca_noroeste_move_borda_sem_arrastar_todo_o_grafico() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Categoria\tValor\na\t1\nb\t2".into()));
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        let _ = app.evento_grafico(Evento::Abrir);
        let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        let id = app.graficos.id.clone().unwrap();
        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(100., 100.)));
        let _ = app.evento_grafico(Evento::Redimensionar(id, Alca::Noroeste));
        assert!(app.graficos.arraste.is_none());
        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(120., 110.)));
        let pos = feathersheet_io::graficos_salvos::listar(
            &app.workbook.aba(0).unwrap().sheet
        ).unwrap()[0]["position"].clone();
        assert_eq!((pos["x"].as_f64(), pos["y"].as_f64()), (Some(100.), Some(90.)));
        assert_eq!((pos["width"].as_f64(), pos["height"].as_f64()),
                   (Some(460.), Some(290.)));
    }

    #[test]
    fn pizza_com_varias_series_e_estilos_geram_svg() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar(
            "Grupo\tValor\tMeta\nA\t10\t30\nB\t20\t40".into(),
        ));
        a.linha = 0;
        a.coluna = 0;
        a.fim_selecao = (0, 0);
        let _ = a.evento_grafico(Evento::Abrir);
        let _ = a.evento_grafico(Evento::Tipo(TipoUi::Pizza));
        assert!(a.graficos.svg.is_some());
        let _ = a.evento_grafico(Evento::SerieVisivel(1, true));
        let _ = a.evento_grafico(Evento::Estilo(
            CampoEstilo::EixoX,
            "Categoria & <Nome>".into(),
        ));
        let _ = a.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        assert!(a
            .graficos
            .svg
            .as_ref()
            .unwrap()
            .contains("Categoria &amp; &lt;Nome&gt;"));
        let _ = a.evento_grafico(Evento::Estilo(CampoEstilo::Cor, "invalida".into()));
        assert!(a.graficos.svg.is_none());
    }
    #[test]
    fn detecta_tabela_na_celula_e_persiste_edicao() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Categoria\tValor\nA\t10\nB\t20".into()));
        a.linha = 0;
        a.coluna = 0;
        a.fim_selecao = (0, 0);
        let _ = a.evento_grafico(Evento::Abrir);
        assert_eq!(a.graficos.intervalo, "A1:B3");
        assert!(a.graficos.seletor_aberto);
        let _ = a.evento_grafico(Evento::Tipo(TipoUi::Colunas));
        assert!(!a.graficos.seletor_aberto);
        let _ = a.evento_grafico(Evento::Titulo("Vendas".into()));
        let _ = a.evento_grafico(Evento::Inserir);
        let lista =
            feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap();
        assert_eq!(lista.len(), 1);
        let id = lista[0]["id"].as_str().unwrap().to_string();
        let _ = a.evento_grafico(Evento::Editar(id));
        assert_eq!(a.graficos.titulo, "Vendas");
        let _ = a.evento_grafico(Evento::Estilo(CampoEstilo::Subtitulo, "Mensal".into()));
        let _ = a.evento_grafico(Evento::Visibilidade(CampoVisivel::Valores, true));
        let _ = a.evento_grafico(Evento::Inserir);
        let lista =
            feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0]["_featherNative"]["subtitulo"], "Mensal");
    }
    #[test]
    fn grafico_da_selecao_atualiza_sem_alterar_agregacao() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Categoria\tValor\nA\t10\nA\t20".into()));
        a.graficos = Estado {
            intervalo: "A1:B3".into(),
            titulo: "Teste & dados".into(),
            cabecalho: true,
            categorias: true,
            colunas: true,
            agrupar: true,
            aba_id: a.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        a.atualizar_grafico();
        assert!(a
            .graficos
            .svg
            .as_ref()
            .unwrap()
            .contains("Teste &amp; dados"));
        assert!(a.graficos.mensagem.contains("1 categorias"));
        a.graficos.tipo = TipoUi::Pizza;
        a.atualizar_grafico();
        assert!(a.graficos.svg.is_some());
        // Resumindo, o intervalo enorme para nos dados (A1:B3) e desenha.
        a.graficos.intervalo = "A1:XFD1048576".into();
        a.atualizar_grafico();
        assert!(a.graficos.svg.is_some(), "{}", a.graficos.mensagem);
        // Mais de 200 mil células: o resumo liga sozinho e não desliga.
        a.graficos.agrupar = false;
        a.atualizar_grafico();
        assert!(a.graficos.agrupar && a.graficos.resumo_obrigatorio && a.graficos.svg.is_some());
        assert!(a.graficos.mensagem.contains("200.000"), "{}", a.graficos.mensagem);
        let _ = a.evento_grafico(Evento::Agrupar(false));
        assert!(a.graficos.agrupar);
        let _ = a.evento_grafico(Evento::SerieConta(0, None));
        assert!(a.graficos.agrupar);
        // Intervalo pequeno de novo: volta a poder desligar.
        let _ = a.evento_grafico(Evento::Intervalo("A1:B3".into()));
        let _ = a.evento_grafico(Evento::Agrupar(false));
        assert!(!a.graficos.agrupar && !a.graficos.resumo_obrigatorio);
    }

    /// Pedido de 16/10: mais de 200 mil linhas, resumido (soma, média,
    /// contagem, desvio padrão), usando todas as linhas.
    #[test]
    fn grafico_resumido_le_mais_de_200_mil_linhas() {
        use feathersheet_core::grafico::chart::AggregateFn;
        let mut a = App::default();
        a.workbook.editar(0, |s| {
            s.set_cell_raw(0, 0, "Gênero".into(), None);
            s.set_cell_raw(0, 1, "Valor".into(), None);
            for r in 1..=300_000u32 {
                s.set_cell_raw(r, 0, ["pop", "rock", "jazz"][(r % 3) as usize].into(), None);
                s.set_cell_raw(r, 1, (r % 10).to_string(), None);
            }
        });
        a.graficos = Estado {
            intervalo: "A1:B300001".into(),
            cabecalho: true, categorias: true, colunas: true, agrupar: true,
            aba_id: a.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        for conta in [AggregateFn::Sum, AggregateFn::Avg, AggregateFn::Count, AggregateFn::Stddev] {
            a.graficos.contas.insert(0, conta);
            a.atualizar_grafico();
            assert!(a.graficos.svg.is_some(), "{conta:?}: {}", a.graficos.mensagem);
            assert!(a.graficos.mensagem.contains("3 categorias"), "{conta:?}: {}", a.graficos.mensagem);
        }
    }
    #[test]
    fn pelo_menu_a_galeria_abre_no_centro_e_nao_onde_o_mouse_estava() {
        // #4: pelo menu "Gráfico…" o seletor aparecia embaixo do Colar.
        let mut app = App::default();
        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(40.0, 160.0)));
        let _ = app.evento_grafico(Evento::AbrirNoCentro);
        assert!(app.graficos.seletor_aberto);
        let pos = crate::geometria_overlay::galeria_graficos(
            app.graficos.ancora_seletor_x, app.tamanho_principal.width, app.tamanho_principal.height,
            226.0, seletor_tipo::LARGURA_POPOVER, seletor_tipo::ALTURA_POPOVER,
        ).unwrap();
        let centro = pos.x + pos.largura / 2.0;
        assert!((centro - app.tamanho_principal.width / 2.0).abs() < 1.0, "{pos:?}");
    }

    #[test]
    fn galeria_do_botao_grafico_nao_segue_o_mouse_apos_abrir() {
        let mut app = App::default();
        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(298.0, 142.0)));
        let _ = app.evento_grafico(Evento::Abrir);
        assert!(app.graficos.seletor_aberto);
        assert_eq!(app.graficos.ancora_seletor_x, 298.0);
        let base: pelicano::Element<'_, Msg> = pelicano::text("Grade").into();
        drop(app.sobrepor_galeria_graficos(base));

        let _ = app.evento_grafico(Evento::Cursor(pelicano::Point::new(480.0, 390.0)));
        assert_eq!(app.graficos.ancora_seletor_x, 298.0);
        let pos = crate::geometria_overlay::galeria_graficos(
            app.graficos.ancora_seletor_x,
            app.tamanho_principal.width,
            app.tamanho_principal.height,
            226.0, seletor_tipo::LARGURA_POPOVER, seletor_tipo::ALTURA_POPOVER,
        ).unwrap();
        assert_eq!(pos.x, 266.0);
        assert!(pos.contido(1280.0, 800.0));
        let _ = app.evento_grafico(Evento::FecharSeletor);
        assert!(!app.graficos.seletor_aberto);
    }

    #[test]
    fn galeria_abre_fecha_e_seleciona_tipo_disponivel() {
        let mut a = App::default();
        assert!(!a.graficos.seletor_aberto);
        let _ = a.evento_grafico(Evento::AlternarSeletor);
        assert!(a.graficos.seletor_aberto);
        let _ = a.evento_grafico(Evento::Tipo(TipoUi::Linhas));
        assert_eq!(a.graficos.tipo, TipoUi::Linhas);
        assert!(!a.graficos.seletor_aberto);
        let _ = a.evento_grafico(Evento::AlternarSeletor);
        let _ = a.evento_grafico(Evento::AlternarSeletor);
        assert!(!a.graficos.seletor_aberto);
    }

    #[test]
    fn area_e_degraus_estao_disponiveis_na_interface_nativa() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Categoria\tVendas\tMeta\nA\t10\t1\nB\t20\t2".into()));
        a.graficos = Estado {
            intervalo: "A1:C3".into(),
            titulo: "Área e degraus".into(),
            cabecalho: true,
            categorias: true,
            colunas: true,
            aba_id: a.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        for (ui, engine) in [
            (TipoUi::Area, ChartType::Area),
            (TipoUi::Degraus, ChartType::StepLine),
        ] {
            assert_eq!(ui.motor(), engine);
            let _ = a.evento_grafico(Evento::Tipo(ui));
            assert!(a.graficos.svg.as_ref().unwrap().contains("<path"));
            assert!(a.graficos.mensagem.contains("2 categorias"));
        }
    }

    #[test]
    fn rejeita_intervalo_invalido_e_aba_removida() {
        let mut a = App::default();
        a.graficos.intervalo = "???".into();
        a.atualizar_grafico();
        assert!(a.graficos.svg.is_none());
        a.graficos.intervalo = "A1:B2".into();
        a.graficos.aba_id = Some(999);
        a.atualizar_grafico();
        assert!(a.graficos.mensagem.contains("removida"));
    }

    /// Gráfico importado do `.xlsx` (com `_excel`): é gráfico do motor, desenhado
    /// na camada de objetos na ordem do desenho, e responde como os do app:
    /// clique seleciona, duplo clique edita, botão direito abre o menu,
    /// Delete exclui, clicar numa célula tira a seleção.
    #[test]
    fn grafico_importado_seleciona_edita_menu_e_delete() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tTotal\nmar\t1\nabr\t2".into()));
        let g = serde_json::json!([{
            "id": "chart_x", "type": "column", "title": "Gráfico",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 1},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 400.0, "height": 200.0},
            "_featherNative": {"cor": "#ff0000", "fundo": "#00000000", "fonte": "9", "titulo": false,
                "legenda": false, "eixos": true, "valores": true, "formato": "moeda:2"},
            "_excel": {"fundoDegrade": [[0.0, "#14223b"], [1.0, "#4472c400"]], "fundoAngulo": 270.0,
                "contorno": "#d9d9d9", "corTexto": "#f2f2f2", "eixoValores": true, "z": 4}
        }]);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(g.to_string());

        let camada = a.graficos_da_camada();
        assert_eq!(camada.len(), 1);
        assert_eq!(camada[0].0, 4, "o z do desenho do Excel");
        let svg = &camada[0].2;
        assert!(svg.contains("linearGradient") && svg.contains("#14223b") && svg.contains("#d9d9d9"), "degradê e contorno do Excel");
        assert!(svg.contains("#f2f2f2") && svg.contains("R$ 2,00"), "cor do texto e formato do Excel");

        // Clique: seleciona; segundo clique logo depois: abre o editor.
        let _ = a.evento_grafico(Evento::Arrastar("chart_x".into()));
        let _ = a.evento_grafico(Evento::Soltar);
        assert_eq!(a.graficos.selecionado.as_deref(), Some("chart_x"));
        assert!(!a.graficos.aberto);
        let _ = a.evento_grafico(Evento::Arrastar("chart_x".into()));
        assert!(a.graficos.aberto, "duplo clique abre o editor");
        let _ = a.evento_grafico(Evento::Fechar);

        // Botão direito: menu com Editar e Excluir.
        let _ = a.evento_grafico(Evento::Contexto("chart_x".into()));
        assert_eq!(a.graficos_contexto().map(|c| c.0), Some("chart_x".to_string()));
        {
            let _tela: Element<'_, Msg> = a.sobrepor_menu_formas(text("grade").into());
            let _graficos: Element<'_, Msg> = a.com_graficos_salvos(text("grade").into());
            let _objetos: Element<'_, Msg> = a.grade_com_objetos(text("grade").into());
        }
        // Passar o mouse pelos itens não fecha o menu (bug de 17/10).
        let _ = a.evento_grafico(Evento::PassouNoMenu);
        assert!(a.graficos_contexto().is_some(), "o menu continua aberto para o clique");
        let _ = a.evento_grafico(Evento::FecharContexto);
        assert!(a.graficos_contexto().is_none());

        // Clicar numa célula tira a seleção; Delete então não apaga o gráfico.
        a.trocar_selecao_por_clique(5, 5);
        assert!(a.graficos.selecionado.is_none());
        assert!(!a.apagar_grafico_selecionado());

        // Selecionado: Delete exclui.
        let _ = a.evento_grafico(Evento::Arrastar("chart_x".into()));
        let _ = a.evento_grafico(Evento::Soltar);
        a.graficos.ultimo_clique = None;
        assert!(a.teclado_objetos(&crate::compat::keyboard::Key::Named(crate::compat::keyboard::key::Named::Delete), crate::compat::keyboard::Modifiers::empty()));
        assert!(feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().is_empty());
    }


    /// Bug de 15/10: botão direito → "Editar gráfico…" num gráfico do `.xlsx`
    /// com os dados em outra aba (`sourceSheet`) fazia o gráfico sumir (a
    /// prévia lia a aba do desenho). Agora o aberto se desenha igual ao
    /// fechado, a edição grava ao vivo e cada uma é um Ctrl+Z.
    #[test]
    fn editar_grafico_com_dados_em_outra_aba_continua_desenhado_e_ao_vivo() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tTotal\nmar\t1\nabr\t\nmai\t3".into()));
        a.workbook.criar_aba("Painel");
        let painel = a.workbook.len() - 1;
        let fonte = a.workbook.nomes()[0].clone();
        let g = serde_json::json!([{
            "id": "chart_x", "type": "column", "title": "Total",
            "range": {"r0": 0, "c0": 0, "r1": 3, "c1": 1}, "sourceSheet": fonte,
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 20.0, "y": 20.0, "width": 600.0, "height": 180.0},
            "_featherNative": {"cor": "#ff0000", "fundo": "#00000000", "valores": true},
            "_excel": {"fundo": "#14223b", "corTexto": "#f2f2f2", "z": 4}
        }]);
        a.workbook.aba_mut(painel).unwrap().sheet.g = Some(g.to_string());
        a.folha = painel;
        let antes = a.graficos_da_camada();
        assert!(antes[0].2.contains("#14223b"), "fechado: vestido do Excel");

        let _ = a.evento_grafico(Evento::Contexto("chart_x".into()));
        let _ = a.evento_grafico(Evento::Editar("chart_x".into()));
        assert!(a.graficos.aberto);
        let aberto = a.graficos_da_camada();
        assert_eq!(aberto.len(), 1);
        assert_eq!(aberto[0].2, antes[0].2, "aberto: o mesmo desenho, não a prévia indisponível");
        assert!(a.graficos.svg.as_deref().is_some_and(|s| s.contains("#14223b")), "prévia do editor = desenho");

        // Edição ao vivo: o título muda na planilha sem "Salvar"; letras
        // seguidas viram um passo; Ctrl+Z volta e o painel acompanha.
        let _ = a.evento_grafico(Evento::Titulo("Tota".into()));
        let _ = a.evento_grafico(Evento::Titulo("Totais".into()));
        let lista = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(painel).unwrap().sheet).unwrap();
        assert_eq!(lista[0]["title"], "Totais");
        assert!(lista[0]["_excel"].is_object() && lista[0]["sourceSheet"] == fonte, "preserva o do Excel");
        assert!(a.graficos_da_camada()[0].2.contains("Totais"));
        let _ = a.update(Msg::Desfazer);
        let lista = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(painel).unwrap().sheet).unwrap();
        assert_eq!(lista[0]["title"], "Total");
        assert_eq!(a.graficos.titulo, "Total", "o painel recarrega depois do Ctrl+Z");

        let _ = a.evento_grafico(Evento::Fechar);
        assert_eq!(a.graficos_da_camada()[0].2, antes[0].2, "fechado de novo: igual");
    }

    /// Painel Formatar gráfico: acordeão com um grupo aberto por vez, cada
    /// edição grava ao vivo no gráfico (estilo no JSON do `.fshz`) e é um
    /// Ctrl+Z; o desenho mostra tema, ponto pintado e margens.
    #[test]
    fn painel_formatar_grafico_acordeao_ao_vivo_e_desfazer() {
        use super::painel::{AlvoCor, Grupo, Mudanca};
        let mut a = App::default();
        let _ = a.preparar_captura_grafico("pontos");
        let id = a.graficos.id.clone().expect("gráfico inserido");
        let salvo = |a: &App| feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap()
            .into_iter().find(|v| v["id"].as_str() == Some(id.as_str())).unwrap();
        let v = salvo(&a);
        let estilo = &v["_featherNative"]["estilo"];
        assert_eq!(estilo["tema"], "oceano");
        assert_eq!(estilo["margens_grafico"]["esquerda"], 22.0);
        assert_eq!(estilo["series"]["0"]["pontos"]["3"]["cor"], "#d81b60");
        assert_eq!(v["style"]["subtitle"], "1º semestre de 2026", "campos antigos acompanham (gravador .xlsx)");
        let svg = &a.graficos_da_camada()[0].2;
        assert!(svg.contains("#f2f8fb") && svg.contains("#d81b60") && svg.contains("R$ mil"), "tema, ponto e título do eixo no desenho");

        // Acordeão: abrir um grupo fecha o anterior; Em breve não abre.
        assert_eq!(a.graficos.painel.grupo, Some(Grupo::Pontos));
        let _ = a.evento_grafico(Evento::Grupo(Grupo::Eixos));
        assert_eq!(a.graficos.painel.grupo, Some(Grupo::Eixos));
        let _ = a.evento_grafico(Evento::Grupo(Grupo::Eixos));
        assert_eq!(a.graficos.painel.grupo, None);

        // Digitar num campo junta num passo; Ctrl+Z volta o campo inteiro.
        let _ = a.evento_grafico(Evento::Formatar(Mudanca::Margem(super::painel::AreaSel::Grafico, super::painel::Lado::Inferior, "4".into())));
        let _ = a.evento_grafico(Evento::Formatar(Mudanca::Margem(super::painel::AreaSel::Grafico, super::painel::Lado::Inferior, "40".into())));
        assert_eq!(salvo(&a)["_featherNative"]["estilo"]["margens_grafico"]["inferior"], 40.0);
        let _ = a.update(Msg::Desfazer);
        assert!(salvo(&a)["_featherNative"]["estilo"]["margens_grafico"]["inferior"].is_null());
        assert!(a.graficos.aberto, "painel continua aberto depois do Ctrl+Z");
        let _ = a.update(Msg::Desfazer);
        assert!(salvo(&a)["_featherNative"]["estilo"]["series"]["0"]["pontos"]["3"].is_null(), "o ponto pintado era o passo anterior");

        // Cada grupo monta nos dois temas, com a paleta aberta.
        a.graficos.painel.cor_menu = Some(AlvoCor::Serie(0));
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            for g in Grupo::TODOS {
                a.graficos.painel.grupo = Some(g);
                let _: Element<'_, Msg> = a.view_graficos();
            }
        }
        feather_ui::tema::definir_tema(false, 0);
    }

/// O exemplo do usuário (15/10) no boletim de teste: A7:G22 em colunas
/// empilhadas só com Prova 1, Prova 2 e Trabalho (nome da legenda trocado);
/// copiar e colar; a cópia vira pizza da quantidade de alunos por Situação.
#[test]
fn boletim_configurar_series_copiar_e_pizza_por_situacao() {
    let mut app = App::default();
    app.workbook = feathersheet_core::modelos::criar("notas-escolares").expect("modelo");
    app.folha = 0;
    app.graficos = Estado {
        intervalo: "A7:G22".into(),
        titulo: "Notas".into(),
        tipo: TipoUi::ColunasEmpilhadas,
        cabecalho: true,
        categorias: true,
        colunas: true,
        aba_id: app.workbook.aba(0).map(|a| a.id),
        ..Default::default()
    };
    app.atualizar_grafico();
    let _ = app.inserir_grafico();
    let nomes: Vec<String> = app.series_da_origem().into_iter().map(|s| s.1).collect();
    // Sem resumir, Situação (só texto) não é série: o motor a pula.
    assert_eq!(nomes, ["Matrícula", "Prova 1", "Prova 2", "Trabalho", "Média"]);
    for n in [0, 4] {
        let _ = app.evento_grafico(Evento::SerieVisivel(n, false));
    }
    let _ = app.evento_grafico(Evento::SerieNome(1, "1ª prova".into()));
    {
        let _painel: Element<'_, Msg> = app.view_graficos();
    }
    assert!(app.graficos.svg.is_some());
    assert_eq!(app.graficos.series, ["1 — 1ª prova", "2 — Prova 2", "3 — Trabalho"]);

    // Copiar e colar com o gráfico selecionado (painel fechado).
    let original = app.graficos.id.clone().expect("gravado");
    app.graficos.aberto = false;
    app.graficos.selecionado = Some(original.clone());
    assert!(app.atalho_grafico("c"));
    assert!(app.atalho_grafico("v"));
    let lista = feathersheet_io::graficos_salvos::listar(&app.workbook.aba(0).unwrap().sheet).unwrap();
    assert_eq!(lista.len(), 2);
    let copia = app.graficos.selecionado.clone().unwrap();
    assert_ne!(copia, original);

    // A cópia: pizza, categorias = Situação, conta = Contar sobre Prova 1.
    let _ = app.editar_grafico_salvo(&copia);
    let _ = app.evento_grafico(Evento::Tipo(TipoUi::Pizza));
    let _ = app.evento_grafico(Evento::CategoriaColuna(Some(6)));
    let p1 = app.series_da_origem().into_iter().find(|s| s.1 == "Prova 1").unwrap().0;
    let _ = app.evento_grafico(Evento::SerieVisivel(p1, true));
    let _ = app.evento_grafico(Evento::SerieConta(p1, Some(AggregateFn::Count)));
    let mut cats = app.graficos.painel.categorias.clone();
    cats.sort();
    assert_eq!(cats, ["Aprovado", "Reprovado"]);

    // O gravado conta os 15 alunos (aprovados + reprovados).
    let lista = feathersheet_io::graficos_salvos::listar(&app.workbook.aba(0).unwrap().sheet).unwrap();
    let v = lista.iter().find(|v| v["id"].as_str() == Some(copia.as_str())).unwrap();
    let mut config: ChartConfig = serde_json::from_value(v.clone()).unwrap();
    config.tipo = ChartType::Column;
    let c = feathersheet_io::grafico_compartilhado::extrair_grafico(&app.workbook.aba(0).unwrap().sheet, &config).unwrap();
    assert_eq!(c.grafico.series.len(), 1);
    let total: f64 = c.grafico.series[0].valores.iter().sum();
    assert_eq!(total, 15.0, "{:?} {:?}", c.grafico.categorias, c.grafico.series[0].valores);
    // O original continua com as três séries.
    let o = lista.iter().find(|v| v["id"].as_str() == Some(original.as_str())).unwrap();
    assert_eq!(o["style"]["hiddenSeries"], serde_json::json!([0, 4]));
}

    /// Gráfico parado na planilha: o mouse sobre a coluna mostra a dica com
    /// categoria, série e valor; clicar no item da legenda desliga a série
    /// (fica hachurado, um passo de Ctrl+Z) com transição, e clicar de novo liga.
    #[test]
    fn dica_do_mouse_e_clique_na_legenda_liga_e_desliga_a_serie() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tVendas\tMeta\nJan\t10\t8\nFev\t30\t20".into()));
        let g = serde_json::json!([{
            "id": "g1", "type": "column", "title": "Vendas",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 2},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 480.0, "height": 300.0}
        }]);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(g.to_string());
        a.config.zoom = 100;
        let v = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().remove(0);
        let d = super::editor::desenhar_grafico_salvo(&a.workbook.aba(0).unwrap().sheet, &v, None).unwrap();
        let mapa = d.mapa.unwrap();

        // Dica sobre a coluna de Fev da série Vendas.
        let alvo = mapa.pontos.iter().find(|p| p.serie == 0 && p.ponto == 1).unwrap();
        let feather_charts::formatado::FormaAlvo::Retangulo { x, y, largura, altura } = alvo.forma else { panic!("coluna") };
        let _ = a.evento_grafico(Evento::Pairar("g1".into(), pelicano::Point::new(x + largura / 2.0, y + altura / 2.0)));
        let dica = a.graficos.interacao.pairar.as_ref().and_then(|p| p.dica.clone()).expect("dica");
        assert_eq!((dica.titulo.as_str(), dica.linhas.join("|").as_str()), ("Fev", "Vendas: 30"));
        // Dica personalizada: sem a série, com prefixo e porcentagem da categoria.
        let mut e = feather_charts::estilo::EstiloGrafico::default();
        e.dica.serie = Some(false);
        e.dica.prefixo = "R$ ".into();
        e.dica.percentual = true;
        let mut ap: Aparencia = Default::default();
        ap.estilo = Some(e);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(serde_json::json!([{
            "id": "g1", "type": "column", "title": "Vendas",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 2},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 480.0, "height": 300.0},
            "_featherNative": ap
        }]).to_string());
        let v2 = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().remove(0);
        let mapa2 = super::editor::desenhar_grafico_salvo(&a.workbook.aba(0).unwrap().sheet, &v2, None).unwrap().mapa.unwrap();
        let alvo2 = mapa2.pontos.iter().find(|p| p.serie == 0 && p.ponto == 1).unwrap();
        let feather_charts::formatado::FormaAlvo::Retangulo { x, y, largura, altura } = alvo2.forma else { panic!("coluna") };
        let _ = a.evento_grafico(Evento::Pairar("g1".into(), pelicano::Point::new(x + largura / 2.0, y + altura / 2.0)));
        let dica = a.graficos.interacao.pairar.as_ref().and_then(|p| p.dica.clone()).expect("dica");
        assert_eq!(dica.linhas, ["R$ 30 (60,0%)"]);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(g.to_string());
        assert!(a.dica_do_grafico("g1", 480.0, 300.0, (0.0, 0.0)).is_some());
        let _ = a.evento_grafico(Evento::SairDoGrafico("g1".into()));
        assert!(a.graficos.interacao.pairar.is_none());

        // Clique no item "Meta" da legenda: desliga, sem arrastar o gráfico.
        let item = mapa.legenda.iter().find(|i| i.indice == 1).unwrap();
        let (lx, ly) = (item.x + item.largura / 2.0, item.y + item.altura / 2.0);
        let _ = a.evento_grafico(Evento::Pairar("g1".into(), pelicano::Point::new(lx, ly)));
        let _ = a.evento_grafico(Evento::Arrastar("g1".into()));
        assert!(a.graficos.arraste.is_none(), "clique na legenda não vira arraste");
        assert!(a.grafico_em_transicao());
        let salvo = |a: &App| feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().remove(0);
        assert_eq!(salvo(&a)[super::interacao::CAMPO_DESLIGADOS], serde_json::json!([1]));
        let svg = &a.graficos_da_camada()[0].2;
        assert!(svg.contains("legenda-amostra desligado"), "item hachurado");
        assert!(svg.contains("barra s1") && svg.contains("<g opacity=\""), "antes por baixo, depois esmaecendo por cima");

        // Fim da transição: Meta some do desenho, a legenda continua.
        a.graficos.interacao.transicao.as_mut().unwrap().inicio -= std::time::Duration::from_secs(1);
        let _ = a.evento_grafico(Evento::QuadroTransicao);
        assert!(!a.grafico_em_transicao());
        let svg = &a.graficos_da_camada()[0].2;
        assert!(!svg.contains("barra s1") && svg.contains("legenda-amostra desligado"));

        // Ctrl+Z religa; clicar de novo também.
        let _ = a.update(Msg::Desfazer);
        assert!(salvo(&a)[super::interacao::CAMPO_DESLIGADOS].is_null());
        let _ = a.update(Msg::Refazer);
        let _ = a.evento_grafico(Evento::Pairar("g1".into(), pelicano::Point::new(lx, ly)));
        let _ = a.evento_grafico(Evento::Arrastar("g1".into()));
        assert_eq!(salvo(&a)[super::interacao::CAMPO_DESLIGADOS], serde_json::json!([]));
    }

    /// A vitrine dos prints monta o gráfico com a dica aberta.
    #[test]
    fn vitrine_dos_prints_abre_a_dica() {
        let mut a = App::default();
        a.config.zoom = 100;
        let _ = a.preparar_captura_grafico("vitrine");
        let p = a.graficos.interacao.pairar.as_ref().expect("mouse sobre o gráfico");
        let d = p.dica.as_ref().expect("dica");
        assert_eq!(d.titulo, "Junho");
        assert!(d.linhas[0].contains(": R$ "), "{:?}", d.linhas);
    }

    /// Arrastar e redimensionar o gráfico: cada gesto é um passo de Ctrl+Z.
    #[test]
    fn mover_e_redimensionar_entram_no_ctrl_z() {
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tVendas\nJan\t10\nFev\t30".into()));
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(serde_json::json!([{
            "id": "g1", "type": "column", "title": "V",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 1},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 400.0, "height": 250.0}
        }]).to_string());
        a.config.zoom = 100;
        let pos = |a: &App| feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap()[0]["position"].clone();
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(10.0, 10.0)));
        let _ = a.evento_grafico(Evento::Arrastar("g1".into()));
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(60.0, 40.0)));
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(110.0, 70.0)));
        let _ = a.evento_grafico(Evento::Soltar);
        assert_eq!(pos(&a)["x"], 400.0);
        let _ = a.evento_grafico(Evento::Redimensionar("g1".into(), super::posicionamento::Alca::Sudeste));
        let _ = a.evento_grafico(Evento::Cursor(pelicano::Point::new(160.0, 120.0)));
        let _ = a.evento_grafico(Evento::Soltar);
        assert_eq!(pos(&a)["width"], 450.0);
        let _ = a.update(Msg::Desfazer);
        assert_eq!((pos(&a)["x"].clone(), pos(&a)["width"].clone()), (serde_json::json!(400.0), serde_json::json!(400.0)), "desfaz só o redimensionamento");
        let _ = a.update(Msg::Desfazer);
        assert_eq!(pos(&a)["x"], 300.0, "o arraste inteiro num passo");
        let _ = a.update(Msg::Refazer);
        assert_eq!(pos(&a)["x"], 400.0);
    }

    /// Tabela com uma série de texto (Cliente) e uma de números (Valor).
    fn app_com_serie_de_texto() -> App {
        let mut a = App::default();
        let _ = a.update(Msg::Colar(
            "Região\tCliente\tValor\nSul\tAna\t10\nSul\tBia\t5\nNorte\tCaio\t7\nSul\tAna\t1".into(),
        ));
        a.graficos = Estado {
            intervalo: "A1:C5".into(),
            cabecalho: true, categorias: true, colunas: true, agrupar: true,
            aba_id: a.workbook.aba(0).map(|s| s.id),
            ..Default::default()
        };
        a
    }

    /// Pedido de 16/10: série com letra não tem Soma; só Contar e Contar
    /// distintos (e "Não resumir" quando o resumo não é obrigatório).
    #[test]
    fn serie_de_texto_so_oferece_contar_e_contar_distintos() {
        let mut a = app_com_serie_de_texto();
        a.atualizar_grafico();
        assert!(a.graficos.serie_texto(0), "Cliente é texto");
        assert!(!a.graficos.serie_texto(1), "Valor é número");
        let contas = |texto, obrigatorio| painel::contas_da_serie(texto, obrigatorio)
            .into_iter().map(|(_, c)| c).collect::<Vec<_>>();
        assert_eq!(contas(true, false), [None, Some(AggregateFn::Count), Some(AggregateFn::CountDistinct)]);
        assert_eq!(contas(true, true), [Some(AggregateFn::Count), Some(AggregateFn::CountDistinct)]);
        assert!(contas(false, false).contains(&Some(AggregateFn::Sum)));
        assert!(!contas(true, false).contains(&Some(AggregateFn::Sum)));
        {
            let _painel: Element<'_, Msg> = a.view_graficos();
        }
        // Contar distintos: no Sul, Ana e Bia (Ana duas vezes); no Norte, Caio.
        let _ = a.evento_grafico(Evento::SerieConta(0, Some(AggregateFn::CountDistinct)));
        let _ = a.evento_grafico(Evento::SerieVisivel(1, false));
        let _ = a.inserir_grafico();
        let aba = a.workbook.aba(0).unwrap();
        let v = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap().remove(0);
        assert_eq!(v["aggregateBySeries"]["0"], "countDistinct");
        let config: ChartConfig = serde_json::from_value(v).unwrap();
        let c = feathersheet_io::grafico_compartilhado::extrair_grafico(&aba.sheet, &config).unwrap();
        assert_eq!(c.grafico.categorias, ["Sul", "Norte"]);
        assert_eq!(c.grafico.series[0].valores, [2.0, 1.0]);
    }

    /// A conta guardada que não serve à série de texto (e a que falta) vira
    /// Contar; a da série de números fica. Trocar a categoria recomeça as
    /// contas e as séries de texto voltam a Contar, no mesmo passo gravado.
    #[test]
    fn serie_de_texto_tem_contar_como_padrao() {
        let mut a = app_com_serie_de_texto();
        a.graficos.contas = std::collections::BTreeMap::from([(0, AggregateFn::Sum), (1, AggregateFn::Avg)]);
        a.atualizar_grafico();
        assert_eq!(a.graficos.contas.get(&0), Some(&AggregateFn::Count));
        assert_eq!(a.graficos.contas.get(&1), Some(&AggregateFn::Avg));
        assert_eq!(a.graficos.conta_da_serie(0), AggregateFn::Count);
        a.graficos.contas.clear();
        assert_eq!(a.graficos.conta_da_serie(0), AggregateFn::Count, "sem conta, texto conta");
        assert_eq!(a.graficos.conta_da_serie(1), AggregateFn::Sum, "sem conta, número soma");

        let _ = a.evento_grafico(Evento::Inserir);
        // Categoria = Valor: sobram Região e Cliente, as duas de texto.
        let _ = a.evento_grafico(Evento::CategoriaColuna(Some(2)));
        assert!(a.graficos.serie_texto(0) && a.graficos.serie_texto(1));
        let v = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().remove(0);
        assert_eq!(v["aggregateBySeries"], serde_json::json!({"0": "count", "1": "count"}));
        assert!(a.graficos.svg.is_some(), "{}", a.graficos.mensagem);
        // A camada já tem o desenho do gráfico em edição: não lê a tabela de novo.
        assert!(!a.graficos.svgs.borrow().is_empty());
    }

    #[test]
    fn icones_da_faixa_criam_o_tipo_e_recomendados_le_os_dados() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Mês\tVendas\nJan\t10\nFev\t20\nMar\t15".into()));
        let _ = app.evento_grafico(Evento::Rapido(Some(TipoUi::Linhas)));
        let lista = feathersheet_io::graficos_salvos::listar(&app.workbook.aba(0).unwrap().sheet).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(app.graficos.tipo, TipoUi::Linhas);
        // Uma série curta, sem negativos: pizza.
        let _ = app.evento_grafico(Evento::Rapido(None));
        assert_eq!(app.graficos.tipo, TipoUi::Pizza);
        let _ = app.update(Msg::Colar("X\tY\n1\t10\n2\t20\n3\t15".into()));
        let _ = app.evento_grafico(Evento::Rapido(None));
        assert_eq!(app.graficos.tipo, TipoUi::Dispersao);
    }

    /// Relato de 26/10 (pelicano): clicar no gráfico não abria a edição. Monta a
    /// janela inteira e clica pelo despacho do pelicano, como a tela faz.
    #[test]
    fn clique_na_tela_chega_ao_grafico() {
        use pelicano::{Event, MouseButton, Point, Size};
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tTotal\nmar\t1\nabr\t2".into()));
        let g = serde_json::json!([{
            "id": "chart_x", "type": "column", "title": "Gráfico",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 1},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 400.0, "height": 200.0}
        }]);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(g.to_string());
        let tela = Size::new(1280.0, 800.0);
        let mut r = pelicano::Renderer::new();
        let mut st = pelicano::States::default();
        let mut area = pelicano::MemoryClipboard::default();
        let el = a.view(window::Id::MAIN);
        r.begin(tela, 1.0, pelicano::Color::TRANSPARENT);
        let n = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::light());
        let mut acertos = Vec::new();
        for y in (0..800).step_by(20) {
            for x in (0..1280).step_by(20) {
                let p = Point::new(x as f32, y as f32);
                let mut msgs = Vec::new();
                let mut cx = pelicano::EventCx { renderer: &mut r, states: &mut st, cursor: Some(p), messages: &mut msgs,
                    window: &mut Vec::new(), redraw: false, clipboard: &mut area };
                pelicano::dispatch(&el, &n, &Event::MousePressed(MouseButton::Left), &mut cx);
                if msgs.iter().any(|m| matches!(m, Msg::Grafico(Evento::Arrastar(_)))) {
                    acertos.push((x, y));
                }
            }
        }
        assert!(!acertos.is_empty(), "nenhum clique chegou ao gráfico");
    }

    #[test]
    fn duplo_clique_abre_o_editor_e_o_painel_responde() {
        use pelicano::{Event, MouseButton, Point, Size};
        let mut a = App::default();
        let _ = a.update(Msg::Colar("Mês\tTotal\nmar\t1\nabr\t2".into()));
        let g = serde_json::json!([{
            "id": "chart_x", "type": "column", "title": "Gráfico",
            "range": {"r0": 0, "c0": 0, "r1": 2, "c1": 1},
            "seriesInColumns": true, "hasHeaders": true, "hasCategoryLabels": true,
            "position": {"x": 300.0, "y": 100.0, "width": 400.0, "height": 200.0}
        }]);
        a.workbook.aba_mut(0).unwrap().sheet.g = Some(g.to_string());
        let tela = Size::new(1280.0, 800.0);
        // Clica (aperta e solta) num ponto, entregando ao App o que a tela mandou.
        let clicar = |a: &mut App, p: Point| -> Vec<String> {
            let mut r = pelicano::Renderer::new();
            let mut st = pelicano::States::default();
            let mut area = pelicano::MemoryClipboard::default();
            let mut msgs = Vec::new();
            {
                let el = a.view(window::Id::MAIN);
                r.begin(tela, 1.0, pelicano::Color::TRANSPARENT);
                let n = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::light());
                for ev in [Event::MousePressed(MouseButton::Left), Event::MouseReleased(MouseButton::Left)] {
                    let mut cx = pelicano::EventCx { renderer: &mut r, states: &mut st, cursor: Some(p), messages: &mut msgs,
                        window: &mut Vec::new(), redraw: false, clipboard: &mut area };
                    pelicano::dispatch(&el, &n, &ev, &mut cx);
                }
            }
            msgs.push(Msg::Grafico(Evento::Soltar));
            let nomes = msgs.iter().map(|m| format!("{m:?}")).collect();
            for m in msgs { let _ = a.update(m); }
            nomes
        };
        // Meio da área que acerta o gráfico (o canto, selecionado, vira alça).
        // Só a tela responde aqui: as mensagens não chegam ao App, para nada mudar de lugar.
        let acertos: Vec<Point> = {
            let mut r = pelicano::Renderer::new();
            let mut st = pelicano::States::default();
            let mut area = pelicano::MemoryClipboard::default();
            let el = a.view(window::Id::MAIN);
            r.begin(tela, 1.0, pelicano::Color::TRANSPARENT);
            let n = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::light());
            (0..800).step_by(20).flat_map(|y| (0..1280).step_by(20).map(move |x| (x, y)))
                .map(|(x, y)| Point::new(x as f32, y as f32))
                .filter(|p| {
                    let mut msgs = Vec::new();
                    let mut cx = pelicano::EventCx { renderer: &mut r, states: &mut st, cursor: Some(*p), messages: &mut msgs,
                        window: &mut Vec::new(), redraw: false, clipboard: &mut area };
                    pelicano::dispatch(&el, &n, &Event::MousePressed(MouseButton::Left), &mut cx);
                    msgs.iter().any(|m| matches!(m, Msg::Grafico(Evento::Arrastar(_))))
                })
                .collect()
        };
        assert!(!acertos.is_empty(), "nenhum clique chegou ao gráfico");
        let alvo = acertos[acertos.len() / 2];
        a.graficos.ultimo_clique = None;
        clicar(&mut a, alvo);
        clicar(&mut a, alvo);
        assert!(a.graficos.aberto, "duplo clique não abriu o editor");
        // Painel aberto: cliques nele precisam virar mensagens do gráfico.
        let mut respostas = std::collections::BTreeSet::new();
        for y in (90..780).step_by(16) {
            for x in (700..1280).step_by(16) {
                for m in clicar(&mut a, Point::new(x as f32, y as f32)) {
                    if m.starts_with("Grafico(") && !m.contains("Soltar") && !m.contains("Cursor") {
                        respostas.insert(m.split(['(', ',']).nth(1).unwrap_or("").to_string());
                    }
                }
                if !a.graficos.aberto { let _ = a.evento_grafico(Evento::Editar("chart_x".into())); }
            }
        }
        assert!(!respostas.is_empty(), "o painel do gráfico não respondeu a nenhum clique");
    }

/// Monta o editor sobre a tabela colada em A1, no tipo pedido (sem gravar).
fn editor_de(tabela: &str, tipo: TipoUi) -> App {
    let mut app = App::default();
    let _ = app.update(Msg::Colar(tabela.into()));
    let linhas = tabela.lines().count() as u32;
    let colunas = tabela.lines().next().map_or(1, |l| l.split('\t').count()) as u32;
    app.graficos = Estado {
        intervalo: format_ref(&feathersheet_core::celulas::refs::Rect::new(0, 0, linhas - 1, colunas - 1)),
        titulo: "Teste".into(),
        tipo,
        cabecalho: true,
        categorias: true,
        colunas: true,
        aba_id: app.workbook.aba(0).map(|a| a.id),
        ..Default::default()
    };
    app.atualizar_grafico();
    app
}

/// Issue #108: a pizza abria com as duas séries marcadas. O valor é escolha
/// única, e os grupos que o tipo não desenha nem aparecem.
#[test]
fn pizza_tem_um_valor_so_e_escolha_unica() {
    let mut app = editor_de("Situação\tAlunos\tMédia\nA\t18\t7\nB\t6\t5", TipoUi::Pizza);
    assert!(app.graficos.svg.is_some(), "{}", app.graficos.mensagem);
    assert_eq!(app.origem_do_valor(), Some(0));
    // Personalizar > Séries só vê a série desenhada.
    assert_eq!(app.graficos.series.len(), 1);
    {
        let _painel: Element<'_, Msg> = app.view_graficos();
    }
    let _ = app.evento_grafico(Evento::SerieVisivel(1, true));
    assert_eq!(app.graficos.ocultas, [0]);
    assert_eq!(app.origem_do_valor(), Some(1));
    // Pizza gravada antes, com `_featherPieSeries` apontando a 2ª visível.
    app.graficos.ocultas.clear();
    app.graficos.serie = Some("2 — Média".into());
    assert_eq!(app.origem_do_valor(), Some(1));
    assert!(!painel::Grupo::Eixos.existe_em(TipoUi::Pizza));
    assert!(!painel::Grupo::Grades.existe_em(TipoUi::Pizza));
    assert!(!painel::Grupo::Analiticos.existe_em(TipoUi::Pizza));
    assert!(painel::Grupo::Pontos.existe_em(TipoUi::Pizza));
    assert!(painel::Grupo::Analiticos.existe_em(TipoUi::Colunas));
    assert!(!painel::Grupo::Legenda.existe_em(TipoUi::Gantt));
}

/// Issue #113: a rosca tem furo e um anel por série, grava `doughnut` com o
/// `holeSize` e volta rosca ao reabrir (antes virava pizza cheia).
#[test]
fn rosca_tem_um_anel_por_serie_e_guarda_o_furo() {
    let mut app = editor_de("Região\t2025\t2026\nNorte\t18\t22\nSul\t12\t9", TipoUi::Rosca);
    let previa = app.graficos.svg.clone().expect("a rosca desenha");
    assert_eq!(previa.matches("class=\"fatia\"").count(), 4, "duas fatias em cada um dos dois anéis");
    {
        let _painel: Element<'_, Msg> = app.view_graficos();
    }
    let _ = app.evento_grafico(Evento::Furo(70));
    let _ = app.inserir_grafico();
    let aba = app.workbook.aba(0).unwrap();
    let salvos = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
    assert_eq!(salvos[0]["type"], "doughnut");
    assert_eq!(salvos[0]["holeSize"], 70);
    let gravado = svg_do_grafico_salvo(&aba.sheet, &salvos[0]).unwrap();
    assert_eq!(gravado.matches("class=\"fatia\"").count(), 4);
    assert!(!painel::Grupo::Eixos.existe_em(TipoUi::Rosca));
}

/// Gantt com as colunas fora de ordem (Fim antes de Início): escolher o
/// papel de cada coluna reordena as séries, na prévia e no gravado.
#[test]
fn papeis_do_gantt_escolhem_a_coluna_de_cada_campo() {
    let mut app = editor_de("Tarefa\tFim\tInício\nA\t5\t1\nB\t12\t4", TipoUi::Gantt);
    // Pela ordem das colunas o início seria o Fim: termina antes de começar.
    assert!(app.graficos.svg.is_none());
    assert_eq!(app.papeis_atuais(), [0, 1]);
    let _ = app.evento_grafico(Evento::Papel(0, 1));
    assert_eq!(app.papeis_atuais(), [1, 0], "a coluna que já era Fim troca de lugar");
    let previa = app.graficos.svg.clone().expect("com Início e Fim certos desenha");
    {
        let _painel: Element<'_, Msg> = app.view_graficos();
    }
    let _ = app.inserir_grafico();
    let aba = app.workbook.aba(0).unwrap();
    let salvos = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
    assert_eq!(salvos[0]["_featherNative"]["papeis"], serde_json::json!([1, 0]));
    // A mesma barra de cada tarefa, e na mesma cor, na prévia e no gravado.
    let barras = |svg: &str| svg.split("<rect class=\"tarefa").skip(1)
        .map(|r| r.split('>').next().unwrap_or("").to_owned()).collect::<Vec<_>>();
    let gravado = svg_do_grafico_salvo(&aba.sheet, &salvos[0]).unwrap();
    assert_eq!(barras(&gravado), barras(&previa));
    assert!(barras(&previa)[0].contains(" fill="), "{:?}", barras(&previa));
    assert_eq!(barras(&previa).len(), 2);
    // Trocar de tipo esquece os papéis; sem papéis o campo nem é gravado.
    let _ = app.evento_grafico(Evento::Tipo(TipoUi::Colunas));
    assert!(app.graficos.aparencia.papeis.is_empty());
    assert!(serde_json::to_value(Aparencia::default()).unwrap().get("papeis").is_none());
}

/// Dispersão: com a coluna de categorias de números, ela é o X (antes o X
/// era a primeira série, e "X, Y" com uma série só não desenhava).
#[test]
fn dispersao_usa_a_coluna_de_x_escolhida() {
    let app = editor_de("X\tY\n1\t10\n2\t20\n3\t15", TipoUi::Dispersao);
    assert!(app.graficos.svg.is_some(), "{}", app.graficos.mensagem);
    assert_eq!(app.graficos.series, ["1 — Y"]);
    // Categoria de texto: o jeito antigo (X = a primeira série).
    let app = editor_de("Ponto\tX\tY\na\t1\t10\nb\t2\t20", TipoUi::Dispersao);
    assert!(app.graficos.svg.is_some(), "{}", app.graficos.mensagem);
    assert_eq!(app.graficos.series, ["1 — Y"]);
}

/// Linha 3D: X, Y e Z escolhidos no painel vão para o `eixos3d` gravado e
/// voltam ao reabrir.
#[test]
fn linha_3d_grava_os_eixos_escolhidos() {
    let mut app = editor_de("A\tB\tC\tD\n1\t2\t1\t9\n2\t4\t3\t8\n3\t3\t5\t7", TipoUi::Linha3d);
    let padrao = app.eixos3d_padrao().unwrap();
    assert_eq!((padrao.x, padrao.y, padrao.z), (0, 1, Some(2)));
    let _ = app.inserir_grafico();
    let _ = app.evento_grafico(Evento::Eixo3d(2, Some(3)));
    {
        let _painel: Element<'_, Msg> = app.view_graficos();
    }
    let id = app.graficos.id.clone().expect("gravado");
    let aba = app.workbook.aba(0).unwrap();
    let salvos = feathersheet_io::graficos_salvos::listar(&aba.sheet).unwrap();
    assert_eq!(salvos[0]["eixos3d"]["z"], 3);
    let _ = app.editar_grafico_salvo(&id);
    assert_eq!(app.graficos.eixos3d.and_then(|e| e.z), Some(3));
}

/// Sem resumir, o motor pula a coluna de texto entre as séries e numera só
/// as de número: o painel tem que contar igual, senão desmarcar "Meta"
/// escondia a série errada.
#[test]
fn coluna_de_texto_entre_as_series_nao_desloca_a_numeracao() {
    let mut app = editor_de("Mês\tVendas\tObs\tMeta\nJan\t12\tok\t10\nFev\t18\tx\t14", TipoUi::Colunas);
    assert_eq!(app.series_da_origem(), [(0, "Vendas".to_owned()), (1, "Meta".to_owned())]);
    let _ = app.evento_grafico(Evento::SerieVisivel(1, false));
    assert_eq!(app.graficos.ocultas, [1]);
    let desenhadas: Vec<&str> = app.graficos.series.iter().map(|s| s.split(" — ").nth(1).unwrap_or("")).collect();
    assert_eq!(desenhadas, ["Vendas"]);
}

#[test]
fn contorno_do_grafico_estica_mesmo_com_a_camada_tirando_a_selecao() {
    use feathersheet_core::formula::ref_drag::Pegada;
    let mut a = App::default();
    let _ = a.update(Msg::Colar("Mês\tVendas\nJan\t10\nFev\t20\nMar\t30".into()));
    a.linha = 0;
    a.coluna = 0;
    a.fim_selecao = (2, 1);
    let _ = a.evento_grafico(Evento::Abrir);
    let _ = a.evento_grafico(Evento::Tipo(TipoUi::Colunas));
    let _ = a.evento_grafico(Evento::Inserir);
    let id = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap()[0]["id"]
        .as_str().unwrap().to_string();
    a.graficos.aberto = false;
    a.graficos.selecionado = Some(id);
    let caixa = a.caixas_do_grafico().remove(0);
    assert_eq!(format_ref(&caixa.rect), "A1:B3");
    // O clique na alça passa antes pela camada de objetos, que tira a seleção.
    a.desselecionar_grafico();
    assert!(a.caixas_do_grafico().is_empty());
    let _ = a.update(Msg::PegarReferencia);
    let _ = a.update(Msg::ArrastarReferencia(caixa.start, Pegada::Redimensionar, (2, 1), (3, 1)));
    let lista = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap();
    let rect: feathersheet_core::celulas::refs::Rect = serde_json::from_value(lista[0]["range"].clone()).unwrap();
    assert_eq!(format_ref(&rect), "A1:B4");
}

/// A lista do Eixo X abre e escolhe (28/10/2026: no painel do gráfico nenhuma
/// lista suspensa abria; as caixas de marcar funcionavam).
#[test]
fn lista_do_eixo_x_abre_e_escolhe() {
    use pelicano::{Event, MouseButton, Point, Size};
    let mut a = App::default();
    let _ = a.update(Msg::Colar("Grupo\tValor\tMeta\nA\t10\t30\nB\t20\t40".into()));
    a.linha = 0; a.coluna = 0; a.fim_selecao = (2, 2);
    let _ = a.evento_grafico(Evento::Abrir);
    let _ = a.evento_grafico(Evento::Tipo(TipoUi::Linhas));
    assert!(a.graficos.aberto);
    let tela = Size::new(1280.0, 800.0);
    let mut r = pelicano::Renderer::new();
    let mut st = pelicano::States::default();
    let mut area = pelicano::MemoryClipboard::default();
    let mut clicar = |a: &mut App, st: &mut pelicano::States, p: Point| -> Vec<Msg> {
        // O mouse chega até o ponto antes de clicar.
        let _ = a.update(Msg::CursorJanela(window::Id::MAIN, p));
        let el = a.view(window::Id::MAIN);
        r.begin(tela, 1.0, pelicano::Color::TRANSPARENT);
        let n = pelicano::layout(&el, tela, &mut r, st, &pelicano::Theme::light());
        let mut msgs = Vec::new();
        let mut cx = pelicano::EventCx { renderer: &mut r, states: st, cursor: Some(p), messages: &mut msgs,
            window: &mut Vec::new(), redraw: false, clipboard: &mut area };
        pelicano::dispatch(&el, &n, &Event::MousePressed(MouseButton::Left), &mut cx);
        drop(el);
        // O que o app recebe junto com o clique (`evento_janela`).
        let soltar = Msg::Grafico(Evento::Soltar);
        // E os pulsos que rodam entre um quadro e outro.
        for m in msgs.iter().cloned().chain([soltar, Msg::ResumoPulso, Msg::MonitorPulso, Msg::CalcularPulso]) { let _ = a.update(m); }
        msgs
    };
    // Procura o campo pela coluna do painel: um clique que, seguido de outro logo
    // abaixo, escolhe uma categoria.
    for y in (100..700).step_by(6) {
        let campo = Point::new(1180.0, y as f32);
        let _ = clicar(&mut a, &mut st, campo);
        let escolha = clicar(&mut a, &mut st, Point::new(1180.0, y as f32 + 60.0));
        if escolha.iter().any(|m| matches!(m, Msg::Grafico(Evento::CategoriaColuna(_)))) { return; }
        let _ = clicar(&mut a, &mut st, Point::new(5.0, 790.0));
    }
    panic!("nenhum clique abriu a lista do Eixo X e escolheu uma coluna");
}

/// Gráfico de linhas sobre uma dinâmica de 12.664 linhas (DATA, RAMAL, EH, LINHA,
/// BLOCO, valor), como a do usuário em 28/10/2026: o quadro mostrava "Prévia
/// indisponível" e trocar o Eixo X não fazia nada.
#[test]
fn grafico_de_linhas_sobre_dinamica_grande_desenha_e_troca_o_eixo_x() {
    let mut a = App::default();
    let n = 12_664u32;
    let mut linhas = vec![vec!["DATA".to_string(), "RAMAL".into(), "EH".into(), "LINHA".into(), "BLOCO".into(), "Máx de NIV".into()]];
    for r in 0..n - 2 {
        let ini = 6000 + 200 * (r % 40);
        // Como a do usuário: a data vem como série no começo e como texto no fim.
        let data = if r < 6000 { "46240".to_string() } else { format!("7/{}/2026", 1 + r % 28) };
        linhas.push(vec![data, "RLT".into(), format!("EH 0{}/03", 1 + r % 3),
            (1 + r % 2).to_string(), format!("{ini} - {}", ini + 200), format!("{}", 0.8 + (r % 97) as f64 / 31.0)]);
    }
    linhas.push(vec!["Total Geral".into(), String::new(), String::new(), String::new(), String::new(), "1.96".into()]);
    a.workbook.aba_mut(0).unwrap().sheet.gravar_bloco_literais(0, 0, linhas);
    a.linha = 0; a.coluna = 0; a.fim_selecao = (n - 1, 5);
    let _ = a.evento_grafico(Evento::Abrir);
    let _ = a.evento_grafico(Evento::Tipo(TipoUi::Linhas));
    let ver = |a: &App, passo: &str| {
        let svg = a.graficos.svg.as_deref().unwrap_or("");
        assert!(!svg.is_empty() && !svg.contains("indisponível"), "{passo}: sem desenho; mensagem: {}", a.graficos.mensagem);
    };
    ver(&a, "abrir");
    for c in [Some(4), None, Some(0), Some(1)] {
        let _ = a.evento_grafico(Evento::CategoriaColuna(c));
        assert_eq!(a.graficos.categorias.then_some(a.graficos.coluna_categoria), c, "a escolha {c:?} não ficou");
        ver(&a, &format!("categoria {c:?}"));
    }
    let _ = a.evento_grafico(Evento::Inserir);
    let valor = feathersheet_io::graficos_salvos::listar(&a.workbook.aba(0).unwrap().sheet).unwrap().remove(0);
    let svg = super::editor::svg_do_grafico_salvo(&a.workbook.aba(0).unwrap().sheet, &valor);
    assert!(svg.is_some(), "o gráfico gravado não desenha: {valor}");
}

/// Bancada (29/10/2026): gráfico de linhas de 12.664 linhas com o BLOCO ("4887.75 - 5000")
/// no Eixo X travava a janela. Mede quanto custa cada quadro do app inteiro.
/// `cargo test --profile medir --bin feathersheet medir_quadros_grafico_com_bloco -- --ignored --nocapture`
#[test]
#[ignore]
fn medir_quadros_grafico_com_bloco() {
    use pelicano::{Point, Size};
    let mut a = App::default();
    let n = 12_664u32;
    let mut linhas = vec![vec!["DATA".to_string(), "RAMAL".into(), "EH".into(), "LINHA".into(), "BLOCO".into(), "NIV_MAX_5M".into()]];
    for r in 0..n - 1 {
        let ini = 4887.75 + 7.5 * r as f64;
        linhas.push(vec!["06/08/2026".into(), "RLT".into(), format!("EH 0{}/03", 1 + r % 3),
            (1 + r % 2).to_string(), format!("{ini} - {}", ini + 112.25), format!("{}", 0.8 + (r % 97) as f64 / 31.0)]);
    }
    a.workbook.aba_mut(0).unwrap().sheet.gravar_bloco_literais(0, 0, linhas);
    a.linha = 0; a.coluna = 0; a.fim_selecao = (n - 1, 5);
    let quadro = |a: &App, rotulo: &str| {
        let tela = pelicano::Size::new(2000.0, 1125.0);
        let mut r = pelicano::Renderer::new();
        let st = pelicano::States::default();
        for _ in 0..2 {
            let t = std::time::Instant::now();
            let el = a.view(window::Id::MAIN);
            r.begin(tela, 1.28, pelicano::Color::BLACK);
            let arvore = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::dark());
            pelicano::draw(&el, &arvore, &mut r, &st, &pelicano::Theme::dark(), None, false);
            eprintln!("{rotulo}: {:?}", t.elapsed());
        }
    };
    quadro(&a, "sem gráfico");
    let _ = a.evento_grafico(Evento::Abrir);
    let _ = a.evento_grafico(Evento::Tipo(TipoUi::Linhas));
    let _ = a.evento_grafico(Evento::Inserir);
    let t = std::time::Instant::now();
    let _ = a.evento_grafico(Evento::CategoriaColuna(Some(4)));
    eprintln!("escolher BLOCO: {:?}", t.elapsed());
    let tela = Size::new(2000.0, 1125.0);
    let mut r = pelicano::Renderer::new();
    let st = pelicano::States::default();
    for i in 0..6 {
        let t = std::time::Instant::now();
        let el = a.view(window::Id::MAIN);
        let tv = t.elapsed();
        r.begin(tela, 1.28, pelicano::Color::BLACK);
        let arvore = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::dark());
        let tl = t.elapsed();
        pelicano::draw(&el, &arvore, &mut r, &st, &pelicano::Theme::dark(), Some(Point::new(1900.0, 600.0 + i as f32)), false);
        eprintln!("quadro {i}: view {tv:?} · layout {:?} · desenho {:?}", tl - tv, t.elapsed() - tl);
        drop(el);
        let _ = a.update(Msg::CursorJanela(window::Id::MAIN, Point::new(1900.0, 600.0 + i as f32)));
    }
}

/// #110: quanto a thread da janela gasta com os gráficos do relatório do usuário,
/// ao abrir (`adiantar_graficos`) e ao trocar para cada aba.
/// `FEATHERSHEET_XLSX=… cargo test --profile medir --bin feathersheet medir_graficos_ao_abrir_e_trocar -- --ignored --nocapture`
#[test]
#[ignore]
fn medir_graficos_ao_abrir_e_trocar() {
    let path = std::path::PathBuf::from(std::env::var("FEATHERSHEET_XLSX").expect("FEATHERSHEET_XLSX"));
    let bytes = std::fs::read(&path).unwrap();
    let mut a = App::default();
    let t = std::time::Instant::now();
    a.carregar(path, bytes).unwrap();
    eprintln!("carregar (com adiantar_graficos): {:?}", t.elapsed());
    for i in 0..a.workbook.len() {
        let t = std::time::Instant::now();
        let n = a.graficos_da_aba(i).len();
        eprintln!("aba {i} ({n} gráficos): {:?}", t.elapsed());
    }
    let tela = pelicano::Size::new(2000.0, 1125.0);
    let mut r = pelicano::Renderer::new();
    let st = pelicano::States::default();
    for i in (0..a.workbook.len()).chain([0, 1]) {
        let t = std::time::Instant::now();
        let _ = a.update(Msg::Aba(i));
        let el = a.view(window::Id::MAIN);
        r.begin(tela, 1.28, pelicano::Color::BLACK);
        let arvore = pelicano::layout(&el, tela, &mut r, &st, &pelicano::Theme::dark());
        pelicano::draw(&el, &arvore, &mut r, &st, &pelicano::Theme::dark(), None, false);
        eprintln!("trocar para a aba {i} e desenhar: {:?}", t.elapsed());
    }
}

/// Gráfico sobre uma dinâmica: o Total Geral (linha e coluna) não entra, como no Excel.
/// Numa aba sem dinâmica, uma linha "Total Geral" é dado da pessoa e fica.
#[test]
fn grafico_numa_aba_com_dinamica_deixa_o_total_geral_de_fora() {
    use feathersheet_core::grafico::chart::{ChartConfig, ChartRange, ChartType};
    let mut a = App::default();
    let _ = a.update(Msg::Colar("Item\tnovo (Valor)\tTotal Geral (Valor)\nA\t1\t1\nB\t2\t2\nTotal Geral\t3\t3".into()));
    let config = ChartConfig {
        tipo: ChartType::Column, range: ChartRange { r0: 0, c0: 0, r1: 3, c1: 2 },
        series_in_columns: true, has_headers: true, has_category_labels: true, ..Default::default()
    };
    let comum = super::dados_do_grafico(&a.workbook.aba(0).unwrap().sheet, &config, false, None);
    assert_eq!(comum.categories, ["A", "B", "Total Geral"]);
    assert_eq!(comum.series.len(), 2);
    a.workbook.aba_mut(0).unwrap().sheet.dn = Some("[]".into());
    let d = super::dados_do_grafico(&a.workbook.aba(0).unwrap().sheet, &config, false, None);
    assert_eq!(d.categories, ["A", "B"]);
    assert_eq!(d.series.len(), 1);
    assert_eq!(d.series[0].values, [1.0, 2.0]);
}
