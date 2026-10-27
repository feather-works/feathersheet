//! Menus suspensos da Página Inicial: AutoSoma, Preencher, Classificar/Filtrar,
//! Formatar (grupo Células) e Mesclar.
//! Painéis ancorados à Ribbon, sem substituir o conteúdo da planilha.
use super::*;
use super::impressao_ui::{rotulo_paginas, EscalaUi, MargemUi, PapelUi};
use feathersheet_core::planilha::impressao::Orientacao;

const LARGURA: f32 = 262.0;

/// Linha de um menu suspenso: rótulo de seção, comando ou divisória.
enum Item {
    Titulo(&'static str),
    Acao(&'static str, &'static str, Msg),
    /// Item com texto que vem da pasta ou das preferências (fontes recentes, conexões).
    Dinamico(String, &'static str, Msg),
    Divisor,
}

/// Inserir > Símbolo e Equação: grades de caracteres por seção.
type Grade = &'static [(&'static str, &'static [&'static str])];
const SIMBOLOS: Grade = &[
    ("MOEDA", &["R$", "€", "£", "¥", "¢", "₹", "₩", "₽", "₿", "$"]),
    ("MARCAS E PONTUAÇÃO", &["©", "®", "™", "§", "¶", "†", "‡", "•", "…", "°", "‰", "º", "ª", "«", "»", "¿", "¡", "–", "—"]),
    ("SETAS", &["←", "→", "↑", "↓", "↔", "↕", "⇐", "⇒", "⇔", "↗", "↘"]),
    ("FRAÇÕES", &["½", "⅓", "⅔", "¼", "¾", "⅕", "⅛", "⅜", "⅝", "⅞"]),
];
const EQUACAO: Grade = &[
    ("OPERADORES", &["±", "×", "÷", "≠", "≈", "≡", "≤", "≥", "∞", "√", "∛", "∑", "∏", "∫", "∂", "∆", "∇", "∝"]),
    ("POTÊNCIAS E ÍNDICES", &["⁰", "¹", "²", "³", "⁴", "ⁿ", "₀", "₁", "₂", "₃"]),
    ("LETRAS GREGAS", &["α", "β", "γ", "δ", "ε", "θ", "λ", "µ", "π", "ρ", "σ", "τ", "φ", "ω", "Δ", "Σ", "Ω"]),
    ("CONJUNTOS E LÓGICA", &["∈", "∉", "⊂", "⊆", "∪", "∩", "∅", "∀", "∃", "¬", "∧", "∨", "ℕ", "ℤ", "ℝ"]),
];
const POR_LINHA: usize = 8;

fn grade(tipo: &str) -> Option<Grade> {
    match tipo { "simbolo" => Some(SIMBOLOS), "equacao" => Some(EQUACAO), _ => None }
}

fn itens(tipo: &str) -> Vec<Item> {
    use Item::{Acao, Divisor, Titulo};
    match tipo {
        "autosoma" => vec![
            Titulo(tr!("INSERIR FUNÇÃO")),
            Acao(tr!("Soma · Alt+="), "sum", Msg::AutoSoma),
            Acao(tr!("Média"), "math-function", Msg::AutoConta("MÉDIA")),
            Acao(tr!("Contagem"), "list-numbers", Msg::AutoConta("CONT.NÚM")),
            Acao(tr!("Máximo"), "sort-descending", Msg::AutoConta("MÁXIMO")),
            Acao(tr!("Mínimo"), "sort-ascending", Msg::AutoConta("MÍNIMO")),
        ],
        "preencher" => vec![
            Titulo(tr!("PREENCHER CÉLULAS")),
            Acao(tr!("Para baixo · Ctrl+D"), "arrow-down", Msg::Preencher(DirecaoPreenchimento::Baixo)),
            Acao(tr!("Para direita · Ctrl+R"), "seta_direita", Msg::Preencher(DirecaoPreenchimento::Direita)),
            Acao(tr!("Para cima"), "sort-ascending", Msg::Preencher(DirecaoPreenchimento::Cima)),
            Acao(tr!("Para esquerda"), "seta_esquerda", Msg::Preencher(DirecaoPreenchimento::Esquerda)),
            Acao(tr!("Continuar até o fim"), "arrow-step", Msg::Preencher(DirecaoPreenchimento::AteOFim)),
            Acao(tr!("Série…"), "list-numbers", Msg::Serie(serie_ui::Evento::Abrir)),
            Divisor,
            Acao(tr!("Preenchimento relâmpago · Ctrl+E"), "bolt", Msg::PreenchimentoRelampago),
        ],
        // Validação ▾ da aba Dados, como no Excel.
        "validacao" => vec![
            Titulo(tr!("VALIDAÇÃO DE DADOS")),
            Acao(tr!("Validação de dados…"), "checks", Msg::ValidacaoAbrir),
            Acao(tr!("Circular dados inválidos"), "circle", Msg::ValidacaoEv(validacao_dialogo::Evento::Circular)),
            Acao(tr!("Limpar círculos de validação"), "eraser",
                Msg::ValidacaoEv(validacao_dialogo::Evento::LimparCirculos)),
        ],
        // Formatar ▾ do grupo Células, na ordem do Excel.
        "formatar" => vec![
            Titulo(tr!("TAMANHO DA CÉLULA")),
            Acao(tr!("Altura da linha…"), "line-height",
                Msg::Dimensao(dimensao_dialogo::Evento::Abrir(false))),
            Acao(tr!("AutoAjuste da altura da linha"), "arrow-autofit-height", Msg::AjustarAltura),
            Acao(tr!("Largura da coluna…"), "arrow-autofit-width",
                Msg::Dimensao(dimensao_dialogo::Evento::Abrir(true))),
            Acao(tr!("AutoAjuste da largura da coluna"), "arrows-horizontal", Msg::AjustarLargura),
            Acao(tr!("Largura padrão…"), "arrow-autofit-width", Msg::Dimensao(dimensao_dialogo::Evento::AbrirPadrao)),
            Titulo("VISIBILIDADE"),
            Acao(tr!("Ocultar linhas"), "eye-off", Msg::OcultarDimensoes(false)),
            Acao(tr!("Ocultar colunas"), "eye-off", Msg::OcultarDimensoes(true)),
            Acao(tr!("Reexibir linhas"), "eye", Msg::ReexibirDimensoes(false)),
            Acao(tr!("Reexibir colunas"), "eye", Msg::ReexibirDimensoes(true)),
            Divisor,
            Acao(tr!("Formatar células…"), "edit", Msg::FormatarCelulasAbrir),
            Divisor,
            Titulo(tr!("PROTEÇÃO")),
            Acao(tr!("Proteger ou desproteger planilha…"), "lock", Msg::Protecao(protecao_ui::Evento::Abrir)),
            Acao(tr!("Bloquear/desbloquear célula"), "lock", Msg::Protecao(protecao_ui::Evento::AlternarBloqueio)),
        ],
        // Inserir ▾ / Excluir ▾ do grupo Células, como no Excel.
        "inserir" => vec![
            Titulo("INSERIR"),
            Acao(tr!("Inserir células… · Ctrl++"), "table", Msg::Celulas(celulas_dialogo::Evento::Abrir(true))),
            Acao(tr!("Inserir linhas na planilha"), "plus", Msg::InserirMenu(Estrutura::InserirLinha, false)),
            Acao(tr!("Inserir colunas na planilha"), "columns", Msg::InserirMenu(Estrutura::InserirColuna, false)),
            Acao(tr!("Inserir planilha · Shift+F11"), "file-plus", Msg::AdicionarAba),
        ],
        "simbolo" | "equacao" => vec![],
        "excluir" => vec![
            Titulo("EXCLUIR"),
            Acao(tr!("Excluir células… · Ctrl+-"), "table", Msg::Celulas(celulas_dialogo::Evento::Abrir(false))),
            Acao(tr!("Excluir linhas da planilha"), "trash", Msg::Estrutura(Estrutura::ExcluirLinha)),
            Acao(tr!("Excluir colunas da planilha"), "columns", Msg::Estrutura(Estrutura::ExcluirColuna)),
        ],
        // Limpar ▾ do grupo Edição, como no Excel.
        "limpar" => vec![
            Titulo("LIMPAR"),
            Acao(tr!("Limpar tudo"), "eraser", Msg::LimparTudo),
            Acao(tr!("Limpar formatos"), "eraser", Msg::LimparFormato),
            Acao(tr!("Limpar conteúdo · Delete"), "eraser", Msg::Limpar),
            Acao(tr!("Limpar comentários"), "message-circle", Msg::Comentario(comentarios_ui::Evento::Limpar)),
            Acao(tr!("Limpar hiperlinks"), "link-off", Msg::LimparHiperlinks),
        ],
        // Sublinhado ▾ do grupo Fonte, como no Excel.
        "sublinhado" => vec![
            Titulo("SUBLINHADO"),
            Acao(tr!("Sublinhado · Ctrl+U"), "underline", Msg::AlternarEstilo("underline".into())),
            Acao(tr!("Sublinhado duplo"), "underline-double",
                Msg::AlternarEstilo(feathersheet_core::operacoes::sublinhado::CHAVE_DUPLO.into())),
        ],
        // Aba Layout da Página: Margens ▾, Orientação ▾, Tamanho ▾,
        // Área de impressão ▾ e Escala ▾.
        "pg_margens" => vec![
            Titulo("MARGENS"),
            Acao(tr!("Normal · 12 mm"), "borda_externa_espessa", Msg::Impressao(EventoImpressao::Margem(MargemUi::Normal))),
            Acao(tr!("Estreita · 6,4 mm nas laterais"), "borda_externa_espessa", Msg::Impressao(EventoImpressao::Margem(MargemUi::Estreita))),
            Acao(tr!("Larga · 25,4 mm"), "borda_externa_espessa", Msg::Impressao(EventoImpressao::Margem(MargemUi::Larga))),
        ],
        "pg_orientacao" => vec![
            Titulo(tr!("ORIENTAÇÃO")),
            Acao(tr!("Retrato"), "orientacao_pagina", Msg::Impressao(EventoImpressao::Orientacao(Orientacao::Retrato))),
            Acao(tr!("Paisagem"), "orientacao_pagina", Msg::Impressao(EventoImpressao::Orientacao(Orientacao::Paisagem))),
        ],
        "pg_tamanho" => vec![
            Titulo(tr!("TAMANHO DO PAPEL")),
        ].into_iter().chain(PapelUi::TODOS.into_iter().map(|p|
            Acao(p.rotulo(), "arrows-maximize", Msg::Impressao(EventoImpressao::Papel(p))))).collect(),
        "pg_area" => vec![
            Titulo(tr!("ÁREA DE IMPRESSÃO")),
            Acao(tr!("Definir área de impressão"), "area_impressao", Msg::Impressao(EventoImpressao::DefinirArea)),
            Acao(tr!("Adicionar à área de impressão"), "plus", Msg::Impressao(EventoImpressao::AdicionarArea)),
            Acao(tr!("Limpar área de impressão"), "limpar_tudo", Msg::Impressao(EventoImpressao::LimparArea)),
        ],
        "obter_dados" => {
            let abrir = |e: EventoOpenQuery| Msg::OpenQuery(EventoOpenQuery::AbrirCom(Box::new(e)));
            vec![
                Titulo(tr!("OBTER DADOS")),
                Acao(tr!("De Texto/CSV, Excel ou JSON…"), "folder-open", abrir(EventoOpenQuery::ImportarArquivo)),
                Acao(tr!("Da Web…"), "link", abrir(EventoOpenQuery::MostrarWeb)),
                Acao(tr!("De Tabela/Intervalo"), "table", abrir(EventoOpenQuery::CarregarSelecao)),
                Acao(tr!("De uma tabela da pasta…"), "table", abrir(EventoOpenQuery::MostrarTabelas)),
                Divisor,
                Acao(tr!("Abrir o OpenQuery"), "table", Msg::OpenQuery(EventoOpenQuery::Abrir)),
            ]
        }
        "pg_largura" | "pg_altura" => {
            let largura = tipo == "pg_largura";
            let mut v = vec![Titulo(if largura { tr!("LARGURA") } else { tr!("ALTURA") })];
            v.extend((0..=9).map(|n| Acao(rotulo_paginas(n), if largura { "arrows-horizontal" } else { "arrows-vertical" },
                Msg::Impressao(if largura { EventoImpressao::LarguraPaginas(n) } else { EventoImpressao::AlturaPaginas(n) }))));
            v
        }
        "pg_escala" => {
            const PORCENTAGENS: [(&str, u16); 12] = [("25%", 25), ("50%", 50), ("70%", 70), ("75%", 75), ("80%", 80),
                ("90%", 90), ("100%", 100), ("110%", 110), ("125%", 125), ("150%", 150), ("200%", 200), ("400%", 400)];
            let mut v = vec![Titulo(tr!("ESCALA"))];
            v.extend(PORCENTAGENS.iter().map(|&(r, n)| Acao(r, "zoom-in", Msg::Impressao(EventoImpressao::Escala(EscalaUi::Porcentagem(n))))));
            v
        }
        "mesclar" => vec![
            Titulo("MESCLAR"),
            Acao(tr!("Mesclar e centralizar"), "align-center",
                Msg::MesclarModo(mesclas::ModoMescla::Centralizar)),
            Acao(tr!("Mesclar através"), "columns", Msg::MesclarModo(mesclas::ModoMescla::Atraves)),
            Acao(tr!("Mesclar células"), "square", Msg::MesclarModo(mesclas::ModoMescla::Mesclar)),
            Acao(tr!("Desmesclar células"), "desmesclar_celulas",
                Msg::MesclarModo(mesclas::ModoMescla::Desmesclar)),
        ],
        _ => vec![
            Titulo(tr!("CLASSIFICAR E FILTRAR")),
            Acao(tr!("Classificar de A a Z"), "sort-ascending", Msg::Ordenar(true)),
            Acao(tr!("Classificar de Z a A"), "sort-descending", Msg::Ordenar(false)),
            Acao(tr!("Classificação personalizada…"), "sort-ascending",
                Msg::Classificar(classificar::Evento::Abrir)),
            Acao(tr!("Ativar/desativar filtro"), "filter",
                Msg::TabelaAcao(tabelas::Acao::AlternarAutoFiltro)),
            Acao(tr!("Reaplicar"), "refresh", Msg::TabelaAcao(tabelas::Acao::Reaplicar)),
            Acao(tr!("Selecionar região atual"), "table", Msg::SelecionarRegiaoAtual),
        ],
    }
}

/// Altura estimada do painel, para não deixá-lo passar da borda da janela.
fn altura(tipo: &str, itens: &[Item]) -> f32 {
    if let Some(g) = grade(tipo) {
        return 20.0 + g.iter().map(|(_, s)| 22.0 + s.len().div_ceil(POR_LINHA) as f32 * 33.0).sum::<f32>();
    }
    20.0 + itens.iter().map(|item| match item {
        Item::Titulo(_) => 22.0,
        Item::Acao(..) | Item::Dinamico(..) => 37.0,
        Item::Divisor => 9.0,
    }).sum::<f32>()
}

impl App {
    /// O menu fica junto ao botão acionado, sem ocupar a área de trabalho.
    /// Os itens fixos de [`itens`] mais os que dependem do estado.
    fn itens_menu(&self, tipo: &str) -> Vec<Item> {
        let reabrir = |e: EventoOpenQuery| Msg::OpenQuery(EventoOpenQuery::AbrirCom(Box::new(e)));
        let lista: Vec<Item> = match tipo {
            "fontes_recentes" => self.config.fontes_recentes.iter().map(|f| Item::Dinamico(
                f.clone(), if f.starts_with("http") { "link" } else { "file" },
                reabrir(EventoOpenQuery::AbrirRecente(f.clone())))).collect(),
            "conexoes" => self.conexoes_existentes().into_iter().map(|(id, nome)| Item::Dinamico(
                nome, "table", reabrir(EventoOpenQuery::EditarSalva(id)))).collect(),
            _ => return itens(tipo),
        };
        let titulo = if tipo == "conexoes" { tr!("CONSULTAS DESTA PASTA") } else { tr!("FONTES RECENTES") };
        let vazio = if tipo == "conexoes" { tr!("Nenhuma consulta salva nesta pasta.") } else { tr!("Nenhuma fonte usada ainda.") };
        let mut v = vec![Item::Titulo(titulo)];
        if lista.is_empty() { v.push(Item::Titulo(vazio)); } else { v.extend(lista); }
        v
    }

    fn menu_edicao(&self, tipo: &'static str) -> Element<'_, Msg> {
        let mut coluna = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(3);
        for (secao, simbolos) in grade(tipo).unwrap_or_default() {
            coluna = coluna.push(text(crate::idioma::tr(secao)).size(11).color(COR_TEXTO_MUTED()));
            for linha in simbolos.chunks(POR_LINHA) {
                coluna = coluna.push(pelicano::row(linha.iter().map(|s| {
                    button(text(*s).size(15).align_x(pelicano::Align::Center).width(Length::FILL))
                        .width(Length::Fixed(28.0)).padding([4, 0])
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::EdicaoMenuAcao(Box::new(Msg::InserirSimbolo((*s).to_owned()))))
                        .into()
                })).spacing(2));
            }
        }
        for (i, item) in self.itens_menu(tipo).into_iter().enumerate() {
            coluna = match item {
                Item::Titulo(titulo) => coluna.push(
                    container(text(titulo).size(11).color(COR_TEXTO_MUTED()))
                        .padding(pelicano::Padding { top: if i == 0 { 0.0 } else { 6.0 }, ..Default::default() }),
                ),
                Item::Acao(rotulo, icone, msg) => coluna.push(
                    button(row![
                        feather_ui::icones::icone(icone, 17.),
                        text(rotulo).size(12),
                    ].spacing(9).align(pelicano::Align::Center))
                        .width(Length::FILL)
                        .padding([8, 10])
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::EdicaoMenuAcao(Box::new(msg))),
                ),
                Item::Dinamico(rotulo, icone, msg) => coluna.push(
                    button(row![
                        feather_ui::icones::icone(icone, 17.),
                        text(rotulo).size(12),
                    ].spacing(9).align(pelicano::Align::Center))
                        .width(Length::FILL)
                        .padding([8, 10])
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::EdicaoMenuAcao(Box::new(msg))),
                ),
                Item::Divisor => coluna.push(
                    container(pelicano::horizontal_rule(1.0)).padding([4, 0]),
                ),
            };
        }
        container(coluna).padding(10).width(Length::Fixed(LARGURA))
            .style(estilo_popup_menu).into()
    }

    pub(super) fn sobrepor_menu_edicao<'a>(
        &'a self, base: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        let Some(tipo) = self.edicao_menu else { return base; };
        let altura = altura(tipo, &self.itens_menu(tipo));
        let x = self.edicao_popup_x.clamp(
            8.0, (self.tamanho_principal.width - LARGURA - 8.0).max(8.0),
        );
        let y = self.edicao_popup_y.clamp(
            8.0, (self.tamanho_principal.height - altura - 8.0).max(8.0),
        );
        let fora = pelicano::mouse_area(
            Space::new(Length::FILL, Length::FILL)
        ).on_press(Msg::FecharMenuEdicao);
        stack![
            base, fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                row![
                    Space::new(Length::Fixed(x), 0.0),
                    self.menu_edicao(tipo),
                ],
            ],
        ].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn menus_flutuantes_de_edicao_preservam_grade() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuEdicao("autosoma"));
        assert_eq!(app.edicao_menu, Some("autosoma"));
        let _: Element<'_, Msg> = app.menu_edicao("autosoma");
        let _: Element<'_, Msg> = app.menu_edicao("preencher");
        let _: Element<'_, Msg> = app.menu_edicao("formatar");
        let _: Element<'_, Msg> = app.menu_edicao("mesclar");
        app.update(Msg::AlternarMenuEdicao("classificar"));
        assert_eq!(app.edicao_menu, Some("classificar"));
        app.update(Msg::FecharMenuEdicao);
        assert!(app.edicao_menu.is_none());
    }

    #[test]
    fn icones_dos_menus_existem_no_catalogo() {
        for tipo in ["autosoma", "preencher", "classificar", "formatar", "mesclar", "validacao", "limpar", "sublinhado",
            "pg_margens", "pg_orientacao", "pg_tamanho", "pg_area", "pg_escala", "pg_largura", "pg_altura", "obter_dados"] {
            for item in itens(tipo) {
                if let Item::Acao(rotulo, icone, _) = item {
                    assert!(feather_ui::icones::existe(icone), "{tipo}: {rotulo} ({icone})");
                }
            }
        }
    }

    #[test]
    fn formatar_abre_o_dialogo_de_altura_pelo_menu() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuEdicao("formatar"));
        app.update(Msg::EdicaoMenuAcao(Box::new(
            Msg::Dimensao(dimensao_dialogo::Evento::Abrir(false)))));
        assert!(app.edicao_menu.is_none());
        assert!(app.dimensao_modal.as_ref().is_some_and(|m| !m.coluna));
    }
}

#[cfg(test)]
mod testes_recuo_limpar {
    use super::*;
    use feathersheet_core::operacoes::recuo::recuo_de;

    #[test]
    fn botoes_de_recuo_e_limpar_tudo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "abc".into());
        let _ = app.update(Msg::AlterarRecuo(1));
        let _ = app.update(Msg::AlterarRecuo(1));
        assert_eq!(recuo_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), 2);
        let _ = app.update(Msg::AlterarRecuo(-1));
        assert_eq!(recuo_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), 1);
        // Limpar formatos tira o recuo e deixa o texto.
        let _ = app.update(Msg::LimparFormato);
        assert_eq!(recuo_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), 0);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "abc");
        // Limpar tudo tira conteúdo e formato numa só ação desfazível.
        let _ = app.update(Msg::AlterarRecuo(3));
        let _ = app.update(Msg::LimparTudo);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(0, 0), "");
        assert_eq!(recuo_de(sheet, 0, 0), 0);
        let _ = app.update(Msg::Desfazer);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(0, 0), "abc");
        assert_eq!(recuo_de(sheet, 0, 0), 3);
    }

    #[test]
    fn menu_limpar_existe_com_cinco_acoes() {
        // Limpar tudo, formatos, conteúdo, comentários e hiperlinks, como no Excel.
        assert_eq!(itens("limpar").iter().filter(|i| matches!(i, Item::Acao(..))).count(), 5);
    }

    #[test]
    fn limpar_hiperlinks_tira_o_link_da_selecao_e_mantem_o_texto() {
        let mut app = App::default();
        for r in 0..3 {
            app.workbook.escrever_celula(0, r, 0, format!("site {r}"));
        }
        let mut patch = EstiloParcial::new();
        patch.insert("link".into(), Campo::Texto("https://exemplo.com.br".into()));
        patch.insert("bold".into(), Campo::Logico(true));
        app.workbook.editar(0, |s| s.set_style(&Rect::new(0, 0, 2, 0), &patch));
        let _ = app.update(Msg::Selecionar(0, 0));
        let _ = app.update(Msg::Estender(1, 0));
        let _ = app.update(Msg::EdicaoMenuAcao(Box::new(Msg::LimparHiperlinks)));
        let link = |app: &App, r| app.workbook.aba(0).unwrap().sheet.estilo_resolvido(r, 0).unwrap().link;
        assert!(link(&app, 0).is_none() && link(&app, 1).is_none());
        assert!(link(&app, 2).is_some(), "fora da seleção não muda");
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(1, 0), "site 1");
        assert!(s.estilo_resolvido(1, 0).unwrap().bold, "formatação preservada");
        let _ = app.update(Msg::Desfazer);
        assert!(link(&app, 0).is_some() && link(&app, 1).is_some(), "um Ctrl+Z só");
    }

    #[test]
    fn sublinhado_duplo_pelo_menu_e_recuo_pelo_teclado() {
        use feathersheet_core::operacoes::sublinhado::{sublinhado_de, Sublinhado};
        let mut app = App::default();
        let id = window::novo_id();
        app.janelas.principal = Some(id);
        app.workbook.escrever_celula(0, 0, 0, "abc".into());
        let _ = app.update(Msg::AlternarMenuEdicao("sublinhado"));
        let _: Element<'_, Msg> = app.menu_edicao("sublinhado");
        let Item::Acao(_, _, duplo) = itens("sublinhado").remove(2) else { panic!() };
        let _ = app.update(Msg::EdicaoMenuAcao(Box::new(duplo)));
        assert!(app.edicao_menu.is_none());
        assert_eq!(sublinhado_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), Sublinhado::Duplo);
        // Ctrl+U troca o duplo pelo simples; de novo, tira.
        let ctrl = keyboard::Modifiers::CTRL;
        let _ = app.update(Msg::Teclado(id, keyboard::Key::Character("u".into()), ctrl));
        assert_eq!(sublinhado_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), Sublinhado::Simples);
        let _ = app.update(Msg::Teclado(id, keyboard::Key::Character("u".into()), ctrl));
        assert_eq!(sublinhado_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), Sublinhado::Nenhum);
        // Ctrl+Alt+Tab / Ctrl+Alt+Shift+Tab: recuo, como no Excel.
        let tab = keyboard::Key::Named(keyboard::key::Named::Tab);
        let _ = app.update(Msg::Teclado(id, tab.clone(), ctrl | keyboard::Modifiers::ALT));
        let _ = app.update(Msg::Teclado(id, tab.clone(), ctrl | keyboard::Modifiers::ALT));
        assert_eq!(recuo_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), 2);
        let _ = app.update(Msg::Teclado(id, tab, ctrl | keyboard::Modifiers::ALT | keyboard::Modifiers::SHIFT));
        assert_eq!(recuo_de(&app.workbook.aba(0).unwrap().sheet, 0, 0), 1);
    }
}
