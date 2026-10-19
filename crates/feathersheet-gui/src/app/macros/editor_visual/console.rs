//! Console inferior, ferramentas de inspeção e barra de estado do VBE.
use super::*;
use iced::widget::column;

pub(super) fn terminal<'a>(app: &'a App, altura: f32) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let mut abas = row![].spacing(4);
    for (i, (nome, icone)) in [
        (tr!("Janela Imediata"), "terminal"),
        (tr!("Locais"), "list-details"),
        (tr!("Pilha de chamadas"), "sitemap"),
        (tr!("Inspeção"), "eye"),
        (tr!("Histórico"), "clock"),
    ].iter().enumerate() {
        abas = abas.push(botao_icone(nome, icone, event(EventoVisual::Painel(i as u8)),
            app.macro_visual.painel == i as u8));
    }
    abas = abas.push(Space::with_width(Length::Fill));
    let historico = app.macro_visual.painel == PAINEL_HISTORICO;
    // No Histórico, Limpar esvazia a lista de execuções; nos outros, a saída.
    abas = abas.push(if historico {
        botao_icone(tr!("Limpar histórico"), "trash",
            (app.historico_macros.total() > 0).then(|| Msg::MacroVisual(EventoVisual::HistoricoLimpar)), false)
    } else {
        botao_icone(tr!("Limpar"), "trash", event(EventoVisual::LimparSaida), false)
    });
    abas = abas.push(botao_icone_simples("x", event(EventoVisual::Console)));
    let body: Element<'a, Msg> = match app.macro_visual.painel {
        0 => {
            let saida = if app.macro_saida.trim().is_empty() {
                tr!("Saída das macros (console.log / log) aparece aqui.")
            } else { &app.macro_saida };
            column![
                scrollable(text(saida.to_owned()).font(iced::Font::MONOSPACE).size(12))
                    .height(Length::Fill),
                text_input(tr!("Digite uma instrução e pressione Enter (interativo pendente)"), "")
                    .size(12).padding([5, 7]),
            ].spacing(4).padding([5, 8]).into()
        }
        1 => column![
            text(tr!("Nome                         Valor                         Tipo")).size(12)
                .font(iced::Font::MONOSPACE),
            text(tr!("A macro não está pausada. Entre no depurador para inspecionar variáveis."))
                .size(12).color(cor_secundaria(escuro)),
        ].spacing(12).padding(12).into(),
        PAINEL_HISTORICO => super::historico::painel(app),
        2 => column![
            text(tr!("Nome da função                         Linha")).size(12)
                .font(iced::Font::MONOSPACE),
            text(tr!("A macro não está pausada. Pilha de chamadas indisponível."))
                .size(12).color(cor_secundaria(escuro)),
        ].spacing(12).padding(12).into(),
        _ => column![
            row![
                text_input(tr!("Adicionar expressão — ex.: sheet.get('A1')"), "")
                    .size(12).padding([5, 7]).width(Length::Fill),
                botao_icone_simples("plus", None),
                botao_icone_simples("refresh", None),
            ].spacing(4),
            text(tr!("Expressão                       Valor                  Tipo")).size(12)
                .font(iced::Font::MONOSPACE),
            text(tr!("Nenhuma inspeção. Avaliação de expressões ainda pendente."))
                .size(12).color(cor_secundaria(escuro)),
        ].spacing(8).padding(12).into(),
    };
    container(column![
        container(abas).padding([5, 8]).style(estilo_cabecalho(escuro)),
        body,
    ].spacing(4).padding(4))
        // Altura da divisória (o histórico começa mais alto: editor_visual.rs).
        .height(Length::Fixed(altura)).width(Length::Fill)
        .style(estilo_area(escuro)).into()
}

/// "Ln X, Col Y" do cursor no código (só com um módulo aberto).
pub(super) fn posicao_cursor(app: &App) -> String {
    if !app.macro_visual.documento.starts_with("modulo:") { return String::new(); }
    let (l, c) = crate::app::macros::codigo::linha_coluna(&app.macro_codigo);
    trf!("Ln {l}, Col {c}")
}

pub(super) fn rodape<'a>(app: &'a App) -> Element<'a, Msg> {
    let escuro = app.config.escuro;
    let nome = &app.macro_visual.documento;
    let total = app.macro_codigo.text().lines().count().max(1);
    let documento = if nome.starts_with("planilha:") { app.macro_nome.as_str() }
        else { nome.strip_prefix("modulo:").unwrap_or(nome) };
    container(row![
        text(tr!("FeatherWorks  /  Editor de macros")).size(10).color(cor_secundaria(escuro)),
        Space::with_width(Length::Fill),
        text(trf!("Módulo: {documento}")).size(10),
        text(posicao_cursor(app)).size(10),
        text(trf!("{total} linha(s)")).size(10),
        text(tr!("JavaScript (Boa)")).size(10),
        text(tr!("Execução local")).size(10).color(cor_secundaria(escuro)),
    ].spacing(16).align_y(Alignment::Center))
    .padding([7, 11]).width(Length::Fill)
    .style(estilo_cabecalho(escuro)).into()
}

