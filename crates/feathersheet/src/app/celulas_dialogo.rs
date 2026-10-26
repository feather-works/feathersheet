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
        let feito = match (coluna, inserir) {
            (false, true) => self.workbook.inserir_linhas(self.folha, at, quantas),
            (false, false) => self.workbook.excluir_linhas(self.folha, at, quantas),
            (true, true) => self.workbook.inserir_colunas(self.folha, at, quantas),
            (true, false) => self.workbook.excluir_colunas(self.folha, at, quantas),
        };
        if feito.is_none() {
            self.status = tr!("Não foi possível alterar a estrutura").into();
            return;
        }
        let tabela = tabelas::refluir(&mut self.workbook, self.folha, coluna, at, quantas, !inserir);
        let mescla = mesclas::refluir(&mut self.workbook, self.folha, coluna, at, quantas, !inserir);
        self.alterado = true;
        self.trocar_selecao(self.linha, self.coluna);
        let nome = if coluna { format!("{}:{}", col_name(at), col_name(at + quantas - 1)) }
            else { format!("{}:{}", at + 1, at + quantas) };
        self.status = match (tabela, mescla) {
            (Ok(()), Ok(())) => trf!("{} {nome} {}", if coluna { tr!("Colunas") } else { tr!("Linhas") },
                if inserir { "inseridas" } else { tr!("excluídas") }),
            (Err(e), _) | (_, Err(e)) => trf!("Estrutura alterada; revise: {e}"),
        };
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
