//! Aba "Histórico" do painel inferior do editor de macros: as execuções
//! desta sessão (`historico_macros.rs`), a mais recente em cima, com filtro
//! por macro, por gatilho e "só erros"; clicar numa linha abre o detalhe à
//! direita (erro inteiro, arquivos, respostas) e o "Ir para a macro".
use super::*;
use pelicano::column;
use crate::app::historico_macros::{respostas_texto, Execucao, ResultadoExecucao, TipoGatilho};
use feather_ui::tema::{COR_TEXTO, COR_TEXTO_MUTED};

/// Linhas desenhadas de uma vez (o anel tem até 500; a lista rola).
const LINHAS_VISIVEIS: usize = 500;

fn chip<'a>(rotulo: &str, icone: Option<&str>, acao: Msg, ativo: bool) -> Element<'a, Msg> {
    let mut conteudo = row(Vec::new()).spacing(4).align(pelicano::Align::Center);
    if let Some(icone) = icone { conteudo = conteudo.push(feather_ui::icones::icone(icone, 14.0)); }
    conteudo = conteudo.push(text(rotulo.to_owned()).size(11.5));
    button(conteudo).padding([4, 8]).on_press(acao).style(estilo_botao_ide(ativo)).into()
}

fn cortar(texto: &str, max: usize) -> String {
    let primeira = texto.lines().next().unwrap_or("");
    if primeira.chars().count() <= max && !texto.contains('\n') { return primeira.to_owned(); }
    let mut s: String = primeira.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn sem_prefixo(msg: &str) -> &str {
    msg.strip_prefix("Error: ").or_else(|| msg.strip_prefix("Erro: ")).unwrap_or(msg)
}

fn linha<'a>(e: &Execucao, escolhida: bool) -> Element<'a, Msg> {
    let muted = COR_TEXTO_MUTED();
    let resultado = match &e.resultado {
        // O motor já escreve "Error: …"; o ícone e a coluna dizem que é erro.
        ResultadoExecucao::Erro(msg) => trf!("Erro: {}", cortar(sem_prefixo(msg), 26)),
        outro => outro.rotulo().to_owned(),
    };
    let conteudo = row![
        text(e.hora.clone()).size(11.5).font(pelicano::Font::MONO).color(muted)
            .width(Length::Fixed(66.0)),
        container(row![
            feather_ui::icones::icone(e.resultado.icone(), 14.0),
            text(resultado).size(12),
        ].spacing(5).align(pelicano::Align::Center)).width(Length::Fill(5)),
        text(cortar(&e.macro_, 28)).size(12).width(Length::Fill(3)),
        container(row![
            feather_ui::icones::icone(e.gatilho.tipo().icone(), 14.0),
            text(cortar(&e.gatilho.descricao(), 40)).size(12),
        ].spacing(5).align(pelicano::Align::Center)).width(Length::Fill(5)),
        text(format!("{} ms", e.ms)).size(11.5).color(muted).width(Length::Fixed(62.0))
            .align_x(pelicano::Align::End),
    ].spacing(10).align(pelicano::Align::Center);
    button(conteudo).width(Length::FILL).padding([4, 8])
        .on_press(Msg::MacroVisual(EventoVisual::HistoricoSelecionar(e.id)))
        .style(estilo_botao_ide(escolhida)).into()
}

fn par<'a>(nome: &str, valor: String) -> Element<'a, Msg> {
    row![
        text(nome.to_owned()).size(11).color(COR_TEXTO_MUTED()).width(Length::Fixed(78.0)),
        text(valor).size(12).width(Length::FILL),
    ].spacing(6).into()
}

fn detalhe<'a>(app: &'a App, e: &Execucao) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let mut corpo = column![
        row![
            feather_ui::icones::icone(e.resultado.icone(), 17.0),
            text(e.resultado.rotulo()).size(13.5).font(pelicano::Font { weight: 600, ..Default::default() }),
        ].spacing(6).align(pelicano::Align::Center),
        par(tr!("Macro"), e.macro_.clone()),
        par(tr!("Gatilho"), e.gatilho.descricao()),
        par(tr!("Quando"), trf!("{} · {} ms", e.hora, e.ms)),
        par(tr!("Planilha"), if e.alterou { "alterada".into() } else { tr!("sem mudança").into() }),
    ].spacing(5);
    if !e.respostas.is_empty() {
        corpo = corpo.push(par(tr!("Respostas"), respostas_texto(&e.respostas)));
    }
    match &e.resultado {
        ResultadoExecucao::Erro(msg) => {
            corpo = corpo.push(text(tr!("Erro")).size(11).color(COR_TEXTO_MUTED()));
            corpo = corpo.push(container(text(msg.clone()).size(12).font(pelicano::Font::MONO))
                .padding([6, 8]).width(Length::FILL).style(estilo_cabecalho(escuro)));
        }
        ResultadoExecucao::Esperando(pergunta) => {
            corpo = corpo.push(par(tr!("Pergunta"), pergunta.clone()));
        }
        _ => {}
    }
    if !e.arquivos.is_empty() {
        corpo = corpo.push(text(tr!("Arquivos")).size(11).color(COR_TEXTO_MUTED()));
        for a in &e.arquivos {
            corpo = corpo.push(text(a.clone()).size(11.5));
        }
    }
    // Só módulos guardados no catálogo abrem no editor.
    let existe = app.macro_catalogo.obter(&e.macro_).is_some();
    let ir = botao_icone(tr!("Ir para a macro"), "file-code",
        existe.then(|| Msg::MacroVisual(EventoVisual::HistoricoIrPara(e.macro_.clone()))), false);
    let ir: Element<'a, Msg> = if existe { ir } else {
        pelicano::tooltip(ir, tr!("Esta macro não está guardada na biblioteca"),
            pelicano::TooltipPosition::Top).style(feather_ui::tema::estilo_dica).into()
    };
    container(column![
        scrollable(corpo.padding([0, 4])).height(Length::FILL),
        row![Space::new(Length::FILL, 0.0), ir],
    ].spacing(4))
        .padding([6, 8]).width(Length::Fixed(330.0)).height(Length::FILL)
        .style(estilo_cabecalho(escuro)).into()
}

fn vazio<'a>(titulo: &str, frase: &str) -> Element<'a, Msg> {
    container(column![
        feather_ui::icones::icone("clock", 30.0),
        text(titulo.to_owned()).size(13).color(COR_TEXTO()),
        text(frase.to_owned()).size(11.5).color(COR_TEXTO_MUTED()),
    ].spacing(6).align(pelicano::Align::Center))
        .width(Length::FILL).height(Length::FILL)
        .align_x(pelicano::Align::Center)
        .align_y(pelicano::Align::Center).into()
}

pub(super) fn painel<'a>(app: &'a App) -> Element<'a, Msg> {
    let h = &app.historico_macros;
    let ev = |e: EventoVisual| Msg::MacroVisual(e);
    let mut filtros = row![
        container(row![
            feather_ui::icones::icone("search", 14.0),
            text_input(tr!("Filtrar por macro"), &h.filtro_macro)
                .on_input(|t| Msg::MacroVisual(EventoVisual::HistoricoFiltroMacro(t)))
                .size(12).padding([4, 6]).width(Length::Fixed(190.0)),
        ].spacing(4).align(pelicano::Align::Center)),
        container(Space::new(Length::Fixed(1.0), 0.0)).height(Length::Fixed(20.0))
            .style(estilo_divisor(app.config.escuro)),
        chip(tr!("Todos"), None, ev(EventoVisual::HistoricoFiltroGatilho(None)), h.filtro_gatilho.is_none()),
    ].spacing(5).align(pelicano::Align::Center);
    for t in TipoGatilho::TODOS {
        filtros = filtros.push(chip(t.rotulo(), Some(t.icone()),
            ev(EventoVisual::HistoricoFiltroGatilho(Some(t))), h.filtro_gatilho == Some(t)));
    }
    let lista: Vec<&Execucao> = h.filtradas().take(LINHAS_VISIVEIS).collect();
    let erros = h.filtradas().filter(|e| e.erro()).count();
    filtros = filtros
        .push(container(Space::new(Length::Fixed(1.0), 0.0)).height(Length::Fixed(20.0))
            .style(estilo_divisor(app.config.escuro)))
        .push(chip(tr!("Só erros"), Some("bug"), ev(EventoVisual::HistoricoSoErros), h.so_erros))
        .push(Space::new(Length::FILL, 0.0))
        .push(text(trf!("{} de {} execuções · {} com erro", lista.len(), h.total(), erros))
            .size(11).color(COR_TEXTO_MUTED()));

    let corpo: Element<'a, Msg> = if h.total() == 0 {
        vazio(tr!("Nenhuma macro rodou nesta sessão"),
            tr!("Cada execução aparece aqui: Executar (F5), botão na planilha, evento ou MCP."))
    } else if lista.is_empty() {
        vazio(tr!("Nenhuma execução com estes filtros"), tr!("Mude o gatilho, o nome ou desligue \"Só erros\"."))
    } else {
        let cabecalho = row![
            text("HORA").size(10.5).width(Length::Fixed(66.0)),
            text("RESULTADO").size(10.5).width(Length::Fill(5)),
            text("MACRO").size(10.5).width(Length::Fill(3)),
            text("GATILHO").size(10.5).width(Length::Fill(5)),
            text(tr!("DURAÇÃO")).size(10.5).width(Length::Fixed(62.0)).align_x(pelicano::Align::End),
        ].spacing(10).padding([0, 8]);
        let cabecalho = pelicano::container(cabecalho).style(|_| pelicano::Style {
            text_color: Some(COR_TEXTO_MUTED()), ..Default::default()
        });
        let mut linhas = column(Vec::new()).spacing(1);
        for e in &lista {
            linhas = linhas.push(linha(e, h.selecionada == Some(e.id)));
        }
        let tabela = column![
            cabecalho,
            scrollable(linhas).height(Length::FILL),
        ].spacing(3).width(Length::FILL);
        let escolhida = h.selecionada.and_then(|id| h.obter(id));
        match escolhida {
            Some(e) => row![tabela, detalhe(app, e)].spacing(8).height(Length::FILL).into(),
            None => tabela.into(),
        }
    };
    column![filtros, corpo].spacing(6).padding([5, 8]).height(Length::FILL).into()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::app::historico_macros::GatilhoMacro;

    #[test]
    fn historico_monta_vazio_cheio_filtrado_e_com_detalhe_nos_dois_temas() {
        let mut app = App::default();
        app.evento_macro_visual(EventoVisual::Painel(PAINEL_HISTORICO));
        assert_eq!(app.macro_visual.painel, PAINEL_HISTORICO);
        for escuro in [false, true] {
            app.config.escuro = escuro;
            let _ = janela(&app);
        }
        app.rodar_macro(GatilhoMacro::manual("teste"), "Boa", "sheet.set('A1', 1);", vec![]);
        app.rodar_macro(GatilhoMacro::Botao { rotulo: "Ir".into() }, "Ruim", "throw new Error('x');", vec![]);
        let id = app.historico_macros.filtradas().next().unwrap().id;
        app.evento_macro_visual(EventoVisual::HistoricoSelecionar(id));
        assert_eq!(app.historico_macros.selecionada, Some(id));
        for escuro in [false, true] {
            app.config.escuro = escuro;
            let _ = janela(&app);
        }
        app.evento_macro_visual(EventoVisual::HistoricoSoErros);
        app.evento_macro_visual(EventoVisual::HistoricoFiltroMacro("nada".into()));
        let _ = janela(&app);
        app.evento_macro_visual(EventoVisual::HistoricoFiltroGatilho(Some(TipoGatilho::Botao)));
        app.evento_macro_visual(EventoVisual::HistoricoLimpar);
        assert_eq!(app.historico_macros.total(), 0);
        let _ = janela(&app);
    }

    #[test]
    fn menu_exibir_abre_o_historico() {
        let i = super::super::barra_menu::menu_itens(2).iter()
            .position(|(r, _)| r.starts_with("Histórico de macros")).expect("item no Exibir");
        assert!(matches!(acao_menu_visual(2, i),
            Some(Msg::MacroVisual(EventoVisual::Painel(PAINEL_HISTORICO)))));
    }

    #[test]
    fn ir_para_a_macro_abre_o_modulo_no_editor() {
        let mut app = App::default();
        app.macro_catalogo = Default::default();
        app.macro_catalogo.macros.push(feather_macros::Macro {
            name: "Relatorio".into(), code: "log(1);".into(), auto: false });
        app.evento_macro_visual(EventoVisual::HistoricoIrPara("Relatorio".into()));
        assert_eq!(app.macro_visual.documento, "modulo:Relatorio");
        assert!(app.macro_codigo.text().contains("log(1)"));
    }
}
