use super::*;

fn interface() -> Interface {
    let mut app = Planilha::default();
    let id = window::Id::unique();
    app.janelas.principal = Some(id);
    app.em_tela_inicio = false;
    Interface::com_app(app)
}

#[test]
fn todas_as_abas_usam_os_mesmos_pixels_da_interface_original() {
    let mut i = interface();
    for escuro in [false, true] {
        i.app.config.tema_auto = false;
        i.app.config.escuro = escuro;
        definir_tema(escuro, 0);
        for (nome, aba) in [
            ("inicio", AbaFaixa::Inicio),
            ("inserir", AbaFaixa::Inserir),
            ("desenhar", AbaFaixa::Desenhar),
            ("layout", AbaFaixa::Layout),
            ("formulas", AbaFaixa::Formulas),
            ("dados", AbaFaixa::Dados),
            ("revisao", AbaFaixa::Revisao),
            ("exibir", AbaFaixa::Exibir),
            ("macros", AbaFaixa::Macros),
            ("ferramentas", AbaFaixa::Ferramentas),
            ("tabela", AbaFaixa::Tabela),
            ("forma", AbaFaixa::EditarForma),
            ("dinamica", AbaFaixa::Dinamica),
        ] {
            i.app.faixa_ativa = aba;
            i.revisao += 1;
            let mut original = EstadoJanela::nova(
                i.app.janelas.principal.unwrap(),
                iced::Size::new(1280., 800.),
            );
            original.desenhar(&i.app, i.revisao, 1., &i.sender, p::WindowId::MAIN);
            let png = p::capture(
                &i.view(),
                p::Size::new(1280., 800.),
                1.,
                &i.theme(),
                &mut p::Renderer::new(),
                None,
            );
            let atual = image::load_from_memory(&png).unwrap().to_rgba8();
            let diferentes = atual
                .as_raw()
                .iter()
                .zip(&original.pixels)
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(diferentes, 0, "aba {nome}, escuro={escuro}");
            if let Some(dir) = std::env::var_os("FEATHERSHEET_PELICANO_PREVIAS") {
                std::fs::write(
                    PathBuf::from(dir).join(format!(
                        "feathersheet-1para1-{}-{nome}.png",
                        if escuro { "escuro" } else { "claro" }
                    )),
                    png,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn operacao_de_foco_e_colagem_usam_o_editor_original() {
    use f::StreamExt;
    let mut i = interface();
    let foco = iced::widget::text_input::focus::<Comando>(iced::widget::text_input::Id::new(
        "feathersheet.fx",
    ));
    let mut stream = iced_runtime::task::into_stream(foco).unwrap();
    f::executor::block_on(async {
        while let Some(a) = stream.next().await {
            i.acao(a);
        }
    });
    let mut clipboard = p::MemoryClipboard(Some("9".into()));
    for (key, text, modifiers) in [
        (
            p::Key::Character("4".into()),
            Some("4".into()),
            p::Modifiers::empty(),
        ),
        (p::Key::Character("v".into()), None, p::Modifiers::CONTROL),
    ] {
        let ev = p::Event::Key {
            key,
            text,
            modifiers,
            pressed: true,
        };
        let mut mensagens = Vec::new();
        let mut janelas = Vec::new();
        let mut renderer = p::Renderer::new();
        let mut states = p::States::default();
        {
            let view = i.view();
            let node = p::widget::layout(
                &view,
                p::Size::new(1280., 800.),
                &mut renderer,
                &states,
                &i.theme(),
            );
            let mut cx = p::EventCx {
                renderer: &mut renderer,
                states: &mut states,
                cursor: None,
                messages: &mut mensagens,
                window: &mut janelas,
                redraw: false,
                clipboard: &mut clipboard,
            };
            p::widget::dispatch(&view,&node,&p::Event::ModifiersChanged(modifiers),&mut cx);
            assert_eq!(
                p::widget::dispatch(&view, &node, &ev, &mut cx),
                p::Captured::Yes
            );
        }
        for msg in mensagens {
            if let Msg::Comando(cmd) = msg {
                i.app.update(cmd);
                i.revisao += 1;
            }
        }
    }
    assert_eq!(i.app.edicao, "49");
}

#[test]
fn janela_auxiliar_mantem_view_e_identidade_da_janela_original() {
    let mut i = interface();
    let tarefa = i.app.alternar(Auxiliar::FormatarCelulas);
    use f::StreamExt;
    let mut stream = iced_runtime::task::into_stream(tarefa).unwrap();
    let acao = f::executor::block_on(stream.next()).unwrap();
    i.acao(acao);
    assert_eq!(i.janelas.len(), 2);
    let (&pid, estado) = i
        .janelas
        .iter()
        .find(|(id, _)| **id != p::WindowId::MAIN)
        .unwrap();
    assert_eq!(
        i.app.janelas.tipo(estado.borrow().id),
        Some(Auxiliar::FormatarCelulas)
    );
    let png = p::capture(
        &i.view_window(pid),
        p::Size::new(720., 600.),
        1.,
        &i.theme(),
        &mut p::Renderer::new(),
        None,
    );
    assert!(image::load_from_memory(&png).is_ok());
}
