//! Menu ancorado à Ribbon e janela própria de regras, sem cobrir a grade.
use super::*;
use pelicano::column; use pelicano::row;
use feather_ui::seletor_cor;

/// Os campos de cor da regra (#2: amostra que abre o seletor, sem o hexa à mostra).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorRegra { Min, Meio, Principal, Negativa, Texto }

impl CorRegra {
    fn valor(self, app: &App) -> &str {
        match self {
            Self::Min => &app.condicional_cor_min,
            Self::Meio => &app.condicional_cor_meio,
            Self::Principal => &app.condicional_cor,
            Self::Negativa => &app.condicional_cor_negativa,
            Self::Texto => &app.condicional_cor_texto,
        }
    }
    fn msg(self, hex: String) -> Msg {
        match self {
            Self::Min => Msg::CondicionalCorMin(hex),
            Self::Meio => Msg::CondicionalCorMeio(hex),
            Self::Principal => Msg::CondicionalCor(hex),
            Self::Negativa => Msg::CondicionalCorNegativa(hex),
            Self::Texto => Msg::CondicionalCorTexto(hex),
        }
    }
}

/// Um evento do seletor de cor do `feather-ui` aplicado ao estado (HSVA e o
/// texto do hexa); devolve a cor nova quando ela muda.
// shortcut: gráfico, Formatar células, formas e faixa têm cópias desta conta; juntar quando mexer nelas.
pub(super) fn aplicar_evento_cor(evento: seletor_cor::Evento, hsva: &mut [f32; 4], hex: &mut String) -> Option<String> {
    use seletor_cor::Evento as E;
    let do_hsva = |h: &[f32; 4]| {
        let [r, g, b] = seletor_cor::hsv_para_rgb(h[0], h[1], h[2]);
        seletor_cor::hex_de([r, g, b, (h[3] * 255.).round() as u8])
    };
    let nova = match evento {
        E::Fechar => return None,
        E::Escolhida(cor) => cor,
        E::SaturacaoBrilho(s, v) => { hsva[1] = s.clamp(0., 1.); hsva[2] = v.clamp(0., 1.); do_hsva(hsva) }
        E::Matiz(h) => { hsva[0] = h.rem_euclid(360.); do_hsva(hsva) }
        E::Alfa(a) => { hsva[3] = a.clamp(0., 1.); do_hsva(hsva) }
        E::TextoHex(valor) => {
            *hex = valor.clone();
            if !seletor_cor::hex_valido(&valor) { return None; }
            format!("#{}", valor.trim().trim_start_matches('#').to_lowercase())
        }
        E::Canal(i, valor) => {
            let n = valor.trim().parse::<u8>().ok().filter(|_| i < 4)?;
            let mut rgba = seletor_cor::rgba_de(&do_hsva(hsva));
            rgba[i] = n;
            seletor_cor::hex_de(rgba)
        }
    };
    if !seletor_cor::hex_valido(&nova) { return None; }
    *hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&nova), hsva[0]);
    *hex = nova.clone();
    Some(nova)
}

impl App {
    /// Clique na amostra: abre (ou fecha) o seletor daquele campo.
    pub(super) fn abrir_cor_condicional(&mut self, campo: CorRegra) {
        if self.condicional_cor_aberta == Some(campo) {
            self.condicional_cor_aberta = None;
            return;
        }
        let atual = campo.valor(self).to_owned();
        self.condicional_hex = atual.clone();
        if seletor_cor::hex_valido(&atual) {
            self.condicional_hsva = seletor_cor::rgba_para_hsva(seletor_cor::rgba_de(&atual), self.condicional_hsva[0]);
        }
        self.condicional_cor_aberta = Some(campo);
    }

    pub(super) fn evento_cor_condicional(&mut self, evento: seletor_cor::Evento) -> Task<Msg> {
        let Some(campo) = self.condicional_cor_aberta else { return Task::none() };
        if matches!(evento, seletor_cor::Evento::Fechar) {
            self.condicional_cor_aberta = None;
            return Task::none();
        }
        match aplicar_evento_cor(evento, &mut self.condicional_hsva, &mut self.condicional_hex) {
            Some(hex) => self.update(campo.msg(hex)),
            None => Task::none(),
        }
    }
}

const LARGURA_MENU: f32 = 355.0;
const LARGURA_MINIMA_MENU: f32 = 250.0;
const MARGEM_MENU: f32 = 8.0;

impl App {
    /// Mantém a borda esquerda ancorada ao botão da Ribbon. Reduz a largura
    /// antes de deslocar o popup, sobretudo quando a janela está estreita.
    /// A altura se adapta ao espaço restante e o conteúdo usa scroll.
    fn geometria_menu_condicional(&self) -> (f32, f32, f32, f32) {
        let largura_janela = self.tamanho_principal.width.max(MARGEM_MENU * 2.0 + 1.0);
        let altura_janela = self.tamanho_principal.height.max(MARGEM_MENU * 2.0 + 1.0);
        let minimo = LARGURA_MINIMA_MENU.min(largura_janela - 2.0 * MARGEM_MENU);
        let x = self.condicional_menu_x.clamp(
            MARGEM_MENU,
            (largura_janela - minimo - MARGEM_MENU).max(MARGEM_MENU),
        );
        let largura = (largura_janela - x - MARGEM_MENU).min(LARGURA_MENU).max(1.0);
        let y = self.condicional_menu_y.clamp(
            MARGEM_MENU,
            (altura_janela - MARGEM_MENU - 120.0).max(MARGEM_MENU),
        );
        let altura = (altura_janela - y - MARGEM_MENU).min(600.0).max(1.0);
        (x, y, largura, altura)
    }

    pub(super) fn view_condicional(&self) -> Element<'_, Msg> {
        use feathersheet_core::operacoes::rules::Condition;
        use pelicano::pick_list; use pelicano::tooltip;
        let regras=condicional::regras(&self.workbook,self.folha);
        let negrito = pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT };

        // ---- coluna da esquerda: as regras da aba, com prévia e ações
        let mut lista=column![].spacing(6);
        if regras.is_empty() {
            lista=lista.push(container(column![
                feather_ui::icones::icone("palette", 28.0),
                text(tr!("Nenhuma regra nesta aba")).size(12.5),
                text(tr!("Selecione células e crie a primeira regra ao lado."))
                    .size(11).color(COR_TEXTO_MUTED()),
            ].spacing(6).align(pelicano::Align::Center)).width(Length::FILL).align_x(pelicano::Align::Center).padding([28, 12]));
        }
        let total = regras.len();
        let selecao = self.area_selecionada();
        let toca_selecao = |r: &feathersheet_core::operacoes::rules::ConditionalRule| r.ranges.iter()
            .any(|a| a.r0 <= selecao.r1 && selecao.r0 <= a.r1 && a.c0 <= selecao.c1 && selecao.c0 <= a.c1);
        let mut mostradas = 0;
        for (i,regra) in regras.iter().enumerate() {
            // O índice continua o da aba: mover, editar e excluir agem na regra certa.
            if self.condicional_so_selecao && !toca_selecao(regra) { continue; }
            mostradas += 1;
            let faixas = regra.ranges.iter().map(|a| endereco_area(&Rect::new(a.r0, a.c0, a.r1, a.c1))).collect::<Vec<_>>().join(", ");
            let (descricao, amostra): (String, Element<'_, Msg>) = match &regra.condition {
                Condition::Cell(c) => (
                    descrever_regra(c.op, &c.values),
                    amostra_estilo(c.style.bg.as_deref(), c.style.color.as_deref(),
                        c.style.bold.unwrap_or(false), c.style.italic.unwrap_or(false)),
                ),
                Condition::ColorScale(e) => (tr!("Escala de cores").into(), amostra_escala(
                    &[Some(e.min.color.as_str()), e.mid.as_ref().map(|m| m.color.as_str()), Some(e.max.color.as_str())])),
                Condition::DataBar(b) => (tr!("Barras de dados").into(), amostra_barra(&b.color)),
            };
            let editando = self.condicional_editando == Some(i);
            let acao = |icone: &'static str, dica: &'static str, msg: Option<Msg>| -> Element<'_, Msg> {
                tooltip(
                    button(container(feather_ui::icones::icone(icone, 15.0))
                            .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::FILL).align_y(pelicano::Align::Center))
                        .width(Length::Fixed(26.0)).height(Length::Fixed(26.0)).padding(0)
                        .style(|tema, estado| {
                            let mut e = estilo_botao_pequeno(false)(tema, estado);
                            e.border.radius = [6.0; 4];
                            e
                        })
                        .on_press_maybe(msg),
                    dica,
                    pelicano::TooltipPosition::Top,
                ).padding([4, 8]).style(estilo_dica).into()
            };
            let cartao = column![
                row![
                    amostra,
                    column![
                        text(descricao).size(12.5),
                        text(faixas).size(11).color(COR_TEXTO_MUTED()),
                    ].spacing(2).width(Length::FILL),
                ].spacing(10).align(pelicano::Align::Center),
                row![
                    text(format!("{}º", i + 1)).size(10.5).color(COR_TEXTO_MUTED()),
                    Space::new(Length::FILL, 0.0),
                    acao("chevron-up", tr!("Aplicar antes"), (i > 0).then_some(Msg::CondicionalMover(i,-1))),
                    acao("chevron-down", tr!("Aplicar depois"), (i + 1 < total).then_some(Msg::CondicionalMover(i,1))),
                    acao("edit", tr!("Editar regra"), Some(Msg::CondicionalEditar(i))),
                    acao("trash", tr!("Excluir regra"), Some(Msg::CondicionalRemover(i))),
                ].spacing(2).align(pelicano::Align::Center),
            ].spacing(6);
            lista=lista.push(container(cartao).padding([9, 10]).width(Length::FILL)
                .style(move |_: &pelicano::Theme| pelicano::Style {
                    background: Some(pelicano::Background::Color(
                        if editando { COR_AZUL_FUNDO() } else { COR_PAINEL() })),
                    border: pelicano::Border {
                        color: if editando { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() },
                        width: 1.0, radius: [9.0; 4],
                    },
                    ..Default::default()
                }));
        }
        if self.condicional_so_selecao && mostradas == 0 && total > 0 {
            lista = lista.push(text(tr!("Nenhuma regra toca a seleção atual.")).size(11.5).color(COR_TEXTO_MUTED()));
        }
        let filtro = row![
            text(tr!("Mostrar:")).size(11.5).color(COR_TEXTO_MUTED()),
            button(text(tr!("Esta planilha")).size(11.5)).padding([4, 9])
                .style(estilo_botao_pequeno(!self.condicional_so_selecao)).on_press(Msg::CondicionalSoSelecao(false)),
            button(text(tr!("Seleção atual")).size(11.5)).padding([4, 9])
                .style(estilo_botao_pequeno(self.condicional_so_selecao)).on_press(Msg::CondicionalSoSelecao(true)),
        ].spacing(6).align(pelicano::Align::Center);
        let esquerda = column![
            filtro,
            row![
                text(trf!("REGRAS ({total})")).size(10.5).color(COR_TEXTO_MUTED()),
                Space::new(Length::FILL, 0.0),
                button(row![feather_ui::icones::icone("plus", 14.0), text(tr!("Nova")).size(12)]
                        .spacing(5).align(pelicano::Align::Center))
                    .padding([5, 10]).style(estilo_botao_secundario)
                    .on_press(Msg::CondicionalNovaRegra),
            ].align(pelicano::Align::Center),
            feather_ui::tema::rolagem(lista).height(Length::FILL),
            row![
                button(row![feather_ui::icones::icone("eraser", 14.0), text(tr!("Limpar seleção")).size(12)]
                        .spacing(6).align(pelicano::Align::Center))
                    .padding([6, 10]).style(estilo_botao_secundario)
                    .on_press(Msg::CondicionalLimparSelecao),
                button(row![feather_ui::icones::icone("trash", 14.0), text(tr!("Limpar todas")).size(12)]
                        .spacing(6).align(pelicano::Align::Center))
                    .padding([6, 10]).style(estilo_botao_secundario)
                    .on_press_maybe((total > 0).then_some(Msg::CondicionalLimparTudo)),
            ].spacing(6),
        ].spacing(10).padding(14).width(Length::Fixed(330.0)).height(Length::FILL);

        // ---- coluna da direita: o formulário da regra
        let campo = |rotulo: &str, controle: Element<'static, Msg>| -> Element<'static, Msg> {
            column![text(rotulo.to_owned()).size(11.5).color(COR_TEXTO_MUTED()), controle]
                .spacing(5).into()
        };
        let entrada = |dica: &str, valor: &str, msg: fn(String) -> Msg| -> Element<'static, Msg> {
            text_input(dica.to_owned(), valor.to_owned()).on_input(msg).padding([7, 9]).size(12.5)
                .width(Length::FILL).into()
        };
        // Amostra + nome da cor; o clique abre o seletor (o hexa fica lá dentro).
        let aberta = self.condicional_cor_aberta;
        let cor_campo = |rotulo: &str, vazio: &str, valor: &str, alvo: CorRegra| -> Element<'static, Msg> {
            let legenda = if valor.trim().is_empty() { vazio.to_owned() } else { tr!("Alterar…").to_owned() };
            campo(rotulo, button(row![
                    amostra_cor(valor),
                    text(legenda).size(12.5),
                    Space::new(Length::FILL, 0.0),
                    feather_ui::icones::icone(if aberta == Some(alvo) { "chevron-up" } else { "chevron-down" }, 13.0),
                ].spacing(8).align(pelicano::Align::Center))
                .padding([3, 6]).width(Length::FILL)
                .style(estilo_botao_pequeno(aberta == Some(alvo)))
                .on_press(Msg::CondicionalCorAbrir(alvo)).into())
        };
        let tipo = self.condicional_operador;
        let mut formulario = column![
            text(if self.condicional_editando.is_some() { tr!("Editar regra") } else { tr!("Nova regra") })
                .size(15).font(negrito),
            campo(tr!("Tipo da regra"), pick_list(&TipoCondicional::TODOS[..], Some(tipo),
                Msg::CondicionalTipo).padding([7, 10]).size(12.5)
                .style(crate::compat::estilo_menu_da_faixa).handle_color(COR_AZUL_ACENTO()).width(Length::FILL).into()),
            campo(tr!("Aplica-se a"), row![
                text_input("A1:B10".to_owned(), self.condicional_intervalo.clone())
                    .on_input(Msg::CondicionalIntervalo).padding([7, 9]).size(12.5).width(Length::FILL),
                tooltip(
                    button(row![feather_ui::icones::icone("pointer", 14.0),
                            text(if self.condicional_pegando { tr!("Pronto") } else { tr!("Selecionar") }).size(12)]
                            .spacing(5).align(pelicano::Align::Center))
                        .padding([6, 10]).style(estilo_botao_pequeno(self.condicional_pegando))
                        .on_press(Msg::CondicionalPegar),
                    tr!("Selecione a área na planilha; o endereço vem para cá"),
                    pelicano::TooltipPosition::Top,
                ).padding([4, 8]).style(estilo_dica),
            ].spacing(6).align(pelicano::Align::Center).into()),
        ].spacing(12);
        match tipo.aridade() {
            1 => formulario = formulario.push(campo(tr!("Valor, texto ou fórmula"),
                entrada(tr!("Ex.: 10, Aprovado ou =A1>B1"), &self.condicional_valor, Msg::CondicionalValor))),
            2 => formulario = formulario.push(row![
                container(campo(tr!("De"), entrada("10", &self.condicional_valor, Msg::CondicionalValor))).width(Length::FILL),
                container(campo(tr!("Até"), entrada("20", &self.condicional_valor2, Msg::CondicionalValor2))).width(Length::FILL),
            ].spacing(10)),
            _ => {}
        }
        formulario = formulario.push(text(tr!("Formatação")).size(13).font(negrito));
        let previa: Element<'_, Msg> = match tipo {
            TipoCondicional::Escala => {
                formulario = formulario
                    .push(row![
                        container(cor_campo(tr!("Cor do menor valor"), "", &self.condicional_cor_min, CorRegra::Min)).width(Length::FILL),
                        container(cor_campo(tr!("Cor do maior valor"), "", &self.condicional_cor, CorRegra::Principal)).width(Length::FILL),
                    ].spacing(10))
                    .push(row![
                        toggler("", self.condicional_ponto_meio).on_toggle(Msg::CondicionalPontoMeio).size(18),
                        text(tr!("Usar cor intermediária")).size(12),
                    ].spacing(8).align(pelicano::Align::Center));
                if self.condicional_ponto_meio {
                    formulario = formulario.push(cor_campo(tr!("Cor intermediária"), "",
                        &self.condicional_cor_meio, CorRegra::Meio));
                }
                amostra_escala(&[Some(self.condicional_cor_min.as_str()),
                    self.condicional_ponto_meio.then_some(self.condicional_cor_meio.as_str()),
                    Some(self.condicional_cor.as_str())])
            }
            TipoCondicional::Barra => {
                formulario = formulario
                    .push(row![
                        container(cor_campo(tr!("Cor da barra"), "", &self.condicional_cor, CorRegra::Principal)).width(Length::FILL),
                        container(cor_campo(tr!("Barras negativas"), tr!("Como a barra"), &self.condicional_cor_negativa, CorRegra::Negativa)).width(Length::FILL),
                    ].spacing(10))
                    .push(row![
                        toggler("", self.condicional_ocultar_valor).on_toggle(Msg::CondicionalOcultarValor).size(18),
                        text(tr!("Mostrar só a barra, sem o número")).size(12),
                    ].spacing(8).align(pelicano::Align::Center));
                amostra_barra(&self.condicional_cor)
            }
            _ => {
                let efeito = |icone: &'static str, dica: &'static str, ligado: bool, msg: fn(bool) -> Msg| -> Element<'static, Msg> {
                    tooltip(
                        button(container(feather_ui::icones::icone(icone, 16.0))
                                .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::FILL).align_y(pelicano::Align::Center))
                            .width(Length::Fixed(34.0)).height(Length::Fixed(32.0)).padding(0)
                            .style(move |tema, estado| {
                                let mut e = estilo_botao_pequeno(ligado)(tema, estado);
                                e.border.radius = [7.0; 4];
                                e.border.width = 1.0;
                                e.border.color = if ligado { COR_AZUL_ACENTO() } else { COR_BORDA() };
                                e
                            })
                            .on_press(msg(!ligado)),
                        dica,
                        pelicano::TooltipPosition::Top,
                    ).padding([4, 8]).style(estilo_dica).into()
                };
                formulario = formulario
                    .push(row![
                        container(cor_campo(tr!("Cor de fundo"), tr!("Sem preenchimento"), &self.condicional_cor, CorRegra::Principal)).width(Length::FILL),
                        container(cor_campo(tr!("Cor do texto"), tr!("Automática"), &self.condicional_cor_texto, CorRegra::Texto)).width(Length::FILL),
                    ].spacing(10))
                    .push(row![
                        efeito("bold", tr!("Negrito"), self.condicional_negrito, Msg::CondicionalNegrito),
                        efeito("italic", tr!("Itálico"), self.condicional_italico, Msg::CondicionalItalico),
                        efeito("underline", tr!("Sublinhado"), self.condicional_sublinhado, Msg::CondicionalSublinhado),
                        efeito("strikethrough", tr!("Tachado"), self.condicional_riscado, Msg::CondicionalRiscado),
                    ].spacing(6));
                let fundo = (!self.condicional_cor.trim().is_empty()).then_some(self.condicional_cor.as_str());
                let texto = (!self.condicional_cor_texto.trim().is_empty()).then_some(self.condicional_cor_texto.as_str());
                amostra_estilo(fundo, texto, self.condicional_negrito, self.condicional_italico)
            }
        };
        if let Some(alvo) = aberta {
            let seletor = seletor_cor::painel_embutido(tr!("Cor"), alvo.valor(self), self.condicional_hsva, &self.condicional_hex)
                .map(Msg::CondicionalSeletorCor);
            formulario = formulario.push(seletor);
            if alvo == CorRegra::Texto {
                formulario = formulario.push(button(text(tr!("Cor automática")).size(12)).padding([5, 10])
                    .style(estilo_botao_pequeno(false)).on_press(Msg::CondicionalCorTexto(String::new())));
            }
        }
        formulario = formulario.push(campo(tr!("Prévia"), container(previa)
            .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::Fixed(64.0)).align_y(pelicano::Align::Center)
            .style(|_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
                ..Default::default()
            }).into()));
        let mut botoes = row![Space::new(Length::FILL, 0.0)].spacing(8);
        if self.condicional_editando.is_some() {
            botoes = botoes.push(button(text(tr!("Cancelar edição")).size(12.5)).padding([8, 14])
                .style(estilo_botao_secundario).on_press(Msg::CondicionalNovaRegra));
        }
        botoes = botoes.push(button(text(if self.condicional_editando.is_some() {
                tr!("Salvar regra") } else { tr!("Criar regra") }).size(12.5))
            .padding([8, 18]).style(estilo_botao_primario).on_press(Msg::CondicionalAdicionar));
        let direita = column![
            feather_ui::tema::rolagem(formulario.padding([2, 14])).height(Length::FILL),
            botoes,
        ].spacing(12).padding([14, 18]).width(Length::FILL).height(Length::FILL);

        // ---- moldura: cabeçalho, duas colunas
        let nome_aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
        let cabecalho = row![
            column![
                text(tr!("Formatação condicional")).size(18).font(negrito),
                text(trf!("Regras da aba “{nome_aba}”, aplicadas de cima para baixo."))
                    .size(12).color(COR_TEXTO_MUTED()),
            ].spacing(3).width(Length::FILL),
        ].align(pelicano::Align::Center).padding([14, 18]);
        let divisor_h = container(Space::new(0.0, Length::Fixed(1.0))).width(Length::FILL).style(estilo_linha);
        let divisor_v = container(Space::new(Length::Fixed(1.0), 0.0)).height(Length::FILL).style(estilo_linha);
        let corpo = column![
            cabecalho,
            divisor_h,
            row![
                container(esquerda).height(Length::FILL).style(|_: &pelicano::Theme| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                    ..Default::default()
                }),
                divisor_v,
                direita,
            ].height(Length::FILL),
        ];
        container(corpo).width(Length::FILL).height(Length::FILL)
            .style(|_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL())),
                ..Default::default()
            }).into()
    }

    /// Pictograma específico para cada regra: miniatura de célula e acento
    /// cromático, sem depender de fontes de ícones ou assets externos.
    fn icone_regra(tipo: TipoCondicional) -> Element<'static, Msg> {
        let (cor, marca) = match tipo {
            TipoCondicional::Maior => ("#e45250", "M11 17l0-8m-4 4l4-4 4 4"),
            TipoCondicional::Menor => ("#2379da", "M11 8v8m-4-4l4 4 4-4"),
            TipoCondicional::Entre => ("#ef9a21", "M6 12h10m-7-3l-3 3 3 3m4-6l3 3-3 3"),
            TipoCondicional::Igual => ("#9154cc", "M6 10h10M6 14h10"),
            TipoCondicional::Contem => ("#d14c75", "M5 11h12M5 15h7"),
            TipoCondicional::Duplicado => ("#f27a32", "M6 7h9v10H6zM10 5h8v9"),
            TipoCondicional::AcimaMedia => ("#23965e", "M6 16l5-7 5 7M11 9V6"),
            TipoCondicional::AbaixoMedia => ("#e75459", "M6 9l5 7 5-7M11 16v2"),
            TipoCondicional::Unico => ("#9356d0", "M6 12l3 3 6-7"),
            TipoCondicional::Vazia => ("#287de2", "M6 7h10v10H6z"),
            TipoCondicional::NaoVazia => ("#2a9ca5", "M6 7h10v10H6zM9 12h4"),
            TipoCondicional::Escala => ("#2b9b70", "M5 9h4v7H5zM9 9h4v7H9zM13 9h4v7h-4z"),
            TipoCondicional::Barra => ("#2877ce", "M5 16V9h3v7m2 0V6h3v10m2 0v-4h3v4"),
            TipoCondicional::Formula => ("#9356c9", "M7 7l3 5-3 5m8-10l-3 10"),
            _ => ("#64748b", "M5 12h13"),
        };
        let imagem = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none">
            <rect x="1" y="1" width="22" height="22" rx="5" fill="{cor}" opacity=".13"/>
            <rect x="3" y="3" width="18" height="18" rx="3" stroke="{cor}" stroke-width="1.2"/>
            <path d="{marca}" stroke="{cor}" stroke-linecap="round" stroke-linejoin="round" stroke-width="2"/>
            </svg>"##);
        // O desenho é nosso e sempre válido; se não ler, fica o espaço do ícone.
        match pelicano::Svg::from_bytes(imagem.as_bytes()) {
            Some(s) => pelicano::svg(s).width(21.0).height(21.0).into(),
            None => Space::new(21.0, 21.0).into(),
        }
    }

    fn menu_condicional(&self) -> Element<'_, Msg> {
        let mut opcoes = column![
            container(text(tr!("FORMATAÇÃO CONDICIONAL")).size(11.5).color(COR_TEXTO_MUTED()))
                .padding([8, 9]),
        ].spacing(2);
        let secoes: &[(&str, &[(&str, TipoCondicional)])] = &[
            (tr!("REALÇAR CÉLULAS"), &[
                (tr!("Maior que…"), TipoCondicional::Maior),
                (tr!("Menor que…"), TipoCondicional::Menor),
                (tr!("Entre…"), TipoCondicional::Entre),
                (tr!("Igual a…"), TipoCondicional::Igual),
                (tr!("Texto que contém…"), TipoCondicional::Contem),
                (tr!("Valores duplicados…"), TipoCondicional::Duplicado),
            ]),
            (tr!("PRIMEIROS / ÚLTIMOS VALORES"), &[
                (tr!("Acima da média…"), TipoCondicional::AcimaMedia),
                (tr!("Abaixo da média…"), TipoCondicional::AbaixoMedia),
            ]),
            (tr!("OUTRAS REGRAS"), &[
                (tr!("Valores únicos…"), TipoCondicional::Unico),
                (tr!("Células vazias…"), TipoCondicional::Vazia),
                (tr!("Células não vazias…"), TipoCondicional::NaoVazia),
                (tr!("Escala de cores…"), TipoCondicional::Escala),
                (tr!("Barras de dados…"), TipoCondicional::Barra),
                (tr!("Usar uma fórmula…"), TipoCondicional::Formula),
            ]),
        ];
        for &(titulo, regras) in secoes {
            opcoes = opcoes.push(
                container(Space::new(0.0, Length::Fixed(1.0)))
                    .width(Length::FILL).style(estilo_linha)
            );
            opcoes = opcoes.push(
                container(text(titulo).size(10.5).color(COR_TEXTO_MUTED()))
                    .padding([9, 9])
            );
            for &(rotulo, tipo) in regras {
                opcoes = opcoes.push(
                    button(row![
                        Self::icone_regra(tipo),
                        text(rotulo).size(12.5),
                        Space::new(Length::FILL, 0.0),
                        feather_ui::icones::icone("chevron-right", 14.0),
                    ].spacing(10).align(pelicano::Align::Center))
                        .on_press(Msg::CondicionalPredefinida(tipo))
                        .style(estilo_item_menu(false, true))
                        .width(Length::FILL)
                        .padding([7, 10])
                );
            }
        }
        opcoes = opcoes.push(
            container(Space::new(0.0, Length::Fixed(1.0)))
                .width(Length::FILL).style(estilo_linha)
        );
        for (rotulo, icone, evento) in [
            (tr!("Nova regra…"), "plus", Msg::CondicionalNovaRegra),
            (tr!("Gerenciar regras…"), "settings", Msg::CondicionalMostrar),
            (tr!("Limpar regras da seleção"), "eraser", Msg::CondicionalLimparSelecao),
            (tr!("Limpar regras da planilha"), "trash", Msg::CondicionalLimparTudo),
        ] {
            opcoes = opcoes.push(
                button(row![
                    feather_ui::icones::icone(icone, 18.0),
                    text(rotulo).size(12.5),
                ].spacing(11).align(pelicano::Align::Center))
                    .on_press(evento)
                    .style(estilo_item_menu(false, true))
                    .width(Length::FILL)
                    .padding([9, 10])
            );
        }
        let (_, _, largura, altura) = self.geometria_menu_condicional();
        container(pelicano::scrollable(opcoes.padding(8))
            .height(Length::Fixed(altura)))
            .width(Length::Fixed(largura))
            .style(estilo_popup_menu)
            .into()
    }

    /// Popup abaixo da Ribbon (as regras abrem em janela própria).
    pub(super) fn sobrepor_condicional<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.condicional_menu_aberto {
            let fora = pelicano::mouse_area(
                Space::new(Length::FILL, Length::FILL)
            ).on_press(Msg::CondicionalAbrir);
            let (x, y, _, _) = self.geometria_menu_condicional();
            stack![
                base,
                fora,
                column![
                    Space::new(0.0, Length::Fixed(y)),
                    row![
                        Space::new(Length::Fixed(x), 0.0),
                        self.menu_condicional(),
                    ],
                ],
            ].height(Length::FILL).into()
        } else { base }
    }
}

/// A regra em português: "Igual a “Aprovado”", "Maior que 0", "Entre 1 e 10".
fn descrever_regra(op: feathersheet_core::operacoes::rules::CellOp, valores: &[String]) -> String {
    use feathersheet_core::operacoes::rules::CellOp::*;
    let v = |i: usize| -> String {
        let s = valores.get(i).map(String::as_str).unwrap_or("").trim();
        if s.starts_with('=') || s.replace(',', ".").parse::<f64>().is_ok() { s.to_owned() }
        else { format!("“{s}”") }
    };
    match op {
        GreaterThan => trf!("Maior que {}", v(0)),
        GreaterOrEqual => trf!("Maior ou igual a {}", v(0)),
        LessThan => trf!("Menor que {}", v(0)),
        LessOrEqual => trf!("Menor ou igual a {}", v(0)),
        Equal => trf!("Igual a {}", v(0)),
        NotEqual => trf!("Diferente de {}", v(0)),
        Between => trf!("Entre {} e {}", v(0), v(1)),
        NotBetween => trf!("Fora de {} a {}", v(0), v(1)),
        IsEmpty => tr!("Célula vazia").into(),
        IsNotEmpty => tr!("Célula preenchida").into(),
        TextContains => trf!("Texto contém {}", v(0)),
        TextNotContains => trf!("Texto não contém {}", v(0)),
        TextStartsWith => trf!("Começa com {}", v(0)),
        TextEndsWith => trf!("Termina com {}", v(0)),
        DateBefore => trf!("Data antes de {}", v(0)),
        DateAfter => trf!("Data depois de {}", v(0)),
        DateOn => trf!("Data igual a {}", v(0)),
        Duplicate => tr!("Valores duplicados").into(),
        Unique => tr!("Valores únicos").into(),
        AboveAverage => tr!("Acima da média").into(),
        BelowAverage => tr!("Abaixo da média").into(),
        Formula => trf!("Fórmula {}", valores.first().map(String::as_str).unwrap_or("")),
    }
}

fn cor_de(hex: &str) -> Option<pelicano::Color> {
    feather_ui::seletor_cor::hex_valido(hex).then(|| {
        let [r, g, b, a] = feather_ui::seletor_cor::rgba_de(hex);
        pelicano::Color::rgb8(r, g, b).alpha(a as f32 / 255.0)
    })
}

/// Quadradinho com a cor digitada (xadrez vazio quando não há cor válida).
fn amostra_cor<'a>(hex: &str) -> Element<'a, Msg> {
    let cor = cor_de(hex);
    container(Space::new(Length::Fixed(28.0), Length::Fixed(28.0)))
        .style(move |_: &pelicano::Theme| pelicano::Style {
            background: cor.map(pelicano::Background::Color),
            border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [6.0; 4] },
            ..Default::default()
        }).into()
}

/// "AaBb" pintado como a regra pinta a célula.
fn amostra_estilo<'a>(fundo: Option<&str>, cor: Option<&str>, negrito: bool, italico: bool) -> Element<'a, Msg> {
    let fundo = fundo.and_then(cor_de);
    let mut t = text("AaBb 123").size(12).color(cor.and_then(cor_de).unwrap_or(COR_TEXTO()));
    if negrito || italico {
        t = t.font(pelicano::Font {
            weight: if negrito { 700 } else { 400 },
            italic: italico,
            ..pelicano::Font::DEFAULT
        });
    }
    container(t).width(Length::Fixed(78.0)).align_x(pelicano::Align::Center).height(Length::Fixed(30.0)).align_y(pelicano::Align::Center)
        .style(move |_: &pelicano::Theme| pelicano::Style {
            background: Some(pelicano::Background::Color(fundo.unwrap_or(COR_PAINEL()))),
            border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [5.0; 4] },
            ..Default::default()
        }).into()
}

/// Faixa com as cores da escala, da menor para a maior.
fn amostra_escala<'a>(cores: &[Option<&str>]) -> Element<'a, Msg> {
    let mut faixa = row![].spacing(0);
    for cor in cores.iter().flatten().filter_map(|c| cor_de(c)) {
        faixa = faixa.push(container(Space::new(Length::Fixed(26.0), Length::Fixed(30.0)))
            .style(move |_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(cor)), ..Default::default()
            }));
    }
    container(faixa).padding(1).style(|_: &pelicano::Theme| pelicano::Style {
        border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [5.0; 4] },
        ..Default::default()
    }).into()
}

/// Três barras de tamanhos diferentes na cor escolhida.
fn amostra_barra<'a>(hex: &str) -> Element<'a, Msg> {
    let cor = cor_de(hex).unwrap_or(COR_AZUL_ACENTO());
    let mut barras = column![].spacing(3);
    for largura in [64.0, 40.0, 22.0] {
        barras = barras.push(container(Space::new(Length::Fixed(largura), Length::Fixed(6.0)))
            .style(move |_: &pelicano::Theme| pelicano::Style {
                background: Some(pelicano::Background::Color(cor)),
                border: pelicano::Border { radius: [2.0; 4], ..Default::default() },
                ..Default::default()
            }));
    }
    container(barras).padding([5, 6]).width(Length::Fixed(78.0)).style(|_: &pelicano::Theme| pelicano::Style {
        background: Some(pelicano::Background::Color(COR_PAINEL())),
        border: pelicano::Border { color: COR_BORDA(), width: 1.0, radius: [5.0; 4] },
        ..Default::default()
    }).into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn opcoes_do_dropdown_usam_apenas_icones_svg_cadastrados() {
        for icone in [
            "sort-ascending", "arrow-down", "arrows-horizontal", "math-function",
            "search", "copy", "circle-check", "square", "palette", "chart-bar",
            "plus", "settings", "eraser", "trash", "chevron-right", "pointer",
        ] {
            assert!(feather_ui::icones::existe(icone), "Ícone ausente: {icone}");
        }
    }

    #[test]
    fn menu_abre_abaixo_do_botao_sem_abrir_painel_lateral() {
        let mut app = App::default();
        app.ponteiro_x = 670.0;
        app.update(Msg::CondicionalAbrir);
        assert!(app.condicional_menu_aberto);
        assert!(!app.condicional_aberto);
        assert!(app.condicional_menu_x > 0.0);
        let _: Element<'_, Msg> = app.menu_condicional();
        for tipo in [TipoCondicional::Maior, TipoCondicional::Duplicado,
                     TipoCondicional::Escala, TipoCondicional::Barra] {
            let _: Element<'static, Msg> = App::icone_regra(tipo);
        }
        app.update(Msg::CondicionalMostrar);
        assert!(!app.condicional_menu_aberto);
        assert!(app.condicional_aberto);
        let _: Element<'_, Msg> = app.view_condicional();
        app.update(Msg::CondicionalFechar);
        assert!(!app.condicional_aberto);
    }

    #[test]
    fn popup_se_ancora_ao_botao_e_se_adapta_a_janela_estreita() {
        let mut app = App::default();
        app.tamanho_principal = Size::new(411.0, 744.0);
        app.ponteiro_x = 139.0;
        app.ponteiro_y = 95.0;
        app.update(Msg::CondicionalAbrir);
        let (x, y, largura, altura) = app.geometria_menu_condicional();
        // O menu deve iniciar perto do botão (x ~ 104), não saltar para x=48.
        assert!((x - 104.0).abs() < 1.0);
        assert!((y - 131.0).abs() < 1.0);
        assert!(largura >= 290.0 && largura < LARGURA_MENU);
        assert!(x + largura <= 411.0 - MARGEM_MENU + 0.1);
        assert!(y + altura <= 744.0 - MARGEM_MENU + 0.1);
        let _: Element<'_, Msg> = app.menu_condicional();
        let _: Element<'_, Msg> = app.sobrepor_condicional(
            Space::new(Length::FILL, Length::FILL).into()
        );
    }

    #[test]
    fn popup_permanece_visivel_apos_redimensionar_e_em_telas_amplas() {
        let mut app = App::default();
        app.tamanho_principal = Size::new(1280.0, 900.0);
        app.ponteiro_x = 780.0;
        app.ponteiro_y = 110.0;
        app.update(Msg::CondicionalAbrir);
        let (x, y, largura, _) = app.geometria_menu_condicional();
        assert_eq!(x, 745.0);
        assert_eq!(y, 146.0);
        assert_eq!(largura, LARGURA_MENU);
        app.tamanho_principal = Size::new(280.0, 350.0);
        let (x, y, largura, altura) = app.geometria_menu_condicional();
        assert!(x >= MARGEM_MENU);
        assert!(x + largura <= 280.0 - MARGEM_MENU + 0.1);
        assert!(y >= MARGEM_MENU);
        assert!(y + altura <= 350.0 - MARGEM_MENU + 0.1);
    }

    #[test]
    fn aplica_se_a_define_as_areas_da_regra_e_recusa_endereco_invalido() {
        let mut app = App::default();
        app.update(Msg::CondicionalNovaRegra);
        assert!(app.condicional_aberto);
        assert_eq!(app.condicional_intervalo, "A1", "começa pela seleção");
        app.update(Msg::CondicionalIntervalo("B2:B5; D1".into()));
        app.update(Msg::CondicionalAdicionar);
        let regras = condicional::regras(&app.workbook, app.folha);
        assert_eq!(regras.len(), 1);
        let areas: Vec<_> = regras[0].ranges.iter().map(|a| (a.r0, a.c0, a.r1, a.c1)).collect();
        assert_eq!(areas, vec![(1, 1, 4, 1), (0, 3, 0, 3)]);
        // Editar traz as áreas de volta para o campo.
        app.update(Msg::CondicionalEditar(0));
        assert_eq!(app.condicional_intervalo, "B2:B5; D1");
        app.update(Msg::CondicionalIntervalo("B2:xx".into()));
        app.update(Msg::CondicionalAdicionar);
        assert_eq!(condicional::regras(&app.workbook, app.folha)[0].ranges.len(), 2, "endereço inválido não grava");
        // Só as regras que tocam a seleção (A1 não toca B2:B5 nem D1).
        app.update(Msg::CondicionalSoSelecao(true));
        let _: Element<'_, Msg> = app.view_condicional();
    }

    #[test]
    fn regras_rapidas_duplicados_e_media_usam_motor_original() {
        let mut app = App::default();
        for tipo in [
            TipoCondicional::Duplicado, TipoCondicional::AcimaMedia,
            TipoCondicional::AbaixoMedia,
        ] {
            app.update(Msg::CondicionalPredefinida(tipo));
            assert_eq!(app.condicional_operador, tipo);
            assert!(app.condicional_aberto);
            app.update(Msg::CondicionalAdicionar);
        }
        let regras = condicional::regras(&app.workbook, app.folha);
        assert_eq!(regras.len(), 3);
        use feathersheet_core::operacoes::rules::{Condition, CellOp};
        for (regra, esperado) in regras.iter().zip([
            CellOp::Duplicate, CellOp::AboveAverage, CellOp::BelowAverage
        ]) {
            assert!(matches!(&regra.condition, Condition::Cell(c) if c.op == esperado));
        }
    }
}
