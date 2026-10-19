//! Testes da camada de objetos: desfazer por gesto, âncora nas células,
//! alvo das alças, barra/menus/aba montando nos dois temas, contexto,
//! texto, macro e área de transferência das formas.
use super::camada::{Alca, Camada, TipoGesto, TAMANHO_PADRAO};
use super::interface::SUB_FRENTE;
use crate::app::menu_contexto::ItemMenu;
use super::*;
use iced::Point;

fn app() -> App {
    let mut app = App::default();
    app.config.zoom = 100;
    // O catálogo de macros salvo na máquina não entra no teste.
    app.macro_catalogo = Default::default();
    app
}

fn lidos(app: &App) -> ObjetosAba {
    ObjetosAba::ler(app.workbook.aba(0).unwrap().sheet.ob.as_deref()).unwrap()
}

fn criar(app: &mut App, forma: &'static str, ret: Retangulo) -> u64 {
    let _ = app.update(Msg::Objetos(Evento::Escolher(forma)));
    assert_eq!(app.objetos.inserir, Some(forma));
    let _ = app.update(Msg::Objetos(Evento::Criar { forma, ret, espelhar: false }));
    assert!(app.objetos.inserir.is_none());
    app.objetos.selecionado.expect("a forma nova fica selecionada").1
}

#[test]
fn criar_mover_formatar_e_apagar_sao_um_desfazer_cada() {
    let mut app = app();
    let id = criar(&mut app, "retangulo", Retangulo::novo(130.0, 45.0, 160.0, 60.0));
    let criado = app.workbook.aba(0).unwrap().sheet.ob.clone();
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.de.linha, o.de.coluna), (1, 1), "âncora na célula B2");
    let ret = Retangulo::novo(300.0, 200.0, 160.0, 60.0);
    let _ = app.update(Msg::Objetos(Evento::Transformar { id, ret, giro: 30.0, medida: Medida::default() }));
    let _ = app.update(Msg::Objetos(Evento::Preenchimento(Some("#c00000"))));
    let _ = app.update(Msg::Objetos(Evento::Espessura(4.5)));
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.giro, o.aparencia.preenchimento.as_deref(), o.aparencia.espessura), (30.0, Some("#c00000"), 4.5));
    assert_eq!(o.retangulo(&ReguaAba(&app.workbook.aba(0).unwrap().sheet)), ret);
    // Delete apaga; quatro Ctrl+Z voltam ao recém-criado.
    assert!(app.apagar_objeto_selecionado());
    assert!(lidos(&app).objetos.is_empty());
    for _ in 0..4 {
        app.historico(false);
    }
    assert_eq!(app.workbook.aba(0).unwrap().sheet.ob, criado);
    app.historico(false);
    assert!(app.workbook.aba(0).unwrap().sheet.ob.is_none());
    // Seleção órfã depois do Ctrl+Z não quebra nada.
    assert!(app.objeto_selecionado().is_none());
    assert!(!app.apagar_objeto_selecionado());
    app.historico(true);
    assert_eq!(app.workbook.aba(0).unwrap().sheet.ob, criado);
}

#[test]
fn largura_da_coluna_estica_a_forma_e_esc_desfaz_a_escolha() {
    let mut app = app();
    let id = criar(&mut app, "elipse", Retangulo::novo(130.0, 45.0, 600.0, 60.0));
    let largura = |app: &App| lidos(app).obter(id).unwrap().retangulo(&ReguaAba(&app.workbook.aba(0).unwrap().sheet)).largura;
    let antes = largura(&app);
    app.workbook.aba_mut(0).unwrap().sheet.col_widths.insert(2, 300);
    assert!(
        (largura(&app) - antes - (300.0 - crate::grade::largura_padrao_da(&app.workbook.aba(0).unwrap().sheet))).abs() < 0.01,
        "coluna C (inteira dentro da elipse) mais larga estica a elipse"
    );
    let _ = app.update(Msg::Objetos(Evento::Escolher("estrela-5")));
    assert!(app.esc_objetos());
    assert!(app.objetos.inserir.is_none());
    let _ = app.update(Msg::Objetos(Evento::Selecionar(id)));
    assert!(app.objeto_selecionado().is_some());
    assert!(app.esc_objetos(), "Esc tira a seleção");
    assert!(!app.esc_objetos());
}

#[test]
fn camada_acha_alca_giro_e_objeto_e_ignora_area_vazia() {
    let mut app = app();
    let id = criar(&mut app, "elipse", Retangulo::novo(100.0, 100.0, 200.0, 100.0));
    let vista = app.vista_tinta().unwrap();
    let aba = app.workbook.aba(0).unwrap();
    let itens: Vec<ItemTela> = lidos(&app).objetos.iter().map(|o| ItemTela::de(o, &ReguaAba(&aba.sheet))).collect();
    let camada = Camada { itens, graficos: Vec::new(), selecionado: Some(id), inserir: None, vista, travar: false, editando: None };
    let r = camada.ret_tela(Retangulo::novo(100.0, 100.0, 200.0, 100.0));
    assert_eq!(camada.alvo(Point::new(r.x + r.width, r.y + r.height), false), Some((id, TipoGesto::Alca(Alca::Se))));
    assert_eq!(camada.alvo(Point::new(r.center_x(), r.y - 22.0), false), Some((id, TipoGesto::Girar)));
    assert_eq!(camada.alvo(r.center(), false), Some((id, TipoGesto::Mover)));
    assert_eq!(camada.alvo(Point::new(r.x + r.width + 40.0, r.center_y()), false), None);
    // Ponto da tela → absoluto.
    let p = camada.absoluto(Point::new(r.x, r.y));
    assert!((p.x - 100.0).abs() < 0.01 && (p.y - 100.0).abs() < 0.01, "{p:?}");
}

#[test]
fn forma_com_macro_executa_no_clique_e_ctrl_seleciona() {
    let mut app = app();
    app.macro_catalogo.macros.push(feather_macros::Macro { name: "Relatorio".into(), code: "log('ok')".into(), auto: false });
    let id = criar(&mut app, "retangulo", Retangulo::novo(100.0, 100.0, 200.0, 100.0));
    let _ = app.update(Msg::Objetos(Evento::AtribuirMacro));
    let _ = app.update(Msg::Objetos(Evento::MacroEscolher(Some("Relatorio".into()))));
    let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
    let _ = app.update(Msg::Objetos(Evento::MacroConfirmar));
    assert!(app.objetos.macro_dialogo.is_none());
    assert_eq!(lidos(&app).obter(id).unwrap().macro_nome.as_deref(), Some("Relatorio"));
    let _ = app.update(Msg::Objetos(Evento::Desselecionar));
    let vista = app.vista_tinta().unwrap();
    let aba = app.workbook.aba(0).unwrap();
    let itens: Vec<ItemTela> = lidos(&app).objetos.iter().map(|o| ItemTela::de(o, &ReguaAba(&aba.sheet))).collect();
    let camada = Camada { itens, graficos: Vec::new(), selecionado: None, inserir: None, vista, travar: false, editando: None };
    let centro = camada.ret_tela(Retangulo::novo(100.0, 100.0, 200.0, 100.0)).center();
    assert_eq!(camada.alvo(centro, false), Some((id, TipoGesto::Macro)));
    assert_eq!(camada.alvo(centro, true), Some((id, TipoGesto::Mover)), "Ctrl+clique seleciona");
    // "Nenhuma" tira a macro (outro Ctrl+Z).
    let _ = app.update(Msg::Objetos(Evento::Selecionar(id)));
    let _ = app.update(Msg::Objetos(Evento::AtribuirMacro));
    let _ = app.update(Msg::Objetos(Evento::MacroEscolher(None)));
    let _ = app.update(Msg::Objetos(Evento::MacroConfirmar));
    assert!(lidos(&app).obter(id).unwrap().macro_nome.is_none());
    app.historico(false);
    assert_eq!(lidos(&app).obter(id).unwrap().macro_nome.as_deref(), Some("Relatorio"));
}

#[test]
fn texto_da_forma_confirma_e_esc_cancela() {
    let mut app = app();
    let id = criar(&mut app, "elipse", Retangulo::novo(50.0, 50.0, 160.0, 80.0));
    let _ = app.update(Msg::Objetos(Evento::EditarTexto(id)));
    let _ = app.update(Msg::Objetos(Evento::TextoDigitado("  Etapa 1 ".into())));
    let _: Element<'_, Msg> = app.grade_com_objetos(text("grade").into());
    let _ = app.update(Msg::Objetos(Evento::TextoConfirmar));
    assert_eq!(lidos(&app).obter(id).unwrap().texto.as_deref(), Some("Etapa 1"));
    let _ = app.update(Msg::Objetos(Evento::EditarTexto(id)));
    let _ = app.update(Msg::Objetos(Evento::TextoDigitado("outro".into())));
    assert!(app.esc_objetos());
    assert_eq!(lidos(&app).obter(id).unwrap().texto.as_deref(), Some("Etapa 1"));
}

#[test]
fn copiar_colar_duplicar_e_ordem_pela_area_interna() {
    let mut app = app();
    let a = criar(&mut app, "retangulo", Retangulo::novo(40.0, 40.0, 100.0, 60.0));
    let _b = criar(&mut app, "elipse", Retangulo::novo(200.0, 40.0, 100.0, 60.0));
    let _ = app.update(Msg::Objetos(Evento::Selecionar(a)));
    let _ = app.update(Msg::Objetos(Evento::Copiar));
    let _ = app.update(Msg::Objetos(Evento::Colar));
    let _ = app.update(Msg::Objetos(Evento::Colar));
    let o = lidos(&app);
    assert_eq!(o.objetos.len(), 4);
    let regua = ReguaAba(&app.workbook.aba(0).unwrap().sheet);
    let ultimo = o.objetos.last().unwrap().retangulo(&regua);
    assert!((ultimo.x - 72.0).abs() < 0.01, "a segunda colagem anda mais um passo: {ultimo:?}");
    let _ = app.update(Msg::Objetos(Evento::Duplicar));
    assert_eq!(lidos(&app).objetos.len(), 5);
    // Ordem: a primeira vai para a frente (fim da lista).
    let _ = app.update(Msg::Objetos(Evento::Selecionar(a)));
    let _ = app.update(Msg::Objetos(Evento::Ordem(Ordem::Frente)));
    assert_eq!(lidos(&app).objetos.last().unwrap().id, a);
    // Recortar tira a forma; um Ctrl+Z a devolve.
    let _ = app.update(Msg::Objetos(Evento::Recortar));
    assert!(lidos(&app).obter(a).is_none());
    app.historico(false);
    assert!(lidos(&app).obter(a).is_some());
}

#[test]
fn trocar_forma_e_arredondar_cantos_pela_aba() {
    let mut app = app();
    let id = criar(&mut app, "retangulo", Retangulo::novo(40.0, 40.0, 200.0, 100.0));
    let _ = app.update(Msg::Objetos(Evento::CampoRaio("20".into())));
    let _ = app.update(Msg::Objetos(Evento::AplicarRaio));
    assert_eq!(lidos(&app).obter(id).unwrap().ajustes, vec![0.2; 4]);
    let _ = app.update(Msg::Objetos(Evento::AlternarMenu(ModoMenu::Trocar)));
    assert_eq!(app.objetos.menu, Some(ModoMenu::Trocar));
    let _ = app.update(Msg::Objetos(Evento::Escolher("losango")));
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.forma.as_str(), o.ajustes.len()), ("losango", 0));
    assert!(app.objetos.menu.is_none());
    // Tamanho com proporção travada.
    let _ = app.update(Msg::Objetos(Evento::TravarProporcao(true)));
    let _ = app.update(Msg::Objetos(Evento::CampoLargura("100".into())));
    assert_eq!(app.objetos.campos.altura, "50");
    let _ = app.update(Msg::Objetos(Evento::AplicarTamanho));
    let r = lidos(&app).obter(id).unwrap().retangulo(&ReguaAba(&app.workbook.aba(0).unwrap().sheet));
    assert!((r.largura - 100.0).abs() < 0.01 && (r.altura - 50.0).abs() < 0.01, "{r:?}");
}

#[test]
fn barra_menus_contexto_e_aba_montam_nos_dois_temas() {
    for escuro in [false, true] {
        let mut app = app();
        app.config.escuro = escuro;
        let id = criar(&mut app, "estrela-5", Retangulo::novo(50.0, 50.0, 100.0, 100.0));
        for p in [Painel::Contorno, Painel::Preenchimento] {
            let _ = app.update(Msg::Objetos(Evento::AbrirPainel(p)));
            let _: Element<'_, Msg> = app.grade_com_objetos(text("grade").into());
            let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
        }
        let _ = app.update(Msg::Objetos(Evento::AlternarMenu(ModoMenu::Inserir)));
        assert_eq!(app.objetos.menu, Some(ModoMenu::Inserir));
        let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
        let _ = app.update(Msg::Objetos(Evento::FecharMenu));
        assert!(app.objetos.menu.is_none());
        // Menu de contexto com o submenu "Trazer para frente" aberto.
        let _ = app.update(Msg::Objetos(Evento::Contexto(id)));
        let _ = app.update(Msg::Objetos(Evento::ContextoSub(Some(SUB_FRENTE))));
        let o = app.objeto_selecionado().unwrap();
        assert!(matches!(&app.itens_contexto(&o)[SUB_FRENTE], ItemMenu::Sub { rotulo, .. } if rotulo == "Trazer para frente"));
        let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
        let _ = app.update(Msg::Objetos(Evento::DoContexto(Box::new(Evento::Duplicar))));
        assert!(app.objetos.contexto.is_none());
        assert_eq!(lidos(&app).objetos.len(), 2);
        // Aba Editar forma: grupos e larguras batem; menus ▾ abrem.
        let _ = app.update(Msg::Objetos(Evento::FormatarForma));
        assert_eq!(app.faixa_ativa, AbaFaixa::EditarForma);
        assert_eq!(app.montar_grupos_faixa().len(), app.faixa_larguras().len());
        for m in [MenuFaixa::Frente, MenuFaixa::Tras, MenuFaixa::Girar, MenuFaixa::Alinhar] {
            let _ = app.update(Msg::Objetos(Evento::MenuFaixa(m)));
            let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
        }
        let _: Element<'_, Msg> = app.aba_editar_forma();
        // Tirar a seleção volta para Inserir.
        assert!(app.esc_objetos());
        while app.esc_objetos() {}
        assert_eq!(app.faixa_ativa, AbaFaixa::Inserir);
        assert_eq!(app.montar_grupos_faixa().len(), app.faixa_larguras().len());
    }
}

#[test]
fn captura_insere_formas_de_exemplo_e_seleciona_a_estrela() {
    let mut app = app();
    app.preparar_captura_objetos("1");
    let o = lidos(&app);
    assert_eq!(o.objetos.len(), 8);
    assert_eq!(app.objeto_selecionado().unwrap().forma, "estrela-5");
    assert!(o.objetos.iter().any(|x| x.texto.as_deref() == Some("Etapa 1")));
    assert_eq!(decimal(0.75), "0,75");
    assert_eq!(decimal(3.0), "3");
    for modo in ["menu", "painel", "contexto", "faixa", "ajuste"] {
        let mut app = self::app();
        app.preparar_captura_objetos(modo);
        let _: Element<'_, Msg> = app.grade_com_objetos(text("grade").into());
        let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
    }
    assert!(TAMANHO_PADRAO.0 > 0.0);
}

#[test]
fn icone_entra_na_celula_ativa_selecionado_so_traco_e_um_desfazer() {
    let mut app = app();
    let _ = app.update(Msg::Objetos(Evento::AlternarMenu(ModoMenu::Icones)));
    assert_eq!(app.objetos.menu, Some(ModoMenu::Icones));
    let _ = app.update(Msg::Objetos(Evento::BuscaIcones("casa".into())));
    assert!(feather_shapes::icones::buscar(&app.objetos.busca_icones, 10).iter().any(|i| i.id == "home"));
    app.linha = 3;
    app.coluna = 2;
    let _ = app.update(Msg::Objetos(Evento::InserirIcone("home")));
    assert!(app.objetos.menu.is_none(), "inserir fecha a galeria");
    let id = app.objetos.selecionado.expect("o ícone novo fica selecionado").1;
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.tipo, o.forma.as_str()), (modelo::TipoObjeto::Icone, "home"));
    assert_eq!((o.de.linha, o.de.coluna), (3, 2), "âncora na célula ativa C4");
    assert!(o.aparencia.preenchimento.is_none() && o.aparencia.contorno.is_some());
    let regua = ReguaAba(&app.workbook.aba(0).unwrap().sheet);
    let it = ItemTela::de(&o, &regua);
    assert!(it.icone && !it.linha && it.preenchimento.is_none());
    // "heart" também é um prstGeom de forma: o tipo decide, não o nome.
    let _ = app.update(Msg::Objetos(Evento::InserirIcone("heart")));
    let coracao = lidos(&app).objetos.last().cloned().unwrap();
    assert!(ItemTela::de(&coracao, &ReguaAba(&app.workbook.aba(0).unwrap().sheet)).icone);
    app.historico(false);
    app.historico(false);
    assert!(lidos(&app).objetos.is_empty());
}

#[test]
fn galeria_sem_busca_traz_todos_os_icones_uma_vez_com_os_populares_primeiro() {
    let mut app = app();
    let lista = app.icones_da_busca();
    let todos = feather_shapes::icones::todos().len();
    assert_eq!(lista.len(), todos, "rolagem infinita: todos, sem repetir");
    let unicos: std::collections::HashSet<&str> = lista.iter().map(|i| i.id).collect();
    assert_eq!(unicos.len(), todos);
    assert_eq!(lista[0].id, feather_shapes::icones::buscar("", 1)[0].id);
    // Mesmo termo: a mesma lista (cache), sem refazer a busca.
    assert!(Arc::ptr_eq(&lista, &app.icones_da_busca()));
    let _ = app.update(Msg::Objetos(Evento::RolarIcones(4000.0)));
    let _ = app.update(Msg::Objetos(Evento::BuscaIcones("seta".into())));
    assert_eq!(app.objetos.rolagem_icones, 0.0, "busca nova volta ao topo");
    let setas = app.icones_da_busca();
    assert!(setas.len() > 160, "sem o teto antigo de 160 resultados");
}

#[test]
fn wordart_insere_na_celula_ativa_desenha_contorno_e_cada_mudanca_e_um_desfazer() {
    use feather_shapes::wordart::{Transformacao, ESTILOS};
    let mut app = app();
    let _ = app.update(Msg::Objetos(Evento::AlternarMenu(ModoMenu::WordArt)));
    assert_eq!(app.objetos.menu, Some(ModoMenu::WordArt));
    let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
    app.linha = 4;
    app.coluna = 1;
    let _ = app.update(Msg::Objetos(Evento::EstiloWordArt(2)));
    let id = app.objetos.selecionado.expect("o WordArt novo fica selecionado").1;
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.tipo, o.forma.as_str()), (modelo::TipoObjeto::Wordart, "reto"));
    assert_eq!((o.de.linha, o.de.coluna), (4, 1), "âncora na célula ativa B5");
    assert_eq!(o.texto.as_deref(), Some(wordart::TEXTO_PADRAO));
    assert_eq!(o.aparencia.preenchimento.as_deref(), Some(ESTILOS[2].preenchimento));
    assert_eq!(wordart::chave_estilo(&o), Some("dourado"));
    assert!(o.extras.contains_key("sombra"), "Dourado tem sombra");
    let it = ItemTela::de(&o, &ReguaAba(&app.workbook.aba(0).unwrap().sheet));
    assert!(it.wordart.is_some() && !it.linha && !it.icone);
    // Contorno não vazio, com o miolo do "O" e a deformação mudando os pontos.
    let reto = wordart::contornos("O", Transformacao::Reto, 1.0);
    assert!(reto.0.len() >= 2, "O tem contorno externo e interno");
    assert_ne!(reto.0, wordart::contornos("O", Transformacao::Arco, 1.0).0);
    assert!(wordart::caminho("Seu texto", it.wordart.as_ref().unwrap(), 300.0, 80.0, (false, false)).is_some());
    // Galeria e barra montam com o WordArt selecionado (deformação à vista).
    let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
    let _: Element<'_, Msg> = app.grade_com_objetos(text("grade").into());
    let _ = app.grupos_editar_forma();

    // Trocar estilo, deformar e sombra: um Ctrl+Z cada.
    let _ = app.update(Msg::Objetos(Evento::EstiloWordArt(1)));
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!(wordart::chave_estilo(&o), Some("contornado"));
    assert_eq!(o.aparencia.contorno.as_deref(), Some(ESTILOS[1].contorno));
    assert!(!o.extras.contains_key("sombra"), "Contornado não tem sombra");
    assert_eq!(lidos(&app).objetos.len(), 1, "com WordArt selecionado o estilo troca, não insere");
    let _ = app.update(Msg::Objetos(Evento::DeformarWordArt(Transformacao::Arco, 0.7)));
    let _ = app.update(Msg::Objetos(Evento::SombraWordArt(true)));
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.forma.as_str(), o.ajustes.clone()), ("arco", vec![0.7]));
    assert!(o.extras.contains_key("sombra"));
    app.historico(false);
    assert!(!lidos(&app).obter(id).unwrap().extras.contains_key("sombra"));
    app.historico(false);
    assert_eq!(lidos(&app).obter(id).unwrap().forma, "reto");
    app.historico(false);
    assert_eq!(wordart::chave_estilo(lidos(&app).obter(id).unwrap()), Some("dourado"));
    app.historico(false);
    assert!(lidos(&app).objetos.is_empty(), "inserir é um Ctrl+Z só");
}

#[test]
fn wordart_texto_pelo_f2_e_captura_abre_a_galeria() {
    let mut app = app();
    let _ = app.update(Msg::Objetos(Evento::EstiloWordArt(0)));
    let id = app.objetos.selecionado.unwrap().1;
    let _ = app.update(Msg::Objetos(Evento::EditarTexto(id)));
    let _ = app.update(Msg::Objetos(Evento::TextoDigitado("Vendas".into())));
    let _ = app.update(Msg::Objetos(Evento::TextoConfirmar));
    assert_eq!(lidos(&app).obter(id).unwrap().texto.as_deref(), Some("Vendas"));

    let mut app = self::app();
    app.preparar_captura_objetos("wordart");
    assert_eq!(app.objetos.menu, Some(ModoMenu::WordArt));
    let lista = lidos(&app);
    assert_eq!(lista.objetos.iter().filter(|o| o.tipo == modelo::TipoObjeto::Wordart).count(), 4);
    let _: Element<'_, Msg> = app.sobrepor_menu_formas(text("base").into());
}

#[test]
fn alinhamento_do_texto_grava_desfaz_e_posiciona() {
    let mut app = app();
    let id = criar(&mut app, "retangulo", Retangulo::novo(100.0, 50.0, 200.0, 100.0));
    assert!(!app.workbook.aba(0).unwrap().sheet.ob.as_deref().unwrap().contains("alinhar"), "padrão não grava");
    let _ = app.update(Msg::Objetos(Evento::AlinharTextoH(modelo::AlinharH::Esquerda)));
    let _ = app.update(Msg::Objetos(Evento::AlinharTextoV(modelo::AlinharV::Topo)));
    let o = lidos(&app).obter(id).cloned().unwrap();
    assert_eq!((o.alinhar.h, o.alinhar.v), (modelo::AlinharH::Esquerda, modelo::AlinharV::Topo));
    assert!(app.workbook.aba(0).unwrap().sheet.ob.as_deref().unwrap().contains("\"alinhar\""));
    let r = iced::Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(200.0, 100.0));
    let (p, h, v) = super::camada::posicao_texto(r, o.alinhar, 1.0);
    assert_eq!((p.x, p.y), (17.0, 27.0));
    assert_eq!((h, v), (iced::alignment::Horizontal::Left, iced::alignment::Vertical::Top));
    app.historico(false);
    assert_eq!(lidos(&app).obter(id).unwrap().alinhar.h, modelo::AlinharH::Esquerda);
    app.historico(false);
    assert!(lidos(&app).obter(id).unwrap().alinhar.e_padrao(), "dois Ctrl+Z voltam ao centro/meio");
}

#[test]
fn caixa_de_texto_nasce_branca_e_editando() {
    let mut app = app();
    let _ = app.update(Msg::Objetos(Evento::CaixaTexto));
    assert_eq!(app.objetos.inserir, Some("retangulo"));
    let _ = app.update(Msg::Objetos(Evento::Criar { forma: "retangulo", ret: Retangulo::novo(10.0, 10.0, 120.0, 60.0), espelhar: false }));
    let o = &lidos(&app).objetos[0];
    assert_eq!(o.aparencia.preenchimento.as_deref(), Some("#ffffff"));
    assert_eq!(app.objetos.texto.as_ref().map(|t| t.0), Some(o.id));
    // A próxima forma do menu volta ao normal.
    let id = criar(&mut app, "retangulo", Retangulo::novo(200.0, 10.0, 50.0, 50.0));
    assert_ne!(lidos(&app).objetos.iter().find(|o| o.id == id).unwrap().aparencia.preenchimento.as_deref(), Some("#ffffff"));
}
