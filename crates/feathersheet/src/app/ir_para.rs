//! Janela "Ir para" (F5 / Ctrl+G) com as opções do "Ir para especial" do
//! Excel. A busca das células é do motor
//! (`feathersheet_core::operacoes::ir_para_especial`); o resultado vira uma
//! seleção de várias áreas (`selecao_multipla.rs`).
use super::*;
use feathersheet_core::operacoes::ir_para_especial::{agrupar, localizar, Especial, Tipos};
use pelicano::checkbox; use pelicano::column; use pelicano::radio;

const ID_REFERENCIA: &str = "feathersheet.ir-para-referencia";
/// Acima disto a varredura demoraria demais para um clique.
const TETO_CELULAS: u64 = 5_000_000;
/// Áreas selecionadas no máximo (cada uma é desenhada e percorrida).
const TETO_AREAS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Opcao {
    /// Só a referência digitada.
    #[default]
    Referencia,
    EmBranco,
    Constantes,
    Formulas,
    DiferencasNaLinha,
    DiferencasNaColuna,
    RegiaoAtual,
    UltimaCelula,
    SomenteVisiveis,
    FormatacaoCondicional,
    ValidacaoDeDados,
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Referencia(String),
    Opcao(Opcao),
    Numeros(bool),
    Texto(bool),
    Logicos(bool),
    Erros(bool),
    Confirmar,
    Fechar,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Estado {
    pub referencia: String,
    pub opcao: Opcao,
    pub tipos: Tipos,
    pub erro: Option<String>,
}

impl App {
    pub(super) fn evento_ir_para(&mut self, evento: Evento) -> Task<Msg> {
        let Some(e) = self.ir_para.as_mut() else {
            if matches!(evento, Evento::Abrir) {
                if !self.confirmar() { return Task::none(); }
                self.ir_para = Some(Estado::default());
                return Task::focus(ID_REFERENCIA);
            }
            return Task::none();
        };
        match evento {
            Evento::Abrir => {}
            Evento::Referencia(t) => {
                e.referencia = t;
                e.opcao = Opcao::Referencia;
                e.erro = None;
            }
            Evento::Opcao(o) => { e.opcao = o; e.erro = None; }
            Evento::Numeros(v) => e.tipos.numeros = v,
            Evento::Texto(v) => e.tipos.texto = v,
            Evento::Logicos(v) => e.tipos.logicos = v,
            Evento::Erros(v) => e.tipos.erros = v,
            Evento::Fechar => self.ir_para = None,
            Evento::Confirmar => {
                let e = e.clone();
                if e.opcao == Opcao::Referencia {
                    if e.referencia.trim().is_empty() {
                        self.definir_erro_ir_para(tr!("Digite uma referência ou escolha uma opção."));
                        return Task::none();
                    }
                    self.ir_para = None;
                    return self.update(Msg::IrPara(e.referencia));
                }
                match self.ir_para_especial(e.opcao, e.tipos) {
                    Ok(()) => self.ir_para = None,
                    Err(m) => self.definir_erro_ir_para(&m),
                }
            }
        }
        Task::none()
    }

    fn definir_erro_ir_para(&mut self, m: &str) {
        if let Some(e) = &mut self.ir_para { e.erro = Some(m.into()); }
    }

    /// Onde procurar: com uma célula só selecionada, a área usada da aba
    /// inteira (como o Excel); senão, as áreas selecionadas, cortadas nela.
    fn areas_de_busca(&self) -> Vec<Rect> {
        let Some(aba) = self.workbook.aba(self.folha) else { return Vec::new() };
        if aba.sheet.is_empty() { return Vec::new(); }
        let usado = aba.sheet.used_rect();
        let areas = self.areas_selecionadas();
        let uma_celula = areas.len() == 1 && areas[0].r0 == areas[0].r1 && areas[0].c0 == areas[0].c1;
        if uma_celula { return vec![usado]; }
        areas.into_iter().filter_map(|a| {
            let r = Rect::new(a.r0.max(usado.r0), a.c0.max(usado.c0),
                a.r1.min(usado.r1), a.c1.min(usado.c1));
            (r.r0 <= r.r1 && r.c0 <= r.c1).then_some(r)
        }).collect()
    }

    fn ir_para_especial(&mut self, opcao: Opcao, tipos: Tipos) -> Result<(), String> {
        let especial = match opcao {
            Opcao::EmBranco => Some(Especial::EmBranco),
            Opcao::Constantes => Some(Especial::Constantes(tipos)),
            Opcao::Formulas => Some(Especial::Formulas(tipos)),
            Opcao::DiferencasNaLinha => Some(Especial::DiferencasNaLinha),
            Opcao::DiferencasNaColuna => Some(Especial::DiferencasNaColuna),
            _ => None,
        };
        let areas: Vec<Rect> = if let Some(especial) = especial {
            if matches!(especial, Especial::Constantes(t) | Especial::Formulas(t)
                if !(t.numeros || t.texto || t.logicos || t.erros)) {
                return Err(tr!("Marque ao menos um tipo de valor.").into());
            }
            let busca = self.areas_de_busca();
            let total: u64 = busca.iter().map(|a|
                u64::from(a.r1 - a.r0 + 1) * u64::from(a.c1 - a.c0 + 1)).sum();
            if total > TETO_CELULAS {
                return Err(trf!("A área tem {total} células; selecione até {TETO_CELULAS}."));
            }
            let Some(aba) = self.workbook.aba(self.folha) else { return Ok(()) };
            let ativa = (self.linha, self.coluna);
            let mut celulas = Vec::new();
            for area in &busca {
                celulas.extend(localizar(&aba.sheet, area, especial, ativa,
                    |r, c| self.workbook.valor(self.folha, r, c)));
            }
            celulas.sort_unstable();
            agrupar(&celulas)
        } else {
            match opcao {
                Opcao::RegiaoAtual => { self.selecionar_regiao_atual(); return Ok(()); }
                Opcao::UltimaCelula => {
                    let Some(aba) = self.workbook.aba(self.folha) else { return Ok(()) };
                    let u = aba.sheet.used_rect();
                    vec![Rect::new(u.r1, u.c1, u.r1, u.c1)]
                }
                Opcao::SomenteVisiveis => self.areas_visiveis(),
                Opcao::FormatacaoCondicional => condicional::regras(&self.workbook, self.folha)
                    .into_iter().flat_map(|r| r.ranges)
                    .map(|f| Rect::new(f.r0, f.c0, f.r1, f.c1)).collect(),
                Opcao::ValidacaoDeDados => {
                    let json = self.workbook.aba(self.folha).and_then(|a| a.sheet.va.as_deref());
                    validacao_ui::carregar_regras(json)?.into_iter().flat_map(|v| v.ranges).collect()
                }
                _ => Vec::new(),
            }
        };
        if areas.is_empty() { return Err(tr!("Nenhuma célula encontrada.").into()); }
        let quantas = areas.len();
        let mut areas = areas;
        areas.truncate(TETO_AREAS);
        // A primeira área achada vira a ativa (o Excel começa pelo topo).
        areas.rotate_left(1);
        self.selecionar_areas(areas);
        self.status = if quantas > TETO_AREAS {
            trf!("{quantas} áreas encontradas; selecionadas as primeiras {TETO_AREAS}.")
        } else { trf!("{quantas} área(s) selecionada(s).") };
        Ok(())
    }

    /// A seleção sem as linhas e colunas ocultas (altura/largura zero).
    fn areas_visiveis(&self) -> Vec<Rect> {
        let Some(aba) = self.workbook.aba(self.folha) else { return Vec::new() };
        let trechos = |a: u32, b: u32, mapa: &std::collections::HashMap<u32, u32>| {
            let mut v: Vec<(u32, u32)> = Vec::new();
            // Só os índices ocultos são consultados: o resto é contínuo.
            let mut ocultos: Vec<u32> = mapa.iter()
                .filter(|(i, w)| **w == 0 && **i >= a && **i <= b).map(|(i, _)| *i).collect();
            ocultos.sort_unstable();
            let mut inicio = a;
            for o in ocultos {
                if o > inicio { v.push((inicio, o - 1)); }
                inicio = o + 1;
            }
            if inicio <= b { v.push((inicio, b)); }
            v
        };
        let mut areas = Vec::new();
        for a in self.areas_selecionadas() {
            for (r0, r1) in trechos(a.r0, a.r1, &aba.sheet.row_heights) {
                for (c0, c1) in trechos(a.c0, a.c1, &aba.sheet.col_widths) {
                    areas.push(Rect::new(r0, c0, r1, c1));
                }
            }
        }
        areas
    }

    /// O painel sozinho (testes).
    #[cfg(test)]
    fn view_ir_para(&self) -> Element<'_, Msg> {
        self.dialogo_ir_para().painel()
    }

    fn dialogo_ir_para(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.ir_para.as_ref().expect("janela aberta");
        let ev = Msg::IrParaJanela;
        let grupo = |t: &'static str| text(t).size(10.5).color(COR_TEXTO_MUTED());
        let opcao = |rotulo: &'static str, o: Opcao| -> Element<'static, Msg> {
            radio(rotulo, o, Some(e.opcao), |o| Msg::IrParaJanela(Evento::Opcao(o)))
                .size(12.5).into()
        };
        let duas = |a: Element<'static, Msg>, b: Element<'static, Msg>| row![
            container(a).width(Length::Fill(1)),
            container(b).width(Length::Fill(1)),
        ];
        let com_tipos = matches!(e.opcao, Opcao::Constantes | Opcao::Formulas);
        let tipo = |rotulo: &'static str, v: bool, f: fn(bool) -> Evento| {
            let c = checkbox(rotulo, v).size(12);
            if com_tipos { c.on_toggle(move |b| Msg::IrParaJanela(f(b))) } else { c }
        };
        let mut corpo = column![
            row![
                text(tr!("Referência:")).size(12.5).width(Length::Fixed(86.0)),
                text_input(tr!("A1, B2:D10 ou um nome"), &e.referencia)
                    .id(ID_REFERENCIA)
                    .on_input(|t| Msg::IrParaJanela(Evento::Referencia(t)))
                    .on_submit(ev(Evento::Confirmar))
                    .size(13).padding([5, 8]).width(Length::FILL),
            ].spacing(8).align(pelicano::Align::Center),
            column![
                grupo("ESPECIAL"),
                duas(opcao(tr!("Em branco"), Opcao::EmBranco),
                    opcao(tr!("Diferenças na linha"), Opcao::DiferencasNaLinha)),
                duas(opcao(tr!("Constantes"), Opcao::Constantes),
                    opcao(tr!("Diferenças na coluna"), Opcao::DiferencasNaColuna)),
                duas(opcao(tr!("Fórmulas"), Opcao::Formulas),
                    opcao(tr!("Última célula"), Opcao::UltimaCelula)),
                row![
                    Space::new(Length::Fixed(22.0), 0.0),
                    tipo(tr!("Números"), e.tipos.numeros, Evento::Numeros),
                    tipo(tr!("Texto"), e.tipos.texto, Evento::Texto),
                    tipo(tr!("Lógicos"), e.tipos.logicos, Evento::Logicos),
                    tipo(tr!("Erros"), e.tipos.erros, Evento::Erros),
                ].spacing(12),
                duas(opcao(tr!("Região atual"), Opcao::RegiaoAtual),
                    opcao(tr!("Somente células visíveis"), Opcao::SomenteVisiveis)),
                duas(opcao(tr!("Formatação condicional"), Opcao::FormatacaoCondicional),
                    opcao(tr!("Validação de dados"), Opcao::ValidacaoDeDados)),
            ].spacing(8),
        ].spacing(16);
        if let Some(erro) = &e.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(tr!("Ir para"), corpo)
            .icone("arrow-right")
            .largura(500.0)
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", Some(ev(Evento::Confirmar)))
    }

    pub(super) fn sobrepor_ir_para<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.ir_para.is_none() { return base; }
        self.dialogo_ir_para().sobre(base)
    }
}
#[cfg(test)]
mod testes {
    use super::*;

    fn escolher(app: &mut App, o: Opcao) {
        let _ = app.update(Msg::IrParaJanela(Evento::Abrir));
        let _ = app.update(Msg::IrParaJanela(Evento::Opcao(o)));
        let _ = app.update(Msg::IrParaJanela(Evento::Confirmar));
    }

    #[test]
    fn em_branco_seleciona_os_buracos_da_area_usada() {
        let mut app = App::default();
        for (r, c) in [(0, 0), (0, 1), (1, 0), (2, 0), (2, 1)] {
            app.workbook.escrever_celula(0, r, c, "x".into());
        }
        escolher(&mut app, Opcao::EmBranco);
        assert!(app.ir_para.is_none());
        assert_eq!(app.areas_selecionadas(), vec![Rect::new(1, 1, 1, 1)]);
    }

    #[test]
    fn formulas_so_numericas_e_aviso_quando_nada_bate() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=1+1".into());
        app.workbook.escrever_celula(0, 2, 0, "=\"a\"".into());
        app.workbook.escrever_celula(0, 4, 0, "=2*3".into());
        let _ = app.update(Msg::IrParaJanela(Evento::Abrir));
        let _ = app.update(Msg::IrParaJanela(Evento::Opcao(Opcao::Formulas)));
        let _ = app.update(Msg::IrParaJanela(Evento::Texto(false)));
        let _ = app.update(Msg::IrParaJanela(Evento::Confirmar));
        let mut areas = app.areas_selecionadas();
        areas.sort_by_key(|a| a.r0);
        assert_eq!(areas, vec![Rect::new(0, 0, 0, 0), Rect::new(4, 0, 4, 0)]);
        escolher(&mut app, Opcao::ValidacaoDeDados);
        assert!(app.ir_para.as_ref().is_some_and(|e| e.erro.is_some()));
    }

    #[test]
    fn somente_visiveis_pula_linha_oculta_e_referencia_vai_direto() {
        let mut app = App::default();
        app.workbook.editar(0, |s| { s.row_heights.insert(2, 0); });
        let _ = app.update(Msg::IrPara("A1:B5".into()));
        escolher(&mut app, Opcao::SomenteVisiveis);
        assert_eq!(app.areas_selecionadas().len(), 2);
        let _ = app.update(Msg::IrParaJanela(Evento::Abrir));
        let _ = app.update(Msg::IrParaJanela(Evento::Referencia("C7".into())));
        let _ = app.update(Msg::IrParaJanela(Evento::Confirmar));
        assert_eq!((app.linha, app.coluna), (6, 2));
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _ = app.update(Msg::IrParaJanela(Evento::Abrir));
            let _: Element<'_, Msg> = app.view_ir_para();
        }
    }
}
