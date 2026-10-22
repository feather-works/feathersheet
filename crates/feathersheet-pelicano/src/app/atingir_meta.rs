//! Janela "Atingir meta" (Dados → Previsão), como no Excel: "Definir
//! célula" (uma fórmula) chega a "Para valor" mudando "Alterando a célula".
//! A busca numérica é do motor (`operacoes::atingir_meta`); aqui ela escreve
//! cada tentativa sem histórico e, no fim, "Manter" grava o valor achado num
//! único passo de desfazer, ou "Desfazer" devolve o valor original.
//!
//! A janela não bloqueia a planilha: com o cursor num campo de célula,
//! clicar na grade escreve o endereço nele.
use super::*;
use feathersheet_core::celulas::{input::parse_literal, number::to_js_string,
    refs::parse_cell_ref, values::Value};
use feathersheet_core::operacoes::atingir_meta::{buscar, ITERACOES, TOLERANCIA};
use pelicano::column;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { Definir, Alterando }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Definir(String),
    Valor(String),
    Alterando(String),
    Confirmar,
    Manter,
    Desfazer,
    Fechar,
}

/// O que a busca achou, mostrado antes de a pessoa decidir.
#[derive(Debug, Clone)]
pub(super) struct Resultado {
    pub celula: (u32, u32),
    pub original: Option<Snap>,
    pub x: f64,
    pub y: f64,
    pub convergiu: bool,
    pub tentativas: usize,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub definir: String,
    pub valor: String,
    pub alterando: String,
    /// Campo que recebe o endereço clicado na grade.
    pub campo: Campo,
    pub resultado: Option<Resultado>,
    pub erro: Option<String>,
    /// Aba onde a busca roda (a janela não acompanha a troca de aba).
    pub aba: usize,
}

impl App {
    pub(super) fn evento_atingir_meta(&mut self, evento: Evento) -> Task<Msg> {
        if let Evento::Abrir = evento {
            if !self.confirmar() { return Task::none(); }
            // Com uma fórmula ativa, ela já entra como "Definir célula".
            let ativa = self.workbook.aba(self.folha)
                .is_some_and(|a| a.sheet.get_raw(self.linha, self.coluna).starts_with('='));
            self.atingir_meta = Some(Estado {
                definir: if ativa { cell_name(self.linha, self.coluna) } else { String::new() },
                valor: String::new(), alterando: String::new(),
                campo: if ativa { Campo::Alterando } else { Campo::Definir },
                resultado: None, erro: None, aba: self.folha,
            });
            return Task::none();
        }
        let Some(e) = self.atingir_meta.as_mut() else { return Task::none() };
        match evento {
            Evento::Abrir => {}
            Evento::Definir(t) => { e.definir = t; e.campo = Campo::Definir; e.erro = None; }
            Evento::Valor(t) => { e.valor = t; e.erro = None; }
            Evento::Alterando(t) => { e.alterando = t; e.campo = Campo::Alterando; e.erro = None; }
            Evento::Confirmar => {
                if let Err(m) = self.executar_atingir_meta() {
                    if let Some(e) = &mut self.atingir_meta { e.erro = Some(m); }
                }
            }
            Evento::Manter => {
                if let Some(r) = self.atingir_meta.take().and_then(|e| e.resultado.map(|r| (e.aba, r))) {
                    let (aba, r) = r;
                    // Volta ao original sem histórico e grava o novo COM
                    // histórico: o Ctrl+Z leva de volta ao valor de antes.
                    self.restaurar_meta(aba, &r);
                    self.workbook.escrever_celula(aba, r.celula.0, r.celula.1, to_js_string(r.x));
                    self.alterado = true;
                    self.status = trf!("{} = {} mantido", cell_name(r.celula.0, r.celula.1),
                        to_js_string(r.x));
                }
            }
            Evento::Desfazer | Evento::Fechar => {
                if let Some(e) = self.atingir_meta.take() {
                    if let Some(r) = e.resultado { self.restaurar_meta(e.aba, &r); }
                }
            }
        }
        Task::none()
    }

    /// Clique na grade com a janela aberta: o endereço vai para o campo ativo.
    pub(super) fn atingir_meta_clique(&mut self, r: u32, c: u32) {
        let Some(e) = self.atingir_meta.as_mut() else { return };
        if e.resultado.is_some() || self.folha != e.aba { return; }
        let endereco = cell_name(r, c);
        match e.campo {
            Campo::Definir => e.definir = endereco,
            Campo::Alterando => e.alterando = endereco,
        }
        e.erro = None;
    }

    fn restaurar_meta(&mut self, aba: usize, r: &Resultado) {
        let (linha, coluna) = r.celula;
        let original = r.original.clone();
        self.workbook.editar(aba, |s| { s.apply(linha, coluna, original, false); });
    }

    fn executar_atingir_meta(&mut self) -> Result<(), String> {
        let e = self.atingir_meta.clone().expect("janela aberta");
        let celula = |t: &str, nome: &str| parse_cell_ref(&t.trim().replace('$', "").to_uppercase())
            .ok_or(trf!("\"{nome}\" precisa ser o endereço de uma célula, como B10."));
        let (dr, dc) = celula(&e.definir, tr!("Definir célula"))?;
        let (ar, ac) = celula(&e.alterando, tr!("Alterando a célula"))?;
        let alvo = match parse_literal(e.valor.trim()).value {
            Value::Number(n) => n,
            _ => return Err(tr!("\"Para valor\" precisa ser um número.").into()),
        };
        let aba = self.workbook.aba(e.aba).ok_or(tr!("A aba não existe mais."))?;
        if !aba.sheet.get_raw(dr, dc).starts_with('=') {
            return Err(trf!("{} precisa conter uma fórmula.", cell_name(dr, dc)));
        }
        if aba.sheet.get_raw(ar, ac).starts_with('=') {
            return Err(trf!("{} precisa conter um valor, não uma fórmula.", cell_name(ar, ac)));
        }
        let original = aba.sheet.snapshot(ar, ac);
        let estilo = original.as_ref().and_then(|s| s.style.clone());
        let inicio = match self.workbook.valor(e.aba, ar, ac) {
            Value::Number(n) => n,
            Value::Empty => 0.0,
            _ => return Err(trf!("{} precisa conter um número.", cell_name(ar, ac))),
        };
        let workbook = &mut self.workbook;
        let indice = e.aba;
        // Cada tentativa escreve sem histórico; o motor recalcula a fórmula.
        let busca = buscar(|x| {
            let snap = Snap { raw: to_js_string(x), style: estilo.clone() };
            workbook.editar(indice, |s| { s.apply(ar, ac, Some(snap), false); });
            match workbook.valor(indice, dr, dc) { Value::Number(n) => Some(n), _ => None }
        }, inicio, alvo, ITERACOES, TOLERANCIA);
        let Some(b) = busca else {
            self.workbook.editar(indice, |s| { s.apply(ar, ac, original.clone(), false); });
            return Err(trf!("{} dá erro com o valor atual de {}.", cell_name(dr, dc), cell_name(ar, ac)));
        };
        // A planilha mostra a solução enquanto a pessoa decide.
        let snap = Snap { raw: to_js_string(b.x), style: estilo };
        self.workbook.editar(indice, |s| { s.apply(ar, ac, Some(snap), false); });
        if let Some(e) = &mut self.atingir_meta {
            e.resultado = Some(Resultado { celula: (ar, ac), original, x: b.x, y: b.y,
                convergiu: b.convergiu, tentativas: b.tentativas });
        }
        Ok(())
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    fn view_atingir_meta(&self) -> Element<'_, Msg> {
        self.dialogo_atingir_meta().painel()
    }

    fn dialogo_atingir_meta(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.atingir_meta.as_ref().expect("janela aberta");
        let negrito = pelicano::Font { weight: 600, ..Default::default() };
        let ev = Msg::AtingirMeta;
        let mut corpo = pelicano::column(Vec::<Element<_>>::new()).spacing(14);
        let resolvido = e.resultado.is_some();
        if let Some(r) = &e.resultado {
            let alvo = cell_name(r.celula.0, r.celula.1);
            let (frase, detalhe) = if r.convergiu {
                (tr!("Solução encontrada.").to_owned(),
                    trf!("{alvo} = {} faz o resultado valer {}.", to_js_string(r.x), to_js_string(r.y)))
            } else {
                (tr!("Não foi possível chegar ao valor pedido.").to_owned(),
                    trf!("O mais perto foi {alvo} = {} (resultado {}), em {} tentativas.",
                        to_js_string(r.x), to_js_string(r.y), r.tentativas))
            };
            corpo = corpo.push(column![
                text(frase).size(14).font(negrito),
                text(detalhe).size(12.5).color(COR_TEXTO_MUTED()),
            ].spacing(4));
        } else {
            let campo = |rotulo: &'static str, valor: &str, dica: &'static str, ativo: bool,
                         f: fn(String) -> Evento| row![
                text(rotulo).size(12.5).width(Length::Fixed(132.0)),
                text_input(dica, valor.to_owned()).on_input(move |t| Msg::AtingirMeta(f(t)))
                    .on_submit(Msg::AtingirMeta(Evento::Confirmar))
                    .size(13).padding([5, 8]).width(Length::FILL)
                    .style(move |tema, status| {
                        let mut s = pelicano::input_default(tema, status);
                        if ativo { s.border.color = COR_AZUL_ACENTO(); }
                        s
                    }),
            ].spacing(8).align(pelicano::Align::Center);
            corpo = corpo
                .push(campo(tr!("Definir célula:"), &e.definir, tr!("célula com a fórmula, ex.: B10"),
                    e.campo == Campo::Definir, Evento::Definir))
                .push(campo(tr!("Para valor:"), &e.valor, tr!("o resultado desejado, ex.: 1000"),
                    false, Evento::Valor))
                .push(campo(tr!("Alterando a célula:"), &e.alterando, tr!("célula com o valor, ex.: B3"),
                    e.campo == Campo::Alterando, Evento::Alterando))
                .push(row![
                    feather_ui::icones::icone("info-circle", 14.0),
                    text(tr!("Clique na planilha para preencher o campo destacado."))
                        .size(11.5).color(COR_TEXTO_MUTED()),
                ].spacing(6).align(pelicano::Align::Center));
            if let Some(erro) = &e.erro {
                corpo = corpo.push(row![
                    feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO()),
                ].spacing(6).align(pelicano::Align::Center));
            }
        }
        // Não modal, à direita: os cliques na planilha preenchem os campos.
        let d = self.dialogo(tr!("Atingir meta"), corpo)
            .icone("math-function")
            .topo(14.0)
            .altura_maxima((self.tamanho_principal.height - self.topo_dialogo() - 28.0).max(300.0))
            .largura(440.0)
            .nao_modal()
            .a_direita()
            .ao_fechar(ev(Evento::Fechar));
        if resolvido {
            d.secundario(tr!("Desfazer"), Some(ev(Evento::Desfazer)))
                .primario(tr!("Manter"), Some(ev(Evento::Manter)))
        } else {
            d.secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
                .primario("OK", Some(ev(Evento::Confirmar)))
        }
    }

    /// Flutua no canto superior direito da grade, sem bloquear os cliques
    /// nas células (eles preenchem os campos).
    pub(super) fn sobrepor_atingir_meta<'a>(&'a self, centro: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.atingir_meta.is_none() { return centro; }
        self.dialogo_atingir_meta().sobre(centro)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn planilha() -> App {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 2, 1, "1".into());             // B3
        app.workbook.escrever_celula(0, 9, 1, "=B3*2+10".into());      // B10
        app
    }

    fn preencher(app: &mut App, definir: &str, valor: &str, alterando: &str) {
        let _ = app.update(Msg::AtingirMeta(Evento::Abrir));
        let _ = app.update(Msg::AtingirMeta(Evento::Definir(definir.into())));
        let _ = app.update(Msg::AtingirMeta(Evento::Valor(valor.into())));
        let _ = app.update(Msg::AtingirMeta(Evento::Alterando(alterando.into())));
        let _ = app.update(Msg::AtingirMeta(Evento::Confirmar));
    }

    #[test]
    fn acha_a_solucao_e_manter_grava_com_um_desfazer() {
        let mut app = planilha();
        preencher(&mut app, "B10", "1000", "$B$3");
        let r = app.atingir_meta.as_ref().unwrap().resultado.clone().expect("achou");
        assert!(r.convergiu && (r.x - 495.0).abs() < 0.01);
        let _: Element<'_, Msg> = app.view_atingir_meta();
        let _ = app.update(Msg::AtingirMeta(Evento::Manter));
        assert!(app.atingir_meta.is_none());
        let Value::Number(b10) = app.workbook.valor(0, 9, 1) else { panic!("B10 sem número") };
        assert!((b10 - 1000.0).abs() <= TOLERANCIA, "{b10}");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0, 2, 1), Value::Number(1.));
        assert_eq!(app.workbook.valor(0, 9, 1), Value::Number(12.));
    }

    #[test]
    fn desfazer_devolve_o_original_e_erros_ficam_na_janela() {
        let mut app = planilha();
        preencher(&mut app, "B10", "1000", "B3");
        let _ = app.update(Msg::AtingirMeta(Evento::Desfazer));
        assert_eq!(app.workbook.valor(0, 2, 1), Value::Number(1.));
        // Definir sem fórmula e alvo que não é número.
        preencher(&mut app, "B3", "10", "B10");
        assert!(app.atingir_meta.as_ref().unwrap().erro.as_deref().unwrap().contains("fórmula"));
        let _ = app.update(Msg::AtingirMeta(Evento::Definir("B10".into())));
        let _ = app.update(Msg::AtingirMeta(Evento::Valor("muito".into())));
        let _ = app.update(Msg::AtingirMeta(Evento::Confirmar));
        assert!(app.atingir_meta.as_ref().unwrap().erro.as_deref().unwrap().contains("número"));
    }

    #[test]
    fn clique_na_grade_preenche_o_campo_ativo_e_janela_monta_nos_dois_temas() {
        let mut app = planilha();
        let _ = app.update(Msg::Selecionar(9, 1));
        let _ = app.update(Msg::AtingirMeta(Evento::Abrir));
        assert_eq!(app.atingir_meta.as_ref().unwrap().definir, "B10");
        let _ = app.update(Msg::Selecionar(2, 1));
        assert_eq!(app.atingir_meta.as_ref().unwrap().alterando, "B3");
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _: Element<'_, Msg> = app.view_atingir_meta();
        }
    }
}
