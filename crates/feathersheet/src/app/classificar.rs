//! Janela "Classificar" (classificação personalizada do Excel): vários
//! níveis, cada um com coluna e ordem, "Meus dados têm cabeçalhos" e
//! "Diferenciar maiúsculas". A ordem é planejada no motor
//! (`ops::planejar_ordenacao_com`) e aplicada numa permutação só: um Ctrl+Z.
use super::*;
use feathersheet_core::celulas::{refs::col_name, values::Value};
use feathersheet_core::operacoes::ops;
use pelicano::checkbox; use pelicano::column; use pelicano::tooltip; use pelicano::TooltipPosition;

/// O Excel aceita até 64 níveis.
const MAX_NIVEIS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Nivel {
    pub coluna: u32,
    pub crescente: bool,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Selecionar(usize),
    Adicionar,
    Excluir,
    Copiar,
    Mover(i32),
    /// Nível e rótulo da coluna escolhida na lista.
    Coluna(usize, String),
    Ordem(usize, String),
    Cabecalho(bool),
    Maiusculas(bool),
    AlternarOpcoes,
    Confirmar,
    Fechar,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub area: Rect,
    pub cabecalho: bool,
    pub niveis: Vec<Nivel>,
    pub selecionado: usize,
    pub maiusculas: bool,
    pub opcoes: bool,
    pub erro: Option<String>,
}

impl App {
    pub(super) fn evento_classificar(&mut self, evento: Evento) -> Task<Msg> {
        if let Evento::Abrir = evento {
            self.abrir_classificar();
            return Task::none();
        }
        let rotulos = self.rotulos_classificar();
        let Some(e) = self.classificar.as_mut() else { return Task::none() };
        e.erro = None;
        match evento {
            Evento::Abrir => {}
            Evento::Selecionar(i) => e.selecionado = i.min(e.niveis.len().saturating_sub(1)),
            Evento::Adicionar | Evento::Copiar if e.niveis.len() >= MAX_NIVEIS =>
                e.erro = Some(trf!("No máximo {MAX_NIVEIS} níveis.")),
            Evento::Adicionar => {
                // A próxima coluna ainda sem nível, como o Excel sugere.
                let usada = |c: u32| e.niveis.iter().any(|n| n.coluna == c);
                let coluna = (e.area.c0..=e.area.c1).find(|c| !usada(*c)).unwrap_or(e.area.c0);
                e.niveis.push(Nivel { coluna, crescente: true });
                e.selecionado = e.niveis.len() - 1;
            }
            Evento::Copiar => {
                let copia = e.niveis[e.selecionado];
                e.niveis.insert(e.selecionado + 1, copia);
                e.selecionado += 1;
            }
            Evento::Excluir => {
                if e.niveis.len() > 1 {
                    e.niveis.remove(e.selecionado);
                    e.selecionado = e.selecionado.min(e.niveis.len() - 1);
                }
            }
            Evento::Mover(d) => {
                let alvo = e.selecionado as i64 + i64::from(d);
                if alvo >= 0 && (alvo as usize) < e.niveis.len() {
                    e.niveis.swap(e.selecionado, alvo as usize);
                    e.selecionado = alvo as usize;
                }
            }
            Evento::Coluna(i, rotulo) => {
                if let (Some(n), Some(p)) = (e.niveis.get_mut(i), rotulos.iter().position(|r| *r == rotulo)) {
                    n.coluna = e.area.c0 + p as u32;
                }
                e.selecionado = i;
            }
            Evento::Ordem(i, rotulo) => {
                if let Some(n) = e.niveis.get_mut(i) {
                    n.crescente = ORDENS.iter().any(|par| par[0] == rotulo || tr!(par[0]) == rotulo);
                }
                e.selecionado = i;
            }
            Evento::Cabecalho(v) => e.cabecalho = v,
            Evento::Maiusculas(v) => e.maiusculas = v,
            Evento::AlternarOpcoes => e.opcoes = !e.opcoes,
            Evento::Fechar => self.classificar = None,
            Evento::Confirmar => self.aplicar_classificar(),
        }
        Task::none()
    }

    /// A área é a seleção; com uma célula só, a região atual dos dados.
    fn abrir_classificar(&mut self) {
        self.edicao_menu = None;
        if !self.confirmar() { return; }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        if self.tem_varias_areas() {
            self.status = tr!("Classificar não funciona com várias áreas selecionadas").into();
            return;
        }
        let selecao = self.area_selecionada();
        let usada = aba.sheet.used_rect();
        let area = if selecao.r0 == selecao.r1 && selecao.c0 == selecao.c1 {
            if aba.sheet.is_empty() || selecao.r0 > usada.r1 || selecao.c0 > usada.c1 {
                self.status = tr!("Selecione os dados que deseja classificar").into();
                return;
            }
            ops::regiao_atual(&aba.sheet, selecao)
        } else {
            // Coluna inteira: só até o fim dos dados.
            Rect::new(selecao.r0, selecao.c0, selecao.r1.min(usada.r1), selecao.c1)
        };
        if area.r0 >= area.r1 {
            self.status = tr!("Selecione mais de uma linha para classificar").into();
            return;
        }
        // Cabeçalho provável: primeira linha só texto e a segunda com algum
        // número ou data (a mesma suposição do Excel).
        let texto = |r: u32, c: u32| matches!(self.workbook.valor(self.folha, r, c), Value::Text(_));
        let numero = |r: u32, c: u32| matches!(self.workbook.valor(self.folha, r, c), Value::Number(_));
        let cabecalho = (area.c0..=area.c1).all(|c| texto(area.r0, c))
            && (area.c0..=area.c1).any(|c| numero(area.r0 + 1, c));
        let coluna = self.coluna.clamp(area.c0, area.c1);
        self.classificar = Some(Estado {
            area, cabecalho, niveis: vec![Nivel { coluna, crescente: true }],
            selecionado: 0, maiusculas: false, opcoes: false, erro: None,
        });
    }

    /// Rótulo de cada coluna da área: o cabeçalho, ou "Coluna X".
    fn rotulos_classificar(&self) -> Vec<String> {
        let Some(e) = &self.classificar else { return Vec::new() };
        (e.area.c0..=e.area.c1).map(|c| {
            let titulo = feathersheet_core::celulas::values::to_text(
                &self.workbook.valor(self.folha, e.area.r0, c));
            if e.cabecalho && !titulo.trim().is_empty() { titulo }
            else { trf!("Coluna {}", col_name(c)) }
        }).collect()
    }

    /// Coluna numérica (ordem "menor/maior") ou de texto ("A a Z").
    fn coluna_numerica(&self, e: &Estado, coluna: u32) -> bool {
        let inicio = e.area.r0 + u32::from(e.cabecalho);
        (inicio..=e.area.r1.min(inicio + 200))
            .map(|r| self.workbook.valor(self.folha, r, coluna))
            .find(|v| !matches!(v, Value::Empty))
            .is_some_and(|v| matches!(v, Value::Number(_)))
    }

    fn aplicar_classificar(&mut self) {
        let Some(e) = self.classificar.clone() else { return };
        let chaves: Vec<ops::SortKey> = e.niveis.iter()
            .map(|n| ops::SortKey { col: n.coluna, ascending: n.crescente }).collect();
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let plano = ops::planejar_ordenacao_com(e.area.clone(), &chaves, e.cabecalho,
            aba.sheet.used_rect().r1, e.maiusculas, |r, c| self.workbook.valor(self.folha, r, c));
        self.classificar = None;
        self.cabecalho_dados = e.cabecalho;
        let area = e.area.clone();
        match plano {
            Some(plano) => {
                self.workbook.editar(self.folha, |sheet|
                    ops::aplicar_ordenacao(sheet, area.c0, area.c1, &plano));
                self.alterado = true;
                self.status = trf!("Dados classificados por {} nível(is)", e.niveis.len());
            }
            None => self.status = tr!("Os dados já estão nessa ordem").into(),
        }
        self.trocar_selecao(area.r0, area.c0);
        self.fim_selecao = (area.r1, area.c1);
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    fn view_classificar(&self) -> Element<'_, Msg> {
        self.dialogo_classificar().painel()
    }

    fn dialogo_classificar(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.classificar.as_ref().expect("janela aberta");
        let negrito = pelicano::Font { weight: 600, ..Default::default() };
        let ev = Msg::Classificar;
        let rotulos = self.rotulos_classificar();
        let botao_icone = |icone: &'static str, rotulo: &'static str, dica: &'static str, msg: Option<Msg>| tooltip(
            button(row![feather_ui::icones::icone(icone, 15.0), text(rotulo).size(12.5)]
                    .spacing(6).align(pelicano::Align::Center))
                .padding([6, 10]).style(estilo_botao_secundario).on_press_maybe(msg),
            dica,
            TooltipPosition::Bottom,
        ).padding([4, 8]).style(estilo_dica);
        let so_icone = |icone: &'static str, dica: &'static str, msg: Option<Msg>| tooltip(
            button(container(feather_ui::icones::icone(icone, 15.0))
                    .width(Length::FILL).height(Length::FILL).center())
                .width(Length::Fixed(30.0)).height(Length::Fixed(30.0)).padding(0)
                .style(estilo_botao_secundario).on_press_maybe(msg),
            dica,
            TooltipPosition::Bottom,
        ).padding([4, 8]).style(estilo_dica);
        let n = e.niveis.len();
        let barra = row![
            botao_icone("plus", tr!("Adicionar nível"), tr!("Mais uma coluna para desempatar"),
                (n < MAX_NIVEIS).then_some(ev(Evento::Adicionar))),
            botao_icone("trash", tr!("Excluir nível"), tr!("Remove o nível selecionado"),
                (n > 1).then_some(ev(Evento::Excluir))),
            botao_icone("copy", tr!("Copiar nível"), tr!("Duplica o nível selecionado"),
                (n < MAX_NIVEIS).then_some(ev(Evento::Copiar))),
            so_icone("arrow-up", tr!("Subir nível"), (e.selecionado > 0).then_some(ev(Evento::Mover(-1)))),
            so_icone("arrow-down", tr!("Descer nível"),
                (e.selecionado + 1 < n).then_some(ev(Evento::Mover(1)))),
            Space::new(Length::FILL, 0.0),
            checkbox(tr!("Meus dados têm cabeçalhos"), e.cabecalho)
                .on_toggle(|v| Msg::Classificar(Evento::Cabecalho(v))).size(12.5),
        ].spacing(6).align(pelicano::Align::Center);

        let cab = |t: &'static str, w: Length| container(text(t).size(11.5).font(negrito)
            .color(COR_TEXTO_MUTED())).width(w);
        let (w1, w2, w3, w4) = (Length::Fixed(118.0), Length::Fill(3),
            Length::Fill(2), Length::Fill(3));
        let mut linhas = column![
            container(row![cab("", w1), cab(tr!("Coluna"), w2), cab(tr!("Classificar em"), w3), cab(tr!("Ordem"), w4)]
                .spacing(8)).padding([6, 10]).width(Length::FILL)
                .style(|_| pelicano::Style {
                    background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                    border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [6.0; 4] },
                    ..Default::default()
                }),
        ].spacing(4);
        for (i, nivel) in e.niveis.iter().enumerate() {
            let rotulo = rotulos.get((nivel.coluna - e.area.c0) as usize).cloned().unwrap_or_default();
            let par = ORDENS[usize::from(self.coluna_numerica(e, nivel.coluna))];
            let ordem = tr!(if nivel.crescente { par[0] } else { par[1] });
            let ordens: Vec<String> = par.iter().map(|s| tr!(*s).to_string()).collect();
            let selecionado = i == e.selecionado;
            let linha = row![
                button(text(if i == 0 { tr!("Classificar por") } else { tr!("E depois por") }).size(12.5))
                    .width(w1).padding([6, 8]).style(estilo_botao_pequeno(selecionado))
                    .on_press(ev(Evento::Selecionar(i))),
                crate::compat::pick_list_vec(rotulos.clone(), Some(rotulo), move |r| Msg::Classificar(Evento::Coluna(i, r)))
                    .size(12.5).padding([4, 8]).width(w2),
                container(tooltip(
                    container(text(tr!("Valores")).size(12.5)).padding([6, 8]).width(Length::FILL),
                    tr!("Cor da célula, cor da fonte e ícone: em breve"),
                    TooltipPosition::Bottom,
                ).padding([4, 8]).style(estilo_dica)).width(w3),
                crate::compat::pick_list_vec(ordens, Some(ordem.to_string()), move |o| Msg::Classificar(Evento::Ordem(i, o)))
                    .size(12.5).padding([4, 8]).width(w4),
            ].spacing(8).align(pelicano::Align::Center);
            linhas = linhas.push(container(linha).padding([2, 10]));
        }

        let mut corpo = column![
            barra,
            container(rolagem(linhas)).max_height(260.0),
        ].spacing(14);
        let opcoes = button(row![
            text(tr!("Opções")).size(12.5),
            feather_ui::icones::icone(if e.opcoes { "chevron-up" } else { "chevron-down" }, 14.0),
        ].spacing(5).align(pelicano::Align::Center))
            .padding([5, 10]).style(estilo_botao_pequeno(e.opcoes)).on_press(ev(Evento::AlternarOpcoes));
        if e.opcoes {
            corpo = corpo.push(column![
                checkbox(tr!("Diferenciar maiúsculas e minúsculas"), e.maiusculas)
                    .on_toggle(|v| Msg::Classificar(Evento::Maiusculas(v))).size(12.5),
                tooltip(
                    row![feather_ui::icones::icone("info-circle", 14.0),
                        text(tr!("Orientação: de cima para baixo (da esquerda para a direita em breve)"))
                            .size(11.5).color(COR_TEXTO_MUTED())].spacing(6).align(pelicano::Align::Center),
                    tr!("Classificar colunas lado a lado ainda não está disponível"),
                    TooltipPosition::Bottom,
                ).padding([4, 8]).style(estilo_dica),
            ].spacing(8));
        }
        if let Some(erro) = &e.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(tr!("Classificar"), corpo)
            .icone("sort-ascending")
            .subtitulo(trf!("Área {}:{}", cell_name(e.area.r0, e.area.c0), cell_name(e.area.r1, e.area.c1)))
            .largura(680.0)
            .ao_fechar(ev(Evento::Fechar))
            .rodape_esquerda(opcoes)
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", Some(ev(Evento::Confirmar)))
    }

    pub(super) fn sobrepor_classificar<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.classificar.is_none() { return base; }
        self.dialogo_classificar().sobre(base)
    }
}

/// Rótulos da ordem: [crescente, decrescente] para texto e para número.
const ORDENS: [[&str; 2]; 2] = [
    ["A a Z", "Z a A"],
    ["Do menor para o maior", "Do maior para o menor"],
];

#[cfg(test)]
mod testes {
    use super::*;

    fn planilha() -> App {
        let mut app = App::default();
        let dados = [["Cidade", "Valor"], ["Rio", "1"], ["Belém", "5"], ["Rio", "9"], ["Belém", "2"]];
        for (r, linha) in dados.iter().enumerate() {
            for (c, t) in linha.iter().enumerate() {
                app.workbook.escrever_celula(0, r as u32, c as u32, (*t).into());
            }
        }
        app
    }

    #[test]
    fn abre_na_regiao_com_cabecalho_e_classifica_dois_niveis_num_desfazer() {
        let mut app = planilha();
        let _ = app.update(Msg::Selecionar(2, 0));
        let _ = app.update(Msg::Classificar(Evento::Abrir));
        let e = app.classificar.as_ref().expect("abriu");
        assert_eq!(e.area, Rect::new(0, 0, 4, 1));
        assert!(e.cabecalho);
        assert_eq!(app.rotulos_classificar(), ["Cidade", "Valor"]);
        let _ = app.update(Msg::Classificar(Evento::Adicionar));
        let _ = app.update(Msg::Classificar(Evento::Ordem(1, "Do maior para o menor".into())));
        let _: Element<'_, Msg> = app.view_classificar();
        let _ = app.update(Msg::Classificar(Evento::Confirmar));
        let valor = |app: &App, r| app.workbook.valor(0, r, 1);
        assert_eq!((1..=4).map(|r| valor(&app, r)).collect::<Vec<_>>(),
            [5., 2., 9., 1.].map(Value::Number));
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Text("Cidade".into()));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(valor(&app, 1), Value::Number(1.));
    }

    #[test]
    fn niveis_copiar_mover_e_excluir_e_janela_nos_dois_temas() {
        let mut app = planilha();
        let _ = app.update(Msg::Classificar(Evento::Abrir));
        let _ = app.update(Msg::Classificar(Evento::Coluna(0, "Valor".into())));
        let _ = app.update(Msg::Classificar(Evento::Copiar));
        let _ = app.update(Msg::Classificar(Evento::Coluna(1, "Cidade".into())));
        let _ = app.update(Msg::Classificar(Evento::Mover(-1)));
        let e = app.classificar.as_ref().unwrap();
        assert_eq!(e.niveis.iter().map(|n| n.coluna).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(e.selecionado, 0);
        let _ = app.update(Msg::Classificar(Evento::Excluir));
        assert_eq!(app.classificar.as_ref().unwrap().niveis.len(), 1);
        let _ = app.update(Msg::Classificar(Evento::AlternarOpcoes));
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _: Element<'_, Msg> = app.view_classificar();
        }
        let _ = app.update(Msg::Classificar(Evento::Fechar));
        assert!(app.classificar.is_none());
    }
}
