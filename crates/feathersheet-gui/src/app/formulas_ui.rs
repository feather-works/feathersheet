//! Aba Fórmulas da faixa, como a do Excel: biblioteca de funções por
//! categoria, nomes definidos, auditoria (setas de rastreio) e cálculo.
//!
//! As contas ficam no motor (`Sheet::precedentes`/`dependentes`,
//! `Workbook::calcular_agora`); aqui só se escolhe o que mostrar.
use super::*;
use iced::widget::{checkbox, column};
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::funcoes::{function_help, functions};
#[cfg(test)]
use feathersheet_core::planilha::sheet::nomes_selecao::Bordas;

/// Quantas funções usadas por último a aba guarda (o Excel guarda 10).
const MAX_RECENTES: usize = 10;
/// Um intervalo maior que isto não é aberto célula a célula no próximo nível
/// de precedentes: `SOMA(A:A)` teria um milhão de setas.
const MAX_CELULAS_NIVEL: u64 = 2_000;

/// Categorias com botão próprio na faixa, na ordem do Excel; as demais ficam
/// em "Mais funções".
const CATEGORIAS_FAIXA: [(&str, &str, &str); 6] = [
    ("Financeiras", "Financeira", "currency-dollar"),
    ("Lógica", "Lógica", "checks"),
    ("Texto", "Texto", "text-size"),
    ("Data e hora", "Data e hora", "relogio"),
    ("Pesquisa e referência", "Pesquisa e referência", "search"),
    ("Matemática", "Matemática", "equation"),
];

/// Menu suspenso aberto a partir da aba Fórmulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuFormulas {
    /// Índice em `function_help::CATEGORIES`.
    Categoria(usize),
    Mais,
    Recentes,
    Nomes,
    Calculo,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Menu(MenuFormulas),
    FecharMenu,
    UsarNome(String),
    RastrearPrecedentes,
    RastrearDependentes,
    RemoverSetas,
    Manual(bool),
    CalcularAgora,
    CalcularPlanilha,
    SelecionarDerramada,
    /// Criar a partir da seleção: abrir, marcar uma borda (0 topo, 1
    /// esquerda, 2 base, 3 direita), criar e cancelar.
    CriarAbrir,
    CriarBorda(u8),
    CriarConfirmar,
    CriarFechar,
}

/// Uma seta de rastreio: de quem a fórmula lê (`origem`) até a fórmula.
#[derive(Debug, Clone, PartialEq)]
pub struct Seta {
    pub origem: Rect,
    pub destino: (u32, u32),
    /// A origem está em outra aba: desenha tracejada, com o ícone de planilha.
    pub externa: bool,
    /// A origem tem erro: desenha em vermelho, como o Excel.
    pub erro: bool,
}

/// Setas da aba onde foram traçadas. Cada clique em Rastrear avança um nível
/// a partir da última fronteira; mudar a planilha apaga tudo (como o Excel).
/// Célula onde o rastreio começou e a fronteira do próximo nível.
type Nivel = Option<((u32, u32), Vec<(u32, u32)>)>;

#[derive(Debug, Default)]
pub struct Rastreio {
    aba_id: Option<u32>,
    versao: u32,
    pub setas: Vec<Seta>,
    precedentes: Nivel,
    dependentes: Nivel,
}

/// Nome em português da função, quando existe apelido (SUM → SOMA).
fn nome_exibido(canonico: &str) -> String {
    // O de-para do idioma (SOMA, SUM, SUMA).
    crate::idioma::nome_funcao(canonico)
}

/// Primeira frase da ajuda, cortada para caber na linha do menu.
fn resumo(canonico: &str) -> String {
    let doc = crate::idioma::ajuda_funcao(canonico).map(|(_, d)| d).unwrap_or_default();
    let doc = doc.as_str();
    let frase = doc.split(". ").next().unwrap_or(doc).trim_end_matches('.');
    if frase.chars().count() > 48 {
        format!("{}…", frase.chars().take(47).collect::<String>())
    } else { frase.to_owned() }
}

/// Menu pelo nome (modo de captura): uma categoria ou Mais/Recentes/Nomes/Cálculo.
pub(super) fn menu_por_nome(nome: &str) -> Option<MenuFormulas> {
    match nome {
        "Mais" => Some(MenuFormulas::Mais),
        "Recentes" => Some(MenuFormulas::Recentes),
        "Nomes" => Some(MenuFormulas::Nomes),
        "Cálculo" => Some(MenuFormulas::Calculo),
        _ => function_help::CATEGORIES.iter().position(|(c, _)| *c == nome).map(MenuFormulas::Categoria),
    }
}

fn indice_categoria(nome: &str) -> usize {
    function_help::CATEGORIES.iter().position(|(c, _)| *c == nome).unwrap_or(0)
}

impl App {
    pub(super) fn evento_formulas(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Menu(menu) => {
                self.formulas_menu = if self.formulas_menu == Some(menu)
                    && !matches!(menu, MenuFormulas::Categoria(_)) { None } else { Some(menu) };
                // A posição é a do clique, gravada uma vez (como Bordas e Colar).
                self.formulas_menu_x = self.ponteiro_x - 20.;
                // Borda superior do menu logo abaixo do botão clicado: botões
                // grandes (Recentes, Mais) são mais altos que os pequenos.
                let abaixo = if matches!(menu, MenuFormulas::Recentes | MenuFormulas::Mais) { 22. } else { 12. };
                self.formulas_menu_y = self.ponteiro_y + abaixo;
            }
            Evento::FecharMenu => self.formulas_menu = None,
            Evento::UsarNome(nome) => {
                self.formulas_menu = None;
                return self.inserir_no_editor(&nome);
            }
            Evento::RastrearPrecedentes => self.rastrear(true),
            Evento::RastrearDependentes => self.rastrear(false),
            Evento::RemoverSetas => {
                self.rastreio = Rastreio::default();
                self.status = tr!("Setas de rastreio removidas").into();
            }
            Evento::Manual(manual) => {
                self.formulas_menu = None;
                self.config.calculo_manual = manual;
                self.config.gravar();
                self.aplicar_modo_calculo();
                self.status = if manual {
                    tr!("Cálculo manual: F9 calcula a pasta, Shift+F9 só a planilha").into()
                } else { tr!("Cálculo automático").into() };
            }
            Evento::CalcularAgora => {
                let inicio = Instant::now();
                self.workbook.calcular_agora();
                self.status = trf!("Pasta calculada em {} ms", inicio.elapsed().as_millis());
            }
            Evento::CalcularPlanilha => {
                let inicio = Instant::now();
                self.workbook.calcular_aba(self.folha);
                self.status = trf!("Planilha calculada em {} ms", inicio.elapsed().as_millis());
            }
            Evento::SelecionarDerramada => {
                let area = self.workbook.aba(self.folha)
                    .and_then(|a| a.sheet.area_derramada_em(self.linha, self.coluna));
                match area {
                    Some(area) => {
                        self.trocar_selecao(area.r0, area.c0);
                        self.fim_selecao = (area.r1, area.c1);
                        self.atualizar_endereco_da_area(area);
                    }
                    None => self.status = tr!("A célula não faz parte de uma matriz derramada").into(),
                }
            }
            Evento::CriarAbrir => {
                self.formulas_menu = None;
                let area = self.area_selecionada();
                if (area.r0, area.c0) == (area.r1, area.c1) {
                    self.status = tr!("Selecione a tabela com os títulos, não uma célula só").into();
                } else {
                    self.criar_nomes = self.workbook.aba(self.folha)
                        .map(|a| a.sheet.sugerir_bordas(&area));
                }
            }
            Evento::CriarBorda(lado) => {
                if let Some(b) = &mut self.criar_nomes {
                    let campo = match lado { 0 => &mut b.topo, 1 => &mut b.esquerda, 2 => &mut b.base, _ => &mut b.direita };
                    *campo = !*campo;
                }
            }
            Evento::CriarConfirmar => self.criar_nomes_da_selecao(),
            Evento::CriarFechar => self.criar_nomes = None,
        }
        Task::none()
    }

    /// Nomes que a janela vai criar, já com a aba no intervalo.
    fn nomes_previstos(&self) -> Vec<(String, Rect)> {
        let (Some(bordas), Some(aba)) = (self.criar_nomes, self.workbook.aba(self.folha)) else {
            return Vec::new();
        };
        aba.sheet.nomes_da_selecao(&self.area_selecionada(), bordas).into_iter()
            .map(|(nome, mut area)| { area.sheet = Some(aba.sheet.name.clone()); (nome, area) })
            .collect()
    }

    fn criar_nomes_da_selecao(&mut self) {
        let nomes = self.nomes_previstos();
        if nomes.is_empty() {
            self.status = tr!("Nenhum título encontrado nas bordas marcadas").into();
            return;
        }
        // Como no Excel: um nome que já existe passa a apontar para a área nova.
        let quantos = nomes.len();
        for (nome, area) in nomes {
            self.workbook.remover_nome(&nome);
            self.workbook.definir_nome(&nome, area);
        }
        self.alterado = true;
        self.criar_nomes = None;
        self.status = trf!("{quantos} nome(s) criado(s) a partir da seleção");
    }

    fn view_criar_nomes(&self) -> Element<'_, Msg> {
        let b = self.criar_nomes.unwrap_or_default();
        let caixa = |rotulo: &'static str, marcado: bool, lado: u8| -> Element<'_, Msg> {
            checkbox(rotulo, marcado).size(15).text_size(12.5)
                .on_toggle(move |_| Msg::Formulas(Evento::CriarBorda(lado)))
                .width(Length::Fill).into()
        };
        let previstos = self.nomes_previstos();
        let existentes: Vec<String> = self.workbook.nomeados().into_iter().map(|n| n.nome.to_uppercase()).collect();
        let substitui = previstos.iter().filter(|(n, _)| existentes.contains(&n.to_uppercase())).count();
        let lista: Vec<&str> = previstos.iter().map(|(n, _)| n.as_str()).collect();
        let resumo = match lista.len() {
            0 => tr!("Nenhum nome: as bordas marcadas não têm títulos").to_string(),
            n if n <= 6 => trf!("Serão criados: {}", lista.join(", ")),
            n => trf!("Serão criados: {} e mais {}", lista[..6].join(", "), n - 6),
        };
        let mut corpo = column![
            row![
                text(tr!("Criar nomes a partir da seleção")).size(17).font(iced::Font {
                    weight: iced::font::Weight::Semibold, ..Default::default() }),
                Space::with_width(Length::Fill),
                feather_ui::formatacao::dica(
                    button(feather_ui::icones::icone("x", 15.)).padding(4)
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::Formulas(Evento::CriarFechar)),
                    tr!("Fechar (Esc)")),
            ].align_y(Alignment::Center),
            text(tr!("Usar os títulos que estão em:")).size(12).color(COR_TEXTO_MUTED()),
            row![caixa(tr!("Linha superior"), b.topo, 0), caixa(tr!("Coluna esquerda"), b.esquerda, 1)].spacing(10),
            row![caixa(tr!("Linha inferior"), b.base, 2), caixa(tr!("Coluna direita"), b.direita, 3)].spacing(10),
            text(resumo).size(12),
        ].spacing(10);
        if substitui > 0 {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.),
                text(trf!("{substitui} já existe(m) e passa(m) a apontar para a área nova"))
                    .size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(6).align_y(Alignment::Center));
        }
        corpo = corpo.push(row![
            Space::with_width(Length::Fill),
            button(text(tr!("Cancelar")).size(13)).padding([7, 14]).style(estilo_botao_secundario)
                .on_press(Msg::Formulas(Evento::CriarFechar)),
            button(text(tr!("Criar")).size(13)).padding([7, 14]).style(estilo_botao_primario)
                .on_press_maybe((!previstos.is_empty()).then_some(Msg::Formulas(Evento::CriarConfirmar))),
        ].spacing(8));
        container(corpo).width(Length::Fixed(460.)).padding(20).style(estilo_painel_faixa).into()
    }

    pub(super) fn sobrepor_criar_nomes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.criar_nomes.is_none() { return base; }
        let fora = iced::widget::mouse_area(Space::new(Length::Fill, Length::Fill))
            .on_press(Msg::Formulas(Evento::CriarFechar));
        let painel = container(iced::widget::opaque(self.view_criar_nomes()))
            .center_x(Length::Fill).center_y(Length::Fill);
        stack![base, fora, painel].height(Length::Fill).into()
    }

    fn atualizar_endereco_da_area(&mut self, area: Rect) {
        self.endereco = if (area.r0, area.c0) == (area.r1, area.c1) {
            cell_name(area.r0, area.c0)
        } else {
            format!("{}:{}", cell_name(area.r0, area.c0), cell_name(area.r1, area.c1))
        };
    }

    /// O modo de cálculo é preferência da pessoa (como no Excel, vale para a
    /// pasta aberta). Uma pasta nova ou aberta nasce automática no motor:
    /// conferido a cada mensagem, custa uma comparação.
    pub(super) fn aplicar_modo_calculo(&mut self) {
        if self.workbook.calculo_manual() != self.config.calculo_manual {
            self.workbook.definir_calculo_manual(self.config.calculo_manual);
        }
    }

    /// Guarda a função no topo das recentes (sem repetir).
    pub(super) fn lembrar_funcao(&mut self, nome: &str) {
        let nome = nome.trim().to_uppercase();
        if nome.is_empty() { return; }
        let lista = &mut self.config.funcoes_recentes;
        lista.retain(|n| *n != nome);
        lista.insert(0, nome);
        lista.truncate(MAX_RECENTES);
        self.config.gravar();
    }

    /// Insere um nome definido na fórmula em edição, ou começa uma fórmula com ele.
    fn inserir_no_editor(&mut self, texto: &str) -> Task<Msg> {
        if self.editor_formula.ativo && self.edicao.starts_with('=') {
            self.inserir_no_cursor(texto)
        } else {
            self.update(Msg::Digitar(format!("={texto}")))
        }
    }

    /// Inserir > Símbolo: entra onde está o cursor da edição; sem edição,
    /// começa a digitar a célula com ele.
    pub(super) fn inserir_simbolo(&mut self, texto: &str) -> Task<Msg> {
        if self.editor_formula.ativo { self.inserir_no_cursor(texto) } else { self.update(Msg::Digitar(texto.to_owned())) }
    }

    fn inserir_no_cursor(&mut self, texto: &str) -> Task<Msg> {
        {
            let posicao = self.editor_formula.posicao(&self.edicao);
            let indice = self.edicao.char_indices().nth(posicao)
                .map(|(b, _)| b).unwrap_or(self.edicao.len());
            let mut nova = self.edicao.clone();
            nova.insert_str(indice, texto);
            let edicao = self.update(Msg::Editar(nova));
            Task::batch([
                edicao,
                text_input::focus(text_input::Id::new("feathersheet.fx")),
                text_input::move_cursor_to(text_input::Id::new("feathersheet.fx"),
                    posicao + texto.chars().count()),
            ])
        }
    }

    /// As setas só valem para a aba e a versão em que foram traçadas.
    pub(super) fn setas_visiveis(&self) -> &[Seta] {
        let Some(aba) = self.workbook.aba(self.folha) else { return &[] };
        if self.rastreio.aba_id == Some(aba.id) && self.rastreio.versao == aba.versao {
            &self.rastreio.setas
        } else { &[] }
    }

    /// Um nível a mais de setas a partir da célula ativa. Clicar de novo
    /// com a mesma célula ativa avança para o nível seguinte.
    fn rastrear(&mut self, precedentes: bool) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        if self.rastreio.aba_id != Some(aba.id) || self.rastreio.versao != aba.versao {
            self.rastreio = Rastreio { aba_id: Some(aba.id), versao: aba.versao, ..Default::default() };
        }
        let ativa = (self.linha, self.coluna);
        let estado = if precedentes { &self.rastreio.precedentes } else { &self.rastreio.dependentes };
        let fronteira = match estado {
            Some((inicio, fronteira)) if *inicio == ativa => fronteira.clone(),
            _ => vec![ativa],
        };
        let sheet = &aba.sheet;
        let mut novas: Vec<Seta> = Vec::new();
        let mut proxima: Vec<(u32, u32)> = Vec::new();
        for (r, c) in fronteira {
            if precedentes {
                for p in sheet.precedentes(r, c) {
                    let erro = p.aba.is_none() && p.area.area() <= MAX_CELULAS_NIVEL
                        && (p.area.r0..=p.area.r1).any(|lr| (p.area.c0..=p.area.c1).any(|lc|
                            matches!(sheet.get_value(lr, lc), Value::Error(_))));
                    novas.push(Seta { origem: p.area.clone(), destino: (r, c), externa: p.aba.is_some(), erro });
                    if p.aba.is_none() && p.area.area() <= MAX_CELULAS_NIVEL {
                        for lr in p.area.r0..=p.area.r1 {
                            for lc in p.area.c0..=p.area.c1 {
                                if sheet.is_formula(lr, lc) { proxima.push((lr, lc)); }
                            }
                        }
                    }
                }
            } else {
                for d in sheet.dependentes(r, c) {
                    novas.push(Seta {
                        origem: Rect::new(r, c, r, c), destino: d, externa: false,
                        erro: matches!(sheet.get_value(r, c), Value::Error(_)),
                    });
                    proxima.push(d);
                }
            }
        }
        novas.retain(|s| !self.rastreio.setas.contains(s));
        novas.dedup();
        proxima.sort_unstable();
        proxima.dedup();
        let quantas = novas.len();
        self.rastreio.setas.extend(novas);
        let estado = Some((ativa, proxima));
        if precedentes { self.rastreio.precedentes = estado } else { self.rastreio.dependentes = estado }
        self.status = match (quantas, precedentes) {
            (0, true) => tr!("Nenhum precedente a mais para mostrar").into(),
            (0, false) => tr!("Nenhum dependente a mais para mostrar").into(),
            (n, true) => trf!("{n} seta(s) de precedentes; clique de novo para o próximo nível"),
            (n, false) => trf!("{n} seta(s) de dependentes; clique de novo para o próximo nível"),
        };
    }

    /// Botão pequeno de menu: ícone, rótulo e a seta de lista.
    fn formulas_pequeno_menu(&self, rotulo: &str, icone: &str, menu: MenuFormulas) -> Element<'_, Msg> {
        self.formulas_pequeno_menu_se(rotulo, icone, menu, true)
    }

    fn formulas_pequeno_menu_se(&self, rotulo: &str, icone: &str, menu: MenuFormulas, ligado: bool) -> Element<'_, Msg> {
        button(row![
            feather_ui::icones::icone(icone, 17.),
            text(rotulo.to_owned()).size(11).wrapping(text::Wrapping::None),
            feather_ui::icones::icone("chevron-down", 11.),
        ].spacing(5).align_y(Alignment::Center))
            .padding([2, 6])
            .style(estilo_botao_pequeno(self.formulas_menu == Some(menu)))
            .on_press_maybe(ligado.then_some(Msg::Formulas(Evento::Menu(menu))))
            .into()
    }

    pub(super) fn grupos_formulas(&self) -> Vec<Element<'_, Msg>> {
        let menu = |m| Some(Msg::Formulas(Evento::Menu(m)));
        let aberto = |m| self.formulas_menu == Some(m);
        let categoria = |i: usize| {
            let (nome, rotulo, icone) = CATEGORIAS_FAIXA[i];
            self.formulas_pequeno_menu(tr!(rotulo), icone, MenuFormulas::Categoria(indice_categoria(nome)))
        };
        let biblioteca = self.faixa_linha(vec![
            self.faixa_grande("AutoSoma", "sum", Some(Msg::AutoSoma), false),
            self.faixa_grande_seta(tr!("Recentes"), "recentes", menu(MenuFormulas::Recentes),
                aberto(MenuFormulas::Recentes)),
            self.faixa_pilha(vec![categoria(0), categoria(1), categoria(2)]),
            self.faixa_pilha(vec![categoria(3), categoria(4), categoria(5)]),
            self.faixa_grande_seta(tr!("Mais"), "math-function", menu(MenuFormulas::Mais),
                aberto(MenuFormulas::Mais)),
        ]);
        let tem_nomes = !self.workbook.nomeados().is_empty();
        let tem_setas = !self.setas_visiveis().is_empty();
        vec![
            self.faixa_grupo(tr!("Biblioteca de funções"), biblioteca),
            self.faixa_grupo(tr!("Nomes definidos"), self.faixa_linha(vec![
                self.faixa_grande(tr!("Gerenciador"), "tag", Some(Msg::NomesAbrir(false)),
                    self.nomes_modal.is_some()),
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Definir nome"), "edit", Some(Msg::NomesAbrir(true)), false),
                    self.formulas_pequeno_menu_se(tr!("Usar em fórmula"), "list-select",
                        MenuFormulas::Nomes, tem_nomes),
                    self.faixa_pequeno(tr!("Criar da seleção"), "table",
                        Some(Msg::Formulas(Evento::CriarAbrir)), self.criar_nomes.is_some()),
                ]),
            ])),
            self.faixa_grupo(tr!("Auditoria de fórmulas"), self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    feather_ui::formatacao::dica(self.faixa_pequeno(tr!("Precedentes"), "arrow-right",
                        Some(Msg::Formulas(Evento::RastrearPrecedentes)), false), tr!("Rastrear precedentes")),
                    feather_ui::formatacao::dica(self.faixa_pequeno(tr!("Dependentes"), "sitemap",
                        Some(Msg::Formulas(Evento::RastrearDependentes)), false), tr!("Rastrear dependentes")),
                    self.faixa_pequeno(tr!("Remover setas"), "eraser",
                        tem_setas.then_some(Msg::Formulas(Evento::RemoverSetas)), false),
                ]),
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Mostrar fórmulas"), "code",
                        Some(Msg::AlternarMostrarFormulas), self.mostrar_formulas),
                    self.faixa_pequeno(tr!("Avaliar fórmula"), "math-function",
                        Some(Msg::Avaliar(avaliar_ui::Evento::Abrir)), self.avaliar.is_some()),
                    self.faixa_pequeno(tr!("Explicar erro"), "info-circle",
                        Some(Msg::ExplicacaoErroAbrir), false),
                ]),
                feather_ui::formatacao::dica(self.faixa_grande(tr!("Inspeção"), "eye",
                    Some(Msg::Inspecao(inspecao_ui::Evento::Abrir)),
                    self.janelas.aberta(Auxiliar::Inspecao)), tr!("Janela de inspeção")),
            ])),
            self.faixa_grupo(tr!("Cálculo"), self.faixa_pilha(vec![
                self.formulas_pequeno_menu(tr!("Opções de cálculo"), "settings", MenuFormulas::Calculo),
                self.faixa_pequeno(tr!("Calcular agora"), "refresh",
                    Some(Msg::Formulas(Evento::CalcularAgora)), false),
                self.faixa_pequeno(tr!("Calcular planilha"), "table",
                    Some(Msg::Formulas(Evento::CalcularPlanilha)), false),
            ])),
        ]
    }

    /// Linha de menu: ícone opcional, título e uma descrição apagada.
    fn item_menu<'a>(titulo: String, detalhe: String, msg: Msg, ativo: bool) -> Element<'a, Msg> {
        button(column![
            text(titulo).size(12.5),
            text(detalhe).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(1))
            .width(Length::Fill).padding([5, 10])
            .style(estilo_botao_pequeno(ativo))
            .on_press(msg).into()
    }

    fn conteudo_menu_formulas(&self, menu: MenuFormulas) -> (String, Element<'_, Msg>) {
        let mut lista = column![].spacing(1);
        let titulo = match menu {
            MenuFormulas::Categoria(i) => {
                let (categoria, nomes) = function_help::CATEGORIES[i.min(function_help::CATEGORIES.len() - 1)];
                let mut ordenados: Vec<&str> = nomes.to_vec();
                ordenados.sort_by_key(|n| nome_exibido(n).to_lowercase());
                for canonico in ordenados {
                    let nome = nome_exibido(canonico);
                    lista = lista.push(Self::item_menu(nome.clone(), resumo(canonico),
                        Msg::InserirFuncao(nome), false));
                }
                crate::idioma::categoria_funcao(categoria).to_owned()
            }
            MenuFormulas::Mais => {
                let na_faixa: Vec<&str> = CATEGORIAS_FAIXA.iter().map(|c| c.0).collect();
                for (i, (categoria, nomes)) in function_help::CATEGORIES.iter().enumerate() {
                    if na_faixa.contains(categoria) { continue; }
                    lista = lista.push(button(row![
                        text(crate::idioma::categoria_funcao(categoria)).size(12.5),
                        Space::with_width(Length::Fill),
                        text(format!("{}", nomes.len())).size(11).color(COR_TEXTO_MUTED()),
                        feather_ui::icones::icone("chevron-right", 14.),
                    ].spacing(8).align_y(Alignment::Center))
                        .width(Length::Fill).padding([7, 10])
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::Formulas(Evento::Menu(MenuFormulas::Categoria(i)))));
                }
                lista = lista.push(Space::with_height(6)).push(button(row![
                    feather_ui::icones::icone("search", 15.),
                    text(tr!("Todas as funções na Ajuda")).size(12.5),
                ].spacing(8).align_y(Alignment::Center))
                    .width(Length::Fill).padding([7, 10])
                    .style(estilo_botao_pequeno(false)).on_press(Msg::Ajuda));
                tr!("Mais funções").to_owned()
            }
            MenuFormulas::Recentes => {
                if self.config.funcoes_recentes.is_empty() {
                    lista = lista.push(text(tr!("Nenhuma função usada ainda. As funções inseridas pela faixa aparecem aqui."))
                        .size(12).color(COR_TEXTO_MUTED()));
                }
                for nome in &self.config.funcoes_recentes {
                    let canonico = functions::canonical(nome).unwrap_or(nome);
                    lista = lista.push(Self::item_menu(nome.clone(), resumo(canonico),
                        Msg::InserirFuncao(nome.clone()), false));
                }
                tr!("Usadas recentemente").to_owned()
            }
            MenuFormulas::Nomes => {
                for n in self.workbook.nomeados() {
                    lista = lista.push(Self::item_menu(n.nome.clone(), format!("{}:{}", cell_name(n.area.r0, n.area.c0), cell_name(n.area.r1, n.area.c1)),
                        Msg::Formulas(Evento::UsarNome(n.nome.clone())), false));
                }
                tr!("Usar em fórmula").to_owned()
            }
            MenuFormulas::Calculo => {
                let manual = self.config.calculo_manual;
                lista = lista
                    .push(Self::item_menu(tr!("Automático").into(),
                        tr!("Recalcula as fórmulas a cada mudança").into(),
                        Msg::Formulas(Evento::Manual(false)), !manual))
                    .push(Self::item_menu(tr!("Manual").into(),
                        tr!("Só calcula com F9 (pasta) ou Shift+F9 (planilha)").into(),
                        Msg::Formulas(Evento::Manual(true)), manual));
                tr!("Opções de cálculo").to_owned()
            }
        };
        (titulo, lista.into())
    }

    /// Menu suspenso ancorado onde foi clicado, como o de Bordas.
    pub(super) fn sobrepor_menu_formulas<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(menu) = self.formulas_menu else { return base };
        if self.faixa_ativa != AbaFaixa::Formulas { return base; }
        let (titulo, lista) = self.conteudo_menu_formulas(menu);
        let largura = 340.;
        let altura_max = (self.tamanho_principal.height - 220.).clamp(160., 420.);
        let caixa = container(column![
            row![
                text(titulo).size(14),
                Space::with_width(Length::Fill),
                feather_ui::formatacao::dica(
                    button(feather_ui::icones::icone("x", 14.))
                        .padding(4).style(estilo_botao_pequeno(false))
                        .on_press(Msg::Formulas(Evento::FecharMenu)),
                    tr!("Fechar (Esc)")),
            ].align_y(Alignment::Center),
            container(feather_ui::tema::rolagem(lista)).max_height(altura_max),
        ].spacing(8))
            .padding(10).width(Length::Fixed(largura))
            .style(estilo_popup_menu);
        let x = self.formulas_menu_x.min((self.tamanho_principal.width - largura - 8.).max(8.)).max(8.);
        let y = self.formulas_menu_y.min((self.tamanho_principal.height - altura_max - 60.).max(8.)).max(8.);
        let fora = iced::widget::mouse_area(Space::new(Length::Fill, Length::Fill))
            .on_press(Msg::Formulas(Evento::FecharMenu));
        stack![
            base, fora,
            column![
                Space::with_height(Length::Fixed(y)),
                row![Space::with_width(Length::Fixed(x)), iced::widget::opaque(caixa)],
            ],
        ].height(Length::Fill).into()
    }

    /// Aviso "Calcular" no rodapé: só aparece no modo manual com conta esperando.
    pub(super) fn aviso_calcular(&self) -> Element<'_, Msg> {
        if !(self.config.calculo_manual && self.workbook.calculo_pendente_manual()) {
            return Space::with_width(0).into();
        }
        feather_ui::formatacao::dica(
            button(text(tr!("Calcular")).size(11))
                .padding([4, 9]).style(estilo_botao_pequeno(true))
                .on_press(Msg::Formulas(Evento::CalcularAgora)),
            tr!("Há fórmulas por calcular (F9)"),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com(celulas: &[(u32, u32, &str)]) -> App {
        let mut app = App::default();
        for &(r, c, v) in celulas {
            app.workbook.escrever_celula(0, r, c, v.into());
        }
        app
    }

    #[test]
    fn precedentes_avancam_um_nivel_por_clique() {
        let mut app = app_com(&[(0, 0, "1"), (0, 1, "=A1*2"), (0, 2, "=B1+A1")]);
        app.linha = 0; app.coluna = 2;
        let _ = app.update(Msg::Formulas(Evento::RastrearPrecedentes));
        assert_eq!(app.setas_visiveis().len(), 2, "C1 lê B1 e A1");
        let _ = app.update(Msg::Formulas(Evento::RastrearPrecedentes));
        // B1 lê A1: a seta A1→B1 entra no segundo nível.
        assert_eq!(app.setas_visiveis().len(), 3);
        assert!(app.setas_visiveis().iter().any(|s| s.destino == (0, 1)));
        let _ = app.update(Msg::Formulas(Evento::RemoverSetas));
        assert!(app.setas_visiveis().is_empty());
    }

    #[test]
    fn dependentes_e_setas_somem_quando_a_planilha_muda() {
        let mut app = app_com(&[(0, 0, "1"), (1, 0, "=A1+1"), (2, 0, "=SOMA(A1:A2)")]);
        let _ = app.update(Msg::Formulas(Evento::RastrearDependentes));
        let destinos: Vec<_> = app.setas_visiveis().iter().map(|s| s.destino).collect();
        assert!(destinos.contains(&(1, 0)) && destinos.contains(&(2, 0)), "{destinos:?}");
        app.workbook.escrever_celula(0, 5, 5, "x".into());
        assert!(app.setas_visiveis().is_empty());
    }

    #[test]
    fn precedente_com_erro_e_de_outra_aba_sao_marcados() {
        let mut app = app_com(&[(0, 0, "=1/0"), (0, 1, "=A1+Plan2!A1")]);
        app.workbook.criar_aba("Plan2");
        app.linha = 0; app.coluna = 1;
        let _ = app.update(Msg::Formulas(Evento::RastrearPrecedentes));
        let setas = app.setas_visiveis();
        assert!(setas.iter().any(|s| s.erro && !s.externa));
        assert!(setas.iter().any(|s| s.externa));
    }

    #[test]
    fn modo_manual_espera_o_f9_e_mostra_calcular() {
        let mut app = app_com(&[(0, 0, "2"), (0, 1, "=A1*10")]);
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Number(20.));
        let _ = app.update(Msg::Formulas(Evento::Manual(true)));
        assert!(app.workbook.calculo_manual());
        app.workbook.escrever_celula(0, 0, 0, "3".into());
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Number(20.));
        assert!(app.workbook.calculo_pendente_manual());
        let _: Element<'_, Msg> = app.aviso_calcular();
        let _ = app.update(Msg::Formulas(Evento::CalcularAgora));
        assert_eq!(app.workbook.valor(0, 0, 1), Value::Number(30.));
        // Pasta nova herda a preferência.
        app.workbook = feathersheet_core::planilha::workbook::Workbook::new();
        let _ = app.update(Msg::Formulas(Evento::FecharMenu));
        assert!(app.workbook.calculo_manual());
        let _ = app.update(Msg::Formulas(Evento::Manual(false)));
        assert!(!app.workbook.calculo_manual());
    }

    #[test]
    fn inserir_funcao_guarda_recentes_sem_repetir() {
        let mut app = App::default();
        let _ = app.update(Msg::InserirFuncao("SOMA".into()));
        let _ = app.update(Msg::Cancelar);
        let _ = app.update(Msg::InserirFuncao("PROCX".into()));
        let _ = app.update(Msg::Cancelar);
        let _ = app.update(Msg::InserirFuncao("SOMA".into()));
        assert_eq!(app.config.funcoes_recentes[..2], ["SOMA".to_string(), "PROCX".to_string()]);
    }

    #[test]
    fn ctrl_barra_seleciona_a_area_derramada() {
        let mut app = app_com(&[(0, 0, "=SEQUÊNCIA(3;2)")]);
        let _ = app.workbook.valor(0, 0, 0);
        app.linha = 1; app.coluna = 1;
        let _ = app.update(Msg::Formulas(Evento::SelecionarDerramada));
        assert_eq!(app.area_selecionada(), Rect::new(0, 0, 2, 1));
    }

    #[test]
    fn criar_da_selecao_usa_os_titulos_e_substitui_nome_existente() {
        let mut app = app_com(&[(0, 1, "Preço"), (0, 2, "Qtd"), (1, 0, "Jan"), (1, 1, "10"), (1, 2, "3"),
            (2, 0, "Fev"), (2, 1, "12"), (2, 2, "4")]);
        app.workbook.definir_nome("Preço", Rect::new(9, 9, 9, 9));
        app.linha = 0; app.coluna = 0; app.fim_selecao = (2, 2);
        let _ = app.update(Msg::Formulas(Evento::CriarAbrir));
        assert_eq!(app.criar_nomes, Some(Bordas { topo: true, esquerda: true, ..Default::default() }));
        let _: Element<'_, Msg> = app.view_criar_nomes();
        let _ = app.update(Msg::Formulas(Evento::CriarBorda(1)));
        let _ = app.update(Msg::Formulas(Evento::CriarConfirmar));
        assert!(app.criar_nomes.is_none());
        let nomes = app.workbook.nomeados();
        assert_eq!(nomes.len(), 2, "só as colunas: a esquerda foi desmarcada");
        let preco = nomes.iter().find(|n| n.nome == "Preço").unwrap();
        assert_eq!((preco.area.r0, preco.area.c0, preco.area.r1, preco.area.c1), (1, 1, 2, 1));
        app.workbook.escrever_celula(0, 5, 5, "=SOMA(Preço)".into());
        assert_eq!(app.workbook.valor(0, 5, 5), Value::Number(22.));
    }

    #[test]
    fn menu_fica_onde_abriu_e_nao_segue_o_mouse() {
        let mut app = app_com(&[]);
        app.faixa_ativa = AbaFaixa::Formulas;
        app.ponteiro_x = 300.;
        let _ = app.update(Msg::Formulas(Evento::Menu(MenuFormulas::Categoria(0))));
        assert_eq!(app.formulas_menu_x, 280.);
        // Passar o mouse pela lista não move o menu.
        app.ponteiro_x = 600.;
        assert_eq!(app.formulas_menu_x, 280.);
    }

    #[test]
    fn todos_os_menus_montam() {
        let mut app = app_com(&[(0, 0, "1")]);
        app.faixa_ativa = AbaFaixa::Formulas;
        let mut menus = vec![MenuFormulas::Mais, MenuFormulas::Recentes, MenuFormulas::Nomes, MenuFormulas::Calculo];
        menus.extend((0..function_help::CATEGORIES.len()).map(MenuFormulas::Categoria));
        for m in menus {
            app.formulas_menu = Some(m);
            let base: Element<'_, Msg> = Space::new(Length::Fill, Length::Fill).into();
            let _ = app.sobrepor_menu_formulas(base);
        }
        for (categoria, _, icone) in CATEGORIAS_FAIXA {
            assert!(function_help::CATEGORIES.iter().any(|(c, _)| *c == categoria), "{categoria}");
            assert!(feather_ui::icones::existe(icone), "{icone}");
        }
        for icone in ["sum", "list", "math-function", "tag", "edit", "list-select", "arrow-right",
            "sitemap", "eraser", "code", "info-circle", "settings", "refresh", "table"] {
            assert!(feather_ui::icones::existe(icone), "{icone}");
        }
    }
}
