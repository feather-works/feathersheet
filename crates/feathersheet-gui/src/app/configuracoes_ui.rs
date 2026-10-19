//! Preferências nativas FeatherSheet, derivadas do contrato SettingsPanel do TS.
//! Layout compartilhado fica em feather-ui; nenhuma ação fictícia é exposta.
use super::*;

const ZOOM_OPCOES: &[&str] = &[
    "25%", "50%", "75%", "90%", "100%", "110%", "125%", "150%",
    "175%", "200%", "250%", "300%", "400%",
];
const ZOOM_LIMITES: &[&str] = &[
    "25%", "50%", "75%", "90%", "100%", "125%", "150%",
    "200%", "250%", "300%", "400%",
];
fn nome_zoom(opcoes: &[&'static str], valor: u16) -> Option<&'static str> {
    opcoes.iter().copied().find(|v|
        v.trim_end_matches('%').parse::<u16>().ok() == Some(valor))
}
pub(super) const ATALHOS: &[(&str, &str, &str)] = &[
    ("save", "Salvar documento", "Ctrl+S"),
    ("saveAs", "Salvar como…", "Ctrl+Shift+S"),
    ("createTable", "Criar tabela", "Ctrl+T"),
    ("toggleFilter", "Alternar filtro", "Ctrl+Shift+L"),
    ("findReplace", "Localizar e substituir", "Ctrl+H"),
    ("toggleFormulas", "Mostrar fórmulas", "Ctrl+`"),
    ("colarValores", "Colar somente valores", "Ctrl+Shift+V"),
];

pub(super) fn opcao<'a>(rotulo: &str, icone: &str, ativo: bool, evento: Msg)
    -> iced::widget::Button<'a, Msg>
{
    button(row![
        feather_ui::icones::icone(icone, 14.0),
        text(rotulo.to_owned()).size(11.5),
    ].spacing(6).align_y(Alignment::Center))
        .style(estilo_botao_pequeno(ativo))
        .padding([6, 9])
        .on_press(evento)
}

pub(super) fn linha<'a>(titulo: &str, dica: &str, controle: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    // Versão compacta (padding vertical 7) para a janela caber sem rolagem.
    iced::widget::column![
        row![
            iced::widget::column![
                text(titulo.to_owned()).size(13.5).color(COR_TEXTO()),
                text(dica.to_owned()).size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(2).width(Length::Fill),
            controle.into(),
        ].spacing(14).align_y(Alignment::Center).padding([7, 0]),
        container(Space::with_height(1)).width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(COR_BORDA_SUAVE())),
                ..Default::default()
            }),
    ].spacing(0).width(Length::Fill).into()
}

/// Recuperação e salvamento automáticos (Configurações e Primeiros passos).
pub(super) fn linhas_salvamento(app: &App) -> Element<'_, Msg> {
    iced::widget::column![
        linha(tr!("Recuperação automática"),
            tr!("Vale para qualquer planilha alterada: .fshz, .xlsx, .csv, .parquet e as nunca salvas. Guarda uma cópia à parte (não mexe no seu arquivo); se o computador desligar ou o app fechar sem salvar, ela aparece na tela inicial para recuperar."),
            toggler(app.config.recuperacao_automatica).on_toggle(Msg::RecuperacaoAutomatica).style(estilo_toggle)),
        linha(tr!("Salvamento automático"),
            tr!("Só para arquivos .fshz já salvos: grava por cima deles. Um .xlsx, .csv ou .parquet aberto e planilhas novas NÃO são gravados sozinhos; use Salvar como… em .fshz para que passem a ser."),
            toggler(app.config.auto_salvar).on_toggle(Msg::AutoSalvar).style(estilo_toggle)),
        if app.config.auto_salvar {
            linha(tr!("Gravar após inatividade"), tr!("Intervalo mínimo; pode aumentar se o arquivo for muito grande ou a gravação anterior for lenta."),
                feather_ui::componentes::lista_suspensa(
                    tr!("Intervalo"), &["2 s", "5 s", "15 s", "30 s", "60 s"],
                    Some(match app.config.intervalo_auto_salvar {
                        2 => "2 s", 5 => "5 s", 30 => "30 s", 60 => "60 s", _ => "15 s",
                    }),
                    |valor| Msg::IntervaloAutoSalvar(
                        valor.split_whitespace().next().unwrap_or("15").parse().unwrap_or(15)
                    ),
                    110.0,
                ))
        } else {
            Space::with_height(0).into()
        },
    ].spacing(0).into()
}

/// Automático / Claro / Escuro (Configurações e Primeiros passos).
pub(super) fn opcoes_tema(app: &App) -> Element<'_, Msg> {
    row![
        opcao(tr!("Automático"), "device-desktop", app.config.tema_auto, Msg::TemaAuto),
        opcao(tr!("Claro"), "eye", !app.config.tema_auto && !app.config.escuro, Msg::Tema(false)),
        opcao(tr!("Escuro"), "eye-off", !app.config.tema_auto && app.config.escuro, Msg::Tema(true)),
    ].spacing(3).into()
}

/// Idioma da interface e das fórmulas (Configurações e Primeiros passos).
pub(super) fn lista_idioma(app: &App) -> Element<'_, Msg> {
    feather_ui::componentes::lista_suspensa(
        tr!("Idioma"), &["Português (Brasil)", "English (United States)", "Español"],
        Some(match app.config.idioma.as_str() {
            "en-US" => "English (United States)", "es-ES" => "Español", _ => "Português (Brasil)",
        }),
        |v| Msg::Idioma(match v { "English (United States)" => "en-US", "Español" => "es-ES", _ => "pt-BR" }),
        210.0,
    ).into()
}

/// As amostras da cor de destaque (Configurações e Primeiros passos).
pub(super) fn amostras_acento(app: &App) -> Element<'_, Msg> {
    let mut acentos = row![].spacing(5);
    for (i, (nome, claro, escuro_hex)) in ACENTOS.iter().enumerate() {
        let selecionado = app.config.acento == i;
        let valor = u32::from_str_radix(
            (if escuro() { escuro_hex } else { claro }).trim_start_matches('#'), 16
        ).unwrap_or(0);
        let cor = iced::Color::from_rgb8(
            (valor >> 16) as u8, (valor >> 8) as u8, valor as u8
        );
        let amostra = container(Space::with_width(Length::Fixed(22.0)))
            .width(Length::Fixed(24.0))
            .height(Length::Fixed(24.0))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(cor)),
                border: iced::Border {
                    color: if selecionado { COR_FOCO(COR_PAINEL()) } else { COR_BORDA_SUAVE() },
                    width: if selecionado { 2.0 } else { 1.0 },
                    radius: 5.0.into(),
                },
                ..Default::default()
            });
        acentos = acentos.push(iced::widget::tooltip(
            button(amostra).padding([4, 4])
                .style(estilo_botao_pequeno(selecionado))
                .on_press(Msg::Acento(i)),
            container(text((*nome).to_owned()).size(11))
                .padding([4, 7]).style(estilo_dica),
            iced::widget::tooltip::Position::Bottom,
        ));
    }
    acentos.into()
}

/// Índices das seções no menu à esquerda (a ordem de `secoes` em `visualizar`).
pub(super) const SECAO_MCP: usize = 5;
pub(super) const SECAO_SOBRE: usize = 6;

pub(super) fn visualizar(app: &App) -> Element<'_, Msg> {
    let secoes = [tr!("Aparência"), tr!("Salvamento"), tr!("Planilha"), "OpenQuery", tr!("Atalhos"), tr!("IA (MCP)"), tr!("Sobre")];
    let menu = feather_ui::configuracoes::menu(
        tr!("Configurações"),
        secoes.iter().enumerate().map(|(i, nome)|
            ((*nome).to_string(), i == app.config_secao, Msg::SecaoConfig(i))),
    );
    let corpo: Element<'_, Msg> = match app.config_secao {
        0 => {
            let escolhido = nome_zoom(ZOOM_OPCOES, app.config.zoom);
            let minimo = nome_zoom(ZOOM_LIMITES, app.config.zoom_minimo);
            let maximo = nome_zoom(ZOOM_LIMITES, app.config.zoom_maximo);
            iced::widget::column![
                linha(tr!("Tema"), tr!("Automático acompanha o sistema na abertura."), opcoes_tema(app)),
                linha(tr!("Cor de destaque"), tr!("Personaliza a interface."), amostras_acento(app)),
                linha(tr!("Zoom padrão"), tr!("Aproximação aplicada à grade e salva localmente."),
                    feather_ui::componentes::lista_suspensa(
                        tr!("Zoom"), ZOOM_OPCOES,
                        escolhido,
                        |v| Msg::Zoom(v.trim_end_matches('%').parse().unwrap_or(100)),
                        115.0,
                    )),
                linha(tr!("Limites de zoom"), tr!("Define a faixa permitida (25% a 400%)."),
                    row![
                        text(tr!("Mín.")).size(11).color(COR_TEXTO_MUTED()),
                        feather_ui::componentes::lista_suspensa(
                            tr!("Mínimo"), ZOOM_LIMITES, minimo,
                            |v| Msg::ZoomMinimo(v.trim_end_matches('%').parse().unwrap_or(50)),
                            102.0,
                        ),
                        text(tr!("Máx.")).size(11).color(COR_TEXTO_MUTED()),
                        feather_ui::componentes::lista_suspensa(
                            tr!("Máximo"), ZOOM_LIMITES, maximo,
                            |v| Msg::ZoomMaximo(v.trim_end_matches('%').parse().unwrap_or(200)),
                            102.0,
                        ),
                    ].spacing(6).align_y(Alignment::Center)),
                linha(tr!("Letra da grade"), tr!("Aplicada às células sem fonte própria."),
                    feather_ui::componentes::lista_suspensa(
                        tr!("Fonte"), crate::idioma::lista(&["Sem serifa", "Com serifa", "Monoespaçada"]),
                        Some(match app.config.fonte_grade.as_str() {
                            "mono" => tr!("Monoespaçada"), "serif" => tr!("Com serifa"), _ => tr!("Sem serifa"),
                        }),
                        |v| Msg::FonteGrade(match crate::idioma::chave_de(&[tr!("Monoespaçada"), tr!("Com serifa")], v).unwrap_or("") {
                            "Monoespaçada" => "mono", "Com serifa" => "serif", _ => "sans",
                        }),
                        152.0,
                    )),
                linha(tr!("Corpo da letra"), tr!("Tamanho padrão em pixels."),
                    feather_ui::componentes::lista_suspensa(
                        tr!("Tamanho"), &["11", "12", "13", "14", "16"],
                        Some(match app.config.tamanho_fonte_grade {
                            11 => "11", 12 => "12", 14 => "14", 16 => "16", _ => "13",
                        }),
                        |v| Msg::CorpoGrade(v.parse().unwrap_or(13)),
                        94.0,
                    )),
                linha(tr!("Barra de janela"), tr!("Reinicie para aplicar a alteração."),
                    feather_ui::componentes::lista_suspensa(
                        tr!("Barra"), crate::idioma::lista(&["Personalizada", "Sistema"]),
                        Some(if app.config.barra_propria { tr!("Personalizada") } else { tr!("Sistema") }),
                        |v| Msg::BarraPropria(v == tr!("Personalizada")),
                        145.0,
                    )),
                text(tr!("Esquemas completos de cores ainda não estão disponíveis no motor nativo."))
                    .size(11).color(COR_TEXTO_MUTED()),
            ].spacing(0).into()
        }
        1 => {
            let nome = app.caminho.as_ref().and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| tr!("Nenhum arquivo salvo ainda").to_owned());
            iced::widget::column![
                linha(tr!("Salvar"), &nome,
                    row![
                        opcao(tr!("Salvar agora"), "device-floppy", false, Msg::Salvar),
                        opcao(tr!("Salvar como…"), "file-export", false, Msg::SalvarComo),
                    ].spacing(6)),
                linhas_salvamento(app),
                linha(tr!("Cópia de segurança"),
                    tr!("Cria uma cópia independente do último .fshz salvo, sem carregar seus dados na RAM."),
                    row![
                        opcao(tr!("Criar backup"), "copy", false, Msg::BackupCriar),
                        opcao(tr!("Recuperar cópia…"), "folder-open", false, Msg::RecuperarTemporario),
                    ].spacing(6)),
                text(tr!("O salvamento automático não substitui cópias de segurança. Para documentos novos, use Salvar como… primeiro."))
                    .size(11).color(COR_TEXTO_MUTED()),
            ].spacing(0).into()
        }
        2 => iced::widget::column![
            linha(tr!("Idioma"), tr!("Muda nomes das funções (SOMA ou SUM), separadores (; ou ,) e a Ajuda F1."),
                lista_idioma(app)),
            linha(tr!("Separadores de fórmulas e texto/CSV"), tr!("Definidos pelo idioma; Padrão aplicado às fórmulas e ao texto/CSV."),
                text(if app.config.idioma == "en-US" { tr!("Vírgula (,)") } else { tr!("Ponto e vírgula (;)") }).size(12)),
            linha(tr!("Detectar separador do arquivo CSV"), tr!("Desligado: usa o idioma (; em português, , em inglês). Ligado: detecta um único separador; os demais ficam no texto."),
                toggler(app.config.csv_detectar_separador).on_toggle(Msg::CsvDetectarSeparador).style(estilo_toggle)),
            linha(tr!("Linhas de grade"), tr!("Mostra ou oculta o quadriculado das células."),
                toggler(app.linhas_grade).on_toggle(Msg::LinhasGrade).style(estilo_toggle)),
            linha(tr!("Barra de fórmulas"), tr!("Campo fx acima da planilha."),
                toggler(app.mostrar_fx).on_toggle(|_| Msg::AlternarBarraFx).style(estilo_toggle)),
            linha(tr!("Mostrar fórmulas"), tr!("Mostra a expressão em vez do resultado."),
                toggler(app.mostrar_formulas).on_toggle(|_| Msg::AlternarMostrarFormulas).style(estilo_toggle)),
            linha(tr!("Realçar linha e coluna"), tr!("Destaca os eixos da célula selecionada."),
                toggler(app.config.realce_eixos).on_toggle(Msg::RealceEixos).style(estilo_toggle)),
            linha(tr!("Faixa de opções"), tr!("Ctrl+F1 recolhe ou expande a faixa."),
                toggler(!app.config.faixa_recolhida).on_toggle(|_| Msg::Faixa).style(estilo_toggle)),
            linha(tr!("Monitor de atividade"), tr!("CPU, memória e informações da sessão."),
                opcao(tr!("Abrir monitor"), "device-desktop", false, Msg::Monitor)),
            linha(tr!("Vigia de memória ao abrir"),
                &descricao_memoria(),
                feather_ui::componentes::lista_suspensa(
                    tr!("Limite"), crate::idioma::lista(super::limite_memoria::opcoes_limite()),
                    Some(match app.config.limite_memoria_gb {
                        0 => tr!("0 GB (desligado)"), 2 => "2 GB", 4 => "4 GB", 8 => "8 GB",
                        12 => "12 GB", 16 => "16 GB", 32 => "32 GB", 64 => "64 GB",
                        128 => "128 GB", _ => "6 GB",
                    }),
                    |v| Msg::LimiteMemoriaGb(v.split_whitespace().next().unwrap_or("6").parse().unwrap_or(6)),
                    165.0,
                )),
            linha(tr!("Núcleos para abrir e calcular"),
                &descricao_nucleos(),
                feather_ui::componentes::lista_suspensa(
                    tr!("Núcleos"), &NUCLEOS[..feathersheet_core::paralelismo::maximo().min(NUCLEOS.len())],
                    NUCLEOS.get(feathersheet_core::paralelismo::nucleos().saturating_sub(1)).copied(),
                    |v| Msg::Nucleos(v.parse().unwrap_or(0)),
                    165.0,
                )),
            linha(tr!("Janela no Linux (Wayland)"),
                tr!("Com mesa digitalizadora a janela abre pelo XWayland: no Wayland nativo a ponta da caneta não clica. Vale na próxima abertura."),
                feather_ui::componentes::lista_suspensa(
                    tr!("Janela"), crate::idioma::lista(&crate::configuracao::ROTULOS_JANELAS_LINUX),
                    crate::configuracao::JANELAS_LINUX.iter().find(|(_, v)| *v == app.config.janela_linux).map(|(r, _)| tr!(*r)),
                    |r| Msg::Caneta(super::desenhar::AjusteCaneta::Janela(
                        crate::configuracao::JANELAS_LINUX.iter().find(|(x, _)| tr!(*x) == r).map_or("auto", |(_, v)| *v))),
                    230.0,
                )),
            linha(tr!("Caneta: usar pressão da mesa"),
                tr!("Lê a força da caneta (XInput2 numa janela X11/XWayland, Windows Ink no Windows). Vale na próxima abertura."),
                toggler(app.config.caneta_pressao)
                    .on_toggle(|v| Msg::Caneta(super::desenhar::AjusteCaneta::Pressao(v))).style(estilo_toggle)),
            linha(tr!("Curva da pressão"), tr!("Leve: pouca força já engrossa o traço. Firme: pede mais força."),
                feather_ui::componentes::lista_suspensa(
                    tr!("Curva"), crate::idioma::lista(&crate::configuracao::ROTULOS_CURVAS_CANETA),
                    crate::configuracao::CURVAS_CANETA.iter().find(|(_, g)| *g == app.config.caneta_gama).map(|(n, _)| tr!(*n)),
                    |v| Msg::Caneta(super::desenhar::AjusteCaneta::Curva(
                        crate::configuracao::CURVAS_CANETA.iter().find(|(n, _)| tr!(*n) == v).map_or(0.75, |(_, g)| *g))),
                    165.0,
                )),
            linha(tr!("Sem mesa: espessura pela velocidade"), tr!("Com mouse ou caneta sem pressão, o traço rápido sai mais fino."),
                toggler(app.config.caneta_velocidade)
                    .on_toggle(|v| Msg::Caneta(super::desenhar::AjusteCaneta::Velocidade(v))).style(estilo_toggle)),
            linha(tr!("Mostrar a pressão durante o traço"), tr!("Indicador \"pressão 0,63\" no canto da planilha (para testar a mesa)."),
                toggler(app.config.caneta_mostrar_pressao)
                    .on_toggle(|v| Msg::Caneta(super::desenhar::AjusteCaneta::MostrarPressao(v))).style(estilo_toggle)),
            linha(tr!("Caneta entra no modo desenhar automaticamente"),
                tr!("Encostar a ponta na planilha já risca (a borracha da caneta apaga); o mouse ou Esc voltam à seleção. Faixa, menus e janelas continuam recebendo clique."),
                toggler(app.config.caneta_auto_desenhar)
                    .on_toggle(|v| Msg::Caneta(super::desenhar::AjusteCaneta::AutoDesenhar(v))).style(estilo_toggle)),
            text(tr!("Largura e altura padrão e estilo inicial de tabelas: em breve."))
                .size(11).color(COR_TEXTO_MUTED()),
        ].spacing(0).into(),
        3 => iced::widget::column![
            text("OpenQuery").size(16).color(COR_TEXTO()),
            text(tr!("O editor de consultas ainda não foi integrado às Configurações."))
                .size(12).color(COR_TEXTO_MUTED()),
            text(tr!("O limite padrão de pré-visualização será configurável quando o executor de consultas estiver disponível. Nenhuma consulta será modificada por esta tela."))
                .size(12).color(COR_TEXTO_MUTED()),
        ].spacing(12).into(),
        4 => {
            let mut col = iced::widget::column![
                linha(tr!("Personalizar atalhos"),
                    tr!("Selecione uma combinação e pressione o novo atalho. Esc cancela."),
                    opcao(tr!("Restaurar padrões"), "refresh", false, Msg::RestaurarAtalhos)),
            ].spacing(0);
            for &(id, rotulo, padrao) in ATALHOS {
                let capturando = app.atalho_captura == Some(id);
                let atual = app.config.atalhos.get(id).map(String::as_str).unwrap_or(padrao);
                col = col.push(linha(tr!(rotulo), &trf!("Padrão: {padrao}"),
                    button(text(if capturando { tr!("Pressione teclas…").to_owned() } else { atual.to_owned() }).size(12))
                        .padding([7, 12])
                        .width(Length::Fixed(154.0))
                        .style(estilo_botao_pequeno(capturando))
                        .on_press(Msg::CapturarAtalho(id))));
            }
            col.into()
        }
        SECAO_MCP => secao_mcp(app),
        _ => iced::widget::column![
            text(trf!("FeatherSheet {}", super::atualizacao::versao_atual())).size(17),
            secao_atualizacao(app),
            text(tr!("Aplicativo desktop nativo. As células, fórmulas e arquivos são processados localmente."))
                .size(12),
            text(tr!("As preferências são guardadas no diretório de configuração do usuário, fora do arquivo da planilha."))
                .size(12),
            Space::with_height(4),
            feather_ui::creditos::colaboradores(),
        ].spacing(12).into()
    };
    // Fecha pelo X da barra de título, como Docs e Slide: sem segundo "x" no corpo.
    feather_ui::configuracoes::tela(menu, secoes[app.config_secao], corpo, None)
}

/// Comando que liga uma IA ao FeatherSheet: o próprio executável com `--mcp`.
fn comando_mcp() -> (String, String) {
    let exe = std::env::current_exe().map(|p| p.display().to_string())
        .unwrap_or_else(|_| "feathersheet".into());
    let claude_code = format!("claude mcp add feathersheet -- \"{exe}\" --mcp");
    let json = serde_json::json!({ "mcpServers": { "feathersheet": { "command": exe, "args": ["--mcp"] } } });
    (claude_code, serde_json::to_string_pretty(&json).unwrap_or_default())
}

/// Configurações → IA (MCP): ligar o canal ao vivo, somente leitura, como
/// conectar e o que a IA pode fazer. Nada sai da máquina: o canal é local.
fn secao_mcp(app: &App) -> Element<'_, Msg> {
    let (claude_code, json) = comando_mcp();
    let estado = if !app.config.mcp_ativo {
        tr!("Desligado").to_owned()
    } else if super::mcp_vivo::escutando() {
        trf!("Ligado · {} pedido(s) nesta sessão", super::mcp_vivo::pedidos())
    } else {
        app.mcp_estado.clone()
    };
    let codigo = |t: String| container(text(t.clone()).size(11).font(iced::Font::MONOSPACE))
        .padding([8, 10]).width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(COR_PAINEL_FAIXA())),
            border: iced::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: 6.0.into() },
            ..Default::default()
        });
    let mut ferramentas = iced::widget::column![].spacing(3);
    for f in feathersheet_mcp::esquemas::todas() {
        let nome = f["name"].as_str().unwrap_or("").to_owned();
        let le = feathersheet_mcp::permissoes::so_le(&nome, &serde_json::json!({ "acao": "listar" }));
        let pode = app.config.mcp_ativo && (le || !app.config.mcp_so_leitura);
        ferramentas = ferramentas.push(row![
            feather_ui::icones::icone(if pode { "check" } else { "x" }, 13.0),
            text(nome).size(12).font(iced::Font::MONOSPACE).width(Length::Fixed(170.0)),
            text(if le { tr!("lê") } else { tr!("escreve ou age na tela") }).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(8).align_y(Alignment::Center));
    }
    iced::widget::column![
        linha(tr!("Permitir que uma IA use o FeatherSheet"),
            tr!("Desligado: nenhuma IA alcança a planilha aberta. Ligado: um assistente desta máquina (Claude, por exemplo) lê, resume e edita a planilha da tela; cada chamada que escreve é um Ctrl+Z."),
            toggler(app.config.mcp_ativo).on_toggle(Msg::McpAtivo).style(estilo_toggle)),
        linha(tr!("Somente leitura"),
            tr!("A IA pode ler, resumir, procurar e conferir, mas não escreve, não formata, não salva nem aperta botões."),
            toggler(app.config.mcp_so_leitura).on_toggle(Msg::McpSoLeitura).style(estilo_toggle)),
        linha(tr!("Estado"), tr!("O canal só aceita programas desta máquina que conhecem a chave da sessão."),
            text(estado).size(12)),
        text(tr!("Conectar ao Claude Code (terminal)")).size(13),
        row![codigo(claude_code.clone()),
            opcao(tr!("Copiar"), "copy", false, Msg::CopiarTexto(claude_code))].spacing(8).align_y(Alignment::Center),
        text(tr!("Conectar a um aplicativo de IA (arquivo de configuração MCP)")).size(13),
        row![codigo(json.clone()),
            opcao(tr!("Copiar"), "copy", false, Msg::CopiarTexto(json))].spacing(8).align_y(Alignment::Start),
        text(tr!("Com o FeatherSheet fechado, o mesmo comando trabalha direto nos arquivos .fshz, .xlsx e .csv.")).size(11).color(COR_TEXTO_MUTED()),
        text(tr!("O que a IA pode fazer")).size(13),
        ferramentas,
        text(tr!("Nada sai do computador: o FeatherSheet não envia a planilha para lugar nenhum. É o seu assistente de IA que decide o que fazer com o que lê.")).size(11).color(COR_TEXTO_MUTED()),
    ].spacing(10).into()
}

/// "Procurar atualizações": só consulta o GitHub quando a pessoa clica, e instalar
/// abre o terminal do sistema com cada passo à vista (ver `atualizacao.rs`).
fn secao_atualizacao(app: &App) -> Element<'_, Msg> {
    use super::atualizacao::{instalacao_atual, Estado, Evento};
    let ev = Msg::Atualizacao;
    let inst = instalacao_atual();
    let procurar = opcao(tr!("Procurar atualizações"), "refresh", false, ev(Evento::Procurar));
    let estado: Element<'_, Msg> = match &app.atualizacao {
        Estado::Parado => text(tr!("Consulta só github.com/feather-works/feathersheet quando você clicar. Nada sobre você ou suas planilhas é enviado."))
            .size(11.5).color(COR_TEXTO_MUTED()).into(),
        Estado::Procurando => text(tr!("Procurando no GitHub…")).size(12).color(COR_TEXTO_MUTED()).into(),
        Estado::EmDia => row![feather_ui::icones::icone("check", 14.0),
            text(tr!("Você já está na versão mais nova.")).size(12)].spacing(6).align_y(Alignment::Center).into(),
        Estado::Disponivel(l) => {
            let notas: String = l.notas.lines().filter(|x| !x.trim().is_empty()).take(6).collect::<Vec<_>>().join("\n");
            let mut acoes = row![opcao(tr!("Ver no GitHub"), "info-circle", false, ev(Evento::AbrirPagina))].spacing(8);
            if inst.arquivo().is_some() {
                acoes = acoes.push(opcao(&trf!("Atualizar para a {}", l.versao), "terminal", true, ev(Evento::Instalar)));
            }
            iced::widget::column![
                text(trf!("Versão {} disponível.", l.versao)).size(13),
                text(notas).size(11.5).color(COR_TEXTO_MUTED()),
                acoes,
                text(if inst.pede_senha() {
                    tr!("Atualizar abre o terminal com os passos à vista: baixa do GitHub, confere o arquivo e instala por cima (pede sua senha lá).")
                } else if inst.arquivo().is_some() {
                    tr!("Atualizar abre o terminal com os passos à vista: baixa do GitHub, confere o arquivo e instala por cima.")
                } else {
                    tr!("Esta cópia não veio de um instalador: baixe a versão nova pela página do GitHub.")
                }).size(11).color(COR_TEXTO_MUTED()),
            ].spacing(8).into()
        }
        Estado::Instalando(msg) => row![feather_ui::icones::icone("terminal", 14.0), text(msg.clone()).size(12)]
            .spacing(6).align_y(Alignment::Center).into(),
        Estado::Erro(msg) => row![feather_ui::icones::icone("alert-triangle", 14.0), text(msg.clone()).size(12)]
            .spacing(6).align_y(Alignment::Center).into(),
    };
    iced::widget::column![
        linha(tr!("Atualizações"), inst.descricao(), procurar),
        estado,
    ].spacing(8).into()
}

/// Apenas comandos com implementação nativa real são editáveis.
pub(super) fn comando_do_atalho(app: &App, combo: &str) -> Option<Msg> {
    ATALHOS.iter().find_map(|&(id, _, padrao)| {
        let atual = app.config.atalhos.get(id).map(String::as_str).unwrap_or(padrao);
        if atual != combo { return None; }
        match id {
            "save" => Some(Msg::Salvar),
            "saveAs" => Some(Msg::SalvarComo),
            "createTable" => Some(Msg::TabelaAbrir),
            "toggleFilter" => Some(Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro)),
            "findReplace" => Some(Msg::BuscaAba(true)),
            "toggleFormulas" => Some(Msg::AlternarMostrarFormulas),
            "colarValores" => Some(Msg::ColarValoresSolicitar),
            _ => None,
        }
    })
}

/// Não permitir combinações globais, edição básica ou duplicadas.
pub(super) fn atalho_valido(app: &App, id: &str, combo: &str) -> Result<(), &'static str> {
    if !combo.contains('+') || combo.len() > 40 {
        return Err(tr!("Escolha um atalho com Ctrl ou Alt."));
    }
    if matches!(combo,
        "Ctrl+C" | "Ctrl+X" | "Ctrl+V" | "Ctrl+Z" | "Ctrl+Y" |
        "Ctrl+B" | "Ctrl+I" | "Ctrl+U" | "Ctrl+N" | "Ctrl+O" |
        "Ctrl+F" | "Ctrl+F1" | "Alt+F11" | "Ctrl+Shift+M"
    ) {
        return Err(tr!("Combinação reservada por outro comando."));
    }
    for &(outro, _, padrao) in ATALHOS {
        let atual = app.config.atalhos.get(outro).map(String::as_str).unwrap_or(padrao);
        if outro != id && atual == combo { return Err(tr!("Este atalho já está em uso.")); }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    #[test]
    fn atalhos_editados_despacham_comando_real_e_respeitam_conflitos() {
        let mut app = App::default();
        assert!(matches!(comando_do_atalho(&app, "Ctrl+S"), Some(Msg::Salvar)));
        app.config.atalhos.insert("save".into(), "Alt+S".into());
        assert!(comando_do_atalho(&app, "Ctrl+S").is_none());
        assert!(matches!(comando_do_atalho(&app, "Alt+S"), Some(Msg::Salvar)));
        assert!(atalho_valido(&app, "saveAs", "Alt+S").is_err());
        assert!(atalho_valido(&app, "save", "Ctrl+C").is_err());
    }

    #[test]
    fn menu_nativo_possui_sete_secoes_ativas() {
        let mut app = App::default();
        for secao in 0..=SECAO_SOBRE {
            app.config_secao = secao;
            let _: Element<'_, Msg> = visualizar(&app);
        }
    }

    #[test]
    fn sobre_abre_a_ultima_secao_e_o_mcp_tem_a_sua() {
        let mut app = App::default();
        let _ = app.update(Msg::MostrarSobre);
        assert_eq!(app.config_secao, SECAO_SOBRE);
        let _ = app.update(Msg::SecaoConfig(SECAO_MCP));
        assert_eq!(app.config_secao, SECAO_MCP);
        let _ = app.update(Msg::SecaoConfig(99));
        assert_eq!(app.config_secao, SECAO_SOBRE);
    }

    #[test]
    fn preferencias_da_grade_atualizam_a_configuracao_sem_editar_workbook() {
        let mut app = App::default();
        let antes = app.workbook.aba(0).unwrap().sheet.len();
        app.update(Msg::LinhasGrade(false));
        app.update(Msg::AlternarMostrarFormulas);
        app.update(Msg::FonteGrade("mono"));
        app.update(Msg::CorpoGrade(16));
        assert!(!app.linhas_grade && !app.config.linhas_grade);
        assert!(app.mostrar_formulas && app.config.mostrar_formulas);
        assert_eq!(app.config.fonte_grade, "mono");
        assert_eq!(app.config.tamanho_fonte_grade, 16);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.len(), antes);
        assert!(!app.alterado);
    }

    #[test]
    fn limites_zoom_sao_persistentes_sem_editar_celulas() {
        let mut app = App::default();
        let antes = app.workbook.aba(0).unwrap().sheet.len();
        app.update(Msg::ZoomMinimo(125));
        assert_eq!((app.config.zoom_minimo, app.config.zoom_maximo, app.config.zoom), (125, 200, 125));
        app.update(Msg::Zoom(60));
        assert_eq!(app.config.zoom, 125);
        app.update(Msg::ZoomMaximo(300));
        app.update(Msg::Zoom(400));
        assert_eq!(app.config.zoom, 300);
        app.update(Msg::ControleZoom(true));
        app.update(Msg::Roda(-1));
        assert_eq!(app.config.zoom, 250);
        app.update(Msg::ZoomMaximo(75));
        assert_eq!((app.config.zoom_minimo, app.config.zoom_maximo, app.config.zoom), (75, 75, 75));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.len(), antes);
        assert!(!app.alterado);
    }

    #[test]
    fn autosave_nunca_abre_dialogo_para_arquivo_sem_caminho() {
        let mut app = App::default();
        app.config.auto_salvar = true;
        app.config.intervalo_auto_salvar = 2;
        app.alterado = true;
        app.auto_salvar_desde = Some(Instant::now() - Duration::from_secs(3));
        app.update(Msg::MonitorPulso);
        assert!(app.alterado);
        assert!(app.caminho.is_none());
    }

    #[test]
    fn autosave_grava_documento_fshz_apos_inatividade() {
        let mut app = App::default();
        let dir = std::env::temp_dir().join(format!(
            "feathersheet-autosave-{}-{:?}", std::process::id(), std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("salvamento.fshz");
        app.workbook.escrever_celula(0, 0, 0, "42".into());
        app.caminho = Some(path.clone());
        app.alterado = true;
        app.config.auto_salvar = true;
        app.config.intervalo_auto_salvar = 2;
        app.auto_salvar_desde = Some(Instant::now() - Duration::from_secs(3));
        let _ = app.update(Msg::MonitorPulso);
        app.concluir_gravacao_teste(path.clone(), false);
        assert!(!app.alterado, "gravação periódica deve limpar o sinal de alterações");
        assert!(std::fs::metadata(&path).unwrap().len() > 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn autosave_adapta_intervalo_quando_gravacao_anterior_foi_lenta() {
        let mut app = App::default();
        let dir = std::env::temp_dir().join(format!(
            "feathersheet-autosave-lento-{}-{:?}",
            std::process::id(), std::thread::current().id(),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auto-lento.fshz");
        app.workbook.escrever_celula(0, 0, 0, "123".into());
        app.caminho = Some(path.clone());
        app.alterado = true;
        app.config.auto_salvar = true;
        app.config.intervalo_auto_salvar = 2;
        // TS: 8 segundos por snapshot -> o próximo só após 30 segundos.
        app.auto_salvar_ultimo_custo_ms = 8_000;
        app.auto_salvar_desde = Some(Instant::now() - Duration::from_secs(5));
        app.update(Msg::MonitorPulso);
        assert!(app.alterado, "não salvar em loop quando I/O for pesado");
        assert!(!path.exists(), "não escreveu antes do cooldown");
        app.auto_salvar_desde = Some(Instant::now() - Duration::from_secs(31));
        let _ = app.update(Msg::MonitorPulso);
        app.concluir_gravacao_teste(path.clone(), false);
        assert!(!app.alterado, "autosave continua ativo após o cooldown");
        assert!(path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn idioma_configurado_localiza_edicao_sem_reinterpretar_documento() {
        let mut app = App::default();
        app.config.idioma = "en-US".into();
        crate::idioma::definir_na_thread("en-US");
        app.update(Msg::Editar("=SUM(1,2,3.5)".into()));
        app.confirmar();
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(6.5));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "=SUM(1;2;3.5)");
        app.trocar_selecao(0, 0);
        assert_eq!(app.edicao, "=SUM(1,2,3.5)");
        app.config.idioma = "pt-BR".into();
        crate::idioma::definir_na_thread("pt-BR");
        app.trocar_selecao(0, 0);
        // De-para dos nomes: em português a barra mostra SOMA.
        assert_eq!(app.edicao, "=SOMA(1;2;3.5)");
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(6.5));
    }

    #[test]
    fn dividir_texto_padrao_segue_idioma_das_configuracoes() {
        for (idioma, texto) in [("pt-BR", "Ana;Dev;ES"), ("en-US", "Ana,Dev,ES")] {
            let mut app = App::default();
            app.config.idioma = idioma.into();
            app.workbook.escrever_celula(0, 0, 0, texto.into());
            app.update(Msg::FaixaOpcao("dividir_texto", "Idioma"));
            assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("Ana".into()));
            assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("ES".into()));
        }
    }

}

/// Rótulos da lista de núcleos (a lista suspensa da `feather-ui` só aceita
/// texto fixo): mostra-se de 1 até o total − 1 do processador.
static NUCLEOS: [&str; 128] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "40", "41", "42", "43", "44", "45", "46", "47", "48", "49", "50", "51", "52", "53", "54", "55", "56", "57", "58", "59", "60", "61", "62", "63", "64", "65", "66", "67", "68", "69", "70", "71", "72", "73", "74", "75", "76", "77", "78", "79", "80", "81", "82", "83", "84", "85", "86", "87", "88", "89", "90", "91", "92", "93", "94", "95", "96", "97", "98", "99", "100", "101", "102", "103", "104", "105", "106", "107", "108", "109", "110", "111", "112", "113", "114", "115", "116", "117", "118", "119", "120", "121", "122", "123", "124", "125", "126", "127", "128"];

/// "AMD Ryzen 7 … · até 4,67 GHz · 16 threads (1 fica para a janela; padrão 15)."
fn descricao_nucleos() -> String {
    use feathersheet_core::paralelismo as p;
    let nome = p::nome_do_processador().unwrap_or_else(|| tr!("Processador").into());
    let ghz = p::frequencia_maxima_ghz()
        .map(|g| {
            let n = format!("{g:.2}");
            let n = if crate::idioma::lingua() == crate::idioma::Lingua::En { n } else { n.replace('.', ",") };
            trf!(" · até {n} GHz")
        })
        .unwrap_or_default();
    let threads = trf!("{} threads (1 fica para a janela e o sistema; padrão {}).",
        p::total_do_sistema(), p::padrao());
    format!("{nome}{ghz} · {threads}")
}

/// "32 GB de RAM instalados. Confere o uso…"
fn descricao_memoria() -> String {
    let base = tr!("Confere o uso de RAM entre etapas da leitura. 0 desliga; não é limite rígido.");
    match super::limite_memoria::ram_total_gb() {
        Some(gb) => trf!("{gb} GB de RAM instalados. {base}"),
        None => base.to_string(),
    }
}
