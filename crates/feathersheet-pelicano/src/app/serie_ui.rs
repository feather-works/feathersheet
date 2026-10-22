//! Diálogo "Série" (Página Inicial > Preencher ▾ > Série…), como no Excel:
//! Série em linhas/colunas, tipo (Linear, Crescimento, Data,
//! AutoPreenchimento), unidade de data, tendência, incremento e limite.
//! O cálculo é do motor (`feathersheet_core::operacoes::serie`,
//! `Sheet::preencher_serie`); aqui só se escolhe origem, destino e opções.
use super::*;
use super::dialogo_interno::dlg;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::serie::{ConfigSerie, DirecaoSerie, TipoSerie, UnidadeData};
use pelicano::checkbox; use pelicano::column; use pelicano::radio;

/// "Tipo" da caixa do Excel. Tendência é a caixa de seleção, não um tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo { Linear, Crescimento, Data, AutoPreenchimento }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Direcao(DirecaoSerie),
    Tipo(Tipo),
    Unidade(UnidadeData),
    Tendencia(bool),
    Incremento(String),
    Limite(String),
    Aplicar,
    Fechar,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    pub area: Rect,
    pub direcao: DirecaoSerie,
    pub tipo: Tipo,
    pub unidade: UnidadeData,
    pub tendencia: bool,
    pub incremento: String,
    pub limite: String,
    pub erro: Option<String>,
}

impl Estado {
    /// Tendência só vale para Linear e Crescimento (como no Excel).
    fn tendencia_ativa(&self) -> bool {
        self.tendencia && matches!(self.tipo, Tipo::Linear | Tipo::Crescimento)
    }

    /// A configuração do motor, ou o erro do campo mal preenchido.
    pub fn config(&self) -> Result<ConfigSerie, String> {
        let numero = |texto: &str, campo: &str| -> Result<Option<f64>, String> {
            let t = texto.trim();
            if t.is_empty() { return Ok(None); }
            t.replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite()).map(Some)
                .ok_or_else(|| trf!("{campo}: digite um número"))
        };
        let tipo = match (self.tipo, self.tendencia_ativa()) {
            (Tipo::Linear, true) => TipoSerie::TendenciaLinear,
            (Tipo::Crescimento, true) => TipoSerie::TendenciaExponencial,
            (Tipo::Linear, false) | (Tipo::AutoPreenchimento, _) => TipoSerie::Linear,
            (Tipo::Crescimento, false) => TipoSerie::Geometrica,
            (Tipo::Data, _) => TipoSerie::Data(self.unidade),
        };
        let incremento = if self.tendencia_ativa() { None } else { numero(&self.incremento, tr!("Incremento"))? };
        let limite = match self.tipo {
            // Data aceita o limite como data digitada (15/03/2026) ou número de série.
            Tipo::Data if !self.limite.trim().is_empty() && numero(&self.limite, "").is_err() => {
                Some(feathersheet_core::celulas::input::parse_date_text(self.limite.trim())
                    .ok_or(tr!("Limite: digite uma data ou um número"))?)
            }
            _ => numero(&self.limite, tr!("Limite"))?,
        };
        Ok(ConfigSerie { tipo, direcao: self.direcao, incremento, limite })
    }
}

/// As sementes: o começo da primeira linha (ou coluna) da seleção na
/// direção da série. Com incremento (ou AutoPreenchimento sem tendência)
/// basta a primeira célula para números; tendência e série sem incremento
/// usam as células preenchidas seguidas do começo.
fn origem(sheet: &feathersheet_core::planilha::sheet::Sheet, area: &Rect, direcao: DirecaoSerie,
    todas: bool, aceita: impl Fn(&str, &Value) -> bool) -> Rect
{
    let a = area.normalized();
    let colunas = direcao == DirecaoSerie::Colunas;
    let comprimento = if colunas { a.r1 - a.r0 + 1 } else { a.c1 - a.c0 + 1 };
    let mut n = 1;
    if todas {
        while n < comprimento {
            let (r, c) = if colunas { (a.r0 + n, a.c0) } else { (a.r0, a.c0 + n) };
            if !aceita(&sheet.get_raw(r, c), &sheet.get_value(r, c)) { break; }
            n += 1;
        }
    }
    if colunas { Rect::new(a.r0, a.c0, a.r0 + n - 1, a.c1) } else { Rect::new(a.r0, a.c0, a.r1, a.c0 + n - 1) }
}

/// Opção da caixa; `ativo = false` a mostra esmaecida (fora do contexto).
fn opcao<'a, V: Copy + Eq + 'a>(rotulo: &'static str, valor: V, atual: V, msg: fn(V) -> Msg, ativo: bool)
    -> Element<'a, Msg>
{
    let r = radio(rotulo, valor, Some(atual), msg).size(12.5);
    // shortcut: o Toggle do pelicano só troca a cor da marca; o rótulo e o anel não esmaecem, até o Toggle ganhar `style`.
    if ativo { r.into() } else { r.color(COR_TEXTO_MUTED()).into() }
}

fn numero_semente(raw: &str, valor: &Value) -> bool {
    matches!(valor, Value::Number(_)) && !raw.starts_with('=')
}

impl App {
    fn abrir_serie(&mut self) {
        self.edicao_menu = None;
        if !self.confirmar() { return; }
        let area = self.area_selecionada().normalized();
        // Como o Excel: mais linhas que colunas = série em colunas.
        let direcao = if area.r1 - area.r0 >= area.c1 - area.c0 { DirecaoSerie::Colunas } else { DirecaoSerie::Linhas };
        // Data quando a primeira célula tem formato de data.
        let data = self.workbook.aba(self.folha).is_some_and(|aba| {
            use feathersheet_core::celulas::input::{parse_literal, NumberFormat};
            matches!(parse_literal(&aba.sheet.get_raw(area.r0, area.c0)).format,
                Some(NumberFormat::Date | NumberFormat::Datetime))
        });
        self.serie = Some(Estado {
            area, direcao,
            tipo: if data { Tipo::Data } else { Tipo::Linear },
            unidade: UnidadeData::Dia,
            tendencia: false,
            incremento: "1".into(),
            limite: String::new(),
            erro: None,
        });
    }

    fn aplicar_serie(&mut self) {
        let Some(estado) = self.serie.clone() else { return };
        let erro = |app: &mut Self, msg: String| {
            if let Some(s) = &mut app.serie { s.erro = Some(msg); }
        };
        let Some(sheet) = self.workbook.aba(self.folha).map(|a| &a.sheet) else { return };
        if estado.tipo == Tipo::AutoPreenchimento {
            // A mesma regra da alça: as células preenchidas do começo são o padrão.
            let o = origem(sheet, &estado.area, estado.direcao, true, |raw, _| !raw.is_empty());
            self.serie = None;
            self.aplicar_preenchimento(&o, &estado.area, false);
            return;
        }
        let cfg = match estado.config() {
            Ok(cfg) => cfg,
            Err(e) => return erro(self, e),
        };
        let todas = estado.tendencia_ativa() || cfg.incremento.is_none();
        let o = origem(sheet, &estado.area, estado.direcao, todas, numero_semente);
        if !numero_semente(&sheet.get_raw(o.r0, o.c0), &sheet.get_value(o.r0, o.c0)) {
            return erro(self, tr!("A primeira célula da seleção precisa ter um número ou uma data.").into());
        }
        let mut resultado = Ok(0);
        self.workbook.editar(self.folha, |s| resultado = s.preencher_serie(&o, &estado.area, &cfg));
        match resultado {
            Err(e) => erro(self, e),
            Ok(0) => erro(self, if o == estado.area {
                tr!("Selecione as células de destino junto com o valor inicial, ou informe o limite.").into()
            } else {
                tr!("Nenhuma célula foi preenchida: confira o incremento e o limite.").into()
            }),
            Ok(n) => {
                self.serie = None;
                self.alterado = true;
                self.atualizar_circulos();
                self.status = trf!("Série: {n} célula(s) preenchida(s); Ctrl+Z desfaz.");
            }
        }
    }

    /// `FEATHERSHEET_CAPTURAR_DIALOGO=serie|serie_data`: valor inicial em A1 e A1:A12 selecionado.
    pub(super) fn preparar_captura_serie(&mut self, nome: &str) -> Task<Msg> {
        self.workbook.escrever_celula(self.folha, 0, 0, if nome == "serie_data" { "31/01/2026" } else { "1" }.into());
        self.trocar_selecao(0, 0);
        self.fim_selecao = (11, 0);
        self.abrir_serie();
        if let Some(s) = &mut self.serie {
            if nome == "serie_data" { s.unidade = UnidadeData::Mes; } else { s.limite = "100".into(); }
        }
        Task::none()
    }

    pub(super) fn evento_serie(&mut self, evento: Evento) -> Task<Msg> {
        if let (Some(s), false) = (&mut self.serie, matches!(evento, Evento::Abrir | Evento::Aplicar | Evento::Fechar)) {
            s.erro = None;
            match evento {
                Evento::Direcao(d) => s.direcao = d,
                Evento::Tipo(t) => s.tipo = t,
                // Unidade só vale com o tipo Data (as opções ficam apagadas).
                Evento::Unidade(u) if s.tipo == Tipo::Data => s.unidade = u,
                Evento::Tendencia(v) => s.tendencia = v,
                Evento::Incremento(v) => s.incremento = v,
                Evento::Limite(v) => s.limite = v,
                _ => {}
            }
            return Task::none();
        }
        match evento {
            Evento::Abrir => self.abrir_serie(),
            Evento::Aplicar => self.aplicar_serie(),
            Evento::Fechar => self.serie = None,
            _ => {}
        }
        Task::none()
    }

    pub(super) fn sobrepor_serie<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(s) = &self.serie else { return base };
        let ev = |e: Evento| Msg::Serie(e);
        let grupo = |titulo: &'static str, itens: Vec<Element<'a, Msg>>| -> Element<'a, Msg> {
            column![dlg::secao(titulo), column(itens).spacing(8)].spacing(8).width(Length::FILL).into()
        };
        let serie_em = grupo(tr!("Série em"), vec![
            opcao(tr!("Linhas"), DirecaoSerie::Linhas, s.direcao, |d| Msg::Serie(Evento::Direcao(d)), true),
            opcao(tr!("Colunas"), DirecaoSerie::Colunas, s.direcao, |d| Msg::Serie(Evento::Direcao(d)), true),
        ]);
        let tipo = grupo(tr!("Tipo"), [
            (tr!("Linear"), Tipo::Linear), (tr!("Crescimento"), Tipo::Crescimento),
            (tr!("Data"), Tipo::Data), ("AutoPreenchimento", Tipo::AutoPreenchimento),
        ].into_iter().map(|(r, t)| opcao(r, t, s.tipo, |t| Msg::Serie(Evento::Tipo(t)), true)).collect());
        let data = s.tipo == Tipo::Data;
        let unidade = grupo(tr!("Unidade de data"), [
            (tr!("Dia"), UnidadeData::Dia), (tr!("Dia da semana"), UnidadeData::DiaUtil),
            (tr!("Mês"), UnidadeData::Mes), (tr!("Ano"), UnidadeData::Ano),
        ].into_iter().map(|(r, u)| opcao(r, u, s.unidade, |u| Msg::Serie(Evento::Unidade(u)), data)).collect());

        let pode_tendencia = matches!(s.tipo, Tipo::Linear | Tipo::Crescimento);
        let tendencia = checkbox(tr!("Tendência"), s.tendencia && pode_tendencia).size(12.5);
        let tendencia = if pode_tendencia { tendencia.on_toggle(|v| Msg::Serie(Evento::Tendencia(v))) } else { tendencia };
        let sem_incremento = s.tendencia_ativa() || s.tipo == Tipo::AutoPreenchimento;
        let campo = |rotulo: &'static str, dica: &'static str, valor: &str, ativo: bool, f: fn(String) -> Evento| {
            let entrada = text_input(dica, valor.to_owned()).size(13).padding([6, 8]).width(Length::Fixed(150.0));
            row![
                text(rotulo).size(12.5).color(if ativo { COR_TEXTO() } else { COR_TEXTO_MUTED() })
                    .width(Length::Fixed(90.0)),
                if ativo { entrada.on_input(move |v| ev(f(v))) } else { entrada },
            ].spacing(8).align(pelicano::Align::Center)
        };
        let dica_limite = if data { tr!("Data ou número") } else { tr!("Opcional") };
        let mut corpo = column![
            row![serie_em, tipo, unidade].spacing(18),
            tendencia,
            row![
                campo(tr!("Incremento"), "1", &s.incremento, !sem_incremento, Evento::Incremento),
                campo(tr!("Limite"), dica_limite, &s.limite, s.tipo != Tipo::AutoPreenchimento, Evento::Limite),
            ].spacing(18),
        ].spacing(16);
        if let Some(erro) = &s.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        let alvo = feathersheet_core::celulas::refs::format_ref(&s.area);
        self.dialogo(tr!("Série"), corpo)
            .icone("list-numbers")
            .subtitulo(trf!("Intervalo {alvo} · o valor inicial fica na primeira célula"))
            .largura(540.0)
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", Some(ev(Evento::Aplicar)))
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com(celulas: &[(u32, u32, &str)], area: (u32, u32, u32, u32)) -> App {
        let mut app = App::default();
        for &(r, c, v) in celulas { app.workbook.escrever_celula(0, r, c, v.into()); }
        app.linha = area.0;
        app.coluna = area.1;
        app.fim_selecao = (area.2, area.3);
        let _ = app.update(Msg::Serie(Evento::Abrir));
        app
    }

    fn raw(app: &App, r: u32, c: u32) -> String { app.workbook.aba(0).unwrap().sheet.get_raw(r, c) }

    #[test]
    fn linear_com_incremento_e_um_ctrl_z() {
        let mut app = app_com(&[(0, 0, "1")], (0, 0, 5, 0));
        assert_eq!(app.serie.as_ref().unwrap().direcao, DirecaoSerie::Colunas);
        let _: Element<'_, Msg> = app.sobrepor_serie(Space::new(0.0, 0).into());
        let _ = app.update(Msg::Serie(Evento::Incremento("2,5".into())));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert!(app.serie.is_none(), "{:?}", app.serie.as_ref().and_then(|s| s.erro.clone()));
        assert_eq!((raw(&app, 1, 0), raw(&app, 5, 0)), ("3.5".into(), "13.5".into()));
        let _ = app.update(Msg::Desfazer);
        assert_eq!((raw(&app, 1, 0), raw(&app, 5, 0)), ("".into(), "".into()));
        assert_eq!(raw(&app, 0, 0), "1");
    }

    #[test]
    fn crescimento_com_limite_em_linhas() {
        let mut app = app_com(&[(0, 0, "2")], (0, 0, 0, 9));
        assert_eq!(app.serie.as_ref().unwrap().direcao, DirecaoSerie::Linhas);
        let _ = app.update(Msg::Serie(Evento::Tipo(Tipo::Crescimento)));
        let _ = app.update(Msg::Serie(Evento::Incremento("2".into())));
        let _ = app.update(Msg::Serie(Evento::Limite("40".into())));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert_eq!((raw(&app, 0, 1), raw(&app, 0, 4), raw(&app, 0, 5)), ("4".into(), "32".into(), "".into()));
    }

    #[test]
    fn tendencia_linear_usa_as_sementes() {
        let mut app = app_com(&[(0, 0, "1"), (1, 0, "3"), (2, 0, "5")], (0, 0, 4, 0));
        let _ = app.update(Msg::Serie(Evento::Tendencia(true)));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert_eq!((raw(&app, 3, 0), raw(&app, 4, 0)), ("7".into(), "9".into()));
        assert_eq!(raw(&app, 1, 0), "3");
    }

    #[test]
    fn data_por_mes_e_unidade_so_com_data() {
        // 31/01/2026 = serial 46053; mês a mês para no último dia do mês.
        let mut app = app_com(&[(0, 0, "46053")], (0, 0, 2, 0));
        let _ = app.update(Msg::Serie(Evento::Unidade(UnidadeData::Mes)));
        assert_eq!(app.serie.as_ref().unwrap().unidade, UnidadeData::Dia, "sem Data a unidade não muda");
        let _ = app.update(Msg::Serie(Evento::Tipo(Tipo::Data)));
        let _ = app.update(Msg::Serie(Evento::Unidade(UnidadeData::Mes)));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert_eq!(raw(&app, 1, 0), "46081"); // 28/02/2026
    }

    #[test]
    fn data_digitada_abre_como_data_e_mantem_o_formato() {
        let mut app = app_com(&[(0, 0, "31/01/2026")], (0, 0, 2, 0));
        assert_eq!(app.serie.as_ref().unwrap().tipo, Tipo::Data);
        let _ = app.update(Msg::Serie(Evento::Unidade(UnidadeData::Mes)));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_value(1, 0), Value::Number(46081.0));
        assert!(s.estilo_mesclado(0, 0).is_some());
        assert_eq!(s.estilo_mesclado(1, 0), s.estilo_mesclado(0, 0));
    }

    #[test]
    fn erros_ficam_no_dialogo() {
        let mut app = app_com(&[(0, 0, "texto")], (0, 0, 3, 0));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert!(app.serie.as_ref().unwrap().erro.as_deref().unwrap().contains("número"));
        let mut app = app_com(&[(0, 0, "1")], (0, 0, 0, 0));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert!(app.serie.as_ref().unwrap().erro.as_deref().unwrap().contains("limite"));
        let _ = app.update(Msg::Serie(Evento::Incremento("abc".into())));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert!(app.serie.as_ref().unwrap().erro.as_deref().unwrap().starts_with("Incremento"));
        let _ = app.update(Msg::Serie(Evento::Fechar));
        assert!(app.serie.is_none());
    }

    #[test]
    fn autopreenchimento_usa_a_alca() {
        let mut app = app_com(&[(0, 0, "Seg")], (0, 0, 2, 0));
        let _ = app.update(Msg::Serie(Evento::Tipo(Tipo::AutoPreenchimento)));
        let _ = app.update(Msg::Serie(Evento::Aplicar));
        assert!(app.serie.is_none());
        assert!(!raw(&app, 2, 0).is_empty());
    }

    #[test]
    fn menu_preencher_abre_a_serie() {
        let mut app = App::default();
        let _ = app.update(Msg::AlternarMenuEdicao("preencher"));
        let _ = app.update(Msg::EdicaoMenuAcao(Box::new(Msg::Serie(Evento::Abrir))));
        assert!(app.serie.is_some() && app.edicao_menu.is_none());
    }
}
