//! Galerias flutuantes de tabela e de estilos de célula da Ribbon.
//! Ações reais: paleta de tabelas no motor e patch único de estilo da seleção.
use super::*;

const LARGURA_TABELAS: f32 = 354.0;
const LARGURA_ESTILOS: f32 = 418.0;
const MARGEM_POPUP: f32 = 8.0;
const LARGURA_MIN_TABELAS: f32 = 180.0;
const LARGURA_MIN_ESTILOS: f32 = 190.0;
const ALTURA_MIN_VISIVEL: f32 = 90.0;
/// Mesma ordem do TS (`TEMAS_DE_TABELA`); as cores vêm de `tabelas::paleta`.
pub(super) const TEMAS_TABELA: &[&str] = &[
    "padrao", "azul", "laranja", "verde", "vermelho", "roxo", "cinza",
];

fn hex_da_cor(c: pelicano::Color) -> String {
    let byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

#[derive(Clone, Copy)]
pub(super) struct Preset {
    id: &'static str,
    nome: &'static str,
    fundo: &'static str,
    texto: &'static str,
    tamanho: f64,
    negrito: bool,
}
const fn preset(id: &'static str, nome: &'static str, fundo: &'static str,
    texto: &'static str, tamanho: f64, negrito: bool) -> Preset {
    Preset { id, nome, fundo, texto, tamanho, negrito }
}
const BASICOS: &[Preset] = &[
    preset("normal","Normal","#ffffff","#202124",13.0,false),
    preset("bom","Bom","#cdf4d9","#17683a",13.0,true),
    preset("ruim","Ruim","#ffe0e1","#b42332",13.0,true),
    preset("neutro","Neutro","#fff0af","#936000",13.0,false),
];
const DADOS: &[Preset] = &[
    preset("calculo","Cálculo","#e8efff","#2354a1",13.0,true),
    preset("entrada","Entrada","#ffe3c6","#9a4906",13.0,false),
    preset("nota","Nota","#fff9c7","#705b09",13.0,false),
    preset("saida","Saída","#ecf0f5","#334155",13.0,false),
    preset("vinculada","Vinculada","#e8f5e9","#166534",13.0,false),
];
const TITULOS: &[Preset] = &[
    preset("titulo","Título","#ffffff","#1165c5",23.0,true),
    preset("titulo2","Título 2","#ffffff","#23578d",18.0,true),
    preset("titulo3","Título 3","#ffffff","#365981",15.0,true),
    preset("cab1","Cabeçalho 1","#e9f2ff","#1d4f84",13.0,true),
    preset("cab2","Cabeçalho 2","#edf2f7","#34516e",13.0,true),
    preset("cab3","Cabeçalho 3","#f1f5f9","#475569",13.0,true),
];
const ACENTOS: &[Preset] = &[
    preset("a1_20","20% · Azul","#d8e9ff","#1755a4",12.0,false),
    preset("a2_20","20% · Laranja","#ffeadb","#a35011",12.0,false),
    preset("a3_20","20% · Verde","#ddf4e3","#146c37",12.0,false),
    preset("a1_40","40% · Azul","#b4d5ff","#134982",12.0,false),
    preset("a2_40","40% · Laranja","#ffd3b2","#8c3c0a",12.0,false),
    preset("a3_40","40% · Verde","#b8e9c5","#13512a",12.0,false),
    preset("a1_60","60% · Azul","#80b4f3","#103761",12.0,false),
    preset("a2_60","60% · Laranja","#ffac70","#633008",12.0,false),
    preset("a3_60","60% · Verde","#7fd69a","#0c4624",12.0,false),
    preset("a1","Azul","#2876d4","#ffffff",13.0,true),
    preset("a2","Laranja","#ee791f","#ffffff",13.0,true),
    preset("a3","Verde","#258c4c","#ffffff",13.0,true),
];

fn todos() -> impl Iterator<Item = &'static Preset> {
    BASICOS.iter().chain(DADOS).chain(TITULOS).chain(ACENTOS)
}
fn cor_hex(h: &str) -> pelicano::Color {
    let h = h.trim_start_matches('#');
    if h.len() != 6 { return pelicano::Color::WHITE; }
    let v = u32::from_str_radix(h, 16).unwrap_or(0xffffff);
    pelicano::Color::rgb8(((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8)
}
fn separador<'a>() -> Element<'a, Msg> {
    container(Space::new(0.0, Length::Fixed(1.0)))
        .width(Length::FILL)
        .style(|_| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_BORDA_SUAVE())),
            ..Default::default()
        }).into()
}
fn secao<'a>(nome: &'static str) -> Element<'a, Msg> {
    container(text(nome).size(10.5).color(COR_TEXTO_MUTED()))
        .padding([9, 5]).into()
}
fn mini_tabela(nome: &str) -> pelicano::SvgView {
    // Cabeçalho, faixas e borda vêm da mesma paleta que a grade usa para pintar a tabela.
    let (cabecalho, faixa, borda) = tabelas::paleta(nome, false);
    let (cabecalho, faixa, borda) = (hex_da_cor(cabecalho), hex_da_cor(faixa), hex_da_cor(borda));
    let imagem = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="70" height="47" viewBox="0 0 70 47">
    <rect x="1" y="1" width="68" height="45" rx="3" fill="#ffffff" stroke="{borda}" stroke-width="1.4"/>
    <path d="M2 4 Q2 2 5 2 H65 Q68 2 68 5 V13 H2Z" fill="{cabecalho}"/>
    <rect x="2" y="22" width="66" height="8" fill="{faixa}"/>
    <rect x="2" y="38" width="66" height="7" fill="{faixa}"/>
    <path d="M23 14V45 M46 14V45 M3 21H67 M3 30H67 M3 38H67" stroke="{borda}" stroke-width=".9" opacity=".75"/>
    <path d="M7 7h10m9 0h14m10 0h13" stroke="#ffffff" stroke-width="1.4" stroke-linecap="round"/>
    </svg>"##);
    // O SVG é montado aqui com cores válidas: falhar é erro de programação.
    pelicano::svg(pelicano::Svg::from_bytes(imagem.as_bytes()).expect("SVG da miniatura de tabela"))
        .width(70.0).height(47.0)
}
pub(super) fn icone_tabela() -> pelicano::SvgView {
    mini_tabela("azul").width(25.0).height(23.0)
}

impl App {
    pub(super) fn alternar_menu_estilos(&mut self, tabela: bool) -> Task<Msg> {
        let ativo = if tabela { !self.tabelas_menu_aberto } else { !self.estilos_menu_aberto };
        self.tabelas_menu_aberto = tabela && ativo;
        self.estilos_menu_aberto = !tabela && ativo;
        if ativo {
            self.condicional_menu_aberto = false;
            self.bordas_menu_aberto = false;
            self.colar_menu_aberto = false;
            self.orientacao_menu_aberto = false;
            self.menu_aberto = false;
            self.cor_ativa = None;
            self.contexto = None;
            self.tabela_popup_filtro = false;
            // Ancorar no botão que recebeu o clique, não no rodapé da Ribbon.
            // Deixar o popup ligeiramente alinhado à borda esquerda do botão.
            self.estilos_popup_x = (self.ponteiro_x - 25.0).max(MARGEM_POPUP);
            // A posição fixa antiga (232px) deixava a galeria distante.
            // Próximo da seta mesmo em janelas baixas; o conteúdo rola dentro.
            self.estilos_popup_y = (self.ponteiro_y + 25.0).max(130.0);
        }
        Task::none()
    }

    /// (x, y, largura, altura) do popup responsivo. Prioriza a âncora,
    /// diminui a largura e só então desloca a origem se for indispensável.
    /// O espaço vertical determina a altura da rolagem, não a posição do topo.
    fn geometria_popup_estilos(&self, tabela: bool) -> (f32, f32, f32, f32) {
        let janela_w = self.tamanho_principal.width.max(MARGEM_POPUP * 2.0 + 1.0);
        let janela_h = self.tamanho_principal.height.max(MARGEM_POPUP * 2.0 + 1.0);
        let (preferida, minima) = if tabela {
            (LARGURA_TABELAS, LARGURA_MIN_TABELAS)
        } else {
            (LARGURA_ESTILOS, LARGURA_MIN_ESTILOS)
        };
        let largura_min = minima.min(janela_w - 2.0 * MARGEM_POPUP);
        let x = self.estilos_popup_x.clamp(
            MARGEM_POPUP,
            (janela_w - largura_min - MARGEM_POPUP).max(MARGEM_POPUP),
        );
        let largura = (janela_w - x - MARGEM_POPUP).min(preferida).max(1.0);
        let y = self.estilos_popup_y.clamp(
            MARGEM_POPUP,
            (janela_h - MARGEM_POPUP - ALTURA_MIN_VISIVEL).max(MARGEM_POPUP),
        );
        let disponivel = (janela_h - y - MARGEM_POPUP).max(1.0);
        let desejada = if tabela {
            let linhas = TEMAS_TABELA.len().div_ceil(Self::colunas_tabela(largura));
            130.0 + linhas as f32 * 73.0
        } else {
            610.0
        };
        (x, y, largura, disponivel.min(desejada))
    }

    fn colunas_tabela(largura: f32) -> usize {
        // Cada cartão ocupa 78px + espaço de 5px entre cartões + 18px de borda.
        if largura >= 350.0 { 4 }
        else if largura >= 262.0 { 3 }
        else if largura >= 179.0 { 2 }
        else { 1 }
    }

    fn colunas_celulas(largura: f32) -> usize {
        if largura >= 376.0 { 3 }
        else if largura >= 185.0 { 2 }
        else { 1 }
    }

    fn largura_cartao_celula(largura: f32) -> f32 {
        let n = Self::colunas_celulas(largura);
        ((largura - 22.0 - (n - 1) as f32 * 5.0) / n as f32)
            .max(1.0).min(116.0)
    }

    fn miniatura_tabela(&self, nome: &'static str,
        selecionado: bool) -> Element<'_, Msg> {
        let corpo = pelicano::column![
            mini_tabela(nome),
            text(match nome {
                "padrao" => tr!("Padrão"), "azul" => tr!("Azul"), "laranja" => tr!("Laranja"),
                "verde" => tr!("Verde"), "vermelho" => tr!("Vermelho"), "roxo" => tr!("Roxo"), _ => tr!("Cinza"),
            }).size(10).color(COR_TEXTO()),
        ].spacing(2).align(pelicano::Align::Center);
        button(corpo)
            .width(Length::Fixed(78.0))
            .padding([4, 3])
            .style(estilo_item_menu(selecionado, true))
            .on_press(Msg::TabelaEstiloEscolher(nome))
            .into()
    }
    fn menu_tabelas(&self, largura: f32, altura: f32) -> Element<'_, Msg> {
        let ativa = tabelas::sob_celula(&self.workbook,self.folha,self.linha,self.coluna);
        let tema = ativa.as_ref().and_then(|t| t["estilo"]["tema"].as_str());
        let mut grade = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(5);
        for grupo in TEMAS_TABELA.chunks(Self::colunas_tabela(largura)) {
            let mut linha = row(Vec::<pelicano::Element<_>>::new()).spacing(5);
            for &nome in grupo {
                linha = linha.push(self.miniatura_tabela(nome, tema == Some(nome)));
            }
            grade = grade.push(linha);
        }
        let botoes = pelicano::column![
            secao(tr!("CRIAR TABELA")),
            button(row![
                icone_tabela(),
                text(tr!("Tabela…")).size(13),
                Space::new(Length::FILL, 0.0),
                text("Ctrl+T").size(11).color(COR_TEXTO_MUTED()),
            ].spacing(9).align(pelicano::Align::Center))
                .width(Length::FILL).padding([9, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaCriarPeloMenu),
            separador(),
            secao(tr!("ESTILOS DE TABELA")),
            grade,
            container(text(if ativa.is_some() {
                tr!("Selecione um estilo para a tabela atual.")
            } else {
                tr!("Escolha uma cor para criar uma nova tabela.")
            }).size(11).color(COR_TEXTO_MUTED())).padding([6, 4]),
        ].spacing(5).padding(9);
        container(pelicano::scrollable(botoes)
            .height(Length::Fixed(altura)))
            .width(Length::Fixed(largura))
            .style(estilo_popup_menu).into()
    }
    fn item_celula(&self, p: &'static Preset, largura: f32) -> Element<'_, Msg> {
        let nome = tr!(p.nome);
        let cor = cor_hex(p.texto);
        let fundo = cor_hex(p.fundo);
        let tamanho = if p.tamanho >= 20.0 { 17.0 }
            else if p.tamanho >= 17.0 { 14.0 } else { 11.0 };
        button(container(text(nome).size(tamanho).color(cor)
            .no_wrap())
            .height(Length::Fixed(31.0)).align_y(pelicano::Align::Center)
            .padding([0, 5])
            .width(Length::FILL)
            .style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(fundo)),
                border: pelicano::Border {
                    color: COR_BORDA_SUAVE(), width: 1.0, radius: [3.0; 4],
                },
                ..Default::default()
            }))
        .width(Length::Fixed(largura)).padding(2)
        .style(estilo_item_menu(false, true))
        .on_press(Msg::EstiloCelulaAplicar(p.id))
        .into()
    }
    fn grupo_celulas(&self, titulo: &'static str, presets: &'static [Preset],
        colunas: usize, largura_item: f32) -> Element<'_, Msg> {
        let mut grade = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(3);
        for grupo in presets.chunks(colunas) {
            let mut linha = row(Vec::<pelicano::Element<_>>::new()).spacing(5);
            for p in grupo {
                linha = linha.push(self.item_celula(p, largura_item));
            }
            grade = grade.push(linha);
        }
        pelicano::column![secao(titulo), grade, separador()].spacing(5).into()
    }
    /// "Personalizados": os estilos criados pela pessoa, com o próprio
    /// formato na amostra e a lixeira para excluir.
    fn grupo_pessoais(&self, colunas: usize, largura_item: f32) -> Element<'_, Msg> {
        let lista = estilo_novo::da_pasta(&self.workbook);
        if lista.is_empty() { return Space::new(0.0, Length::Shrink).into(); }
        let mut grade = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(3);
        let itens: Vec<(usize, estilo_novo::EstiloPessoal)> = lista.into_iter().enumerate().collect();
        for grupo in itens.chunks(colunas) {
            let mut linha = row(Vec::<pelicano::Element<_>>::new()).spacing(5);
            for (i, e) in grupo {
                let s = feathersheet_core::celulas::estilo_parcial::para_style(&e.estilo);
                let cor = s.color.as_deref().map(cor_hex).unwrap_or(COR_TEXTO());
                let fundo = s.bg.as_deref().map(cor_hex).unwrap_or(pelicano::Color::WHITE);
                let i = *i;
                linha = linha.push(row![
                    button(container(text(e.nome.clone()).size(11).color(cor)
                        .font(pelicano::Font { weight: if s.bold { 700 } else { 400 }, ..Default::default() })
                        .no_wrap())
                        .height(Length::Fixed(31.0)).align_y(pelicano::Align::Center).padding([0, 5]).width(Length::FILL)
                        .style(move |_| pelicano::Style {
                            background: Some(pelicano::Background::Color(fundo)),
                            border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [3.0; 4] },
                            ..Default::default()
                        }))
                        .width(Length::FILL).padding(2)
                        .style(estilo_item_menu(false, true))
                        .on_press(Msg::NovoEstilo(estilo_novo::Evento::Aplicar(i))),
                    button(feather_ui::icones::icone("trash", 13.0)).padding([4, 2])
                        .style(estilo_item_menu(false, true))
                        .on_press(Msg::NovoEstilo(estilo_novo::Evento::Excluir(i))),
                ].align(pelicano::Align::Center).width(Length::Fixed(largura_item)));
            }
            grade = grade.push(linha);
        }
        pelicano::column![secao("PERSONALIZADOS"), grade, separador()].spacing(5).into()
    }

    fn menu_estilos_celula(&self, largura: f32, altura: f32) -> Element<'_, Msg> {
        let colunas = Self::colunas_celulas(largura);
        let largura_item = Self::largura_cartao_celula(largura);
        let itens = pelicano::column![
            self.grupo_pessoais(colunas, largura_item),
            self.grupo_celulas(tr!("BOM, RUIM E NEUTRO"), BASICOS, colunas, largura_item),
            self.grupo_celulas(tr!("DADOS E MODELO"), DADOS, colunas, largura_item),
            self.grupo_celulas(tr!("TÍTULOS E CABEÇALHOS"), TITULOS, colunas, largura_item),
            self.grupo_celulas(tr!("ESTILOS DE CÉLULA COM TEMA"), ACENTOS, colunas, largura_item),
            button(row![
                feather_ui::icones::icone("plus", 16.0),
                text(tr!("Novo estilo de célula…")).size(12.5),
            ].spacing(8).align(pelicano::Align::Center))
                .width(Length::FILL).padding([9, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::NovoEstilo(estilo_novo::Evento::Abrir)),
            button(row![
                feather_ui::icones::icone("eraser", 16.0),
                text(tr!("Limpar formatação")).size(12.5),
            ].spacing(8).align(pelicano::Align::Center))
                .width(Length::FILL).padding([9, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::LimparFormato),
        ].spacing(5).padding(9);
        container(pelicano::scrollable(itens).height(Length::Fixed(altura)))
            .width(Length::Fixed(largura))
            .style(estilo_popup_menu).into()
    }
    pub(super) fn sobrepor_estilos<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let tabela = self.tabelas_menu_aberto;
        let celula = self.estilos_menu_aberto;
        if !tabela && !celula { return base; }
        let (x, y, largura, altura) = self.geometria_popup_estilos(tabela);
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::FecharMenusEstilos);
        let menu = if tabela {
            self.menu_tabelas(largura, altura)
        } else {
            self.menu_estilos_celula(largura, altura)
        };
        stack![base, fora, pelicano::column![
            Space::new(0.0, Length::Fixed(y)),
            row![Space::new(Length::Fixed(x), 0.0), menu],
        ]].height(Length::FILL).into()
    }
    pub(super) fn aplicar_preset_celula(&mut self, id: &'static str) {
        let Some(p) = todos().find(|p| p.id == id) else { return; };
        self.confirmar();
        let mut patch = EstiloParcial::new();
        // Redefine os campos do preset de forma atômica, preservando fórmulas,
        // valores, bordas, formato numérico e campos não relacionados.
        for campo in ["bg", "color", "fontSize", "bold"] {
            patch.insert(campo.into(), Campo::Nulo);
        }
        if id != "normal" {
            patch.insert("bg".into(), Campo::Texto(p.fundo.into()));
            patch.insert("color".into(), Campo::Texto(p.texto.into()));
            patch.insert("fontSize".into(), Campo::Numero(p.tamanho));
            patch.insert("bold".into(), Campo::Logico(p.negrito));
        }
        let area = self.area_selecionada();
        self.workbook.editar(self.folha, |aba| aba.set_style(&area, &patch));
        self.alterado = true;
        self.status = trf!("Estilo de célula \"{}\" aplicado.", tr!(p.nome));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn galerias_oferecem_padroes_e_cores_suportadas_pelo_motor() {
        assert_eq!(TEMAS_TABELA.len(), 7);
        assert!(TEMAS_TABELA.contains(&"roxo"));
        assert_eq!(todos().count(), 27);
        for p in todos() {
            assert!(p.fundo.starts_with('#') && p.texto.starts_with('#'));
            assert!(!p.nome.is_empty());
        }
    }
    #[test]
    fn estilo_aplicado_na_selecao_preserva_valor_e_pode_voltar_ao_normal() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "42".into());
        app.update(Msg::EstiloCelulaAplicar("bom"));
        let e = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0).unwrap();
        assert_eq!(e.bg.as_deref(), Some("#cdf4d9"));
        assert!(e.bold);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "42");
        app.update(Msg::EstiloCelulaAplicar("normal"));
        let e = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0).unwrap_or_default();
        assert!(e.bg.is_none());
    }
    #[test]
    fn paleta_inicializa_criacao_de_tabela_e_grava_tema_escolhido() {
        let mut app = App::default();
        app.update(Msg::TabelaEstiloEscolher("roxo"));
        assert!(app.tabela_area.is_some());
        assert_eq!(app.tabela_estilo_pendente, Some("roxo"));
        app.update(Msg::TabelaConfirmar);
        let tabela = tabelas::sob_celula(&app.workbook, 0, 0, 0).unwrap();
        assert_eq!(tabela["estilo"]["tema"], "roxo");
        assert_eq!(app.tabela_estilo_pendente, None);
    }

    #[test]
    fn tabela_abre_abaixo_do_botao_sem_saltar_para_esquerda() {
        let mut app = App::default();
        app.tamanho_principal = Size::new(468.0, 803.0);
        app.ponteiro_x = 215.0;
        app.ponteiro_y = 127.0;
        app.update(Msg::AlternarMenuTabelas);
        let (x, y, largura, altura) = app.geometria_popup_estilos(true);
        assert_eq!(x, 190.0);
        assert_eq!(y, 152.0);
        assert!(y < 170.0, "menu de tabela deve ficar junto à seta");
        assert!(largura >= 262.0 && largura < LARGURA_TABELAS);
        assert_eq!(App::colunas_tabela(largura), 3);
        assert!(x + largura <= 468.0 - MARGEM_POPUP + 0.1);
        assert!(y + altura <= 803.0 - MARGEM_POPUP + 0.1);
        let _: Element<'_, Msg> = app.menu_tabelas(largura, altura);
        let _: Element<'_, Msg> = app.sobrepor_estilos(
            Space::new(Length::FILL, Length::FILL).into(),
        );
    }

    #[test]
    fn estilos_celula_mantem_ancora_e_reorganiza_cartoes() {
        let mut app = App::default();
        app.tamanho_principal = Size::new(468.0, 803.0);
        app.ponteiro_x = 298.0;
        app.update(Msg::AlternarMenuEstilos);
        let (x, y, largura, altura) = app.geometria_popup_estilos(false);
        assert_eq!(x, 270.0);
        assert_eq!(y, 145.0);
        assert!(y < 170.0, "estilos de célula deve ficar junto à seta");
        assert!(largura >= 190.0 && largura < LARGURA_ESTILOS);
        assert_eq!(App::colunas_celulas(largura), 2);
        assert!(App::largura_cartao_celula(largura) <= 116.0);
        assert!(x + largura <= 468.0 - MARGEM_POPUP + 0.1);
        assert!(y + altura <= 803.0 - MARGEM_POPUP + 0.1);
        let _: Element<'_, Msg> = app.menu_estilos_celula(largura, altura);
        let _: Element<'_, Msg> = app.sobrepor_estilos(
            Space::new(Length::FILL, Length::FILL).into(),
        );
    }

    #[test]
    fn galeria_de_tabela_abre_junto_ao_botao_em_janela_compacta() {
        let mut app = App::default();
        app.tamanho_principal = Size::new(355.0, 707.0);
        // Referência do botão Tabela na faixa da captura: aproximadamente x=155, y=110.
        app.ponteiro_x = 155.0;
        app.ponteiro_y = 110.0;
        app.update(Msg::AlternarMenuTabelas);
        let (x, y, largura, altura) = app.geometria_popup_estilos(true);
        assert_eq!((x, y), (130.0, 135.0));
        assert!(y < 160.0, "não deve abrir lá embaixo, junto à grade");
        assert!(x + largura <= app.tamanho_principal.width - MARGEM_POPUP + 0.1);
        assert!(y + altura <= app.tamanho_principal.height - MARGEM_POPUP + 0.1);
        let _: Element<'_, Msg> = app.menu_tabelas(largura, altura);
    }

    #[test]
    fn galerias_cabem_em_janela_estreita_baixa_e_larga() {
        let mut app = App::default();
        for (w, h, clique_x) in [
            (320.0, 340.0, 270.0),
            (270.0, 250.0, 245.0),
            (1280.0, 900.0, 735.0),
        ] {
            app.tamanho_principal = Size::new(w, h);
            app.ponteiro_x = clique_x;
            for tabela in [true, false] {
                app.alternar_menu_estilos(tabela);
                let (x, y, largura, altura) = app.geometria_popup_estilos(tabela);
                assert!(x >= MARGEM_POPUP);
                assert!(y >= MARGEM_POPUP);
                assert!(largura > 0.0 && altura > 0.0);
                assert!(x + largura <= w - MARGEM_POPUP + 0.1);
                assert!(y + altura <= h - MARGEM_POPUP + 0.1);
                if tabela {
                    let _: Element<'_, Msg> = app.menu_tabelas(largura, altura);
                } else {
                    let _: Element<'_, Msg> = app.menu_estilos_celula(largura, altura);
                }
                app.update(Msg::FecharMenusEstilos);
            }
        }
        // Em largura normal continuam as galerias completas do mockup.
        assert_eq!(App::colunas_tabela(LARGURA_TABELAS), 4);
        assert_eq!(App::colunas_celulas(LARGURA_ESTILOS), 3);
    }

    #[test]
    fn popups_sao_flutuantes_e_fecham_com_escape_ou_selecao() {
        let mut app = App::default();
        app.update(Msg::AlternarMenuTabelas);
        assert!(app.tabelas_menu_aberto && !app.estilos_menu_aberto);
        let (_, _, largura, altura) = app.geometria_popup_estilos(true);
        let _: Element<'_,Msg> = app.menu_tabelas(largura, altura);
        app.update(Msg::AlternarMenuEstilos);
        assert!(!app.tabelas_menu_aberto && app.estilos_menu_aberto);
        let (_, _, largura, altura) = app.geometria_popup_estilos(false);
        let _: Element<'_,Msg> = app.menu_estilos_celula(largura, altura);
        app.update(Msg::EstiloCelulaAplicar("ruim"));
        assert!(!app.estilos_menu_aberto);
    }
}
