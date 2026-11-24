//! Diálogos "Inserir" e "Excluir" células do Excel (Página Inicial →
//! Inserir ▾ / Excluir ▾, botão direito, Ctrl++ e Ctrl+-): deslocar as
//! células da seleção ou mexer na linha/coluna inteira. A regra do
//! deslocamento mora no motor (`planilha::deslocar_celulas`); aqui ficam a
//! escolha e as recusas que dependem dos metadados da interface (mesclas e
//! tabelas no caminho).
use super::*;
use feathersheet_core::celulas::refs::{col_name, Rect};
use feathersheet_core::planilha::deslocar_celulas::{bloco_deslocado, Deslocamento};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcao { Vertical, Horizontal, Linhas, Colunas }

#[derive(Debug, Clone)]
pub enum Evento { Abrir(bool), Opcao(Opcao), Aplicar, Fechar }

#[derive(Debug, Clone)]
pub(super) struct Estado {
    /// `true` = Inserir; `false` = Excluir.
    pub inserir: bool,
    pub opcao: Opcao,
}

fn rotulo(inserir: bool, opcao: Opcao) -> &'static str {
    match (inserir, opcao) {
        (true, Opcao::Vertical) => tr!("Deslocar células para baixo"),
        (true, Opcao::Horizontal) => tr!("Deslocar células para a direita"),
        (false, Opcao::Vertical) => tr!("Deslocar células para cima"),
        (false, Opcao::Horizontal) => tr!("Deslocar células para a esquerda"),
        (_, Opcao::Linhas) => tr!("Linha inteira"),
        (_, Opcao::Colunas) => tr!("Coluna inteira"),
    }
}

fn cruzam(a: &Rect, b: &Rect) -> bool {
    a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1
}

/// O que [`aplicar_trechos`] fez: quantos trechos, se o motor recusou um, e os avisos de tabela e mescla.
pub(super) struct TrechosAplicados {
    pub feitos: usize,
    pub falhou: bool,
    pub tabela: Result<(), String>,
    pub mescla: Result<(), String>,
}

/// Insere/exclui cada trecho `(início, quantas)`, de baixo para cima (os de cima não mudam de lugar),
/// com tabelas, mesclas e minigráficos acompanhando. Só a pasta: roda na janela ou na outra thread.
pub(super) fn aplicar_trechos(w: &mut Workbook, folha: usize, coluna: bool, inserir: bool, trechos: &[(u32, u32)],
    andou: &dyn Fn(usize)) -> TrechosAplicados {
    let mut r = TrechosAplicados { feitos: 0, falhou: false, tabela: Ok(()), mescla: Ok(()) };
    for &(ini, n) in trechos.iter().rev() {
        let feito = match (coluna, inserir) {
            (false, true) => w.inserir_linhas(folha, ini, n),
            (false, false) => w.excluir_linhas(folha, ini, n),
            (true, true) => w.inserir_colunas(folha, ini, n),
            (true, false) => w.excluir_colunas(folha, ini, n),
        };
        if feito.is_none() { r.falhou = true; return r; }
        let t = tabelas::refluir(w, folha, coluna, ini, n, !inserir);
        let m = mesclas::refluir(w, folha, coluna, ini, n, !inserir);
        super::minigraficos_ui::refluir_minigraficos_na_pasta(w, folha, coluna, ini, n, !inserir);
        if r.tabela.is_ok() { r.tabela = t; }
        if r.mescla.is_ok() { r.mescla = m; }
        r.feitos += 1;
        andou(r.feitos);
    }
    r
}

impl App {
    fn abrir_celulas(&mut self, inserir: bool) {
        if !self.confirmar() { return; }
        // Linhas ou colunas inteiras selecionadas: o Excel não pergunta.
        if self.selecao_e_linhas_inteiras() || self.selecao_e_colunas_inteiras() {
            let opcao = if self.selecao_e_linhas_inteiras() { Opcao::Linhas } else { Opcao::Colunas };
            self.aplicar_celulas(inserir, opcao);
            return;
        }
        // Seleção mais larga que alta desloca para baixo/cima, como o Excel sugere.
        let a = self.area_selecionada();
        let opcao = if a.c1 - a.c0 > a.r1 - a.r0 { Opcao::Vertical } else if inserir { Opcao::Vertical } else { Opcao::Horizontal };
        self.edicao_menu = None;
        self.contexto = None;
        self.celulas_modal = Some(Estado { inserir, opcao });
    }

    /// Faz a operação escolhida na seleção. Um Ctrl+Z desfaz o deslocamento;
    /// linha e coluna inteiras seguem o Inserir/Excluir linha da faixa.
    pub(super) fn aplicar_celulas(&mut self, inserir: bool, opcao: Opcao) {
        self.celulas_modal = None;
        let a = self.area_selecionada();
        let (coluna, at, quantas) = match opcao {
            Opcao::Linhas => (false, a.r0, (a.r1 - a.r0 + 1).min(MAX_ROWS - a.r0)),
            Opcao::Colunas => (true, a.c0, (a.c1 - a.c0 + 1).min(MAX_COLS - a.c0)),
            Opcao::Vertical | Opcao::Horizontal => {
                let sentido = if opcao == Opcao::Vertical { Deslocamento::Vertical } else { Deslocamento::Horizontal };
                self.status = match self.deslocar_celulas(inserir, sentido, &a) {
                    Ok(()) => trf!("Células {} ({})", if inserir { "inseridas" } else { tr!("excluídas") },
                        rotulo(inserir, opcao).trim_start_matches("Deslocar células ")),
                    Err(e) => e,
                };
                return;
            }
        };
        // Excluir linhas com filtro ligado apaga só as visíveis, como o Excel numa lista filtrada: cada faixa
        // seguida de linhas que aparecem, de baixo para cima (as de cima não mudam de lugar).
        let trechos: Vec<(u32, u32)> = match (coluna, inserir, self.workbook.aba(self.folha)) {
            (false, false, Some(aba)) => {
                let pular = tabelas::puladas_pelo_filtro(&aba.sheet);
                let mut t: Vec<(u32, u32)> = Vec::new();
                for r in (at..at + quantas).filter(|&r| !pular(r)) {
                    match t.last_mut() {
                        Some((ini, n)) if *ini + *n == r => *n += 1,
                        _ => t.push((r, 1)),
                    }
                }
                t
            }
            _ => vec![(at, quantas)],
        };
        if trechos.is_empty() {
            self.status = tr!("Todas as linhas selecionadas estão escondidas pelo filtro: nada a excluir.").into();
            return;
        }
        // Muitos trechos (lista filtrada grande): a exclusão vai para outra thread, com a pasta, e a janela
        // mostra o andamento. Medido em 23/11/2026: 5000 trechos em 10 mil linhas com fórmulas = 2,5 s na janela.
        // shortcut: um `excluir_linhas` por trecho (cada um corrige as fórmulas da pasta toda); cortar várias
        // faixas numa operação só do motor (`mover_estrutura` com vários cortes) se o tempo em fundo incomodar.
        let total: u32 = trechos.iter().map(|&(_, n)| n).sum();
        let foto = self.foto_antes_da_estrutura();
        let status_ok = trf!("{total} linhas visíveis excluídas (as escondidas pelo filtro ficaram)");
        if !coluna && !inserir && trechos.len() >= super::calculo_em_fundo::TRECHOS_EM_FUNDO {
            self.excluir_trechos_em_fundo(trechos, foto, status_ok);
            return;
        }
        let r = aplicar_trechos(&mut self.workbook, self.folha, coluna, inserir, &trechos, &|_| {});
        if r.falhou {
            self.status = tr!("Não foi possível alterar a estrutura").into();
            // Trechos de baixo já saíram: a foto fica no Ctrl+Z para voltar a pasta inteira.
            self.guardar_foto_da_estrutura(foto, r.feitos > 0);
            return;
        }
        let (tabela, mescla) = (r.tabela, r.mescla);
        // Tabelas e mesclas reajustadas também guardam o "antes": um Ctrl+Z
        // as poria nas coordenadas velhas, com as células já deslocadas (#46).
        if let Some(aba) = self.workbook.aba_mut(self.folha) { aba.sheet.esquecer_historico(); }
        self.alterado = true;
        self.trocar_selecao(self.linha, self.coluna);
        let nome = if coluna { format!("{}:{}", col_name(at), col_name(at + quantas - 1)) }
            else { format!("{}:{}", at + 1, at + quantas) };
        self.status = match (tabela, mescla) {
            (Ok(()), Ok(())) if total != quantas => status_ok,
            (Ok(()), Ok(())) => trf!("{} {nome} {}", if coluna { tr!("Colunas") } else { tr!("Linhas") },
                if inserir { "inseridas" } else { tr!("excluídas") }),
            (Err(e), _) | (_, Err(e)) => trf!("Estrutura alterada; revise: {e}"),
        };
        self.guardar_foto_da_estrutura(foto, true);
    }

    fn deslocar_celulas(&mut self, inserir: bool, sentido: Deslocamento, a: &Rect) -> Result<(), String> {
        let sheet = &self.workbook.aba(self.folha).ok_or(tr!("Aba ausente."))?.sheet;
        // O motor não move mesclas nem tabelas: recusar como o Excel quando
        // a área que anda (ou some) passa por uma delas.
        let anda = bloco_deslocado(a, sentido, inserir);
        let afetadas: Vec<Rect> = [Some(a.clone()), anda].into_iter().flatten().collect();
        if mesclas::ler(sheet).iter().any(|m| afetadas.iter().any(|x| cruzam(m, x))) {
            return Err(tr!("Esta operação deslocaria parte de células mescladas. Desmescle ou selecione linhas/colunas inteiras.").into());
        }
        if tabelas::areas(sheet).iter().any(|t| afetadas.iter().any(|x| cruzam(t, x))) {
            return Err(tr!("Esta operação deslocaria parte de uma tabela. Use linha ou coluna inteira.").into());
        }
        let mudanca = if inserir {
            self.workbook.inserir_celulas(self.folha, a.clone(), sentido)?
        } else {
            self.workbook.excluir_celulas(self.folha, a.clone(), sentido)?
        };
        let _ = mudanca;
        self.alterado = true;
        self.trocar_selecao(self.linha, self.coluna);
        Ok(())
    }

    pub(super) fn evento_celulas(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir(inserir) => self.abrir_celulas(inserir),
            Evento::Opcao(o) => if let Some(m) = &mut self.celulas_modal { m.opcao = o; },
            Evento::Aplicar => if let Some(m) = self.celulas_modal.clone() { self.aplicar_celulas(m.inserir, m.opcao); },
            Evento::Fechar => self.celulas_modal = None,
        }
        Task::none()
    }

    fn view_celulas(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let s = self.celulas_modal.as_ref().expect("diálogo de células aberto");
        let mut opcoes = pelicano::column(Vec::<Element<_>>::new()).spacing(9);
        for o in [Opcao::Vertical, Opcao::Horizontal, Opcao::Linhas, Opcao::Colunas] {
            opcoes = opcoes.push(pelicano::radio(rotulo(s.inserir, o), o, Some(s.opcao),
                |o| Msg::Celulas(Evento::Opcao(o))).size(13));
            // shortcut: o pelicano põe 8 px entre a bolinha e o rótulo (o original pedia 9), aceitar até o Toggle ganhar `spacing`.
        }
        self.dialogo(if s.inserir { tr!("Inserir células") } else { tr!("Excluir células") }, opcoes)
            .icone(if s.inserir { "plus" } else { "trash" })
            .subtitulo(if s.inserir { tr!("As fórmulas que leem as células deslocadas acompanham.") }
                else { tr!("Quem lia uma célula excluída passa a mostrar #REF!.") })
            .largura(420.0)
            .ao_fechar(Msg::Celulas(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(Msg::Celulas(Evento::Fechar)))
            .primario("OK", Some(Msg::Celulas(Evento::Aplicar)))
    }

    pub(super) fn sobrepor_celulas<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.celulas_modal.is_none() { return base; }
        self.view_celulas().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn raw(app: &App, r: u32, c: u32) -> String { app.workbook.aba(0).unwrap().sheet.get_raw(r, c) }

    #[test]
    fn inserir_para_baixo_pelo_dialogo_e_um_desfazer() {
        let mut app = App::default();
        for (r, t) in [(0u32, "a"), (1, "b"), (2, "c")] { app.workbook.escrever_celula(0, r, 0, t.into()); }
        app.workbook.escrever_celula(0, 1, 1, "fica".into());
        app.workbook.escrever_celula(0, 0, 3, "=A2".into());
        let _ = app.update(Msg::Selecionar(1, 0));
        let _ = app.update(Msg::Celulas(Evento::Abrir(true)));
        assert!(app.celulas_modal.is_some());
        let _ = app.update(Msg::Celulas(Evento::Opcao(Opcao::Vertical)));
        let _ = app.update(Msg::Celulas(Evento::Aplicar));
        assert!(app.celulas_modal.is_none());
        assert_eq!((raw(&app, 1, 0), raw(&app, 2, 0), raw(&app, 1, 1)), ("".into(), "b".into(), "fica".into()));
        assert_eq!(raw(&app, 0, 3), "=A3");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(raw(&app, 1, 0), "b");
    }

    #[test]
    fn excluir_para_a_esquerda_e_recusa_com_mescla_no_caminho() {
        let mut app = App::default();
        for (c, t) in [(0u32, "x"), (1, "y"), (2, "z")] { app.workbook.escrever_celula(0, 0, c, t.into()); }
        let _ = app.update(Msg::Selecionar(0, 1));
        app.aplicar_celulas(false, Opcao::Horizontal);
        assert_eq!((raw(&app, 0, 1), raw(&app, 0, 2)), ("z".into(), "".into()));
        app.workbook.editar(0, |s| { mesclas::mesclar(s, Rect::new(0, 3, 0, 4)); });
        app.aplicar_celulas(true, Opcao::Horizontal);
        assert!(app.status.contains("mescladas"), "{}", app.status);
        assert_eq!(raw(&app, 0, 1), "z", "nada mudou");
    }

    #[test]
    fn linha_inteira_pelo_dialogo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "1".into());
        app.workbook.escrever_celula(0, 1, 0, "2".into());
        let _ = app.update(Msg::Selecionar(0, 0));
        app.aplicar_celulas(true, Opcao::Linhas);
        assert_eq!((raw(&app, 0, 0), raw(&app, 1, 0)), ("".into(), "1".into()));
    }
}
