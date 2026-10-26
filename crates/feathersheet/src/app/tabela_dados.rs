//! Janela "Tabela de dados" (Dados → Previsão), como no Excel: célula de
//! entrada da linha e/ou da coluna para a tabela selecionada. As contas são
//! do motor (`operacoes::tabela_dados`); o resultado entra como valores numa
//! transação (um Ctrl+Z). Diferente do Excel, não é uma fórmula viva: o
//! botão "Atualizar tabela" refaz a última tabela desta sessão.
use super::*;
use feathersheet_core::celulas::refs::parse_cell_ref;
use feathersheet_core::operacoes::tabela_dados::calcular;
use pelicano::column;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { Linha, Coluna }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Linha(String),
    Coluna(String),
    Confirmar,
    /// Refaz a última tabela com os mesmos parâmetros.
    Atualizar,
    Fechar,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub area: Rect,
    pub aba: usize,
    pub linha: String,
    pub coluna: String,
    pub campo: Campo,
    pub erro: Option<String>,
}

/// O que a última tabela usou, para "Atualizar tabela".
#[derive(Debug, Clone)]
pub(crate) struct Ultima {
    aba: usize,
    area: Rect,
    linha: Option<(u32, u32)>,
    coluna: Option<(u32, u32)>,
}

impl App {
    pub(super) fn evento_tabela_dados(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if !self.confirmar() { return Task::none(); }
                if self.tem_varias_areas() {
                    self.status = tr!("Tabela de dados não funciona com várias áreas selecionadas").into();
                    return Task::none();
                }
                self.tabela_dados = Some(Estado {
                    area: self.area_selecionada(), aba: self.folha,
                    linha: String::new(), coluna: String::new(),
                    campo: Campo::Linha, erro: None,
                });
            }
            Evento::Linha(t) => if let Some(e) = &mut self.tabela_dados {
                e.linha = t; e.campo = Campo::Linha; e.erro = None;
            },
            Evento::Coluna(t) => if let Some(e) = &mut self.tabela_dados {
                e.coluna = t; e.campo = Campo::Coluna; e.erro = None;
            },
            Evento::Confirmar => {
                let Some(e) = self.tabela_dados.clone() else { return Task::none() };
                let celula = |t: &str| -> Result<Option<(u32, u32)>, String> {
                    let t = t.trim();
                    if t.is_empty() { return Ok(None); }
                    parse_cell_ref(&t.replace('$', "").to_uppercase()).map(Some)
                        .ok_or(trf!("\"{t}\" não é o endereço de uma célula."))
                };
                let ultima = celula(&e.linha).and_then(|l| celula(&e.coluna).map(|c| Ultima {
                    aba: e.aba, area: e.area.clone(), linha: l, coluna: c }));
                match ultima.and_then(|u| self.rodar_tabela_dados(&u).map(|()| u)) {
                    Ok(u) => { self.tabela_dados_ultima = Some(u); self.tabela_dados = None; }
                    Err(m) => if let Some(e) = &mut self.tabela_dados { e.erro = Some(m); },
                }
            }
            Evento::Atualizar => {
                let Some(u) = self.tabela_dados_ultima.clone() else { return Task::none() };
                if let Err(m) = self.rodar_tabela_dados(&u) { self.status = m; }
            }
            Evento::Fechar => self.tabela_dados = None,
        }
        Task::none()
    }

    /// Clique na grade com a janela aberta: o endereço vai para o campo ativo.
    pub(super) fn tabela_dados_clique(&mut self, r: u32, c: u32) {
        let Some(e) = self.tabela_dados.as_mut() else { return };
        if self.folha != e.aba { return; }
        let endereco = cell_name(r, c);
        match e.campo {
            Campo::Linha => e.linha = endereco,
            Campo::Coluna => e.coluna = endereco,
        }
        e.erro = None;
    }

    fn rodar_tabela_dados(&mut self, u: &Ultima) -> Result<(), String> {
        let resultado = calcular(&mut self.workbook, u.aba, &u.area, u.linha, u.coluna)?;
        let quantas = resultado.len();
        self.workbook.editar(u.aba, |s| {
            s.abrir_transacao();
            for (r, c, t) in &resultado { s.set_cell(*r, *c, t); }
            s.fechar_transacao();
        });
        self.alterado = true;
        self.status = trf!("Tabela de dados: {quantas} resultado(s) gravado(s) como valores");
        Ok(())
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    fn view_tabela_dados(&self) -> Element<'_, Msg> {
        self.dialogo_tabela_dados().painel()
    }

    fn dialogo_tabela_dados(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.tabela_dados.as_ref().expect("janela aberta");
        let ev = Msg::TabelaDados;
        let campo = |rotulo: &'static str, valor: &str, ativo: bool, f: fn(String) -> Evento| row![
            text(rotulo).size(12.5).width(Length::Fixed(196.0)),
            text_input(tr!("ex.: B3"), valor.to_owned()).on_input(move |t| Msg::TabelaDados(f(t)))
                .on_submit(Msg::TabelaDados(Evento::Confirmar))
                .size(13).padding([5, 8]).width(Length::FILL)
                .style(move |tema, status| {
                    let mut s = pelicano::input_default(tema, status);
                    if ativo { s.border.color = COR_AZUL_ACENTO(); }
                    s
                }),
        ].spacing(8).align(pelicano::Align::Center);
        let mut corpo = column![
            campo(tr!("Célula de entrada da linha:"), &e.linha, e.campo == Campo::Linha, Evento::Linha),
            campo(tr!("Célula de entrada da coluna:"), &e.coluna, e.campo == Campo::Coluna, Evento::Coluna),
            row![
                feather_ui::icones::icone("info-circle", 14.0),
                text(tr!("Clique na planilha para preencher o campo destacado. Os resultados \
                      entram como valores; use \"Atualizar tabela\" depois de mudar os dados."))
                    .size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(6),
        ].spacing(14);
        if let Some(erro) = &e.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        // Não modal, à direita: os cliques na planilha preenchem os campos.
        self.dialogo(tr!("Tabela de dados"), corpo)
            .icone("table")
            .subtitulo(trf!("Tabela {}:{}", cell_name(e.area.r0, e.area.c0), cell_name(e.area.r1, e.area.c1)))
            .topo(14.0)
            .altura_maxima((self.tamanho_principal.height - self.topo_dialogo() - 28.0).max(300.0))
            .largura(470.0)
            .nao_modal()
            .a_direita()
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", Some(ev(Evento::Confirmar)))
    }

    pub(super) fn sobrepor_tabela_dados<'a>(&'a self, centro: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.tabela_dados.is_none() { return centro; }
        self.dialogo_tabela_dados().sobre(centro)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::Value;

    #[test]
    fn grava_resultados_num_desfazer_e_atualizar_refaz() {
        let mut app = App::default();
        for ((r, c), t) in [((0, 0), "5"), ((0, 1), "=A1*100"), ((1, 3), "1"),
            ((2, 3), "2"), ((0, 4), "=B1")] {
            app.workbook.escrever_celula(0, r, c, t.into());
        }
        let _ = app.update(Msg::IrPara("D1:E3".into()));
        let _ = app.update(Msg::TabelaDados(Evento::Abrir));
        let _ = app.update(Msg::TabelaDados(Evento::Coluna("A1".into())));
        let _: Element<'_, Msg> = app.view_tabela_dados();
        let _ = app.update(Msg::TabelaDados(Evento::Confirmar));
        assert!(app.tabela_dados.is_none());
        assert_eq!(app.workbook.valor(0, 2, 4), Value::Number(200.));
        // Mudar uma entrada e atualizar refaz com os mesmos parâmetros.
        app.workbook.escrever_celula(0, 2, 3, "7".into());
        let _ = app.update(Msg::TabelaDados(Evento::Atualizar));
        assert_eq!(app.workbook.valor(0, 2, 4), Value::Number(700.));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 2, 4), Value::Number(200.));
    }

    #[test]
    fn erro_fica_na_janela_e_clique_preenche_o_campo() {
        let mut app = App::default();
        let _ = app.update(Msg::IrPara("D1:E3".into()));
        let _ = app.update(Msg::TabelaDados(Evento::Abrir));
        let _ = app.update(Msg::Selecionar(0, 0));
        assert_eq!(app.tabela_dados.as_ref().unwrap().linha, "A1");
        let _ = app.update(Msg::TabelaDados(Evento::Confirmar));
        assert!(app.tabela_dados.as_ref().unwrap().erro.is_some());
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _: Element<'_, Msg> = app.view_tabela_dados();
        }
    }
}
