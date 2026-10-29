//! Editor principal, menus e comandos contextuais.
use super::*;
/// Submenus do menu de contexto da célula.
pub(super) const SUB_COLAR: u8 = 0;
pub(super) const SUB_INSERIR: u8 = 1;
/// Largura do botão Menu com a faixa recolhida (posição das listas do menu clássico).
const LARGURA_BOTAO_MENU: f32 = 84.0;
/// Largura de cada título do menu clássico (cabe "FERRAMENTAS").
const LARGURA_TITULO_MENU: f32 = 96.0;
use pelicano::column;

/// Editor nativo Iced com cursores e quebras de linha em uma única célula.
fn editor_multilinha<'a>(conteudo: &'a pelicano::Content,
    fonte: f32, altura: f32) -> Element<'a, Msg> {
    container(pelicano::text_editor(conteudo)
        // O Id vai no próprio editor: `campo_fx::focar_multilinha` foca por ele.
        .id("feathersheet.multilinha")
        .on_change(Msg::EditarMultilinha)
        .on_key(|tecla, mods| {
            use crate::compat::keyboard::key::Named; use crate::compat::keyboard::Key;
            use crate::compat::keyboard::Modificadores as _;
            if *tecla == Key::Named(Named::Enter) {
                // Alt+Enter: o editor quebra a linha sozinho.
                if mods.alt() { None } else { Some(Msg::EnviarEditor) }
            } else if *tecla == Key::Named(Named::Escape) {
                Some(Msg::Cancelar)
            } else {
                None
            }
        })
        .size(fonte).padding(5))
        .width(Length::FILL).height(Length::Fixed(altura)).into()
}


impl App {
    pub(super) fn view_editor(&self, id: window::Id) -> Element<'_, Msg> {
        let titulo = self.nome_documento();
        let barra = cromo::barra_editor(
            // Sem o botão "Arquivos": o botão Menu da faixa já abre o mesmo painel.
            &titulo, self.alterado, None,
            [
                cromo::Acao { icone:"file-plus", dica:tr!("Novo (Ctrl+N)"), mensagem:Some(Msg::Novo) },
                cromo::Acao { icone:"folder-open", dica:tr!("Abrir (Ctrl+O)"), mensagem:Some(Msg::Abrir) },
                cromo::Acao { icone:"device-floppy", dica:tr!("Salvar (Ctrl+S)"), mensagem:Some(Msg::Salvar) },
                cromo::Acao { icone:"device-floppy", dica:tr!("Salvar como"), mensagem:Some(Msg::SalvarComo) },
                cromo::Acao { icone:"file-export", dica:tr!("Exportar Excel"), mensagem:Some(Msg::ExportarXlsx) },
                cromo::Acao { icone:"arrow-back-up", dica:tr!("Desfazer (Ctrl+Z)"), mensagem:Some(Msg::Desfazer) },
                cromo::Acao { icone:"arrow-forward-up", dica:tr!("Refazer (Ctrl+Y)"), mensagem:Some(Msg::Refazer) },
                cromo::Acao { icone:"help", dica:tr!("Ajuda (F1)"), mensagem:Some(Msg::Ajuda) },
                cromo::Acao { icone:"device-desktop", dica:tr!("Monitor (F12)"), mensagem:Some(Msg::Monitor) },
            ], Msg::Configuracoes,
        );
        let topo: Element<'_, Msg> = if self.config.barra_propria {
            cromo::barra_integrada(barra, Self::acoes(id))
        } else { barra };
        let botao_menu = button(row![feather_ui::icones::icone("menu-2", 14.0), text(tr!("Menu")).size(13)]
            .spacing(6).align(pelicano::Align::Center))
            .padding([7, 12]).style(estilo_botao_secundario).on_press(Msg::Menu)
            // Largura fixa com a faixa recolhida: a lista do menu clássico se alinha por ela.
            .width(if self.config.faixa_recolhida { Length::Fixed(LARGURA_BOTAO_MENU) } else { Length::Shrink });
        // Faixa recolhida: os menus clássicos (Arquivo, Editar…) tomam o lugar das abas.
        let abas_escolha: Element<'_, Msg> = if self.config.faixa_recolhida {
            row![botao_menu, self.barra_menu_classico()].spacing(4).align(pelicano::Align::Center).into()
        } else { row![
            botao_menu,
            self.aba_faixa(tr!("Página Inicial"), AbaFaixa::Inicio),
            self.aba_faixa(tr!("Inserir"), AbaFaixa::Inserir),
            self.aba_faixa(tr!("Desenhar"), AbaFaixa::Desenhar),
            self.aba_faixa(tr!("Layout da Página"), AbaFaixa::Layout),
            self.aba_faixa(tr!("Fórmulas"), AbaFaixa::Formulas),
            self.aba_faixa(tr!("Dados"), AbaFaixa::Dados),
            self.aba_faixa(tr!("Revisão"), AbaFaixa::Revisao),
            self.aba_faixa(tr!("Exibir"), AbaFaixa::Exibir),
            self.aba_faixa(tr!("Macros"), AbaFaixa::Macros),
            self.aba_faixa(tr!("Ferramentas"), AbaFaixa::Ferramentas),
            // Contextual: visível só quando a seleção está dentro de uma tabela,
            // como em ribbon-tabela.ts. Não cria aba permanente.
            if tabelas::sob_celula(&self.workbook,self.folha,self.linha,self.coluna).is_some() {
                self.aba_faixa(tr!("Design da Tabela"), AbaFaixa::Tabela)
            } else { Space::new(0, 0.0).into() },
            // Contextual: "Tabela Dinâmica" com a célula numa dinâmica (ou o painel aberto).
            if self.dinamica_na_faixa() {
                self.aba_faixa(tr!("Tabela Dinâmica"), AbaFaixa::Dinamica)
            } else { Space::new(0, 0.0).into() },
            // Contextual: "Editar forma" só enquanto houver forma selecionada.
            if self.tem_forma_selecionada() { self.aba_editar_forma() }
            else { Space::new(0, 0.0).into() },
        ].spacing(4).align(pelicano::Align::Center).into() };
        let abas = row![
            // Reservar altura própria para a rolagem das abas: não cobrir os títulos.
            scrollable(abas_escolha).direction(pelicano::Direction::Horizontal)
                .height(Length::Fixed(44.)).width(Length::FILL),
            // Salvar, Desfazer e Refazer já estão na barra superior do editor.
            // Evitar duplicá-los libera espaço para as abas e para a pesquisa.
            self.faixa_pesquisa(),
            button(text(if self.config.faixa_recolhida {tr!("Expandir faixa")} else {tr!("Recolher faixa")}).size(12))
                .on_press(Msg::Faixa).style(estilo_botao_pequeno(false)),
        ].spacing(4).padding([4,8]).align(pelicano::Align::Center);
        let faixa = if self.config.faixa_recolhida { container(Space::new(0.0, 0)).into() }
        else { self.view_faixa() };
        // Seleção exatamente igual a uma tabela: a caixa mostra o nome dela
        // (Enter seleciona a tabela pelo nome). Enquanto o usuário digita, mostra o texto.
        let nome_tabela = if self.endereco_digitado { None } else {
            self.workbook.aba(self.folha).and_then(|aba| {
                let area = self.area_selecionada();
                tabelas::nome_da_area_exata(&tabelas::visuais(&aba.sheet), &area)
                    .map(str::to_owned)
            })
        };
        let caixa_nome = nome_tabela.or_else(|| self.nome_da_selecao())
            .unwrap_or_else(|| self.endereco.clone());
        let end = text_input("A1", caixa_nome.clone())
            .on_input(Msg::AlterarEndereco)
            .on_submit(Msg::IrPara(caixa_nome.clone()))
            .width(100).padding(8);
        let expansao: Element<'_, Msg> = if self.edicao_inline.is_none() {
            if let Some(conteudo) = &self.multilinha {
                column![
                    text(tr!("Alt+Enter: nova linha  •  Enter: confirmar  •  Esc: cancelar"))
                        .size(11).color(COR_TEXTO_MUTED()),
                    editor_multilinha(conteudo, 14.0, 105.0),
                ].spacing(4).into()
            } else { Space::new(0.0, 0).into() }
        } else { Space::new(0.0, 0).into() };
        // Vizinha de matriz derramada: a fórmula da âncora aparece em cinza,
        // como no Excel (a célula em si não tem conteúdo para editar).
        let exemplo_fx = self.workbook.aba(self.folha)
            .filter(|_| self.edicao.is_empty())
            .and_then(|a| a.sheet.area_derramada_em(self.linha, self.coluna)
                .filter(|area| (area.r0, area.c0) != (self.linha, self.coluna))
                .map(|area| feathersheet_core::celulas::locale::apresentar_formula(
                    &a.sheet.get_raw(area.r0, area.c0), &self.regionalizacao())))
            .unwrap_or_else(|| tr!("Valor ou fórmula (ex.: =SOMA(A1:A5))").to_string());
        // Apontando para a fórmula de outra janela: a barra mostra a fórmula
        // de lá (só leitura), com o arquivo dela, como nas duas pastas do Excel.
        let campo_fx: Element<'_, Msg> = match self.vinculos.formula_remota.as_ref().filter(|_| !self.editor_formula.ativo) {
            Some((arquivo, formula)) => row![
                text(format!("[{arquivo}]")).size(12).color(COR_AZUL_ACENTO()),
                text_input("", formula).width(Length::FILL).padding(8),
            ].spacing(6).align(pelicano::Align::Center).width(Length::FILL).into(),
            None => CampoFx::new(
                text_input(exemplo_fx, &self.edicao)
                    .id("feathersheet.fx")
                    .on_input(Msg::Editar).on_submit(Msg::EnviarEditor)
                    .width(Length::FILL).padding(8),
                &self.edicao
            ).into(),
        };
        let editor=column![
            row![
                end, text("fx").size(16).color(COR_AZUL_ACENTO()),
                campo_fx,
                button(feather_ui::icones::icone("circle-check", 16.0)
                    .color(pelicano::Color::WHITE))
                    .padding([6, 9]).style(estilo_botao_primario).on_press(Msg::Confirmar)
            ].spacing(8).align(pelicano::Align::Center),
            expansao,
        ].padding([7,12]).spacing(3);
        // O painel fica imediatamente abaixo de fx, sobre a grade. Antes,
        // a assinatura era enviada só à barra de status, fora do campo de visão.
        let mut assistente: Option<Element<'_, Msg>> = None;
        if self.mostrar_fx && self.editor_formula.ativo {
            let sugestoes = self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US");
            if !sugestoes.is_empty() {
                let mut opcoes = column(Vec::new()).spacing(2);
                for (i, s) in sugestoes.iter().enumerate() {
                    let linha = row![
                        text(s.name.clone()).size(13),
                        Space::new(Length::FILL, 0.0),
                        text(s.help.map(|h| crate::idioma::assinatura(&s.name, h))
                            .unwrap_or_else(|| s.signature.clone())).size(11).color(COR_TEXTO_MUTED()),
                    ].spacing(8).align(pelicano::Align::Center);
                    opcoes = opcoes.push(
                        button(linha)
                            .style(estilo_botao_pequeno(i == self.editor_formula.sugestao))
                            .on_press(Msg::AceitarSugestao(i))
                            .padding([6, 9]).width(Length::FILL),
                    );
                }
                assistente = Some(container(opcoes)
                    .padding(5).width(Length::Fixed(420.0))
                    .style(estilo_painel_faixa).into());
            } else if let Some((assinatura, indice, argumento, descricao)) =
                self.dica_funcao_ativa()
            {
                let info = column![
                    text(assinatura).size(13).color(COR_AZUL_ACENTO()),
                    text(trf!("Argumento {indice}: {argumento}")).size(12),
                    text(descricao).size(11).color(COR_TEXTO_MUTED()),
                ].spacing(5);
                assistente = Some(container(info)
                    .padding(10).width(Length::Fixed(510.0))
                    .style(estilo_painel_faixa).into());
            }
        }
        // A tinta fica por cima de imagens e botões (e captura o clique só no modo desenho).
        let grid = self.grade_com_segmentacoes(self.grade_com_tinta(self.grade_com_botoes_macro(self.grade_com_objetos(self.grade_com_imagens()))));
        // O Canvas calcula a posição real no duplo clique, considerando zoom,
        // rolagem, dimensões variáveis e painéis congelados. Só a célula ativa
        // recebe um editor; nunca criamos widgets para a grade inteira.
        let grid: Element<'_, Msg> = if let Some(edi) = self.edicao_inline.filter(|e|
            e.folha == self.folha && e.linha == self.linha && e.coluna == self.coluna
            && self.editor_formula.ativo)
        {
            let multiline = self.multilinha.as_ref();
            let largura = if multiline.is_some() { edi.largura.max(240.0) }
                else { edi.largura.max(40.0) };
            let altura = if multiline.is_some() {
                ((self.edicao.split('\n').count() as f32 + 1.0)
                    * (self.config.tamanho_fonte_grade as f32 * 1.45))
                    .max(edi.altura).clamp(90.0, 300.0)
            } else { edi.altura.max(24.0) };
            // A aparência da própria célula (#90): letra, tamanho, negrito,
            // itálico, cor e fundo, como no Excel.
            let visual = crate::estilos_comuns::projetar_estilo(self.workbook.aba(edi.folha)
                .and_then(|a| a.sheet.estilo_resolvido(edi.linha, edi.coluna)).as_ref(), None);
            let basico = &visual.comum.basico;
            let cor_rgba = |[r, g, b, a]: [u8; 4]| pelicano::Color::rgb8(r, g, b).alpha(a as f32 / 255.0);
            // A célula é clara mesmo no tema escuro (a grade não escurece).
            let fundo = basico.fundo.map(cor_rgba).unwrap_or(pelicano::Color::WHITE);
            let cor_texto = basico.texto.map(cor_rgba).or(Some(pelicano::Color::rgb8(32, 33, 36)));
            let fonte = (basico.tamanho.unwrap_or(self.config.tamanho_fonte_grade as f32)
                * self.config.zoom as f32 / 100.0).max(11.0);
            let letra = crate::estilos_comuns::fonte_da_celula(&visual, &self.config.fonte_grade);
            let entrada: Element<'_, Msg> = if let Some(conteudo) = multiline {
                editor_multilinha(conteudo, fonte, altura - 4.0)
            } else {
                CampoFx::new(
                    text_input("", &self.edicao)
                        .id("feathersheet.celula")
                        .on_input(Msg::Editar).on_submit(Msg::EnviarEditor)
                        .size(fonte).font(letra).padding([3, 5]).width(Length::FILL)
                        // Sem a borda arredondada e o fundo do campo: a moldura é a
                        // do container, do tamanho da célula (o campo parecia menor).
                        // shortcut: o text_input do pelicano não tem align_x; célula
                        // centralizada edita à esquerda até o pelicano ganhar isso.
                        .style(move |t, _| pelicano::Style::default().text_color(cor_texto.unwrap_or(t.text))),
                    &self.edicao,
                ).into()
            };
            let camada = column![
                Space::new(0.0, Length::Fixed(edi.y.max(0.0))),
                row![
                    Space::new(Length::Fixed(edi.x.max(0.0)), 0.0),
                    container(entrada)
                        .width(Length::Fixed(largura))
                        .height(Length::Fixed(altura))
                        .align_y(pelicano::Align::Center)
                        .style(move |_| pelicano::Style {
                            background: Some(pelicano::Background::Color(fundo)),
                            border: pelicano::Border {
                                color: COR_AZUL_ACENTO(),
                                width: 2.0,
                                radius: [0.0; 4],
                            },
                            ..Default::default()
                        }),
                ],
            ];
            stack![grid, camada].height(Length::FILL).width(Length::FILL).into()
        } else { grid };
        // Abas + posição + zoom no MESMO rodapé (antes eram duas barras
        // sobrepostas em altura: 42 px das abas + ~35 px de status).
        // Mantém ações e o aviso temporário de abas, ganhando área útil.
        // Ícones SVG com arredondamento; o sinal Unicode '−' falhava em algumas fontes.
        let botao_zoom = |nome: &'static str, percentual: Option<u16>| {
            button(feather_ui::icones::icone(nome, 16.0)
                .color(pelicano::Color::WHITE))
                .on_press_maybe(percentual.map(Msg::Zoom))
                .width(Length::Fixed(28.0)).height(Length::Fixed(28.0))
                .padding(5)
                .style(|tema, estado| {
                    let mut estilo = estilo_botao_primario(tema, estado);
                    estilo.border.radius = [9.0; 4];
                    estilo
                })
        };
        let zoom = container(row![
            botao_zoom("zoom-out", zoom::proximo(
                self.config.zoom, -1, self.config.zoom_minimo, self.config.zoom_maximo)),
            // Como na barra TS: clicar no percentual restaura o zoom de 100%.
            button(text(format!("{}%", self.config.zoom)).size(11))
                .padding([4, 7]).style(estilo_botao_pequeno(false))
                .on_press(Msg::Zoom(100)),
            botao_zoom("zoom-in", zoom::proximo(
                self.config.zoom, 1, self.config.zoom_minimo, self.config.zoom_maximo)),
        ].spacing(7).align(pelicano::Align::Center).padding(2))
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border {
                    color: COR_BORDA(), width: 1.0, radius: [12.0; 4],
                },
                ..Default::default()
            });
        // Da posição ao zoom: o botão direito abre "Personalizar barra de
        // status" (as abas à esquerda têm o menu próprio delas).
        let celula: Element<'_, Msg> = if self.barra_status_mostra("celula") {
            text(self.texto_celula_abas()).size(11).color(COR_TEXTO_MUTED()).into()
        } else { Space::new(0, 0.0).into() };
        let zoom: Element<'_, Msg> = if self.barra_status_mostra("zoom") {
            zoom.into()
        } else { Space::new(0, 0.0).into() };
        let status = pelicano::mouse_area(
            // As guias ocupam o resto (Fill): isto fica à direita, colado no zoom.
            row![celula, self.view_resumo_selecao(), self.aviso_calcular(), zoom]
                .spacing(5).height(Length::FILL).align(pelicano::Align::Center),
        ).on_right_press(Msg::BarraStatus(barra_status::Evento::Abrir));
        let rodape = row![
            self.view_abas(),
            // Aviso temporário ao lado das guias (antes cobria as abas).
            self.view_aviso_abas().unwrap_or_else(|| Space::new(0, 0.0).into()),
            self.view_progresso_abertura(),
            self.view_progresso_dinamica(),
            status,
        ].spacing(5).padding([0, 8]).height(Length::Fixed(36.0))
            .align(pelicano::Align::Center);
        let centro: Element<'_, Msg> = if let Some((tipo, x, y)) = self.contexto {
            let fora = button(Space::new(Length::FILL, 0.0))
                .width(Length::FILL).height(Length::FILL)
                .style(|_, _| pelicano::Style::default())
                .on_press(Msg::FecharContexto);
            let popup = if matches!(tipo, Contexto::Aba(_)) {
                column![
                    Space::new(0.0, Length::FILL),
                    row![
                        Space::new(Length::Fixed(x), 0.0),
                        self.view_contexto(tipo, self.contexto_tamanho)
                    ]
                ]
            } else {
                column![
                    Space::new(0.0, Length::Fixed(y)),
                    row![Space::new(Length::Fixed(x), 0.0),
                        self.view_contexto(tipo, self.contexto_tamanho)]
                ]
            };
            match self.view_submenu_contexto(tipo, x, y) {
                Some(sub) if !matches!(tipo, Contexto::Aba(_)) => stack![grid, fora, popup, sub].height(Length::FILL).into(),
                _ => stack![grid, fora, popup].height(Length::FILL).into(),
            }
        } else { grid };
        // A busca flutua sobre a grade, no canto superior direito, sem
        // empurrar a planilha para baixo (como a janela do Excel).
        let centro: Element<'_, Msg> = match self.view_busca() {
            Some(janela) => stack![
                centro,
                janela,
            ].height(Length::FILL).width(Length::FILL).into(),
            None => centro,
        };
        let centro = self.sobrepor_atingir_meta(centro);
        let centro = self.sobrepor_tabela_dados(centro);
        let centro = self.sobrepor_cenarios(centro);
        let editor_visivel: Element<'_, Msg> = if self.mostrar_fx {
            container(editor).style(estilo_container_topo).into()
        } else { Space::new(0.0, 0).into() };
        let corpo = column![
            editor_visivel,
            self.view_faixa_salvamento(),
            self.view_faixa_vinculos(),
            self.com_graficos_salvos(centro),
            container(rodape).style(estilo_rodape),
        ].height(Length::FILL);
        let base: Element<'_, Msg> = column![
            topo,
            container(abas).style(estilo_barra_abas),
            container(faixa).style(estilo_corpo_faixa),
            stack![
                corpo,
                column![
                    Space::new(0.0, 52),
                    row![Space::new(132, 0.0),
                        assistente.unwrap_or_else(|| Space::new(0.0, 0).into())]
                ]
            ].height(Length::FILL)
        ].height(Length::FILL).into();
        let base: Element<'_, Msg> = if self.efeitos_imagem.aberto {
            // Overlay de edição: a grade não perde área e a imagem permanece
            // ancorada. Fechar/Exportar são ações explícitas do painel.
            stack![
                base,
                column![
                    Space::new(0.0, Length::Fixed(160.)),
                    row![
                        Space::new(Length::Fixed(166.), 0.0),
                        container(self.efeitos_imagem.painel())
                            .width(Length::Fixed(555.))
                            .style(estilo_painel_faixa),
                    ]
                ]
            ].height(Length::FILL).into()
        } else { base };
        let base=self.sobrepor_galeria_graficos(base);
        let base=self.sobrepor_editor_botao_macro(base);
        let base: Element<'_, Msg> = if self.reexibir_abas_aberto {
            // Reexibição por escolha explícita, como a lista do TypeScript.
            // As abas permanecem em memória e na mesma posição da pasta.
            let fora = pelicano::mouse_area(
                Space::new(Length::FILL, Length::FILL))
                .on_press(Msg::FecharAbasOcultas);
            let mut lista = column![
                text(tr!("Reexibir aba")).size(19),
                text(tr!("Selecione qual aba oculta deseja tornar visível.")).size(12)
                    .color(COR_TEXTO_MUTED()),
            ].spacing(10);
            for i in self.workbook.abas_ocultas() {
                if let Some(aba) = self.workbook.aba(i) {
                    lista = lista.push(
                        button(row![
                            feather_ui::icones::icone("table", 16.0),
                            text(aba.sheet.name.clone()).size(13),
                        ].spacing(9).align(pelicano::Align::Center))
                        .width(Length::FILL).padding([10, 12])
                        .on_press(Msg::ReexibirAba(i))
                        .style(estilo_botao_secundario));
                }
            }
            lista = lista.push(
                button(tr!("Cancelar"))
                    .on_press(Msg::FecharAbasOcultas)
                    .style(estilo_botao_secundario));
            let painel = container(scrollable(lista.padding(16)).height(Length::Shrink))
                .width(Length::Fixed(390.0))
                .style(estilo_painel_faixa);
            stack![
                base,
                fora,
                container(painel)
                    .center(),
            ].height(Length::FILL).into()
        } else { base };

        let base: Element<'_, Msg> = if let Some(area) = &self.tabela_area {
            let local = endereco_area(area);
            let proposta = column![
                text(tr!("Criar tabela")).size(18),
                text(trf!("Intervalo: {local}")).size(12),
                text(tr!("Nome da tabela")).size(12),
                text_input("Tabela1", &self.tabela_nome)
                    .on_input(Msg::TabelaNome).padding(8),
                row![
                    text(tr!("Minha tabela tem cabeçalhos")).size(12),
                    toggler("", self.tabela_cabecalho).on_toggle(Msg::TabelaCabecalho),
                ].spacing(12).align(pelicano::Align::Center),
                text(tr!("O intervalo será preservado. O estilo azul e as faixas são visuais."))
                    .size(11),
                row![
                    button(tr!("Cancelar")).on_press(Msg::TabelaCancelar)
                        .style(estilo_botao_secundario),
                    button(tr!("Criar tabela")).on_press(Msg::TabelaConfirmar)
                        .style(estilo_botao_primario),
                ].spacing(10),
            ].spacing(10);
            // No meio da tela, como os outros diálogos (antes ficava preso sob a faixa).
            stack![
                base,
                container(container(proposta).width(Length::Fixed(380.))
                    .padding(16).style(estilo_painel_faixa))
                    .center(),
            ].height(Length::FILL).into()
        } else { base };
        // Dois popups independentes: o principal preserva valores e ações
        // enquanto "Filtrar por condição" abre um flyout separado na frente.
        let base: Element<'_, Msg> = if self.tabela_popup_filtro {
            let fora = pelicano::mouse_area(
                Space::new(Length::FILL, Length::FILL)
            ).on_press(Msg::TabelaAcao(tabelas::Acao::Fechar));
            let largura = tabelas::filtro_popup::LARGURA
                .min((self.tamanho_principal.width - 16.0).max(1.0));
            let x = self.tabela_popup_x.clamp(
                8.0, (self.tamanho_principal.width - largura - 8.0).max(8.0)
            );
            let topo_grade = 43.0 + 52.0
                + if self.config.faixa_recolhida { 0.0 }
                  else { faixa_nativa::ALTURA_FAIXA_NATIVA }
                + if self.mostrar_fx { 54.0 } else { 0.0 };
            let desejado = topo_grade + self.tabela_popup_y + 4.0;
            // O tamanho do painel principal não muda ao entrar no submenu.
            let y = desejado.min(
                (self.tamanho_principal.height - 490.0 - 8.0).max(8.0)
            );
            let principal = column![
                Space::new(0.0, Length::Fixed(y)),
                row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.menu_filtro_tabela(),
                ],
            ];
            if self.tabela_filtro_pagina != 0 {
                let altura_flyout = if self.tabela_filtro_pagina == 1 { 310.0 } else { 180.0 };
                let (sub_x, sub_y) = tabelas::filtro_popup::posicao_condicoes(
                    x, y, self.tamanho_principal.width, self.tamanho_principal.height,
                    altura_flyout,
                );
                // Última camada do stack: recebe cliques antes do principal.
                // A janelinha troca de lado junto à borda, sem desaparecer.
                stack![
                    base,
                    fora,
                    principal,
                    column![
                        Space::new(0.0, Length::Fixed(sub_y)),
                        row![
                            Space::new(Length::Fixed(sub_x), 0.0),
                            self.menu_condicoes_tabela(),
                        ],
                    ],
                ].height(Length::FILL).into()
            } else {
                stack![base, fora, principal].height(Length::FILL).into()
            }
        } else { base };
        let base: Element<'_, Msg> = if self.tabela_popup_totais {
            let fora = pelicano::mouse_area(
                Space::new(Length::FILL, Length::FILL)
            ).on_press(Msg::TabelaAcao(tabelas::Acao::Fechar));
            let x = self.tabela_popup_x.clamp(8.0,
                (self.tamanho_principal.width - tabelas::totais_popup::LARGURA - 8.0).max(8.0));
            let topo_grade = 43.0 + 52.0
                + if self.config.faixa_recolhida { 0.0 }
                  else { faixa_nativa::ALTURA_FAIXA_NATIVA }
                + if self.mostrar_fx { 54.0 } else { 0.0 };
            // A linha de totais costuma ficar no pé da tela: se não couber
            // abaixo da célula, a lista abre para cima.
            let abaixo = topo_grade + self.tabela_popup_y + 4.0;
            let y = if abaixo + tabelas::totais_popup::ALTURA + 8.0 > self.tamanho_principal.height {
                (topo_grade + self.tabela_popup_y - tabelas::totais_popup::ALTURA - 28.0).max(8.0)
            } else { abaixo };
            stack![
                base, fora,
                column![
                    Space::new(0.0, Length::Fixed(y)),
                    row![Space::new(Length::Fixed(x), 0.0), self.menu_totais_tabela()],
                ],
            ].height(Length::FILL).into()
        } else { base };
        let base: Element<'_, Msg> = if self.barra_menu_aberta && self.config.faixa_recolhida {
            // Topo (barra do título e menus) fica de fora: passar o mouse troca de menu.
            let topo_lista = 43.0 + 52.0 - 4.0;
            stack![
                base,
                column![
                    Space::new(0.0, Length::Fixed(topo_lista)),
                    pelicano::mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(Msg::FecharMenu),
                ],
                column![Space::new(0.0, Length::Fixed(topo_lista)), self.painel_menu_classico()],
            ].height(Length::FILL).into()
        } else { base };
        let base = self.sobrepor_condicional(base);
        let base = self.sobrepor_perfil_coluna(base);
        let base = self.sobrepor_estatisticas(base);
        let base = self.sobrepor_barra_status(base);
        let base = self.sobrepor_nomes(base);
        let base = self.sobrepor_congelar(base);
        let base = self.sobrepor_menu_tinta(base);
        let base = self.sobrepor_menu_formas(base);
        let base = self.sobrepor_menu_imagem(base);
        let base = self.sobrepor_menu_formulas(base);
        let base = self.sobrepor_criar_nomes(base);
        let base = self.sobrepor_avaliar(base);
        let base = self.sobrepor_link(base);
        let base = self.sobrepor_comentario(base);
        let base = self.sobrepor_dimensao(base);
        let base = self.sobrepor_celulas(base);
        let base = self.sobrepor_subtotal(base);
        let base = self.sobrepor_protecao(base);
        let base = self.sobrepor_novo_estilo(base);
        let base = self.sobrepor_colar_especial(base);
        let base = self.sobrepor_ir_para(base);
        let base = self.sobrepor_classificar(base);
        let base = self.sobrepor_dinamica(base);
        let base = if let Some(alvo) = self.cor_ativa {
            let atual = self.cor_da_celula(alvo);
            let titulo = match alvo {
                AlvoCor::Texto => tr!("Cor do texto"),
                AlvoCor::Fundo => tr!("Cor de preenchimento"),
            };
            let painel = seletor_cor::painel(titulo, &atual, self.cor_hsva, &self.cor_hex)
                .map(Msg::SeletorCor);
            let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
                .on_press(Msg::SeletorCor(EventoCor::Fechar));
            // Overlay, como no FeatherSlide: mantém a grade no lugar.
            // Alinhado ao grupo Fonte no início da Ribbon.
            let pos_x = match alvo { AlvoCor::Texto => 185., AlvoCor::Fundo => 260. };
            stack![
                base,
                fora,
                column![
                    Space::new(0.0, Length::Fixed(128.)),
                    row![Space::new(Length::Fixed(pos_x), 0.0), painel],
                ],
            ].height(Length::FILL).into()
        } else { base };
        let base = self.sobrepor_faixa_extras(base);
        let base = self.sobrepor_faixa_grupo(base);
        let base = self.sobrepor_menu_colagem(base);
        let base = self.sobrepor_menu_bordas(base);
        let base = self.sobrepor_menu_orientacao(base);
        let base = self.sobrepor_estilos(base);
        let base = self.sobrepor_menu_edicao(base);
        let base = self.sobrepor_menu_principal(base);
        if self.macro_rodando.is_some() {
            return self.sobrepor_macro_rodando(base);
        }
        if self.calculo_rodando.is_some() {
            return self.sobrepor_calculo_rodando(base);
        }
        if self.macro_pergunta.is_some() || !self.macro_mensagens.is_empty() {
            return self.sobrepor_dialogo_macro(base);
        }

        // Confirmação modal sobre TODA a janela: não substitui nem redimensiona
        // a grade e impede cliques/atalhos acidentais enquanto o usuário decide.
        if let Some(destino) = self.pendente {
            let contexto = match destino {
                Pendente::Novo | Pendente::Modelo(_) => tr!("criar uma nova planilha"),
                Pendente::Abrir | Pendente::Recente => tr!("abrir outra planilha"),
                Pendente::RecuperarTemporario => tr!("recuperar uma cópia temporária"),
                Pendente::Sair => tr!("fechar o FeatherSheet"),
            };
            let largura = 500.0_f32.min((self.tamanho_principal.width - 28.0).max(260.0));
            let cabecalho = row![
                feather_ui::icones::icone("info-circle", 26.0),
                column![
                    text(tr!("Alterações não salvas")).size(20),
                    text(trf!("Antes de {contexto}, escolha o que fazer."))
                        .size(12).color(COR_TEXTO_MUTED()),
                ].spacing(5)
            ].spacing(12).align(pelicano::Align::Center);
            let documento = container(row![
                feather_ui::icones::icone("file-plus", 18.0),
                text(self.nome_documento()).size(13),
                Space::new(Length::FILL, 0.0),
                text(tr!("Não salvo")).size(11).color(COR_TEXTO_MUTED()),
            ].spacing(9).align(pelicano::Align::Center))
                .width(Length::FILL).padding(12).style(estilo_container_topo);
            let salvar = button(row![
                feather_ui::icones::icone("device-floppy", 18.0),
                text(tr!("Salvar e continuar")).size(14),
            ].spacing(10).align(pelicano::Align::Center))
                .on_press(Msg::SalvarAntes).width(Length::FILL)
                .padding([12, 14]).style(estilo_botao_primario);
            let temporario = button(row![
                feather_ui::icones::icone("clock", 18.0),
                column![
                    text(tr!("Guardar temporariamente")).size(14),
                    text(tr!("Recupere depois pelo menu Arquivo")).size(11),
                ].spacing(3),
            ].spacing(10).align(pelicano::Align::Center))
                .on_press(Msg::GuardarTemporariamente).width(Length::FILL)
                .padding([10, 14]).style(estilo_botao_secundario);
            let descartar = button(text(tr!("Descartar alterações")).size(12))
                .on_press(Msg::Descartar).width(Length::FILL)
                .padding([10, 8]).style(|tema, estado| {
                    let mut estilo = estilo_botao_secundario(tema, estado);
                    estilo.text_color = Some(pelicano::Color::rgb8(184, 49, 57));
                    estilo
                });
            let cancelar = button(text(tr!("Cancelar")).size(12))
                .on_press(Msg::Cancelar).width(Length::FILL)
                .padding([10, 8]).style(estilo_botao_secundario);
            let painel = container(column![
                cabecalho,
                documento,
                text(tr!("Não perca as alterações desta planilha.")).size(12)
                    .color(COR_TEXTO_MUTED()),
                salvar,
                temporario,
                row![descartar, cancelar].spacing(10),
            ].spacing(13))
                .padding(22).width(Length::Fixed(largura))
                .style(estilo_painel_faixa);
            let fundo = pelicano::mouse_area(
                container(Space::new(Length::FILL, Length::FILL))
                    .style(|_| pelicano::Style {
                        background: Some(pelicano::Background::Color(
                            pelicano::Color::rgba(0.0, 0.0, 0.0, 0.42))),
                        ..Default::default()
                    })
            ).on_press(Msg::Cancelar);
            return stack![
                base,
                fundo,
                container(painel).width(Length::FILL).height(Length::FILL)
                    .align_x(pelicano::Align::Center)
                    .align_y(pelicano::Align::Center),
            ].height(Length::FILL).into();
        }
        if self.erro_explicando.is_some() {
            return self.sobrepor_explicacao_erro(base);
        }
        let base = self.sobrepor_aviso_xlsx(base);
        let base = self.sobrepor_aviso_macros(base);
        let base = self.sobrepor_editar_links(base);
        // Validação de dados e o alerta de erro, no diálogo interno comum.
        let base = self.sobrepor_serie(base);
        self.sobrepor_validacao(base)
    }

    /// Menu nativo hierárquico, construído a partir da definição original de menus.ts.
    /// Opções alinhadas ao contexto de célula, cabeçalho ou aba do TypeScript.
    /// Ações ainda sem implementação Rust não aparecem como botões falsos.
    pub(super) fn opcoes_contexto(&self, tipo: Contexto) -> Vec<Option<(String, Msg)>> {
        let mut opcoes: Vec<Option<(String, Msg)>> = vec![];
        if !matches!(tipo, Contexto::Aba(_)) {
            opcoes.extend([
                Some((tr!("Recortar").into(), Msg::Recortar)),
                Some((tr!("Copiar").into(), Msg::Copiar)),
                Some((tr!("Colar").into(), Msg::ColarSolicitar)),
                // As variações de colar ficam no submenu (submenu_contexto 0).
                Some((tr!("Colar especial").into(), Msg::ContextoSub(Some(SUB_COLAR)))),
            ]);
            if matches!(tipo, Contexto::Celula(..)) {
                opcoes.push(Some((tr!("Inserir").into(), Msg::ContextoSub(Some(SUB_INSERIR)))));
            }
            opcoes.extend([
                None,
            ]);
        }
        match tipo {
            Contexto::Celula(_, _) => {
                // Como em botao-direito.ts: o diagnóstico só aparece com erro na célula ativa.
                if feathersheet_core::formula::explicar::explicar_erro(&self.workbook, self.folha, self.linha, self.coluna).is_some() {
                    opcoes.extend([
                        Some((tr!("Explicar este erro").into(), Msg::ExplicacaoErroAbrir)),
                        None,
                    ]);
                }
                // Imagem: tirar da célula, pôr a solta selecionada na célula,
                // ou escolher um arquivo para a célula ativa.
                use imagem_na_celula::Evento as Img;
                if self.celula_tem_imagem() {
                    opcoes.push(Some((tr!("Colocar imagem sobre as células").into(), Msg::ImagemCelula(Img::ColocarSobre))));
                }
                if self.imagem_selecionada.is_some() {
                    opcoes.push(Some((tr!("Colocar imagem na célula").into(), Msg::ImagemCelula(Img::ColocarNaCelula))));
                }
                opcoes.extend([
                    Some((tr!("Inserir imagem na célula…").into(), Msg::ImagemCelula(Img::Inserir))),
                    None,
                ]);
                opcoes.extend([
                    Some((tr!("Limpar conteúdo").into(), Msg::Limpar)),
                    None,
                    // Inserir linha/coluna/células estão no submenu Inserir ▸.
                    Some((tr!("Excluir linha").into(), Msg::Estrutura(Estrutura::ExcluirLinha))),
                    Some((tr!("Excluir coluna").into(), Msg::Estrutura(Estrutura::ExcluirColuna))),
                    Some((tr!("Excluir…").into(), Msg::Celulas(celulas_dialogo::Evento::Abrir(false)))),
                    None,
                    Some((tr!("Localizar e substituir").into(), Msg::Busca)),
                    Some((tr!("Validação de dados").into(), Msg::ValidacaoAbrir)),
                    None,
                    Some((tr!("Inserir / editar link").into(), Msg::LinkEditar)),
                    self.link_da_celula(self.folha, self.linha, self.coluna)
                        .map(|_| (tr!("Abrir link").into(), Msg::LinkAbrir)),
                    Some((tr!("Remover hiperlink").into(), Msg::LinkRemover)),
                    None,
                ]);
                opcoes.extend(self.opcoes_contexto_comentario());
                opcoes.extend([
                    None,
                    Some((tr!("Limpar formatação").into(), Msg::LimparFormato)),
                    Some((tr!("Definir intervalo nomeado…").into(), Msg::NomesAbrir(true))),
                    Some((tr!("Perfil da coluna").into(), Msg::PerfilColunaAbrir)),
                ]);
                // Itens de tabela dinâmica e de tabela, na mesma regra de botao-direito.ts.
                let dinamica = self.dinamica_sob_celula();
                let tabela = tabelas::sob_celula(&self.workbook, self.folha, self.linha, self.coluna).is_some();
                let mut bloco: Vec<Option<(String, Msg)>> = vec![];
                if dinamica {
                    bloco.push(Some((tr!("Editar Tabela Dinâmica…").into(), Msg::Dinamica(dinamica_ui::Evento::Editar))));
                }
                if tabela {
                    bloco.extend([
                        Some((tr!("Linha de totais").into(), Msg::TabelaAcao(tabelas::Acao::Totais))),
                        Some((tr!("Renomear tabela…").into(), Msg::TabelaAcao(tabelas::Acao::IniciarRenomeacao))),
                        None,
                        Some((tr!("Alternar / Remover Filtros").into(), Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro))),
                        Some((tr!("Converter em Intervalo").into(), Msg::TabelaAcao(tabelas::Acao::Converter))),
                    ]);
                } else if !dinamica {
                    bloco.push(Some((tr!("Criar Tabela").into(), Msg::TabelaAbrir)));
                }
                if !bloco.is_empty() {
                    opcoes.push(None);
                    opcoes.extend(bloco);
                }
            }
            Contexto::Linha(_) => {
                opcoes.extend([
                    Some((tr!("Inserir linha acima").into(), Msg::InserirMenu(Estrutura::InserirLinha, false))),
                    Some((tr!("Inserir linha abaixo").into(), Msg::InserirMenu(Estrutura::InserirLinha, true))),
                    Some((tr!("Excluir linha").into(), Msg::Estrutura(Estrutura::ExcluirLinha))),
                    None,
                    Some((tr!("Limpar conteúdo").into(), Msg::Limpar)),
                    None,
                    Some((tr!("Altura da linha…").into(), Msg::Dimensao(dimensao_dialogo::Evento::Abrir(false)))),
                    Some((tr!("Ocultar linhas").into(), Msg::OcultarDimensoes(false))),
                    Some((tr!("Reexibir linhas").into(), Msg::ReexibirDimensoes(false))),
                ]);
            }
            Contexto::Coluna(_) => {
                opcoes.extend([
                    Some((tr!("Inserir coluna à esquerda").into(), Msg::InserirMenu(Estrutura::InserirColuna, false))),
                    Some((tr!("Inserir coluna à direita").into(), Msg::InserirMenu(Estrutura::InserirColuna, true))),
                    Some((tr!("Excluir coluna").into(), Msg::Estrutura(Estrutura::ExcluirColuna))),
                    None,
                    Some((tr!("Limpar conteúdo").into(), Msg::Limpar)),
                    None,
                    Some((tr!("Perfil da coluna").into(), Msg::PerfilColunaAbrir)),
                    None,
                    Some((tr!("Largura da coluna…").into(), Msg::Dimensao(dimensao_dialogo::Evento::Abrir(true)))),
                    Some((tr!("Ocultar colunas").into(), Msg::OcultarDimensoes(true))),
                    Some((tr!("Reexibir colunas").into(), Msg::ReexibirDimensoes(true))),
                ]);
            }
            Contexto::Aba(i) => {
                opcoes.extend([
                    Some((tr!("Nova aba").into(), Msg::AdicionarAba)),
                    Some((tr!("Renomear aba").into(), Msg::EditarNomeAba)),
                    Some((tr!("Cor da aba…").into(), Msg::AlternarPaletaAba)),
                    Some((tr!("Duplicar aba").into(), Msg::DuplicarAba)),
                ]);
                let visiveis = self.workbook.abas_visiveis().collect::<Vec<_>>();
                let (esquerda, direita) = self.vizinhas_visiveis(i);
                // Só apresentar deslocamentos possíveis. Abas ocultas não
                // aparecem no rodapé, mas continuam na ordenação física.
                if let Some(alvo) = esquerda {
                    opcoes.push(Some((tr!("Mover aba à esquerda").into(), Msg::MoverAba(i, alvo))));
                }
                if let Some(alvo) = direita {
                    opcoes.push(Some((tr!("Mover aba à direita").into(), Msg::MoverAba(i, alvo))));
                }
                if let Some(&primeira) = visiveis.first() {
                    if esquerda.is_some() && primeira != esquerda.unwrap_or(i) {
                        opcoes.push(Some((tr!("Mover para o início").into(), Msg::MoverAba(i, primeira))));
                    }
                }
                if let Some(&ultima) = visiveis.last() {
                    if direita.is_some() && ultima != direita.unwrap_or(i) {
                        opcoes.push(Some((tr!("Mover para o fim").into(), Msg::MoverAba(i, ultima))));
                    }
                }
                if visiveis.len() > 1 {
                    opcoes.push(Some((tr!("Ocultar aba").into(), Msg::OcultarAba(i))));
                }
                if !self.workbook.abas_ocultas().is_empty() {
                    opcoes.push(Some((tr!("Reexibir abas…").into(), Msg::MostrarAbasOcultas)));
                }
                if self.workbook.len() > 1 && visiveis.len() > 1 {
                    opcoes.push(Some((tr!("Excluir aba").into(), Msg::RemoverAba)));
                }
            }
        }
        opcoes
    }

    /// A célula ativa está na área de resultado de uma tabela dinâmica
    /// (mesma regra de `dinamica_ui::edicao::localizar_selecionada`).
    fn dinamica_sob_celula(&self) -> bool {
        let Some(aba) = self.workbook.aba(self.folha) else { return false };
        let Some(texto) = aba.sheet.dn.as_deref() else { return false };
        let Ok(definicoes) = serde_json::from_str::<Vec<serde_json::Value>>(texto) else { return false };
        let (linha, coluna) = (u64::from(self.linha), u64::from(self.coluna));
        definicoes.iter().any(|d| {
            let area = &d["areaResultado"];
            let (Some(r0), Some(r1), Some(c0), Some(c1)) = (
                area["r0"].as_u64(), area["r1"].as_u64(), area["c0"].as_u64(), area["c1"].as_u64(),
            ) else { return false };
            (r0..=r1).contains(&linha) && (c0..=c1).contains(&coluna)
        })
    }

    /// Itens de um submenu do menu de contexto (Colar especial ▸, Inserir ▸):
    /// as mesmas ações que ficavam soltas no menu.
    pub(super) fn submenu_contexto(&self, sub: u8) -> Vec<menu_contexto::ItemMenu> {
        use menu_contexto::ItemMenu as I;
        let x = |m: Msg| Msg::ContextoExecutar(Box::new(m));
        match sub {
            SUB_COLAR => vec![
                I::acao(tr!("Colar somente valores"), "clipboard", x(Msg::ColarValoresSolicitar)),
                I::acao(tr!("Colar valores com formatação"), "clipboard", x(Msg::ColarEspecialSolicitar(ModoColagem::ValoresComFormato))),
                I::acao(tr!("Colar fórmulas sem formatação"), "math-function", x(Msg::ColarEspecialSolicitar(ModoColagem::FormulasSemFormato))),
                I::acao(tr!("Colar fórmulas com formatação"), "math-function", x(Msg::ColarSolicitar)),
                I::acao(tr!("Colar transposto"), "arrows-horizontal", x(Msg::ColarEspecialSolicitar(ModoColagem::Transposta))),
                I::acao(tr!("Colar somente formatação"), "brush", x(Msg::ColarFormatoSolicitar)),
            ],
            _ => vec![
                I::acao(tr!("Linha acima"), "table", x(Msg::InserirMenu(Estrutura::InserirLinha, false))),
                I::acao(tr!("Linha abaixo"), "table", x(Msg::InserirMenu(Estrutura::InserirLinha, true))),
                I::acao(tr!("Coluna à esquerda"), "columns", x(Msg::InserirMenu(Estrutura::InserirColuna, false))),
                I::acao(tr!("Coluna à direita"), "columns", x(Msg::InserirMenu(Estrutura::InserirColuna, true))),
                I::Separador,
                I::acao(tr!("Células (deslocar para baixo ou para a direita)…"), "layout-grid", x(Msg::Celulas(celulas_dialogo::Evento::Abrir(true)))),
            ],
        }
    }

    /// O submenu aberto do menu de contexto, ao lado do item, na mesma
    /// camada do menu (coordenadas da área da grade).
    pub(super) fn view_submenu_contexto(&self, tipo: Contexto, x: f32, y: f32) -> Option<Element<'_, Msg>> {
        let sub = self.contexto_sub?;
        let opcoes = self.opcoes_contexto(tipo);
        let indice = opcoes.iter().position(|o| matches!(o, Some((_, Msg::ContextoSub(Some(k)))) if *k == sub))?;
        let topo: f32 = 5.0 + opcoes[..indice].iter()
            .map(|o| if o.is_some() { menu_contexto::ALTURA_ITEM } else { menu_contexto::ALTURA_SEPARADOR }).sum::<f32>();
        let itens = self.submenu_contexto(sub);
        let largura = self.contexto_tamanho.0;
        let sx = if x + 2.0 * largura + 8.0 <= self.tamanho_principal.width { x + largura - 4.0 } else { (x - menu_contexto::LARGURA + 4.0).max(0.0) };
        let lista = menu_contexto::lista(&itens, None, move |_| Msg::ContextoSub(Some(sub)), menu_contexto::LARGURA);
        Some(column![
            Space::new(0.0, Length::Fixed(y + topo)),
            row![Space::new(Length::Fixed(sx), 0.0), pelicano::opaque(lista)],
        ].into())
    }

    pub(super) fn view_contexto(&self, tipo: Contexto, (largura, altura): (f32, f32)) -> Element<'_, Msg> {
        if matches!(tipo, Contexto::Aba(_)) && self.paleta_aba_aberta {
            let mut esquerda = column(Vec::new()).spacing(2);
            let mut direita = column(Vec::new()).spacing(2);
            for (i, (nome, hex)) in cores_aba::PALETA.iter().enumerate() {
                let cor = cores_aba::cor_css(hex).unwrap_or(pelicano::Color::BLACK);
                let item = button(row![
                    text("●").size(17).color(cor),
                    text(tr!(*nome)).size(13),
                ].align(pelicano::Align::Center).spacing(4))
                    .padding([5, 6]).width(Length::FILL)
                    .style(estilo_item_menu(false, true))
                    .on_press(Msg::PintarAba(Some(*hex)));
                if i % 2 == 0 { esquerda = esquerda.push(item); }
                else { direita = direita.push(item); }
            }
            let lista = column![
                button(text(tr!("← Voltar")).size(12))
                    .on_press(Msg::VoltarMenuAba)
                    .style(estilo_item_menu(false, true)),
                row![esquerda.width(Length::FILL), direita.width(Length::FILL)].spacing(2),
                button(text(tr!("Sem cor")).size(12))
                    .on_press(Msg::PintarAba(None))
                    .style(estilo_item_menu(false, true)),
            ].padding(5).spacing(3);
            return pelicano::opaque(container(scrollable(lista).height(Length::Fixed(altura)))
                .width(Length::Fixed(largura)).style(estilo_popup_menu)).into();
        }
        let mut lista = column(Vec::new()).padding(5).spacing(2);
        for (i, opcao) in self.opcoes_contexto(tipo).into_iter().enumerate() {
            if let Some((rotulo, msg)) = opcao {
                // Item com submenu: seta ▸ (ícone), abre ao passar o mouse ou clicar.
                let sub = match msg { Msg::ContextoSub(Some(k)) => Some(k), _ => None };
                let mut linha = row![
                    feather_ui::icones::icone(Self::icone_menu(&rotulo), 16.0),
                    text(rotulo).size(13.5),
                ].spacing(8).align(pelicano::Align::Center);
                if sub.is_some() {
                    linha = linha.push(Space::new(Length::FILL, 0.0))
                        .push(feather_ui::icones::icone("chevron-right", 14.0));
                }
                let botao = button(linha).padding([7, 10]).width(Length::FILL)
                    .style(estilo_item_menu(sub.is_some() && sub == self.contexto_sub, true))
                    .on_press(Msg::ContextoEscolher(i));
                lista = lista.push(pelicano::mouse_area(botao).on_enter(Msg::ContextoSub(sub)));
            } else {
                lista = lista.push(container(container(Space::new(0.0, 1))
                    .width(Length::FILL).style(estilo_linha)).padding([1, 0]));
            }
        }
        // Em um viewport menor que o menu padrão, mantém a altura/largura
        // disponíveis e permite scroll interno, sem invadir a barra fx/tabs.
        // `opaque`: a roda no fim da lista não vaza para a grade (#105).
        pelicano::opaque(container(scrollable(lista).height(Length::Fixed(altura)))
            .width(Length::Fixed(largura)).style(estilo_popup_menu)).into()
    }

    /// Menus clássicos (Arquivo, Editar…) mostrados no lugar das abas com a faixa recolhida.
    /// Passar o mouse troca de menu só quando algum já está aberto, como no Excel 2003.
    pub(super) fn barra_menu_classico(&self) -> Element<'_, Msg> {
        let dados = menu_nativo::catalogo();
        let mut categorias = row(Vec::new()).spacing(2).align(pelicano::Align::Center);
        for (i, menu) in dados.menus.iter().enumerate() {
            let aberto = self.barra_menu_aberta && self.menu_grupo == i;
            let botao = button(text(tr!(&menu.titulo).to_uppercase()).size(11.5)
                    .align_x(pelicano::Align::Center).width(Length::FILL))
                .height(Length::Fixed(22.0))
                .width(Length::Fixed(LARGURA_TITULO_MENU))
                .padding([0, 5])
                .style(estilo_titulo_menu(aberto))
                .on_press(Msg::MenuGrupo(i));
            categorias = categorias.push(if self.barra_menu_aberta {
                pelicano::mouse_area(botao).on_enter(Msg::MenuGrupo(i)).into()
            } else { Element::from(botao) });
        }
        categorias.into()
    }

    /// Lista suspensa do menu clássico aberto, com submenus, Voltar e teclado.
    pub(super) fn painel_menu_classico(&self) -> Element<'_, Msg> {
        let dados = menu_nativo::catalogo();
        let mut conteudo = column(Vec::new()).spacing(0).padding([7, 6]);
        if !self.menu_caminho.is_empty() {
            let nivel = &self.menu_caminho;
            let mut trilha = dados.menus.get(self.menu_grupo)
                .map(|m| tr!(&m.titulo).to_owned()).unwrap_or_default();
            let mut itens = dados.menus.get(self.menu_grupo)
                .map(|m| m.itens.as_slice()).unwrap_or(&[]);
            for &i in nivel {
                if let Some(item) = itens.get(i) {
                    trilha.push_str(" › ");
                    trilha.push_str(tr!(&item.rotulo));
                    itens = &item.filhos;
                }
            }
            conteudo = conteudo.push(
                button(row![
                    text("‹").size(17),
                    text(tr!("Voltar")).size(12.5),
                    Space::new(Length::FILL, 0.0),
                    text(trilha).size(10).color(COR_TEXTO_MUTED()),
                ].spacing(7).align(pelicano::Align::Center))
                    .width(Length::FILL)
                    .height(Length::Fixed(32.0))
                    .padding([5, 8])
                    .style(estilo_item_menu(false, true))
                    .on_press(Msg::MenuVoltar)
            );
        }

        let nivel = menu_nativo::itens(self.menu_grupo, &self.menu_caminho).unwrap_or(&[]);
        for (indice, item) in nivel.iter().enumerate() {
            if item.separador {
                conteudo = conteudo.push(
                    container(container(Space::new(0.0, Length::Fixed(1.0)))
                        .width(Length::FILL).style(estilo_linha))
                        .padding([6, 0])
                );
                continue;
            }
            let disponivel = self.item_menu_disponivel(item);
            let marcado = self.item_menu_marcado(item);
            let mut linha = row(Vec::new()).spacing(12).align(pelicano::Align::Center);
            if marcado {
                linha = linha.push(
                    container(text("✓").size(13).color(COR_AZUL_ACENTO()))
                        .width(Length::Fixed(16.0))
                );
            } else if !item.icone.is_empty() {
                linha = linha.push(
                    feather_ui::icones::icone(menu_nativo::icone_tabler(&item.icone), 16.0)
                );
            } else {
                linha = linha.push(Space::new(Length::Fixed(16.0), Length::Fixed(16.0)));
            }
            linha = linha.push(text(tr!(&item.rotulo)).size(13.5))
                .push(Space::new(Length::FILL, 0.0));
            if !item.atalho.is_empty() {
                linha = linha.push(text(&item.atalho).size(12).color(COR_TEXTO_MUTED()));
            }
            if !item.filhos.is_empty() {
                linha = linha.push(text("▸").size(11).color(COR_TEXTO_MUTED()));
            }
            conteudo = conteudo.push(
                button(linha)
                    .height(Length::Fixed(32.0))
                    .width(Length::FILL)
                    .padding([5, 8])
                    .style(estilo_item_menu(disponivel && self.menu_cursor == indice, disponivel))
                    .on_press_maybe(disponivel.then_some(Msg::MenuEscolher(indice)))
            );
        }

        // Painéis curtos usam somente sua altura natural; rolagem aparece
        // apenas quando a lista não cabe no espaço abaixo da barra.
        let altura_natural: f32 = nivel.iter().map(|item| {
            if item.separador { 13.0 } else { 32.0 }
        }).sum::<f32>() + 14.0
            + if self.menu_caminho.is_empty() { 0.0 } else { 32.0 };
        let altura_disponivel = (self.tamanho_principal.height - 300.0).max(150.0);
        let altura = altura_natural.min(altura_disponivel);
        let painel = container(scrollable(conteudo)
            .height(Length::Fixed(altura)))
            .width(Length::Fixed(LARGURA_MENU))
            .style(estilo_popup_menu);

        // A posição acompanha a categoria (depois do botão Menu) e vira para a
        // esquerda se o menu encostaria na borda direita, como fitPanel() no TypeScript.
        let x = (8.0 + LARGURA_BOTAO_MENU + 4.0 + self.menu_grupo as f32 * (LARGURA_TITULO_MENU + 2.0))
            .min((self.tamanho_principal.width - LARGURA_MENU - 8.0).max(0.0));
        row![Space::new(Length::Fixed(x), 0.0), painel].into()
    }

    /// Marca visualmente opções de estado já refletidas pela grade/configuração.
    pub(super) fn item_menu_marcado(&self, item: &menu_nativo::Item) -> bool {
        let Some(acao) = &item.acao else { return false };
        match acao.id.as_str() {
            "toggleGridlines" => self.linhas_grade,
            "toggleRealceEixos" => self.config.realce_eixos,
            "toggleFormulaBar" => self.mostrar_fx,
            "toggleShowFormulas" => self.mostrar_formulas,
            "toggleShowPrintArea" => self.quebras_mostradas(),
            "setTheme" => acao.args.first().and_then(serde_json::Value::as_str)
                == Some(if self.config.tema_auto { "auto" }
                    else if self.config.escuro { "dark" } else { "light" }),
            "setZoom" => acao.args.first().and_then(serde_json::Value::as_f64)
                .is_some_and(|z| (z * 100. - f64::from(self.config.zoom)).abs() < 0.1),
            _ => false,
        }
    }

    pub(super) fn item_menu_disponivel(&self, item: &menu_nativo::Item) -> bool {
        !item.separador && (!item.filhos.is_empty()
            || item.acao.as_ref().and_then(|a| self.evento_menu(a)).is_some())
    }

    /// Converte IDs e parâmetros do TypeScript em mensagens do motor nativo.
    /// Ações sem equivalente ficam desabilitadas, em vez de virar botões inertes.
    pub(super) fn evento_menu(&self, acao: &menu_nativo::Acao) -> Option<Msg> {
        let arg = acao.args.first();
        let texto = arg.and_then(serde_json::Value::as_str).unwrap_or("");
        Some(match acao.id.as_str() {
            "newWorkbook" => Msg::Novo,
            "insertChart" => Msg::Grafico(graficos::Evento::AbrirNoCentro),
            "openFile" => Msg::Abrir,
            "save" => Msg::Salvar,
            "saveAs" => Msg::SalvarComo,
            "makeCopy" => Msg::MenuSalvarCopia,
            "exportar" if texto == "fshz" => Msg::MenuSalvarCopia,
            "exportar" if texto == "xlsx" => Msg::ExportarXlsx,
            "exportar" if texto == "json" => Msg::ExportarJson,
            "exportar" if texto == "csv" => Msg::ExportarDelimitado(false),
            "exportar" if texto == "tsv" => Msg::ExportarDelimitado(true),
            "undo" => Msg::Desfazer,
            "redo" => Msg::Refazer,
            "clipboard" => match texto {
                "copy" => Msg::Copiar, "cut" => Msg::Recortar,
                "paste" => Msg::ColarSolicitar, _ => return None,
            },
            "pasteValues" => Msg::ColarValoresSolicitar,
            "pasteFormats" => Msg::ColarFormatoSolicitar,
            "pasteTranspose" => Msg::ColarEspecialSolicitar(ModoColagem::Transposta),
            "pasteValuesFormats" => Msg::ColarEspecialSolicitar(ModoColagem::ValoresComFormato),
            "pasteFormulas" => Msg::ColarEspecialSolicitar(ModoColagem::FormulasSemFormato),
            "pasteFormulasFormats" => Msg::ColarEspecialSolicitar(ModoColagem::Completa),
            "toggleFormulaBar" => Msg::AlternarBarraFx,
            "toggleShowFormulas" => Msg::AlternarMostrarFormulas,
            "print" => Msg::Impressao(EventoImpressao::Abrir),
            "setPrintArea" => Msg::Impressao(EventoImpressao::DefinirArea),
            "clearPrintArea" => Msg::Impressao(EventoImpressao::LimparArea),
            "toggleShowPrintArea" => Msg::Impressao(EventoImpressao::AlternarQuebras),
            "toggleRealceEixos" => Msg::RealceEixos(!self.config.realce_eixos),
            "insertImage" => Msg::ImagemLaboratorioAbrir,
            "createTable" => Msg::TabelaAbrir,
            "clearFormat" => Msg::LimparFormato,
            "clearAll" => Msg::LimparTudo,
            "clearComments" => Msg::Comentario(comentarios_ui::Evento::Limpar),
            "clearHyperlinks" => Msg::LimparHiperlinks,
            "reapply" => Msg::TabelaAcao(tabelas::Acao::Reaplicar),
            "indent" => Msg::AlterarRecuo(arg?.as_i64()?.clamp(-15, 15) as i32),
            "drawBorder" => Msg::BordaDesenho(desenho_borda_ui::Evento::Modo(match texto {
                "grade" => feathersheet_core::operacoes::desenho_borda::ModoBorda::Grade,
                "apagar" => feathersheet_core::operacoes::desenho_borda::ModoBorda::Apagar,
                _ => feathersheet_core::operacoes::desenho_borda::ModoBorda::Contorno,
            })),
            "autoFitColumn" => Msg::AjustarLargura,
            "alternatingColors" => Msg::CondicionalCoresAlternadas,
            "openFunctions" => Msg::MostrarCatalogoFuncoes,
            "showAbout" => Msg::MostrarSobre,
            "condicional" => match texto {
                "nova" | "gerenciar-planilha" | "gerenciar-selecao" =>
                    Msg::CondicionalMostrar,
                "limpar-selecao" => Msg::CondicionalLimparSelecao,
                "limpar-planilha" => Msg::CondicionalLimparTudo,
                "realcar:greaterThan" => Msg::CondicionalPredefinida(TipoCondicional::Maior),
                "realcar:lessThan" => Msg::CondicionalPredefinida(TipoCondicional::Menor),
                "realcar:between" => Msg::CondicionalPredefinida(TipoCondicional::Entre),
                "realcar:equal" => Msg::CondicionalPredefinida(TipoCondicional::Igual),
                "realcar:textContains" => Msg::CondicionalPredefinida(TipoCondicional::Contem),
                "realcar:duplicate" => Msg::CondicionalPredefinida(TipoCondicional::Duplicado),
                "realcar:unique" => Msg::CondicionalPredefinida(TipoCondicional::Unico),
                "realcar:isEmpty" => Msg::CondicionalPredefinida(TipoCondicional::Vazia),
                "realcar:aboveAverage" => Msg::CondicionalPredefinida(TipoCondicional::AcimaMedia),
                "realcar:belowAverage" => Msg::CondicionalPredefinida(TipoCondicional::AbaixoMedia),
                // Pintar a linha inteira é uma regra por fórmula (=$A1>10).
                "linha" => Msg::CondicionalPredefinida(TipoCondicional::Formula),
                "barra" => Msg::CondicionalPredefinida(TipoCondicional::Barra),
                "escala" => Msg::CondicionalPredefinida(TipoCondicional::Escala),
                _ => return None,
            },
            "clearContents" => Msg::Limpar,
            "deleteRows" => Msg::Estrutura(Estrutura::ExcluirLinha),
            "deleteColumns" => Msg::Estrutura(Estrutura::ExcluirColuna),
            "findReplace" => Msg::BuscaAba(true),
            "goTo" => Msg::IrParaJanela(ir_para::Evento::Abrir),
            "customSort" => Msg::Classificar(classificar::Evento::Abrir),
            "novoModulo" => Msg::MacroNovoModulo,
            "novoComponente" => Msg::MacroNovoFormulario,
            "insertMacroButton" => Msg::BotaoMacroNovo,
            "openMacroHelp" => Msg::AjudaAbrirSecao("macros"),
            "goalSeek" => Msg::AtingirMeta(atingir_meta::Evento::Abrir),
            "dataTable" => Msg::TabelaDados(tabela_dados::Evento::Abrir),
            "scenarios" => Msg::Cenarios(cenarios_ui::Evento::Abrir),
            "toggleGridlines" => Msg::LinhasGrade(!self.linhas_grade),
            "freeze" => {
                let linhas = acao.args.first().and_then(serde_json::Value::as_u64);
                let colunas = acao.args.get(1).and_then(serde_json::Value::as_u64);
                let codigo = match (linhas, colunas) {
                    (Some(0), None) => 4,
                    (Some(1), None) => 1,
                    (Some(2), None) => 5,
                    (None, Some(0)) => 6,
                    (None, Some(1)) => 2,
                    (None, Some(2)) => 7,
                    _ => return None,
                };
                Msg::CongelarAplicar(codigo)
            },
            "freezeCurrentRow" => Msg::CongelarAplicar(8),
            "freezeCurrentCol" => Msg::CongelarAplicar(9),
            "setZoom" => Msg::Zoom((arg?.as_f64()? * 100.0).round() as u16),
            "insertRows" => Msg::InserirMenu(Estrutura::InserirLinha, texto == "below"),
            "insertColumns" => Msg::InserirMenu(Estrutura::InserirColuna, texto == "right"),
            "addSheet" => Msg::AdicionarAba,
            "insertFunction" if matches!(texto, "SOMA" | "MÉDIA") =>
                Msg::InserirFuncao(texto.to_owned()),
            "toggleText" if matches!(texto, "bold" | "italic" | "underline" | "doubleUnderline" | "strike") =>
                Msg::AlternarEstilo(texto.to_owned()),
            "setFormat" if matches!(texto, "auto" | "number" | "integer" | "currency" |
                "percent" | "date" | "datetime" | "time" | "text") =>
                Msg::EstiloMenu("format".into(), if texto == "auto" { Campo::Nulo }
                    else { Campo::Texto(texto.into()) }),
            "setAlign" if matches!(texto, "left" | "center" | "right") =>
                Msg::EstiloMenu("align".into(), Campo::Texto(texto.into())),
            "setVAlign" if matches!(texto, "top" | "middle" | "bottom") =>
                Msg::EstiloMenu("vAlign".into(), Campo::Texto(texto.into())),
            "setWrap" => Msg::EstiloMenu("wrap".into(), Campo::Logico(arg?.as_bool()?)),
            "setFontSize" => Msg::EstiloMenu("fontSize".into(), Campo::Numero(arg?.as_f64()?)),
            "setTheme" if texto == "dark" || texto == "light" => Msg::Tema(texto == "dark"),
            "setTheme" if texto == "auto" => Msg::TemaAuto,
            // "sheet" era a antiga Classificar página: hoje A–Z já expande para a região.
            "sort" =>
                Msg::Ordenar(acao.args.get(1)?.as_bool()?),
            // Msg::Mesclar alterna: só serve ao item certo conforme a seleção já mesclada.
            "mesclar" | "desmesclar" => {
                let area = self.area_selecionada();
                let mesclada = self.workbook.aba(self.folha).is_some_and(|aba|
                    mesclas::exatamente_mesclada(&aba.sheet, &area));
                if mesclada == (acao.id == "desmesclar") { Msg::Mesclar } else { return None }
            },
            "toggleFilter" => Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro),
            "splitText" => Msg::AbrirAssistenteDivisao,
            "createPivotTable" => Msg::Dinamica(dinamica_ui::Evento::Abrir),
            "home" => Msg::MenuPagina(menu_principal::Pagina::Inicio),
            "columnStats" => Msg::EstatisticasAbrir,
            "editarLink" => Msg::LinkEditar,
            "removerLink" => Msg::LinkRemover,
            "abrirLink" => Msg::LinkAbrir,
            "nameSelection" => Msg::NomesAbrir(true),
            "manageNames" => Msg::NomesAbrir(false),
            "openOpenQuery" => Msg::OpenQuery(EventoOpenQuery::Abrir),
            "explainError" | "explainFormulaError" => Msg::ExplicacaoErroAbrir,
            "dataValidation" | "validation" | "openValidation" => Msg::ValidacaoAbrir,
            "removeDuplicates" => Msg::RemoverDuplicados,
            "trimSpaces" => Msg::ApararEspacos,
            "openMacros" => Msg::Macros,
            "openMemoryProfiler" => Msg::Monitor,
            "renameSheet" => Msg::EditarNomeAba,
            "deleteSheet" if self.workbook.len() > 1 => Msg::RemoverAba,
            "openSettings" => Msg::Configuracoes,
            "openHelp" | "showShortcuts" => Msg::Ajuda,
            "openPrimeirosPassos" => Msg::PrimeirosPassosAbrir,
            "openNovidades" => Msg::NovidadesAbrir,
            "fullscreen" => Msg::TelaInteira,
            "renameFile" => Msg::RenomearArquivo,
            "duplicateSheet" => Msg::DuplicarAba,
            _ => return None,
        })
    }

}

#[cfg(test)]
mod testes_idioma {
    use super::*;

    /// Com en-US, nenhum item dos menus de contexto (célula, linha, coluna,
    /// aba) e nenhum nome de aba da faixa fica em português.
    #[test]
    fn menus_de_contexto_e_abas_da_faixa_ficam_em_ingles() {
        crate::idioma::definir_na_thread("en-US");
        let app = App::default();
        let mut faltam = Vec::new();
        for tipo in [Contexto::Celula(0, 0), Contexto::Linha(0), Contexto::Coluna(0), Contexto::Aba(0)] {
            for (rotulo, _) in app.opcoes_contexto(tipo).into_iter().flatten() {
                if crate::idioma::dicionario().contains_key(&rotulo) {
                    faltam.push(rotulo);
                }
            }
        }
        for pt in ["Página Inicial", "Inserir", "Desenhar", "Layout da Página", "Fórmulas",
            "Dados", "Revisão", "Exibir", "Ferramentas", "Design da Tabela"] {
            if tr!(pt) == pt { faltam.push(pt.to_owned()); }
        }
        crate::idioma::definir_na_thread("pt-BR");
        assert!(faltam.is_empty(), "ainda em português no en-US: {faltam:?}");
    }
}
