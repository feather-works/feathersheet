//! Fórmulas → Avaliar fórmula: o diálogo do Excel que calcula a fórmula da
//! célula ativa um pedaço por vez. A conta é do motor
//! (`formula::avaliar_passos`); aqui ficam o desenho e o Entrar/Sair, que
//! empilha a fórmula de uma célula referenciada.
use super::*;
use pelicano::column; use pelicano::rich_text; use pelicano::span;
use feathersheet_core::formula::avaliar_passos::Avaliacao;

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Abrir,
    Avaliar,
    Entrar,
    Sair,
    Reiniciar,
    Fechar,
}

/// A pilha: a primeira é a célula de onde se abriu; Entrar acrescenta.
#[derive(Debug, Clone)]
pub struct Estado {
    pub pilha: Vec<Avaliacao>,
}

impl App {
    pub(super) fn evento_avaliar(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                self.formulas_menu = None;
                match Avaliacao::nova(&self.workbook, self.folha, self.linha, self.coluna) {
                    Some(a) => self.avaliar = Some(Estado { pilha: vec![a] }),
                    None => self.status = trf!("{} não tem fórmula para avaliar", cell_name(self.linha, self.coluna)),
                }
            }
            Evento::Avaliar => {
                if let Some(a) = self.avaliar.as_mut().and_then(|e| e.pilha.last_mut()) {
                    a.avancar(&self.workbook);
                }
            }
            Evento::Entrar => {
                let Some(estado) = self.avaliar.as_mut() else { return Task::none() };
                let Some(topo) = estado.pilha.last() else { return Task::none() };
                if let Some((aba, r, c)) = topo.estado(&self.workbook).pode_entrar {
                    let indice = match aba {
                        Some(nome) => self.workbook.nomes().iter().position(|n| n.eq_ignore_ascii_case(&nome)),
                        None => Some(topo.aba),
                    };
                    if let Some(a) = indice.and_then(|i| Avaliacao::nova(&self.workbook, i, r, c)) {
                        estado.pilha.push(a);
                    }
                }
            }
            Evento::Sair => {
                if let Some(e) = self.avaliar.as_mut() {
                    if e.pilha.len() > 1 { e.pilha.pop(); }
                }
            }
            Evento::Reiniciar => {
                if let Some(e) = self.avaliar.as_mut() {
                    e.pilha.truncate(1);
                    if let Some(a) = e.pilha.last_mut() { a.reiniciar(); }
                }
            }
            Evento::Fechar => self.avaliar = None,
        }
        Task::none()
    }

    fn referencia_avaliada(&self, a: &Avaliacao) -> String {
        format!("{}!{}", self.workbook.nomes().get(a.aba).cloned().unwrap_or_default(), cell_name(a.linha, a.coluna))
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    pub(super) fn view_avaliar(&self) -> Element<'_, Msg> {
        match self.dialogo_avaliar() {
            Some(d) => d.painel(),
            None => Space::new(0.0, 0).into(),
        }
    }

    fn dialogo_avaliar(&self) -> Option<super::dialogo_interno::Dialogo<'_, Msg>> {
        let estado = self.avaliar.as_ref()?;
        let topo = estado.pilha.last()?;
        let e = topo.estado(&self.workbook);
        // Caminho de Entrar: Plan1!E8 › Plan1!B1.
        let mut caminho = row(Vec::new()).spacing(6).align(pelicano::Align::Center);
        for (i, a) in estado.pilha.iter().enumerate() {
            if i > 0 { caminho = caminho.push(feather_ui::icones::icone("chevron-right", 13.)); }
            caminho = caminho.push(text(self.referencia_avaliada(a)).size(12.5)
                .color(if i + 1 == estado.pilha.len() { COR_TEXTO() } else { COR_TEXTO_MUTED() }));
        }
        let caracteres: Vec<char> = e.texto.chars().collect();
        let (i, f) = e.proximo.unwrap_or((caracteres.len(), caracteres.len()));
        let pedaco = |de: usize, ate: usize| caracteres[de..ate].iter().collect::<String>();
        let fonte = pelicano::Font::MONO;
        let formula = rich_text([
            span(pedaco(0, i)).font(fonte),
            span(pedaco(i, f)).font(fonte).underline().color(COR_AZUL_ACENTO())
                .background(COR_AZUL_FUNDO()),
            span(pedaco(f, caracteres.len())).font(fonte),
        ]).size(14);
        let caixa = container(formula).padding(12).width(Length::FILL)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
                ..Default::default()
            });
        let mut passos = column(Vec::new()).spacing(3);
        for p in topo.historico.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
            passos = passos.push(text(p.clone()).size(12).font(fonte).color(COR_TEXTO_MUTED()));
        }
        let dica = if e.concluida {
            tr!("Avaliação concluída: este é o valor da célula. Reiniciar volta ao começo.").to_string()
        } else if e.pode_entrar.is_some() {
            tr!("Avaliar calcula o trecho sublinhado; Entrar abre a fórmula da célula sublinhada.").to_string()
        } else {
            tr!("Avaliar calcula o trecho sublinhado.").to_string()
        };
        let corpo = column![
            row![text(tr!("Referência:")).size(12).color(COR_TEXTO_MUTED()), caminho].spacing(8).align(pelicano::Align::Center),
            caixa,
            passos,
            row![feather_ui::icones::icone("info-circle", 15.), text(dica).size(11.5).color(COR_TEXTO_MUTED())]
                .spacing(6).align(pelicano::Align::Center),
        ].spacing(12);
        Some(self.dialogo(tr!("Avaliar fórmula"), corpo)
            .icone("math-function")
            .largura(600.0)
            .ao_fechar(Msg::Avaliar(Evento::Fechar))
            .rodape_esquerda(super::dialogo_interno::dlg::botao_secundario(tr!("Entrar"),
                e.pode_entrar.is_some().then_some(Msg::Avaliar(Evento::Entrar))))
            .rodape_esquerda(super::dialogo_interno::dlg::botao_secundario(tr!("Sair"),
                (estado.pilha.len() > 1).then_some(Msg::Avaliar(Evento::Sair))))
            .secundario(tr!("Reiniciar"), Some(Msg::Avaliar(Evento::Reiniciar)))
            .primario(tr!("Avaliar"), (!e.concluida).then_some(Msg::Avaliar(Evento::Avaliar))))
    }

    pub(super) fn sobrepor_avaliar<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        match self.dialogo_avaliar() {
            Some(d) => d.sobre(base),
            None => base,
        }
    }
    /// Texto do diálogo para o MCP (`sheet_tela`).
    pub(super) fn descrever_avaliar(&self) -> Option<String> {
        let estado = self.avaliar.as_ref()?;
        let topo = estado.pilha.last()?;
        let e = topo.estado(&self.workbook);
        let caminho: Vec<String> = estado.pilha.iter().map(|a| self.referencia_avaliada(a)).collect();
        let sublinhado = e.proximo.map(|(i, f)| e.texto.chars().skip(i).take(f - i).collect::<String>());
        Some(trf!("Diálogo aberto: Avaliar fórmula · {} · {}{}{}", caminho.join(" > "), e.texto,
            sublinhado.map(|s| trf!(" · sublinhado: {s}")).unwrap_or_default(),
            if e.concluida { tr!(" · concluída") } else { "" }))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn avalia_entra_sai_e_reinicia() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "2".into());
        app.workbook.escrever_celula(0, 0, 1, "=A1*10".into());
        app.workbook.escrever_celula(0, 0, 2, "=B1+1".into());
        app.trocar_selecao(0, 2);
        let _ = app.update(Msg::Avaliar(Evento::Abrir));
        let _: Element<'_, Msg> = app.view_avaliar();
        let _ = app.update(Msg::Avaliar(Evento::Entrar));
        assert_eq!(app.avaliar.as_ref().unwrap().pilha.len(), 2, "B1 tem fórmula: dá para entrar");
        let _ = app.update(Msg::Avaliar(Evento::Avaliar));
        assert!(app.descrever_avaliar().unwrap().contains("=2*10"));
        let _ = app.update(Msg::Avaliar(Evento::Sair));
        for _ in 0..5 { let _ = app.update(Msg::Avaliar(Evento::Avaliar)); }
        assert!(app.descrever_avaliar().unwrap().contains("=21 · concluída"), "{:?}", app.descrever_avaliar());
        let _: Element<'_, Msg> = app.view_avaliar();
        let _ = app.update(Msg::Avaliar(Evento::Reiniciar));
        assert!(app.descrever_avaliar().unwrap().contains("=B1+1"));
        let _ = app.update(Msg::Avaliar(Evento::Fechar));
        assert!(app.avaliar.is_none());
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Number(21.), "avaliar não muda a planilha");
        assert!(!app.alterado);
    }

    #[test]
    fn celula_sem_formula_avisa() {
        let mut app = App::default();
        let _ = app.update(Msg::Avaliar(Evento::Abrir));
        assert!(app.avaliar.is_none());
        assert!(app.status.contains("não tem fórmula"));
    }
}
