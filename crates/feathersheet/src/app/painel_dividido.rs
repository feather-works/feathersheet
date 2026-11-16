//! Exibir ▸ Janela ▸ Dividir (#83): dois painéis da pasta, um em cima do outro, cada um com
//! a sua aba e a sua rolagem. Só um painel é o ativo (o estado de sempre: `folha`, rolagem,
//! seleção); o outro guarda o seu aqui e é desenhado só com as células. Qualquer clique ou
//! roda no outro painel troca os dois de papel e repassa a mensagem, que cai na aba certa.
//! Os painéis não mudam de lugar ao trocar: `ativo_embaixo` diz onde está o ativo.
use super::*;

#[derive(Default)]
pub(super) struct PainelDividido {
    aba_id: u32,
    linha_inicio: u32,
    coluna_inicio: u32,
    linha: u32,
    coluna: u32,
    ativo_embaixo: bool,
    /// Régua própria do outro painel: com abas diferentes, uma régua só seria refeita a cada quadro.
    eixos: RefCell<crate::grade::EstadoEixos>,
}

impl App {
    /// Liga (o outro painel começa igual ao atual, como no Excel) ou desliga a divisão.
    pub(super) fn alternar_divisao(&mut self) {
        if self.painel_dividido.take().is_some() {
            self.status = tr!("Divisão removida").into();
            return;
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        self.painel_dividido = Some(PainelDividido {
            aba_id: aba.id,
            linha_inicio: self.linha_inicio,
            coluna_inicio: self.coluna_inicio,
            linha: self.linha,
            coluna: self.coluna,
            ..Default::default()
        });
        self.status = tr!("Janela dividida: clique no outro painel e escolha a aba dele").into();
    }

    /// Mensagem vinda do painel inativo: ele vira o ativo e a mensagem segue.
    pub(super) fn no_outro_painel(&mut self, m: Msg) -> Task<Msg> {
        let Some(p) = self.painel_dividido.as_ref() else { return self.update(m) };
        let destino = self.workbook.indice_do_id(p.aba_id).unwrap_or(self.folha);
        let (li, ci, l, c) = (p.linha_inicio, p.coluna_inicio, p.linha, p.coluna);
        let atual = (self.workbook.aba(self.folha).map_or(p.aba_id, |a| a.id),
            self.linha_inicio, self.coluna_inicio, self.linha, self.coluna);
        let apontando = self.editor_formula.ativo && self.edicao.starts_with('=');
        let troca = if destino != self.folha {
            self.update(Msg::Aba(destino))
        } else {
            if !apontando && !self.confirmar() { return Task::none(); }
            Task::none()
        };
        // Troca recusada (nome de aba em edição, valor inválido): nada muda.
        if self.folha != destino { return troca; }
        let Some(p) = self.painel_dividido.as_mut() else { return troca };
        (p.aba_id, p.linha_inicio, p.coluna_inicio, p.linha, p.coluna) = atual;
        p.ativo_embaixo = !p.ativo_embaixo;
        self.eixos_grade.swap(&p.eixos);
        self.linha_inicio = li;
        self.coluna_inicio = ci;
        // Com fórmula aberta o clique aponta a referência: a célula ativa continua a de origem.
        if !apontando { self.trocar_selecao_por_clique(l, c); }
        let seguinte = self.update(m);
        Task::batch([troca, seguinte])
    }

    /// A grade ativa e, com a janela dividida, o outro painel (só células) com a divisória.
    pub(super) fn com_painel_dividido<'a>(&'a self, ativo: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(p) = &self.painel_dividido else { return ativo };
        let Some(indice) = self.workbook.indice_do_id(p.aba_id) else { return ativo };
        let (congelar_linhas, congelar_colunas) = self.congelados.get(&p.aba_id).copied().unwrap_or((0, 0));
        let base = self.grade();
        // shortcut: o outro painel não mostra imagens, gráficos, tinta nem botões de macro;
        // aparecem ao clicar nele. Desenhar as camadas lá quando alguém pedir.
        let outro = Grade {
            eixos: &p.eixos,
            aba: indice,
            linha_inicio: p.linha_inicio,
            coluna_inicio: p.coluna_inicio,
            congelar_linhas,
            congelar_colunas,
            selecionada: (p.linha, p.coluna),
            area: Rect::new(p.linha, p.coluna, p.linha, p.coluna),
            linhas_grade: self.linhas_grade && !self.grade_oculta.contains(&p.aba_id),
            rascunho: None,
            refs_formula: Vec::new(),
            lista_aberta: false,
            opcoes_coluna: None,
            areas_extras: &[],
            impressao: None,
            copiada: None,
            setas: &[],
            comentarios: Default::default(),
            circulos_validacao: &[],
            ..base
        };
        let outro: Element<'a, Msg> = Element::from(crate::compat::canvas(outro)
            .width(Length::FILL).height(Length::FILL))
            .map(|m| Msg::NoOutroPainel(Box::new(m)));
        let divisoria = container(Space::new(Length::FILL, Length::Fixed(4.0)))
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_TEXTO_MUTED().alpha(0.45))),
                ..Default::default()
            });
        let (cima, baixo) = if p.ativo_embaixo { (outro, ativo) } else { (ativo, outro) };
        pelicano::column![cima, divisoria, baixo].width(Length::FILL).height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn clicar_no_outro_painel_troca_aba_e_rolagem() {
        let mut app = App::default();
        let _ = app.update(Msg::AdicionarAba);
        let _ = app.update(Msg::Aba(0));
        app.linha_inicio = 40;
        app.trocar_selecao_por_clique(45, 2);
        app.alternar_divisao();
        // O painel de cima vai para a aba 2, linha 1.
        let _ = app.update(Msg::Aba(1));
        app.linha_inicio = 0;
        assert_eq!(app.folha, 1);
        // Clique no painel de baixo (aba 1, rolado na linha 40): ele vira o ativo.
        let _ = app.update(Msg::NoOutroPainel(Box::new(Msg::Selecionar(46, 3))));
        assert_eq!(app.folha, 0);
        assert_eq!(app.linha_inicio, 40);
        assert_eq!((app.linha, app.coluna), (46, 3));
        let p = app.painel_dividido.as_ref().unwrap();
        assert!(p.ativo_embaixo);
        assert_eq!((app.workbook.indice_do_id(p.aba_id), p.linha_inicio), (Some(1), 0));
        app.alternar_divisao();
        assert!(app.painel_dividido.is_none());
    }
}
