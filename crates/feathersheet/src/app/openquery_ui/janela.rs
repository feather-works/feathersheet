//! Janela própria do OpenQuery, no mesmo desenho do editor de macros (Alt+F11):
//! Ribbon com ícones cromáticos, explorador à esquerda e etapas à direita,
//! sem o painel permanente de propriedades. Opções aparecem sob demanda;
//! o log inferior fica recolhido. Só o desenho mora aqui; os dados
//! continuam no `App` e o motor de transformação no `feathersheet-core`.
use super::*;
use pelicano::column; use pelicano::row; use pelicano::scrollable;

// Estilos e controles compartilhados com o editor de macros (sem duplicar).
pub(super) use crate::app::macros::editor_visual::estilos as ide;

const LARGURA_ESQUERDA: f32 = 245.0;
const LARGURA_ETAPAS: f32 = 250.0;
const ALTURA_PAINEL_INFERIOR: f32 = 140.0;

/// Painel com cabeçalho (título + ícone) e corpo rolável só dentro da lista.
fn painel<'a>(
    titulo_painel: &str, nome_icone: &str, corpo: Element<'a, Msg>, altura: Length, escuro: bool,
) -> Element<'a, Msg> {
    container(column![
        container(row![
            text(titulo_painel.to_owned()).size(13),
            Space::new(Length::FILL, 0.0),
            feather_ui::icones::icone(nome_icone, 15.0),
        ].align(pelicano::Align::Center)).padding([11, 12]).width(Length::FILL)
            .style(ide::estilo_cabecalho(escuro)),
        // O conteúdo do scroll deve ter altura intrínseca, mesmo se um
        // subpainel retornar Element com altura Fill (Iced 0.13).
        scrollable(container(corpo).height(Length::Shrink).width(Length::FILL))
            .height(Length::FILL),
    ].spacing(3).padding(4))
        .style(ide::estilo_area(escuro)).width(Length::FILL).height(altura).into()
}

fn entrada<'a>(valor: &'a str, dica_campo: &'static str, evento: fn(String) -> Evento)
    -> Element<'a, Msg>
{
    text_input(dica_campo, valor)
        .on_input(move |v| om(evento(v)))
        .size(11).padding([4.0, 6.0])
        .style(estilo_entrada_suspensa).into()
}

fn botao_seta_cima<'a>(dica_texto: &str, evento: Evento) -> Element<'a, Msg> {
    dica(
        button(icone_seta_cima(15.0)).padding([5, 7]).on_press(om(evento))
            .style(ide::estilo_botao_ide(false)),
        dica_texto,
    )
}

fn botao_so_icone<'a>(
    nome_icone: &str, dica_texto: &str, evento: Evento,
) -> Element<'a, Msg> {
    dica(ide::botao_icone_simples(nome_icone, Some(om(evento))), dica_texto)
}

impl App {
    /// Esc: fecha o assistente modal, se houver. Devolve se havia um aberto.
    pub(in crate::app) fn fechar_assistente_openquery(&mut self) -> bool {
        let Some(e) = self.openquery.as_mut() else { return false };
        if e.editor_avancado.take().is_some() {
            e.script_avancado = None;
            e.erro_avancado.clear();
            return true;
        }
        e.assistente.take().is_some()
    }

    /// Conteúdo da janela auxiliar `Auxiliar::OpenQuery`.
    pub(in crate::app) fn view_openquery(&self) -> Element<'_, Msg> {
        let Some(e) = &self.openquery else {
            return Space::new(Length::FILL, Length::FILL).into();
        };
        let escuro = escuro();
        let visual = e.tabela_visual();
        let mut corpo = row(Vec::<Element<Msg>>::new()).spacing(8).height(Length::FILL);
        if !e.consultas_recolhidas {
            corpo = corpo.push(self.esquerda_openquery(e, escuro));
        }
        corpo = corpo.push(self.centro_openquery(e, escuro));
        if !e.etapas_recolhidas {
            corpo = corpo.push(self.etapas_openquery(e, escuro));
        }
        let mut estrutura = column![
            barra_menus(e, escuro),
            barra_faixa(e, escuro),
        ].spacing(6).height(Length::FILL).width(Length::FILL);
        if e.parametros_abertos {
            estrutura = estrutura.push(barra_parametros(e, escuro));
        }
        estrutura = estrutura.push(corpo);
        if e.painel_inferior_aberto {
            estrutura = estrutura.push(self.painel_inferior_openquery(e, visual, escuro));
        }
        estrutura = estrutura.push(self.status_openquery(e, visual, escuro));
        let base: Element<'_, Msg> = container(estrutura).padding(8)
            .style(ide::estilo_fundo_editor(escuro))
            .height(Length::FILL).width(Length::FILL).into();
        if e.editor_avancado.is_some() {
            let janela = opaque(container(opaque(self.painel_editor_avancado_openquery(e, escuro)))
                .padding([28, 44]).width(Length::FILL).align_x(pelicano::Align::Center).height(Length::FILL).align_y(pelicano::Align::Center)
                .width(Length::FILL).height(Length::FILL)
                .style(|_| pelicano::Style {
                    background: Some(Background::Color(pelicano::Color::rgba(0.0, 0.0, 0.0, 0.48))),
                    ..pelicano::Style::default()
                }));
            return stack![base, janela].height(Length::FILL).into();
        }
        if e.assistente.is_none() {
            return base;
        }
        // O assistente (mesclar, acrescentar, pivotar) é modal dentro desta janela.
        let assistente = opaque(container(opaque(self.painel_assistente_openquery(e)))
            .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::FILL).align_y(pelicano::Align::Center)
            .width(Length::FILL).height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(Background::Color(pelicano::Color::rgba(0.0, 0.0, 0.0, 0.40))),
                ..pelicano::Style::default()
            }));
        stack![base, assistente].height(Length::FILL).into()
    }

    /// Janela ampla, com rascunho editável e o script derivado somente leitura.
    fn painel_editor_avancado_openquery<'a>(
        &'a self, e: &'a Estado, escuro: bool,
    ) -> Element<'a, Msg> {
        let script = e.script_avancado.is_some();
        let editor = e.editor_avancado.as_ref().expect("editor avançado aberto");
        let titulo = if script { tr!("Script completo · visualização") }
            else { tr!("Editor avançado · etapas JavaScript") };
        let nota = if script {
            tr!("Código ilustrativo do fluxo completo. Não é executado como JavaScript: as etapas são processadas pelo FeatherSheet.")
        } else {
            tr!("Edite const etapas = [ ... ]; para transformar os dados. Aplicar valida todas as etapas antes de modificar a consulta.")
        };
        let codigo: Element<'a, Msg> = if let Some(gerado) = e.script_avancado.as_ref() {
            // shortcut: o text_editor do pelicano não tem modo só leitura (o do Iced sem on_action era); a edição aqui não vai a lugar nenhum e some ao gerar de novo, até o pelicano ganhar `read_only`.
            text_editor(gerado).idle_focus(false).font(pelicano::Font::MONO).size(13.0).into()
        } else {
            // Como no Iced, só escreve depois de clicado (há outro editor por baixo).
            text_editor(editor).idle_focus(false)
                .on_change(om(Evento::EditarEditorAvancado))
                .font(pelicano::Font::MONO).size(13.0).into()
        };
        let mut corpo = column![
            row![
                feather_ui::icones::icone_colorido("code", 20.0, "#7d5cc6"),
                text(titulo).size(17).font(negrito()),
                Space::new(Length::FILL, 0.0),
                ide::botao_icone(tr!("Fechar editor avançado sem aplicar"), "x",
                    Some(om(Evento::FecharEditorAvancado)), false),
            ].spacing(9).align(pelicano::Align::Center),
            row![
                ide::botao_aba(tr!("Etapas JavaScript"), "code",
                    Some(om(Evento::VoltarEditorAvancado)), !script),
                ide::botao_aba(tr!("Script completo"), "code",
                    Some(om(Evento::MostrarScriptAvancado)), script),
            ].spacing(4),
            text(nota).size(12).color(ide::cor_secundaria(escuro)),
            container(codigo).height(Length::FILL).width(Length::FILL)
                .padding(9).style(ide::estilo_area(escuro)),
        ].spacing(10).padding(16).width(Length::FILL).height(Length::FILL);
        if !e.erro_avancado.is_empty() {
            corpo = corpo.push(text(e.erro_avancado.clone()).size(12).color(cor_erro()));
        }
        corpo = corpo.push(row![
            text(trf!("{} etapas atuais · execução em Rust", e.passos.len()))
                .size(11).color(ide::cor_secundaria(escuro)),
            Space::new(Length::FILL, 0.0),
            ide::botao(tr!("Cancelar"), Some(om(Evento::FecharEditorAvancado)), false),
            ide::botao_primario(tr!("Aplicar etapas"), "checks",
                Some(om(Evento::AplicarEditorAvancado))),
        ].spacing(10).align(pelicano::Align::Center));
        container(corpo).width(Length::FILL).height(Length::FILL)
            .style(ide::estilo_area(escuro)).into()
    }

    /// Esquerda: somente as consultas, com nome editável no cabeçalho do painel.
    fn esquerda_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        let mut arvore = column![
            text(tr!("NOME DA CONSULTA")).size(10).color(ide::cor_secundaria(escuro)),
            entrada(&e.nome, tr!("Nome da consulta"), Evento::Nome),
            Space::new(0.0, Length::Fixed(8.0)),
            ide::grupo_arvore(tr!("Consultas salvas"), "folder-open", escuro),
        ].spacing(3).padding(7);
        for (id, nome) in e.salvas.iter().take(35) {
            let atual = e.edicao.as_deref() == Some(id.as_str());
            arvore = arvore.push(row![
                ide::item_arvore(nome, "table", 12.0, Some(om(Evento::EditarSalva(id.clone()))), atual),
                botao_so_icone("refresh", tr!("Atualizar o resultado desta consulta"),
                    Evento::AtualizarSalva(id.clone())),
                botao_so_icone("x", tr!("Desvincular consulta (os valores da aba permanecem)"),
                    Evento::PedirExclusao(id.clone())),
            ].align(pelicano::Align::Center));
        }
        if e.salvas.is_empty() {
            arvore = arvore.push(container(legenda_oq(tr!("Nenhuma consulta gravada neste arquivo.")))
                .padding([4, 12]));
        }
        if let Some(id) = &e.exclusao_pendente {
            let nome = e.salvas.iter().find(|(chave, _)| chave == id)
                .map(|(_, nome)| nome.as_str()).unwrap_or(tr!("Consulta"));
            arvore = arvore.push(container(column![
                texto_oq(trf!("Desvincular '{nome}'? Os valores da aba permanecem.")),
                row![
                    ide::botao(tr!("Cancelar"), Some(om(Evento::CancelarExclusao)), false),
                    ide::botao(tr!("Desvincular"), Some(om(Evento::ConfirmarExclusao)), true),
                ].spacing(ESPACO_PEQUENO),
            ].spacing(ESPACO_PEQUENO)).padding([4, 8]));
        }
        arvore = arvore.push(ide::grupo_arvore(tr!("Origem da consulta"), "folder-open", escuro));
        if e.fonte.cabecalho.is_empty() {
            arvore = arvore.push(container(legenda_oq(tr!("Nenhuma fonte escolhida.")))
                .padding([4, 12]));
        } else {
            arvore = arvore.push(container(column![
                texto_oq(e.origem_aba.clone()),
                legenda_oq(trf!("{} colunas • {} linhas", e.fonte.cabecalho.len(), e.fonte.linhas.len())),
            ].spacing(2)).padding([3, 12]));
        }
        arvore = arvore.push(ide::grupo_arvore(tr!("Tabelas existentes"), "table", escuro));
        let tabelas = tabelas_existentes(&self.workbook);
        if tabelas.is_empty() {
            arvore = arvore.push(container(legenda_oq(
                tr!("Nenhuma tabela encontrada. Crie uma na planilha com Ctrl+T.")
            )).padding([4, 12]));
        }
        for (indice, nome, _) in tabelas.into_iter().take(35) {
            let chave = trf!("Tabela: {nome}");
            if e.fonte.cabecalho.is_empty() {
                arvore = arvore.push(ide::item_arvore(&nome, "table", 12.0,
                    Some(om(Evento::CarregarTabela(indice, nome.clone()))), false));
            } else {
                arvore = arvore.push(ide::item_arvore(&nome, "table", 12.0,
                    Some(om(Evento::FonteTabela(indice, nome.clone()))),
                    e.fontes.contains_key(&chave)));
            }
        }
        if !e.fonte.cabecalho.is_empty() {
            arvore = arvore.push(container(legenda_oq(
                tr!("Clique em outra tabela para adicioná-la à combinação.")
            )).padding([4, 12]));
        }

        container(painel(
            &trf!("Consultas [{}]", e.salvas.len() + 1),
            "sitemap", arvore.into(), Length::FILL, escuro,
        )).width(Length::Fixed(LARGURA_ESQUERDA)).height(Length::FILL).into()
    }

    /// Tela de entrada: nenhuma aba é importada automaticamente.
    fn selecionar_fonte_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        let mut conteudo = column![
            text(tr!("Obter dados")).size(21).font(negrito()),
            texto_oq(tr!("Escolha uma fonte externa ou uma tabela já existente nesta pasta.")),
            row![
                dica(button(column![
                    feather_ui::icones::icone_colorido("folder-open", 30.0, "#d68126"),
                    text(tr!("Arquivo externo")).size(14).font(negrito()),
                    legenda_oq(tr!("CSV, TSV, JSON ou Excel")),
                ].spacing(8)).on_press(om(Evento::ImportarArquivo))
                    .padding(16).style(ide::estilo_botao_ide(false)),
                    tr!("Selecionar arquivo do computador")),
                dica(button(column![
                    feather_ui::icones::icone_colorido("link", 30.0, "#2179a7"),
                    text(tr!("Internet / API")).size(14).font(negrito()),
                    legenda_oq(tr!("API, CSV ou tabela HTML")),
                ].spacing(8)).on_press(om(Evento::MostrarWeb))
                    .padding(16).style(ide::estilo_botao_ide(e.mostrar_web)),
                    tr!("Buscar dados de uma URL")),
            ].spacing(10),
            row![
                dica(button(column![
                    feather_ui::icones::icone_colorido("table", 30.0, "#21865c"),
                    text(tr!("Tabela existente")).size(14).font(negrito()),
                    legenda_oq(tr!("Tabelas Ctrl+T desta pasta")),
                ].spacing(8)).on_press(om(Evento::MostrarTabelas))
                    .padding(16).style(ide::estilo_botao_ide(e.mostrar_tabelas)),
                    tr!("Escolher tabela estruturada, não uma aba")),
                dica(button(column![
                    feather_ui::icones::icone_colorido("edit", 30.0, "#7d5cc6"),
                    text(tr!("Inserir dados")).size(14).font(negrito()),
                    legenda_oq(tr!("Digitar ou colar CSV/TSV")),
                ].spacing(8)).on_press(om(Evento::AbaCentro(2)))
                    .padding(16).style(ide::estilo_botao_ide(e.mostrar_manual)),
                    tr!("Criar uma tabela manual")),
            ].spacing(10),
        ].spacing(16).padding(24).width(Length::FILL);
        if e.mostrar_web {
            conteudo = conteudo.push(container(column![
                text(tr!("Endereço Web / API")).size(13).font(negrito()),
                entrada(&e.url_web, "https://exemplo.com/api/dados.json", Evento::UrlWeb),
                row![
                    ide::botao_primario(tr!("Conectar e analisar"), "link",
                        Some(om(Evento::ImportarWeb))),
                    legenda_oq(tr!("JSON, CSV, TSV ou HTML com tabelas • 8 MiB • 25 s")),
                    legenda_oq(tr!("JSON aninhado: use #json=dados.itens[0] no final da URL.")),
                ].spacing(10).align(pelicano::Align::Center),
            ].spacing(9).padding(16)).width(Length::FILL)
                .style(ide::estilo_cabecalho(escuro)));
        }
        if e.mostrar_web && !e.tabelas_web.is_empty() {
            let mut escolher = column![
                row![
                    feather_ui::icones::icone_colorido("table", 20.0, "#21865c"),
                    text(trf!("{} tabelas encontradas na página", e.tabelas_web.len()))
                        .size(14).font(negrito()),
                ].spacing(8).align(pelicano::Align::Center),
                legenda_oq(tr!("Selecione a tabela antes de importar; a fonte atual não será alterada.")),
            ].spacing(8);
            let mut opcoes = column(Vec::<Element<Msg>>::new()).spacing(3);
            for (indice, tabela) in e.tabelas_web.iter().enumerate() {
                opcoes = opcoes.push(ide::item_arvore(
                    &trf!("Tabela {}  •  {} linhas  •  {} colunas",
                        indice + 1, tabela.linhas.len(), tabela.cabecalho.len()),
                    "table", 0.0,
                    Some(om(Evento::SelecionarTabelaWeb(indice))),
                    indice == e.indice_tabela_web,
                ));
            }
            escolher = escolher.push(scrollable(opcoes).height(Length::Fixed(120.0)));
            if let Some(tabela) = e.tabelas_web.get(e.indice_tabela_web) {
                escolher = escolher.push(text(trf!(
                    "Pré-visualização da tabela {} (primeiras 8 linhas e 6 colunas)",
                    e.indice_tabela_web + 1,
                )).size(12).font(negrito()));
                let mut cabecalho = row(Vec::<Element<Msg>>::new()).spacing(0);
                for titulo in tabela.cabecalho.iter().take(6) {
                    cabecalho = cabecalho.push(container(
                        text(titulo.clone()).size(11).font(negrito())
                    ).width(Length::Fill(1)).padding([5, 7])
                        .style(ide::estilo_cabecalho(escuro)));
                }
                let mut grade = column![cabecalho].spacing(0);
                for linha in tabela.linhas.iter().take(8) {
                    let mut celulas = row(Vec::<Element<Msg>>::new()).spacing(0);
                    for coluna in 0..tabela.cabecalho.len().min(6) {
                        let valor = linha.get(coluna).unwrap_or(&Value::Empty);
                        celulas = celulas.push(container(text(
                            feathersheet_core::celulas::values::to_text(valor)
                        ).size(11)).width(Length::Fill(1)).padding([5, 7])
                            .style(ide::estilo_celula_propriedade(escuro, false)));
                    }
                    grade = grade.push(celulas);
                }
                escolher = escolher.push(scrollable(grade)
                    .height(Length::Fixed(192.0)).width(Length::FILL));
            }
            let possui_linhas = e.tabelas_web.get(e.indice_tabela_web)
                .is_some_and(|tabela| !tabela.linhas.is_empty());
            if !possui_linhas {
                escolher = escolher.push(legenda_oq(
                    tr!("A tabela selecionada não possui registros para importar.")
                ));
            }
            escolher = escolher.push(row![
                ide::botao(tr!("Cancelar seleção"), Some(om(Evento::CancelarSelecaoWeb)), false),
                ide::botao_primario(tr!("Importar tabela selecionada"), "checks",
                    possui_linhas.then(|| om(Evento::ConfirmarTabelaWeb))),
            ].spacing(10).align(pelicano::Align::Center));
            conteudo = conteudo.push(container(escolher.spacing(9).padding(12))
                .width(Length::FILL).style(ide::estilo_area(escuro)));
        }
        if e.mostrar_tabelas || (e.fonte.cabecalho.is_empty() && !e.mostrar_web) {
            let tabelas = tabelas_existentes(&self.workbook);
            conteudo = conteudo.push(text(tr!("Tabelas existentes na pasta"))
                .size(14).font(negrito()));
            if tabelas.is_empty() {
                conteudo = conteudo.push(legenda_oq(
                    tr!("Não há tabelas estruturadas. Selecione células no FeatherSheet e crie uma tabela (Ctrl+T).")
                ));
            } else {
                for (indice, nome, area) in tabelas.into_iter().take(50) {
                    let aba = self.workbook.aba(indice)
                        .map(|a| a.sheet.name.as_str()).unwrap_or("?");
                    conteudo = conteudo.push(ide::item_arvore(
                        &trf!("{}  •  {}  ({} colunas)", nome, aba, area.c1 - area.c0 + 1),
                        "table", 0.0,
                        Some(om(Evento::CarregarTabela(indice, nome))), false,
                    ));
                }
            }
        }
        if !e.erro.is_empty() {
            conteudo = conteudo.push(text(e.erro.clone()).size(12).color(cor_erro()));
        }
        container(scrollable(conteudo).height(Length::FILL))
            .style(ide::estilo_area(escuro))
            .width(Length::FILL).height(Length::FILL).into()
    }

    /// Centro: JavaScript editável acima da prévia, sem ocultar a tabela.
    fn centro_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        if (e.fonte.cabecalho.is_empty() && !e.mostrar_manual) || e.mostrar_web || e.mostrar_tabelas {
            return self.selecionar_fonte_openquery(e, escuro);
        }
        let ativa = if e.mostrar_manual { 2 } else if e.exibir_codigo { 1 } else if e.exibir_script { 3 } else { 0 };
        let abas = container(row![
            ide::botao_aba(tr!("Prévia de dados"), "table", Some(om(Evento::AbaCentro(0))), ativa == 0),
            ide::botao_aba("JavaScript", "code", Some(om(Evento::AbaCentro(1))), ativa == 1),
            ide::botao_aba(tr!("Tabela manual"), "forms", Some(om(Evento::AbaCentro(2))), ativa == 2),
            ide::botao_aba(tr!("Script gerado"), "code", Some(om(Evento::AbaCentro(3))), ativa == 3),
        ].spacing(3)).width(Length::FILL).padding([4, 6])
            .style(ide::estilo_cabecalho(escuro));
        let conteudo: Element<'_, Msg> = match ativa {
            2 => container(column![
                texto_oq(tr!("Cole CSV ou TSV: cabeçalho na primeira linha; textos são dados, não fórmulas")),
                text_editor(&e.entrada_manual).idle_focus(false)
                    .on_change(om(Evento::EditarManual))
                    .font(pelicano::Font::MONO).size(TAM_TEXTO),
                row![
                    ide::botao(tr!("Cancelar entrada"), Some(om(Evento::AbaCentro(0))), false),
                    ide::botao_primario(tr!("Usar tabela manual"), "checks", Some(om(Evento::AplicarManual))),
                ].spacing(ESPACO_MEDIO),
            ].spacing(ESPACO_MEDIO).padding(8))
                .width(Length::FILL).height(Length::FILL)
                .style(ide::estilo_area(escuro)).into(),
            1 => column![
                container(column![
                    row![
                        feather_ui::icones::icone_colorido("code", 16.0, "#d68126"),
                        texto_oq(tr!("JavaScript declarativo • const etapas = [ ... ];")),
                        Space::new(Length::FILL, 0.0),
                        ide::botao(tr!("Editor avançado"), Some(om(Evento::AbrirEditorAvancado)), false),
                        ide::botao_primario(tr!("Aplicar"), "checks", Some(om(Evento::Atualizar))),
                    ].spacing(8).align(pelicano::Align::Center),
                    container(text_editor(&e.codigo).idle_focus(false).on_change(om(Evento::Codigo))
                        .font(pelicano::Font::MONO).size(TAM_TEXTO))
                        .height(Length::Fixed(100.0)),
                ].spacing(5).padding(8)).width(Length::FILL)
                    .style(ide::estilo_area(escuro)),
                self.previa_openquery(e, escuro),
            ].spacing(6).width(Length::FILL).height(Length::FILL).into(),
            3 => {
                let gerado = script::gerar(&e.nome, &e.fonte.cabecalho, &e.passos)
                    .unwrap_or_else(|erro| trf!("Não foi possível gerar o script: {erro}"));
                container(column![
                    texto_oq(tr!("JavaScript gerado para referência • somente leitura • execução real em Rust")),
                    scrollable(text(gerado).font(pelicano::Font::MONO).size(TAM_TEXTO))
                        .width(Length::FILL).height(Length::FILL),
                ].spacing(ESPACO_MEDIO).padding(8))
                    .width(Length::FILL).height(Length::FILL)
                    .style(ide::estilo_area(escuro)).into()
            }
            _ => self.previa_openquery(e, escuro),
        };
        let mut centro = column![abas].spacing(5).width(Length::FILL).height(Length::FILL);
        if (ativa == 0 || ativa == 1) && e.origem_aba.starts_with("Arquivo: ") {
            centro = centro.push(row![
                legenda_oq(tr!("Origem externa • A atualização sempre exige escolher o arquivo novamente.")),
                ide::botao_icone(tr!("Reimportar arquivo e manter etapas"), "refresh",
                    Some(om(Evento::ReimportarArquivo)), false),
            ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center));
        }
        centro.push(conteudo).into()
    }

    /// Barra de expressão e prévia tabular com números de linhas, tipos e seleção.
    fn previa_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        let visual = e.tabela_visual();
        let descricao = match e.passo_previa.filter(|i| *i < e.passos.len()) {
            Some(i) => trf!("Etapa {}: {}", i + 1, passo_rotulo(&e.passos[i])),
            None => trf!("Fonte: {}", e.origem_aba),
        };
        let barra_fx = container(row![
            text("ƒx").size(16).font(negrito()).color(ide::cor_acento()),
            text(descricao).size(TAM_TEXTO).font(pelicano::Font::MONO)
                .color(ide::cor_texto(escuro)),
        ].spacing(ESPACO_GRANDE).align(pelicano::Align::Center))
            .width(Length::FILL).padding([8, 12])
            .style(ide::estilo_cabecalho(escuro));

        let mut cabecalho = row![
            container(text("#").size(11).color(ide::cor_secundaria(escuro)))
                .width(Length::Fixed(38.0)).padding([8, 10])
                .style(ide::estilo_cabecalho(escuro))
        ].spacing(0);
        for (c, nome) in visual.cabecalho.iter().take(PREVIA_COLUNAS).enumerate() {
            let estat = qualidade::analisar(visual, c);
            let cor = match estat.tipo {
                qualidade::Tipo::Inteiro | qualidade::Tipo::Decimal =>
                    pelicano::Color::rgb8(34, 138, 91),
                qualidade::Tipo::Logico =>
                    pelicano::Color::rgb8(121, 92, 198),
                qualidade::Tipo::Data =>
                    pelicano::Color::rgb8(178, 111, 34),
                qualidade::Tipo::Texto =>
                    pelicano::Color::rgb8(33, 121, 167),
            };
            cabecalho = cabecalho.push(button(column![
                row![
                    text(estat.codigo()).size(10).font(negrito()).color(cor),
                    text(nome.clone()).size(TAM_TEXTO).font(negrito()),
                    Space::new(Length::FILL, 0.0),
                    feather_ui::icones::icone("chevron-down", 12.0),
                ].spacing(6).align(pelicano::Align::Center),
                text(estat.resumo()).size(9).color(ide::cor_secundaria(escuro)),
            ].spacing(2))
                .width(Length::Fill(1)).padding([8, 7])
                .style(ide::estilo_botao_ide(e.coluna == c))
                .on_press(om(Evento::Coluna(c))));
        }
        let mut grade = column![cabecalho].spacing(0);
        for (indice, linha) in visual.linhas.iter().take(PREVIA_LINHAS).enumerate() {
            let mut celulas = row![
                container(text((indice + 1).to_string()).size(TAM_LEGENDA)
                    .color(ide::cor_secundaria(escuro)))
                    .width(Length::Fixed(38.0)).padding([5, 10])
                    .style(ide::estilo_celula_propriedade(escuro, true))
            ].spacing(0);
            for val in linha.iter().take(PREVIA_COLUNAS) {
                celulas = celulas.push(container(text(feathersheet_core::celulas::values::to_text(val)).size(TAM_LEGENDA))
                    .padding([5, 8]).width(Length::Fill(1))
                    .style(ide::estilo_celula_propriedade(escuro, false)));
            }
            grade = grade.push(celulas);
        }
        column![
            barra_fx,
            container(scrollable(grade).height(Length::FILL))
                .width(Length::FILL).height(Length::FILL).padding(4)
                .style(ide::estilo_area(escuro)),
        ].spacing(5).width(Length::FILL).height(Length::FILL).into()
    }

    /// Direita: "Etapas aplicadas", com selecionar, mover e remover.
    fn etapas_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        let mut lista = column(Vec::<Element<Msg>>::new()).spacing(1).padding(4);
        if !e.fonte.cabecalho.is_empty() {
            lista = lista.push(ide::item_arvore(tr!("Fonte"), "table", 0.0,
                Some(om(Evento::Prever(None))), e.passo_previa.is_none()));
        }
        for (i, passo) in e.passos.iter().enumerate() {
            lista = lista.push(row![
                ide::item_arvore(&format!("{}. {}", i + 1, passo_rotulo(passo)),
                    "list-details", 0.0, Some(om(Evento::Prever(Some(i)))),
                    e.passo_previa == Some(i)),
                botao_seta_cima(tr!("Mover para cima"), Evento::Mover(i, -1)),
                botao_so_icone("arrow-down", tr!("Mover para baixo"), Evento::Mover(i, 1)),
                botao_so_icone("x", tr!("Remover esta etapa"), Evento::Remover(i)),
            ].align(pelicano::Align::Center));
        }
        if e.passos.is_empty() {
            lista = lista.push(container(legenda_oq(
                if e.fonte.cabecalho.is_empty() {
                    tr!("Selecione uma fonte de dados para iniciar.")
                } else {
                    tr!("Nenhuma transformação aplicada. Use a faixa de opções.")
                }
            )).padding([6, 8]));
        }
        container(painel(tr!("Etapas aplicadas"), "list", lista.into(), Length::FILL, escuro))
            .width(Length::Fixed(LARGURA_ETAPAS)).height(Length::FILL).into()
    }

    /// Painel inferior com abas: erros, colunas e log.
    fn painel_inferior_openquery<'a>(
        &'a self, e: &'a Estado, visual: &'a Tabela, escuro: bool,
    ) -> Element<'a, Msg> {
        let erros = if e.erro.is_empty() { 0 } else { 1 };
        let rotulos = [
            (trf!("Erros ({erros})"), "bug"),
            (trf!("Colunas ({})", visual.cabecalho.len()), "columns"),
            (tr!("Log").to_string(), "terminal"),
        ];
        let mut abas = row(Vec::<Element<Msg>>::new()).spacing(4);
        for (i, (rotulo, nome_icone)) in rotulos.iter().enumerate() {
            abas = abas.push(ide::botao_icone(rotulo, nome_icone,
                Some(om(Evento::PainelInferior(i as u8))), e.painel_inferior == i as u8));
        }
        let corpo: Element<'_, Msg> = match e.painel_inferior {
            0 => if e.erro.is_empty() {
                column![text(tr!("Nenhum erro nas etapas aplicadas.")).size(12)
                    .color(ide::cor_secundaria(escuro))].padding(12).into()
            } else {
                scrollable(column![text(e.erro.clone()).size(12).color(cor_erro())]
                    .padding(12).width(Length::FILL)).height(Length::FILL).into()
            },
            1 => {
                let mut colunas = column(Vec::<Element<Msg>>::new()).spacing(1).padding(4);
                for (c, nome) in visual.cabecalho.iter().enumerate() {
                    colunas = colunas.push(ide::item_arvore(
                        &format!("{}. {}", c + 1, nome), "columns", 0.0,
                        Some(om(Evento::Coluna(c))), e.coluna == c));
                }
                scrollable(colunas.width(Length::FILL)).height(Length::FILL).into()
            }
            _ => scrollable(column![
                text(trf!("Última mensagem: {}", self.status)).font(pelicano::Font::MONO).size(12),
                text(trf!("Etapas aplicadas: {}", e.passos.len()))
                    .font(pelicano::Font::MONO).size(12),
                text(trf!("Fontes adicionais: {}", e.fontes.len()))
                    .font(pelicano::Font::MONO).size(12),
                text(trf!("Limites: fonte até {MAX_CELULAS_FONTE} células, saída até {MAX_CELULAS_SAIDA}."))
                    .font(pelicano::Font::MONO).size(12).color(ide::cor_secundaria(escuro)),
            ].spacing(4).padding(12).width(Length::FILL)).height(Length::FILL).into(),
        };
        container(column![
            container(abas).padding([5, 8]).width(Length::FILL)
                .style(ide::estilo_cabecalho(escuro)),
            corpo,
        ].spacing(4).padding(4)).height(Length::Fixed(ALTURA_PAINEL_INFERIOR)).width(Length::FILL)
            .style(ide::estilo_area(escuro)).into()
    }

    /// Status compacto: acesso aos painéis sem deixar um console vazio ocupando a tela.
    fn status_openquery<'a>(&'a self, e: &'a Estado, visual: &'a Tabela, escuro: bool) -> Element<'a, Msg> {
        let secundaria = ide::cor_secundaria(escuro);
        let erros = if e.erro.is_empty() { 0 } else { 1 };
        container(row![
            text(trf!("{} colunas", visual.cabecalho.len())).size(11),
            text(trf!("{} linhas", visual.linhas.len())).size(11),
            text(trf!("Fonte: {}", e.origem_aba)).size(10).color(secundaria),
            Space::new(Length::FILL, 0.0),
            ide::botao_icone(&trf!("Erros ({erros})"), "bug",
                Some(om(Evento::PainelInferior(0))), e.painel_inferior_aberto && e.painel_inferior == 0),
            ide::botao_icone(tr!("Colunas"), "columns",
                Some(om(Evento::PainelInferior(1))), e.painel_inferior_aberto && e.painel_inferior == 1),
            ide::botao_icone(tr!("Log"), "terminal",
                Some(om(Evento::PainelInferior(2))), e.painel_inferior_aberto && e.painel_inferior == 2),
            text(if e.erro.is_empty() { tr!("Nenhum erro") } else { tr!("Verificar erros") })
                .size(11).color(if e.erro.is_empty() {
                    pelicano::Color::rgb8(34, 138, 91)
                } else { cor_erro() }),
        ].spacing(10).align(pelicano::Align::Center))
            .padding([3, 10]).width(Length::FILL)
            .style(ide::estilo_cabecalho(escuro)).into()
    }
}

/// Barra de menus: as abas da antiga faixa viram menus da janela.
fn barra_menus<'a>(e: &Estado, escuro: bool) -> Element<'a, Msg> {
    let mut linha = row(Vec::<Element<Msg>>::new()).spacing(3).align(pelicano::Align::Center);
    for (rotulo, aba) in [
        (tr!("Página Inicial"), AbaFaixa::Inicio),
        (tr!("Transformar"), AbaFaixa::Transformar),
        (tr!("Adicionar Coluna"), AbaFaixa::Adicionar),
        (tr!("Exibir"), AbaFaixa::Exibir),
    ] {
        linha = linha.push(ide::botao_menu(rotulo, Some(om(Evento::Aba(aba))), e.aba_faixa == aba));
    }
    linha = linha.push(Space::new(Length::FILL, 0.0));
    linha = linha.push(text("OPENQUERY").size(10).color(ide::cor_secundaria(escuro)));
    container(linha).padding([4, 8]).style(ide::estilo_area(escuro)).width(Length::FILL).into()
}

/// Botões verticais com SVGs cromáticos nativos, sem glifos Unicode de fallback.
fn botao_ribbon(
    rotulo: &'static str, icone: &'static str, cor: &'static str,
    evento: Evento, ativo: bool,
) -> Element<'static, Msg> {
    let conteudo = column![
        feather_ui::icones::icone_colorido(icone, 24.0, cor),
        text(rotulo).size(11).align_x(pelicano::Align::Center),
    ].align(pelicano::Align::Center).spacing(7);
    // Largura pelo rótulo: com 76 px fixos a faixa passava de 1000 px de janela.
    button(conteudo).padding([9, 8]).height(Length::Fixed(80.0))
        .on_press(om(evento)).style(ide::estilo_botao_ide(ativo)).into()
}

fn grupo_ribbon<'a>(
    rotulo: &'static str, acoes: Vec<Element<'a, Msg>>, escuro: bool,
) -> Element<'a, Msg> {
    // Sem `.width(FILL)` no rótulo: no pelicano ele esticava o grupo até a largura da janela.
    let conteudo = column![
        row(acoes).spacing(0).align(pelicano::Align::Center),
        text(rotulo).size(10).color(ide::cor_secundaria(escuro)),
    ].spacing(5).align(pelicano::Align::Center);
    container(conteudo).padding([1, 4]).into()
}

/// Uma única faixa contextual substitui as duas barras antigas.
fn barra_faixa<'a>(e: &Estado, escuro: bool) -> Element<'a, Msg> {
    const VERDE: &str = "#21865c";
    const AZUL: &str = "#2179a7";
    const LARANJA: &str = "#d68126";
    const ROXO: &str = "#7d5cc6";
    const VERMELHO: &str = "#ca5555";
    let grupos = match e.aba_faixa {
        AbaFaixa::Inicio => vec![
            grupo_ribbon("FECHAR", vec![
                botao_ribbon(tr!("Salvar e\ncarregar"), "device-floppy", VERDE, Evento::Carregar, false),
                botao_ribbon(tr!("Fechar"), "x", VERMELHO, Evento::Fechar, false),
                botao_ribbon("Carregar\nem...", "file-export", AZUL, Evento::AbrirDestino, e.parametros_abertos && e.acao_parametros.is_none()),
            ], escuro),
            grupo_ribbon(tr!("OBTER DADOS"), vec![
                botao_ribbon(tr!("De arquivo"), "folder-open", LARANJA, Evento::ImportarArquivo, false),
                botao_ribbon(tr!("Da Web"), "link", AZUL, Evento::MostrarWeb, e.mostrar_web),
                botao_ribbon(tr!("De tabela"), "table", VERDE, Evento::MostrarTabelas, e.mostrar_tabelas),
            ], escuro),
            grupo_ribbon("CONSULTA", vec![
                botao_ribbon("Atualizar\nfonte", "refresh", VERDE, Evento::AtualizarFonte, false),
                botao_ribbon(tr!("Editor\navançado"), "code", ROXO, Evento::AbrirEditorAvancado, e.editor_avancado.is_some()),
            ], escuro),
            grupo_ribbon("COLUNAS", vec![
                botao_ribbon(tr!("Renomear"), "edit", AZUL, Evento::Adicionar("renomear"), false),
                botao_ribbon(tr!("Remover"), "trash", VERMELHO, Evento::Adicionar("remover_coluna"), false),
            ], escuro),
            grupo_ribbon("LINHAS", vec![
                botao_ribbon(tr!("Filtrar"), "filter", ROXO, Evento::Adicionar("filtrar"), false),
                botao_ribbon(tr!("Manter N"), "list-numbers", VERDE, Evento::Adicionar("limite"), false),
                botao_ribbon(tr!("Distintos"), "list-select", AZUL, Evento::Adicionar("distintas"), false),
            ], escuro),
            grupo_ribbon("COMBINAR", vec![
                botao_ribbon(tr!("Mesclar"), "sitemap", AZUL,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Juntar)), false),
                botao_ribbon(tr!("Acrescentar"), "list", VERDE,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Acrescentar)), false),
            ], escuro),
        ],
        AbaFaixa::Transformar => vec![
            grupo_ribbon(tr!("TIPO DE DADOS"), vec![
                botao_ribbon(tr!("Texto"), "text-size", AZUL, Evento::Adicionar("texto"), false),
                botao_ribbon(tr!("Número"), "math-function", VERDE, Evento::Adicionar("numero"), false),
            ], escuro),
            grupo_ribbon("LINHAS", vec![
                botao_ribbon(tr!("Filtrar"), "filter", ROXO, Evento::Adicionar("filtrar"), false),
                botao_ribbon("Remover\nerros", "eraser", VERMELHO, Evento::Adicionar("remover_erros"), false),
                botao_ribbon(tr!("Agrupar"), "chart-bar", LARANJA, Evento::Adicionar("agrupar"), false),
            ], escuro),
            grupo_ribbon("ESTRUTURA", vec![
                botao_ribbon(tr!("Dividir\nem linhas"), "columns", AZUL, Evento::Adicionar("dividir"), false),
                botao_ribbon(tr!("Pivotar"), "pivot-table", ROXO,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Pivotar)), false),
                botao_ribbon(tr!("Despivotar"), "layout-grid", AZUL, Evento::Adicionar("despivotar"), false),
            ], escuro),
            grupo_ribbon(tr!("CABEÇALHO"), vec![
                botao_ribbon(tr!("Promover"), "table", VERDE, Evento::Adicionar("promover"), false),
                botao_ribbon(tr!("Rebaixar"), "list", LARANJA, Evento::Adicionar("rebaixar"), false),
            ], escuro),
            grupo_ribbon("ORDENAR", vec![
                botao_ribbon(tr!("A → Z"), "sort-ascending", AZUL, Evento::Adicionar("ordenar"), false),
                botao_ribbon(tr!("Z → A"), "sort-descending", AZUL, Evento::Adicionar("decrescente"), false),
            ], escuro),
        ],
        AbaFaixa::Adicionar => vec![
            grupo_ribbon(tr!("NOVA COLUNA"), vec![
                botao_ribbon(tr!("Índice"), "list-numbers", AZUL, Evento::Adicionar("indice"), false),
                botao_ribbon("Mesclar\ncolunas", "link", VERDE, Evento::Adicionar("mesclar"), false),
            ], escuro),
            grupo_ribbon("CONSULTAS", vec![
                botao_ribbon(tr!("Juntar"), "arrow-step", ROXO, Evento::Adicionar("juntar"), false),
                botao_ribbon(tr!("Acrescentar"), "list", AZUL,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Acrescentar)), false),
            ], escuro),
        ],
        AbaFaixa::Exibir => vec![
            grupo_ribbon(tr!("PAINÉIS"), vec![
                botao_ribbon(tr!("Consultas"), "layout-sidebar", AZUL,
                    Evento::AlternarConsultas, !e.consultas_recolhidas),
                botao_ribbon(tr!("Etapas"), "list-details", VERDE,
                    Evento::AlternarEtapas, !e.etapas_recolhidas),
                botao_ribbon(tr!("Erros e\nlog"), "terminal", ROXO,
                    Evento::AlternarPainelInferior, e.painel_inferior_aberto),
            ], escuro),
            grupo_ribbon(tr!("PRÉVIA"), vec![
                botao_ribbon("Fonte\ninicial", "table", VERDE,
                    Evento::Prever(None), e.passo_previa.is_none()),
                botao_ribbon(tr!("Ver dados"), "table", AZUL, Evento::AbaCentro(0), !e.exibir_codigo && !e.mostrar_manual),
                botao_ribbon("JavaScript", "code", ROXO, Evento::AbaCentro(1), e.exibir_codigo),
            ], escuro),
        ],
    };
    // Iced 0.13 não calcula corretamente o tamanho intrínseco desta
    // scrollable horizontal aninhada: em Linux ela se torna uma faixa vazia.
    // Sem scroll: todos os grupos principais cabem numa janela comum.
    let linha = row(grupos).spacing(2).align(pelicano::Align::Start);
    container(linha)
        .height(Length::Fixed(110.0))
        .padding([3, 5]).width(Length::FILL)
        .style(ide::estilo_area(escuro)).into()
}

/// Campos anteriormente fixos em "Propriedades" surgem apenas quando necessários.
fn barra_parametros<'a>(e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
    let mut faixa = row(Vec::<Element<Msg>>::new()).spacing(9).align(pelicano::Align::Center);
    let (titulo, campo): (&str, Option<Element<'a, Msg>>) = match e.acao_parametros {
        Some("filtrar") => (tr!("Filtrar coluna:"), Some(entrada(&e.filtro, tr!("Ex.: >100 ou SP*"), Evento::Filtro))),
        Some("renomear") => (tr!("Novo nome:"), Some(entrada(&e.renomear, tr!("Nome da coluna"), Evento::Renomear))),
        Some("limite") => (tr!("Manter primeiras linhas:"), Some(entrada(&e.quantidade, "100", Evento::Quantidade))),
        Some("dividir") => (tr!("Separador:"), Some(entrada(&e.separador, ";", Evento::Separador))),
        _ => (tr!("Carregar resultado em:"), None),
    };
    faixa = faixa.push(text(titulo).size(12).font(negrito()));
    if let Some(campo) = campo {
        faixa = faixa.push(container(campo).width(Length::Fixed(260.0)));
        faixa = faixa.push(ide::botao_primario(tr!("Aplicar"), "checks", Some(om(Evento::AplicarParametros))));
    } else {
        for (rotulo, modo) in [
            (tr!("Nova aba"), DestinoModo::NovaAba),
            (tr!("Aba atual"), DestinoModo::AbaAtual),
            (tr!("Apenas conexão"), DestinoModo::ApenasConexao),
        ] {
            faixa = faixa.push(ide::botao(rotulo, Some(om(Evento::Destino(modo))), e.destino_modo == modo));
        }
    }
    faixa = faixa.push(Space::new(Length::FILL, 0.0))
        .push(ide::botao_icone_simples("x", Some(om(Evento::AlternarParametros))));
    container(faixa).padding([7, 12]).width(Length::FILL)
        .style(ide::estilo_cabecalho(escuro)).into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn painel_aceita_corpo_fill_sem_panic() {
        let corpo: Element<'_, Msg> = pelicano::column![
            text("Colunas")
        ].height(Length::FILL).into();
        let _: Element<'_, Msg> = painel(
            "Consultas", "table", corpo, Length::FILL, false,
        );
    }

    #[test]
    fn janela_propria_abre_fecha_e_monta_todas_as_areas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "MWh".into());
        app.workbook.escrever_celula(0, 1, 0, "42".into());
        let _ = app.evento_openquery_janela(Evento::Abrir);
        assert!(app.janelas.aberta(Auxiliar::OpenQuery));
        for escuro in [false, true] {
            app.config.escuro = escuro;
            for aba in [AbaFaixa::Inicio, AbaFaixa::Transformar, AbaFaixa::Adicionar, AbaFaixa::Exibir] {
                let _ = app.evento_openquery_janela(Evento::Aba(aba));
                for centro in 0..3 {
                    let _ = app.evento_openquery_janela(Evento::AbaCentro(centro));
                    for inferior in 0..3 {
                        let _ = app.evento_openquery_janela(Evento::PainelInferior(inferior));
                        let _ = app.view_openquery();
                    }
                }
            }
        }
        let _ = app.evento_openquery_janela(Evento::Adicionar("distintas"));
        let _ = app.evento_openquery_janela(Evento::Assistente(
            assistentes::Evento::Abrir(assistentes::Tipo::Pivotar)));
        let _ = app.view_openquery();
        assert!(app.fechar_assistente_openquery());
        let _ = app.evento_openquery_janela(Evento::AlternarConsultas);
        let _ = app.evento_openquery_janela(Evento::AlternarEtapas);
        let _ = app.view_openquery();
        let _ = app.evento_openquery_janela(Evento::Fechar);
        assert!(app.openquery.is_none());
        assert!(!app.janelas.aberta(Auxiliar::OpenQuery));
    }
}
