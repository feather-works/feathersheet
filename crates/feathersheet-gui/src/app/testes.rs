//! Testes básicos de interface, menus, fórmulas e edição.
use super::*;

#[path = "testes/edicao_avancada.rs"]
mod edicao_avancada;

#[test]
fn duplo_clique_edita_diretamente_e_enter_confirma() {
    let mut app = App::default();
    app.workbook.escrever_celula(0, 1, 1, "Antes".into());
    let _ = app.update(Msg::EditarNaCelula(1, 1, 180.0, 58.0, 122.0, 28.0));
    assert!(app.edicao_inline.is_some());
    assert!(app.editor_formula.ativo);
    assert_eq!(app.edicao, "Antes");
    let _ = app.update(Msg::Editar("Depois".into()));
    let _ = app.update(Msg::EnviarEditor);
    assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 1), "Depois");
    assert_eq!((app.linha, app.coluna), (2, 1));
    assert!(!app.editor_formula.ativo);
    assert!(app.edicao_inline.is_none());
}

#[test]
fn esc_cancela_edicao_inline_sem_alterar_celula() {
    let mut app = App::default();
    app.workbook.escrever_celula(0, 0, 0, "Original".into());
    let _ = app.update(Msg::EditarNaCelula(0, 0, 90.0, 30.0, 122.0, 28.0));
    let _ = app.update(Msg::Editar("Descartar".into()));
    let _ = app.update(Msg::Cancelar);
    assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Original");
    assert_eq!(app.edicao, "Original");
    assert!(app.edicao_inline.is_none());
    assert!(!app.editor_formula.ativo);
}


#[test]
fn modelo_orcamento_domestico_preenche_pasta_pelo_motor_real() {
    use feathersheet_core::celulas::values::Value;
    let mut app = App::default();
    assert!(!app.alterado);
    let _ = app.update(Msg::NovoDeModelo("orcamento-domestico"));
    assert_eq!(app.workbook.nomes(), vec!["Orçamento".to_string()]);
    assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("Orçamento doméstico".into()));
}

    #[test]
    fn aviso_aba_excluida_aparece_na_barra_e_some_em_cinco_segundos() {
        use super::*;
        let mut app = App::default();
        // O status inicial não deve produzir banner permanente.
        assert!(app.aviso_abas.is_none());
        app.update(Msg::AdicionarAba);
        app.update(Msg::RemoverAba);
        assert_eq!(app.status, "Aba excluída");
        assert_eq!(app.aviso_abas.as_ref().map(|(s, _)| s.as_str()),
            Some("Aba excluída"));

        // O pulso existente não remove uma notificação ainda recente.
        app.update(Msg::MonitorPulso);
        assert!(app.aviso_abas.is_some());

        // Simula 5s passados sem precisar de sleep nem bloquear o teste.
        app.aviso_abas.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(6);
        app.update(Msg::MonitorPulso);
        assert!(app.aviso_abas.is_none());
        // A mensagem histórica continua disponível internamente.
        assert_eq!(app.status, "Aba excluída");
    }

    #[test]
    fn aviso_substituido_por_acao_mais_recente_e_sem_rodape_extra() {
        use super::*;
        let mut app = App::default();
        app.update(Msg::AdicionarAba);
        app.update(Msg::RemoverAba);
        let antigo = app.aviso_abas.as_ref().unwrap().1;
        app.aviso_abas.as_mut().unwrap().1 = antigo - Duration::from_secs(10);
        // Ação nova atualiza o texto e reinicia o prazo do banner.
        app.update(Msg::RemoverAba);
        let (mensagem, instante) = app.aviso_abas.as_ref().unwrap();
        assert_eq!(mensagem, "A última aba visível não pode ser excluída");
        assert!(instante.elapsed() < Duration::from_secs(5));
        app.update(Msg::MonitorPulso);
        assert!(app.aviso_abas.is_some());

        // Conferência do widget sem exigir backend GPU/Iced.
        let _abas: iced::Element<'_, Msg> = app.view_abas();
    }

    #[test]
    fn exclusoes_seguidas_com_status_igual_reiniciam_prazo() {
        use super::*;
        let mut app = App::default();
        app.update(Msg::AdicionarAba);
        app.update(Msg::AdicionarAba);
        app.update(Msg::RemoverAba);
        assert_eq!(app.status, "Aba excluída");
        app.aviso_abas.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(10);
        // A segunda exclusão emite o mesmo texto, mas é um novo evento.
        app.update(Msg::RemoverAba);
        let (mensagem, instante) = app.aviso_abas.as_ref().unwrap();
        assert_eq!(mensagem, "Aba excluída");
        assert!(instante.elapsed() < Duration::from_secs(5));
        assert_eq!(app.workbook.len(), 1);
    }

    #[test]
    fn ui013_ajuda_sem_regressao_no_workbook() {
        use super::*;
        let mut app=App::default();
        let abas=app.workbook.len();
        let celulas=app.workbook.aba(0).map(|a|a.sheet.len()).unwrap_or(0);
        let _=app.update(Msg::AjudaSecao(2));
        assert_eq!(app.ajuda_estado.secao,2);
        let _=app.update(Msg::AjudaEvento(feather_ui::ajuda::Evento::Buscar(
            "sheet.set".into()
        )));
        assert!(ajuda::modelo().quantidade_encontrada("sheet.set")>0);
        assert_eq!(app.ajuda_estado.busca,"sheet.set");
        let _=app.update(Msg::AjudaEvento(feather_ui::ajuda::Evento::Proximo));
        assert!(app.ajuda_estado.busca.is_empty());
        assert_eq!(app.workbook.len(),abas);
        assert_eq!(app.workbook.aba(0).map(|a|a.sheet.len()).unwrap_or(0),celulas);
    }
    #[test]
    fn imagem_pode_ser_arrastada_e_redimensionada_pelo_mouse() {
        use super::*;
        use iced::Point;
        use std::io::Cursor;

        let mut dados = Vec::new();
        let foto = image::RgbaImage::from_pixel(200, 100, image::Rgba([25, 80, 120, 220]));
        image::DynamicImage::ImageRgba8(foto)
            .write_to(&mut Cursor::new(&mut dados), image::ImageFormat::Png).unwrap();
        let mut app = App::default();
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((dados, "foto.png".into())))));
        let id = app.imagem_selecionada.as_ref().unwrap().1.clone();
        let dados_imagem = |app: &App| -> serde_json::Value {
            serde_json::from_str(
                app.workbook.aba(0).unwrap().sheet.im.as_ref().unwrap()
            ).unwrap()
        };
        let antes = dados_imagem(&app);
        assert_eq!(antes[0]["position"]["width"], 200.);
        assert_eq!(antes[0]["position"]["height"], 100.);
        app.alterado = false;

        // O clique inicia gesto; movimentação apenas pré-visualiza o estado,
        // não gera JSON nem suja o FSHZ a cada posição do cursor.
        let origem = Point::new(150., 120.);
        app.update(Msg::Grafico(graficos::Evento::Cursor(origem)));
        app.update(Msg::ImagemPressionar(
            id.clone(), imagens_importadas::Gesto::Mover,
        ));
        app.update(Msg::Grafico(graficos::Evento::Cursor(Point::new(180., 140.))));
        assert_eq!(dados_imagem(&app), antes, "arraste deve salvar só no mouse-up");
        assert!(!app.alterado);
        app.update(Msg::Grafico(graficos::Evento::Soltar));
        let movida = dados_imagem(&app);
        assert_eq!(movida[0]["position"]["x"], 60.);
        assert_eq!(movida[0]["position"]["y"], 44.);
        assert_eq!(movida[0]["src"], antes[0]["src"]);
        assert!(app.alterado);

        app.alterado = false;
        let antes_resize = movida.clone();
        app.update(Msg::Grafico(graficos::Evento::Cursor(origem)));
        app.update(Msg::ImagemPressionar(
            id.clone(), imagens_importadas::Gesto::Redimensionar,
        ));
        app.update(Msg::Grafico(graficos::Evento::Cursor(Point::new(230., 120.))));
        assert_eq!(dados_imagem(&app), antes_resize);
        app.update(Msg::Grafico(graficos::Evento::Soltar));
        let redimensionada = dados_imagem(&app);
        assert_eq!(redimensionada[0]["position"]["width"], 280.);
        assert_eq!(redimensionada[0]["position"]["height"], 140.);
        assert_eq!(redimensionada[0]["position"]["x"], 60.);
        assert_eq!(redimensionada[0]["position"]["y"], 44.);
        assert!(app.alterado);
        assert_eq!(redimensionada[0]["id"], id);
    }

    #[test]
    fn arrastar_imagem_cancelado_por_escape_nao_muda_fshz() {
        use super::*;
        use std::io::Cursor;
        let mut dados = Vec::new();
        let img = image::RgbaImage::from_pixel(80, 40, image::Rgba([100, 120, 140, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut dados), image::ImageFormat::Png).unwrap();
        let mut app = App::default();
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((dados, "teste.png".into())))));
        app.alterado = false;
        let antes = app.workbook.aba(0).unwrap().sheet.im.clone();
        let id = app.imagem_selecionada.as_ref().unwrap().1.clone();
        app.update(Msg::ImagemPressionar(id, imagens_importadas::Gesto::Mover));
        app.update(Msg::Grafico(graficos::Evento::Cursor(iced::Point::new(200., 200.))));
        app.update(Msg::Cancelar);
        assert!(app.arraste_imagem.is_none());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.im, antes);
        assert!(!app.alterado);
    }

    #[test]
    fn criar_tabela_pelo_botao_reconhece_cabecalho_e_salva_fshz() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Produto".into());
        app.workbook.escrever_celula(0, 0, 1, "Preco".into());
        app.workbook.escrever_celula(0, 1, 0, "Livro".into());
        app.workbook.escrever_celula(0, 1, 1, "25".into());
        app.update(Msg::Selecionar(0,0));
        app.update(Msg::TabelaAbrir);
        assert_eq!(app.tabela_area, Some(Rect::new(0,0,1,1)));
        assert!(app.tabela_cabecalho);
        assert_eq!(app.tabela_nome, "Tabela1");
        app.update(Msg::TabelaConfirmar);
        assert!(app.alterado);
        assert!(app.tabela_area.is_none());
        assert!(app.status.starts_with("Tabela Tabela1 criada"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,1), "25");
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook, 0).unwrap();
        let wb = feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        let tabelas: Vec<serde_json::Value> = serde_json::from_str(
            wb.aba(0).unwrap().sheet.tb.as_deref().unwrap()).unwrap();
        assert_eq!(tabelas[0]["nome"],"Tabela1");
        assert_eq!(tabelas[0]["area"]["r1"], 1);
    }

    #[test]
    fn nome_da_tabela_eh_editado_somente_ao_renomear() {
        let mut app = App::default();
        app.update(Msg::TabelaAbrir);
        app.update(Msg::TabelaConfirmar);
        assert!(app.tabela_editando.is_some());
        assert!(!app.tabela_renomeando);
        app.update(Msg::TabelaAcao(tabelas::Acao::IniciarRenomeacao));
        assert!(app.tabela_renomeando);
        app.update(Msg::TabelaAcao(tabelas::Acao::Nome("Descartado".into())));
        app.update(Msg::TabelaAcao(tabelas::Acao::CancelarRenomeacao));
        assert!(!app.tabela_renomeando);
        assert_eq!(app.tabela_nome_novo, "Tabela1");
        app.update(Msg::TabelaAcao(tabelas::Acao::IniciarRenomeacao));
        app.update(Msg::TabelaAcao(tabelas::Acao::Nome("Vendas2026".into())));
        app.update(Msg::TabelaAcao(tabelas::Acao::Renomear));
        assert!(!app.tabela_renomeando);
        assert_eq!(tabelas::sob_celula(&app.workbook, 0, 0, 0)
            .unwrap()["nome"], "Vendas2026");
        app.update(Msg::TabelaAcao(tabelas::Acao::IniciarRenomeacao));
        app.update(Msg::Selecionar(7, 7));
        assert!(!app.tabela_renomeando);
    }

    #[test]
    fn criar_tabela_vazia_e_cancelar_nao_corrompem_valores() {
        let mut app = App::default();
        app.update(Msg::TabelaAbrir);
        assert_eq!(app.tabela_area, Some(Rect::new(0,0,5,2)));
        app.update(Msg::TabelaCancelar);
        assert!(app.workbook.aba(0).unwrap().sheet.tb.is_none());
        assert!(!app.alterado);
        app.update(Msg::TabelaAbrir);
        app.update(Msg::TabelaConfirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0), "Coluna1");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,2), "Coluna3");
        app.update(Msg::TabelaAbrir);
        app.update(Msg::TabelaConfirmar);
        assert!(app.status.contains("cruza outra tabela"));
    }

    #[test]
    fn imagem_na_ribbon_insere_sem_painel_de_efeitos_ou_alterar_celulas() {
        use super::*;
        use std::io::Cursor;
        let mut dados = Vec::new();
        let pixels = image::RgbaImage::from_pixel(10, 6, image::Rgba([20, 90, 135, 245]));
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut Cursor::new(&mut dados), image::ImageFormat::Png)
            .unwrap();

        let mut app = App::default();
        app.update(Msg::Selecionar(2, 3));
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((dados, "foto.png".into())))));
        assert!(!app.efeitos_imagem.aberto, "Importar não deve abrir o editor");
        assert!(app.alterado, "Importar deve marcar a pasta como modificada");
        assert!(app.status.starts_with("Imagem inserida na planilha:"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2,3), "");
        let imagens: serde_json::Value = serde_json::from_str(
            app.workbook.aba(0).unwrap().sheet.im.as_deref().unwrap()
        ).unwrap();
        assert_eq!(imagens.as_array().unwrap().len(), 1);
        assert_eq!(imagens[0]["name"], "foto.png");
        assert!(imagens[0]["src"].as_str().unwrap().starts_with("data:image/png;base64,"));
        // Um elemento sobreposto não pode mover a barra de fórmulas nem o histórico de células.
        let _: iced::Element<'_, Msg> = app.grade_com_imagens();
    }

    #[test]
    fn efeitos_editam_raster_importado_sem_tocar_celulas_e_persistem_no_fshz() {
        use std::io::Cursor;
        let mut dados = Vec::new();
        let pixels = image::RgbaImage::from_pixel(5, 4, image::Rgba([150, 50, 20, 128]));
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut Cursor::new(&mut dados), image::ImageFormat::Png).unwrap();
        let mut app = App::default();
        app.update(Msg::Selecionar(3, 2));
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((dados, "efeito.png".into())))));
        let antes: serde_json::Value = serde_json::from_str(
            app.workbook.aba(0).unwrap().sheet.im.as_deref().unwrap()
        ).unwrap();
        assert!(!app.efeitos_imagem.aberto, "Inserir não abre editor automaticamente");
        app.update(Msg::ImagemEfeitosAbrir);
        assert!(app.efeitos_imagem.aberto);
        assert!(app.efeitos_imagem.imagem_id.is_some());
        let _: Element<'_, Msg> = app.efeitos_imagem.painel();

        app.update(Msg::ImagemLaboratorioAplicar("cinza"));
        assert!(app.status.starts_with("Efeito aplicado à imagem"));
        let depois: serde_json::Value = serde_json::from_str(
            app.workbook.aba(0).unwrap().sheet.im.as_deref().unwrap()
        ).unwrap();
        assert_eq!(depois[0]["id"], antes[0]["id"]);
        assert_eq!(depois[0]["position"], antes[0]["position"]);
        assert_ne!(depois[0]["src"], antes[0]["src"]);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 2), "");

        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook, 0).unwrap();
        let reaberta = feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap()
        );
        let fshz: serde_json::Value = serde_json::from_str(
            reaberta.aba(0).unwrap().sheet.im.as_deref().unwrap()
        ).unwrap();
        assert_eq!(fshz, depois);
        app.update(Msg::ImagemLaboratorioAplicar("original"));
        let original: serde_json::Value = serde_json::from_str(
            app.workbook.aba(0).unwrap().sheet.im.as_deref().unwrap()
        ).unwrap();
        assert_eq!(original[0]["position"], antes[0]["position"]);
        app.update(Msg::ImagemLaboratorioFechar);
        assert!(!app.efeitos_imagem.aberto);
    }

    #[test]
    fn importar_imagem_invalida_nao_modifica_aba() {
        use super::*;
        let mut app = App::default();
        app.update(Msg::ImagemLaboratorioCarregada(Ok(Some((
            "não é PNG".as_bytes().to_vec(), "quebrado.png".into(),
        )))));
        assert!(app.workbook.aba(0).unwrap().sheet.im.is_none());
        assert!(!app.alterado);
        assert!(!app.efeitos_imagem.aberto);
        assert!(!app.status.is_empty());
    }


    #[test]
    fn ui009_popup_do_viewport_e_scroll_preservam_formula_e_selecao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 3, 4, "=SOMA(A1:A2)".into());
        app.update(Msg::Selecionar(3, 4));
        let original = app.workbook.aba(0).unwrap().sheet.get_raw(3, 4).to_owned();
        let popup = crate::geometria_overlay::PosicaoPopup::contexto(
            295.0, 175.0, 300.0, 180.0,
        ).unwrap();
        app.update(Msg::AbrirContextoNaGrade(Contexto::Celula(3, 4), popup));
        // Largura é LARGURA_MENU (272), não a do viewport: 272 cabe em 300.
        assert_eq!(app.contexto_tamanho, (popup.largura, popup.altura));
        assert_eq!(app.contexto_tamanho, (272.0, 180.0));
        assert!(matches!(
            app.contexto,
            Some((Contexto::Celula(3, 4), x, y)) if x == popup.x && y == popup.y
        ));
        app.update(Msg::Rolar(10, 7));
        assert!(app.contexto.is_none(), "popup não pode ficar sobre a célula após scroll");
        assert_eq!((app.linha_inicio, app.coluna_inicio), (10, 7));
        assert_eq!((app.linha, app.coluna), (3, 4));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 4), original);
    }

    #[test]
    fn ui009_esc_fecha_popup_sem_alterar_dados_e_tem_tamanho_padrao_na_aba() {
        use iced::keyboard::{Key, Modifiers, key::Named};
        let mut app = App::default();
        let id = window::Id::unique();
        app.janelas.principal = Some(id);
        app.update(Msg::AbrirContexto(Contexto::Celula(0, 0), 8.0, 8.0));
        assert_eq!(app.contexto_tamanho, (272.0, 340.0));
        assert!(app.contexto.is_some());
        app.update(Msg::Teclado(id, Key::Named(Named::Escape), Modifiers::default()));
        assert!(app.contexto.is_none());
        assert_eq!((app.linha, app.coluna), (0, 0));
    }

    #[test]
    fn atalhos_globais_da_suite_nao_editam_celulas() {
        use iced::keyboard::{key::Named, Key, Modifiers};
        let mut app = App::default();
        let id = iced::window::Id::unique();
        app.janelas.principal = Some(id);
        let texto = app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).to_owned();
        let _ = app.update(Msg::Teclado(id, Key::Named(Named::F1), Modifiers::default()));
        assert!(app.janelas.aberta(Auxiliar::Ajuda));
        let ajuda_id = app.janelas.id(Auxiliar::Ajuda).unwrap();
        // F1 deve conseguir construir a janela de Ajuda sem panic no scrollable.
        let _: Element<'_, Msg> = app.view(ajuda_id);
        // A mesma tecla logo em seguida é a repetição/troca de foco do X11: ignorada.
        let _ = app.update(Msg::Teclado(ajuda_id, Key::Named(Named::F1), Modifiers::default()));
        assert!(app.janelas.aberta(Auxiliar::Ajuda), "F1 repetido não fecha na hora");
        app.ultima_alternancia = None;
        let _ = app.update(Msg::Teclado(ajuda_id, Key::Named(Named::F1), Modifiers::default()));
        assert!(!app.janelas.aberta(Auxiliar::Ajuda));
        let _ = app.update(Msg::Teclado(id, Key::Named(Named::F12), Modifiers::default()));
        assert!(app.janelas.aberta(Auxiliar::Monitor));
        let monitor_id = app.janelas.id(Auxiliar::Monitor).unwrap();
        let _ = app.update(Msg::Teclado(monitor_id, Key::Named(Named::Escape), Modifiers::default()));
        assert!(!app.janelas.aberta(Auxiliar::Monitor));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), texto);
    }
    #[test]
    fn menu_de_funcoes_insere_no_cursor_sem_perder_formula_existente() {
        let mut app = App::default();
        app.update(Msg::Digitar("=SOMA(A1)+".into()));
        let fim = app.edicao.chars().count();
        app.editor_formula.posicionar(&app.edicao, fim);
        app.update(Msg::InserirFuncao("MÉDIA".into()));
        assert_eq!(app.edicao, "=SOMA(A1)+MÉDIA(");
    }

    #[test]
    fn botao_direito_preserva_selecao_e_executa_no_motor() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 2, 3, "Teste".into());
        app.update(Msg::AbrirContexto(Contexto::Celula(2, 3), 40.0, 40.0));
        assert_eq!((app.linha, app.coluna), (2, 3));
        assert!(matches!(app.contexto, Some((Contexto::Celula(2, 3), _, _))));
        let indice = app.opcoes_contexto(Contexto::Celula(2, 3))
            .iter().position(|o| o.as_ref().is_some_and(|(titulo, _)| titulo == "Limpar conteúdo"))
            .unwrap();
        app.update(Msg::ContextoEscolher(indice));
        assert!(app.contexto.is_none());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 3), "");
    }

    #[test]
    fn menu_contextual_seleciona_linha_coluna_e_aba_corretas() {
        let mut app = App::default();
        let _ = app.update(Msg::AbrirContexto(Contexto::Linha(5), 12.0, 24.0));
        assert_eq!(app.linha, 5);
        assert_eq!(app.fim_selecao.1, MAX_COLS - 1);
        assert!(matches!(app.contexto, Some((Contexto::Linha(5), _, _))));

        let _ = app.update(Msg::AbrirContexto(Contexto::Coluna(3), 12.0, 24.0));
        assert_eq!(app.coluna, 3);
        assert_eq!(app.fim_selecao.0, MAX_ROWS - 1);
        assert!(matches!(app.contexto, Some((Contexto::Coluna(3), _, _))));

        let _ = app.update(Msg::AdicionarAba);
        let _ = app.update(Msg::Aba(0));
        let _ = app.update(Msg::AbrirContexto(Contexto::Aba(1), 12.0, 24.0));
        assert_eq!(app.folha, 1);
        assert!(matches!(app.contexto, Some((Contexto::Aba(1), _, _))));
    }

    #[test]
    fn contexto_especifico_diferencia_cabecalhos_e_aba() {
        let app = App::default();
        let rotulos = |tipo| app.opcoes_contexto(tipo).into_iter()
            .flatten().map(|(titulo, _)| titulo).collect::<Vec<_>>();
        assert!(rotulos(Contexto::Linha(0)).iter().any(|t| t == "Excluir linha"));
        assert!(rotulos(Contexto::Coluna(0)).iter().any(|t| t == "Excluir coluna"));
        assert!(rotulos(Contexto::Aba(0)).iter().any(|t| t == "Renomear aba"));
        assert!(!rotulos(Contexto::Aba(0)).iter().any(|t| t == "Excluir aba"));
    }

    #[test]
    fn menu_nativo_conecta_acao_sem_depender_do_rotulo() {
        use menu_nativo::Acao;
        let app = App::default();
        let executar = |id: &str, args: Vec<serde_json::Value>| {
            app.evento_menu(&Acao { id: id.into(), args })
        };
        assert!(matches!(executar("save", vec![]), Some(Msg::Salvar)));
        assert!(matches!(executar("openFile", vec![]), Some(Msg::Abrir)));
        assert!(matches!(executar("clipboard", vec![serde_json::json!("cut")]), Some(Msg::Recortar)));
        assert!(matches!(executar("setZoom", vec![serde_json::json!(1.25)]), Some(Msg::Zoom(125))));
        assert!(matches!(executar("insertFunction", vec![serde_json::json!("SOMA")]),
            Some(Msg::InserirFuncao(ref n)) if n == "SOMA"));
        assert!(matches!(executar("setFormat", vec![serde_json::json!("number")]),
            Some(Msg::EstiloMenu(_, Campo::Texto(ref valor))) if valor == "number"));
        assert!(matches!(executar("createPivotTable", vec![]),
            Some(Msg::Dinamica(dinamica_ui::Evento::Abrir))));
        assert!(matches!(executar("exportar", vec![serde_json::json!("csv")]),
            Some(Msg::ExportarDelimitado(false))));
        assert!(matches!(executar("exportar", vec![serde_json::json!("tsv")]),
            Some(Msg::ExportarDelimitado(true))));
    }

    #[test]
    fn menu_typescript_restaurado_mostra_nove_categorias_e_submenus() {
        let mut app = App::default();
        let catalogo = menu_nativo::catalogo();
        let esperadas = [
            "Arquivo", "Editar", "Ver", "Inserir", "Formatar",
            "Dados", "Macros", "Ferramentas", "Ajuda",
        ];
        assert_eq!(catalogo.menus.len(), esperadas.len());
        for (menu, titulo) in catalogo.menus.iter().zip(esperadas) {
            assert_eq!(menu.titulo, titulo);
        }
        // Exercita a Ribbon Iced real: menu, categoria, submenu e voltar.
        {
            // O Element toma empréstimo imutável do App. Soltar antes de
            // disparar as mutações do teste evita manter borrow ativo.
            let _menu: Element<'_, Msg> = app.view_menu_typescript();
        }
        app.update(Msg::Menu);
        assert!(app.menu_aberto);
        let ver = catalogo.menus.iter().position(|m| m.titulo == "Ver").unwrap();
        app.update(Msg::MenuGrupo(ver));
        let mostrar = menu_nativo::itens(ver, &[]).unwrap()
            .iter().position(|v| v.rotulo == "Mostrar").unwrap();
        app.update(Msg::MenuEscolher(mostrar));
        assert_eq!(app.menu_caminho, vec![mostrar]);
        let itens = menu_nativo::itens(ver, &app.menu_caminho).unwrap();
        assert!(itens.iter().any(|v| v.acao.as_ref()
            .is_some_and(|a| a.id == "toggleRealceEixos")));
        app.update(Msg::MenuVoltar);
        assert!(app.menu_caminho.is_empty());
        app.update(Msg::FecharMenu);
        assert!(!app.menu_aberto);
    }

    #[test]
    fn mostrar_formulas_menu_toggle_e_dados_nao_mutados() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "5".into());
        app.workbook.escrever_celula(0, 0, 1, "=A1*3".into());
        let acao = menu_nativo::Acao {
            id: "toggleShowFormulas".into(), args: vec![],
        };
        let menu = menu_nativo::catalogo().menus.iter()
            .find(|menu| menu.titulo == "Ver").unwrap();
        let item = menu.itens.iter().find(|item| item.rotulo == "Mostrar").unwrap()
            .filhos.iter().find(|item| item.rotulo == "Fórmulas").unwrap();
        assert!(app.item_menu_disponivel(item));
        assert!(!app.item_menu_marcado(item));
        assert!(matches!(app.evento_menu(&acao), Some(Msg::AlternarMostrarFormulas)));
        let antes = app.workbook.aba(0).unwrap().sheet.get_raw(0, 1);
        let resultado = app.workbook.valor(0, 0, 1);
        let alterado = app.alterado;
        app.update(Msg::AlternarMostrarFormulas);
        assert!(app.mostrar_formulas);
        assert!(app.item_menu_marcado(item));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 1), antes);
        assert_eq!(app.workbook.valor(0, 0, 1), resultado);
        assert_eq!(app.alterado, alterado);
        app.update(Msg::AlternarMostrarFormulas);
        assert!(!app.mostrar_formulas);
        assert!(!app.item_menu_marcado(item));
    }

    #[test]
    fn atalho_typescript_ctrl_crase_tem_identificador_iced() {
        let combo = feather_ui::atalhos::combo_de(
            &keyboard::Key::Character("`".into()), keyboard::Modifiers::CTRL,
        );
        assert_eq!(combo.as_deref(), Some("Ctrl+`"));
    }

    #[test]
    fn menu_conecta_acoes_rust_existentes_sem_falsos_atalhos() {
        use menu_nativo::Acao;
        let mut app = App::default();
        let executar = |app: &App, id: &str, args: Vec<serde_json::Value>| {
            app.evento_menu(&Acao { id: id.to_owned(), args })
        };
        assert!(matches!(executar(&app, "pasteValues", vec![]),
            Some(Msg::ColarValoresSolicitar)));
        assert!(matches!(executar(&app, "pasteFormats", vec![]),
            Some(Msg::ColarFormatoSolicitar)));
        assert!(matches!(executar(&app, "pasteTranspose", vec![]),
            Some(Msg::ColarEspecialSolicitar(ModoColagem::Transposta))));
        assert!(matches!(executar(&app, "pasteValuesFormats", vec![]),
            Some(Msg::ColarEspecialSolicitar(ModoColagem::ValoresComFormato))));
        assert!(matches!(executar(&app, "pasteFormulas", vec![]),
            Some(Msg::ColarEspecialSolicitar(ModoColagem::FormulasSemFormato))));
        assert!(matches!(executar(&app, "pasteFormulasFormats", vec![]),
            Some(Msg::ColarEspecialSolicitar(ModoColagem::Completa))));
        assert!(matches!(executar(&app, "insertImage", vec![]),
            Some(Msg::ImagemLaboratorioAbrir)));
        assert!(matches!(executar(&app, "createTable", vec![]),
            Some(Msg::TabelaAbrir)));
        assert!(matches!(executar(&app, "alternatingColors", vec![]),
            Some(Msg::CondicionalCoresAlternadas)));
        assert!(matches!(executar(&app, "clearFormat", vec![]),
            Some(Msg::LimparFormato)));
        assert!(matches!(executar(&app, "autoFitColumn", vec![]),
            Some(Msg::AjustarLargura)));
        assert!(matches!(executar(&app, "toggleFormulaBar", vec![]),
            Some(Msg::AlternarBarraFx)));
        assert!(matches!(executar(&app, "openFunctions", vec![]),
            Some(Msg::MostrarCatalogoFuncoes)));
        assert!(matches!(executar(&app, "condicional",
            vec![serde_json::json!("limpar-selecao")]),
            Some(Msg::CondicionalLimparSelecao)));
        assert!(matches!(executar(&app, "condicional",
            vec![serde_json::json!("limpar-planilha")]),
            Some(Msg::CondicionalLimparTudo)));
        assert!(matches!(executar(&app, "condicional",
            vec![serde_json::json!("nova")]),
            Some(Msg::CondicionalMostrar)));
        assert!(matches!(executar(&app, "createPivotTable", vec![]),
            Some(Msg::Dinamica(dinamica_ui::Evento::Abrir))));
        assert!(matches!(executar(&app, "splitText", vec![]),
            Some(Msg::AbrirAssistenteDivisao)));
        assert!(matches!(executar(&app, "toggleFilter", vec![]),
            Some(Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro))));
        assert!(matches!(executar(&app, "home", vec![]),
            Some(Msg::MenuPagina(menu_principal::Pagina::Inicio))));
        assert!(matches!(executar(&app, "fullscreen", vec![]), Some(Msg::TelaInteira)));
        assert!(matches!(executar(&app, "renameFile", vec![]), Some(Msg::RenomearArquivo)));
        assert!(matches!(executar(&app, "duplicateSheet", vec![]), Some(Msg::DuplicarAba)));
        // Mesclar alterna: só o item correspondente ao estado da seleção fica ligado.
        assert!(matches!(executar(&app, "mesclar", vec![]), Some(Msg::Mesclar)));
        assert!(executar(&app, "desmesclar", vec![]).is_none());
        app.update(Msg::Estender(0, 2));
        app.update(Msg::Mesclar);
        assert!(matches!(executar(&app, "desmesclar", vec![]), Some(Msg::Mesclar)));
        assert!(executar(&app, "mesclar", vec![]).is_none());
        let realce = menu_nativo::Acao { id: "toggleRealceEixos".into(), args: vec![] };
        assert!(matches!(app.evento_menu(&realce), Some(Msg::RealceEixos(false))));
        app.update(Msg::RealceEixos(false));
        assert!(matches!(app.evento_menu(&realce), Some(Msg::RealceEixos(true))));
        let itens = menu_nativo::catalogo().menus.iter()
            .find(|m| m.titulo == "Editar").unwrap().itens.iter()
            .find(|i| i.rotulo == "Colar especial").unwrap();
        let formato = itens.filhos.iter().find(|i| i.rotulo == "Somente formatação").unwrap();
        assert!(formato.atalho.is_empty(), "Não prometer atalho inexistente");
        assert!(app.item_menu_disponivel(formato));
    }

    #[test]
    fn ui010_mouse_selecao_arrasto_e_scroll_nao_modificam_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 2, 3, "FeatherOffice".into());
        let _ = app.update(Msg::Selecionar(2, 3));
        let _ = app.update(Msg::Estender(5, 6));
        let area = app.area_selecionada();
        assert_eq!((area.r0, area.c0, area.r1, area.c1), (2, 3, 5, 6));
        let antes = app.workbook.aba(0).unwrap().sheet.get_raw(2, 3).to_string();
        let _ = app.update(Msg::Rolar(12, 8));
        assert_eq!((app.linha_inicio, app.coluna_inicio), (12, 8));
        let apos = app.area_selecionada();
        assert_eq!((apos.r0, apos.c0, apos.r1, apos.c1), (area.r0, area.c0, area.r1, area.c1));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 3), antes);
    }

    #[test]
    fn ui010_foco_da_barra_de_formulas_nao_dispara_atalho_global() {
        use iced::keyboard::{key::Named, Key, Modifiers};
        let mut app = App::default();
        let id = window::Id::unique();
        app.janelas.principal = Some(id);
        // Tecla oriunda da caixa de texto simples não navega a planilha
        // nem abre o Monitor. A mesma tecla na janela principal abre F12.
        let _ = app.update(Msg::TecladoNoCampo(
            id, Key::Named(Named::F12), Modifiers::default(),
        ));
        assert!(!app.janelas.aberta(Auxiliar::Monitor));
        let _ = app.update(Msg::Teclado(
            id, Key::Named(Named::F12), Modifiers::default(),
        ));
        assert!(app.janelas.aberta(Auxiliar::Monitor));
        let monitor = app.janelas.id(Auxiliar::Monitor).unwrap();
        let _ = app.update(Msg::Teclado(monitor, Key::Named(Named::Escape), Modifiers::default()));
        assert!(!app.janelas.aberta(Auxiliar::Monitor));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "");
    }

    #[test]
    fn menu_inserir_aba_nao_move_dados_da_linha_anterior() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Primeira linha".into());
        app.inserir_menu(Estrutura::InserirLinha, true);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Primeira linha");
    }

    #[test]
    fn ctrl_seta_aponta_para_fim_do_intervalo_continuo() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"1".into());
        app.workbook.escrever_celula(0,1,0,"2".into());
        app.workbook.escrever_celula(0,2,0,"3".into());
        assert_eq!(app.destino_ctrl_seta_de((0,0),1,0),(2,0));
    }
    #[test]
    fn ctrl_shift_setas_selecionam_o_bloco_inteiro_como_no_excel() {
        let mut app=App::default();
        for r in 0..4 { for c in 0..3 { app.workbook.escrever_celula(0,r,c,"x".into()); } }
        // Ctrl+Shift+→ a partir de A1 vai até C1…
        let fim=app.destino_ctrl_seta(0,1,true);
        app.update(Msg::Estender(fim.0,fim.1));
        assert_eq!(app.fim_selecao,(0,2));
        // …e Ctrl+Shift+↓ sai da PONTA (C1), não de A1: A1:C4.
        let fim=app.destino_ctrl_seta(1,0,true);
        app.update(Msg::Estender(fim.0,fim.1));
        let area=app.area_selecionada();
        assert_eq!((area.r0,area.c0,area.r1,area.c1),(0,0,3,2));
    }
    #[test]
    fn enter_confirma_formula_e_move_para_celula_abaixo() {
        let mut app=App::default();
        app.update(Msg::Selecionar(4,2));
        app.update(Msg::Editar("=1+2".into()));
        app.update(Msg::EnviarEditor);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(4,2),"=1+2");
        assert_eq!((app.linha,app.coluna),(5,2));
        assert!(!app.editor_formula.ativo);
        assert_eq!(app.edicao,"");
    }
    #[test]
    fn enter_confirma_texto_e_check_nao_move() {
        let mut app=App::default();
        app.update(Msg::Editar("abc".into()));
        app.update(Msg::EnviarEditor);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"abc");
        assert_eq!((app.linha,app.coluna),(1,0));
        app.update(Msg::Editar("xyz".into()));
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"xyz");
        assert_eq!((app.linha,app.coluna),(1,0));
        assert!(!app.editor_formula.ativo);
    }
    #[test]
    fn campo_fx_selecionado_substitui_so_a_referencia_marcada() {
        let mut app=App::default();
        app.update(Msg::Editar("=A1+B2+C3".into()));
        app.update(Msg::SelecionarTextoFx(4,6));
        app.update(Msg::Selecionar(5,3));
        assert_eq!(app.edicao,"=A1+D6+C3");
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"=A1+D6+C3");
    }
    #[test]
    fn f4_pela_interface_altera_apenas_a_referencia_sob_o_cursor() {
        let mut app=App::default();
        app.update(Msg::Editar("=A1+B2+C3".into()));
        app.update(Msg::PosicionarCursorFx(3));
        let id=iced::window::Id::unique();
        app.update(Msg::Teclado(id,keyboard::Key::Named(keyboard::key::Named::F4),
            keyboard::Modifiers::default()));
        assert_eq!(app.edicao,"=$A$1+B2+C3");
        assert_eq!(app.editor_formula.posicao(&app.edicao),5);
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"=$A$1+B2+C3");
    }
    #[test]
    fn edicao_em_meio_preserva_texto_da_formula_e_posicao() {
        let mut app=App::default();
        app.update(Msg::Editar("=SOMA(;B2)".into()));
        app.editor_formula.posicionar(&app.edicao,6);
        app.update(Msg::Selecionar(0,0));
        assert_eq!(app.edicao,"=SOMA(A1;B2)");
        assert_eq!(app.editor_formula.posicao(&app.edicao),8);
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"=SOMA(A1;B2)");
    }
    #[test]
    fn escape_em_formula_entre_abas_retorna_a_origem_sem_gravar() {
        let mut app=App::default();
        let outra=app.workbook.criar_aba("Dados");
        app.update(Msg::Selecionar(9,1));
        app.update(Msg::Editar("=SOMA(".into()));
        app.update(Msg::Aba(outra));
        app.update(Msg::Selecionar(5,4));
        app.update(Msg::Cancelar);
        assert_eq!(app.folha,0);
        assert_eq!((app.linha,app.coluna),(9,1));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(9,1),"");
        assert!(!app.editor_formula.ativo);
    }
    #[test]
    fn cabecalhos_de_outra_aba_escrevem_colunas_e_linhas_inteiras() {
        let mut app=App::default();
        let outra=app.workbook.criar_aba("modulos");
        app.workbook.escrever_celula(outra,2,2,"BYD".into());
        app.workbook.escrever_celula(outra,2,3,"450".into());
        app.workbook.escrever_celula(0,1,1,"BYD".into());
        app.update(Msg::Editar("=PROCV(B2;".into()));
        app.update(Msg::Aba(outra));
        app.update(Msg::SelecionarColuna(2));
        app.update(Msg::EstenderColuna(8));
        assert_eq!(app.edicao,"=PROCV(B2;'modulos'!C:I");
        app.update(Msg::Editar(format!("{};2;FALSO)",app.edicao)));
        app.update(Msg::Confirmar);
        assert_eq!(app.folha,0);
        assert_eq!(app.workbook.valor(0,0,0),feathersheet_core::Value::Number(450.));
        app.update(Msg::Selecionar(0,4));
        app.update(Msg::Editar("=SOMA(".into()));
        app.update(Msg::SelecionarLinha(2));
        assert_eq!(app.edicao,"=SOMA(3:3");
    }
    #[test]
    fn formula_pode_apontar_em_outra_aba_sem_gravar_na_aba_destino() {
        let mut app=App::default();
        let outra=app.workbook.criar_aba("Base de Vendas");
        app.workbook.escrever_celula(outra,1,2,"2".into());
        app.workbook.escrever_celula(outra,3,4,"3".into());
        app.update(Msg::Editar("=SOMA(".into()));
        app.update(Msg::Aba(outra));
        assert_eq!(app.folha,outra);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"");
        app.update(Msg::Selecionar(1,2));
        app.update(Msg::Estender(3,4));
        assert_eq!(app.edicao,"=SOMA('Base de Vendas'!C2:E4");
        app.update(Msg::Editar(format!("{})",app.edicao)));
        app.update(Msg::Confirmar);
        assert_eq!(app.folha,0);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),
            "=SOMA('Base de Vendas'!C2:E4)");
        assert_eq!(app.workbook.aba(outra).unwrap().sheet.get_raw(0,0),"");
        assert_eq!(app.workbook.valor(0,0,0),feathersheet_core::Value::Number(5.));
    }
    #[test]
    fn digitacao_mostra_rascunho_sem_gravar_ate_confirmar() {
        let mut app=App::default();
        app.update(Msg::Editar("123".into()));
        assert!(app.editor_formula.ativo);
        assert_eq!(app.edicao,"123");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"");
        app.update(Msg::Cancelar);
        assert_eq!(app.edicao,"");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"");
        app.update(Msg::Editar("456".into()));
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"456");
    }
    #[test]
    fn escolher_celulas_monta_formula_sem_trocar_celula_ativa() {
        let mut app=App::default();
        app.update(Msg::Editar("=SOMA(".into()));
        app.update(Msg::Selecionar(1,1));
        assert_eq!(app.edicao,"=SOMA(B2");
        assert_eq!((app.linha,app.coluna),(0,0));
        app.update(Msg::Estender(3,2));
        assert_eq!(app.edicao,"=SOMA(B2:C4");
        assert_eq!((app.linha,app.coluna),(0,0));
        app.update(Msg::Editar(format!("{}{}",app.edicao,")")));
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"=SOMA(B2:C4)");
    }
    #[test]
    fn seletor_de_cor_atualiza_estilo_com_rgb_e_alfa() {
        let mut app = App::default();
        let _ = app.update(Msg::AbrirSeletorCor(AlvoCor::Fundo));
        assert_eq!(app.cor_ativa, Some(AlvoCor::Fundo));
        let _ = app.update(Msg::SeletorCor(EventoCor::Escolhida("#d93025".into())));
        assert_eq!(app.cor_da_celula(AlvoCor::Fundo), "#d93025");
        let _ = app.update(Msg::SeletorCor(EventoCor::Alfa(0.5)));
        assert_eq!(app.cor_da_celula(AlvoCor::Fundo).len(), 9);
        let _ = app.update(Msg::SeletorCor(EventoCor::TextoHex("#abcdef".into())));
        assert_eq!(app.cor_da_celula(AlvoCor::Fundo), "#abcdef");
        let _ = app.update(Msg::SeletorCor(EventoCor::Escolhida(String::new())));
        assert_eq!(app.cor_da_celula(AlvoCor::Fundo), "");
        let _ = app.update(Msg::SeletorCor(EventoCor::Fechar));
        assert!(app.cor_ativa.is_none());
    }

    #[test]
    fn ajuda_procx_ativa_arg_quinto_abaixo_da_barra_fx() {
        let mut app = App::default();
        let _ = app.update(Msg::Editar("=PROCX(C7;B4:B5;C4:C5;\"\";".into()));
        let (assinatura, indice, argumento, descricao) = app.dica_funcao_ativa().unwrap();
        assert!(assinatura.starts_with("PROCX(procurado;"), "{assinatura}");
        assert_eq!(indice, 5);
        assert_eq!(argumento, "[modo]");
        assert!(descricao.contains("Busca moderna"));
    }

    #[test]
    fn ajuda_sempre_segue_a_funcao_mais_interna() {
        let mut app = App::default();
        let _ = app.update(Msg::Editar("=SOMA(SE(A1>0;".into()));
        let (assinatura, indice, _, _) = app.dica_funcao_ativa().unwrap();
        assert!(assinatura.starts_with("SE("), "{assinatura}");
        assert_eq!(indice, 2);
        let _ = app.update(Msg::Cancelar);
        assert!(app.dica_funcao_ativa().is_none());
    }

    #[test]
    fn sugestoes_podem_ser_inseridas_antes_de_confirmar_formula() {
        let mut app=App::default();
        app.update(Msg::Editar("=SOM".into()));
        let nome=app.editor_formula.sugestoes(&app.edicao,app.config.idioma=="en-US");
        assert!(nome.iter().any(|s|s.name=="SOMA"));
        let idx=nome.iter().position(|s|s.name=="SOMA").unwrap();
        app.update(Msg::AceitarSugestao(idx));
        assert_eq!(app.edicao,"=SOMA(");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"");
    }
    #[test]
    fn fechar_com_edicao_pendente_pede_confirmacao() {
        let mut app = App::default();
        let _ = app.update(Msg::Editar("123".into()));
        let _ = app.update(Msg::Novo);
        assert_eq!(app.pendente, Some(Pendente::Novo));
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Number(123.));
        let _ = app.update(Msg::DestinoSalvar(None));
        assert_eq!(app.pendente, None);
        assert!(app.alterado);
    }

    #[test]
    fn selecao_invertida_copia_formata_limpa_e_desfaz() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("1\t2\n3\t4".into()));
        let _ = app.update(Msg::IrPara("B2:A1".into()));
        assert_eq!(app.area_selecionada(), Rect::new(0,0,1,1));
        assert_eq!(app.copiar_selecao().unwrap(), "1\t2\n3\t4");
        let _ = app.update(Msg::Formatar(Formatacao::Negrito));
        for r in 0..2 { for c in 0..2 {
            assert!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(r,c).unwrap().bold);
        }}
        let _ = app.update(Msg::Limpar);
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Empty);
        assert_eq!(app.workbook.valor(0,1,1), feathersheet_core::Value::Empty);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Number(1.));
        assert_eq!(app.workbook.valor(0,1,1), feathersheet_core::Value::Number(4.));
    }

    #[test]
    fn copiar_colar_preserva_texto_com_quebras_e_aspas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0,0,0,"linha 1\nlinha 2\t\"texto\"".into());
        app.trocar_selecao(0,0);
        let dados = app.copiar_selecao().unwrap();
        let _ = app.update(Msg::Selecionar(1,0));
        let _ = app.update(Msg::Colar(dados));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0), "linha 1\nlinha 2\t\"texto\"");
    }


    #[test]
    fn copiar_colar_celula_preserva_formula_e_personalizacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=2+3".into());
        let mut estilo = EstiloParcial::new();
        estilo.insert("bold".into(), Campo::Logico(true));
        estilo.insert("color".into(), Campo::Texto("#123456".into()));
        estilo.insert("bg".into(), Campo::Texto("#FFDD00".into()));
        estilo.insert("format".into(), Campo::Texto("currency".into()));
        let mut bordas = std::collections::BTreeMap::new();
        bordas.insert("bottom".into(), Campo::Logico(true));
        estilo.insert("borders".into(), Campo::Objeto(bordas));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::new(0, 0, 0, 0), &estilo));

        let original = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0);
        let dados = app.copiar_selecao().unwrap();
        let _ = app.update(Msg::Selecionar(3, 2));
        let _ = app.update(Msg::Colar(dados));

        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(3, 2), "=2+3");
        assert_eq!(sheet.estilo_resolvido(3, 2), original);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.snapshot(3, 2), None);
        let _ = app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(3, 2), original);
    }

    #[test]
    fn copiar_colar_intervalo_preserva_formato_de_celulas_vazias() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        let mut fundo = EstiloParcial::new();
        fundo.insert("bg".into(), Campo::Texto("#112233".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::new(0, 1, 0, 1), &fundo));
        let _ = app.update(Msg::IrPara("A1:B1".into()));
        let dados = app.copiar_selecao().unwrap();
        let _ = app.update(Msg::Selecionar(2, 0));
        let _ = app.update(Msg::Colar(dados));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(2, 0), "10");
        assert_eq!(sheet.get_raw(2, 1), "");
        assert_eq!(sheet.estilo_resolvido(2, 1), sheet.estilo_resolvido(0, 1));
    }

    #[test]
    fn colar_texto_externo_nao_reutiliza_formatacao_interna() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        let mut cor = EstiloParcial::new();
        cor.insert("color".into(), Campo::Texto("#FF0000".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::new(0, 0, 0, 0), &cor));
        let _ = app.copiar_selecao();
        let _ = app.update(Msg::Selecionar(2, 0));
        let _ = app.update(Msg::Colar("99".into()));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(2, 0), "99");
        assert!(sheet.estilo_resolvido(2, 0).is_none());
    }

    /// Pedido de 17/10: o Ctrl+C deixa a área marcada (tracejado) até uma
    /// edição; o recortar não marca (a origem já foi limpa).
    #[test]
    fn ctrl_c_marca_a_area_copiada_ate_editar() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "1".into());
        let _ = app.update(Msg::Selecionar(0, 0));
        app.fim_selecao = (2, 1);
        let _ = app.update(Msg::Copiar);
        assert_eq!(app.marca_copia, Some((0, Rect::new(0, 0, 2, 1))));
        assert_eq!(app.grade().copiada, Some(Rect::new(0, 0, 2, 1)));
        // Digitar numa célula tira a marca.
        let _ = app.update(Msg::Selecionar(5, 5));
        let _ = app.update(Msg::Editar("x".into()));
        let _ = app.update(Msg::EnviarEditor);
        assert!(app.marca_copia.is_none());
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::Recortar);
        assert!(app.marca_copia.is_none(), "recortar não deixa marca");
    }

    /// 16/10: colar até "a última linha" gravava 100 milhões de células e
    /// travava. 17/10: um valor simples cola até o fim de verdade (em
    /// blocos, no motor); fórmula continua parando na última linha com dados.
    #[test]
    #[ignore = "cola até a linha 100 milhões: pesado demais para a suíte; o usuário testa no app"]
    fn colar_ate_o_fim_da_planilha() {
        use feathersheet_core::celulas::refs::MAX_ROWS;
        use feathersheet_core::celulas::values::Value;
        // A1 escrito, copiado e colado em A2:A100000000: as 100 milhões.
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "7".into());
        for r in 0..10 { app.workbook.escrever_celula(0, r, 1, r.to_string()); }
        let _ = app.update(Msg::Selecionar(0, 0));
        let copiado = app.copiar_selecao().unwrap();
        let _ = app.update(Msg::Selecionar(1, 0));
        app.fim_selecao = (MAX_ROWS - 1, 0);
        let _ = app.update(Msg::Colar(copiado));
        let v = |app: &App, r| app.workbook.valor(0, r, 0);
        for r in [1, 9, 10, 65_536, MAX_ROWS - 1] {
            assert_eq!(v(&app, r), Value::Number(7.0), "linha {r}");
        }
        assert!(app.status.contains("99999999 células coladas"), "{}", app.status);
        // Um Ctrl+Z desfaz a colagem inteira.
        let _ = app.update(Msg::Desfazer);
        assert_eq!((v(&app, 1), v(&app, MAX_ROWS - 1), v(&app, 0)), (Value::Empty, Value::Empty, Value::Number(7.0)));
        // Fórmula: dados até a linha 10, coluna inteira de C vai até C10.
        let _ = app.update(Msg::Selecionar(0, 2));
        app.fim_selecao = (MAX_ROWS - 1, 2);
        let _ = app.update(Msg::Colar("=B1".into()));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        // (Texto de fora: a fórmula vai igual em todas as linhas.)
        assert_eq!((sheet.get_raw(9, 2), sheet.get_raw(10, 2)), ("=B1".into(), String::new()));
        assert!(app.status.contains("C10"), "{}", app.status);
        // Seleção que não chega ao fim cola inteira, como antes.
        let _ = app.update(Msg::Selecionar(0, 3));
        app.fim_selecao = (19, 3);
        let _ = app.update(Msg::Colar("y".into()));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(19, 3), "y");
        // 19/10: data também vai até o fim, com o formato numa faixa de
        // estilo — digitada de fora (E) e copiada daqui com o formato (F).
        let _ = app.update(Msg::Selecionar(0, 4));
        app.fim_selecao = (MAX_ROWS - 1, 4);
        let _ = app.update(Msg::Colar("16/10/2026".into()));
        let _ = app.update(Msg::Selecionar(0, 4));
        let copiado = app.copiar_selecao().unwrap();
        let _ = app.update(Msg::Selecionar(1, 5));
        app.fim_selecao = (MAX_ROWS - 1, 5);
        let _ = app.update(Msg::Colar(copiado));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        for c in [4, 5] {
            assert_eq!(sheet.get_value(MAX_ROWS - 1, c), sheet.get_value(0, 4), "coluna {c}");
            assert!(sheet.get_style(MAX_ROWS - 1, c).is_some(), "coluna {c}: o formato de data vai junto");
        }
    }

    #[test]
    fn duplicar_aba_ativa_a_copia_e_desfaz_com_ctrl_z() {
        use super::*;
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "7".into());
        app.update(Msg::DuplicarAba);
        assert_eq!(app.workbook.nomes(), vec!["Planilha1", "Planilha1 (2)"]);
        assert_eq!(app.folha, 1);
        assert_eq!(app.nome_aba, "Planilha1 (2)");
        assert_eq!(app.workbook.aba(1).unwrap().sheet.get_raw(0, 0), "7");
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.nomes(), vec!["Planilha1"]);
        assert_eq!(app.folha, 0);
        app.update(Msg::Refazer);
        assert_eq!(app.workbook.nomes(), vec!["Planilha1", "Planilha1 (2)"]);
        assert_eq!(app.folha, 1);
    }

#[test]
fn aba_ativa_da_ribbon_e_persistida_sem_modificar_a_planilha() {
    let mut app = App::default();
    assert_eq!(app.config.faixa_aba, "inicio");
    assert!(!app.alterado);

    let _ = app.update(Msg::AbaFaixa(AbaFaixa::Dados));
    assert_eq!(app.faixa_ativa, AbaFaixa::Dados);
    assert_eq!(app.config.faixa_aba, "dados");

    // O catálogo abre Inserir e também memoriza a aba.
    let _ = app.update(Msg::MostrarCatalogoFuncoes);
    assert_eq!(app.faixa_ativa, AbaFaixa::Inserir);
    assert_eq!(app.config.faixa_aba, "inserir");

    // A aba de tabela é contextual e não pode substituir a preferência.
    app.selecionar_faixa(AbaFaixa::Tabela);
    assert_eq!(app.faixa_ativa, AbaFaixa::Tabela);
    assert_eq!(app.config.faixa_aba, "inserir");
    assert!(!app.alterado);
}
