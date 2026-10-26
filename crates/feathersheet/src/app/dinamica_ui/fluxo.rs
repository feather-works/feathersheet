//! Fluxo de criação igual ao do TypeScript: primeiro só origem e destino;
//! depois o painel lateral "Campos da Tabela Dinâmica", que atualiza a grade
//! a cada mudança. Opções avançadas ficam recolhidas.
use super::*;
use feather_ui::icones::icone;
use pelicano::column;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fase { Origem, Campos }

const LARGURA_PAINEL: f32 = 370.0;

fn item<'a>(nome: &'static str, rotulo: String, tam: f32) -> Element<'a, Msg> {
    row![icone(nome, 14.0), text(rotulo).size(tam)].spacing(6).align(pelicano::Align::Center).into()
}

impl App {
    /// Mudanças que devem refletir de imediato na grade (fase Campos).
    pub(super) fn evento_atualiza_ao_vivo(evento: &Evento) -> bool {
        matches!(evento,
            Evento::Preset(_) | Evento::Mover(..) | Evento::DuplicarValor(_) | Evento::RemoverValor(_)
            | Evento::OperacaoValor(_) | Evento::OrigemAplicar | Evento::Linha(_) | Evento::Coluna(_)
            | Evento::Valor(_) | Evento::Operacao(_) | Evento::Agrupar | Evento::Layout
            | Evento::RepetirRotulos | Evento::Subtotais | Evento::Tema | Evento::FiltroValor(_)
            | Evento::FiltroModo | Evento::LimparFiltro | Evento::AdicionarCalculado
            | Evento::RemoverCalculado(_) | Evento::Recolher(_) | Evento::RecolherTodos(_)
            | Evento::Campo(_) | Evento::Remover(..) | Evento::RemoverFiltro(_)
            | Evento::ArrastoSoltar | Evento::MoverPara(..) | Evento::Reordenar(..)
            | Evento::FiltroTudo | Evento::FiltroCancelar | Evento::LayoutEscolher(_)
            | Evento::AgruparEscolher(_) | Evento::TemaEscolher(_)
            | Evento::OperacaoEscolher(..) | Evento::MostrarComo(..) | Evento::Ordenar(..)
            | Evento::AgruparCampo(..) | Evento::FaixaTexto(..) | Evento::TotaisGerais { .. }
            | Evento::AtualizarSozinha(_))
    }

    /// Recalcula a dinâmica aberta no painel sem fechá-lo.
    pub(super) fn atualizar_ao_vivo(&mut self) {
        let Some(e) = self.dinamica.clone() else { return };
        if e.fase != Fase::Campos || e.editando.is_none() { return; }
        let id = e.editando.clone().unwrap_or_default();
        let destino = (0..self.workbook.len()).find(|&i| self.workbook.aba(i)
            .and_then(|a| a.sheet.dn.as_deref())
            .and_then(|txt| serde_json::from_str::<Vec<serde_json::Value>>(txt).ok())
            .is_some_and(|d| d.iter().any(|p| p["id"] == id.as_str())));
        let Some(destino) = destino else { return };
        match self.aplicar_estado_na_dinamica(destino, &e) {
            Ok(()) => {
                // Lista de grupos recolhíveis acompanha o novo resultado.
                if let Ok((d, _, meta)) = edicao::localizar_selecionada(self) {
                    if let Ok(novo) = edicao::ler_estado(self, d, &meta) {
                        if let Some(atual) = self.dinamica.as_mut() { atual.grupos = novo.grupos; }
                    }
                }
            }
            Err(erro) => self.status = erro,
        }
    }

    /// Primeiro passo: confirmar origem e destino.
    pub(super) fn view_dinamica_origem<'a>(&'a self, e: &'a Estado) -> Element<'a, Msg> {
        let aba = self.workbook.aba(e.folha_origem).map(|a| a.sheet.name.as_str()).unwrap_or("");
        let destino = |rotulo: &'static str, ativo: bool, nova: bool| -> Element<'a, Msg> {
            button(item(if ativo { "checkbox" } else { "square" }, rotulo.into(), 12.0))
                .style(estilo_botao_pequeno(ativo))
                .on_press(Msg::Dinamica(Evento::DestinoNova(nova))).into()
        };
        let invalido = !e.origem_texto.trim().is_empty() && !origem::texto_valido(&e.origem_texto);
        let erro: Element<'a, Msg> = if invalido {
            text(tr!("Intervalo inválido. Use um formato como A1:D10.")).size(11).color(COR_TEXTO_MUTED()).into()
        } else { Space::new(0.0, 0).into() };
        let painel = column![
            row![
                icone("pivot-table", 22.0),
                text(tr!("Criar Tabela Dinâmica")).size(18),
                Space::new(Length::FILL, 0.0),
                button(icone("x", 16.0)).on_press(Msg::Dinamica(Evento::Fechar))
                    .style(estilo_botao_secundario),
            ].align(pelicano::Align::Center).spacing(9),
            text(tr!("Selecione a fonte dos dados. Em seguida escolha os campos no painel lateral."))
                .size(12).color(COR_TEXTO_MUTED()),
            text(trf!("Intervalo de Dados (Tabela Origem) — {aba}")).size(12),
            text_input("A1:D100", &e.origem_texto)
                .on_input(|v| Msg::Dinamica(Evento::OrigemTexto(v)))
                .on_submit(Msg::Dinamica(Evento::Criar))
                .padding(7),
            erro,
            text(tr!("Destino")).size(12),
            row![
                destino(tr!("Nova Aba"), e.nova_aba, true),
                destino(tr!("Aba Atual (ao lado da seleção)"), !e.nova_aba, false),
            ].spacing(8),
            row![
                text(tr!("A origem permanece intacta.")).size(11).color(COR_TEXTO_MUTED()),
                Space::new(Length::FILL, 0.0),
                button(tr!("Cancelar")).on_press(Msg::Dinamica(Evento::Fechar)).style(estilo_botao_secundario),
                button("OK").on_press(Msg::Dinamica(Evento::Criar)).style(estilo_botao_primario),
            ].spacing(8).align(pelicano::Align::Center),
        ].spacing(11);
        container(painel).padding(20).width(Length::Fixed(480.0)).style(estilo_painel_faixa).into()
    }

    /// Painel "Campos da Tabela Dinâmica".
    pub(super) fn view_dinamica_campos<'a>(&'a self, e: &'a Estado) -> Element<'a, Msg> {
        let c0 = e.origem.c0 as usize;
        let real = e.nomes.len().saturating_sub(e.calculados.len());
        let rotulo = |c: usize| e.nomes.get(c.saturating_sub(c0))
            .cloned().unwrap_or_else(|| trf!("Coluna {}", c + 1));
        let arrastando = e.ui.arrasto.as_ref().is_some_and(|a| a.movido);

        // ---- Lista de campos, com busca
        let busca = feather_ui::ajuda::normalizar(&e.ui.busca_campo);
        let mut campos = column(Vec::<pelicano::Element<_>>::new()).spacing(1);
        let mut algum = false;
        for (i, nome) in e.nomes.iter().enumerate() {
            if !busca.is_empty() && !feather_ui::ajuda::normalizar(nome).contains(&busca) { continue; }
            algum = true;
            let col = c0 + i;
            let marcado = e.linhas.contains(&col) || e.colunas.contains(&col)
                || e.valores.iter().any(|v| v.coluna == col)
                || e.filtros.iter().any(|f| f.coluna == col);
            let filtrado = e.filtros.iter().any(|f| f.coluna == col
                && !(f.excluir && f.valores.is_empty()));
            let caixa = button(icone(if marcado { "checkbox" } else { "square" }, 15.0))
                .padding([3, 4]).style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::Campo(i)));
            // O nome é a alça de arrastar; clique sem arrastar marca/desmarca.
            let nome = pelicano::mouse_area(
                container(text(nome.clone()).size(12)).padding([5, 4]).width(Length::FILL))
                .on_press(Msg::Dinamica(Evento::ArrastoInicio(col, None)))
                .cursor_icon(pelicano::CursorIcon::Grab);
            let mut linha = row![caixa, nome].spacing(2).align(pelicano::Align::Center);
            if i < real {
                linha = linha.push(button(icone("filter", 13.0)).padding([3, 5])
                    .style(estilo_botao_pequeno(filtrado))
                    .on_press(Msg::Dinamica(Evento::FiltroCampo(i))));
            }
            campos = campos.push(linha);
        }
        if !algum {
            campos = campos.push(text(tr!("Nenhum campo com esse nome")).size(11).color(COR_TEXTO_MUTED()));
        }
        let lista = pelicano::mouse_area(
            // Margem à direita: o ícone de filtro ficava atrás da barra de rolagem.
            container(scrollable(container(campos).padding(pelicano::Padding { right: 12.0, ..Default::default() }))
                .height(Length::Fixed(196.0)))
                .padding(4).width(Length::FILL)
                .style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_PAINEL())),
                    border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
                    ..Default::default()
                }))
            .on_enter(Msg::Dinamica(Evento::ArrastoSobre(Some(AreaDinamica::Lista))))
            .on_exit(Msg::Dinamica(Evento::ArrastoSair(AreaDinamica::Lista)));

        // ---- As quatro áreas
        let area = |titulo: &'static str, area: AreaDinamica, itens: Vec<(usize, String)>| -> Element<'a, Msg> {
            let destacada = arrastando && e.ui.sob == Some(area);
            let mut col = column![text(titulo).size(10.5).color(COR_TEXTO_MUTED())].spacing(3);
            if itens.is_empty() {
                col = col.push(text(if arrastando { tr!("Solte aqui") } else { tr!("Arraste campos aqui") })
                    .size(11).color(COR_TEXTO_MUTED()));
            }
            let total = itens.len();
            // Onde o campo arrastado vai entrar: uma linha azul.
            let marca = || -> Element<'a, Msg> {
                container(Space::new(Length::FILL, Length::Fixed(2.0))).style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_AZUL_ACENTO())),
                    ..Default::default()
                }).into()
            };
            let sob_item = if arrastando { e.ui.sob_item.filter(|(a, _)| *a == area) } else { None };
            for (i, (coluna, texto)) in itens.into_iter().enumerate() {
                if sob_item == Some((area, i)) { col = col.push(marca()); }
                let menu_aberto = e.ui.menu_item == Some((area, i));
                let chip = row![
                    // Uma linha só: "Soma de Acumulado" quebrava em duas.
                    pelicano::mouse_area(container(text(texto).size(11.5)
                        .no_wrap())
                        .padding([4, 6]).width(Length::FILL).clip(true))
                        .on_press(Msg::Dinamica(Evento::ArrastoInicio(coluna, Some((area, i)))))
                        .cursor_icon(pelicano::CursorIcon::Grab),
                    button(icone("chevron-down", 12.0)).padding([3, 4])
                        .style(estilo_botao_pequeno(menu_aberto))
                        .on_press(Msg::Dinamica(Evento::MenuItem(Some((area, i))))),
                ].spacing(0).align(pelicano::Align::Center);
                let caixa_chip = container(chip).style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_PAINEL())),
                    border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [6.0; 4] },
                    ..Default::default()
                });
                // Só durante o arrasto: passar o mouse à toa não gera mensagem.
                let campo: Element<'a, Msg> = if arrastando {
                    pelicano::mouse_area(caixa_chip)
                        .on_enter(Msg::Dinamica(Evento::ArrastoSobreItem(Some((area, i)))))
                        .on_exit(Msg::Dinamica(Evento::ArrastoSobreItem(None)))
                        .into()
                } else { caixa_chip.into() };
                col = col.push(campo);
            }
            if arrastando && total > 0 && e.ui.sob == Some(area) && sob_item.is_none() { col = col.push(marca()); }
            let caixa = container(scrollable(col).height(Length::Fixed(112.0)))
                .padding(6).width(Length::FILL)
                .style(move |_| pelicano::Style {
                    background: Some(pelicano::Background::Color(
                        if destacada { COR_AZUL_FUNDO() } else { COR_PAINEL_FAIXA() })),
                    border: pelicano::Border {
                        color: if destacada { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() },
                        width: if destacada { 1.5 } else { 1.0 }, radius: [8.0; 4],
                    },
                    ..Default::default()
                });
            pelicano::mouse_area(caixa)
                .on_enter(Msg::Dinamica(Evento::ArrastoSobre(Some(area))))
                .on_exit(Msg::Dinamica(Evento::ArrastoSair(area)))
                .into()
        };
        let filtros = e.filtros.iter().map(|f| (f.coluna, rotulo(f.coluna))).collect();
        let colunas = e.colunas.iter().map(|&c| (c, rotulo(c))).collect();
        let linhas = e.linhas.iter().map(|&c| (c, rotulo(c))).collect();
        let valores = e.valores.iter()
            .map(|v| (v.coluna, trf!("{} de {}", nome_operacao(v.op), rotulo(v.coluna)))).collect();
        let grade = column![
            row![area("FILTROS", AreaDinamica::Filtros, filtros),
                area("COLUNAS", AreaDinamica::Colunas, colunas)].spacing(8),
            row![area("LINHAS", AreaDinamica::Linhas, linhas),
                area("VALORES", AreaDinamica::Valores, valores)].spacing(8),
        ].spacing(8);

        let opcoes: Element<'a, Msg> = if e.avancado { self.controles_avancados(e) }
            else { Space::new(0.0, 0).into() };
        let corpo = column![
            row![
                icone("search", 15.0),
                text_input(tr!("Buscar campo"), &e.ui.busca_campo)
                    .on_input(|v| Msg::Dinamica(Evento::BuscaCampo(v)))
                    .padding(5).size(12),
            ].spacing(6).align(pelicano::Align::Center),
            lista,
            text(tr!("Arraste os campos entre as áreas")).size(11).color(COR_TEXTO_MUTED()),
            grade,
            // O menu ▾ de um campo sai embaixo das áreas: dentro da caixa
            // (112 px, com rolagem) ele ficava cortado.
            match e.ui.menu_item {
                Some((area, i)) => {
                    let total = match area {
                        AreaDinamica::Linhas => e.linhas.len(),
                        AreaDinamica::Colunas => e.colunas.len(),
                        AreaDinamica::Valores => e.valores.len(),
                        AreaDinamica::Filtros => e.filtros.len(),
                        AreaDinamica::Lista => 0,
                    };
                    self.menu_do_item(e, area, i, total)
                }
                None => Space::new(0.0, 0).into(),
            },
            match e.ui.config_campo {
                Some((area, i)) => self.view_config_campo(e, area, i),
                None => Space::new(0.0, 0).into(),
            },
            pelicano::horizontal_rule(1.0),
            button(item(if e.avancado { "chevron-down" } else { "chevron-right" }, tr!("Origem e campos calculados").into(), 12.5))
                .style(estilo_botao_pequeno(false)).padding([4, 2])
                .on_press(Msg::Dinamica(Evento::AlternarAvancado)),
            opcoes,
        ].spacing(10);
        let painel = column![
            row![
                icone("pivot-table", 20.0),
                text(tr!("Campos da Tabela Dinâmica")).size(15),
                Space::new(Length::FILL, 0.0),
                button(icone("x", 16.0)).on_press(Msg::Dinamica(Evento::Fechar)).style(estilo_botao_secundario),
            ].align(pelicano::Align::Center).spacing(8),
            scrollable(container(corpo).padding(pelicano::Padding { right: 10.0, ..Default::default() }))
                .height(Length::FILL),
        ].spacing(10);
        let topo = 43.0 + 52.0 + if self.config.faixa_recolhida { 0.0 }
            else { super::super::faixa_nativa::ALTURA_FAIXA_NATIVA };
        let altura = (self.tamanho_principal.height - topo - 40.0).max(260.0);
        // O botão direito no painel fica no painel: sem isto atravessava
        // e abria o menu da célula da grade embaixo.
        pelicano::mouse_area(container(painel).padding(12).width(Length::Fixed(LARGURA_PAINEL))
            .height(Length::Fixed(altura)).style(estilo_painel_faixa))
            .on_right_press(Msg::Dinamica(Evento::MenuItem(None)))
            .into()
    }

    /// O menu ▾ de um campo numa área: reordenar, mudar de área, configurar, remover.
    fn menu_do_item<'a>(&'a self, e: &'a Estado, area: AreaDinamica, i: usize, total: usize) -> Element<'a, Msg> {
        let opcao = |ic: &'static str, rotulo: String, msg: Option<Msg>| -> Element<'a, Msg> {
            button(item(ic, rotulo, 11.5)).width(Length::FILL).padding([4, 6])
                .style(estilo_botao_pequeno(false)).on_press_maybe(msg).into()
        };
        let mut menu = column![
            opcao("chevron-up", tr!("Mover para cima").into(),
                (i > 0).then_some(Msg::Dinamica(Evento::Reordenar(area, i, -1)))),
            opcao("chevron-down", tr!("Mover para baixo").into(),
                (i + 1 < total).then_some(Msg::Dinamica(Evento::Reordenar(area, i, 1)))),
        ].spacing(1);
        let calculado = e.coluna_em(area, i).is_some_and(|c|
            c >= e.origem.c0 as usize + e.nomes.len().saturating_sub(e.calculados.len()));
        for (destino, nome) in [(AreaDinamica::Filtros, tr!("Filtros")), (AreaDinamica::Colunas, tr!("Colunas")),
            (AreaDinamica::Linhas, tr!("Linhas")), (AreaDinamica::Valores, tr!("Valores"))] {
            if destino == area || (calculado && destino != AreaDinamica::Valores) { continue; }
            menu = menu.push(opcao("arrow-right", trf!("Mover para {nome}"),
                Some(Msg::Dinamica(Evento::MoverPara(area, i, destino)))));
        }
        match area {
            AreaDinamica::Valores => {
                let atual = e.valores.get(i).map(|v| nome_operacao(v.op)).unwrap_or("");
                // Abre as configurações com a lista de contas (Soma, Média,
                // Desvio padrão, Moda...): antes alternava às cegas.
                menu = menu.push(opcao("sum", trf!("Resumir por: {atual}…"),
                    Some(Msg::Dinamica(Evento::ConfigurarCampo(Some((area, i)))))));
                menu = menu.push(opcao("plus", tr!("Duplicar com outra conta").into(),
                    Some(Msg::Dinamica(Evento::DuplicarValor(i)))));
                menu = menu.push(opcao("percentage", tr!("Mostrar valores como…").into(),
                    Some(Msg::Dinamica(Evento::ConfigurarCampo(Some((area, i)))))));
            }
            AreaDinamica::Linhas | AreaDinamica::Colunas => {
                menu = menu.push(opcao("sort-ascending", tr!("Classificar e agrupar…").into(),
                    Some(Msg::Dinamica(Evento::ConfigurarCampo(Some((area, i)))))));
            }
            AreaDinamica::Filtros => if let Some(col) = e.coluna_em(area, i) {
                menu = menu.push(opcao("filter", tr!("Escolher itens…").into(),
                    Some(Msg::Dinamica(Evento::FiltroCampo(col - e.origem.c0 as usize)))));
            },
            _ => {}
        }
        let remover = match area {
            AreaDinamica::Filtros => e.coluna_em(area, i).map(|c| Evento::RemoverFiltro(c)),
            AreaDinamica::Valores => Some(Evento::RemoverValor(i)),
            AreaDinamica::Linhas => Some(Evento::Remover(EixoDinamica::Linhas, i)),
            AreaDinamica::Colunas => Some(Evento::Remover(EixoDinamica::Colunas, i)),
            AreaDinamica::Lista => None,
        };
        menu = menu.push(opcao("trash", tr!("Remover campo").into(), remover.map(Msg::Dinamica)));
        container(menu).padding(4).width(Length::FILL).style(estilo_popup_menu).into()
    }

    /// Lista flutuante de itens de um campo (o filtro), ao lado do painel.
    fn view_filtro_campo<'a>(&'a self, e: &'a Estado, col: usize) -> Element<'a, Msg> {
        let nome = e.nomes.get(col.saturating_sub(e.origem.c0 as usize)).cloned().unwrap_or_default();
        let filtro = e.filtros.iter().find(|f| f.coluna == col);
        let visivel = |chave: &String| filtro.is_none_or(|f|
            if f.excluir { !f.valores.contains(chave) } else { f.valores.contains(chave) });
        let tudo = e.valores_filtro.iter().all(|(chave, _)| visivel(chave));
        let busca = feather_ui::ajuda::normalizar(&e.ui.filtro_busca);
        let mut itens = column![
            button(item(if tudo { "checkbox" } else { "square" }, tr!("(Selecionar tudo)").into(), 12.0))
                .width(Length::FILL).style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::FiltroTudo)),
        ].spacing(1);
        for (chave, rotulo) in &e.valores_filtro {
            if !busca.is_empty() && !feather_ui::ajuda::normalizar(rotulo).contains(&busca) { continue; }
            itens = itens.push(button(item(if visivel(chave) { "checkbox" } else { "square" }, rotulo.clone(), 12.0))
                .width(Length::FILL).style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::FiltroValor(chave.clone()))));
        }
        let aviso: Element<'a, Msg> = if e.filtro_cortado {
            text(tr!("Lista parcial: mostra os primeiros valores da coluna.")).size(10.5)
                .color(COR_TEXTO_MUTED()).into()
        } else { Space::new(0.0, 0).into() };
        let cartao = column![
            row![icone("filter", 16.0), text(trf!("Filtrar: {nome}")).size(13.5)]
                .spacing(7).align(pelicano::Align::Center),
            row![
                icone("search", 14.0),
                text_input(tr!("Buscar"), &e.ui.filtro_busca)
                    .on_input(|v| Msg::Dinamica(Evento::FiltroBusca(v))).padding(5).size(12),
            ].spacing(6).align(pelicano::Align::Center),
            scrollable(itens).height(Length::Fixed(260.0)),
            aviso,
            row![
                Space::new(Length::FILL, 0.0),
                button(text(tr!("Cancelar")).size(12)).style(estilo_botao_secundario)
                    .on_press(Msg::Dinamica(Evento::FiltroCancelar)),
                button(text("OK").size(12)).style(estilo_botao_primario)
                    .on_press(Msg::Dinamica(Evento::FiltroOk)),
            ].spacing(8),
        ].spacing(9);
        pelicano::mouse_area(container(cartao).padding(12).width(Length::Fixed(260.0)).style(estilo_popup_menu))
            .on_right_press(Msg::Dinamica(Evento::MenuItem(None)))
            .into()
    }
    pub(in crate::app) fn sobrepor_dinamica<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        // Sem o painel de campos aberto, os cartões das segmentações.
        let Some(e) = &self.dinamica else { return self.sobrepor_segmentacoes(base); };
        if e.fase == Fase::Origem {
            let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
                .on_press(Msg::Dinamica(Evento::Fechar));
            let janela = container(self.view_dinamica_origem(e))
                .center()
                .width(Length::FILL).height(Length::FILL);
            return stack![base, fora, janela].height(Length::FILL).into();
        }
        // Painel lateral à direita: a grade continua visível e clicável.
        let topo = 43.0 + 52.0 + if self.config.faixa_recolhida { 0.0 }
            else { super::super::faixa_nativa::ALTURA_FAIXA_NATIVA };
        let x = (self.tamanho_principal.width - LARGURA_PAINEL - 8.0).max(0.0);
        let mut camadas = stack![
            base,
            column![
                Space::new(0.0, Length::Fixed(topo + 4.0)),
                row![Space::new(Length::Fixed(x), 0.0), self.view_dinamica_campos(e)],
            ],
        ].height(Length::FILL);
        if let Some(col) = e.campo_filtro {
            camadas = camadas.push(column![
                Space::new(0.0, Length::Fixed(topo + 60.0)),
                row![Space::new(Length::Fixed((x - 268.0).max(0.0)), 0.0), self.view_filtro_campo(e, col)],
            ]);
        }
        // O campo "na mão" segue o mouse enquanto é arrastado.
        if let Some(a) = e.ui.arrasto.as_ref().filter(|a| a.movido) {
            let nome = e.nomes.get(a.coluna.saturating_sub(e.origem.c0 as usize)).cloned().unwrap_or_default();
            camadas = camadas.push(column![
                Space::new(0.0, Length::Fixed((a.pos.y + 8.0).max(0.0))),
                row![
                    Space::new(Length::Fixed((a.pos.x + 12.0).max(0.0)), 0.0),
                    container(item("grip-vertical", nome, 12.0)).padding([5, 9]).style(estilo_popup_menu),
                ],
            ]);
        }
        camadas.into()
    }
}

