//! Menus nativos e barra de execução do editor de macros.
use super::*;

pub(super) fn menu_itens(id: u8) -> Vec<(&'static str, Option<Msg>)> {
    match id {
        0 => vec![
            (tr!("Novo módulo JavaScript"), Some(Msg::MacroNovoModulo)),
            (tr!("Abrir .js  Ctrl+O"), Some(Msg::MacroAbrir)),
            (tr!("Guardar módulo  Ctrl+S"), Some(Msg::MacroGuardarModulo)),
            (tr!("Salvar .js  Ctrl+Shift+S"), Some(Msg::MacroSalvar)),
            (tr!("Importar catálogo"), Some(Msg::MacroImportarCatalogo)),
            (tr!("Importar VBA do Excel (.xlsm)"), Some(Msg::VbaImportar)),
            (tr!("Exportar catálogo"), Some(Msg::MacroExportarCatalogo)),
            (tr!("Fechar editor  Alt+Q"), Some(Msg::FecharAux(Auxiliar::Macros))),
        ],
        1 => vec![
            (tr!("Desfazer  Ctrl+Z"), event(EventoVisual::Codigo(EventoCodigo::Desfazer))),
            (tr!("Refazer  Ctrl+Y"), event(EventoVisual::Codigo(EventoCodigo::Refazer))),
            (tr!("Localizar  Ctrl+F"), event(EventoVisual::Codigo(EventoCodigo::Abrir(false)))),
            (tr!("Substituir  Ctrl+H"), event(EventoVisual::Codigo(EventoCodigo::Abrir(true)))),
            (tr!("Localizar próxima  F3"), event(EventoVisual::Codigo(EventoCodigo::Proximo))),
            (tr!("Buscar macro pelo nome"), event(EventoVisual::Codigo(EventoCodigo::FocarProjeto))),
        ],
        2 => vec![
            (tr!("Explorador de projeto"), event(EventoVisual::Explorador)),
            (tr!("Propriedades"), event(EventoVisual::Propriedades)),
            (tr!("Janela Imediata  Ctrl+G"), event(EventoVisual::Painel(0))),
            (tr!("Locais"), event(EventoVisual::Painel(1))),
            (tr!("Pilha de chamadas"), event(EventoVisual::Painel(2))),
            (tr!("Janela de Inspeção"), event(EventoVisual::Painel(3))),
            (tr!("Histórico de macros"), event(EventoVisual::Painel(PAINEL_HISTORICO))),
            (tr!("Mostrar/ocultar saída"), event(EventoVisual::Console)),
        ],
        3 => vec![
            (tr!("Módulo"), Some(Msg::MacroNovoModulo)), (tr!("Componente HTML/CSS"), event(EventoVisual::Documento("componente:previsao".into()))),
            (tr!("Procedimento"), None), (tr!("Evento da planilha"), None),
        ],
        4 => vec![
            (tr!("Executar  F5 / Ctrl+Enter"), Some(Msg::MacroExecutar)),
            (tr!("Limpar Janela Imediata"), event(EventoVisual::LimparSaida)),
        ],
        5 => vec![
            (tr!("Verificar sintaxe"), None), (tr!("Entrar  F8"), None),
            (tr!("Passar  Shift+F8"), None), (tr!("Sair  Ctrl+Shift+F8"), None),
            (tr!("Pausar"), None), (tr!("Parar  Shift+F5"), None),
            (tr!("Ponto de interrupção  F9"), None),
            (tr!("Próxima instrução  Ctrl+F9"), None),
            (tr!("Adicionar inspeção"), event(EventoVisual::Painel(3))),
        ],
        6 => vec![
            (tr!("Opções do editor"), None), (tr!("Fonte / Tabulação"), None),
            (tr!("Sempre depurar"), None),
        ],
        _ => vec![
            (tr!("Guia de macros  F1"), Some(Msg::AjudaAbrirSecao("macros"))),
            (tr!("Atalhos"), Some(Msg::AjudaAbrirSecao("atalhos"))),
        ],
    }
}
/// Mapeamento centralizado: nenhum item dos menus TS deve depender de glifo Unicode.
/// Retorna None apenas quando um novo comando ainda não recebeu ícone.
pub(super) fn icone_do_menu(rotulo: &str) -> Option<&'static str> {
    let pares: &[(&str, &str)] = &[
        ("Novo módulo JavaScript", "file-plus"),
        ("Abrir .js", "folder-open"),
        ("Salvar .js", "device-floppy"),
        ("Guardar módulo", "device-floppy"),
        ("Importar catálogo", "folder-open"),
        ("Importar VBA do Excel (.xlsm)", "file-code"),
        ("Exportar catálogo", "file-export"),
        ("Fechar editor", "x"),
        ("Desfazer", "arrow-back-up"),
        ("Refazer", "arrow-forward-up"),
        ("Localizar próxima", "chevron-down"),
        ("Buscar macro pelo nome", "search"),
        ("Localizar", "search"),
        ("Substituir", "edit"),
        ("Explorador de projeto", "sitemap"),
        ("Propriedades", "list-details"),
        ("Janela Imediata", "terminal"),
        ("Locais", "list"),
        ("Pilha de chamadas", "sitemap"),
        ("Janela de Inspeção", "eye"),
        ("Mostrar/ocultar saída", "eye"),
        ("Histórico de macros", "clock"),
        ("Módulo", "file-code"),
        ("Componente HTML/CSS", "forms"),
        ("Procedimento", "code"),
        ("Evento da planilha", "bolt"),
        ("Executar", "player-play"),
        ("Limpar Janela", "trash"),
        ("Verificar sintaxe", "checks"),
        ("Entrar", "arrow-down"),
        ("Passar", "arrow-step"),
        ("Sair", "arrow-back-up"),
        ("Pausar", "player-pause"),
        ("Parar", "player-stop"),
        ("Ponto de interrupção", "bug"),
        ("Próxima instrução", "arrow-step"),
        ("Adicionar inspeção", "plus"),
        ("Opções do editor", "settings"),
        ("Fonte / Tabulação", "text-size"),
        ("Sempre depurar", "bug"),
        ("Guia de macros", "help"),
        ("Atalhos", "keyboard"),
    ];
    // O rótulo chega traduzido; os prefixos são em português.
    let rotulo = crate::idioma::original(rotulo);
    pares.iter().find_map(|(prefixo, icone)| rotulo.starts_with(prefixo).then_some(*icone))
}

/// Larguras fixas para ancorar o popup na mesma posição do botão, sem
/// consumir uma segunda linha de layout (e sem scroll horizontal aleatório).
const LARGURAS_MENU: [f32; 8] = [76.0, 73.0, 73.0, 74.0, 91.0, 82.0, 110.0, 69.0];
const ESPACO_MENU: f32 = 2.0;

/// Offset dentro da janela do editor. Inclui padding externo da janela (8px)
/// e padding do cabeçalho (8px). O último menu abre alinhado à direita.
pub(super) fn posicao_popup(id: u8) -> f32 {
    let indice = usize::from(id).min(MENUS.len() - 1);
    let inicio = 16.0 + LARGURAS_MENU[..indice].iter().sum::<f32>()
        + ESPACO_MENU * indice as f32;
    if indice == MENUS.len() - 1 {
        // Ajuda: alinhar a borda direita evita popup para fora da janela.
        (inicio + LARGURAS_MENU[indice] - 270.0).max(12.0)
    } else {
        inicio
    }
}

pub(super) fn menus<'a>(app: &'a App) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let mut linha = row(Vec::new()).spacing(ESPACO_MENU).align(pelicano::Align::Center);
    for (i, rotulo) in MENUS.iter().enumerate() {
        let ativo = app.macro_visual.menu == Some(i as u8);
        let conteudo = row![
            text(tr!(*rotulo).to_owned()).size(12),
            feather_ui::icones::icone("chevron-down", 11.0),
        ].spacing(4).align(pelicano::Align::Center);
        linha = linha.push(
            button(conteudo)
                .padding([7, 9])
                .width(Length::Fixed(LARGURAS_MENU[i]))
                .on_press(Msg::MacroVisual(EventoVisual::Menu(i as u8)))
                .style(estilo_botao_ide(ativo)),
        );
    }
    linha = linha.push(Space::new(Length::FILL, 0.0));
    linha = linha.push(text(tr!("EDITOR DE MACROS")).size(10).color(cor_secundaria(escuro)));
    container(linha).padding([4, 8]).style(estilo_area(escuro))
        .width(Length::FILL).into()
}

/// Um dropdown por botão: itens verticais, ícones nativos e comandos
/// preservados. Elemento flutuante, sem aumentar a altura da barra de menus.
pub(super) fn menu_suspenso<'a>(app: &'a App, id: u8) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let mut itens = pelicano::column(Vec::new()).spacing(2).padding(6);
    for (indice, (rotulo, acao)) in menu_itens(id).into_iter().enumerate() {
        let icone = icone_do_menu(rotulo).unwrap_or("info-circle");
        let descricao = row![
            feather_ui::icones::icone(icone, 16.0),
            text(rotulo).size(12),
            Space::new(Length::FILL, 0.0),
        ].spacing(9).align(pelicano::Align::Center);
        let mut botao = button(descricao).width(Length::FILL).padding([8, 10])
            .style(estilo_botao_ide(false));
        if acao.is_some() {
            // Fecha o menu primeiro; o dispatcher encaminha a mensagem real.
            botao = botao.on_press(Msg::MacroVisual(
                EventoVisual::SelecionarMenu(id, indice),
            ));
        }
        itens = itens.push(botao);
    }
    container(itens).width(Length::Fixed(270.0))
        .style(move |_| pelicano::Style {
            text_color: Some(cor_texto(escuro)),
            background: Some(Background::Color(cor_fundo(escuro))),
            border: Border {
                color: cor_borda(escuro),
                width: 1.0,
                radius: [9.0; 4],
            },
            shadow: Some(pelicano::Shadow {
                color: Color::rgb8(0, 0, 0).alpha(if escuro { 0.48 } else { 0.19 }),
                offset: pelicano::Point::new(0.0, 8.0),
                blur: 19.0,
            }),
            ..Default::default()
        }).into()
}

pub(super) fn ferramentas<'a>(app: &'a App) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let linha = row![
        botao_executar(if app.macro_visual.documento.starts_with("modulo:") {
            Some(Msg::MacroExecutar)
        } else { None }),
        botao_icone(tr!("Pausar"), "player-pause", None, false),
        botao_icone(tr!("Parar"), "player-stop", None, false),
        container(Space::new(Length::Fixed(1.0), 0.0))
            .height(Length::Fixed(20.0)).style(estilo_divisor(escuro)),
        botao_icone(tr!("Entrar  F8"), "arrow-down", None, false),
        botao_icone(tr!("Passar"), "arrow-step", None, false),
        botao_icone(tr!("Sair"), "arrow-back-up", None, false),
        botao_icone(tr!("Próxima instrução"), "arrow-step", None, false),
        container(Space::new(Length::Fixed(1.0), 0.0))
            .height(Length::Fixed(20.0)).style(estilo_divisor(escuro)),
        botao_icone(tr!("Módulo"), "file-code", Some(Msg::MacroNovoModulo), false),
        botao_icone(tr!("Componente"), "forms", event(EventoVisual::Documento("componente:previsao".into())), false),
        botao_icone(tr!("Salvar módulo"), "device-floppy", Some(Msg::MacroGuardarModulo), false),
        botao_icone(tr!("Guia"), "help", Some(Msg::AjudaAbrirSecao("macros")), false),
    ].spacing(7).align(pelicano::Align::Center).width(Length::Shrink)
        .height(Length::Fixed(34.0));
    // Uma barra rolável no eixo horizontal não pode conter Fill.
    // Usar Shrink conserva largura intrínseca e permite rolagem sem panic.
    container(
        scrollable(linha).direction(pelicano::Direction::Horizontal).height(Length::Fixed(42.0))
    ).padding([4, 8]).style(estilo_area(escuro)).width(Length::FILL).into()
}

