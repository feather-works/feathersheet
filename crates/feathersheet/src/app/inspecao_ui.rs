//! Fórmulas → Janela de inspeção: acompanha o valor de células que estão
//! longe da tela enquanto se mexe na planilha, como no Excel.
//!
//! Janela de ferramenta (padrão do Alt+F11): barra de ações, uma área com a
//! tabela das inspeções e o rodapé de status. A lista vale para a sessão:
//! guarda o `id` da aba (não o índice, que muda ao mover abas) e some
//! quando outra pasta é aberta.
use super::*;
use pelicano::column;
use feathersheet_core::celulas::locale::apresentar_formula;
use feathersheet_core::celulas::style::format_value;

/// Quantas células uma seleção grande acrescenta de uma vez.
const MAX_POR_VEZ: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inspecao {
    pub aba_id: u32,
    pub linha: u32,
    pub coluna: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Abrir,
    Adicionar,
    Selecionar(usize),
    Ir(usize),
    Excluir(usize),
    ExcluirTodas,
}

/// Uma linha pronta para mostrar.
struct Linha {
    planilha: String,
    nome: String,
    celula: String,
    valor: String,
    formula: String,
}

fn cortar(texto: &str, limite: usize) -> String {
    if texto.chars().count() > limite {
        format!("{}…", texto.chars().take(limite - 1).collect::<String>())
    } else { texto.to_owned() }
}

impl App {
    fn indice_da_aba(&self, id: u32) -> Option<usize> {
        (0..self.workbook.len()).find(|&i| self.workbook.aba(i).is_some_and(|a| a.id == id))
    }

    pub(super) fn evento_inspecao(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => return self.alternar(Auxiliar::Inspecao),
            Evento::Adicionar => {
                let Some(aba_id) = self.workbook.aba(self.folha).map(|a| a.id) else { return Task::none() };
                let area = self.area_selecionada();
                let mut novas = 0;
                'fora: for linha in area.r0..=area.r1 {
                    for coluna in area.c0..=area.c1 {
                        if novas >= MAX_POR_VEZ { break 'fora; }
                        let item = Inspecao { aba_id, linha, coluna };
                        if !self.inspecoes.contains(&item) {
                            self.inspecoes.push(item);
                            novas += 1;
                        }
                    }
                }
                self.inspecao_selecionada = self.inspecoes.len().checked_sub(1);
                self.status = trf!("{novas} inspeção(ões) adicionada(s)");
                if !self.janelas.aberta(Auxiliar::Inspecao) {
                    return self.alternar(Auxiliar::Inspecao);
                }
            }
            Evento::Selecionar(i) => self.inspecao_selecionada = Some(i),
            Evento::Ir(i) => {
                let Some(item) = self.inspecoes.get(i).copied() else { return Task::none() };
                let Some(folha) = self.indice_da_aba(item.aba_id) else { return Task::none() };
                if !self.confirmar() { return Task::none(); }
                self.inspecao_selecionada = Some(i);
                self.folha = folha;
                self.nome_aba = self.workbook.nomes()[folha].clone();
                self.trocar_selecao(item.linha, item.coluna);
                self.endereco = cell_name(item.linha, item.coluna);
                if let Some(id) = self.janelas.principal {
                    return Task::window_of(id, pelicano::WindowCmd::Focus);
                }
            }
            Evento::Excluir(i) => {
                if i < self.inspecoes.len() {
                    self.inspecoes.remove(i);
                    self.inspecao_selecionada = (!self.inspecoes.is_empty())
                        .then(|| i.min(self.inspecoes.len() - 1));
                }
            }
            Evento::ExcluirTodas => {
                self.inspecoes.clear();
                self.inspecao_selecionada = None;
            }
        }
        Task::none()
    }

    /// Esquece inspeções de abas que não existem mais (aba excluída ou pasta trocada).
    pub(super) fn limpar_inspecoes(&mut self) {
        let validas: Vec<Inspecao> = self.inspecoes.iter().copied()
            .filter(|i| self.indice_da_aba(i.aba_id).is_some()).collect();
        if validas.len() != self.inspecoes.len() {
            self.inspecoes = validas;
            self.inspecao_selecionada = None;
        }
    }

    fn linha_inspecao(&self, item: &Inspecao) -> Option<Linha> {
        let indice = self.indice_da_aba(item.aba_id)?;
        let aba = self.workbook.aba(indice)?;
        let (r, c) = (item.linha, item.coluna);
        let nome = self.workbook.nomeados().into_iter()
            .find(|n| (n.area.r0, n.area.c0, n.area.r1, n.area.c1) == (r, c, r, c)
                && n.area.sheet.as_deref().is_none_or(|s| s.eq_ignore_ascii_case(&aba.sheet.name)))
            .map(|n| n.nome).unwrap_or_default();
        let bruto = aba.sheet.get_raw(r, c);
        let formula = if bruto.starts_with('=') {
            apresentar_formula(&bruto, &self.regionalizacao())
        } else { String::new() };
        let valor = format_value(&self.workbook.valor(indice, r, c), aba.sheet.estilo_resolvido(r, c).as_ref());
        Some(Linha { planilha: aba.sheet.name.clone(), nome, celula: cell_name(r, c), valor, formula })
    }

    pub(super) fn view_inspecao(&self) -> Element<'_, Msg> {
        let estilo_area = |_: &pelicano::Theme| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_PAINEL())),
            border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [9.0; 4] },
            ..Default::default()
        };
        let estilo_cabecalho = |_: &pelicano::Theme| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
            border: pelicano::Border { radius: [6.0; 4], ..Default::default() },
            ..Default::default()
        };
        let larguras = [120.0, 110.0, 70.0, 140.0];
        let colunas = |planilha: Element<'static, Msg>, nome: Element<'static, Msg>, celula: Element<'static, Msg>,
            valor: Element<'static, Msg>, formula: Element<'static, Msg>, acoes: Element<'static, Msg>| {
            row![
                container(planilha).width(Length::Fixed(larguras[0])),
                container(nome).width(Length::Fixed(larguras[1])),
                container(celula).width(Length::Fixed(larguras[2])),
                container(valor).width(Length::Fixed(larguras[3])),
                container(formula).width(Length::FILL),
                container(acoes).width(Length::Fixed(64.0)),
            ].spacing(8).align(pelicano::Align::Center)
        };
        let titulo = |t: &'static str| -> Element<'static, Msg> {
            text(t).size(11.5).color(COR_TEXTO_MUTED()).into()
        };
        let cabecalho = container(colunas(titulo(tr!("Planilha")), titulo(tr!("Nome")), titulo(tr!("Célula")),
            titulo(tr!("Valor")), titulo(tr!("Fórmula")), titulo("")))
            .padding([6, 10]).width(Length::FILL).style(estilo_cabecalho);

        let mut lista = column(Vec::new()).spacing(1);
        for (i, item) in self.inspecoes.iter().enumerate() {
            let Some(l) = self.linha_inspecao(item) else { continue };
            let celula = |t: String, limite: usize| -> Element<'static, Msg> {
                text(cortar(&t, limite)).size(12.5).into()
            };
            let acoes: Element<'static, Msg> = row![
                feather_ui::formatacao::dica(
                    button(feather_ui::icones::icone("arrow-right", 14.)).padding(5)
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::Inspecao(Evento::Ir(i))),
                    tr!("Ir até a célula")),
                feather_ui::formatacao::dica(
                    button(feather_ui::icones::icone("trash", 14.)).padding(5)
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::Inspecao(Evento::Excluir(i))),
                    tr!("Excluir inspeção")),
            ].spacing(2).into();
            let linha = colunas(celula(l.planilha, 16), celula(l.nome, 14), celula(l.celula, 9),
                celula(l.valor, 19), text(cortar(&l.formula, 60)).size(12.5).font(pelicano::Font::MONO).into(),
                acoes);
            lista = lista.push(button(linha).width(Length::FILL).padding([3, 10])
                .style(estilo_botao_pequeno(self.inspecao_selecionada == Some(i)))
                .on_press(Msg::Inspecao(Evento::Selecionar(i))));
        }
        let corpo: Element<'_, Msg> = if self.inspecoes.is_empty() {
            container(column![
                feather_ui::icones::icone("eye", 30.).color(COR_TEXTO_MUTED()),
                text(tr!("Nenhuma célula em inspeção")).size(14),
                text(tr!("Selecione células na planilha e clique em Adicionar inspeção."))
                    .size(12).color(COR_TEXTO_MUTED()),
            ].spacing(8).align(pelicano::Align::Center))
                .width(Length::FILL).align_x(pelicano::Align::Center)
                .height(Length::Fixed(220.)).align_y(pelicano::Align::Center).into()
        } else {
            feather_ui::tema::rolagem(lista).height(Length::FILL).into()
        };
        let selecao = self.area_selecionada();
        let alvo = endereco_area(&selecao);
        let ferramentas = row![
            button(row![feather_ui::icones::icone("plus", 15.)
                .color(pelicano::Color::WHITE),
                text(tr!("Adicionar inspeção")).size(12.5)]
                .spacing(6).align(pelicano::Align::Center))
                .padding([6, 10]).style(estilo_botao_primario)
                .on_press(Msg::Inspecao(Evento::Adicionar)),
            text(trf!("Seleção atual: {alvo}")).size(11.5).color(COR_TEXTO_MUTED()),
            Space::new(Length::FILL, 0.0),
            button(row![feather_ui::icones::icone("trash", 15.), text(tr!("Excluir inspeção")).size(12.5)]
                .spacing(6).align(pelicano::Align::Center))
                .padding([6, 10]).style(estilo_botao_secundario)
                .on_press_maybe(self.inspecao_selecionada.map(|i| Msg::Inspecao(Evento::Excluir(i)))),
            button(text(tr!("Excluir todas")).size(12.5))
                .padding([6, 10]).style(estilo_botao_secundario)
                .on_press_maybe((!self.inspecoes.is_empty()).then_some(Msg::Inspecao(Evento::ExcluirTodas))),
        ].spacing(8).align(pelicano::Align::Center);
        let rodape = text(trf!("{} inspeção(ões) · os valores se atualizam a cada mudança na pasta",
            self.inspecoes.len())).size(11.5).color(COR_TEXTO_MUTED());
        container(column![
            ferramentas,
            container(column![cabecalho, corpo].spacing(4)).padding(6)
                .width(Length::FILL).height(Length::FILL).style(estilo_area),
            rodape,
        ].spacing(8))
            .padding([10, 12]).width(Length::FILL).height(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_FUNDO_JANELA())),
                ..Default::default()
            })
            .into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn inspecao_acompanha_valor_e_some_com_a_aba() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "2".into());
        app.workbook.escrever_celula(0, 0, 1, "=A1*10".into());
        app.trocar_selecao(0, 1);
        let _ = app.update(Msg::Inspecao(Evento::Adicionar));
        let _ = app.update(Msg::Inspecao(Evento::Adicionar));
        assert_eq!(app.inspecoes.len(), 1, "não repete a mesma célula");
        assert!(app.janelas.aberta(Auxiliar::Inspecao));
        assert_eq!(app.linha_inspecao(&app.inspecoes[0]).unwrap().valor, "20");
        app.workbook.escrever_celula(0, 0, 0, "3".into());
        let linha = app.linha_inspecao(&app.inspecoes[0]).unwrap();
        assert_eq!((linha.valor.as_str(), linha.formula.as_str(), linha.celula.as_str()), ("30", "=A1*10", "B1"));
        let _: Element<'_, Msg> = app.view_inspecao();
        // Aba excluída: a inspeção dela some.
        app.workbook.criar_aba("Dados");
        app.folha = 1;
        app.trocar_selecao(0, 0);
        let _ = app.update(Msg::Inspecao(Evento::Adicionar));
        assert_eq!(app.inspecoes.len(), 2);
        app.folha = 0;
        app.workbook.remover_aba(1);
        let _ = app.update(Msg::Inspecao(Evento::Selecionar(0)));
        assert_eq!(app.inspecoes.len(), 1);
        let _: Element<'_, Msg> = app.view_inspecao();
    }

    #[test]
    fn ir_volta_para_a_aba_e_celula_inspecionadas() {
        let mut app = App::default();
        app.workbook.criar_aba("Dados");
        app.folha = 1;
        app.trocar_selecao(4, 2);
        let _ = app.update(Msg::Inspecao(Evento::Adicionar));
        app.folha = 0;
        app.trocar_selecao(0, 0);
        let _ = app.update(Msg::Inspecao(Evento::Ir(0)));
        assert_eq!((app.folha, app.linha, app.coluna), (1, 4, 2));
        let _ = app.update(Msg::Inspecao(Evento::Excluir(0)));
        assert!(app.inspecoes.is_empty());
    }
}
