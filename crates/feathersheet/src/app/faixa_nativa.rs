//! Faixa de opções nativa do FeatherSheet: estrutura e ordem herdadas de ribbon.ts.
//! Usa os botões e grupos de feather-ui; comandos de planilha permanecem no App.
use super::*;
use pelicano::column;
use feather_ui::componentes as ui;
use super::faixa_responsiva::{self, Nivel};

// A faixa não usa scroll horizontal. Acessos excedentes ficam no menu Mais,
// enquanto a altura compacta permanece estável entre abas e redimensionamentos.
pub(super) const ALTURA_FAIXA_NATIVA: f32 = 112.0;
pub(super) const ALTURA_CONTROLES_FAIXA: f32 = 100.0;
pub(super) const RESERVA_BOTAO_MAIS: f32 = 108.0;

thread_local! {
    /// Enquanto `Some`, `faixa_grupo` anota o título de cada grupo montado (INT-1: as abas
    /// fora da Página Inicial recolhem grupo a grupo e o botão recolhido mostra o nome).
    static TITULOS_GRUPOS: std::cell::RefCell<Option<Vec<&'static str>>> = const { std::cell::RefCell::new(None) };
    /// O primeiro ícone de botão grande e o primeiro de qualquer botão montados desde o último grupo.
    static ICONE_VISTO: std::cell::RefCell<(Option<String>, Option<String>)> = const { std::cell::RefCell::new((None, None)) };
    /// Ícone de cada grupo (pelo título), para o botão do grupo recolhido não ser sempre o mesmo.
    static ICONES_GRUPOS: std::cell::RefCell<std::collections::HashMap<&'static str, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Anota o ícone de um botão da faixa: o grupo que o contém usa o primeiro como o seu.
fn notar_icone(icone: &str, grande: bool) {
    ICONE_VISTO.with_borrow_mut(|(g, q)| {
        let alvo = if grande { g } else { q };
        if alvo.is_none() { *alvo = Some(icone.to_owned()); }
    });
}

/// Fecha um grupo: guarda o ícone dele (o do primeiro botão grande; sem um, o do primeiro botão).
fn guardar_icone_do_grupo(titulo: &'static str) {
    let (grande, qualquer) = ICONE_VISTO.with_borrow_mut(std::mem::take);
    if let Some(icone) = grande.or(qualquer) {
        ICONES_GRUPOS.with_borrow_mut(|m| { m.insert(titulo, icone); });
    }
}

pub(super) const FONTES: &[&str] = &["Padrão", "Serifada", "Monoespaçada", "Arial", "Georgia", "Courier New", "Verdana", "Times New Roman"];
pub(super) const TAMANHOS: &[&str] = &["6", "8", "9", "10", "11", "12", "13", "14", "16", "18", "20", "24", "28", "32", "36", "48", "72", "96"];
pub(super) const FORMATOS: &[&str] = &["Geral", "1.234,56", "1.235", "R$ 1.234,56", "12,34%", "Data", "Data e hora", "Hora", "Texto", "Mais formatos de número…"];
pub(super) const ZOOMS: &[&str] = &["25%", "50%", "75%", "90%", "100%", "125%", "150%", "200%", "250%", "300%", "400%"];
pub(super) const FUNCOES: &[&str] = &["SOMA", "MÉDIA", "COUNT", "MAX", "MIN"];
/// "Todas" + as categorias do catálogo do motor, na mesma ordem dos
/// catálogos montados abaixo. Era uma lista fixa de 8 nomes: quando o motor
/// ganhou Banco de dados, Engenharia e Financeiras, o índice escolhido
/// passou a abrir o catálogo de outra categoria.
pub(super) static CATEGORIAS: std::sync::LazyLock<Vec<&'static str>> =
    std::sync::LazyLock::new(|| std::iter::once("Todas")
        .chain(feathersheet_core::funcoes::function_help::CATEGORIES.iter().map(|(nome, _)| *nome))
        .collect());
pub(super) const LINHAS: &[&str] = &["Inserir acima", "Inserir abaixo", "Excluir linhas"];
pub(super) const COLUNAS: &[&str] = &["Inserir à esquerda", "Inserir à direita", "Excluir colunas"];
pub(super) const TEMA: &[&str] = &["Claro", "Escuro"];
pub(super) const NUM_FORMAT: &str = "Número";

/// Um catálogo por categoria: a ComboBox mantém o filtro de texto e a
/// navegação por teclado em estado próprio. Inclui nomes em português sem
/// eliminar os nomes canônicos que já eram exibidos no menu anterior.
pub(super) fn montar_catalogos_funcoes() -> Vec<Vec<String>> {
    use feathersheet_core::funcoes::{function_help, functions};
    let categorias = std::iter::once(
        function_help::all().iter().map(|(nome, _)| (*nome).to_owned()).collect::<Vec<_>>()
    ).chain(function_help::CATEGORIES.iter().map(|(_, nomes)|
        nomes.iter().map(|nome| (*nome).to_owned()).collect::<Vec<_>>()
    ));

    categorias.map(|mut nomes| {
        for &(apelido, canonico) in functions::aliases() {
            if nomes.iter().any(|nome| nome == canonico)
                && !nomes.iter().any(|nome| nome == apelido)
            {
                nomes.push(apelido.to_owned());
            }
        }
        nomes.sort_by_key(|nome| nome.to_lowercase());
        nomes.dedup();
        nomes
    }).collect()
}

impl App {
    pub(super) fn faixa_pequeno(&self, rotulo: &str, icone: &str, msg: Option<Msg>, ativo: bool) -> Element<'_, Msg> {
        notar_icone(icone, false);
        ui::botao_pequeno(feather_ui::icones::icone(icone, 17.), rotulo, msg, ativo)
    }

    /// Ações de edição da Página Inicial: traço vetorial colorido, sem mudar
    /// a apresentação neutra das demais abas ou dos outros editores.
    pub(super) fn faixa_pequeno_colorido(
        &self, rotulo: &str, icone: &str, cor: &str, msg: Option<Msg>, ativo: bool,
    ) -> Element<'_, Msg> {
        ui::botao_pequeno(
            feather_ui::icones::icone_colorido(icone, 17., cor), rotulo, msg, ativo,
        )
    }

    pub(super) fn faixa_icone(&self, dica: &str, icone: &str, msg: Option<Msg>, ativo: bool) -> Element<'_, Msg> {
        notar_icone(icone, false);
        ui::botao_icone(feather_ui::icones::icone(icone, 17.), dica, msg, ativo)
    }

    pub(super) fn faixa_cor(&self, alvo: AlvoCor) -> Element<'_, Msg> {
        let hex = self.cor_da_celula(alvo);
        let cor = if feather_ui::seletor_cor::hex_valido(&hex) {
            let [r, g, b, a] = feather_ui::seletor_cor::rgba_de(&hex);
            pelicano::Color::rgb8(r, g, b).alpha(a as f32 / 255.0)
        } else {
            match alvo {
                AlvoCor::Fundo => COR_PAINEL(),
                AlvoCor::Texto => COR_TEXTO(),
            }
        };
        let (icone, dica) = match alvo {
            AlvoCor::Fundo => ("bucket", tr!("Cor de preenchimento da célula")),
            AlvoCor::Texto => ("font-color", tr!("Cor do texto")),
        };
        let amostra = container(Space::new(0.0, 4.0))
            .width(20.0)
            .style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(cor)),
                border: pelicano::Border {
                    color: COR_BORDA(),
                    width: 0.6,
                    radius: [1.0; 4],
                },
                ..Default::default()
            });
        feather_ui::formatacao::dica(
            button(
                column![feather_ui::icones::icone(icone, 18.0), amostra]
                    .spacing(1)
                    .align(pelicano::Align::Center),
            )
            .width(32.0)
            .padding([2, 5])
            .style(estilo_botao_pequeno(self.cor_ativa == Some(alvo)))
            .on_press(Msg::AbrirSeletorCor(alvo)),
            dica,
        )
    }

    /// Controles de tamanho com A+/A−, sem duplicar os ícones +/− ao lado da fonte.
    pub(super) fn faixa_tamanho_acao(
        &self,
        _rotulo: &'static str,
        dica: &'static str,
        delta: i32,
    ) -> Element<'_, Msg> {
        self.faixa_icone(
            dica,
            if delta > 0 {
                "font-increase"
            } else {
                "font-decrease"
            },
            Some(Msg::TamanhoFonteDelta(delta)),
            false,
        )
    }

    /// Sublinhado usa o SVG compartilhado da Ribbon.
    /// Botão dividido, como no Excel: o ícone repete o último sublinhado
    /// (simples ou duplo, conforme a célula) e a seta abre Sublinhado ▾.
    pub(super) fn faixa_sublinhado(&self, ativo: bool) -> Element<'_, Msg> {
        use feathersheet_core::operacoes::sublinhado::{sublinhado_de, Sublinhado, CHAVE_DUPLO};
        let duplo = self.workbook.aba(self.folha)
            .is_some_and(|a| sublinhado_de(&a.sheet, self.linha, self.coluna) == Sublinhado::Duplo);
        let (dica, icone, chave) = if duplo {
            (tr!("Sublinhado duplo"), "underline-double", CHAVE_DUPLO)
        } else {
            (tr!("Sublinhado (Ctrl+U)"), "underline", "underline")
        };
        row![
            self.faixa_icone(dica, icone, Some(Msg::AlternarEstilo(chave.into())), ativo),
            feather_ui::formatacao::dica(
                button(feather_ui::icones::icone("chevron-down", 11.))
                    .padding([6, 1])
                    .style(estilo_botao_pequeno(self.edicao_menu == Some("sublinhado")))
                    .on_press(Msg::AlternarMenuEdicao("sublinhado")),
                tr!("Opções de sublinhado"),
            ),
        ].spacing(0).align(pelicano::Align::Center).into()
    }

    /// Botão único de bordas: ícone com seta abre menu flutuante com prévias.
    /// Não usa pick_list estreita nem aplica "Todas" ao clicar no ícone.
    pub(super) fn faixa_bordas(&self) -> Element<'_, Msg> {
        feather_ui::formatacao::dica(
            button(row![
                feather_ui::icones::icone("border-all", 17.0),
                feather_ui::icones::icone("chevron-down", 12.0),
            ].spacing(3).align(pelicano::Align::Center))
                .padding([4, 6])
                .style(estilo_botao_pequeno(self.bordas_menu_aberto || self.borda_desenho.modo.is_some()))
                .on_press(Msg::AlternarMenuBordas),
            tr!("Bordas"),
        )
    }

    /// Alinhamento vertical usa os SVGs compartilhados da Ribbon.
    /// Só os ícones ocupam a Ribbon; as descrições aparecem no tooltip.
    pub(super) fn faixa_vertical(&self, posicao: &'static str, ativo: bool) -> Element<'_, Msg> {
        let (icone, dica, valor) = match posicao {
            "top" => ("align-top", tr!("Alinhar texto no topo"), "top"),
            "bottom" => ("align-bottom", tr!("Alinhar texto na base"), "bottom"),
            _ => ("align-middle", tr!("Centralizar texto verticalmente"), "middle"),
        };
        self.faixa_icone(
            dica,
            icone,
            Some(Msg::EstiloMenu("vAlign".into(), Campo::Texto(valor.into()))),
            ativo,
        )
    }

    /// Menu suspenso de orientação: ícone + texto + seta dentro de um botão.
    pub(super) fn faixa_inclinar_texto(&self, ativo: bool) -> Element<'_, Msg> {
        let icone = feather_ui::icones::icone("text-rotate", 16.);
        button(row![
            icone, text(tr!("Inclinar")).size(11),
            feather_ui::icones::icone("chevron-down", 12.)
        ].spacing(3).align(pelicano::Align::Center))
        .padding([3,5])
        .style(estilo_botao_pequeno(ativo || self.orientacao_menu_aberto))
        .on_press(Msg::AlternarMenuOrientacao)
        .into()
    }

    pub(super) fn faixa_grande(&self, rotulo: &str, icone: &str, msg: Option<Msg>, ativo: bool) -> Element<'_, Msg> {
        notar_icone(icone, true);
        ui::botao_grande(feather_ui::icones::icone(icone, 23.), rotulo, msg, ativo)
    }

    /// Ícone em placa suave de cor sem recurso gráfico externo.
    /// O botão continua enviando exatamente a mensagem original da Ribbon.
    pub(super) fn faixa_grande_destaque(
        &self, rotulo: &str, icone: &str, cor: &str,
        fundo: (u8, u8, u8), msg: Option<Msg>, ativo: bool,
    ) -> Element<'_, Msg> {
        // Tons mais profundos no tema escuro, sem fundo claro ofuscante.
        let superficie = if self.config.escuro {
            match cor {
                "#21865c" => pelicano::Color::rgb8(29, 66, 52),
                "#2179a7" => pelicano::Color::rgb8(30, 57, 74),
                "#d68126" => pelicano::Color::rgb8(75, 54, 29),
                "#7d5cc6" => pelicano::Color::rgb8(58, 45, 82),
                _ => pelicano::Color::rgb8(79, 44, 49),
            }
        } else {
            pelicano::Color::rgb8(fundo.0, fundo.1, fundo.2)
        };
        // Placa menor que a de antes (25 px + 7): com rótulo de duas linhas
        // ("Formatação\ncondicional") a segunda linha era cortada pela faixa.
        let simbolo = container(feather_ui::icones::icone_colorido(icone, 22., cor))
            .padding(5)
            .style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(superficie)),
                border: pelicano::Border {radius: [8.0; 4], ..Default::default()},
                ..Default::default()
            });
        button(pelicano::column![
            simbolo,
            text(rotulo.to_owned()).size(11)
                .align_x(pelicano::Align::Center),
        ].spacing(2).align(pelicano::Align::Center))
            .padding([2, 7])
            .style(estilo_botao_grande(ativo))
            .on_press_maybe(msg)
            .into()
    }

    /// Ícone de Inserir com seta SVG integrada: sem PickList/dropdown.
    pub(super) fn faixa_grande_seta(&self, rotulo: &str, icone: &str, msg: Option<Msg>, ativo: bool) -> Element<'_, Msg> {
        notar_icone(icone, true);
        button(pelicano::column![
            feather_ui::icones::icone(icone, 23.0),
            pelicano::row![
                text(rotulo.to_owned()).size(11).no_wrap(),
                feather_ui::icones::icone("chevron-down", 12.0),
            ].spacing(3).align(pelicano::Align::Center),
        ].spacing(3).align(pelicano::Align::Center))
            .padding([4, 8])
            .style(estilo_botao_grande(ativo))
            .on_press_maybe(msg)
            .into()
    }

    /// Colar é um botão dividido: ícone = colar padrão, seta = menu flutuante.
    /// A seta é SVG Tabler, não uma PickList/dropdown que ocupa a faixa.
    pub(super) fn faixa_botao_colar(&self) -> Element<'_, Msg> {
        pelicano::column![
            button(pelicano::column![
                feather_ui::icones::icone_colorido("clipboard", 25.0, "#d68126"),
                text(tr!("Colar")).size(11),
            ].spacing(3).align(pelicano::Align::Center))
                .width(Length::Fixed(68.0))
                .padding([4, 8])
                .style(estilo_botao_grande(false))
                .on_press(Msg::ColarSolicitar),
            button(feather_ui::icones::icone("chevron-down", 14.0))
                .width(Length::Fixed(68.0))
                .padding([1, 8])
                .style(estilo_botao_pequeno(self.colar_menu_aberto))
                .on_press(Msg::AlternarMenuColagem),
        ].spacing(0).into()
    }

    /// Botão de gráfico com dois SVG reais (gráfico + chevron), sem
    /// usar o caractere ▾ que algumas fontes substituem por um quadrado.
    pub(super) fn faixa_grafico(&self) -> Element<'_, Msg> {
        crate::compat::button(pelicano::column![
            feather_ui::icones::icone("chart-bar", 23.0),
            pelicano::row![
                pelicano::text(tr!("Gráfico")).size(11),
                feather_ui::icones::icone("chevron-down", 12.0),
            ].spacing(3).align(pelicano::Align::Center),
        ].spacing(3).align(pelicano::Align::Center))
        .padding([4, 8])
        .style(estilo_botao_grande(self.graficos.seletor_aberto))
        .on_press(Msg::Grafico(graficos::Evento::Abrir))
        .into()
    }

    /// `pick_list` é menu suspenso nativo: mouse, hover, foco, teclado, Esc,
    /// escolha e fechamento são tratados pelo widget do Iced, sem WebView.
    pub(super) fn faixa_lista(
        &self, rotulo: &'static str, id: &'static str, opcoes: &'static [&'static str],
        selecionado: Option<&'static str>, largura: f32,
    ) -> Element<'_, Msg> {
        // A lista mostra o texto traduzido; a mensagem leva a opção em português.
        ui::lista_suspensa(
            rotulo, crate::idioma::lista(opcoes), selecionado.map(|s| crate::idioma::tr(s)),
            move |opcao| Msg::FaixaOpcao(id, crate::idioma::chave_de(opcoes, opcao).unwrap_or(opcao)), largura,
        )
    }

    pub(super) fn faixa_grupo<'a>(&self, titulo: &'static str, elementos: Element<'a, Msg>) -> Element<'a, Msg> {
        TITULOS_GRUPOS.with_borrow_mut(|t| if let Some(t) = t { t.push(titulo) });
        guardar_icone_do_grupo(titulo);
        // Todos os grupos, não apenas Fonte: mesmo espaço para legenda.
        // O antigo grupo_faixa com Height::Fill deixava os títulos invadirem
        // a rolagem horizontal no rodapé (ex.: "Alinhamento").
        ui::grupo_faixa_compacto(elementos, crate::idioma::tr(titulo))
    }

    /// Grupo com a setinha ↘ do Office, que abre Formatar células (Ctrl+1) na `aba`.
    pub(super) fn faixa_grupo_com_janela<'a>(&self, titulo: &'static str, aba: super::formatar_celulas::Aba, elementos: Element<'a, Msg>) -> Element<'a, Msg> {
        TITULOS_GRUPOS.with_borrow_mut(|t| if let Some(t) = t { t.push(titulo) });
        guardar_icone_do_grupo(titulo);
        ui::grupo_faixa_com_janela(elementos, crate::idioma::tr(titulo),
            Some((tr!("Formatar células (Ctrl+1)"), Msg::FormatarCelulasNa(aba))))
    }

    pub(super) fn faixa_linha<'a>(&self, itens: Vec<Element<'a, Msg>>) -> Element<'a, Msg> {
        row(itens).spacing(3).align(pelicano::Align::Center).into()
    }

    pub(super) fn faixa_pilha<'a>(&self, itens: Vec<Element<'a, Msg>>) -> Element<'a, Msg> {
        column(itens).spacing(3).into()
    }

    /// Controle numérico curto, com estado ativo e tooltip. Não duplica rótulos e ícones.
    pub(super) fn faixa_numero_botao(
        &self,
        rotulo: &'static str,
        dica: &'static str,
        evento: Msg,
        ativo: bool,
    ) -> Element<'_, Msg> {
        let icone = match rotulo {
            "R$" => "currency-dollar",
            "%" => "percentage",
            ".000" => "thousands-separator",
            ".0←" => "decimal-decrease",
            _ => "decimal-increase",
        };
        self.faixa_icone(dica, icone, Some(evento), ativo)
    }

    /// A seta ao lado do "R$": as moedas do mundo (`feathersheet_core::celulas::moedas`), como
    /// o botão dividido de moeda do Excel. O "R$" sozinho continua usando a moeda da língua.
    pub(super) fn faixa_seta_moeda(&self) -> Element<'_, Msg> {
        ui::botao_icone(feather_ui::icones::icone("chevron-down", 10.), tr!("Escolher a moeda"),
            Some(Msg::AlternarMenuEdicao("moeda")), self.edicao_menu == Some("moeda"))
    }

    /// AutoSoma usa botão dividido: a área Σ executa a soma e a seta abre
    /// um menu flutuante de funções. Nenhuma PickList sobrepõe o ícone.
    pub(super) fn faixa_autosoma_compacto(&self) -> Element<'_, Msg> {
        row![
            button(row![
                feather_ui::icones::icone_colorido("sum", 18., "#21865c"),
                text(tr!("AutoSoma")).size(12),
            ].spacing(5).align(pelicano::Align::Center))
                .padding([6, 4])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::AutoSoma),
            button(feather_ui::icones::icone("chevron-down", 13.))
                .padding([7, 3])
                .style(estilo_botao_pequeno(self.edicao_menu == Some("autosoma")))
                .on_press(Msg::AlternarMenuEdicao("autosoma")),
        ].spacing(1).align(pelicano::Align::Center).into()
    }

    /// Preencher não é mais um controle desabilitado: duplo botão com submenu real.
    pub(super) fn faixa_preencher_compacto(&self) -> Element<'_, Msg> {
        row![
            button(row![
                feather_ui::icones::icone_colorido("arrow-down", 17., "#21865c"),
                text(tr!("Preencher")).size(12),
            ].spacing(5).align(pelicano::Align::Center))
                .padding([6, 4])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::Preencher(DirecaoPreenchimento::Baixo)),
            button(feather_ui::icones::icone("chevron-down", 13.))
                .padding([7, 3])
                .style(estilo_botao_pequeno(self.edicao_menu == Some("preencher")))
                .on_press(Msg::AlternarMenuEdicao("preencher")),
        ].spacing(1).align(pelicano::Align::Center).into()
    }

    /// Limpar ▾: botão dividido; os dois abrem o menu com Limpar tudo / formatos / conteúdo.
    pub(super) fn faixa_limpar_compacto(&self) -> Element<'_, Msg> {
        row![
            button(row![
                feather_ui::icones::icone_colorido("eraser", 17., "#ca5555"),
                text(tr!("Limpar")).size(12),
            ].spacing(5).align(pelicano::Align::Center))
                .padding([6, 4])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::AlternarMenuEdicao("limpar")),
            button(feather_ui::icones::icone("chevron-down", 13.))
                .padding([7, 3])
                .style(estilo_botao_pequeno(self.edicao_menu == Some("limpar")))
                .on_press(Msg::AlternarMenuEdicao("limpar")),
        ].spacing(1).align(pelicano::Align::Center).into()
    }

    /// Busca permanece uma ação real e visível, sem opções não implementadas.
    pub(super) fn faixa_localizar_grande(&self) -> Element<'_, Msg> {
        button(column![
            feather_ui::icones::icone("search", 25.),
            text(tr!("Localizar e")).size(11),
            text("selecionar").size(11),
        ].spacing(3).align(pelicano::Align::Center))
            .padding([5, 8])
            .style(estilo_botao_grande(false))
            .on_press(Msg::Busca).into()
    }

    /// Formatar ▾ do grupo Células: tamanho, visibilidade e Formatar células,
    /// como no Excel. Abre o menu flutuante comum da Página Inicial.
    pub(super) fn faixa_formatar_celulas_menu(&self) -> Element<'_, Msg> {
        let aberto = self.edicao_menu == Some("formatar");
        button(column![
            feather_ui::icones::icone_colorido("edit", 23., "#2179a7"),
            row![
                text(tr!("Formatar")).size(11),
                feather_ui::icones::icone("chevron-down", 12.),
            ].spacing(4).align(pelicano::Align::Center),
        ].spacing(3).align(pelicano::Align::Center))
            .padding([5, 8])
            .style(estilo_botao_grande(aberto))
            .on_press(Msg::AlternarMenuEdicao("formatar")).into()
    }

    /// Botão dividido: o botão faz a ação comum; a seta embaixo abre o menu
    /// `tipo` (Inserir ▾ / Excluir ▾ do grupo Células).
    pub(super) fn faixa_com_seta<'a>(&'a self, botao: Element<'a, Msg>, tipo: &'static str) -> Element<'a, Msg> {
        column![
            botao,
            // Sem `.width(FILL)`: no pelicano o filho que enche estica a coluna `Shrink`
            // até a largura da faixa (o Iced limitava à largura do botão de cima).
            button(feather_ui::icones::icone("chevron-down", 12.))
                .padding([1, 8])
                .style(estilo_botao_pequeno(self.edicao_menu == Some(tipo)))
                .on_press(Msg::AlternarMenuEdicao(tipo)),
        ].width(Length::Shrink).align(pelicano::Align::Center).into()
    }

    /// Mesclar dividido: o ícone alterna mesclar/desmesclar como antes;
    /// a seta abre Mesclar e centralizar, Mesclar através e afins.
    pub(super) fn faixa_mesclar_dividido(&self, mesclada: bool) -> Element<'_, Msg> {
        column![
            button(column![
                feather_ui::icones::icone("square", 23.),
                text(tr!("Mesclar")).size(11),
            ].spacing(3).align(pelicano::Align::Center))
                .padding([5, 8])
                .style(estilo_botao_grande(mesclada))
                .on_press(Msg::Mesclar),
            // Sem `.width(FILL)`: no pelicano o filho que enche estica a coluna `Shrink`
            // até a largura da faixa (o Iced limitava à largura do botão de cima).
            button(feather_ui::icones::icone("chevron-down", 12.))
                .padding([1, 8])
                .style(estilo_botao_pequeno(self.edicao_menu == Some("mesclar")))
                .on_press(Msg::AlternarMenuEdicao("mesclar")),
        ].width(Length::Shrink).align(pelicano::Align::Center).into()
    }

    /// O botão combinado oferece ordenação e filtro em menu suspenso.
    pub(super) fn faixa_classificar_filtrar(&self) -> Element<'_, Msg> {
        button(column![
            row![
                feather_ui::icones::icone_colorido("sort-ascending", 21., "#2179a7"),
                feather_ui::icones::icone("filter", 20.),
            ].spacing(2).align(pelicano::Align::Center),
            text(tr!("Classificar e")).size(11),
            row![
                text(tr!("Filtrar")).size(11),
                feather_ui::icones::icone("chevron-down", 12.),
            ].spacing(4).align(pelicano::Align::Center),
        ].spacing(2).align(pelicano::Align::Center))
            .padding([4, 7])
            .style(estilo_botao_grande(self.edicao_menu == Some("classificar")))
            .on_press(Msg::AlternarMenuEdicao("classificar")).into()
    }



    pub(super) fn faixa_estilo(&self) -> feathersheet_core::celulas::style::Style {
        self.workbook.aba(self.folha)
            .and_then(|aba| aba.sheet.estilo_resolvido(self.linha, self.coluna))
            .unwrap_or_default()
    }

    pub(super) fn faixa_nome_fonte(&self) -> &'static str {
        match self.faixa_estilo().font_family.as_deref().unwrap_or("") {
            "'IBM Plex Serif', Georgia, serif" => "Serifada",
            "'IBM Plex Mono', monospace" => "Monoespaçada",
            "Arial, Helvetica, sans-serif" => "Arial",
            "Georgia, serif" => "Georgia",
            "'Courier New', monospace" => "Courier New",
            "Verdana, sans-serif" => "Verdana",
            "'Times New Roman', serif" => "Times New Roman",
            _ => "Padrão",
        }
    }

    pub(super) fn faixa_nome_formato(&self) -> &'static str {
        use feathersheet_core::celulas::style::NumberFormat;
        let estilo = self.faixa_estilo();
        if estilo.code.is_some() { return FORMATOS[9]; }
        match estilo.format {
            NumberFormat::Number => FORMATOS[1],
            NumberFormat::Integer => FORMATOS[2],
            NumberFormat::Currency => FORMATOS[3],
            NumberFormat::Percent => FORMATOS[4],
            NumberFormat::Date => FORMATOS[5],
            NumberFormat::Datetime => FORMATOS[6],
            NumberFormat::Time => FORMATOS[7],
            NumberFormat::Text => FORMATOS[8],
            _ => FORMATOS[0],
        }
    }

    /// Lista de funções já disponíveis no motor Rust, separadas pelas
    /// categorias da mesma tabela de ajuda do TypeScript.
    pub(super) fn faixa_catalogo_funcoes(&self) -> Element<'_, Msg> {
        let indice = self.categoria_funcoes.min(CATEGORIAS.len() - 1);
        let funcoes = ui::lista_pesquisavel(
            &self.catalogos_funcoes[indice],
            tr!("Buscar função…"),
            Msg::InserirFuncao,
            190.0,
        );
        self.faixa_pilha(vec![
            self.faixa_lista(tr!("Categoria"), "categoria", CATEGORIAS.as_slice(),
                Some(CATEGORIAS[indice]), 190.0),
            funcoes,
        ])
    }

    pub(super) fn faixa_nome_zoom(&self) -> Option<&'static str> {
        ZOOMS.iter().copied().find(|n| n.trim_end_matches('%').parse::<u16>().ok() == Some(self.config.zoom))
    }

    /// Entrada para comandos globais; busca somente os comandos já ligados ao Rust.
    /// Largura do campo de pesquisa de comandos (encolhe em janela estreita).
    pub(super) fn largura_pesquisa(&self) -> f32 {
        // Com aba contextual (Tabela Dinâmica, Design da Tabela…) em janela estreita a busca
        // encolhe: senão o nome da aba ficava cortado atrás dela.
        let contextual = self.dinamica_na_faixa() || self.minigrafico_na_celula().is_some() || self.tem_forma_selecionada()
            || super::tabelas::sob_celula(&self.workbook, self.folha, self.linha, self.coluna).is_some();
        if self.tamanho_principal.width >= 1300.0 { 250.0 } else if contextual { 92.0 } else { 172.0 }
    }

    /// Os comandos achados pela pesquisa (no máximo 5), ou `None` sem texto digitado.
    pub(super) fn resultados_busca(&self) -> Option<pelicano::Column<'_, Msg>> {
        if self.busca_comandos.trim().is_empty() { return None; }
        let encontrados = self.resultados_faixa();
        let mut lista = column(Vec::new()).spacing(2);
        if encontrados.is_empty() {
            lista = lista.push(text(tr!("Nenhum comando disponível")).size(10).color(COR_TEXTO_MUTED()));
        }
        for (i, (nome, _)) in encontrados.into_iter().enumerate().take(5) {
            lista = lista.push(
                button(text(nome).size(11)).padding([3, 7]).width(Length::FILL)
                    .on_press(Msg::BuscaComando(i))
                    .style(estilo_botao_pequeno(false))
            );
        }
        Some(lista)
    }

    pub(super) fn faixa_pesquisa(&self) -> Element<'_, Msg> {
        let largura = self.largura_pesquisa();
        let campo = text_input(if largura < 150.0 { tr!("Pesquisar") } else { tr!("Pesquisar comandos...") }, &self.busca_comandos)
            .on_input(Msg::BuscaComandos).width(Length::Fixed(largura))
            .size(12).padding([6, 9]);
        let mut lista = column![campo].spacing(2);
        // No modo micro a lista flutua (`sobrepor_busca_micro`): a barra de título não cresce.
        if !self.faixa_micro() {
            if let Some(achados) = self.resultados_busca() { lista = lista.push(achados); }
        }
        container(lista).padding([2, 4]).width(Length::Fixed(largura + 13.0)).into()
    }

    pub(super) fn resultados_faixa(&self) -> Vec<(String, menu_nativo::Acao)> {
        let busca = self.busca_comandos.trim().to_lowercase();
        if busca.is_empty() { return vec![]; }
        fn percorrer(app: &App, itens: &[menu_nativo::Item], busca: &str, saida: &mut Vec<(String, menu_nativo::Acao)>) {
            for item in itens {
                if let Some(acao) = &item.acao {
                    // Acha pelo texto mostrado (idioma atual) ou pelo português.
                    let exibido = crate::idioma::tr(&item.rotulo);
                    if !item.rotulo.trim().is_empty() &&
                        (exibido.to_lowercase().contains(busca) || item.rotulo.to_lowercase().contains(busca)) &&
                        app.evento_menu(acao).is_some() &&
                        !saida.iter().any(|(nome, _)| nome == exibido)
                    {
                        saida.push((exibido.to_owned(), acao.clone()));
                    }
                }
                percorrer(app, &item.filhos, busca, saida);
            }
        }
        let mut saida = Vec::new();
        for menu in &menu_nativo::catalogo().menus {
            percorrer(self, &menu.itens, &busca, &mut saida);
        }
        saida.truncate(10);
        saida
    }

    /// Largura real de cada grupo, medida pelo layout do pelicano (as estimativas à mão
    /// erravam por centenas de px e a Inserir estourava a janela). Quando um grupo não
    /// cabe, ele vai inteiro ao menu Mais (sem corte). Guardada por aba, nº de grupos e
    /// idioma: o conteúdo de um grupo quase não muda a largura dele.
    pub(super) fn faixa_larguras(&self) -> Vec<f32> {
        thread_local! {
            static MEDIDAS: std::cell::RefCell<(Option<pelicano::Renderer>, std::collections::HashMap<(u8, usize, u8), Vec<f32>>)> =
                std::cell::RefCell::new((None, std::collections::HashMap::new()));
        }
        let grupos = self.montar_grupos_faixa();
        let chave = (self.faixa_ativa as u8, grupos.len(), crate::idioma::lingua() as u8);
        MEDIDAS.with_borrow_mut(|(medidor, guardadas)| {
            guardadas.entry(chave).or_insert_with(|| {
                let r = medidor.get_or_insert_with(pelicano::Renderer::new);
                let estados = pelicano::States::default();
                let tela = Size::new(5000.0, ALTURA_CONTROLES_FAIXA);
                grupos.iter().map(|g|
                    pelicano::layout(g, tela, r, &estados, &pelicano::Theme::light()).rect.width.ceil()
                ).collect()
            }).clone()
        })
    }

    /// INT-1: os títulos dos grupos da aba ativa e a largura de cada um recolhido, ou `None`
    /// quando algum grupo não passou por `faixa_grupo` (a Desenhar vem pronta da feather-ui):
    /// aí a aba fica com o "Mais" de antes. Guardado como `faixa_larguras`.
    pub(super) fn faixa_recolhiveis(&self) -> Option<(Vec<&'static str>, Vec<f32>)> {
        thread_local! {
            static GUARDADOS: std::cell::RefCell<std::collections::HashMap<(u8, u8), Option<(Vec<&'static str>, Vec<f32>)>>> =
                std::cell::RefCell::new(std::collections::HashMap::new());
        }
        if self.faixa_ativa == AbaFaixa::Inicio { return None; }
        // Por aba e língua: se o número de grupos mudar, `faixa_plano_abas` vê a diferença e cai no "Mais".
        let chave = (self.faixa_ativa as u8, crate::idioma::lingua() as u8);
        GUARDADOS.with_borrow_mut(|g| g.entry(chave).or_insert_with(|| {
            TITULOS_GRUPOS.with_borrow_mut(|t| *t = Some(Vec::new()));
            let grupos = self.montar_grupos_faixa().len();
            let titulos = TITULOS_GRUPOS.with_borrow_mut(Option::take).unwrap_or_default();
            if titulos.len() != grupos { return None; }
            let mut r = pelicano::Renderer::new();
            let estados = pelicano::States::default();
            let tela = Size::new(5000.0, ALTURA_CONTROLES_FAIXA);
            let larguras = titulos.iter().enumerate().map(|(i, t)| {
                let el = self.faixa_grupo_recolhido(i, t);
                pelicano::layout(&el, tela, &mut r, &estados, &pelicano::Theme::light()).rect.width.ceil()
            }).collect();
            Some((titulos, larguras))
        }).clone())
    }

    /// Botão de um grupo recolhido de qualquer aba: abre o grupo inteiro num painel.
    pub(super) fn faixa_grupo_recolhido(&self, indice: usize, titulo: &'static str) -> Element<'_, Msg> {
        let icone = ICONES_GRUPOS.with_borrow(|m| m.get(titulo).cloned()).unwrap_or_else(|| "layout-grid".to_owned());
        let botao = button(pelicano::column![
            feather_ui::icones::icone(&icone, 21.),
            feather_ui::icones::icone("chevron-down", 11.),
        ].spacing(7).align(pelicano::Align::Center))
            .padding([10, 16])
            .style(estilo_botao_grande(self.faixa_grupo_aberto == Some(indice)))
            .on_press(Msg::AlternarGrupoFaixa(indice));
        ui::grupo_faixa_compacto(botao.into(), crate::idioma::tr(titulo))
    }

    /// INT-1, fora da Página Inicial: quais grupos ficam inteiros (`true`) e quantos aparecem.
    /// Primeiro todos recolhidos, quantos couberem (o resto vai ao "Mais"); depois, da
    /// esquerda para a direita, abre inteiro cada um que ainda couber. `None`: a aba não
    /// recolhe (ver `faixa_recolhiveis`).
    pub(super) fn faixa_plano_abas(&self) -> Option<(Vec<bool>, usize)> {
        let (_, recolhidos) = self.faixa_recolhiveis()?;
        let cheios = self.faixa_larguras();
        if cheios.len() != recolhidos.len() || cheios.is_empty() { return None; }
        Some(plano_recolhido(&cheios, &recolhidos, (self.tamanho_principal.width - 18.0).max(0.0)))
    }

    /// Abre só os comandos adicionais de UMA seção da Ribbon.
    /// Não existe o painel enorme "Mais ferramentas" em largura de desktop.
    pub(super) fn faixa_abrir_grupo(&self, indice:usize) -> Element<'_,Msg> {
        feather_ui::formatacao::dica(
            button(feather_ui::icones::icone("chevron-down",13.))
                .padding([4,4])
                .style(estilo_botao_pequeno(self.faixa_grupo_aberto==Some(indice)))
                .on_press(Msg::AlternarGrupoFaixa(indice)),
            tr!("Mais opções desta seção"),
        )
    }

    /// Botão de seção minimizada; o título aparece UMA vez no rodapé.
    pub(super) fn faixa_inicio_recolhido(&self, indice:usize) -> Element<'_,Msg> {
        const ICONES:[&str;7]=[
            "clipboard", "bold", "align-center", "sum",
            "palette", "table", "sum",
        ];
        let botao=button(pelicano::column![
            feather_ui::icones::icone(ICONES[indice],21.),
            feather_ui::icones::icone("chevron-down",11.),
        ].spacing(7).align(pelicano::Align::Center))
            .padding([10,16])
            .style(estilo_botao_grande(self.faixa_grupo_aberto==Some(indice)))
            .on_press(Msg::AlternarGrupoFaixa(indice));
        // A legenda longa fazia o grupo recolhido continuar largo.
        // Nome completo é preservado no menu da seção.
        const ROTULOS:[&str;7]=[
            "Transfer.", "Fonte", "Alinhar", "Número", "Estilos", "Células", "Edição",
        ];
        self.faixa_grupo(ROTULOS[indice],botao.into())
    }

    /// Redução gradual por seção: preserva ações de maior frequência e
    /// disponibiliza todo o restante na seta daquela seção, sem rótulos longos.
    pub(super) fn faixa_inicio_compacto(&self, indice:usize) -> Element<'_,Msg> {
        let estilo=self.faixa_estilo();
        let nome=faixa_responsiva::NOMES[indice];
        let conteudo:Element<'_,Msg>=match indice {
            0 => self.faixa_linha(vec![
                self.faixa_botao_colar(),
                self.faixa_pilha(vec![
                    self.faixa_linha(vec![
                        self.faixa_icone(tr!("Recortar"), "cut",Some(Msg::Recortar),false),
                        self.faixa_icone(tr!("Copiar"), "copy",Some(Msg::Copiar),false),
                    ]),
                    self.faixa_abrir_grupo(indice),
                ]),
            ]),
            1 => {
                let tamanho=estilo.font_size.unwrap_or(13.).round().clamp(6.,96.) as i32;
                self.faixa_pilha(vec![
                    self.faixa_linha(vec![
                        self.faixa_lista(tr!("Fonte"),"fonte",FONTES,Some(self.faixa_nome_fonte()),106.),
                        self.faixa_lista(tr!("Tamanho"),"tamanho",TAMANHOS,
                            TAMANHOS.iter().copied().find(|x|x.parse::<i32>().ok()==Some(tamanho)),55.),
                        self.faixa_tamanho_acao("A+",tr!("Aumentar fonte"),1),
                        self.faixa_tamanho_acao("A−",tr!("Diminuir fonte"),-1),
                    ]),
                    self.faixa_linha(vec![
                        self.faixa_icone(tr!("Negrito"),"bold",Some(Msg::AlternarEstilo("bold".into())),estilo.bold),
                        self.faixa_icone(tr!("Itálico"),"italic",Some(Msg::AlternarEstilo("italic".into())),estilo.italic),
                        self.faixa_sublinhado(estilo.underline),
                        self.faixa_bordas(),
                        self.faixa_cor(AlvoCor::Fundo),
                        self.faixa_cor(AlvoCor::Texto),
                        self.faixa_abrir_grupo(indice),
                    ]),
                ])
            }
            2 => {
                let alinhar=match estilo.align {
                    feathersheet_core::celulas::style::Align::Left=>
                        Some(feather_ui::alinhamento::Horizontal::Esquerda),
                    feathersheet_core::celulas::style::Align::Center=>
                        Some(feather_ui::alinhamento::Horizontal::Centro),
                    feathersheet_core::celulas::style::Align::Right=>
                        Some(feather_ui::alinhamento::Horizontal::Direita),
                    _=>None,
                };
                self.faixa_pilha(vec![
                    self.faixa_linha(vec![
                        feather_ui::alinhamento::seletor_horizontal(
                            alinhar,true,false,
                            |escolha| Msg::EstiloMenu("align".into(),
                                Campo::Texto(match escolha {
                                    feather_ui::alinhamento::Horizontal::Esquerda=>"left",
                                    feather_ui::alinhamento::Horizontal::Centro=>"center",
                                    feather_ui::alinhamento::Horizontal::Direita=>"right",
                                    feather_ui::alinhamento::Horizontal::Justificado=>"justify",
                                }.into())),
                        ),
                        self.faixa_icone(tr!("Quebrar linha"),"text-wrap",
                            Some(Msg::EstiloMenu("wrap".into(),Campo::Logico(!estilo.wrap))),estilo.wrap),
                    ]),
                    self.faixa_linha(vec![
                        self.faixa_vertical("top",estilo.valign==feathersheet_core::celulas::style::VAlign::Top),
                        self.faixa_vertical("middle",estilo.valign==feathersheet_core::celulas::style::VAlign::Middle),
                        self.faixa_vertical("bottom",estilo.valign==feathersheet_core::celulas::style::VAlign::Bottom),
                        self.faixa_icone(tr!("Mesclar"),"square",Some(Msg::Mesclar),false),
                        self.faixa_icone(tr!("Opções de mesclagem"),"chevron-down",
                            Some(Msg::AlternarMenuEdicao("mesclar")),self.edicao_menu==Some("mesclar")),
                        self.faixa_abrir_grupo(indice),
                    ]),
                ])
            }
            3 => self.faixa_pilha(vec![
                self.faixa_lista(tr!("Formato"),"formato",FORMATOS,
                    Some(self.faixa_nome_formato()),126.),
                self.faixa_linha(vec![
                    self.faixa_numero_botao("R$",tr!("Moeda"),
                        Msg::EstiloMenu("format".into(),Campo::Texto("currency".into())),false),
                    self.faixa_seta_moeda(),
                    self.faixa_numero_botao("%",tr!("Percentual"),
                        Msg::EstiloMenu("format".into(),Campo::Texto("percent".into())),false),
                    self.faixa_icone(tr!("Mais decimais"),"decimal-increase",Some(Msg::Decimais(1)),false),
                    self.faixa_abrir_grupo(indice),
                ]),
            ]),
            // Empilhados com rótulo curto, como o Excel em janela estreita.
            4 => self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    self.faixa_pequeno_colorido(tr!("Condicional"),"palette","#7d5cc6",
                        Some(Msg::CondicionalAbrir),self.condicional_menu_aberto),
                    self.faixa_pequeno(tr!("Tabela"),"table",
                        Some(Msg::AlternarMenuTabelas),self.tabelas_menu_aberto),
                    self.faixa_pequeno(tr!("Estilos de célula"),"cell-styles",
                        Some(Msg::AlternarMenuEstilos),self.estilos_menu_aberto),
                ]),
                self.faixa_abrir_grupo(indice),
            ]),
            5 => self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    self.faixa_pequeno_colorido(tr!("Inserir"),"plus","#21865c",
                        Some(Msg::InserirMenu(Estrutura::InserirLinha,false)),false),
                    self.faixa_pequeno_colorido(tr!("Excluir"),"trash","#ca5555",
                        Some(Msg::Estrutura(Estrutura::ExcluirLinha)),false),
                    self.faixa_pequeno_colorido(tr!("Formatar"),"edit","#2179a7",
                        Some(Msg::AlternarMenuEdicao("formatar")),self.edicao_menu==Some("formatar")),
                ]),
                self.faixa_abrir_grupo(indice),
            ]),
            _ => self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("AutoSoma"),"sum",Some(Msg::AutoSoma),false),
                    self.faixa_pequeno(tr!("Classificar"),"filter",
                        Some(Msg::AlternarMenuEdicao("classificar")),false),
                    self.faixa_pequeno(tr!("Localizar"),"search",Some(Msg::BuscaAba(false)),false),
                ]),
                self.faixa_abrir_grupo(indice),
            ]),
        };
        // Mesma setinha ↘ do grupo inteiro (Fonte, Alinhamento, Número).
        let aba=match indice { 1=>Some(super::formatar_celulas::Aba::Fonte), 2=>Some(super::formatar_celulas::Aba::Alinhamento),
            3=>Some(super::formatar_celulas::Aba::Numero), _=>None };
        if let Some(aba)=aba { return self.faixa_grupo_com_janela(nome,aba,conteudo); }
        self.faixa_grupo(nome,conteudo)
    }

    /// Overlay de uma seção minimizada: apenas aquele grupo completo, com
    /// âncora perto do respectivo botão, sem duplicar todo o conteúdo da Ribbon.
    pub(super) fn sobrepor_faixa_grupo<'a>(&'a self,base:Element<'a,Msg>)->Element<'a,Msg> {
        let Some(indice)=self.faixa_grupo_aberto else{return base};
        if self.faixa_ativa!=AbaFaixa::Inicio {return self.sobrepor_grupo_recolhido(base, indice);}
        let plano=faixa_responsiva::planejar(self.tamanho_principal.width);
        if indice>=plano.visiveis||plano.niveis[indice]==Nivel::Completo {return base;}
        let Some(grupo)=self.montar_grupos_faixa().into_iter().nth(indice) else{return base};
        // A largura acompanha a seção aberta; sem um painel vazio de 640 px.
        const LARGURAS:[f32;7]=[220.,390.,335.,248.,450.,315.,322.];
        let largura=(self.tamanho_principal.width-18.)
            .min(LARGURAS[indice]).max(160.);
        let x=(8.+plano.largura_antes(indice))
            .min((self.tamanho_principal.width-largura-8.).max(8.));
        let y=(43.+52.+ALTURA_FAIXA_NATIVA)
            .min((self.tamanho_principal.height-160.).max(8.));
        let painel=container(pelicano::column![
            pelicano::row![
                // O próprio grupo já tem legenda; não repetir o título.
                Space::new(Length::FILL, 0.0),
                button(feather_ui::icones::icone("x",14.))
                    .padding([4,5])
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::FecharGrupoFaixa),
            ].align(pelicano::Align::Center),
            grupo,
        ].spacing(6).padding(10))
            .width(Length::Fixed(largura))
            .style(estilo_popup_menu);
        let fora=pelicano::mouse_area(
            Space::new(Length::FILL,Length::FILL)
        ).on_press(Msg::FecharGrupoFaixa);
        stack![
            base,fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![
                    Space::new(Length::Fixed(x), 0.0),
                    painel,
                ]
            ],
        ].height(Length::FILL).into()
    }

    /// INT-1: o painel de um grupo recolhido fora da Página Inicial, sob o botão dele.
    fn sobrepor_grupo_recolhido<'a>(&'a self, base: Element<'a, Msg>, indice: usize) -> Element<'a, Msg> {
        let (Some((inteiros, visiveis)), Some((_, recolhidos))) = (self.faixa_plano_abas(), self.faixa_recolhiveis()) else { return base };
        if indice >= visiveis || inteiros[indice] { return base; }
        let Some(grupo) = self.montar_grupos_faixa().into_iter().nth(indice) else { return base };
        let larguras = self.faixa_larguras();
        let antes: f32 = (0..indice).map(|j| if inteiros[j] { larguras[j] } else { recolhidos[j] } + 2.0).sum();
        let largura = (larguras[indice] + 24.0).min(self.tamanho_principal.width - 16.0).max(160.0);
        let x = (8.0 + antes).min((self.tamanho_principal.width - largura - 8.0).max(8.0));
        let y = (43. + 52. + ALTURA_FAIXA_NATIVA).min((self.tamanho_principal.height - 160.).max(8.));
        let painel = container(pelicano::column![
            pelicano::row![
                Space::new(Length::FILL, 0.0),
                button(feather_ui::icones::icone("x", 14.)).padding([4, 5])
                    .style(estilo_botao_pequeno(false)).on_press(Msg::FecharGrupoFaixa),
            ].align(pelicano::Align::Center),
            grupo,
        ].spacing(6).padding(10)).width(Length::Fixed(largura)).style(estilo_popup_menu);
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL)).on_press(Msg::FecharGrupoFaixa);
        stack![
            base, fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![Space::new(Length::Fixed(x), 0.0), painel],
            ],
        ].height(Length::FILL).into()
    }

    /// Quantos grupos inteiros cabem na largura real da janela.
    /// A reserva para o botão Mais só é aplicada quando há grupos ocultos.
    /// Em janelas muito estreitas preservamos ao menos o primeiro grupo.
    pub(super) fn faixa_visiveis(&self) -> usize {
        if self.faixa_ativa==AbaFaixa::Inicio {
            return faixa_responsiva::planejar(self.tamanho_principal.width).visiveis;
        }
        if let Some((_, visiveis)) = self.faixa_plano_abas() { return visiveis; }
        let larguras = self.faixa_larguras();
        let disponivel = (self.tamanho_principal.width - 18.0).max(0.0);
        let tudo = larguras.iter().sum::<f32>() + (larguras.len().saturating_sub(1) as f32 * 2.0);
        if tudo <= disponivel { return larguras.len(); }
        let limite = (disponivel - RESERVA_BOTAO_MAIS).max(0.0);
        let mut ocupado = 0.0;
        let mut quantidade = 0;
        for &largura in &larguras {
            let proximo = ocupado + if quantidade > 0 { 2.0 } else { 0.0 } + largura;
            if proximo > limite { break; }
            ocupado = proximo;
            quantidade += 1;
        }
        quantidade.max(1)
    }


}

/// O plano da faixa recolhida (ver `App::faixa_plano_abas`): larguras inteiras e
/// recolhidas de cada grupo e o espaço da janela.
fn plano_recolhido(cheios: &[f32], recolhidos: &[f32], disponivel: f32) -> (Vec<bool>, usize) {
    let n = cheios.len();
    let total = |inteiros: &[bool], v: usize| (0..v).map(|i| if inteiros[i] { cheios[i] } else { recolhidos[i] }).sum::<f32>()
        + v.saturating_sub(1) as f32 * 2.0 + if v < n { RESERVA_BOTAO_MAIS } else { 0.0 };
    let mut inteiros = vec![false; n];
    let visiveis = (1..=n).rev().find(|&v| total(&inteiros, v) <= disponivel).unwrap_or(1);
    for i in 0..visiveis {
        inteiros[i] = true;
        if total(&inteiros, visiveis) > disponivel { inteiros[i] = false; }
    }
    (inteiros, visiveis)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// INT-1: fora da Página Inicial a faixa recolhe grupo a grupo, e o recolhido abre num painel.
    #[test]
    fn dados_em_janela_estreita_recolhe_grupo_e_abre_o_painel() {
        let mut app = App::default();
        let id = crate::compat::window::novo_id();
        app.janelas.principal = Some(id);
        app.tamanho_principal = Size::new(900.0, 560.0);
        app.selecionar_faixa(AbaFaixa::Dados);
        let (inteiros, visiveis) = app.faixa_plano_abas().expect("Dados recolhe");
        assert_eq!(visiveis, app.montar_grupos_faixa().len(), "a 900 px os 6 grupos aparecem");
        let recolhido = inteiros.iter().position(|i| !i).expect("algum grupo recolhido");
        assert!(inteiros.iter().any(|i| *i), "e algum inteiro");
        let _ = app.update(Msg::AlternarGrupoFaixa(recolhido));
        let _: Element<'_, Msg> = app.view(id);
        let soma: f32 = app.faixa_larguras().iter().sum();
        app.tamanho_principal = Size::new(soma + 200.0, 900.0);
        assert!(app.faixa_plano_abas().unwrap().0.iter().all(|i| *i), "cabendo tudo: tudo inteiro");
    }

    #[test]
    fn plano_recolhido_abre_o_que_cabe_e_manda_o_resto_ao_mais() {
        // Cabe tudo inteiro.
        assert_eq!(plano_recolhido(&[100., 100.], &[60., 60.], 300.), (vec![true, true], 2));
        // O primeiro é largo demais: fica recolhido e os seguintes abrem (sem buraco à direita).
        assert_eq!(plano_recolhido(&[400., 100., 100.], &[80., 60., 60.], 300.), (vec![false, true, true], 3));
        // Nem recolhidos cabem todos: o resto vai ao "Mais", que tem a sua reserva.
        let (inteiros, v) = plano_recolhido(&[200.; 6], &[80.; 6], 400.);
        assert!(v < 6 && inteiros.iter().all(|i| !i));
        assert!(v as f32 * 82. + RESERVA_BOTAO_MAIS <= 400. + 2.);
    }
    #[test]
    pub(super) fn inserir_apresenta_icones_existentes_sem_fallback() {
        for nome in [
            "pivot-table", "table-suggest", "table", "photo", "shapes",
            "circle-dot", "cube", "sitemap", "screenshot", "checkbox",
            "chart-suggest", "chart-bar", "chart-columns", "chart-line",
            "chart-pie", "chart-area", "chart-scatter", "chart-combo",
            "chart-waterfall", "chart-map", "chart-pivot", "sparkline-line",
            "sparkline-columns", "sparkline-winloss", "slicer", "timeline",
            "link", "message-circle", "text-box", "header-footer", "wordart",
            "signature", "object", "equation", "omega", "chevron-down",
        ] {
            assert!(feather_ui::icones::existe(nome), "ícone ausente: {nome}");
        }
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inserir;
        let _: Element<'_, Msg> = app.view_faixa();
        let _: Element<'_, Msg> = app.faixa_grande_seta("Tabela dinâmica", "pivot-table", None, false);
    }

    #[test]
    pub(super) fn botao_grafico_usa_icone_vetorial_valido_para_seta() {
        assert!(feather_ui::icones::existe("chart-bar"));
        assert!(feather_ui::icones::existe("chevron-down"));
        let app = App::default();
        let _: Element<'_, Msg> = app.faixa_grafico();
    }

    #[test]
    pub(super) fn botao_dividir_texto_abre_assistente_quatro_etapas_sem_menu() {
        assert!(feather_ui::icones::existe("columns"));
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Dados;
        let _: Element<'_, Msg> = app.view_faixa();
        app.update(Msg::AbrirAssistenteDivisao);
        assert_eq!(app.assistente_divisao.as_ref().map(|e|e.etapa),Some(1));
    }

    #[test]
    pub(super) fn colar_usa_seta_svg_real_e_nao_ocupa_largura_de_dropdown() {
        assert!(feather_ui::icones::existe("clipboard"));
        assert!(feather_ui::icones::existe("chevron-down"));
        let app = App::default();
        let _: Element<'_, Msg> = app.faixa_botao_colar();
    }

    #[test]
    pub(super) fn catalogo_pesquisavel_aceita_aliases_ptbr_e_filtra_por_categoria() {
        let catalogos = montar_catalogos_funcoes();
        assert_eq!(catalogos.len(), CATEGORIAS.len());
        assert!(catalogos[0].iter().any(|nome| nome == "SOMA"));
        assert!(catalogos[0].iter().any(|nome| nome == "SUM"));
        let de = |categoria: &str| &catalogos[CATEGORIAS.iter()
            .position(|c| *c == categoria).expect(categoria)];
        assert!(de("Matemática").iter().any(|nome| nome == "SOMA"));
        assert!(!de("Matemática").iter().any(|nome| nome == "PROCX"));
        assert!(de("Pesquisa e referência").iter().any(|nome| nome == "PROCX"));
    }

    #[test]
    pub(super) fn ribbon_nao_tem_scroll_horizontal_e_os_grupos_excedentes_continuam_acessiveis() {
        assert!(feather_ui::componentes::ALTURA_GRUPO_COMPACTO <= ALTURA_CONTROLES_FAIXA);
        assert!(ALTURA_FAIXA_NATIVA >= ALTURA_CONTROLES_FAIXA);
        let mut app = App::default();
        for aba in [
            AbaFaixa::Inicio, AbaFaixa::Inserir, AbaFaixa::Formulas,
            AbaFaixa::Dados, AbaFaixa::Revisao, AbaFaixa::Exibir, AbaFaixa::Macros,
            AbaFaixa::Tabela, AbaFaixa::Ferramentas, AbaFaixa::EditarForma, AbaFaixa::Dinamica, AbaFaixa::Minigrafico,
        ] {
            app.faixa_ativa = aba;
            let total = app.montar_grupos_faixa().len();
            assert_eq!(total, app.faixa_larguras().len(),
                "estimativas faltando para {aba:?}");
            for largura in [760., 1024., 1280., 1920., 2560.] {
                app.tamanho_principal.width = largura;
                let visiveis = app.faixa_visiveis();
                assert!(visiveis > 0 && visiveis <= total);
                let _: Element<'_, Msg> = app.view_faixa();
                if visiveis < total {
                    let _: Element<'_, Msg> = app.view_faixa_extras();
                }
            }
        }
        app.faixa_ativa = AbaFaixa::Inicio;
        app.tamanho_principal.width = 1280.;
        assert_eq!(app.faixa_visiveis(),app.montar_grupos_faixa().len(),
            "desktop precisa usar compactação por seção, não painel geral");
        let p=faixa_responsiva::planejar(1280.);
        for (indice,n) in p.niveis.iter().enumerate() {
            if *n!=Nivel::Completo {
                let _:Element<'_,Msg>=app.faixa_inicio_compacto(indice);
                let _:Element<'_,Msg>=app.faixa_inicio_recolhido(indice);
            }
        }
        app.tamanho_principal.width = 2560.;
        assert_eq!(app.faixa_visiveis(), app.montar_grupos_faixa().len());
    }

    #[test]
    pub(super) fn menu_do_grupo_nao_abre_painel_gigante_de_mais_ferramentas() {
        let mut app=App::default();
        app.faixa_ativa=AbaFaixa::Inicio;
        app.tamanho_principal.width=1280.;
        let plano=faixa_responsiva::planejar(1280.);
        assert_eq!(plano.visiveis,7);
        assert_ne!(plano.niveis[4],Nivel::Completo);
        app.update(Msg::AlternarGrupoFaixa(4));
        assert_eq!(app.faixa_grupo_aberto,Some(4));
        assert!(!app.faixa_extras_aberto);
        let _:Element<'_,Msg>=app.sobrepor_faixa_grupo(Space::new(Length::FILL,Length::FILL).into());
        app.update(Msg::FecharGrupoFaixa);
        assert!(app.faixa_grupo_aberto.is_none());
    }

    #[test]
    pub(super) fn home_apresenta_icones_coloridos_existentes_sem_acao_falsa() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inicio;
        let _: Element<'_, Msg> = app.view_faixa();
        for nome in ["clipboard", "cut", "copy", "brush", "palette",
            "table", "plus", "trash", "edit", "sum", "filter",
            "bold", "italic", "underline", "strikethrough", "font-color",
            "font-increase", "font-decrease", "border-all", "text-rotate",
            "align-top", "align-middle", "align-bottom", "currency-dollar",
            "percentage", "thousands-separator", "decimal-decrease",
            "decimal-increase", "cell-styles", "alternating-colors"] {
            assert!(feather_ui::icones::existe(nome), "Ícone ausente: {nome}");
        }
        let _: Element<'_, Msg> = app.faixa_grande_destaque(
            "Formatar células", "edit", "#2179a7",
            (224, 242, 252), Some(Msg::FormatarCelulasAbrir), false,
        );
    }

    #[test]
    pub(super) fn menus_tabela_e_estilos_usam_botoes_com_seta() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inicio;
        let _: Element<'_, Msg> = app.view_faixa();
        assert!(feather_ui::icones::existe("chevron-down"));
        app.update(Msg::AlternarMenuTabelas);
        assert!(app.tabelas_menu_aberto);
        app.update(Msg::AlternarMenuEstilos);
        assert!(!app.tabelas_menu_aberto && app.estilos_menu_aberto);
    }

    #[test]
    pub(super) fn inicio_numero_edicao_usam_acoes_reais_e_menus() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inicio;
        let _: Element<'_, Msg> = app.faixa_autosoma_compacto();
        let _: Element<'_, Msg> = app.faixa_classificar_filtrar();
        let _: Element<'_, Msg> = app.faixa_localizar_grande();
        assert_eq!(app.faixa_nome_formato(), "Geral");
        let _: Element<'_, Msg> = app.view_faixa();
    }

    #[test]
    pub(super) fn controles_compactos_de_fonte_montam_com_bordas_e_tamanho() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inicio;
        let _: Element<'_, Msg> = app.faixa_bordas();
        assert!(feather_ui::icones::existe("chevron-down"));
        app.update(Msg::AlternarMenuBordas);
        assert!(app.bordas_menu_aberto);
        app.update(Msg::FecharMenuBordas);
        assert!(!app.bordas_menu_aberto);
        let _: Element<'_, Msg> = app.faixa_inclinar_texto(false);
        let _: Element<'_, Msg> = app.faixa_cor(AlvoCor::Fundo);
        let _: Element<'_, Msg> = app.faixa_cor(AlvoCor::Texto);
        let _: Element<'_, Msg> = app.faixa_sublinhado(false);
        let _: Element<'_, Msg> = app.faixa_tamanho_acao("A+", "Aumentar", 1);
        let _: Element<'_, Msg> = app.view_faixa();
        assert!(feather_ui::icones::existe("bold"));
        assert!(feather_ui::icones::existe("italic"));
        assert!(feather_ui::icones::existe("underline"));
        let antes = app.faixa_estilo().font_size.unwrap_or(13.);
        let _ = app.update(Msg::TamanhoFonteDelta(1));
        assert!(app.faixa_estilo().font_size.unwrap_or(13.) > antes);
        let _ = app.update(Msg::TamanhoFonteDelta(-1));
        assert!(app.faixa_estilo().font_size.unwrap_or(13.) <= antes);
    }

    #[test]
    pub(super) fn ribbon_fonte_compacta_monta_sem_alterar_estado_da_planilha() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Inicio;
        let antes = app.faixa_estilo();
        let _: Element<'_, Msg> = app.view_faixa();
        assert_eq!(app.faixa_estilo(), antes);
        assert_eq!(app.faixa_nome_fonte(), "Padrão");
        // O motor continua a receber a mesma mensagem de tamanho da fonte.
        let _ = app.update(Msg::TamanhoFonteDelta(1));
        assert!(app.faixa_estilo().font_size.unwrap_or(13.) >= 13.);
    }

    #[test]
    pub(super) fn os_controles_de_formato_sincronizam_com_a_celula_ativa() {
        let mut app = App::default();
        assert_eq!(app.faixa_nome_formato(), "Geral");
        app.aplicar_estilo_menu("format", Campo::Texto("currency".into()));
        assert_eq!(app.faixa_nome_formato(), "R$ 1.234,56");
    }
    #[test]
    pub(super) fn categorias_oferecem_todas_as_funcoes_do_motor() {
        use feathersheet_core::funcoes::function_help::{all, CATEGORIES};
        let app=App::default();
        assert_eq!(CATEGORIAS.len(), CATEGORIES.len()+1);
        // Cada nome da caixa abre o catálogo da MESMA categoria do motor.
        let catalogos = montar_catalogos_funcoes();
        for (i, (nome, funcoes)) in CATEGORIES.iter().enumerate() {
            assert_eq!(CATEGORIAS[i+1], *nome);
            for f in *funcoes {
                assert!(catalogos[i+1].iter().any(|o| o == f), "{nome}: {f}");
            }
        }
        for (f, _) in all() {
            assert!(catalogos[0].iter().any(|o| o == f), "Todas: {f}");
        }
        assert_eq!(app.categoria_funcoes, 0);
    }
    #[test]
    pub(super) fn ferramenta_exibir_formulas_usa_o_mesmo_estado_do_menu() {
        let mut app = App::default();
        app.faixa_ativa = AbaFaixa::Exibir;
        let _: Element<'_, Msg> = app.view_faixa();
        assert!(!app.mostrar_formulas);
        app.update(Msg::AlternarMostrarFormulas);
        assert!(app.mostrar_formulas);
        let _: Element<'_, Msg> = app.view_faixa();
    }

    #[test]
    pub(super) fn pesquisa_de_comandos_nao_expoe_acoes_sem_motor() {
        let mut app = App::default();
        app.busca_comandos = "gráfico".into();
        assert!(!app.resultados_faixa().is_empty());
        app.busca_comandos = "salvar".into();
        assert!(!app.resultados_faixa().is_empty());
    }
}
