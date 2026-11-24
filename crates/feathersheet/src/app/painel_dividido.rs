//! Exibir ▸ Janela ▸ Dividir (#83): dois painéis da pasta, um em cima do outro ou lado a lado, ou quatro em
//! grade 2×2, cada um com a sua aba e a sua rolagem. Só um painel é o ativo (o estado de sempre: `folha`,
//! rolagem, seleção); os outros guardam o seu aqui e são desenhados com as células, imagens, formas,
//! gráficos, tinta e botões de macro, sem seleção. Qualquer clique ou roda num outro painel troca os dois
//! de papel e repassa a mensagem, que cai na aba certa. Os painéis não mudam de lugar ao trocar: cada um
//! tem a sua posição (`lugar`), e o ativo, a dele (`lugar_ativo`).
use super::*;

/// Como a janela se divide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divisao {
    /// Um em cima do outro.
    Horizontal,
    LadoALado,
    /// Grade 2×2.
    Quatro,
}

impl Divisao {
    fn lugares(self) -> usize {
        if self == Divisao::Quatro { 4 } else { 2 }
    }
}

/// Um painel que não é o ativo (também a vista da Nova janela).
#[derive(Default)]
pub(super) struct Painel {
    pub(super) aba_id: u32,
    pub(super) linha_inicio: u32,
    pub(super) coluna_inicio: u32,
    pub(super) linha: u32,
    pub(super) coluna: u32,
    /// Posição na janela: 0 e 1 (cima/baixo ou esquerda/direita); com quatro, 0 1 em cima e 2 3 embaixo.
    lugar: usize,
    /// Régua própria do painel: com abas diferentes, uma régua só seria refeita a cada quadro.
    eixos: RefCell<crate::grade::EstadoEixos>,
}

pub(super) struct PainelDividido {
    modo: Divisao,
    lugar_ativo: usize,
    outros: Vec<Painel>,
}

/// A aba e a rolagem que uma camada sobre a grade desenha: a do painel ativo ou a de outro.
#[derive(Clone, Copy)]
pub(super) struct PainelVista {
    pub(super) folha: usize,
    pub(super) linha_inicio: u32,
    pub(super) coluna_inicio: u32,
    /// O outro painel só desenha: sem alças de seleção nem edição no lugar.
    pub(super) ativo: bool,
}

impl PainelDividido {
    /// Os outros ficam com as posições livres, na ordem; sobram ou faltam painéis conforme o modo.
    fn arrumar(&mut self, novo: Painel) {
        let n = self.modo.lugares();
        self.lugar_ativo = self.lugar_ativo.min(n - 1);
        self.outros.truncate(n - 1);
        while self.outros.len() < n - 1 {
            self.outros.push(Painel { eixos: Default::default(), ..novo.copia() });
        }
        let livres = (0..n).filter(|l| *l != self.lugar_ativo);
        for (p, l) in self.outros.iter_mut().zip(livres) { p.lugar = l; }
    }
}

impl Painel {
    pub(super) fn de(aba_id: u32, linha_inicio: u32, coluna_inicio: u32, linha: u32, coluna: u32) -> Painel {
        Painel { aba_id, linha_inicio, coluna_inicio, linha, coluna, ..Default::default() }
    }

    fn copia(&self) -> Painel {
        Painel { aba_id: self.aba_id, linha_inicio: self.linha_inicio, coluna_inicio: self.coluna_inicio,
            linha: self.linha, coluna: self.coluna, lugar: self.lugar, eixos: Default::default() }
    }
}

impl App {
    pub(super) fn painel_ativo(&self) -> PainelVista {
        PainelVista { folha: self.folha, linha_inicio: self.linha_inicio, coluna_inicio: self.coluna_inicio, ativo: true }
    }

    /// Dividida desse jeito (o botão da faixa fica aceso).
    pub(super) fn janela_dividida(&self, modo: Divisao) -> bool {
        self.painel_dividido.as_ref().is_some_and(|p| p.modo == modo)
    }

    /// Liga (os outros painéis começam iguais ao atual, como no Excel), muda o jeito ou desliga (o mesmo botão).
    pub(super) fn alternar_divisao(&mut self, modo: Divisao) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let atual = Painel { aba_id: aba.id, linha_inicio: self.linha_inicio, coluna_inicio: self.coluna_inicio,
            linha: self.linha, coluna: self.coluna, ..Default::default() };
        if let Some(p) = self.painel_dividido.as_mut() {
            if p.modo == modo {
                self.painel_dividido = None;
                self.status = tr!("Divisão removida").into();
                return;
            }
            p.modo = modo;
            p.arrumar(atual);
            return;
        }
        let mut p = PainelDividido { modo, lugar_ativo: 0, outros: Vec::new() };
        p.arrumar(atual);
        self.painel_dividido = Some(p);
        self.status = tr!("Janela dividida: clique no outro painel e escolha a aba dele").into();
    }

    /// Mensagem vinda do painel `i` (não ativo): ele vira o ativo e a mensagem segue.
    pub(super) fn no_outro_painel(&mut self, i: usize, m: Msg) -> Task<Msg> {
        let Some(p) = self.painel_dividido.as_ref().and_then(|d| d.outros.get(i)) else { return self.update(m) };
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
        let Some(d) = self.painel_dividido.as_mut() else { return troca };
        let Some(p) = d.outros.get_mut(i) else { return troca };
        (p.aba_id, p.linha_inicio, p.coluna_inicio, p.linha, p.coluna) = atual;
        std::mem::swap(&mut p.lugar, &mut d.lugar_ativo);
        self.eixos_grade.swap(&p.eixos);
        self.linha_inicio = li;
        self.coluna_inicio = ci;
        // Com fórmula aberta o clique aponta a referência: a célula ativa continua a de origem.
        if !apontando { self.trocar_selecao_por_clique(l, c); }
        let seguinte = self.update(m);
        Task::batch([troca, seguinte])
    }

    /// A grade ativa e, com a janela dividida, os outros painéis com as divisórias.
    pub(super) fn com_painel_dividido<'a>(&'a self, ativo: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(d) = &self.painel_dividido else { return ativo };
        let mut lugares: Vec<Option<Element<'a, Msg>>> = (0..d.modo.lugares()).map(|_| None).collect();
        lugares[d.lugar_ativo] = Some(ativo);
        for (i, p) in d.outros.iter().enumerate() {
            let Some(indice) = self.workbook.indice_do_id(p.aba_id) else { continue };
            lugares[p.lugar] = Some(self.desenhar_outro_painel(p, indice).map(move |m| Msg::NoOutroPainel(i, Box::new(m))));
        }
        let mut lugares = lugares.into_iter().map(|e| e.unwrap_or_else(|| Space::new(Length::FILL, Length::FILL).into()));
        let divisoria = |deitada: bool| {
            let (largura, altura) = if deitada { (Length::FILL, Length::Fixed(4.0)) } else { (Length::Fixed(4.0), Length::FILL) };
            container(Space::new(largura, altura)).style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_TEXTO_MUTED().alpha(0.45))),
                ..Default::default()
            })
        };
        let mut proximo = || lugares.next().unwrap_or_else(|| Space::new(0.0, 0.0).into());
        match d.modo {
            Divisao::Horizontal => pelicano::column![proximo(), divisoria(true), proximo()].width(Length::FILL).height(Length::FILL).into(),
            Divisao::LadoALado => row![proximo(), divisoria(false), proximo()].width(Length::FILL).height(Length::FILL).into(),
            Divisao::Quatro => {
                let cima = row![proximo(), divisoria(false), proximo()].width(Length::FILL).height(Length::FILL);
                let baixo = row![proximo(), divisoria(false), proximo()].width(Length::FILL).height(Length::FILL);
                pelicano::column![cima, divisoria(true), baixo].width(Length::FILL).height(Length::FILL).into()
            }
        }
    }

    /// Um painel que não é o ativo: as células e as mesmas camadas da grade ativa, na ordem dela (a tinta por cima).
    pub(super) fn desenhar_outro_painel<'a>(&'a self, p: &'a Painel, indice: usize) -> Element<'a, Msg> {
        let (congelar_linhas, congelar_colunas) = self.congelados.get(&p.aba_id).copied().unwrap_or((0, 0));
        let base = self.grade();
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
            mostrar_zeros: !self.zeros_ocultos.contains(&p.aba_id),
            limite: self.limite_da_grade(indice, p.linha, p.coluna),
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
        let celulas: Element<'a, Msg> = crate::compat::canvas(outro).width(Length::FILL).height(Length::FILL).into();
        let vista = PainelVista { folha: indice, linha_inicio: p.linha_inicio, coluna_inicio: p.coluna_inicio, ativo: false };
        self.tinta_sobre(self.botoes_macro_sobre(self.objetos_sobre(self.imagens_sobre(celulas, vista), vista), vista), vista)
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
        app.alternar_divisao(Divisao::Horizontal);
        // O painel de cima (o ativo) vai para a aba 2, linha 1.
        let _ = app.update(Msg::Aba(1));
        app.linha_inicio = 0;
        assert_eq!(app.folha, 1);
        // Clique no painel de baixo (aba 1, rolado na linha 40): ele vira o ativo e fica embaixo.
        let _ = app.update(Msg::NoOutroPainel(0, Box::new(Msg::Selecionar(46, 3))));
        assert_eq!(app.folha, 0);
        assert_eq!(app.linha_inicio, 40);
        assert_eq!((app.linha, app.coluna), (46, 3));
        let d = app.painel_dividido.as_ref().unwrap();
        assert_eq!((d.lugar_ativo, d.outros[0].lugar), (1, 0));
        assert_eq!((app.workbook.indice_do_id(d.outros[0].aba_id), d.outros[0].linha_inicio), (Some(1), 0));
        // Lado a lado muda só o sentido; o mesmo botão de novo desfaz.
        app.alternar_divisao(Divisao::LadoALado);
        assert!(app.janela_dividida(Divisao::LadoALado) && !app.janela_dividida(Divisao::Horizontal));
        app.alternar_divisao(Divisao::LadoALado);
        assert!(app.painel_dividido.is_none());
    }

    /// Quatro painéis: três outros nos lugares livres; clicar no de baixo à direita troca os lugares com o ativo;
    /// voltar para dois fica só com um outro; cada painel guarda a sua aba.
    #[test]
    fn dividir_em_quatro_e_voltar_para_dois() {
        let mut app = App::default();
        let _ = app.update(Msg::AdicionarAba);
        let _ = app.update(Msg::Aba(0));
        app.alternar_divisao(Divisao::Quatro);
        let d = app.painel_dividido.as_ref().unwrap();
        assert_eq!(d.outros.iter().map(|p| p.lugar).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(d.lugar_ativo, 0);
        let _: Element<'_, Msg> = app.com_painel_dividido(Space::new(0.0, 0.0).into());
        // O painel de baixo à direita (o terceiro outro) passa a ser o ativo; ele vai para a aba 2.
        let _ = app.update(Msg::NoOutroPainel(2, Box::new(Msg::Selecionar(5, 1))));
        let _ = app.update(Msg::Aba(1));
        let d = app.painel_dividido.as_ref().unwrap();
        assert_eq!((d.lugar_ativo, d.outros[2].lugar), (3, 0));
        assert_eq!(app.folha, 1);
        // De volta a dois: o ativo (que estava no lugar 3) fica no último lugar; sobra um outro.
        app.alternar_divisao(Divisao::Horizontal);
        let d = app.painel_dividido.as_ref().unwrap();
        assert_eq!((d.outros.len(), d.lugar_ativo, d.outros[0].lugar), (1, 1, 0));
        assert_eq!(app.folha, 1, "o ativo continua na aba dele");
        app.alternar_divisao(Divisao::Horizontal);
        assert!(app.painel_dividido.is_none());
    }
}
