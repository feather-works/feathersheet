//! Revisão ▸ Mostrar alterações (#82): painel à direita com o que mudou no texto das células da
//! aba nesta sessão, a mais nova em cima (o registro é do motor: `Sheet::alteracoes`).
//! Com "Só a seleção", só as que tocam a área selecionada, como o Excel faz pelo menu da célula.
use super::*;
use feathersheet_core::celulas::refs::cell_name;
use feathersheet_core::planilha::sheet::edicao::Alteracao;

/// Quantas o painel desenha (o motor guarda mais).
const NO_PAINEL: usize = 200;

#[derive(Debug, Clone)]
pub enum Evento {
    Alternar,
    SoSelecao(bool),
    Ir(u32, u32),
}

fn endereco(a: &Rect) -> String {
    if a.r0 == a.r1 && a.c0 == a.c1 { cell_name(a.r0, a.c0) }
    else { format!("{}:{}", cell_name(a.r0, a.c0), cell_name(a.r1, a.c1)) }
}

fn cortar(t: &str) -> String {
    if t.is_empty() { return tr!("(vazia)").into(); }
    let mut s: String = t.chars().take(40).collect();
    if t.chars().count() > 40 { s.push('…'); }
    s
}

fn toca(a: &Rect, b: &Rect) -> bool { a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1 }

impl App {
    pub(super) fn evento_alteracoes(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Alternar => self.alteracoes_painel = self.alteracoes_painel.map_or(Some(false), |_| None),
            Evento::SoSelecao(so) => if self.alteracoes_painel.is_some() { self.alteracoes_painel = Some(so); },
            Evento::Ir(r, c) => self.trocar_selecao(r, c),
        }
        Task::none()
    }

    /// As alterações da aba ativa que o painel mostra, da mais nova à mais velha.
    pub(super) fn alteracoes_visiveis(&self) -> Vec<&Alteracao> {
        let so = self.alteracoes_painel == Some(true);
        let sel = self.area_selecionada();
        self.workbook.aba(self.folha).map(|a| a.sheet.alteracoes().iter().rev()
            .filter(|x| !so || toca(&x.area, &sel)).take(NO_PAINEL).collect()).unwrap_or_default()
    }

    fn painel_alteracoes(&self) -> Element<'_, Msg> {
        use pelicano::column;
        let m = |e: Evento| Msg::Alteracoes(e);
        let so = self.alteracoes_painel == Some(true);
        let mut lista = column(Vec::<Element<'_, Msg>>::new()).spacing(2);
        let itens = self.alteracoes_visiveis();
        for a in &itens {
            let hora = super::historico_macros::hora_local(std::time::UNIX_EPOCH + std::time::Duration::from_secs(a.quando));
            let o_que = if a.celulas > 1 { trf!("{} células alteradas", a.celulas) }
                else { format!("{} → {}", cortar(&a.antes), cortar(&a.depois)) };
            lista = lista.push(button(column![
                row![
                    text(endereco(&a.area)).size(12).font(pelicano::Font::MONO),
                    Space::new(Length::FILL, 0.0),
                    text(hora).size(11).color(COR_TEXTO_MUTED()),
                ],
                text(o_que).size(12),
            ].spacing(2)).width(Length::FILL).padding([5, 7]).style(estilo_botao_pequeno(false))
                .on_press(m(Evento::Ir(a.area.r0, a.area.c0))));
        }
        if itens.is_empty() {
            lista = lista.push(text(if so { tr!("Nenhuma alteração na seleção.") } else { tr!("Nenhuma alteração nesta aba desde que o arquivo foi aberto.") })
                .size(12).color(COR_TEXTO_MUTED()));
        }
        container(column![
            row![
                text(tr!("Alterações")).size(13),
                Space::new(Length::FILL, 0.0),
                button(feather_ui::icones::icone("x", 15.0)).padding(5).style(estilo_botao_pequeno(false))
                    .on_press(m(Evento::Alternar)),
            ].align(pelicano::Align::Center),
            row![
                text(tr!("Só a seleção")).size(12).width(Length::FILL),
                toggler("", so).on_toggle(move |v| m(Evento::SoSelecao(v))).color(COR_AZUL_ACENTO()),
            ].align(pelicano::Align::Center),
            scrollable(lista).height(Length::Fixed((self.tamanho_principal.height - self.topo_dialogo() - 170.0).max(160.0))),
            text(tr!("Vale para esta sessão: o registro não vai para o arquivo.")).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(8).padding(10)).width(Length::Fixed(300.0)).style(estilo_painel_faixa).into()
    }

    pub(super) fn sobrepor_alteracoes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.alteracoes_painel.is_none() { return base; }
        // Encostado à direita, abaixo da faixa, como o painel do Excel.
        let x = (self.tamanho_principal.width - 308.0).max(8.0);
        stack![
            base,
            pelicano::column![
                Space::new(0.0, Length::Fixed(self.topo_dialogo())),
                row![Space::new(Length::Fixed(x), 0.0), pelicano::opaque(self.painel_alteracoes())],
            ],
        ].width(Length::FILL).height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn painel_lista_a_mais_nova_primeiro_e_filtra_pela_selecao() {
        let mut app = App::default();
        app.workbook.editar(0, |s| { s.set_cell(0, 0, "a"); s.set_cell(4, 4, "b"); });
        let _ = app.update(Msg::Alteracoes(Evento::Alternar));
        let v = app.alteracoes_visiveis();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].depois, "b");
        app.linha = 0; app.coluna = 0;
        app.fim_selecao = (0, 0);
        let _ = app.update(Msg::Alteracoes(Evento::SoSelecao(true)));
        let v = app.alteracoes_visiveis();
        assert_eq!((v.len(), v[0].depois.as_str()), (1, "a"));
        { let _p: Element<'_, Msg> = app.painel_alteracoes(); }
        let _ = app.update(Msg::Alteracoes(Evento::Alternar));
        assert!(app.alteracoes_painel.is_none());
    }
}
