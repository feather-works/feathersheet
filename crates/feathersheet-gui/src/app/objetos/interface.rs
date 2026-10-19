//! A tela dos objetos soltos: a camada sobre a grade, a barra flutuante do
//! objeto selecionado, os painéis de cor, o menu Formas (miniaturas), o menu
//! de contexto da forma, a aba contextual "Editar forma", o campo de texto
//! da forma e o diálogo Atribuir macro.
//!
//! Tudo aqui só MONTA widgets e manda [`Evento`]; a lógica fica em
//! `objetos.rs` (`evento_objetos`). A barra, a aba e o menu de contexto
//! mandam as mesmas mensagens.
use super::camada::{caminhos_centrados, cor_hex, cor_texto, desenhar_icone, posicao_texto, tamanho_texto, Camada, CAB_X, CAB_Y};
use super::*;
use crate::app::menu_contexto::{self, ItemMenu};
use iced::widget::canvas::{self as cv, Frame, Geometry, Program, Stroke};
use iced::widget::{column, mouse_area, opaque, tooltip};
use iced::{Color, Rectangle, Renderer, Theme, Vector};

/// Id da busca da galeria de ícones (recebe o foco ao abrir).
pub(in crate::app) const ID_BUSCA_ICONES: &str = "feathersheet.objeto.busca_icones";
/// Id da grade rolável da galeria (volta ao topo a cada busca).
pub(in crate::app) const ID_GRADE_ICONES: &str = "feathersheet.objeto.grade_icones";
/// Galeria de ícones: por fileira, lado da miniatura, altura de uma fileira
/// (miniatura + padding 2×5 + espaçamento 2) e fileiras extras de folga
/// acima/abaixo da parte visível.
const ICONES_POR_FILEIRA: usize = 8;
const ALTURA_FILEIRA_ICONES: f32 = LADO_ICONE_MENU + 10.0 + 2.0;
const FOLGA_FILEIRAS: usize = 3;
const LADO_ICONE_MENU: f32 = 28.0;
const LARGURA_MENU_ICONES: f32 = 8.0 * (LADO_ICONE_MENU + 10.0 + 2.0) + 36.0;

/// Id do campo de texto da forma (recebe o foco ao editar).
pub(in crate::app) const ID_TEXTO: &str = "feathersheet.objeto.texto";
/// Índice do submenu "Trazer para frente ▸" no menu de contexto da forma.
pub(in crate::app) const SUB_FRENTE: usize = 7;
/// Largura estimada da barra flutuante (para não sair da janela).
/// Altura da barra flutuante (botões de 16 px com folga).
const ALTURA_BARRA: f32 = 38.0;
const LARGURA_BARRA: f32 = 380.0;
/// Menu Formas: miniaturas por fileira, lado da miniatura e largura.
const POR_FILEIRA: usize = 9;
const LADO_MINIATURA: f32 = 26.0;
const LARGURA_MENU: f32 = 9.0 * (LADO_MINIATURA + 8.0 + 4.0) + 36.0;
const LARGURA_PAINEL: f32 = 200.0;
/// Galeria do WordArt: lado da miniatura de estilo e de deformação, e largura.
const MINI_WORDART: (f32, f32) = (96.0, 44.0);
const MINI_DEFORMACAO: (f32, f32) = (72.0, 34.0);
const LARGURA_MENU_WORDART: f32 = 4.0 * (MINI_WORDART.0 + 10.0 + 6.0) + 18.0;

/// Ícone sozinho sempre com dica (GUIA-JANELAS, seção 6).
fn dica<'a>(conteudo: impl Into<Element<'a, Msg>>, texto: &'static str) -> Element<'a, Msg> {
    tooltip(conteudo, container(text(texto).size(11)).padding([4, 8]).style(estilo_dica), tooltip::Position::Bottom).into()
}

fn ev(e: Evento) -> Msg {
    Msg::Objetos(e)
}

/// Campo numérico curto (px). Sem `ativo`, fica desabilitado.
fn campo_num<'a>(valor: &'a str, largura: f32, ativo: bool, ao_digitar: impl Fn(String) -> Msg + 'a, enviar: Msg) -> Element<'a, Msg> {
    let mut c = text_input("0", valor).size(12).padding([3, 5]).width(Length::Fixed(largura));
    if ativo {
        c = c.on_input(ao_digitar).on_submit(enviar);
    }
    c.into()
}

/// Miniatura de uma forma do catálogo (menu Formas).
struct Miniatura(&'static str);

impl Program<Msg> for Miniatura {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let m = 3.0;
        let (w, h) = (bounds.width - 2.0 * m, bounds.height - 2.0 * m);
        frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
        let (cheia, tracos) = caminhos_centrados(self.0, &[], w, h, (false, false));
        let traco = Stroke::default().with_color(COR_TEXTO()).with_width(1.3).with_line_join(cv::LineJoin::Round);
        frame.stroke(&cheia, traco);
        frame.stroke(&tracos, traco);
        vec![frame.into_geometry()]
    }
}

/// Miniatura de um ícone Tabler (galeria de ícones).
struct MiniIcone(&'static str);

impl Program<Msg> for MiniIcone {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
        desenhar_icone(&mut frame, self.0, bounds.width, bounds.height, (false, false), 0.75, COR_TEXTO());
        vec![frame.into_geometry()]
    }
}

/// Miniatura de um estilo pronto do WordArt ("Abc" no estilo, pelo mesmo desenho da camada).
struct MiniWordArt(usize);

impl Program<Msg> for MiniWordArt {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
        let e = feather_shapes::wordart::estilo(self.0);
        let wa = super::wordart::WordArtTela {
            modo: feather_shapes::wordart::Transformacao::Reto,
            intensidade: 0.0,
            sombra: e.sombra.map(|s| super::wordart::SombraTela { cor: cor_hex(s.cor, 0.0), dx: s.dx / 2.0, dy: s.dy / 2.0 }),
        };
        let contorno = (!e.contorno.is_empty()).then(|| cor_hex(e.contorno, 0.0));
        super::wordart::desenhar(&mut frame, "Abc", &wa, bounds.width, bounds.height, (false, false),
            Some(cor_hex(e.preenchimento, 0.0)), contorno, e.espessura, 1.0);
        vec![frame.into_geometry()]
    }
}

/// Miniatura de uma deformação do WordArt ("Abc" deformado na cor do texto).
struct MiniDeformacao(feather_shapes::wordart::Transformacao);

impl Program<Msg> for MiniDeformacao {
    type State = ();
    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
        let wa = super::wordart::WordArtTela { modo: self.0, intensidade: 0.8, sombra: None };
        super::wordart::desenhar(&mut frame, "Abc", &wa, bounds.width, bounds.height, (false, false), Some(COR_TEXTO()), None, 0.0, 1.0);
        vec![frame.into_geometry()]
    }
}

impl App {
    /// A camada de objetos por cima da grade, a barra do selecionado e o
    /// campo de texto da forma em edição.
    pub(in crate::app) fn grade_com_objetos<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(vista) = self.vista_tinta() else { return base };
        let Some(aba) = self.workbook.aba(self.folha) else { return base };
        let Some((aba_id, lidos)) = self.objetos_da_aba() else { return base };
        let graficos: Vec<super::camada::GraficoTela> = self.graficos_da_camada().into_iter()
            .map(|(z, ret, svg)| super::camada::GraficoTela::novo(z, ret, svg)).collect();
        if lidos.objetos.is_empty() && self.objetos.inserir.is_none() && graficos.is_empty() {
            return base;
        }
        let regua = ReguaAba(&aba.sheet);
        let mut itens: Vec<ItemTela> = lidos.objetos.iter().map(|o| ItemTela::de(o, &regua)).collect();
        // Texto que começa com "=" é fórmula, como no Excel: a forma mostra o
        // resultado (=A2 mostra A2 como a grade mostra) e acompanha a célula.
        for it in &mut itens {
            if let Some(corpo) = it.texto.as_deref().and_then(|t| t.strip_prefix('=')).filter(|c| !c.trim().is_empty()) {
                it.texto = Some(self.texto_de_formula_da_forma(corpo));
            }
        }
        let selecionado = self.objetos.selecionado.filter(|(a, id)| *a == aba_id && lidos.obter(*id).is_some()).map(|(_, id)| id);
        let editando = self.objetos.texto.as_ref().map(|(id, _)| *id).filter(|id| selecionado == Some(*id));
        let camada = Camada { itens, graficos, selecionado, inserir: self.objetos.inserir, vista, travar: self.objetos.travar, editando };
        let tela = selecionado.and_then(|id| camada.itens.iter().find(|i| i.id == id)).map(|it| camada.ret_tela(it.ret));
        let giro = selecionado.and_then(|id| camada.itens.iter().find(|i| i.id == id)).map_or(0.0, |i| i.giro);
        let item_editado = editando.and_then(|id| camada.itens.iter().find(|i| i.id == id)).cloned();
        let mut camadas: Vec<Element<'a, Msg>> =
            vec![base, cv::Canvas::new(camada).width(Length::Fill).height(Length::Fill).into()];
        if let (Some(r), Some(o)) = (tela, selecionado.and_then(|id| lidos.obter(id))) {
            let z = vista.zoom;
            let (min_x, min_y) = (CAB_X * z + 4.0, CAB_Y * z + 4.0);
            match (&self.objetos.texto, editando) {
                (Some((_, t)), Some(_)) => {
                    // Edita no lugar: sem caixa branca, na letra, cor e
                    // alinhamento do texto da forma (o desenho dele some).
                    let it = item_editado.expect("o item editado está na camada");
                    let tamanho = tamanho_texto(z);
                    let alto = tamanho * 1.45;
                    let margem = 7.0 * z;
                    let (ancora, h, _) = posicao_texto(r, it.alinhar, z);
                    let y = match it.alinhar.v {
                        modelo::AlinharV::Topo => ancora.y,
                        modelo::AlinharV::Meio => ancora.y - alto / 2.0,
                        modelo::AlinharV::Baixo => ancora.y - alto,
                    };
                    let largura = (r.width - 2.0 * margem).max(40.0);
                    let cor = cor_texto(&it);
                    let campo = text_input(tr!("Digite o texto"), t)
                        .id(text_input::Id::new(ID_TEXTO))
                        .on_input(|t| ev(Evento::TextoDigitado(t)))
                        .on_submit(ev(Evento::TextoConfirmar))
                        .size(tamanho)
                        .padding(0)
                        .align_x(h)
                        .width(Length::Fixed(largura))
                        .style(move |_: &Theme, _| text_input::Style {
                            background: iced::Background::Color(Color::TRANSPARENT),
                            border: iced::Border::default(),
                            icon: cor,
                            placeholder: Color { a: 0.55, ..cor },
                            value: cor,
                            selection: Color { a: 0.35, ..COR_AZUL_ACENTO() },
                        });
                    camadas.push(column![Space::with_height(y.max(min_y)), row![Space::with_width((r.x + margem).max(min_x)), campo]].into());
                }
                _ => {
                    // A caixa da forma girada (não o círculo que a envolve
                    // em qualquer giro): a barra fica logo acima da alça de
                    // giro, sem o vão de antes.
                    let (s, c) = giro.to_radians().sin_cos();
                    let meia = (r.width * s.abs() + r.height * c.abs()) / 2.0;
                    let topo = r.center_y() - meia;
                    let acima = topo - super::camada::DIST_GIRO - 8.0 - ALTURA_BARRA;
                    let y = if acima > min_y { acima } else { r.center_y() + meia + 10.0 };
                    let x = r.x.clamp(min_x, (self.tamanho_principal.width - LARGURA_BARRA).max(min_x));
                    camadas.push(
                        column![Space::with_height(y.max(min_y)), row![Space::with_width(x), self.barra_objeto(o)]].into(),
                    );
                }
            }
        }
        stack(camadas).width(Length::Fill).height(Length::Fill).into()
    }

    /// O resultado da fórmula do texto de uma forma, já formatado.
    fn texto_de_formula_da_forma(&self, corpo: &str) -> String {
        use feathersheet_core::{formula::ast::Node, planilha::bloco::texto_formatado};
        let Ok(no) = feathersheet_core::formula::parser::parse_formula(corpo) else {
            return "#NOME?".into();
        };
        if let Node::Ref { row, col, sheet } = &no {
            let indice = sheet.as_deref().map_or(Some(self.folha), |n| self.workbook.indice_por_nome(n));
            let Some(indice) = indice else { return "#REF!".into() };
            let estilo = self.workbook.aba(indice).and_then(|a| a.sheet.get_style(*row, *col));
            return texto_formatado(&self.workbook.valor(indice, *row, *col), estilo);
        }
        texto_formatado(&self.workbook.avaliar_em(self.folha, 0, 0, &no), None)
    }

    /// Barra flutuante do objeto selecionado.
    fn barra_objeto<'a>(&'a self, o: &ObjetoSolto) -> Element<'a, Msg> {
        let amostra = |hex: Option<&str>| -> Element<'a, Msg> {
            let cor = hex.map(|h| cor_hex(h, 0.0));
            container(Space::new(Length::Fixed(14.0), Length::Fixed(4.0)))
                .style(move |_| container::Style {
                    background: cor.map(iced::Background::Color),
                    border: iced::Border { color: COR_BORDA(), width: if cor.is_some() { 0.0 } else { 1.0 }, radius: 1.0.into() },
                    ..Default::default()
                })
                .into()
        };
        let com_seta = |icone: &'static str, rodape: Element<'a, Msg>, painel: Painel| -> Element<'a, Msg> {
            button(
                row![
                    column![feather_ui::icones::icone(icone, 16.0), rodape].spacing(1).align_x(Alignment::Center),
                    feather_ui::icones::icone("chevron-down", 11.0),
                ]
                .spacing(3)
                .align_y(Alignment::Center),
            )
            .padding([4, 6])
            .style(estilo_botao_pequeno(self.objetos.painel == Some(painel)))
            .on_press(ev(Evento::AbrirPainel(painel)))
            .into()
        };
        let so_icone = |icone: &'static str, msg: Msg| -> Element<'a, Msg> {
            button(feather_ui::icones::icone(icone, 16.0)).padding(6).style(estilo_botao_pequeno(false)).on_press(msg).into()
        };
        let icone = o.tipo == modelo::TipoObjeto::Icone;
        let wordart = o.tipo == modelo::TipoObjeto::Wordart;
        let linha = icone || (o.tipo == modelo::TipoObjeto::Forma && (geometria::so_traco(&o.forma) || modelo::e_linha(&o.forma)));
        let mut barra = row![].spacing(2).align_y(Alignment::Center);
        if wordart {
            barra = barra.push(dica(
                button(feather_ui::icones::icone("wordart", 16.0))
                    .padding(6)
                    .style(estilo_botao_pequeno(self.objetos.menu == Some(ModoMenu::WordArt)))
                    .on_press(ev(Evento::AlternarMenu(ModoMenu::WordArt))),
                tr!("Estilo e deformação do WordArt"),
            ));
        }
        if !linha {
            barra = barra.push(dica(
                com_seta("bucket", amostra(o.aparencia.preenchimento.as_deref()), Painel::Preenchimento),
                if wordart { tr!("Cor das letras") } else { tr!("Preenchimento da forma") },
            ));
        }
        barra = barra
            .push(dica(com_seta("pencil", amostra(o.aparencia.contorno.as_deref()), Painel::Contorno),
                if icone { tr!("Cor do ícone, espessura do traço e transparência") } else if wordart { tr!("Contorno das letras: cor, espessura e transparência") } else { tr!("Contorno: cor, espessura e transparência") }))
            .push(dica(so_icone("arrow-up", ev(Evento::Ordem(Ordem::Frente))), tr!("Trazer para a frente")))
            .push(dica(so_icone("arrow-down", ev(Evento::Ordem(Ordem::Tras))), tr!("Enviar para trás")));
        if !icone && geometria::raios(&o.forma, &o.ajustes).is_some() {
            let cantos = row![
                feather_ui::icones::icone("square", 16.0),
                campo_num(&self.objetos.campos.raio, 44.0, true, |t| ev(Evento::CampoRaio(t)), ev(Evento::AplicarRaio)),
                text("px").size(11).color(COR_TEXTO_MUTED()),
            ]
            .spacing(4)
            .align_y(Alignment::Center);
            barra = barra.push(dica(container(cantos).padding([0, 4]), tr!("Arredondar cantos (raio em px, Enter aplica; por canto na aba Editar forma)")));
        }
        barra = barra
            .push(dica(so_icone("palette", ev(Evento::FormatarForma)), tr!("Editar forma (abre a aba da faixa)")))
            .push(dica(so_icone("trash", ev(Evento::Apagar)), tr!("Excluir forma (Delete)")));
        opaque(container(barra).padding(3).style(estilo_popup_menu))
    }

    /// Painel de cores (preenchimento ou contorno com espessura) e transparência.
    fn painel_objeto<'a>(&self, painel: Painel, o: &ObjetoSolto) -> Element<'a, Msg> {
        // Com o seletor aberto, o painel mostra o rascunho (ainda sem desfazer).
        let (atual, transparencia) = self.cor_do_painel(painel, o);
        let amostra = |hex: &'static str| -> Element<'a, Msg> {
            let escolhida = atual.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(hex));
            let cor = cor_hex(hex, 0.0);
            let msg = match painel {
                Painel::Preenchimento => Evento::Preenchimento(Some(hex)),
                Painel::Contorno => Evento::Contorno(Some(hex)),
            };
            button(Space::new(Length::Fixed(20.0), Length::Fixed(20.0)))
                .padding(0)
                .on_press(ev(msg))
                .style(move |_: &Theme, status| button::Style {
                    background: Some(iced::Background::Color(cor)),
                    border: iced::Border {
                        color: if escolhida || matches!(status, button::Status::Hovered) { COR_AZUL_ACENTO() } else { COR_BORDA() },
                        width: if escolhida { 2.5 } else { 1.0 },
                        radius: 5.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };
        let mut cores = column![].spacing(6);
        for fileira in PALETA.chunks(6) {
            cores = cores.push(row(fileira.iter().map(|&c| amostra(c))).spacing(6));
        }
        let (titulo, sem, msg_sem) = match painel {
            Painel::Preenchimento => (tr!("Preenchimento"), tr!("Sem preenchimento"), Evento::Preenchimento(None)),
            Painel::Contorno => (tr!("Contorno"), tr!("Sem contorno"), Evento::Contorno(None)),
        };
        let rotulo = |t: &'static str| text(t).size(10.5).color(COR_TEXTO_MUTED());
        let mut corpo = column![
            text(titulo).size(13),
            rotulo("COR"),
            cores,
            button(text(sem).size(12))
                .width(Length::Fill)
                .padding([5, 8])
                .style(estilo_item_menu(atual.is_none(), true))
                .on_press(ev(msg_sem)),
            button(row![feather_ui::icones::icone("palette", 14.0), text(tr!("Mais cores…")).size(12)].spacing(6).align_y(Alignment::Center))
                .width(Length::Fill)
                .padding([5, 8])
                .style(estilo_item_menu(self.objetos.mais_cores, true))
                .on_press(ev(Evento::MaisCores)),
        ]
        .spacing(8);
        if painel == Painel::Contorno {
            let cor_linha = o.aparencia.contorno.as_deref().map_or(COR_TEXTO(), |c| cor_hex(c, 0.0));
            let mut grossuras = column![].spacing(2);
            for e in ESPESSURAS {
                let traco = container(Space::with_height((e * 1.2).clamp(1.0, 10.0))).width(60.0).style(move |_| container::Style {
                    background: Some(iced::Background::Color(cor_linha)),
                    ..Default::default()
                });
                grossuras = grossuras.push(
                    button(
                        row![container(traco).width(66.0).center_y(14.0), text(trf!("{} px", decimal(e))).size(12)]
                            .spacing(8)
                            .align_y(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding([3, 8])
                    .style(estilo_botao_pequeno((o.aparencia.espessura - e).abs() < 0.01))
                    .on_press(ev(Evento::Espessura(e))),
                );
            }
            corpo = corpo.push(rotulo("ESPESSURA")).push(grossuras);
        }
        // Botões com o valor escrito (o controle deslizante saía vazio).
        let transparencias = row(TRANSPARENCIAS.iter().map(|&t| {
            button(text(format!("{:.0}%", t * 100.0)).size(12))
                .padding([4, 7])
                .style(estilo_botao_pequeno((transparencia - t).abs() < 0.01))
                .on_press(ev(Evento::Transparencia(t)))
                .into()
        }))
        .spacing(4);
        corpo = corpo.push(rotulo(tr!("TRANSPARÊNCIA"))).push(transparencias);
        let painel_cores = container(corpo.padding(12).width(Length::Fixed(LARGURA_PAINEL))).style(estilo_popup_menu);
        if !self.objetos.mais_cores {
            return painel_cores.into();
        }
        // Seletor completo ao lado: o alfa vira a transparência da forma.
        let alfa = ((1.0 - transparencia).clamp(0.0, 1.0) * 255.0).round() as u8;
        let atual_rgba = atual.as_deref().map_or_else(String::new, |c| {
            let [r, g, b, _] = seletor_cor::rgba_de(c);
            seletor_cor::hex_de([r, g, b, alfa])
        });
        let seletor = seletor_cor::painel(titulo, &atual_rgba, self.objetos.cor_hsva, &self.objetos.cor_hex).map(|e| ev(Evento::Cor(e)));
        row![painel_cores, seletor].spacing(6).into()
    }

    /// Inserir > Ícones: busca (português ou inglês) e a grade dos ícones
    /// Tabler do `feather-shapes`, como a galeria do FeatherSlide.
    fn menu_icones(&self, alto: f32) -> Element<'_, Msg> {
        let busca = &self.objetos.busca_icones;
        let achados = self.icones_da_busca();
        let altura_grade = (alto - 130.0).max(120.0);
        // Rolagem infinita sem montar tudo: só as fileiras à vista (com
        // folga) viram widgets; acima e abaixo, espaço da mesma altura.
        let fileiras = achados.len().div_ceil(ICONES_POR_FILEIRA);
        let primeira = ((self.objetos.rolagem_icones / ALTURA_FILEIRA_ICONES) as usize).saturating_sub(FOLGA_FILEIRAS).min(fileiras);
        let ultima = (primeira + (altura_grade / ALTURA_FILEIRA_ICONES).ceil() as usize + 2 * FOLGA_FILEIRAS).min(fileiras);
        let mut grade = column![Space::with_height(primeira as f32 * ALTURA_FILEIRA_ICONES)];
        for fileira in achados[primeira * ICONES_POR_FILEIRA..(ultima * ICONES_POR_FILEIRA).min(achados.len())].chunks(ICONES_POR_FILEIRA) {
            grade = grade.push(
                row(fileira.iter().map(|ic| {
                    let miniatura = cv::Canvas::new(MiniIcone(ic.id))
                        .width(Length::Fixed(LADO_ICONE_MENU))
                        .height(Length::Fixed(LADO_ICONE_MENU));
                    let botao = button(miniatura).padding(5).style(estilo_botao_pequeno(false)).on_press(ev(Evento::InserirIcone(ic.id)));
                    tooltip(botao, container(text(ic.rotulo()).size(11)).padding([4, 8]).style(estilo_dica), tooltip::Position::Bottom).into()
                }))
                .spacing(2)
                .height(Length::Fixed(ALTURA_FILEIRA_ICONES)),
            );
        }
        grade = grade.push(Space::with_height((fileiras - ultima) as f32 * ALTURA_FILEIRA_ICONES));
        let resumo = if busca.trim().is_empty() {
            trf!("Mais usados primeiro — role para ver todos os {} ícones", achados.len())
        } else if achados.is_empty() {
            tr!("Nenhum ícone encontrado. Tente outra palavra (casa, seta, gráfico, user, chart).").to_owned()
        } else {
            trf!("{} resultado(s)", achados.len())
        };
        let cabecalho = row![
            text(tr!("Ícones")).size(14),
            Space::with_width(Length::Fill),
            dica(
                button(feather_ui::icones::icone("x", 14.0)).padding(4).style(estilo_botao_pequeno(false)).on_press(ev(Evento::FecharMenu)),
                tr!("Fechar (Esc)"),
            ),
        ]
        .align_y(Alignment::Center);
        let corpo = column![
            cabecalho,
            text_input(tr!("Buscar ícone… (casa, seta, relógio, chart)"), busca)
                .id(text_input::Id::new(ID_BUSCA_ICONES))
                .on_input(|t| ev(Evento::BuscaIcones(t)))
                .size(13)
                .padding([6, 9]),
            text(resumo).size(11).color(COR_TEXTO_MUTED()),
            scrollable(grade)
                .id(scrollable::Id::new(ID_GRADE_ICONES))
                .on_scroll(|v| ev(Evento::RolarIcones(v.absolute_offset().y)))
                .height(Length::Fixed(altura_grade)),
            text(tr!("Um clique insere o ícone perto da célula ativa. Ícones Tabler (licença MIT).")).size(11).color(COR_TEXTO_MUTED()),
        ]
        .spacing(8);
        container(corpo.padding(12).width(Length::Fixed(LARGURA_MENU_ICONES))).style(estilo_popup_menu).into()
    }

    /// Inserir > WordArt: os estilos prontos comuns da suíte e, com um WordArt
    /// selecionado, a deformação (Reto, Arco, Onda, Inclinado), a intensidade e a sombra.
    fn menu_wordart(&self) -> Element<'_, Msg> {
        use feather_shapes::wordart::{Transformacao, ESTILOS};
        let selecionado = self.wordart_selecionado();
        let estilo_atual = selecionado.as_ref().and_then(|o| super::wordart::chave_estilo(o).map(str::to_owned));
        let rotulo = |t: &'static str| text(t).size(10.5).color(COR_TEXTO_MUTED());
        let estilos = row(ESTILOS.iter().enumerate().map(|(i, e)| {
            let miniatura = cv::Canvas::new(MiniWordArt(i)).width(Length::Fixed(MINI_WORDART.0)).height(Length::Fixed(MINI_WORDART.1));
            let botao = button(miniatura)
                .padding(5)
                .style(estilo_botao_pequeno(estilo_atual.as_deref() == Some(e.chave)))
                .on_press(ev(Evento::EstiloWordArt(i)));
            tooltip(botao, container(text(e.nome).size(11)).padding([4, 8]).style(estilo_dica), tooltip::Position::Bottom).into()
        }))
        .spacing(6);
        let mut corpo = column![
            row![
                text("WordArt").size(14),
                Space::with_width(Length::Fill),
                dica(
                    button(feather_ui::icones::icone("x", 14.0)).padding(4).style(estilo_botao_pequeno(false)).on_press(ev(Evento::FecharMenu)),
                    tr!("Fechar (Esc)"),
                ),
            ]
            .align_y(Alignment::Center),
            rotulo(if selecionado.is_some() { tr!("ESTILO DO WORDART SELECIONADO") } else { tr!("ESTILOS (UM CLIQUE INSERE PERTO DA CÉLULA ATIVA)") }),
            estilos,
        ]
        .spacing(8);
        if let Some(o) = &selecionado {
            let modo = Transformacao::ler(&o.forma);
            let k = super::wordart::intensidade(o);
            let deformacoes = row(Transformacao::TODOS.into_iter().map(|t| {
                let miniatura = cv::Canvas::new(MiniDeformacao(t)).width(Length::Fixed(MINI_DEFORMACAO.0)).height(Length::Fixed(MINI_DEFORMACAO.1));
                let botao = button(miniatura).padding(5).style(estilo_botao_pequeno(t == modo)).on_press(ev(Evento::DeformarWordArt(t, k)));
                tooltip(botao, container(text(t.to_string()).size(11)).padding([4, 8]).style(estilo_dica), tooltip::Position::Bottom).into()
            }))
            .spacing(6);
            let tem_sombra = o.extras.contains_key("sombra");
            let passo = |icone: &'static str, alvo: f32, texto: &'static str| -> Element<'_, Msg> {
                let mut b = button(feather_ui::icones::icone(icone, 14.0)).padding(5).style(estilo_botao_pequeno(false));
                if modo != Transformacao::Reto {
                    b = b.on_press(ev(Evento::DeformarWordArt(modo, alvo)));
                }
                dica(b, texto)
            };
            let ajustes = row![
                text(tr!("Intensidade")).size(12),
                passo("minus", k - 0.1, tr!("Menos deformação")),
                text(format!("{:.0}%", k * 100.0)).size(12).width(Length::Fixed(40.0)).align_x(iced::alignment::Horizontal::Center),
                passo("plus", k + 0.1, tr!("Mais deformação")),
                Space::with_width(Length::Fill),
                button(text(tr!("Sombra")).size(12)).padding([5, 10]).style(estilo_botao_pequeno(tem_sombra)).on_press(ev(Evento::SombraWordArt(!tem_sombra))),
            ]
            .spacing(6)
            .align_y(Alignment::Center);
            corpo = corpo.push(rotulo(tr!("DEFORMAÇÃO"))).push(deformacoes).push(ajustes);
        }
        corpo = corpo.push(
            text(if selecionado.is_some() {
                tr!("F2 ou duplo clique edita o texto. Cor e contorno das letras pela barra do objeto.")
            } else {
                tr!("Selecione um WordArt para mudar a deformação, a intensidade e a sombra.")
            })
            .size(11)
            .color(COR_TEXTO_MUTED()),
        );
        container(corpo.padding(12).width(Length::Fixed(LARGURA_MENU_WORDART))).style(estilo_popup_menu).into()
    }

    /// O menu Formas (Inserir > Ilustrações, ou Trocar forma): miniaturas por grupo.
    fn menu_formas(&self, alto: f32) -> Element<'_, Msg> {
        let modo = self.objetos.menu.unwrap_or(ModoMenu::Inserir);
        let (titulo, ajuda) = match modo {
            ModoMenu::Inserir => (tr!("Formas"), tr!("Escolha e arraste na planilha. Um clique cria no tamanho padrão.")),
            ModoMenu::Trocar | ModoMenu::Icones | ModoMenu::WordArt => (tr!("Trocar forma"), tr!("A forma selecionada muda de desenho e mantém posição, cores e texto.")),
        };
        let atual: Option<String> = match modo {
            ModoMenu::Inserir => self.objetos.inserir.map(str::to_owned),
            ModoMenu::Trocar | ModoMenu::Icones | ModoMenu::WordArt => self.objeto_selecionado().map(|o| o.forma),
        };
        let mut lista = column![].spacing(6);
        for (grupo, tipos) in geometria::catalogo() {
            lista = lista.push(text(grupo.to_uppercase()).size(10.5).color(COR_TEXTO_MUTED()));
            for fileira in tipos.chunks(POR_FILEIRA) {
                lista = lista.push(
                    row(fileira.iter().map(|t| {
                        let id = t.id();
                        let miniatura = cv::Canvas::new(Miniatura(id))
                            .width(Length::Fixed(LADO_MINIATURA))
                            .height(Length::Fixed(LADO_MINIATURA));
                        let botao = button(miniatura)
                            .padding(4)
                            .style(estilo_botao_pequeno(atual.as_deref() == Some(id)))
                            .on_press(ev(Evento::Escolher(id)));
                        tooltip(botao, container(text(t.nome()).size(11)).padding([4, 8]).style(estilo_dica), tooltip::Position::Bottom)
                            .into()
                    }))
                    .spacing(4),
                );
            }
        }
        let cabecalho = row![
            text(titulo).size(14),
            Space::with_width(Length::Fill),
            dica(
                button(feather_ui::icones::icone("x", 14.0)).padding(4).style(estilo_botao_pequeno(false)).on_press(ev(Evento::FecharMenu)),
                tr!("Fechar (Esc)"),
            ),
        ]
        .align_y(Alignment::Center);
        let corpo = column![
            cabecalho,
            scrollable(lista).height(Length::Fixed((alto - 96.0).max(120.0))),
            text(ajuda).size(11).color(COR_TEXTO_MUTED()),
        ]
        .spacing(8);
        container(corpo.padding(12).width(Length::Fixed(LARGURA_MENU))).style(estilo_popup_menu).into()
    }

    /// Itens do menu de contexto da forma (botão direito).
    pub(super) fn itens_contexto(&self, o: &ObjetoSolto) -> Vec<ItemMenu> {
        let e = |x: Evento| ev(Evento::DoContexto(Box::new(x)));
        let pode_colar = !self.objetos.area.is_empty();
        vec![
            ItemMenu::acao(tr!("Recortar"), "cut", e(Evento::Recortar)).com_atalho("Ctrl+X"),
            ItemMenu::acao(tr!("Copiar"), "copy", e(Evento::Copiar)).com_atalho("Ctrl+C"),
            ItemMenu::talvez(tr!("Colar"), "clipboard", pode_colar.then(|| e(Evento::Colar)), tr!("Copie ou recorte uma forma antes"))
                .com_atalho("Ctrl+V"),
            ItemMenu::acao(tr!("Duplicar"), "plus", e(Evento::Duplicar)).com_atalho("Ctrl+D"),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Editar texto"), "text-size", e(Evento::EditarTexto(o.id))).com_atalho("F2"),
            ItemMenu::acao(tr!("Trocar forma…"), "shapes", e(Evento::AlternarMenu(ModoMenu::Trocar))),
            // SUB_FRENTE
            ItemMenu::sub(tr!("Trazer para frente"), "arrow-up", vec![
                ItemMenu::acao(tr!("Trazer para frente"), "arrow-up", e(Evento::Ordem(Ordem::Frente))),
                ItemMenu::acao(tr!("Avançar um nível"), "chevron-up", e(Evento::Ordem(Ordem::Avancar))),
            ]),
            ItemMenu::sub(tr!("Enviar para trás"), "arrow-down", vec![
                ItemMenu::acao(tr!("Enviar para trás"), "arrow-down", e(Evento::Ordem(Ordem::Tras))),
                ItemMenu::acao(tr!("Recuar um nível"), "chevron-down", e(Evento::Ordem(Ordem::Recuar))),
            ]),
            ItemMenu::em_breve(tr!("Agrupar"), "object", tr!("Em breve: agrupar várias formas selecionadas")),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Atribuir macro…"), "code", e(Evento::AtribuirMacro)),
            ItemMenu::em_breve(tr!("Hiperlink…"), "link", tr!("Em breve: link na forma")),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Formatar forma…"), "palette", e(Evento::FormatarForma)),
            ItemMenu::acao(tr!("Tamanho e posição…"), "arrow-autofit-width", e(Evento::FormatarForma)),
            ItemMenu::Separador,
            ItemMenu::acao(tr!("Excluir"), "trash", e(Evento::Apagar)).com_atalho(tr!("Delete")),
        ]
    }

    /// Itens dos menus da aba Editar forma (Trazer para frente, Enviar para trás, Girar, Alinhar).
    fn itens_menu_faixa(m: MenuFaixa) -> Vec<ItemMenu> {
        match m {
            MenuFaixa::Frente => vec![
                ItemMenu::acao(tr!("Trazer para frente"), "arrow-up", ev(Evento::Ordem(Ordem::Frente))),
                ItemMenu::acao(tr!("Avançar um nível"), "chevron-up", ev(Evento::Ordem(Ordem::Avancar))),
            ],
            MenuFaixa::Tras => vec![
                ItemMenu::acao(tr!("Enviar para trás"), "arrow-down", ev(Evento::Ordem(Ordem::Tras))),
                ItemMenu::acao(tr!("Recuar um nível"), "chevron-down", ev(Evento::Ordem(Ordem::Recuar))),
            ],
            MenuFaixa::Girar => vec![
                ItemMenu::acao(tr!("Girar 90° à direita"), "arrow-forward-up", ev(Evento::Girar(Giro::Direita90))),
                ItemMenu::acao(tr!("Girar 90° à esquerda"), "arrow-back-up", ev(Evento::Girar(Giro::Esquerda90))),
                ItemMenu::acao(tr!("Espelhar horizontalmente"), "arrows-horizontal", ev(Evento::Girar(Giro::EspelharH))),
                ItemMenu::acao(tr!("Espelhar verticalmente"), "arrows-vertical", ev(Evento::Girar(Giro::EspelharV))),
            ],
            MenuFaixa::Alinhar => vec![
                ItemMenu::acao(tr!("Encostar no canto da célula"), "layout-grid", ev(Evento::Alinhar(Alinhar::CantoDaCelula))),
                ItemMenu::acao(tr!("Ajustar às bordas das células"), "arrows-maximize", ev(Evento::Alinhar(Alinhar::AjustarAsCelulas))),
            ],
        }
    }

    /// Diálogo Atribuir macro (lista das macros, com "Nenhuma").
    fn dialogo_macro(&self) -> Option<Element<'_, Msg>> {
        let (_, escolhida) = self.objetos.macro_dialogo.as_ref()?;
        let item = |nome: Option<String>, rotulo: String| -> Element<'_, Msg> {
            let sel = escolhida == &nome;
            button(
                row![feather_ui::icones::icone(if sel { "circle-check" } else { "circle" }, 15.0), text(rotulo).size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([6, 10])
            .style(estilo_item_menu(sel, true))
            .on_press(ev(Evento::MacroEscolher(nome)))
            .into()
        };
        let mut lista = column![item(None, tr!("Nenhuma").into())].spacing(2);
        for m in &self.macro_catalogo.macros {
            lista = lista.push(item(Some(m.name.clone()), m.name.clone()));
        }
        if self.macro_catalogo.macros.is_empty() {
            lista = lista.push(text(tr!("Ainda não há macros. Crie uma no editor de macros (Alt+F11).")).size(12).color(COR_TEXTO_MUTED()));
        }
        let conteudo = column![
            text(tr!("Atribuir macro")).size(19),
            text(tr!("Com macro, o clique na forma a executa; Ctrl+clique ou botão direito seleciona a forma."))
                .size(12)
                .color(COR_TEXTO_MUTED()),
            scrollable(lista).height(Length::Fixed(240.0)),
            row![
                Space::with_width(Length::Fill),
                button(text(tr!("Cancelar")).size(13)).style(estilo_botao_secundario).on_press(ev(Evento::MacroCancelar)),
                button(text("OK").size(13)).style(estilo_botao_primario).on_press(ev(Evento::MacroConfirmar)),
            ]
            .spacing(10),
        ]
        .spacing(10)
        .padding(17);
        Some(container(conteudo).width(Length::Fixed(420.0)).style(estilo_painel_faixa).into())
    }

    /// Tudo que flutua na janela: menu Formas, painel de cores, menus da aba
    /// Editar forma, menu de contexto da forma e o diálogo Atribuir macro.
    pub(in crate::app) fn sobrepor_menu_formas<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let janela = self.tamanho_principal;
        let fora = |msg: Msg| mouse_area(Space::new(Length::Fill, Length::Fill)).on_press(msg);
        let em = |x: f32, y: f32, el: Element<'a, Msg>| -> Element<'a, Msg> {
            column![Space::with_height(y), row![Space::with_width(x), opaque(el)]].into()
        };
        let mut base = base;
        if self.objetos.menu.is_some() {
            let (x, y) = self.objetos.menu_pos;
            let alto = (janela.height - 16.0).min(480.0);
            let (largura, alto) = match self.objetos.menu {
                Some(ModoMenu::Icones) => (LARGURA_MENU_ICONES, alto),
                Some(ModoMenu::WordArt) => (LARGURA_MENU_WORDART, alto.min(330.0)),
                _ => (LARGURA_MENU, alto),
            };
            let x = x.min(janela.width - largura - 8.0).max(8.0);
            let y = y.min(janela.height - alto - 8.0).max(8.0);
            let menu = match self.objetos.menu {
                Some(ModoMenu::Icones) => self.menu_icones(alto),
                Some(ModoMenu::WordArt) => self.menu_wordart(),
                _ => self.menu_formas(alto),
            };
            base = stack![base, fora(ev(Evento::FecharMenu)), em(x, y, menu)]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
        if let (Some(p), Some(o)) = (self.objetos.painel, self.objeto_selecionado()) {
            let (x, y) = self.objetos.painel_pos;
            let alto = if p == Painel::Contorno { 430.0 } else { 220.0 };
            // "Mais cores…" abre o seletor (264 px) à direita do painel.
            let largura = LARGURA_PAINEL + if self.objetos.mais_cores { 270.0 } else { 0.0 };
            let x = x.min(janela.width - largura - 8.0).max(8.0);
            let y = y.min(janela.height - alto - 8.0).max(8.0);
            base = stack![base, fora(ev(Evento::FecharPainel)), em(x, y, self.painel_objeto(p, &o))]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
        if let Some((m, x, y)) = self.objetos.menu_faixa {
            let itens = Self::itens_menu_faixa(m);
            base = menu_contexto::sobrepor(base, &itens, None, |_| ev(Evento::ContextoSub(None)), ev(Evento::FecharMenuFaixa), (x, y), janela);
        }
        if let (Some((x, y)), Some(o)) = (self.objetos.contexto, self.objeto_selecionado()) {
            let itens = self.itens_contexto(&o);
            base = menu_contexto::sobrepor(
                base,
                &itens,
                self.objetos.contexto_sub,
                |s| ev(Evento::ContextoSub(s)),
                ev(Evento::FecharContexto),
                (x, y),
                janela,
            );
        }
        if let Some((id, x, y)) = self.graficos_contexto() {
            let g = |e: crate::app::graficos::Evento| Msg::Grafico(e);
            use crate::app::graficos::Evento as G;
            let mut itens = vec![
                ItemMenu::acao(tr!("Editar gráfico…"), "chart-bar", g(G::Editar(id.clone()))),
                ItemMenu::Separador,
                ItemMenu::acao(tr!("Copiar"), "copy", g(G::Copiar(id.clone()))).com_atalho("Ctrl+C"),
            ];
            if self.grafico_copiado() {
                itens.push(ItemMenu::acao(tr!("Colar"), "clipboard", g(G::Colar)).com_atalho("Ctrl+V"));
            }
            itens.extend([
                ItemMenu::acao(tr!("Duplicar"), "copy-plus", g(G::Duplicar(id.clone()))).com_atalho("Ctrl+D"),
                ItemMenu::Separador,
                ItemMenu::acao(tr!("Excluir"), "trash", g(G::Remover(id))).com_atalho(tr!("Delete")),
            ]);
            base = menu_contexto::sobrepor(base, &itens, None, |_| g(G::PassouNoMenu), g(G::FecharContexto), (x, y), janela);
        }
        if let Some(dialogo) = self.dialogo_macro() {
            base = stack![
                base,
                fora(ev(Evento::MacroCancelar)),
                container(opaque(dialogo)).center_x(Length::Fill).center_y(Length::Fill),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        }
        base
    }

    /// A aba contextual "Editar forma" na fileira de abas (destacada).
    pub(in crate::app) fn aba_editar_forma(&self) -> Element<'_, Msg> {
        let ativa = self.faixa_ativa == AbaFaixa::EditarForma;
        let titulo = text(tr!("Editar forma")).size(13).wrapping(text::Wrapping::None).font(iced::Font {
            weight: if ativa { iced::font::Weight::Semibold } else { iced::font::Weight::Normal },
            ..iced::Font::DEFAULT
        });
        button(titulo)
            .padding([7.0, 12.0])
            .style(move |_: &Theme, status| button::Style {
                background: Some(iced::Background::Color(COR_AZUL_FUNDO())),
                text_color: COR_ACENTO_TEXTO(COR_AZUL_FUNDO()),
                border: iced::Border {
                    color: if ativa || matches!(status, button::Status::Hovered) { COR_AZUL_ACENTO() } else { Color::TRANSPARENT },
                    width: if ativa { 2.0 } else { 1.0 },
                    radius: 3.0.into(),
                },
                ..Default::default()
            })
            .on_press(Msg::AbaFaixa(AbaFaixa::EditarForma))
            .into()
    }

    /// Larguras estimadas dos grupos da aba Editar forma (mesma contagem de
    /// [`Self::grupos_editar_forma`]).
    pub(in crate::app) fn larguras_editar_forma(&self) -> &'static [f32] {
        if self.tem_forma_selecionada() { &[230., 200., 290., 300., 240., 280., 120.] } else { &[300.] }
    }

    /// Os grupos da aba contextual "Editar forma".
    pub(in crate::app) fn grupos_editar_forma(&self) -> Vec<Element<'_, Msg>> {
        let Some(o) = self.objeto_selecionado() else {
            return vec![self.faixa_grupo(
                tr!("Editar forma"),
                container(text(tr!("Selecione uma forma na planilha para editá-la.")).size(12).color(COR_TEXTO_MUTED()))
                    .padding([0, 8])
                    .into(),
            )];
        };
        let aberto = |m: MenuFaixa| self.objetos.menu_faixa.is_some_and(|(a, ..)| a == m);
        let linha = o.tipo == modelo::TipoObjeto::Forma && (geometria::so_traco(&o.forma) || modelo::e_linha(&o.forma));
        let cantos = o.tipo == modelo::TipoObjeto::Forma && geometria::raios(&o.forma, &o.ajustes).is_some();
        let c = &self.objetos.campos;
        let rotulo = |t: &'static str| text(t).size(11).width(Length::Fixed(48.0));
        let px = || text("px").size(11).color(COR_TEXTO_MUTED());

        let preenchimento = if linha {
            dica(self.faixa_grande_seta(tr!("Preenchimento"), "bucket", None, false), tr!("Linhas não têm preenchimento"))
        } else {
            self.faixa_grande_seta(tr!("Preenchimento"), "bucket", Some(ev(Evento::AbrirPainel(Painel::Preenchimento))),
                self.objetos.painel == Some(Painel::Preenchimento))
        };
        let raio: Element<'_, Msg> = if c.por_canto {
            let canto = |i: usize| campo_num(&c.cantos[i], 46.0, cantos, move |t| ev(Evento::CampoCanto(i, t)), ev(Evento::AplicarRaio));
            dica(
                column![row![canto(0), canto(1)].spacing(4), row![canto(3), canto(2)].spacing(4)].spacing(3),
                tr!("Raio de cada canto em px (como na forma: em cima à esquerda e à direita, embaixo à esquerda e à direita)"),
            )
        } else {
            row![rotulo(tr!("Raio")), campo_num(&c.raio, 56.0, cantos, |t| ev(Evento::CampoRaio(t)), ev(Evento::AplicarRaio)), px()]
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
        };
        let grupo_cantos = column![
            raio,
            row![
                self.faixa_pequeno(tr!("Por canto"), "layout-grid", cantos.then(|| ev(Evento::PorCanto(!c.por_canto))), c.por_canto),
                self.faixa_pequeno(tr!("Aplicar"), "check", cantos.then(|| ev(Evento::AplicarRaio)), false),
            ]
            .spacing(3),
        ]
        .spacing(4);
        let grupo_cantos: Element<'_, Msg> =
            if cantos { grupo_cantos.into() } else { dica(grupo_cantos, tr!("Só retângulos têm cantos arredondáveis")) };

        vec![
            self.faixa_grupo(tr!("Inserir formas"), self.faixa_linha(vec![
                self.faixa_grande_seta(tr!("Formas"), "shapes", Some(ev(Evento::AlternarMenu(ModoMenu::Inserir))),
                    self.objetos.menu == Some(ModoMenu::Inserir)),
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Trocar forma"), "shapes", Some(ev(Evento::AlternarMenu(ModoMenu::Trocar))),
                        self.objetos.menu == Some(ModoMenu::Trocar)),
                    self.faixa_pequeno(tr!("Editar texto"), "text-size", Some(ev(Evento::EditarTexto(o.id))), self.objetos.texto.is_some()),
                ]),
            ])),
            self.faixa_grupo(tr!("Texto"), {
                let (h, v) = (o.alinhar.h, o.alinhar.v);
                let sem_texto = linha || o.tipo == modelo::TipoObjeto::Icone;
                let ativo = |m: Msg| (!sem_texto).then_some(m);
                let grupo = self.faixa_linha(vec![
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Topo"), "layout-align-top", ativo(ev(Evento::AlinharTextoV(modelo::AlinharV::Topo))), v == modelo::AlinharV::Topo),
                        self.faixa_pequeno(tr!("Meio"), "layout-align-middle", ativo(ev(Evento::AlinharTextoV(modelo::AlinharV::Meio))), v == modelo::AlinharV::Meio),
                        self.faixa_pequeno(tr!("Baixo"), "layout-align-bottom", ativo(ev(Evento::AlinharTextoV(modelo::AlinharV::Baixo))), v == modelo::AlinharV::Baixo),
                    ]),
                    self.faixa_pilha(vec![
                        self.faixa_pequeno(tr!("Esquerda"), "align-left", ativo(ev(Evento::AlinharTextoH(modelo::AlinharH::Esquerda))), h == modelo::AlinharH::Esquerda),
                        self.faixa_pequeno(tr!("Centro"), "align-center", ativo(ev(Evento::AlinharTextoH(modelo::AlinharH::Centro))), h == modelo::AlinharH::Centro),
                        self.faixa_pequeno(tr!("Direita"), "align-right", ativo(ev(Evento::AlinharTextoH(modelo::AlinharH::Direita))), h == modelo::AlinharH::Direita),
                    ]),
                ]);
                if sem_texto { dica(grupo, tr!("Linhas e ícones não têm texto")) } else { grupo }
            }),
            self.faixa_grupo(tr!("Estilos de forma"), self.faixa_linha(vec![
                preenchimento,
                self.faixa_grande_seta(tr!("Contorno"), "pencil", Some(ev(Evento::AbrirPainel(Painel::Contorno))),
                    self.objetos.painel == Some(Painel::Contorno)),
                dica(self.faixa_grande(tr!("Efeitos"), "wand", Some(ev(Evento::SombraForma)), o.sombra.is_some()), tr!("Liga ou desliga a sombra da forma")),
            ])),
            self.faixa_grupo(tr!("Organizar"), self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Trazer para frente"), "arrow-up", Some(ev(Evento::MenuFaixa(MenuFaixa::Frente))), aberto(MenuFaixa::Frente)),
                    self.faixa_pequeno(tr!("Enviar para trás"), "arrow-down", Some(ev(Evento::MenuFaixa(MenuFaixa::Tras))), aberto(MenuFaixa::Tras)),
                    dica(self.faixa_pequeno(tr!("Agrupar"), "object", None, false), tr!("Em breve: agrupar várias formas selecionadas")),
                ]),
                self.faixa_pilha(vec![
                    self.faixa_pequeno(tr!("Alinhar"), "layout-grid", Some(ev(Evento::MenuFaixa(MenuFaixa::Alinhar))), aberto(MenuFaixa::Alinhar)),
                    self.faixa_pequeno(tr!("Girar"), "refresh", Some(ev(Evento::MenuFaixa(MenuFaixa::Girar))), aberto(MenuFaixa::Girar)),
                ]),
            ])),
            self.faixa_grupo(tr!("Arredondar cantos"), grupo_cantos),
            self.faixa_grupo(tr!("Tamanho"), column![
                row![rotulo(tr!("Largura")), campo_num(&c.largura, 64.0, true, |t| ev(Evento::CampoLargura(t)), ev(Evento::AplicarTamanho)), px()]
                    .spacing(4).align_y(Alignment::Center),
                row![rotulo(tr!("Altura")), campo_num(&c.altura, 64.0, true, |t| ev(Evento::CampoAltura(t)), ev(Evento::AplicarTamanho)), px()]
                    .spacing(4).align_y(Alignment::Center),
                row![
                    self.faixa_pequeno(tr!("Travar proporção"), "lock", Some(ev(Evento::TravarProporcao(!self.objetos.travar))), self.objetos.travar),
                    self.faixa_pequeno(tr!("Aplicar"), "check", Some(ev(Evento::AplicarTamanho)), false),
                ].spacing(3),
            ].spacing(4).into()),
            self.faixa_grupo(tr!("Macro"), self.faixa_grande(tr!("Atribuir macro"), "code", Some(ev(Evento::AtribuirMacro)),
                self.objetos.macro_dialogo.is_some() || o.macro_nome.is_some())),
        ]
    }
}
