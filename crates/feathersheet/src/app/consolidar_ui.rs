//! Dados → Consolidar, como no Excel: a lista de referências (de qualquer
//! aba), a função e os rótulos. A conta mora no motor
//! (`operacoes::consolidar`, `Workbook::consolidar`); o resultado vai para a
//! célula ativa de quando a janela abriu.
use super::*;
use feathersheet_core::celulas::refs::{cell_name, col_name, parse_range_ref, quote_sheet, Rect};
use feathersheet_core::dinamica::pivot::AgregacaoOp;
use super::topicos_ui::Rotulo;

const FUNCOES: [(AgregacaoOp, &str); 8] = [
    (AgregacaoOp::Soma, "Soma"), (AgregacaoOp::Contagem, "Contagem"), (AgregacaoOp::Media, "Média"),
    (AgregacaoOp::Max, "Máximo"), (AgregacaoOp::Min, "Mínimo"), (AgregacaoOp::DesvioPadrao, "Desvio padrão"),
    (AgregacaoOp::Mediana, "Mediana"), (AgregacaoOp::Distintos, "Contagem de distintos"),
];

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Funcao(Rotulo),
    Referencia(String),
    Adicionar,
    Escolher(usize),
    Excluir,
    Topo(bool),
    Esquerda(bool),
    Aplicar,
    Fechar,
}

#[derive(Debug, Clone)]
pub(super) struct EstadoConsolidar {
    funcao: u32,
    referencia: String,
    referencias: Vec<String>,
    escolhida: Option<usize>,
    topo: bool,
    esquerda: bool,
    /// Aba e célula do canto do resultado.
    destino: (usize, u32, u32),
    erro: Option<String>,
}

/// "Fev!A1:C20" (ou "'Minha aba'!A1:C20", ou só "A1:C20" = aba do destino) em retângulo.
fn ler_referencia(texto: &str) -> Option<Rect> {
    let t = texto.trim();
    let (aba, r) = match t.rsplit_once('!') {
        Some((a, r)) => {
            let a = a.trim();
            let a = a.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')).map_or(a.to_string(), |x| x.replace("''", "'"));
            (Some(a), r)
        }
        None => (None, t),
    };
    let rect = parse_range_ref(r.trim()).or_else(|| parse_range_ref(&format!("{0}:{0}", r.trim())))?;
    Some(Rect { sheet: aba, ..rect.normalized() })
}

impl App {
    pub(super) fn evento_consolidar(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if !self.confirmar() { return Task::none(); }
                let a = self.area_selecionada();
                let aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
                let referencia = format!("{}!{}:{}", quote_sheet(&aba), cell_name(a.r0, a.c0), cell_name(a.r1, a.c1));
                self.edicao_menu = None;
                self.contexto = None;
                self.consolidar = Some(EstadoConsolidar { funcao: 0, referencia, referencias: Vec::new(), escolhida: None,
                    topo: false, esquerda: false, destino: (self.folha, self.linha, self.coluna), erro: None });
            }
            Evento::Funcao(r) => if let Some(e) = &mut self.consolidar { e.funcao = r.0; },
            Evento::Referencia(t) => if let Some(e) = &mut self.consolidar { e.referencia = t; },
            Evento::Adicionar => if let Some(e) = &mut self.consolidar {
                let t = e.referencia.trim().to_string();
                if ler_referencia(&t).is_none() {
                    e.erro = Some(tr!("Referência inválida. Use, por exemplo, Jan!A1:C20.").into());
                } else if !e.referencias.iter().any(|r| r.eq_ignore_ascii_case(&t)) {
                    e.referencias.push(t);
                    e.erro = None;
                }
            },
            Evento::Escolher(i) => if let Some(e) = &mut self.consolidar {
                e.escolhida = Some(i);
                if let Some(r) = e.referencias.get(i) { e.referencia = r.clone(); }
            },
            Evento::Excluir => if let Some(e) = &mut self.consolidar {
                if let Some(i) = e.escolhida.take().filter(|&i| i < e.referencias.len()) { e.referencias.remove(i); }
            },
            Evento::Topo(v) => if let Some(e) = &mut self.consolidar { e.topo = v; },
            Evento::Esquerda(v) => if let Some(e) = &mut self.consolidar { e.esquerda = v; },
            Evento::Aplicar => self.aplicar_consolidar(),
            Evento::Fechar => self.consolidar = None,
        }
        Task::none()
    }

    fn aplicar_consolidar(&mut self) {
        let Some(e) = self.consolidar.clone() else { return };
        let origens: Option<Vec<Rect>> = e.referencias.iter().map(|r| ler_referencia(r)).collect();
        let Some(origens) = origens else { return };
        let (aba, linha, coluna) = e.destino;
        let op = FUNCOES[e.funcao as usize % FUNCOES.len()].0;
        match self.workbook.consolidar(&origens, op, e.topo, e.esquerda, aba, linha, coluna) {
            Ok((alt, larg)) => {
                self.consolidar = None;
                self.alterado = true;
                self.folha = aba;
                self.trocar_selecao(linha, coluna);
                self.status = trf!("Consolidado em {}{}: {} × {}. Ctrl+Z desfaz.", col_name(coluna), linha + 1, alt, larg);
            }
            Err(erro) => if let Some(s) = &mut self.consolidar { s.erro = Some(erro); },
        }
    }

    fn view_consolidar(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.consolidar.as_ref().expect("janela Consolidar aberta");
        static ROTULOS: std::sync::LazyLock<Vec<Rotulo>> = std::sync::LazyLock::new(||
            FUNCOES.iter().enumerate().map(|(i, (_, t))| Rotulo(i as u32, tr!(t).to_string())).collect());
        let funcoes: &'static [Rotulo] = &ROTULOS;
        let rotulo = |t: &'static str| text(tr!(t)).size(12).color(COR_TEXTO_MUTED());
        let ev = |x: Evento| Msg::Consolidar(x);
        let mut lista = pelicano::column(Vec::<Element<_>>::new()).spacing(2);
        if e.referencias.is_empty() {
            lista = lista.push(text(tr!("Nenhuma referência ainda: escreva acima e clique em Adicionar.")).size(12).color(COR_TEXTO_MUTED()));
        }
        for (i, r) in e.referencias.iter().enumerate() {
            lista = lista.push(button(text(r.clone()).size(12)).width(Length::FILL).padding([5, 8])
                .style(estilo_item_menu(e.escolhida == Some(i), true)).on_press(ev(Evento::Escolher(i))));
        }
        let mut painel = pelicano::column![
            rotulo("Função"),
            pelicano::pick_list(funcoes, funcoes.get(e.funcao as usize).cloned(), move |r| Msg::Consolidar(Evento::Funcao(r))).width(Length::FILL),
            rotulo("Referência"),
            row![
                text_input("Jan!A1:C20", &e.referencia).on_input(move |t| Msg::Consolidar(Evento::Referencia(t)))
                    .on_submit(ev(Evento::Adicionar)).size(12).padding([6, 8]).width(Length::FILL),
                super::dialogo_interno::dlg::botao_secundario(tr!("Adicionar"), Some(ev(Evento::Adicionar))),
            ].spacing(8).align(pelicano::Align::Center),
            rotulo("Todas as referências"),
            row![
                container(container(super::dialogo_interno::dlg::rolagem(lista.width(Length::FILL))).max_height(140.0))
                    .width(Length::FILL).padding([8, 10]).style(super::dialogo_interno::dlg::estilo_bloco),
                super::dialogo_interno::dlg::botao_secundario(tr!("Excluir"), e.escolhida.map(|_| ev(Evento::Excluir))),
            ].spacing(8),
            rotulo("Usar rótulos na"),
            row![
                pelicano::checkbox(tr!("Linha superior"), e.topo).on_toggle(move |v| Msg::Consolidar(Evento::Topo(v))).size(12.5),
                pelicano::checkbox(tr!("Coluna esquerda"), e.esquerda).on_toggle(move |v| Msg::Consolidar(Evento::Esquerda(v))).size(12.5),
            ].spacing(16),
        ].spacing(8);
        if let Some(erro) = &e.erro {
            painel = painel.push(row![feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center));
        }
        let (aba, l, c) = e.destino;
        let sub = trf!("O resultado vai para {}!{}{}, a célula ativa.", self.workbook.nomes().get(aba).cloned().unwrap_or_default(), col_name(c), l + 1);
        self.dialogo(tr!("Consolidar"), painel)
            .icone("table")
            .subtitulo(sub)
            .largura(480.0)
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", (!e.referencias.is_empty()).then_some(ev(Evento::Aplicar)))
    }

    pub(super) fn sobrepor_consolidar<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.consolidar.is_none() { return base; }
        self.view_consolidar().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn referencia_com_e_sem_aba_e_com_aspas() {
        assert_eq!(ler_referencia("Fev!A1:C3").unwrap().sheet.as_deref(), Some("Fev"));
        assert_eq!(ler_referencia("'Minha aba'!B2").map(|r| (r.sheet, r.r0, r.c1)), Some((Some("Minha aba".into()), 1, 1)));
        assert!(ler_referencia("A1:B2").unwrap().sheet.is_none());
        assert!(ler_referencia("isso não").is_none());
    }

    #[test]
    fn janela_adiciona_duas_abas_e_grava_na_celula_ativa() {
        let mut app = App::default();
        app.workbook.criar_aba("Fev");
        for (aba, v) in [(0, "10"), (1, "5")] {
            app.workbook.escrever_celula(aba, 0, 0, "Sul".into());
            app.workbook.escrever_celula(aba, 0, 1, v.into());
        }
        let _ = app.update(Msg::Selecionar(4, 4));
        let _ = app.update(Msg::Consolidar(Evento::Abrir));
        let nome = app.workbook.nomes()[0].clone();
        for r in [format!("{nome}!A1:B1"), "Fev!A1:B1".into()] {
            let _ = app.update(Msg::Consolidar(Evento::Referencia(r)));
            let _ = app.update(Msg::Consolidar(Evento::Adicionar));
        }
        let _ = app.update(Msg::Consolidar(Evento::Esquerda(true)));
        let _ = app.view_consolidar();
        let _ = app.update(Msg::Consolidar(Evento::Aplicar));
        assert!(app.consolidar.is_none(), "{}", app.status);
        assert_eq!(app.workbook.valor(0, 4, 5), feathersheet_core::celulas::values::Value::Number(15.));
        let _ = app.update(Msg::Desfazer);
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(4, 5).is_empty(), "um Ctrl+Z desfaz");
    }
}
