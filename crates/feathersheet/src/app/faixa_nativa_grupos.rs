//! Montagem dos grupos da Ribbon e renderização do menu de ferramentas extras.
use super::*;
use pelicano::column;
use feather_ui::componentes as ui;
use super::faixa_responsiva::{self, Nivel};
use super::faixa_nativa::*;

impl App {
    /// A lista de grupos é única. Ribbon e menu Mais usam os mesmos widgets,
    /// sem duplicar nem reduzir funcionalidade de controles ocultos.
    pub(super) fn montar_grupos_faixa(&self) -> Vec<Element<'_, Msg>> {
        let estilo = self.faixa_estilo();
        let mut grupos: Vec<Element<'_, Msg>> = Vec::new();
        match self.faixa_ativa {
            AbaFaixa::Inicio => {
                grupos.push(self.faixa_grupo(tr!("Área de transferência"), self.faixa_linha(vec![
                    self.faixa_botao_colar(),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno_colorido(tr!("Recortar"), "cut", "#d68126", Some(Msg::Recortar), false),
                        self.faixa_pequeno_colorido(tr!("Copiar"), "copy", "#2179a7", Some(Msg::Copiar), false),
                        self.faixa_pequeno_colorido(tr!("Pincel"), "brush", "#7d5cc6",
                            Some(Msg::Pincel), self.pincel_formatacao.is_some()),
                    ]),
                ])));
                let tamanho = estilo.font_size.unwrap_or(13.).round().clamp(6., 96.) as i32;
                // Fonte compacta, em duas linhas na ordem de trabalho:
                // [família] [tamanho] [A+] [A−]
                // [B] [I] [Sublinhado] [Bordas ▾] [Balde/cor] [A/cor].
                // Tachado continua disponível no fim para não perder a função.
                let fonte_superior = row![
                    self.faixa_lista(tr!("Fonte"), "fonte", FONTES, Some(self.faixa_nome_fonte()), 126.),
                    self.faixa_lista(tr!("Tamanho"), "tamanho", TAMANHOS,
                        TAMANHOS.iter().copied().find(|v| v.parse::<i32>().ok() == Some(tamanho)), 68.),
                    self.faixa_tamanho_acao("A+", tr!("Aumentar tamanho da fonte"), 1),
                    self.faixa_tamanho_acao("A−", tr!("Diminuir tamanho da fonte"), -1),
                ]
                .spacing(5)
                .align(pelicano::Align::Center);
                let fonte_inferior = row![
                    self.faixa_icone(tr!("Negrito (Ctrl+B)"), "bold", Some(Msg::AlternarEstilo("bold".into())), estilo.bold),
                    self.faixa_icone(tr!("Itálico"), "italic", Some(Msg::AlternarEstilo("italic".into())), estilo.italic),
                    self.faixa_sublinhado(estilo.underline),
                    self.faixa_bordas(),
                    self.faixa_cor(AlvoCor::Fundo),
                    self.faixa_cor(AlvoCor::Texto),
                    self.faixa_icone(tr!("Tachado"), "strikethrough", Some(Msg::AlternarEstilo("strike".into())), estilo.strike),
                ]
                .spacing(4)
                .align(pelicano::Align::Center);
                grupos.push(ui::grupo_faixa_compacto(
                    column![fonte_superior, fonte_inferior].spacing(6).into(),
                    tr!("Fonte"),
                ));
                grupos.push(self.faixa_grupo(tr!("Alinhamento"), self.faixa_linha(vec![
                    self.faixa_pilha(vec![
                        feather_ui::alinhamento::seletor_horizontal(
                            match estilo.align {
                                feathersheet_core::celulas::style::Align::Auto => None,
                                feathersheet_core::celulas::style::Align::Left =>
                                    Some(feather_ui::alinhamento::Horizontal::Esquerda),
                                feathersheet_core::celulas::style::Align::Center =>
                                    Some(feather_ui::alinhamento::Horizontal::Centro),
                                feathersheet_core::celulas::style::Align::Right =>
                                    Some(feather_ui::alinhamento::Horizontal::Direita),
                            },
                            true,
                            false,
                            |escolha| Msg::EstiloMenu(
                                "align".into(),
                                Campo::Texto(match escolha {
                                    feather_ui::alinhamento::Horizontal::Esquerda => "left",
                                    feather_ui::alinhamento::Horizontal::Centro => "center",
                                    feather_ui::alinhamento::Horizontal::Direita => "right",
                                    // A faixa de células expõe apenas três botões;
                                    // nunca mapear Justificado erroneamente para Esquerda.
                                    feather_ui::alinhamento::Horizontal::Justificado => "justify",
                                }.into()),
                            ),
                        ),
                        self.faixa_linha(vec![
                            self.faixa_vertical("top", estilo.valign == feathersheet_core::celulas::style::VAlign::Top),
                            self.faixa_vertical("middle", estilo.valign == feathersheet_core::celulas::style::VAlign::Middle),
                            self.faixa_vertical("bottom", estilo.valign == feathersheet_core::celulas::style::VAlign::Bottom),
                            self.faixa_icone(tr!("Diminuir recuo"), "indent-decrease", Some(Msg::AlterarRecuo(-1)), false),
                            self.faixa_icone(tr!("Aumentar recuo"), "indent-increase", Some(Msg::AlterarRecuo(1)), false),
                        ]),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_inclinar_texto(estilo.rotation.unwrap_or(0) != 0),
                        self.faixa_pequeno(tr!("Quebrar"), "text-wrap",
                            Some(Msg::EstiloMenu("wrap".into(), Campo::Logico(!estilo.wrap))), estilo.wrap),
                    ]),
                    self.faixa_mesclar_dividido(
                        self.workbook.aba(self.folha).is_some_and(|aba|
                            mesclas::exatamente_mesclada(&aba.sheet, &self.area_selecionada()))),
                ])));
                // Número como no mockup: formato em cima; cinco ações compactas abaixo.
                // Cada botão conserva o comando original de formatação da planilha.
                let numero_inferior = row![
                    self.faixa_numero_botao("R$", tr!("Formato de moeda"),
                        Msg::EstiloMenu("format".into(), Campo::Texto("currency".into())),
                        estilo.format == feathersheet_core::celulas::style::NumberFormat::Currency),
                    self.faixa_numero_botao("%", tr!("Formato percentual"),
                        Msg::EstiloMenu("format".into(), Campo::Texto("percent".into())),
                        estilo.format == feathersheet_core::celulas::style::NumberFormat::Percent),
                    self.faixa_numero_botao(".000", tr!("Formato numérico"),
                        Msg::EstiloMenu("format".into(), Campo::Texto("number".into())),
                        estilo.format == feathersheet_core::celulas::style::NumberFormat::Number),
                    self.faixa_numero_botao(".0←", tr!("Diminuir casas decimais"),
                        Msg::Decimais(-1), false),
                    self.faixa_numero_botao(".0→", tr!("Aumentar casas decimais"),
                        Msg::Decimais(1), false),
                ].spacing(1).align(pelicano::Align::Center);
                grupos.push(self.faixa_grupo(NUM_FORMAT,
                    column![
                        self.faixa_lista(tr!("Formato"), "formato", FORMATOS,
                            Some(self.faixa_nome_formato()), 173.),
                        numero_inferior,
                    ].spacing(10).into(),
                ));
                // Ícones de destaque seguem a identidade de cor do mockup.
                // Não há ações fictícias: cada botão aponta para o comando já existente.
                grupos.push(self.faixa_grupo(tr!("Estilos"), self.faixa_linha(vec![
                    self.faixa_grande_destaque(
                        tr!("Formatação\ncondicional"), "palette", "#7d5cc6",
                        (238, 232, 252), Some(Msg::CondicionalAbrir), self.condicional_menu_aberto,
                    ),
                    button(column![
                        feather_ui::icones::icone("table", 25.),
                        row![
                            text(tr!("Tabela")).size(11),
                            feather_ui::icones::icone("chevron-down", 12.0),
                        ].spacing(3).align(pelicano::Align::Center),
                    ].spacing(3).align(pelicano::Align::Center))
                        .padding([4, 8])
                        .style(estilo_botao_grande(self.tabelas_menu_aberto))
                        .on_press(Msg::AlternarMenuTabelas).into(),
                    button(column![
                        feather_ui::icones::icone("cell-styles", 25.),
                        text(tr!("Estilos")).size(11),
                        row![
                            text(tr!("de Célula")).size(11),
                            feather_ui::icones::icone("chevron-down", 12.0),
                        ].spacing(3).align(pelicano::Align::Center),
                    ].spacing(2).align(pelicano::Align::Center))
                        .padding([3, 8])
                        .style(estilo_botao_grande(self.estilos_menu_aberto))
                        .on_press(Msg::AlternarMenuEstilos).into(),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno_colorido(
                            tr!("Cores alternadas"), "alternating-colors", "#7d5cc6",
                            Some(Msg::CondicionalCoresAlternadas), false,
                        ),
                        self.faixa_pequeno(tr!("Limpar formato"), "eraser",
                            Some(Msg::LimparFormato), false),
                    ]),
                ])));
                // Operações estruturais não têm falsas setas de submenu:
                // a legenda explica a ação executada ao clicar.
                grupos.push(self.faixa_grupo(tr!("Células"), self.faixa_linha(vec![
                    self.faixa_com_seta(self.faixa_grande_destaque(
                        "Inserir\nlinha", "plus", "#21865c",
                        (222, 245, 233),
                        Some(Msg::InserirMenu(Estrutura::InserirLinha, false)), false,
                    ), "inserir"),
                    self.faixa_com_seta(self.faixa_grande_destaque(
                        "Excluir\nlinha", "trash", "#ca5555",
                        (253, 232, 232),
                        Some(Msg::Estrutura(Estrutura::ExcluirLinha)), false,
                    ), "excluir"),
                    self.faixa_formatar_celulas_menu(),
                ])));
                // Edição: Σ AutoSoma/Preencher à esquerda e ações grandes à direita.
                // Os ícones de seta só abrem menus suspensos reais.
                grupos.push(self.faixa_grupo(tr!("Edição"), self.faixa_linha(vec![
                    self.faixa_pilha(vec![
                        self.faixa_autosoma_compacto(),
                        // O cálculo de séries, fórmulas relativas e estilos já é Rust:
                        // botão dividido executa para baixo; a seta abre os demais modos.
                        self.faixa_preencher_compacto(),
                        self.faixa_limpar_compacto(),
                    ]),
                    self.faixa_classificar_filtrar(),
                    self.faixa_localizar_grande(),
                ])));
            }
                        AbaFaixa::Inserir => {
                // Primeiro a iconografia; comandos existentes seguem funcionais.
                grupos.push(self.faixa_grupo(tr!("Tabelas"), self.faixa_linha(vec![
                    self.faixa_grande_seta(tr!("Tabela dinâmica"), "pivot-table", Some(Msg::Dinamica(dinamica_ui::Evento::Abrir)), self.dinamica.is_some()),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Dinâmicas recomendadas"), "table-suggest",
                            Some(Msg::Dinamica(dinamica_ui::Evento::Recomendadas)), false),
                        self.faixa_pequeno(tr!("Editar dinâmica"), "edit",
                            Some(Msg::Dinamica(dinamica_ui::Evento::Editar)), false),
                        self.faixa_pequeno(tr!("Atualizar dinâmica"), "refresh",
                            Some(Msg::Dinamica(dinamica_ui::Evento::Atualizar)), false),
                    ]),
                    self.faixa_grande(tr!("Tabela"), "table", Some(Msg::TabelaAbrir), self.tabela_area.is_some()),
                ])));
                grupos.push(self.faixa_grupo(tr!("Ilustrações"), self.faixa_linha(vec![
                    self.faixa_grande_seta(tr!("Imagens"), "photo", Some(Msg::ImagemLaboratorioAbrir), false),
                    // Formas: menu flutuante com miniaturas; o arraste na grade cria (objetos.rs).
                    self.faixa_grande_seta(tr!("Formas"), "shapes", Some(Msg::Objetos(super::objetos::Evento::AlternarMenu(super::objetos::ModoMenu::Inserir))),
                        matches!(self.objetos.menu, Some(super::objetos::ModoMenu::Inserir | super::objetos::ModoMenu::Trocar)) || self.objetos.inserir.is_some()),
                    // Ícones: galeria Tabler com busca; um clique insere (objetos.rs).
                    self.faixa_grande(tr!("Ícones"), "icons", Some(Msg::Objetos(super::objetos::Evento::AlternarMenu(super::objetos::ModoMenu::Icones))),
                        self.objetos.menu == Some(super::objetos::ModoMenu::Icones)),
                    em_breve(self.faixa_grande_seta(tr!("Modelos 3D"), "cube", None, false), tr!("Em breve: modelos 3D")),
                    em_breve(self.faixa_grande("SmartArt", "sitemap", None, false), tr!("Em breve: diagramas SmartArt")),
                    em_breve(self.faixa_grande(tr!("Instantâneo"), "screenshot", None, false), tr!("Em breve: captura de outra janela")),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Efeitos da imagem"), "photo", self.workbook.aba(self.folha).and_then(|aba| aba.sheet.im.as_ref()).map(|_| Msg::ImagemEfeitosAbrir), self.efeitos_imagem.aberto),
                        self.faixa_pequeno(tr!("Imagem na célula…"), "layout-grid",
                            Some(Msg::ImagemCelula(super::imagem_na_celula::Evento::Inserir)), false),
                        // Troca de lugar: a da célula sai; a solta selecionada entra.
                        if self.celula_tem_imagem() {
                            self.faixa_pequeno(tr!("Colocar sobre as células"), "arrow-up-right",
                                Some(Msg::ImagemCelula(super::imagem_na_celula::Evento::ColocarSobre)), false)
                        } else {
                            self.faixa_pequeno(tr!("Colocar na célula"), "arrow-down-right",
                                self.imagem_selecionada.is_some().then_some(
                                    Msg::ImagemCelula(super::imagem_na_celula::Evento::ColocarNaCelula)), false)
                        },
                    ]),
                ])));
                grupos.push(self.faixa_grupo(tr!("Controles"), self.faixa_linha(vec![
                    // Caixas de seleção: insere na seleção; aceso = a seleção tem caixa
                    // e o clique remove (caixas_ui.rs).
                    self.faixa_grande(tr!("Checkbox"), "checkbox", Some(Msg::CaixasInserir), self.selecao_tem_caixa()),
                ])));
                grupos.push(self.faixa_grupo(tr!("Gráficos"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Recomendados"), "chart-suggest", Some(Msg::Grafico(graficos::Evento::Rapido(None))), false),
                    self.faixa_grafico(),
                    self.faixa_pilha(vec![
                        self.faixa_linha(vec![
                    self.faixa_icone(tr!("Colunas"), "chart-columns", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Colunas)))), false),
                    self.faixa_icone(tr!("Linhas"), "chart-line", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Linhas)))), false),
                    self.faixa_icone(tr!("Pizza"), "chart-pie", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Pizza)))), false),
                    self.faixa_icone(tr!("Área"), "chart-area", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Area)))), false),
                ]),
                        self.faixa_linha(vec![
                    self.faixa_icone(tr!("Dispersão"), "chart-scatter", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Dispersao)))), false),
                    self.faixa_icone(tr!("Combinado"), "chart-combo", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Combinado)))), false),
                    self.faixa_icone(tr!("Cascata"), "chart-waterfall", Some(Msg::Grafico(graficos::Evento::Rapido(Some(graficos::TipoUi::Cascata)))), false),
                ]),
                    ]),
                    em_breve(self.faixa_grande_seta(tr!("Mapas"), "chart-map", None, false), tr!("Em breve: gráfico de mapa")),
                    self.faixa_grande_seta(tr!("Gráfico dinâmico"), "chart-pivot",
                        Some(Msg::Dinamica(dinamica_ui::Evento::GraficoDinamico)), false),
                ])));
                grupos.push(self.faixa_grupo(tr!("Minigráficos"), self.faixa_linha(vec![
                    em_breve(self.faixa_grande(tr!("Linha"), "sparkline-line", None, false), tr!("Em breve: minigráfico de linha")),
                    em_breve(self.faixa_grande(tr!("Coluna"), "sparkline-columns", None, false), tr!("Em breve: minigráfico de colunas")),
                    em_breve(self.faixa_grande("Ganhos/Perdas", "sparkline-winloss", None, false), tr!("Em breve: minigráfico de ganhos e perdas")),
                ])));
                grupos.push(self.faixa_grupo(tr!("Filtros"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Segmentação de dados"), "slicer",
                        Some(Msg::Dinamica(dinamica_ui::Evento::SegmentacaoInserir)), false),
                    em_breve(self.faixa_grande(tr!("Linha do tempo"), "timeline", None, false), tr!("Em breve: linha do tempo de datas")),
                ])));
                grupos.push(self.faixa_grupo(tr!("Links"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Inserir link"), "link", Some(Msg::LinkEditar),
                        self.link_modal.is_some()),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Abrir link"), "link",
                            self.link_da_celula(self.folha, self.linha, self.coluna)
                                .map(|_| Msg::LinkAbrir), false),
                        self.faixa_pequeno(tr!("Remover link"), "trash",
                            Some(Msg::LinkRemover), false),
                    ]),
                ])));
                grupos.push(self.faixa_grupo(tr!("Comentários"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Comentário"), "message-circle",
                        Some(Msg::Comentario(comentarios_ui::Evento::Novo)), self.comentario_editor.is_some()),
                ])));
                grupos.push(self.faixa_grupo(tr!("Texto"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Caixa de texto"), "text-box", Some(Msg::Objetos(super::objetos::Evento::CaixaTexto)),
                        self.objetos.caixa_texto && self.objetos.inserir.is_some()),
                    self.faixa_grande(tr!("Cabeçalho e rodapé"), "header-footer", Some(Msg::Impressao(EventoImpressao::Abrir)), false),
                    // WordArt: galeria dos estilos comuns; um clique insere (objetos.rs).
                    self.faixa_grande("WordArt", "wordart", Some(Msg::Objetos(super::objetos::Evento::AlternarMenu(super::objetos::ModoMenu::WordArt))),
                        self.objetos.menu == Some(super::objetos::ModoMenu::WordArt)),
                    em_breve(self.faixa_grande(tr!("Linha de assinatura"), "signature", None, false), tr!("Em breve: linha de assinatura")),
                    em_breve(self.faixa_grande(tr!("Objeto"), "object", None, false), tr!("Em breve: inserir objeto de outro programa")),
                ])));
                grupos.push(self.faixa_grupo(tr!("Símbolos"), self.faixa_linha(vec![
                    self.faixa_grande_seta(tr!("Equação"), "equation", Some(Msg::AlternarMenuEdicao("equacao")), self.edicao_menu == Some("equacao")),
                    self.faixa_grande_seta(tr!("Símbolo"), "omega", Some(Msg::AlternarMenuEdicao("simbolo")), self.edicao_menu == Some("simbolo")),
                ])));
                // Comandos já presentes em Inserir: preservados no final.
                grupos.push(self.faixa_grupo(tr!("Células"), self.faixa_pilha(vec![
                        self.faixa_linha(vec![
                    self.faixa_lista(tr!("Linhas"), "linhas", LINHAS, None, 136.),
                    self.faixa_lista(tr!("Colunas"), "colunas", COLUNAS, None, 150.),
                ]),
                        self.faixa_linha(vec![
                    self.faixa_pequeno(tr!("Nova aba"), "file-plus", Some(Msg::AdicionarAba), false),
                    self.faixa_pequeno(tr!("Excluir linha"), "trash", Some(Msg::Estrutura(Estrutura::ExcluirLinha)), false),
                    self.faixa_pequeno(tr!("Excluir coluna"), "trash", Some(Msg::Estrutura(Estrutura::ExcluirColuna)), false),
                ]),
                    ])));
                grupos.push(self.faixa_grupo(tr!("Fórmulas"), self.faixa_linha(vec![
                    self.faixa_lista(tr!("Função"), "funcao", FUNCOES, None, 100.),
                    self.faixa_grande(tr!("Nomear intervalo"), "tag", Some(Msg::NomesAbrir(true)), self.nomes_modal.is_some()),
                    self.faixa_catalogo_funcoes(),
                    self.faixa_grande(tr!("Explicar erro"), "alerta_triangulo", Some(Msg::ExplicacaoErroAbrir), false),
                ])));
            }
            AbaFaixa::Desenhar => grupos.extend(
                feather_ui::tinta::grupos_faixa(&self.tinta.ui).into_iter().map(|g| g.map(Msg::Desenhar)),
            ),
            AbaFaixa::Layout => grupos.extend(self.grupos_layout_pagina()),
            AbaFaixa::Formulas => grupos.extend(self.grupos_formulas()),
            AbaFaixa::Revisao => grupos.extend(self.grupos_revisao()),
            // Contextual: só com forma selecionada (objetos/interface.rs).
            AbaFaixa::EditarForma => grupos.extend(self.grupos_editar_forma()),
            AbaFaixa::Dados => {
                grupos.push(self.faixa_grupo(tr!("Classificar e filtrar"), self.faixa_linha(vec![
                    self.faixa_grande("AutoFiltro", "filter",
                        Some(Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro)), false),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno("A–Z", "sort-ascending", Some(Msg::Ordenar(true)), false),
                        self.faixa_pequeno("Z–A", "sort-descending", Some(Msg::Ordenar(false)), false),
                        self.faixa_lista(tr!("Classificar página"), "ordenar", ORDENAR, None, 140.),
                    ]),
                ])));
                grupos.push(self.faixa_grupo(tr!("Ferramentas de dados"), self.faixa_linha(vec![
                    // Ação principal abre a janela diretamente: sem menu intermediário.
                    self.faixa_grande(tr!("Dividir texto"), "columns",
                        Some(Msg::AbrirAssistenteDivisao), self.assistente_divisao.is_some()),
                    self.faixa_grande(tr!("Duplicados"), "copy", Some(Msg::RemoverDuplicados), false),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Remover espaços"), "eraser", Some(Msg::ApararEspacos), false),
                        self.faixa_pequeno(tr!("Estatísticas"), "chart-bar", Some(Msg::EstatisticasAbrir), self.estatisticas.is_some()),
                        self.faixa_pequeno(tr!("Perfil da coluna"), "chart-bar", Some(Msg::PerfilColunaAbrir), self.perfil_coluna.is_some()),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_validacao(),
                        self.faixa_pequeno(tr!("Nomes"), "tag", Some(Msg::NomesAbrir(false)), self.nomes_modal.is_some()),
                    ]),
                    // Preenchimento relâmpago (Ctrl+E): rótulo curto para a
                    // faixa caber em 1280 px sem espremer o grupo seguinte.
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Relâmpago"), "bolt", Some(Msg::PreenchimentoRelampago), false),
                        self.faixa_pequeno(tr!("Região atual"), "table", Some(Msg::SelecionarRegiaoAtual), false),
                    ]),
                ])));
                grupos.push(self.grupo_estrutura_topicos());
                grupos.push(self.faixa_grupo(tr!("Previsão"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Cenários"), "list-details",
                        Some(Msg::Cenarios(cenarios_ui::Evento::Abrir)), self.cenarios.is_some()),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Atingir meta…"), "math-function",
                            Some(Msg::AtingirMeta(atingir_meta::Evento::Abrir)), self.atingir_meta.is_some()),
                        self.faixa_pequeno(tr!("Tabela de dados…"), "table",
                            Some(Msg::TabelaDados(tabela_dados::Evento::Abrir)), self.tabela_dados.is_some()),
                        // Sem tabela feita nesta sessão, fica desabilitado.
                        self.faixa_pequeno(tr!("Atualizar tabela"), "refresh",
                            self.tabela_dados_ultima.is_some()
                                .then_some(Msg::TabelaDados(tabela_dados::Evento::Atualizar)), false),
                    ]),
                ])));
                // Design na aba contextual; filtros/ordenação na seta do cabeçalho.
                grupos.push(self.faixa_grupo(tr!("Consultas"), self.faixa_linha(vec![
                    self.faixa_grande("OpenQuery", "table", Some(Msg::OpenQuery(EventoOpenQuery::Abrir)), self.openquery.is_some()),
                    self.faixa_grande(tr!("Tabela dinâmica"), "pivot-table", Some(Msg::Dinamica(dinamica_ui::Evento::Abrir)), self.dinamica.is_some()),
                    // Referências a outros arquivos (`=[Vendas.fshz]Dados!A1`, vinculos.rs).
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Atualizar vínculos"), "refresh",
                            self.workbook.tem_vinculos_externos()
                                .then_some(Msg::Vinculos(vinculos::Evento::AtualizarValores(None))), false),
                        self.faixa_pequeno(tr!("Editar links"), "link",
                            Some(Msg::Vinculos(vinculos::Evento::EditarAbrir)), self.vinculos.editar_aberto),
                    ]),
                ])));
            }
            AbaFaixa::Exibir => {
                grupos.push(self.faixa_grupo(tr!("Mostrar"), self.faixa_linha(vec![
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Linhas de grade"), "table", Some(Msg::LinhasGrade(!self.linhas_grade_visiveis())), self.linhas_grade_visiveis()),
                        self.faixa_pequeno(tr!("Barra de fórmulas"), "math-function", Some(Msg::AlternarBarraFx), self.mostrar_fx),
                        self.faixa_pequeno(tr!("Fórmulas"), "code", Some(Msg::AlternarMostrarFormulas), self.mostrar_formulas),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Pintar linha/coluna"), "palette", Some(Msg::RealceEixos(!self.config.realce_eixos)), self.config.realce_eixos),
                        self.faixa_lista(tr!("Tema"), "tema", TEMA, Some(if self.config.escuro { "Escuro" } else { "Claro" }), 91.),
                        self.faixa_pequeno(tr!("Monitor"), "device-desktop", Some(Msg::Monitor), false),
                    ]),
                ])));
                // Tamanho 0 no mesmo mapa de dimensões do TypeScript.
                // Opera na seleção e fica disponível também no menu de cabeçalhos.
                grupos.push(self.faixa_grupo(tr!("Linhas e colunas"), self.faixa_linha(vec![
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Ocultar linhas"), "eye-off",
                            Some(Msg::OcultarDimensoes(false)), false),
                        self.faixa_pequeno(tr!("Reexibir linhas"), "inspecao",
                            Some(Msg::ReexibirDimensoes(false)), false),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Ocultar colunas"), "eye-off",
                            Some(Msg::OcultarDimensoes(true)), false),
                        self.faixa_pequeno(tr!("Reexibir colunas"), "inspecao",
                            Some(Msg::ReexibirDimensoes(true)), false),
                    ]),
                ])));
                grupos.push(self.faixa_grupo(tr!("Janela"), self.faixa_linha(vec![
                    self.faixa_grande_seta(tr!("Congelar"), "lock", Some(Msg::CongelarMenu), self.congelar_menu_aberto),
                    self.faixa_grande(if self.tela_inteira { tr!("Sair tela inteira") } else { tr!("Tela inteira") },
                        "arrows-maximize", Some(Msg::TelaInteira), self.tela_inteira),
                ])));
                grupos.push(self.faixa_grupo(tr!("Zoom"), self.faixa_lista(tr!("Zoom"), "zoom", ZOOMS, self.faixa_nome_zoom(), 88.)));
                grupos.push(self.faixa_grupo(tr!("Impressão"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Visualizar"), "printer", Some(Msg::Impressao(EventoImpressao::Abrir)), false),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Definir área"), "square", Some(Msg::Impressao(EventoImpressao::DefinirArea)), false),
                        self.faixa_pequeno(tr!("Adicionar à área"), "plus", Some(Msg::Impressao(EventoImpressao::AdicionarArea)), false),
                        self.faixa_pequeno(tr!("Limpar área"), "eraser", Some(Msg::Impressao(EventoImpressao::LimparArea)), false),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Quebras de página"), "file", Some(Msg::Impressao(EventoImpressao::AlternarQuebras)), self.quebras_mostradas()),
                    ]),
                ])));
            }
            AbaFaixa::Macros => {
                grupos.push(self.faixa_grupo(tr!("Editor"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Editor de macros"), "code", Some(Msg::Macros), false),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Novo módulo"), "plus", Some(Msg::MacroNovoModulo), false),
                        self.faixa_pequeno(tr!("Novo componente"), "plus", Some(Msg::MacroNovoFormulario), false),
                        self.faixa_pequeno(tr!("Editar"), "edit", Some(Msg::Macros), false),
                    ]),
                ])));
                grupos.push(self.faixa_grupo(tr!("Executar"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Executar macro"), "player-play", Some(Msg::MacroExecutar), false),
                    self.faixa_grande(tr!("Componentes"), "componentes", Some(Msg::MacroComponentes), false),
                ])));
                grupos.push(self.faixa_grupo(tr!("Planilha"), self.faixa_grande(tr!("Botão de macro"), "code", Some(Msg::BotaoMacroNovo), false)));
                grupos.push(self.faixa_grupo(tr!("Ajuda"), self.faixa_grande(tr!("Guia das funções"), "guia_funcoes", Some(Msg::Ajuda), false)));
            }
            AbaFaixa::Dinamica => {
                let dinamica = self.grupos_faixa_dinamica();
                if dinamica.is_empty() {
                    grupos.push(self.faixa_grupo(tr!("Tabela Dinâmica"),
                        text(tr!("Selecione uma célula de uma tabela dinâmica.")).size(12).into()));
                } else {
                    grupos.extend(dinamica);
                }
            }
            AbaFaixa::Tabela => {
                // Paridade de src/ui/barras/ribbon-tabela.ts.
                if let Some(t)=tabelas::sob_celula(
                    &self.workbook,self.folha,self.linha,self.coluna)
                {
                    let acao_nome = if self.tabela_renomeando {
                        self.faixa_linha(vec![
                            text_input(tr!("Novo nome"),&self.tabela_nome_novo)
                                .on_input(|v|Msg::TabelaAcao(tabelas::Acao::Nome(v)))
                                .on_submit(Msg::TabelaAcao(tabelas::Acao::Renomear))
                                .width(Length::Fixed(154.0)).padding(6).into(),
                            self.faixa_pequeno(tr!("Salvar"),"confirmar",
                                Some(Msg::TabelaAcao(tabelas::Acao::Renomear)),false),
                            self.faixa_pequeno(tr!("Cancelar"),"fechar",
                                Some(Msg::TabelaAcao(tabelas::Acao::CancelarRenomeacao)),false),
                        ])
                    } else {
                        self.faixa_linha(vec![
                            self.faixa_pequeno(tr!("Renomear"),"edit",
                                Some(Msg::TabelaAcao(tabelas::Acao::IniciarRenomeacao)),false),
                        ])
                    };
                    let acao_intervalo = if self.tabela_redimensionando {
                        self.faixa_linha(vec![
                            text_input("A1:C20",&self.tabela_intervalo_novo)
                                .on_input(|v|Msg::TabelaAcao(tabelas::Acao::Intervalo(v)))
                                .on_submit(Msg::TabelaAcao(tabelas::Acao::Redimensionar))
                                .width(Length::Fixed(116.0)).padding(6).into(),
                            self.faixa_pequeno(tr!("Salvar"),"confirmar",
                                Some(Msg::TabelaAcao(tabelas::Acao::Redimensionar)),false),
                            self.faixa_pequeno(tr!("Cancelar"),"fechar",
                                Some(Msg::TabelaAcao(tabelas::Acao::CancelarRedimensionamento)),false),
                        ])
                    } else {
                        self.faixa_linha(vec![
                            self.faixa_pequeno(tr!("Redimensionar"),"table",
                                Some(Msg::TabelaAcao(tabelas::Acao::IniciarRedimensionamento)),false),
                            self.faixa_pequeno(tr!("Converter"),"table",
                                Some(Msg::TabelaAcao(tabelas::Acao::Converter)),false),
                        ])
                    };
                    grupos.push(self.faixa_grupo(tr!("Propriedades"), self.faixa_pilha(vec![
                        acao_nome, acao_intervalo,
                    ])));
                    grupos.push(self.faixa_grupo(tr!("Opções de estilo"),self.faixa_pilha(vec![
                        self.faixa_linha(vec![
                            self.faixa_pequeno(tr!("Faixas por linha"),"layout-rows",
                                Some(Msg::TabelaAcao(tabelas::Acao::Faixas)),t["faixas"]==true),
                            self.faixa_pequeno(tr!("Faixas por coluna"),"layout-columns",
                                Some(Msg::TabelaAcao(tabelas::Acao::FaixasColuna)),
                                t["estilo"]["faixasColuna"]==true),
                        ]),
                        self.faixa_linha(vec![
                            self.faixa_pequeno(tr!("Bordas"),"border-all",
                                Some(Msg::TabelaAcao(tabelas::Acao::Borda)),
                                t["estilo"]["borda"]==true),
                            self.faixa_pequeno(tr!("Linha de totais"),"sum",
                                Some(Msg::TabelaAcao(tabelas::Acao::Totais)),
                                t["mostrarTotais"]==true),
                        ]),
                    ])));
                    // Igual ao TS: um botão abre o menu suspenso (CRIAR TABELA +
                    // grade de miniaturas), o mesmo da aba Início. Sem paleta em bolinhas.
                    let botao_estilos = button(column![
                        estilos_popup::icone_tabela(),
                        row![
                            text(tr!("Estilos de tabela")).size(11),
                            feather_ui::icones::icone("chevron-down", 12.0),
                        ].spacing(3).align(pelicano::Align::Center),
                    ].spacing(3).align(pelicano::Align::Center))
                        .padding([4, 8])
                        .style(estilo_botao_grande(self.tabelas_menu_aberto))
                        .on_press(Msg::AlternarMenuTabelas);
                    grupos.push(self.faixa_grupo(tr!("Estilos de Tabela"), botao_estilos.into()));
                } else {
                    grupos.push(self.faixa_grupo(tr!("Tabela"),
                        text(tr!("Selecione uma célula da tabela")).size(12).into()));
                }
            }
            AbaFaixa::Ferramentas => {
                // Tabela somente na aba contextual; não mais em Ferramentas.
                grupos.push(self.faixa_grupo(tr!("Automação"), self.faixa_catalogo_funcoes()));
                grupos.push(self.faixa_grupo(tr!("Aba"), self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Renomear aba…"), "edit", Some(Msg::EditarNomeAba), false),
                    self.faixa_pequeno(tr!("Duplicar aba"), "copy", Some(Msg::DuplicarAba), false),
                    self.faixa_pequeno(tr!("Excluir aba"), "trash", (self.workbook.len()>1).then_some(Msg::RemoverAba), false),
                ])));
                grupos.push(self.faixa_grupo(tr!("Proteção"), self.faixa_pilha(vec![
                    // Aceso = a célula ativa está bloqueada (o padrão, como no Excel): o clique desbloqueia.
                    self.faixa_pequeno(tr!("Bloquear seleção"), "lock", Some(Msg::Protecao(protecao_ui::Evento::AlternarBloqueio)),
                        self.workbook.aba(self.folha).is_some_and(|a| a.sheet.celula_marcada_bloqueada(self.linha, self.coluna))),
                    self.faixa_pequeno(tr!("Proteger planilha"), "lock", Some(Msg::Protecao(protecao_ui::Evento::Abrir)), false),
                ])));
                grupos.push(self.faixa_grupo(tr!("Aplicativo"), self.faixa_linha(vec![
                    self.faixa_grande(tr!("Ajuda"), "guia_funcoes", Some(Msg::Ajuda), false),
                    self.faixa_grande(tr!("Configurações"), "settings", Some(Msg::Configuracoes), false),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Monitor"), "device-desktop", Some(Msg::Monitor), false),
                        self.faixa_pequeno(tr!("Atalhos"), "atalhos_teclado", Some(Msg::Ajuda), false),
                        self.faixa_pequeno(tr!("Sobre"), "informacao", Some(Msg::MostrarSobre), false),
                    ]),
                ])));
            }
        }
        grupos
    }

    pub(super) fn view_faixa(&self) -> Element<'_, Msg> {
        let todos = self.montar_grupos_faixa();
        let limite = self.faixa_visiveis().min(todos.len());
        let possui_extras = limite < todos.len();
        let mut grupos:Vec<Element<'_,Msg>>=Vec::new();
        if self.faixa_ativa==AbaFaixa::Inicio {
            let plano=faixa_responsiva::planejar(self.tamanho_principal.width);
            for (indice,inteiro) in todos.into_iter().take(limite).enumerate() {
                grupos.push(match plano.niveis[indice] {
                    Nivel::Completo=>inteiro,
                    Nivel::Compacto=>self.faixa_inicio_compacto(indice),
                    Nivel::Recolhido=>self.faixa_inicio_recolhido(indice),
                });
            }
        } else {
            grupos.extend(todos.into_iter().take(limite));
        }
        if possui_extras {
            let botao_mais = button(column![
                feather_ui::icones::icone("chevron-down", 19.0),
                text(tr!("Mais")).size(11),
            ].spacing(4).align(pelicano::Align::Center))
                .padding([7, 12])
                .style(estilo_botao_grande(self.faixa_extras_aberto))
                .on_press(Msg::AlternarMenuExtras);
            grupos.push(ui::grupo_faixa_compacto(botao_mais.into(), tr!("Ferramentas")));
        }
        // Sem widget Scrollable: nunca aparece uma barra horizontal na Ribbon.
        let conteudo = row(grupos)
            .spacing(2)
            .align(pelicano::Align::Center)
            .height(Length::Fixed(ALTURA_CONTROLES_FAIXA));
        container(conteudo)
            .height(Length::Fixed(ALTURA_FAIXA_NATIVA))
            .padding([2, 5])
            .width(Length::FILL).into()
    }

    /// Menu de excedentes: mesmo conteúdo funcional dos grupos originais.
    /// Apenas a lista vertical interna pode rolar; nunca há scroll horizontal.
    pub(super) fn view_faixa_extras(&self) -> Element<'_, Msg> {
        let quantidade_visivel = self.faixa_visiveis();
        let grupos = self.montar_grupos_faixa();
        let ocultos = grupos.len().saturating_sub(quantidade_visivel);
        let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(5);
        for grupo in grupos.into_iter().skip(quantidade_visivel) {
            lista = lista.push(grupo);
        }
        let altura_disponivel = (self.tamanho_principal.height
            - ALTURA_FAIXA_NATIVA - 132.0).max(120.0).min(510.0);
        let painel = column![
            row![
                text(trf!("Mais ferramentas · {ocultos} grupos")).size(13),
                Space::new(Length::FILL, 0.0),
                button(feather_ui::icones::icone("x", 15.0))
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::FecharMenuExtras),
            ].spacing(8).align(pelicano::Align::Center),
            scrollable(lista)
                .direction(pelicano::Direction::Vertical)
                .height(Length::Fixed(altura_disponivel)),
        ].spacing(8).padding(10);
        container(painel)
            .width(Length::Fixed(
                (self.tamanho_principal.width - 24.0).min(640.0).max(1.0)
            ))
            .style(estilo_popup_menu)
            .into()
    }

    /// Overlay abaixo da faixa, ancorado à direita. Não empurra a grade.
    pub(super) fn sobrepor_faixa_extras<'a>(
        &'a self, base: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        if !self.faixa_extras_aberto
            || self.faixa_visiveis() >= self.faixa_larguras().len()
        { return base; }
        let largura = (self.tamanho_principal.width - 24.0).min(640.0).max(1.0);
        let x = (self.tamanho_principal.width - largura - 12.0).max(8.0);
        let altura = (self.tamanho_principal.height
            - ALTURA_FAIXA_NATIVA - 132.0).max(120.0).min(510.0) + 72.0;
        let y_ideal = 43.0 + 52.0 + ALTURA_FAIXA_NATIVA;
        let y = y_ideal.min((self.tamanho_principal.height - altura - 8.0).max(8.0));
        let fora = pelicano::mouse_area(
            Space::new(Length::FILL, Length::FILL)
        ).on_press(Msg::FecharMenuExtras);
        stack![
            base, fora,
            column![
                Space::new(0.0, Length::Fixed(y)),
                row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.view_faixa_extras(),
                ],
            ],
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes_acoes_legadas {
    use super::*;

    #[test]
    fn ribbon_comandos_typescript_tem_tres_abas_nativas() {
        let mut app = App::default();
        for faixa in [AbaFaixa::Inicio, AbaFaixa::Dados, AbaFaixa::Exibir] {
            app.faixa_ativa = faixa;
            assert!(!app.montar_grupos_faixa().is_empty());
        }
    }

    #[test]
    fn modo_tela_inteira_alterna_sem_modificar_a_planilha() {
        let mut app = App::default();
        let versao = app.workbook.aba(0).unwrap().sheet.len();
        assert!(!app.tela_inteira);
        let _ = app.update(Msg::TelaInteira);
        assert!(app.tela_inteira);
        let _ = app.update(Msg::TelaInteira);
        assert!(!app.tela_inteira);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.len(), versao);
        assert!(!app.alterado);
    }

    #[test]
    fn estatisticas_da_ribbon_reutilizam_perfil_nativo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "10".into());
        app.workbook.escrever_celula(0, 1, 0, "20".into());
        let _ = app.update(Msg::PerfilColunaAbrir);
        let visao = app.perfil_coluna.as_ref().expect("painel de perfil nativo");
        assert_eq!(visao.dados.numeros, 2);
        assert_eq!(visao.dados.linhas, 2);
        assert!(!app.alterado);
    }
}

/// Comando ainda sem motor: fica desabilitado e a dica diz o que fará.
fn em_breve<'a>(botao: Element<'a, Msg>, dica: &'static str) -> Element<'a, Msg> {
    pelicano::tooltip(
        botao,
        dica,
        pelicano::TooltipPosition::Bottom,
    )
    .padding([4, 8]).style(estilo_dica)
    .into()
}
