//! Exibir ▸ Janela ▸ Nova janela, Organizar tudo e Alternar janelas (#83). A nova janela mostra a mesma pasta
//! numa janela do sistema (para outro monitor, por exemplo): as abas no alto, a grade da aba escolhida com a sua
//! rolagem, imagens, formas, gráficos e tinta, sempre ao vivo. Ela é uma vista: clicar numa célula leva a janela
//! principal até ela (aba e célula) e a traz para a frente, onde ficam a faixa e a barra de fórmulas.
// shortcut: a nova janela não edita no lugar (o Excel edita em qualquer janela); a edição continua na principal.
// Se pedirem, levar o estado ativo para ela como a janela dividida faz entre os painéis.
use super::*;
use super::painel_dividido::Painel;

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    /// A aba (índice) que a nova janela mostra.
    Aba(usize),
    /// Põe a principal e a nova lado a lado, cada uma com metade da largura da principal.
    Organizar,
    /// Traz a outra janela para a frente.
    Alternar,
}

impl App {
    pub(super) fn evento_nova_janela(&mut self, e: Evento) -> Task<Msg> {
        match e {
            Evento::Abrir => {
                if let Some(id) = self.janelas.id(Auxiliar::NovaJanela) {
                    return Task::window_of(id, pelicano::WindowCmd::Focus);
                }
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                self.janela_extra = Some(Painel::de(aba.id, self.linha_inicio, self.coluna_inicio, self.linha, self.coluna));
                self.alternar(Auxiliar::NovaJanela)
            }
            Evento::Aba(i) => {
                if let (Some(p), Some(aba)) = (self.janela_extra.as_mut(), self.workbook.aba(i)) {
                    *p = Painel::de(aba.id, 0, 0, 0, 0);
                }
                Task::none()
            }
            Evento::Organizar => {
                let (Some(principal), Some(nova)) = (self.janelas.principal, self.janelas.id(Auxiliar::NovaJanela)) else {
                    self.status = tr!("Abra uma Nova janela para organizar as duas.").into();
                    return Task::none();
                };
                // A principal ocupa a tela (maximizada) na maioria das vezes: metade dela para cada uma.
                // shortcut: o Wayland não deixa o app mover janelas; lá só o tamanho muda.
                let metade = Size::new((self.tamanho_principal.width / 2.0).max(400.0), self.tamanho_principal.height.max(300.0));
                Task::batch([
                    Task::window_of(principal, pelicano::WindowCmd::Maximize(false)),
                    Task::window_of(principal, pelicano::WindowCmd::Move(pelicano::Point::new(0.0, 0.0))),
                    Task::window_of(principal, pelicano::WindowCmd::Resize(metade)),
                    Task::window_of(nova, pelicano::WindowCmd::Move(pelicano::Point::new(metade.width, 0.0))),
                    Task::window_of(nova, pelicano::WindowCmd::Resize(metade)),
                ])
            }
            Evento::Alternar => {
                let (Some(principal), Some(nova)) = (self.janelas.principal, self.janelas.id(Auxiliar::NovaJanela)) else {
                    self.status = tr!("Só há uma janela desta pasta.").into();
                    return Task::none();
                };
                self.foco_na_nova = !self.foco_na_nova;
                Task::window_of(if self.foco_na_nova { nova } else { principal }, pelicano::WindowCmd::Focus)
            }
        }
    }

    /// Mensagem da grade da nova janela: a roda rola a vista; o clique leva a principal até a célula.
    pub(super) fn na_nova_janela(&mut self, m: Msg) -> Task<Msg> {
        let Some(p) = self.janela_extra.as_mut() else { return Task::none() };
        match m {
            Msg::RolarNaGrade(linha, coluna, dc, _) => {
                p.linha_inicio = linha.min(MAX_ROWS - 1);
                if dc != 0 { p.coluna_inicio = (i64::from(coluna) + i64::from(dc)).clamp(0, i64::from(MAX_COLS) - 1) as u32; }
                Task::none()
            }
            Msg::Selecionar(r, c) => {
                (p.linha, p.coluna) = (r, c);
                let Some(destino) = self.workbook.indice_do_id(p.aba_id) else { return Task::none() };
                let troca = if destino != self.folha { self.update(Msg::Aba(destino)) } else { Task::none() };
                if self.folha != destino { return troca; }
                self.trocar_selecao_por_clique(r, c);
                self.foco_na_nova = false;
                let foco = self.janelas.principal.map_or_else(Task::none, |id| Task::window_of(id, pelicano::WindowCmd::Focus));
                Task::batch([troca, foco])
            }
            _ => Task::none(),
        }
    }

    pub(super) fn view_nova_janela(&self) -> Element<'_, Msg> {
        let Some(p) = &self.janela_extra else { return Space::new(0.0, 0.0).into() };
        let Some(indice) = self.workbook.indice_do_id(p.aba_id) else {
            return container(text(tr!("A aba desta janela foi excluída: escolha outra.")).size(13)).padding(16).into();
        };
        let mut abas = row(Vec::<Element<'_, Msg>>::new()).spacing(4);
        for (i, nome) in self.workbook.nomes().into_iter().enumerate() {
            abas = abas.push(button(text(nome).size(12)).padding([4, 10]).style(estilo_botao_pequeno(i == indice))
                .on_press(Msg::NovaJanela(Evento::Aba(i))));
        }
        let grade = self.desenhar_outro_painel(p, indice).map(|m| Msg::NaNovaJanela(Box::new(m)));
        pelicano::column![
            // A fileira de abas com altura fixa: o `scrollable` ocupa a altura toda por padrão e a grade sumia.
            container(scrollable(abas).direction(pelicano::Direction::Horizontal).scrollbar_width(3.0).height(Length::Fixed(30.0))).padding([6, 8]),
            grade,
        ].width(Length::FILL).height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A nova janela guarda a sua aba e rolagem; o clique nela leva a principal até a aba e a célula; a roda rola
    /// só a vista; Organizar e Alternar sem a segunda janela avisam.
    #[test]
    fn nova_janela_mostra_outra_aba_e_clique_leva_a_principal() {
        let mut app = App::default();
        let _ = app.update(Msg::AdicionarAba);
        let _ = app.update(Msg::Aba(0));
        let _ = app.update(Msg::NovaJanela(Evento::Abrir));
        assert!(app.janela_extra.is_some());
        let _ = app.update(Msg::NovaJanela(Evento::Aba(1)));
        let _: Element<'_, Msg> = app.view_nova_janela();
        let _ = app.update(Msg::NaNovaJanela(Box::new(Msg::RolarNaGrade(30, 0, 0, false))));
        assert_eq!(app.janela_extra.as_ref().unwrap().linha_inicio, 30);
        assert_eq!((app.folha, app.linha_inicio), (0, 0), "a principal não rola junto");
        let _ = app.update(Msg::NaNovaJanela(Box::new(Msg::Selecionar(32, 2))));
        assert_eq!((app.folha, app.linha, app.coluna), (1, 32, 2));
        // Sem a segunda janela do sistema aberta (teste), Organizar e Alternar só avisam.
        let _ = app.update(Msg::NovaJanela(Evento::Organizar));
        assert!(!app.status.is_empty());
    }

    /// A grade aparece na nova janela (a fileira de abas tomava a altura toda e a janela saía vazia): na
    /// metade de baixo da foto há mais de uma cor (linhas, cabeçalhos).
    #[test]
    fn grade_da_nova_janela_aparece() {
        let mut app = App::default();
        let _ = app.update(Msg::NovaJanela(Evento::Abrir));
        let png = pelicano::capture(&app.view_nova_janela(), Size::new(400.0, 300.0), 1.0, &tema(), &mut pelicano::Renderer::new(), None);
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let cores: std::collections::HashSet<[u8; 4]> = (150..290).flat_map(|y| (10..390).map(move |x| (x, y)))
            .map(|(x, y)| img.get_pixel(x, y).0).collect();
        assert!(cores.len() > 3, "a metade de baixo saiu de uma cor só: {cores:?}");
    }
}
