//! Janela própria do FeatherQuery, no mesmo desenho do editor de macros (Alt+F11):
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
const ALTURA_LINHA_PREVIA: f32 = 26.0;
const ALTURA_CABECALHO_PREVIA: f32 = 48.0;

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
            e.erro_avancado.clear();
            return true;
        }
        if e.modal.take().is_some() || e.dependencias.take().is_some() || e.consultas_pq.take().is_some() { return true; }
        if e.mostrar_manual { e.mostrar_manual = false; return true; }
        if e.menu_obter { e.menu_obter = false; return true; }
        if e.parametros_abertos { e.parametros_abertos = false; e.acao_parametros = None; return true; }
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
        estrutura = estrutura.push(corpo);
        if e.painel_inferior_aberto {
            estrutura = estrutura.push(self.painel_inferior_openquery(e, visual, escuro));
        }
        estrutura = estrutura.push(self.status_openquery(e, visual, escuro));
        let base: Element<'_, Msg> = container(estrutura).padding(8)
            .style(ide::estilo_fundo_editor(escuro))
            .height(Length::FILL).width(Length::FILL).into();
        // Modais da janela: etapa (nova ou pela engrenagem) e "Carregar em…".
        let cartao: Option<Element<'_, Msg>> = if let Some(m) = &e.modal {
            Some(etapa::painel(m, &e.erro_modal))
        } else if let Some(mapa) = &e.dependencias {
            Some(painel_dependencias(mapa))
        } else if e.parametros_abertos {
            Some(painel_destino(e))
        } else if e.mostrar_manual {
            Some(painel_manual(e))
        } else if let Some(lista) = &e.consultas_pq {
            Some(painel_consultas_pq(lista))
        } else {
            None
        };
        if let Some(cartao) = cartao {
            let fundo = opaque(container(opaque(cartao))
                .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::FILL).align_y(pelicano::Align::Center)
                .style(|_| pelicano::Style {
                    background: Some(Background::Color(pelicano::Color::rgba(0.0, 0.0, 0.0, 0.40))),
                    ..pelicano::Style::default()
                }));
            return stack![base, fundo].height(Length::FILL).into();
        }
        if e.menu_obter {
            return crate::app::menu_contexto::sobrepor(
                base, &self.menu_obter_dados(e), e.submenu, |i| om(Evento::Submenu(i)),
                om(Evento::MenuObter(false)), (170.0, 132.0), e.janela,
            );
        }
        if let Some(c) = e.menu_tipo {
            return crate::app::menu_contexto::sobrepor(
                base, &menu_de_tipos(c), None, |i| om(Evento::Submenu(i)),
                om(Evento::MenuTipo(None, None)), (e.posicao_menu.0 - 10.0, e.posicao_menu.1 + 14.0), e.janela,
            );
        }
        if let Some(c) = e.menu_coluna {
            return crate::app::menu_contexto::sobrepor(
                base, &menu_da_coluna(e, c), e.submenu, |i| om(Evento::Submenu(i)),
                om(Evento::MenuColuna(None)), (e.posicao_menu.0 - 40.0, e.posicao_menu.1 + 14.0), e.janela,
            );
        }
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
            // Sempre numa pilha, com a base na 1ª posição: abrir um modal ou o menu
            // não recria a rolagem da prévia (os dados sumiam atrás do modal).
            return stack![base].height(Length::FILL).into();
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

    /// Janela ampla: código com números de linha e cores, lista de métodos que escreve a
    /// etapa no cursor, e o erro sublinhado na linha onde está.
    fn painel_editor_avancado_openquery<'a>(
        &'a self, e: &'a Estado, escuro: bool,
    ) -> Element<'a, Msg> {
        let editor = e.editor_avancado.as_ref().expect("editor avançado aberto");
        let linha_erro = script::linha_do_erro(&e.erro_avancado).filter(|_| !e.erro_avancado.is_empty());
        let avisos = linha_erro.and_then(|n| {
            let texto = editor.line(n)?;
            Some(vec![pelicano::Warning { line: n - 1, range: 0..texto.len().max(1), tip: e.erro_avancado.clone() }])
        }).unwrap_or_default();
        // Como no Iced, só escreve depois de clicado (há outro editor por baixo).
        let codigo: Element<'a, Msg> = text_editor(editor).idle_focus(false)
            .on_change(om(Evento::EditarEditorAvancado))
            .highlight("js", if escuro { pelicano::Highlight::Base16Ocean } else { pelicano::Highlight::InspiredGitHub })
            .line_numbers(true).wrap(false).warnings(avisos)
            .current_line(pelicano::Color::rgba(0.5, 0.5, 0.5, 0.08))
            .font(pelicano::Font::MONO).size(13.0).into();
        let coluna = e.resultado.cabecalho.first().map(String::as_str).unwrap_or("Coluna");
        let mut metodos = column![].spacing(1);
        for (metodo, args, linha) in script::modelos(coluna) {
            metodos = metodos.push(
                button(column![
                    text(metodo).size(12).font(pelicano::Font::MONO),
                    text(if args.is_empty() { tr!("sem argumentos").to_owned() } else { args })
                        .size(10).color(ide::cor_secundaria(escuro)),
                ].spacing(1)).width(Length::FILL).padding([4, 8])
                    .on_press(om(Evento::InserirNoEditor(linha)))
                    .style(ide::estilo_botao_ide(false)));
        }
        let lista = container(column![
            text(tr!("MÉTODOS")).size(10).color(ide::cor_secundaria(escuro)),
            text(tr!("Clique para escrever a etapa no cursor")).size(11).color(ide::cor_secundaria(escuro)),
            scrollable(metodos).height(Length::FILL),
        ].spacing(6)).width(Length::Fixed(230.0)).height(Length::FILL).padding(9)
            .style(ide::estilo_area(escuro));
        let mut corpo = column![
            row![
                feather_ui::icones::icone_colorido("code", 20.0, "#7d5cc6"),
                column![
                    text(tr!("Editor avançado")).size(17).font(negrito()),
                    text(tr!("let fonte = …; e uma linha por etapa: fonte.metodo(…);. As colunas vão pelo nome."))
                        .size(12).color(ide::cor_secundaria(escuro)),
                    text(tr!("Também aceita o M do Power Query: cole e clique Aplicar."))
                        .size(12).color(ide::cor_secundaria(escuro)),
                ].spacing(2),
                Space::new(Length::FILL, 0.0),
                ide::botao_icone(tr!("Fechar editor avançado sem aplicar"), "x",
                    Some(om(Evento::FecharEditorAvancado)), false),
            ].spacing(9).align(pelicano::Align::Center),
            row![
                container(codigo).height(Length::FILL).width(Length::FILL)
                    .padding(4).style(ide::estilo_area(escuro)),
                lista,
            ].spacing(10).height(Length::FILL),
        ].spacing(10).padding(16).width(Length::FILL).height(Length::FILL);
        if !e.erro_avancado.is_empty() {
            corpo = corpo.push(row![
                feather_ui::icones::icone("alert-triangle", 15.0),
                text(e.erro_avancado.clone()).size(12).color(cor_erro()),
            ].spacing(6).align(pelicano::Align::Center));
        } else if !e.aviso_avancado.is_empty() {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0),
                text(e.aviso_avancado.clone()).size(12).color(ide::cor_secundaria(escuro)),
            ].spacing(6).align(pelicano::Align::Center));
        }
        corpo = corpo.push(row![
            text(trf!("{} etapas atuais · Aplicar confere o script inteiro antes de mudar a consulta", e.passos.len()))
                .size(11).color(ide::cor_secundaria(escuro)),
            Space::new(Length::FILL, 0.0),
            ide::botao(tr!("Copiar como Power Query"), Some(om(Evento::CopiarPowerQuery)), false),
            ide::botao(tr!("Cancelar"), Some(om(Evento::FecharEditorAvancado)), false),
            ide::botao_primario(tr!("Aplicar"), "checks",
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
        ].spacing(3).padding(7);
        // Só as consultas: a que está aberta (ainda não carregada) e as gravadas no arquivo.
        if e.edicao.is_none() {
            arvore = arvore.push(ide::item_arvore(&e.nome, "table", 12.0, None, true));
        }
        let ao_abrir: Vec<String> = registro::listar(&self.workbook).unwrap_or_default().into_iter()
            .filter(|s| registro::atualiza_ao_abrir(&s.dados))
            .filter_map(|s| s.dados["id"].as_str().map(str::to_owned)).collect();
        for (id, nome) in e.salvas.iter() {
            let atual = e.edicao.as_deref() == Some(id.as_str());
            let ligado = ao_abrir.contains(id);
            arvore = arvore.push(row![
                ide::item_arvore(nome, "table", 12.0, Some(om(Evento::EditarSalva(id.clone()))), atual),
                dica(button(feather_ui::icones::icone("clock", 15.0)).padding([5, 7])
                    .on_press(om(Evento::AoAbrir(id.clone()))).style(ide::estilo_botao_ide(ligado)),
                    if ligado { tr!("Atualiza ao abrir o arquivo (clique para desligar)") }
                    else { tr!("Atualizar ao abrir o arquivo") }),
                botao_so_icone("pencil", tr!("Renomear consulta"), Evento::PedirRenomear(id.clone())),
                botao_so_icone("refresh", tr!("Atualizar o resultado desta consulta"),
                    Evento::AtualizarSalva(id.clone())),
                botao_so_icone("x", tr!("Desvincular consulta (os valores da aba permanecem)"),
                    Evento::PedirExclusao(id.clone())),
            ].align(pelicano::Align::Center));
        }
        if let Some((_, nome)) = &e.renomeando {
            arvore = arvore.push(container(column![
                text_input(tr!("Novo nome"), nome).on_input(|v| om(Evento::TextoRenomear(v)))
                    .on_submit(om(Evento::ConfirmarRenomear))
                    .size(11).padding([4.0, 6.0]).style(estilo_entrada_suspensa),
                row![
                    ide::botao(tr!("Cancelar"), Some(om(Evento::CancelarRenomear)), false),
                    ide::botao(tr!("Renomear"), Some(om(Evento::ConfirmarRenomear)), true),
                ].spacing(ESPACO_PEQUENO),
            ].spacing(ESPACO_PEQUENO)).padding([4, 8]));
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

        container(painel(
            &trf!("Consultas [{}]", e.salvas.len() + 1),
            "sitemap", arvore.into(), Length::FILL, escuro,
        )).width(Length::Fixed(LARGURA_ESQUERDA)).height(Length::FILL).into()
    }

    /// Tela de entrada: nenhuma aba é importada automaticamente.
    fn selecionar_fonte_openquery<'a>(&'a self, e: &'a Estado, escuro: bool) -> Element<'a, Msg> {
        // Sem fonte, o centro fica em branco: as fontes estão na faixa (OBTER DADOS).
        let mut conteudo = column(Vec::<Element<Msg>>::new()).spacing(16).padding(24).width(Length::FILL);
        let navegador = !e.itens_arquivo.is_empty();
        if e.mostrar_web && !navegador {
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
                    text(if navegador { trf!("Navegador: {} planilhas e tabelas em {}", e.tabelas_web.len(),
                        e.origem_web_pendente.trim_start_matches("Arquivo: ")) }
                        else { trf!("{} tabelas encontradas na página", e.tabelas_web.len()) })
                        .size(14).font(negrito()),
                ].spacing(8).align(pelicano::Align::Center),
                legenda_oq(tr!("Selecione a tabela antes de importar; a fonte atual não será alterada.")),
            ].spacing(8);
            let mut opcoes = column(Vec::<Element<Msg>>::new()).spacing(3);
            for (indice, tabela) in e.tabelas_web.iter().enumerate() {
                let nome = e.itens_arquivo.get(indice).map(|i| importador::nome_visivel(&i.nome)).unwrap_or_else(|| trf!("Tabela {}", indice + 1));
                opcoes = opcoes.push(ide::item_arvore(
                    &trf!("{}  •  {} linhas  •  {} colunas",
                        nome, e.itens_arquivo.get(indice).map_or(tabela.linhas.len(), |i| i.linhas), tabela.cabecalho.len()),
                    "table", 0.0,
                    Some(om(Evento::SelecionarTabelaWeb(indice))),
                    indice == e.indice_tabela_web,
                ));
            }
            escolher = escolher.push(scrollable(opcoes).height(Length::Fixed(120.0)));
            if let Some(tabela) = e.tabelas_web.get(e.indice_tabela_web) {
                escolher = escolher.push(text(trf!(
                    "Pré-visualização de {} (primeiras 8 linhas e 6 colunas)",
                    e.itens_arquivo.get(e.indice_tabela_web).map(|i| importador::nome_visivel(&i.nome)).unwrap_or_else(|| trf!("Tabela {}", e.indice_tabela_web + 1)),
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
        if e.mostrar_tabelas {
            let tabelas = tabelas_existentes(&self.workbook);
            conteudo = conteudo.push(text(tr!("Planilhas desta pasta"))
                .size(14).font(negrito()));
            let mut alguma = false;
            for i in 0..self.workbook.len() {
                let Some(aba) = self.workbook.aba(i) else { continue };
                let area = aba.sheet.used_rect();
                if aba.sheet.is_empty() { continue; }
                alguma = true;
                conteudo = conteudo.push(ide::item_arvore(
                    &trf!("{}  •  {} linhas  •  {} colunas", aba.sheet.name, area.r1 - area.r0, area.c1 - area.c0 + 1),
                    "layout-grid", 0.0, Some(om(Evento::CarregarAba(i))), false,
                ));
            }
            if !alguma {
                conteudo = conteudo.push(legenda_oq(tr!("As planilhas desta pasta estão vazias.")));
            }
            conteudo = conteudo.push(Space::new(0.0, Length::Fixed(8.0)));
            conteudo = conteudo.push(text(tr!("Tabelas (Ctrl+T)"))
                .size(14).font(negrito()));
            if tabelas.is_empty() {
                conteudo = conteudo.push(legenda_oq(
                    tr!("Não há tabelas estruturadas. Selecione células no FeatherSheet e crie uma tabela (Ctrl+T).")
                ));
            } else {
                for (indice, nome, area) in tabelas.into_iter().take(50) {
                    let aba = self.workbook.aba(indice)
                        .map(|a| a.sheet.name.as_str()).unwrap_or("?");
                    // Com uma fonte aberta, a tabela entra na combinação em vez de trocá-la.
                    let chave = trf!("Tabela: {nome}");
                    let (evento, marcada) = if e.fonte.cabecalho.is_empty() {
                        (Evento::CarregarTabela(indice, nome.clone()), false)
                    } else {
                        (Evento::FonteTabela(indice, nome.clone()), e.fontes.contains_key(&chave))
                    };
                    conteudo = conteudo.push(ide::item_arvore(
                        &trf!("{}  •  {}  ({} colunas)", nome, aba, area.c1 - area.c0 + 1),
                        "table", 0.0, Some(om(evento)), marcada,
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
        if e.fonte.cabecalho.is_empty() || e.mostrar_web || e.mostrar_tabelas {
            return self.selecionar_fonte_openquery(e, escuro);
        }
        // O script mora no Editor avançado e Digitar dados é um modal: o centro é só a prévia.
        let conteudo = self.previa_openquery(e, escuro);
        let mut centro = column![].spacing(5).width(Length::FILL).height(Length::FILL);
        if let Some(a) = &e.aviso_tipo {
            let tipo = match a.para.as_str() { "data" | "dataHora" | "hora" => tr!("data"), "logico" => tr!("lógico"), _ => tr!("número") };
            let exemplos = a.exemplos.iter().map(|x| format!("\"{x}\"")).collect::<Vec<_>>().join(", ");
            centro = centro.push(container(row![
                feather_ui::icones::icone("alert-triangle", 16.0),
                texto_oq(trf!("{} linha(s) de {} não converteram para {} e ficaram com erro (ex.: {}).", a.falhas, a.nome, tipo, exemplos)),
                Space::new(Length::FILL, 0.0),
                ide::botao(tr!("Remover essas linhas"), Some(om(Evento::FalhasDeTipo(0))), false),
                ide::botao(tr!("Substituir por 0"), Some(om(Evento::FalhasDeTipo(1))), false),
                ide::botao_icone_simples("x", Some(om(Evento::FalhasDeTipo(2)))),
            ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center)).padding([5, 8]).width(Length::FILL)
                .style(|_| pelicano::Style {
                    background: Some(Background::Color(pelicano::Color::rgba(0.84, 0.51, 0.15, 0.16))),
                    border: Border { color: pelicano::Color::rgba(0.84, 0.51, 0.15, 0.55), width: 1.0, radius: [4.0; 4] },
                    ..Default::default()
                }));
        }
        if e.origem_aba.starts_with("Arquivo: ") {
            centro = centro.push(row![
                legenda_oq(tr!("Origem externa • A atualização sempre exige escolher o arquivo novamente.")),
                ide::botao_icone(tr!("Reimportar arquivo e manter etapas"), "refresh",
                    Some(om(Evento::ReimportarArquivo)), false),
            ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center));
        }
        if e.origem_aba.starts_with("Pasta: ") {
            centro = centro.push(row![
                legenda_oq(tr!("Origem externa • A pasta só é lida de novo quando você pede.")),
                ide::botao_icone(tr!("Reler a pasta e manter etapas"), "refresh",
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
                .width(Length::Fixed(38.0)).height(Length::Fixed(ALTURA_CABECALHO_PREVIA)).padding([8, 10])
                .style(ide::estilo_cabecalho(escuro))
        ].spacing(0);
        let largura = Length::Fixed(LARGURA_COLUNA_PREVIA);
        let formatos = formatos_efetivos(e, visual);
        // Só o que está à vista vira widget; o resto é espaço do mesmo tamanho,
        // para a barra de rolagem continuar certa (50 colunas × 1000 linhas travavam).
        let (ox, oy, bw, bh) = e.rolagem;
        let n_colunas = visual.cabecalho.len();
        let c0 = (((ox - 38.0).max(0.0) / LARGURA_COLUNA_PREVIA) as usize).min(n_colunas);
        let c1 = (c0 + (bw / LARGURA_COLUNA_PREVIA).ceil() as usize + 2).min(n_colunas);
        let n_linhas = visual.linhas.len().min(PREVIA_LINHAS);
        let l0 = (((oy - ALTURA_CABECALHO_PREVIA).max(0.0) / ALTURA_LINHA_PREVIA) as usize).min(n_linhas);
        let l1 = (l0 + (bh / ALTURA_LINHA_PREVIA).ceil() as usize + 2).min(n_linhas);
        let antes = Space::new(Length::Fixed(c0 as f32 * LARGURA_COLUNA_PREVIA), 0.0);
        let depois = || Space::new(Length::Fixed((n_colunas - c1) as f32 * LARGURA_COLUNA_PREVIA), 0.0);
        cabecalho = cabecalho.push(antes);
        for (c, nome) in visual.cabecalho.iter().enumerate().take(c1).skip(c0) {
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
            let selecionada = e.selecionadas.contains(&c);
            let tipo = dica(button(text(codigo_tipo(estat.codigo(), formatos.get(nome))).size(10).font(negrito()).color(cor))
                .padding([4, 5]).style(ide::estilo_botao_ide(e.menu_tipo == Some(c)))
                .on_press(om(Evento::MenuTipo(Some(c), None))), tr!("Tipo de dados"));
            let titulo = button(column![
                text(nome.clone()).size(TAM_TEXTO).font(negrito()).ellipsis(),
                text(estat.resumo()).size(9).color(ide::cor_secundaria(escuro)).ellipsis(),
            ].spacing(2))
                .width(Length::FILL).padding([8, 7])
                .style(ide::estilo_botao_ide(selecionada))
                .on_press(om(Evento::Coluna(c)));
            let seta = button(feather_ui::icones::icone("chevron-down", 14.0))
                .padding([8, 6]).height(Length::FILL)
                .style(ide::estilo_botao_ide(e.menu_coluna == Some(c)))
                .on_press(om(Evento::MenuColuna(Some(c))));
            cabecalho = cabecalho.push(container(row![container(tipo).padding([0, 2]).height(Length::FILL).align_y(pelicano::Align::Center), titulo, seta].height(Length::FILL))
                .width(largura).height(Length::Fixed(ALTURA_CABECALHO_PREVIA)));
        }
        let cabecalho = cabecalho.push(depois());
        // O menu ▾ abre onde está o mouse (o `on_move` dá a posição na janela).
        let cabecalho = pelicano::mouse_area(container(cabecalho).style(ide::estilo_cabecalho(escuro)))
            .on_move(|p| om(Evento::Ponteiro(p.x, p.y)));
        let mut grade = column![
            Space::new(0.0, Length::Fixed(ALTURA_CABECALHO_PREVIA + l0 as f32 * ALTURA_LINHA_PREVIA)),
        ].spacing(0);
        for (indice, linha) in visual.linhas.iter().enumerate().take(l1).skip(l0) {
            let mut celulas = row![
                container(text((indice + 1).to_string()).size(TAM_LEGENDA)
                    .color(ide::cor_secundaria(escuro)))
                    .width(Length::Fixed(38.0)).height(Length::Fixed(ALTURA_LINHA_PREVIA)).padding([5, 10])
                    .style(ide::estilo_celula_propriedade(escuro, true))
            ].spacing(0).push(Space::new(Length::Fixed(c0 as f32 * LARGURA_COLUNA_PREVIA), 0.0));
            for (c, val) in linha.iter().enumerate().take(c1).skip(c0) {
                // Uma linha por célula, com "…": texto longo não desalinha as linhas.
                let texto = match (val, formatos.get(&visual.cabecalho[c.min(n_colunas.saturating_sub(1))])) {
                    (Value::Number(_), Some(estilo)) => feathersheet_core::planilha::bloco::texto_formatado(val, Some(estilo)),
                    _ => feathersheet_core::celulas::values::to_text(val),
                };
                celulas = celulas.push(container(text(texto).size(TAM_LEGENDA).ellipsis())
                    .padding([5, 8]).width(largura).height(Length::Fixed(ALTURA_LINHA_PREVIA)).clip(true)
                    .style(ide::estilo_celula_propriedade(escuro, e.selecionadas.contains(&c))));
            }
            // O resto da largura (inclusive linha mais curta que o cabeçalho).
            let desenhadas = linha.len().min(c1).saturating_sub(c0);
            let resto = n_colunas.saturating_sub(c0 + desenhadas);
            grade = grade.push(celulas.push(Space::new(Length::Fixed(resto as f32 * LARGURA_COLUNA_PREVIA), 0.0)));
        }
        let grade = grade.push(Space::new(0.0, Length::Fixed((n_linhas - l1) as f32 * ALTURA_LINHA_PREVIA)));
        // Cabeçalho fixo: desenhado por cima das linhas, na altura da rolagem.
        let topo = oy.clamp(0.0, (n_linhas as f32 * ALTURA_LINHA_PREVIA).max(0.0));
        let grade = pelicano::stack![grade, column![Space::new(0.0, Length::Fixed(topo)), cabecalho]];
        column![
            barra_fx,
            container(scrollable(grade).direction(pelicano::Direction::Both)
                .on_scroll(|v| om(Evento::RolarPrevia(v))).height(Length::FILL).width(Length::FILL))
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
                if etapa::Modal::de_passo(passo, i, Vec::new()).is_some() {
                    botao_so_icone("settings", tr!("Editar os parâmetros desta etapa"), Evento::EditarPasso(i))
                } else {
                    Space::new(Length::Fixed(29.0), 0.0).into()
                },
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
    linha = linha.push(text("FEATHERQUERY").size(10).color(ide::cor_secundaria(escuro)));
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

/// Botão pequeno (ícone 16 + texto ao lado), empilhado ao lado dos grandes.
fn botao_pequeno(rotulo: &'static str, icone: &'static str, cor: &'static str, evento: Evento) -> Element<'static, Msg> {
    button(row![
        feather_ui::icones::icone_colorido(icone, 16.0, cor),
        text(rotulo).size(11),
    ].spacing(6).align(pelicano::Align::Center))
        .padding([4, 7])
        .on_press(om(evento)).style(ide::estilo_botao_ide(false)).into()
}

/// Até três botões pequenos, um embaixo do outro.
fn pilha(botoes: Vec<Element<'static, Msg>>) -> Element<'static, Msg> {
    container(column(botoes).spacing(1)).height(Length::Fixed(80.0)).align_y(pelicano::Align::Center).into()
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
    let p = botao_pequeno;
    let grupos = match e.aba_faixa {
        AbaFaixa::Inicio => vec![
            grupo_ribbon(tr!("FECHAR"), vec![
                botao_ribbon(tr!("Salvar e\ncarregar"), "device-floppy", VERDE, Evento::Carregar, false),
                pilha(vec![
                    p(tr!("Carregar em…"), "file-export", AZUL, Evento::AbrirDestino),
                    p(tr!("Fechar"), "x", VERMELHO, Evento::Fechar),
                ]),
            ], escuro),
            grupo_ribbon(tr!("OBTER DADOS"), vec![
                botao_ribbon(tr!("Obter\ndados ▾"), "database", LARANJA, Evento::MenuObter(!e.menu_obter), e.menu_obter),
                pilha(vec![
                    p(tr!("De arquivo"), "folder-open", LARANJA, Evento::ImportarArquivo),
                    p(tr!("Da Web"), "link", AZUL, Evento::MostrarWeb),
                    p(tr!("De tabela"), "table", VERDE, Evento::MostrarTabelas),
                ]),
                pilha(vec![
                    p(tr!("Da pasta…"), "folder", LARANJA, Evento::ImportarPasta),
                    p(tr!("Digitar dados"), "edit", ROXO, Evento::AbaCentro(2)),
                ]),
            ], escuro),
            grupo_ribbon(tr!("CONSULTA"), vec![
                botao_ribbon("Atualizar\nfonte", "refresh", VERDE, Evento::AtualizarFonte, false),
                pilha(vec![
                    p(tr!("Editor avançado"), "code", ROXO, Evento::AbrirEditorAvancado),
                    p(tr!("Desfazer"), "arrow-back-up", AZUL, Evento::Desfazer(true)),
                    p(tr!("Refazer"), "arrow-forward-up", AZUL, Evento::Desfazer(false)),
                ]),
                pilha(vec![
                    p(tr!("Duplicar"), "copy", VERDE, Evento::Duplicar),
                    p(tr!("Referenciar"), "link", ROXO, Evento::Referenciar),
                ]),
            ], escuro),
            grupo_ribbon(tr!("COLUNAS"), vec![pilha(vec![
                p(tr!("Renomear…"), "edit", AZUL, Evento::Adicionar("renomear")),
                p(tr!("Remover colunas"), "trash", VERMELHO, Evento::Adicionar("remover_coluna")),
                p(tr!("Remover outras colunas"), "columns", AZUL, Evento::Adicionar("manter_colunas")),
            ])], escuro),
            grupo_ribbon(tr!("LINHAS"), vec![
                botao_ribbon(tr!("Filtrar"), "filter", ROXO, Evento::Adicionar("filtrar"), false),
                pilha(vec![
                    p(tr!("Manter primeiras linhas…"), "list-numbers", VERDE, Evento::Adicionar("limite")),
                    p(tr!("Remover linhas…"), "trash", VERMELHO, Evento::Adicionar("remover_linhas")),
                    p(tr!("Remover duplicatas"), "list-select", AZUL, Evento::Adicionar("distintas")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("COMBINAR"), vec![pilha(vec![
                p(tr!("Mesclar consultas"), "sitemap", AZUL,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Juntar))),
                p(tr!("Acrescentar consultas"), "list", VERDE,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Acrescentar))),
            ])], escuro),
        ],
        AbaFaixa::Transformar => vec![
            grupo_ribbon(tr!("QUALQUER COLUNA"), vec![
                botao_ribbon(tr!("Tipo de\ndados ▾"), "math-function", VERDE,
                    Evento::MenuTipo(Some(e.coluna), Some((30.0, 150.0))), e.menu_tipo.is_some()),
                pilha(vec![
                    p(tr!("Renomear…"), "edit", AZUL, Evento::Adicionar("renomear")),
                    p(tr!("Despivotar outras"), "layout-grid", AZUL, Evento::Adicionar("despivotar")),
                    p(tr!("Pivotar…"), "pivot-table", ROXO,
                        Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Pivotar))),
                ]),
                pilha(vec![
                    p(tr!("Substituir valores…"), "pencil", LARANJA, Evento::Adicionar("substituir")),
                    p(tr!("Substituir erros…"), "eraser", VERMELHO, Evento::Adicionar("substituir_erros")),
                    p(tr!("Preencher…"), "arrow-down", VERDE, Evento::Adicionar("preencher")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("TABELA"), vec![
                botao_ribbon(tr!("Agrupar\npor"), "chart-bar", LARANJA, Evento::Adicionar("agrupar"), false),
                pilha(vec![
                    p(tr!("1ª linha como cabeçalho"), "table", VERDE, Evento::Adicionar("promover")),
                    p(tr!("Cabeçalho como 1ª linha"), "list", LARANJA, Evento::Adicionar("rebaixar")),
                    p(tr!("Transpor"), "arrows-maximize", ROXO, Evento::Adicionar("transpor")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("TEXTO E DATA"), vec![
                pilha(vec![
                    p(tr!("Dividir coluna…"), "columns", AZUL, Evento::Adicionar("dividir")),
                    p(tr!("Mesclar colunas"), "link", VERDE, Evento::Adicionar("mesclar")),
                    p(tr!("Formatar…"), "text-size", ROXO, Evento::Adicionar("formatar")),
                ]),
                pilha(vec![
                    p(tr!("Extrair texto…"), "cut", AZUL, Evento::Adicionar("extrair_texto")),
                    p(tr!("Extrair da data…"), "clock", LARANJA, Evento::Adicionar("derivar")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("MOVER"), vec![
                pilha(vec![
                    p(tr!("Esquerda"), "arrow-left", AZUL, Evento::Adicionar("mover_esquerda")),
                    p(tr!("Direita"), "arrow-right", AZUL, Evento::Adicionar("mover_direita")),
                ]),
                pilha(vec![
                    p(tr!("Início"), "chevron-left", AZUL, Evento::Adicionar("mover_inicio")),
                    p(tr!("Fim"), "chevron-right", AZUL, Evento::Adicionar("mover_fim")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("LINHAS"), vec![pilha(vec![
                p(tr!("Filtrar…"), "filter", ROXO, Evento::Adicionar("filtrar")),
                p(tr!("Remover erros"), "eraser", VERMELHO, Evento::Adicionar("remover_erros")),
                p(tr!("Inverter linhas"), "arrows-vertical", AZUL, Evento::Adicionar("inverter")),
            ])], escuro),
            grupo_ribbon(tr!("ORDENAR"), vec![pilha(vec![
                p(tr!("Crescente"), "sort-ascending", AZUL, Evento::Adicionar("ordenar")),
                p(tr!("Decrescente"), "sort-descending", AZUL, Evento::Adicionar("decrescente")),
            ])], escuro),
        ],
        AbaFaixa::Adicionar => vec![
            grupo_ribbon(tr!("NOVA COLUNA"), vec![
                botao_ribbon(tr!("Coluna\npersonalizada"), "math-function", ROXO, Evento::Adicionar("personalizada"), false),
                botao_ribbon(tr!("Índice"), "list-numbers", AZUL, Evento::Adicionar("indice"), false),
                pilha(vec![
                    p(tr!("Coluna condicional…"), "bolt", LARANJA, Evento::Adicionar("condicional")),
                    p(tr!("Mesclar colunas"), "link", VERDE, Evento::Adicionar("mesclar")),
                    p(tr!("Extrair da data…"), "clock", LARANJA, Evento::Adicionar("derivar")),
                ]),
                pilha(vec![
                    p(tr!("Extrair texto…"), "cut", AZUL, Evento::Adicionar("extrair_texto_nova")),
                    p(tr!("Duplicar coluna"), "copy", VERDE, Evento::Adicionar("duplicar")),
                ]),
            ], escuro),
            grupo_ribbon(tr!("CONSULTAS"), vec![pilha(vec![
                p(tr!("Juntar"), "arrow-step", ROXO, Evento::Adicionar("juntar")),
                p(tr!("Acrescentar consultas"), "list", AZUL,
                    Evento::Assistente(assistentes::Evento::Abrir(assistentes::Tipo::Acrescentar))),
            ])], escuro),
        ],
        AbaFaixa::Exibir => vec![
            grupo_ribbon(tr!("PAINÉIS"), vec![
                botao_ribbon(tr!("Consultas"), "layout-sidebar", AZUL,
                    Evento::AlternarConsultas, !e.consultas_recolhidas),
                botao_ribbon(tr!("Etapas"), "list-details", VERDE,
                    Evento::AlternarEtapas, !e.etapas_recolhidas),
                botao_ribbon(tr!("Erros e\nlog"), "terminal", ROXO,
                    Evento::AlternarPainelInferior, e.painel_inferior_aberto),
                botao_ribbon(tr!("Dependências"), "sitemap", LARANJA,
                    Evento::Dependencias(true), e.dependencias.is_some()),
            ], escuro),
            grupo_ribbon(tr!("PRÉVIA"), vec![
                botao_ribbon("Fonte\ninicial", "table", VERDE,
                    Evento::Prever(None), e.passo_previa.is_none()),
                botao_ribbon(tr!("Ver dados"), "table", AZUL, Evento::AbaCentro(0), !e.mostrar_manual),
                botao_ribbon(tr!("Editor\navançado"), "code", ROXO, Evento::AbrirEditorAvancado, e.editor_avancado.is_some()),
            ], escuro),
        ],
    };
    // Uma divisória entre os grupos, como na faixa da planilha.
    let mut com_divisorias: Vec<Element<'a, Msg>> = Vec::new();
    for (i, g) in grupos.into_iter().enumerate() {
        if i > 0 {
            com_divisorias.push(container(pelicano::vertical_rule(1.0)).height(Length::Fixed(92.0)).padding([4, 2]).into());
        }
        com_divisorias.push(g);
    }
    let grupos = com_divisorias;
    // Iced 0.13 não calcula corretamente o tamanho intrínseco desta
    // scrollable horizontal aninhada: em Linux ela se torna uma faixa vazia.
    // Sem scroll: todos os grupos principais cabem numa janela comum.
    let linha = row(grupos).spacing(2).align(pelicano::Align::Start);
    container(linha)
        .height(Length::Fixed(110.0))
        .padding([3, 5]).width(Length::FILL)
        .style(ide::estilo_area(escuro)).into()
}

/// Os tipos, como o menu do ícone de tipo do Power Query.
const TIPOS: [(&str, &str, &str); 7] = [
    ("Número decimal", "math-function", "decimal"),
    ("Número inteiro", "list-numbers", "inteiro"),
    ("Data", "timeline", "data"),
    ("Hora", "clock", "hora"),
    ("Data/Hora", "timeline", "dataHora"),
    ("Texto", "text-size", "texto"),
    ("Verdadeiro/Falso", "checkbox", "logico"),
];

fn menu_de_tipos(c: usize) -> Vec<crate::app::menu_contexto::ItemMenu> {
    TIPOS.iter().map(|(rotulo, icone, para)| crate::app::menu_contexto::ItemMenu::acao(tr!(rotulo), icone, om(Evento::Tipar(c, para)))).collect()
}

/// O código do tipo no cabeçalho: o da amostra, ou o do formato (data, hora).
fn codigo_tipo(amostra: &'static str, formato: Option<&feathersheet_core::celulas::style::Style>) -> &'static str {
    use feathersheet_core::celulas::style::NumberFormat as F;
    match formato.map(|f| f.format) {
        Some(F::Date) => "DATA",
        Some(F::Time) => "HORA",
        Some(F::Datetime) => "DT/H",
        _ => amostra,
    }
}

/// Formatos da fonte mais os das etapas de tipo até a etapa em prévia.
// shortcut: a etapa de tipo liga pelo índice da coluna no resultado mostrado; colunas movidas depois dela podem trocar o formato.
fn formatos_efetivos(e: &Estado, visual: &Tabela) -> Formatos {
    use feathersheet_core::celulas::style::{NumberFormat as F, Style};
    let mut f = e.formatos.clone();
    let ate = e.passo_previa.map_or(e.passos.len(), |i| (i + 1).min(e.passos.len()));
    for p in &e.passos[..ate] {
        let Passo::Tipar { col, para, .. } = p else { continue };
        let Some(nome) = visual.cabecalho.get(*col) else { continue };
        let formato = match para.as_str() {
            "data" => F::Date,
            "hora" => F::Time,
            "dataHora" => F::Datetime,
            _ => { f.remove(nome); continue; }
        };
        f.insert(nome.clone(), Style { format: formato, ..Style::default() });
    }
    f
}

/// Menu ▾ da coluna, como o do Power Query: age sobre a seleção.
fn menu_da_coluna(e: &Estado, c: usize) -> Vec<crate::app::menu_contexto::ItemMenu> {
    use crate::app::menu_contexto::ItemMenu;
    let acao = |rotulo: &str, icone: &'static str, tipo: &'static str| ItemMenu::acao(rotulo, icone, om(Evento::Adicionar(tipo)));
    let varias = e.selecionadas.len() > 1;
    let _ = c;
    vec![
        acao(tr!("Classificar crescente"), "sort-ascending", "ordenar"),
        acao(tr!("Classificar decrescente"), "sort-descending", "decrescente"),
        acao(tr!("Filtrar…"), "filter", "filtrar"),
        ItemMenu::Separador,
        acao(tr!("Renomear…"), "edit", "renomear"),
        ItemMenu::sub(tr!("Alterar tipo"), "math-function", vec![
            acao(tr!("Texto"), "text-size", "texto"),
            acao(tr!("Número"), "math-function", "numero"),
        ]),
        acao(tr!("Dividir coluna…"), "columns", "dividir"),
        acao(tr!("Extrair texto…"), "cut", "extrair_texto"),
        acao(tr!("Duplicar coluna"), "copy", "duplicar"),
        ItemMenu::sub(tr!("Mover"), "arrows-horizontal", vec![
            acao(tr!("Para a esquerda"), "arrow-left", "mover_esquerda"),
            acao(tr!("Para a direita"), "arrow-right", "mover_direita"),
            acao(tr!("Para o início"), "chevron-left", "mover_inicio"),
            acao(tr!("Para o fim"), "chevron-right", "mover_fim"),
        ]),
        acao(tr!("Agrupar por…"), "chart-bar", "agrupar"),
        ItemMenu::Separador,
        acao(if varias { tr!("Remover colunas") } else { tr!("Remover coluna") }, "trash", "remover_coluna"),
        acao(tr!("Remover outras colunas"), "columns", "manter_colunas"),
        acao(tr!("Remover duplicatas"), "list-select", "distintas"),
        acao(tr!("Remover erros"), "eraser", "remover_erros"),
    ]
}

impl App {
    /// Obter dados ▾: todas as conexões do FeatherQuery num lugar, como o "Obter Dados" do Excel.
    fn menu_obter_dados(&self, e: &Estado) -> Vec<crate::app::menu_contexto::ItemMenu> {
        use crate::app::menu_contexto::ItemMenu;
        let item = |rotulo: &str, icone: &'static str, ev: Evento| ItemMenu::acao(rotulo, icone, om(Evento::DoMenuObter(Box::new(ev))));
        let mut arquivo: Vec<ItemMenu> = importador::TIPOS.iter().enumerate().map(|(i, (nome, ext))| {
            item(&format!("{} (.{})", crate::idioma::tr(nome), ext.join(", .")), "file", Evento::ImportarTipo(i))
        }).collect();
        arquivo.push(ItemMenu::Separador);
        arquivo.push(item(tr!("Pasta (junta todos os arquivos)…"), "folder", Evento::ImportarPasta));
        let mut itens = vec![
            ItemMenu::sub(tr!("Arquivo"), "folder-open", arquivo),
            item(tr!("Da Web…"), "link", Evento::MostrarWeb),
            item(tr!("Tabela ou planilha desta pasta"), "table", Evento::MostrarTabelas),
            item(tr!("Da seleção"), "layout-grid", Evento::CarregarSelecao),
            item(tr!("Digitar dados"), "edit", Evento::AbaCentro(2)),
        ];
        if self.arquivo_xlsx_aberto().is_some() {
            itens.push(item(tr!("Consultas do Power Query deste arquivo…"), "file-import", Evento::ConsultasPowerQuery));
        }
        if !e.salvas.is_empty() {
            itens.push(ItemMenu::sub(tr!("Outra consulta (referência)"), "link",
                e.salvas.iter().map(|(id, nome)| item(nome, "table", Evento::ReferenciarSalva(id.clone()))).collect()));
        }
        if !self.config.fontes_recentes.is_empty() {
            itens.push(ItemMenu::Separador);
            itens.push(ItemMenu::sub(tr!("Fontes recentes"), "clock",
                self.config.fontes_recentes.iter().map(|f| item(f, if f.starts_with("http") { "link" } else { "file" },
                    Evento::AbrirRecente(f.clone()))).collect()));
        }
        itens
    }
}

/// Obter dados ▸ Digitar dados: o texto colado (CSV/TSV) vira a fonte, com a contagem ao vivo.
fn painel_manual<'a>(e: &'a Estado) -> Element<'a, Msg> {
    let lido = match manual::ler(&e.entrada_manual.text()) {
        Ok(t) => legenda_oq(&trf!("{} linhas × {} colunas", t.linhas.len(), t.cabecalho.len())),
        Err(erro) => text(erro).size(TAM_TEXTO).color(cor_erro()).into(),
    };
    let corpo = column![
        container(text_editor(&e.entrada_manual).idle_focus(false)
            .on_change(om(Evento::EditarManual)).line_numbers(true).wrap(false)
            .current_line(pelicano::Color::rgba(0.5, 0.5, 0.5, 0.08))
            .font(pelicano::Font::MONO).size(TAM_TEXTO))
            .height(Length::Fixed(260.0)).width(Length::FILL),
        lido,
    ].spacing(ESPACO_MEDIO);
    feather_ui::dialogo::Dialogo::new(tr!("Digitar dados"), corpo)
        .icone("edit")
        .subtitulo(tr!("Cole ou digite CSV/TSV: cabeçalho na primeira linha; textos são dados, não fórmulas."))
        .largura(620.0)
        .ao_fechar(om(Evento::AbaCentro(0)))
        .secundario(tr!("Cancelar"), Some(om(Evento::AbaCentro(0))))
        .primario(tr!("OK"), Some(om(Evento::AplicarManual)))
        .painel()
}

/// Obter dados ▸ Consultas do Power Query deste arquivo: escolher qual abrir no Editor avançado.
fn painel_consultas_pq<'a>(lista: &'a [(String, String)]) -> Element<'a, Msg> {
    let mut corpo = column![].spacing(ESPACO_PEQUENO);
    for (i, (nome, m)) in lista.iter().enumerate() {
        let etapas = m.matches(" = ").count().saturating_sub(1).max(1);
        corpo = corpo.push(button(row![
            feather_ui::icones::icone("table", 15.0),
            text(nome.clone()).size(TAM_TEXTO).width(Length::FILL),
            legenda_oq(&trf!("~{etapas} etapas")),
        ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center))
            .width(Length::FILL).padding([6, 10])
            .on_press(om(Evento::AbrirConsultaPowerQuery(i)))
            .style(ide::estilo_botao_ide(false)));
    }
    feather_ui::dialogo::Dialogo::new(tr!("Consultas do Power Query deste arquivo"), scrollable(corpo).height(Length::Fixed(320.0)))
        .icone("file-import")
        .subtitulo(tr!("O Excel guardou estas consultas no arquivo. A escolhida abre no Editor avançado, convertida em etapas do FeatherQuery."))
        .largura(560.0)
        .ao_fechar(om(Evento::FecharConsultasPowerQuery))
        .secundario(tr!("Cancelar"), Some(om(Evento::FecharConsultasPowerQuery)))
        .painel()
}

/// "Carregar em…": modal com o destino do resultado.
/// Exibir ▸ Dependências: de onde cada consulta lê e para onde escreve.
fn painel_dependencias<'a>(mapa: &'a [(String, Vec<String>, String)]) -> Element<'a, Msg> {
    let chip = |t: String, icone: &'static str| -> Element<'a, Msg> {
        container(row![feather_ui::icones::icone(icone, 14.0), text(t).size(TAM_TEXTO)]
            .spacing(ESPACO_PEQUENO).align(pelicano::Align::Center))
            .padding([4, 8]).style(|_| pelicano::Style {
                background: Some(Background::Color(pelicano::Color::rgba(0.5, 0.5, 0.5, 0.14))),
                border: Border { color: pelicano::Color::rgba(0.5, 0.5, 0.5, 0.35), width: 1.0, radius: [4.0; 4] },
                ..Default::default()
            }).into()
    };
    let mut corpo = column![].spacing(ESPACO_MEDIO);
    if mapa.is_empty() {
        corpo = corpo.push(legenda_oq(tr!("Nenhuma consulta gravada nesta pasta ainda.")));
    }
    for (nome, fontes, destino) in mapa {
        let mut origem = column![].spacing(2);
        for f in fontes {
            let icone = if f.starts_with("Consulta: ") { "sitemap" } else if f.starts_with("Web: ") { "link" }
                else if f.starts_with("Arquivo: ") || f.starts_with("Pasta: ") { "folder" } else { "table" };
            origem = origem.push(chip(f.clone(), icone));
        }
        corpo = corpo.push(row![
            container(origem).width(Length::Fill(3)),
            text("→").size(16),
            container(chip(nome.clone(), "sitemap")).width(Length::Fill(2)),
            text("→").size(16),
            container(chip(destino.clone(), "file-export")).width(Length::Fill(2)),
        ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center));
    }
    feather_ui::dialogo::Dialogo::new(tr!("Dependências das consultas"), corpo)
        .icone("sitemap")
        .subtitulo(tr!("De onde cada consulta lê e para onde escreve. Quem referencia outra consulta vem depois dela."))
        .largura(720.0)
        .ao_fechar(om(Evento::Dependencias(false)))
        .primario(tr!("Fechar"), Some(om(Evento::Dependencias(false))))
        .painel()
}

fn painel_destino<'a>(e: &'a Estado) -> Element<'a, Msg> {
    let mut opcoes = column![].spacing(ESPACO_MEDIO);
    for (rotulo, legenda, modo) in [
        (tr!("Nova aba"), tr!("Cria uma aba com o resultado."), DestinoModo::NovaAba),
        (tr!("Aba atual"), tr!("Escreve a partir de A1 da aba ativa."), DestinoModo::AbaAtual),
        (tr!("Apenas conexão"), tr!("Guarda a consulta sem escrever células."), DestinoModo::ApenasConexao),
    ] {
        opcoes = opcoes.push(column![
            pelicano::radio(rotulo, modo, Some(e.destino_modo), |m| om(Evento::Destino(m))).size(TAM_TEXTO),
            container(legenda_oq(legenda)).padding([0, 26]),
        ].spacing(2));
    }
    container(column![
        titulo_oq(tr!("Carregar em…")),
        opcoes,
        row![
            Space::new(Length::FILL, 0.0),
            button(text(tr!("Fechar")).size(TAM_TEXTO))
                .on_press(om(Evento::AlternarParametros))
                .style(estilo_botao_secundario).padding([6.0, 12.0]),
        ],
    ].spacing(ESPACO_GRANDE).padding(PADDING_PAINEL + 4.0)).width(Length::Fixed(400.0))
        .style(estilo_janela_oq).into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn painel_aceita_corpo_fill_sem_panic() {
        let corpo: Element<'_, Msg> = pelicano::column![
            text(tr!("Colunas"))
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

    fn com_fonte() -> App {
        let mut app = App::default();
        for (c, nome) in ["Uf", "Tipo", "Votos"].iter().enumerate() {
            app.workbook.escrever_celula(0, 0, c as u32, (*nome).into());
        }
        for (l, (uf, tipo, votos)) in [("RR", "Eleição Ordinária", "10"), ("AC", "Eleição Suplementar", "7")].iter().enumerate() {
            app.workbook.escrever_celula(0, l as u32 + 1, 0, (*uf).into());
            app.workbook.escrever_celula(0, l as u32 + 1, 1, (*tipo).into());
            app.workbook.escrever_celula(0, l as u32 + 1, 2, (*votos).into());
        }
        abrir_aba_para_teste(&mut app);
        app
    }

    #[test]
    fn modal_cria_etapa_e_engrenagem_edita_a_mesma() {
        let mut app = com_fonte();
        let _ = app.evento_openquery_janela(Evento::Coluna(1));
        let _ = app.evento_openquery_janela(Evento::Adicionar("dividir"));
        assert!(app.openquery.as_ref().unwrap().modal.is_some());
        let _ = app.view_openquery();
        let _ = app.evento_openquery_janela(Evento::Etapa(etapa::Evento::Aplicar));
        let e = app.openquery.as_ref().unwrap();
        assert!(e.modal.is_none());
        assert_eq!(e.resultado.cabecalho, ["Uf", "Tipo.1", "Tipo.2", "Votos"]);
        // Engrenagem: a mesma etapa vira "em linhas", sem acrescentar outra.
        let _ = app.evento_openquery_janela(Evento::EditarPasso(0));
        let _ = app.evento_openquery_janela(Evento::Etapa(etapa::Evento::EmLinhas(true)));
        let _ = app.evento_openquery_janela(Evento::Etapa(etapa::Evento::Aplicar));
        let e = app.openquery.as_ref().unwrap();
        assert_eq!(e.passos.len(), 1);
        assert_eq!(e.resultado.linhas.len(), 4);
    }

    #[test]
    fn ctrl_e_shift_selecionam_varias_colunas_e_o_menu_remove_todas() {
        let mut app = com_fonte();
        let _ = app.evento_openquery_janela(Evento::Coluna(0));
        let _ = app.evento_openquery_janela(Evento::Modificadores(pelicano::Modifiers::SHIFT));
        let _ = app.evento_openquery_janela(Evento::Coluna(1));
        assert_eq!(app.openquery.as_ref().unwrap().selecionadas, [0, 1]);
        let _ = app.evento_openquery_janela(Evento::Modificadores(pelicano::Modifiers::default()));
        let _ = app.evento_openquery_janela(Evento::MenuColuna(Some(1)));
        let _ = app.view_openquery();
        let _ = app.evento_openquery_janela(Evento::Adicionar("remover_coluna"));
        let e = app.openquery.as_ref().unwrap();
        assert!(e.menu_coluna.is_none());
        assert_eq!(e.resultado.cabecalho, ["Votos"]);
    }
}
