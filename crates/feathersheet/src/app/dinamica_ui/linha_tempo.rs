//! Linha do tempo (#145, 24/11/2026; referência: a Linha do Tempo do Excel): Inserir ▸ Linha do tempo, com a
//! seleção numa dinâmica, escolhe um campo de data e cria um cartão com os períodos (anos, trimestres, meses ou
//! dias). Um clique escolhe um período; outro clique, noutro período, estende até ele; clicar de novo no período
//! sozinho limpa. O período vira [`Periodo`] nas opções da dinâmica (o motor compara a data, não uma lista de
//! itens, então vale para milhões de linhas). Guardada em `linhasTempo` no JSON da dinâmica.
use super::*;
use feather_ui::icones::icone;
use feathersheet_core::celulas::date_serial::serial_to_ymd;
use feathersheet_core::dinamica::dinamica_layout::Periodo;
use feathersheet_core::funcoes::fn_date::ymd_to_serial;
use pelicano::column;
use serde_json::{json, Value as Json};

const LARGURA_CARTAO: f32 = 380.0;
/// Períodos mostrados no cartão (dias de vários anos dariam milhares de botões).
const MAX_PERIODOS: usize = 400;
/// Linhas lidas para achar a primeira e a última data de uma origem grande.
const AMOSTRA_DATAS: u32 = 50_000;

/// O nível dos botões: anos, trimestres, meses ou dias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nivel { Anos, Trimestres, #[default] Meses, Dias }

impl Nivel {
    const TODOS: [Nivel; 4] = [Nivel::Anos, Nivel::Trimestres, Nivel::Meses, Nivel::Dias];
    fn codigo(self) -> &'static str {
        match self { Nivel::Anos => "anos", Nivel::Trimestres => "trimestres", Nivel::Meses => "meses", Nivel::Dias => "dias" }
    }
    fn de_codigo(t: &str) -> Self {
        Self::TODOS.into_iter().find(|n| n.codigo() == t).unwrap_or_default()
    }
    fn rotulo(self) -> &'static str {
        match self { Nivel::Anos => tr!("Anos"), Nivel::Trimestres => tr!("Trimestres"), Nivel::Meses => tr!("Meses"), Nivel::Dias => tr!("Dias") }
    }
}

/// Uma linha do tempo: o campo de data, o nível, os dias da origem (`min`..`max`) e o período escolhido.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct LinhaTempo {
    pub coluna: usize,
    pub nivel: Nivel,
    pub min: f64,
    pub max: f64,
    pub escolha: Option<(f64, f64)>,
}

impl LinhaTempo {
    pub(super) fn de_json(t: &Json) -> Option<Self> {
        let de = t["de"].as_f64().zip(t["ate"].as_f64());
        Some(LinhaTempo {
            coluna: usize::try_from(t["col"].as_u64()?).ok()?,
            nivel: Nivel::de_codigo(t["nivel"].as_str().unwrap_or("")),
            min: t["min"].as_f64()?, max: t["max"].as_f64()?,
            escolha: de,
        })
    }
    pub(super) fn json(&self) -> Json {
        let mut j = json!({"col": self.coluna, "nivel": self.nivel.codigo(), "min": self.min, "max": self.max});
        if let Some((de, ate)) = self.escolha { j["de"] = json!(de); j["ate"] = json!(ate); }
        j
    }
    /// O filtro do motor, se há período escolhido.
    pub(super) fn periodo(&self) -> Option<Periodo> {
        self.escolha.map(|(de, ate)| Periodo { coluna: self.coluna, de, ate })
    }
}

/// O período do `nivel` que contém o dia `serial`: (primeiro dia, último dia).
fn periodo_contendo(nivel: Nivel, serial: f64) -> (f64, f64) {
    let (y, m, _) = serial_to_ymd(serial.floor() as i64);
    let m = m as i32;
    match nivel {
        Nivel::Anos => (ymd_to_serial(y, 1, 1), ymd_to_serial(y + 1, 1, 1) - 1.0),
        Nivel::Trimestres => { let m0 = (m - 1) / 3 * 3 + 1; (ymd_to_serial(y, m0, 1), ymd_to_serial(y, m0 + 3, 1) - 1.0) }
        Nivel::Meses => (ymd_to_serial(y, m, 1), ymd_to_serial(y, m + 1, 1) - 1.0),
        Nivel::Dias => (serial.floor(), serial.floor()),
    }
}

fn rotulo_periodo(nivel: Nivel, de: f64) -> String {
    const MESES: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
    let (y, m, d) = serial_to_ymd(de as i64);
    let mes = tr!(MESES[(m as usize + 11) % 12]);
    match nivel {
        Nivel::Anos => y.to_string(),
        Nivel::Trimestres => trf!("T{} {}", (m + 2) / 3, y),
        Nivel::Meses => format!("{mes} {y}"),
        Nivel::Dias => format!("{d} {mes} {y}"),
    }
}

/// Os períodos do nível entre `min` e `max` (até [`MAX_PERIODOS`], a partir de `desde`).
fn periodos(nivel: Nivel, min: f64, max: f64, desde: f64) -> Vec<(f64, f64)> {
    let mut saida = Vec::new();
    let mut dia = periodo_contendo(nivel, min.max(desde.min(max))).0;
    while dia <= max && saida.len() < MAX_PERIODOS {
        let p = periodo_contendo(nivel, dia);
        saida.push(p);
        dia = p.1 + 1.0;
    }
    saida
}

/// O clique num período: escolhe, estende a partir de um período sozinho, ou limpa (o mesmo de novo).
fn clicar(nivel: Nivel, escolha: Option<(f64, f64)>, p: (f64, f64)) -> Option<(f64, f64)> {
    match escolha {
        None => Some(p),
        Some(e) if e == p => None,
        Some((de, ate)) if periodo_contendo(nivel, de) == (de, ate) => Some((de.min(p.0), ate.max(p.1))),
        Some(_) => Some(p),
    }
}

/// O primeiro e o último dia com data na coluna. Origem grande: uma amostra espalhada.
// shortcut: acima de AMOSTRA_DATAS linhas a extensão vem de uma amostra (de passo fixo, mais as pontas) e pode não
// ver uma data isolada fora dela; o filtro em si compara todas as linhas. Se pesar, ler a coluna inteira em fundo.
fn extensao(app: &App, e: &Estado, coluna: usize) -> Option<(f64, f64)> {
    let (r0, r1) = (e.origem.r0 + 1, e.origem.r1);
    if r1 < r0 { return None; }
    let passo = ((r1 - r0) / AMOSTRA_DATAS).max(1);
    let mut faixa: Option<(f64, f64)> = None;
    let mut ver = |r: u32| if let Value::Number(n) = app.workbook.valor(e.folha_origem, r, coluna as u32) {
        let n = n.floor();
        faixa = Some(faixa.map_or((n, n), |(a, b)| (a.min(n), b.max(n))));
    };
    for r in (r0..=r1).step_by(passo as usize) { ver(r); }
    for r in r1.saturating_sub(1000).max(r0)..=r1 { ver(r); }
    faixa
}

/// Os campos de data da origem: os de formato data na primeira linha de dados.
fn campos_de_data(app: &App, e: &Estado) -> Vec<(usize, String)> {
    use feathersheet_core::celulas::style::NumberFormat;
    let sheet = app.workbook.aba(e.folha_origem).map(|a| &a.sheet);
    let reais = e.nomes.len().saturating_sub(e.calculados.len());
    e.nomes[..reais].iter().enumerate().map(|(i, n)| (e.origem.c0 as usize + i, n.clone()))
        .filter(|(c, _)| sheet.and_then(|s| s.estilo_resolvido(e.origem.r0 + 1, *c as u32))
            .is_some_and(|st| matches!(st.format, NumberFormat::Date | NumberFormat::Datetime)))
        .collect()
}

/// Um cartão de linha do tempo na tela.
#[derive(Debug, Clone)]
pub(super) struct CartaoTempo {
    destino: usize,
    id: String,
    nome: String,
    linha: LinhaTempo,
}

impl App {
    pub(super) fn evento_linha_tempo(&mut self, evento: Evento) {
        match evento {
            Evento::LinhaTempoInserir => {
                let Some((destino, meta)) = edicao::localizar_na_celula(self, self.folha, self.linha, self.coluna) else {
                    self.status = tr!("Selecione uma célula de uma tabela dinâmica para inserir a linha do tempo.").into();
                    return;
                };
                let e = match edicao::ler_estado(self, destino, &meta) {
                    Ok(e) => e,
                    Err(erro) => { self.status = erro; return; }
                };
                let campos = campos_de_data(self, &e);
                if campos.is_empty() {
                    self.status = tr!("A origem desta dinâmica não tem campo de data (coluna com formato de data).").into();
                    return;
                }
                let id = meta["id"].as_str().unwrap_or_default().to_owned();
                self.segmentacao.escolher_tempo = Some((destino, id, campos));
            }
            Evento::LinhaTempoFecharEscolha => self.segmentacao.escolher_tempo = None,
            Evento::LinhaTempoCampo(coluna) => {
                let Some((destino, id, _)) = self.segmentacao.escolher_tempo.take() else { return };
                self.mudar_linha_tempo(destino, &id, coluna, |app, e, l| {
                    if l.is_none() {
                        match extensao(app, e, coluna) {
                            Some((min, max)) => *l = Some(LinhaTempo { coluna, min, max, ..Default::default() }),
                            None => app.status = tr!("O campo não tem nenhuma data.").into(),
                        }
                    }
                });
            }
            Evento::LinhaTempoNivel(destino, id, coluna, nivel) =>
                self.mudar_linha_tempo(destino, &id, coluna, |_, _, l| if let Some(l) = l { l.nivel = nivel; }),
            Evento::LinhaTempoPeriodo(destino, id, coluna, de, ate) =>
                self.mudar_linha_tempo(destino, &id, coluna, |_, _, l| if let Some(l) = l { l.escolha = clicar(l.nivel, l.escolha, (de, ate)); }),
            Evento::LinhaTempoLimpar(destino, id, coluna) =>
                self.mudar_linha_tempo(destino, &id, coluna, |_, _, l| if let Some(l) = l { l.escolha = None; }),
            Evento::LinhaTempoRemover(destino, id, coluna) =>
                self.mudar_linha_tempo(destino, &id, coluna, |_, _, l| *l = None),
            _ => {}
        }
    }

    /// Lê a dinâmica, muda a linha do tempo do campo `coluna` (`None` = não há / apagar) e recalcula.
    fn mudar_linha_tempo(&mut self, destino: usize, id: &str, coluna: usize,
        mudar: impl FnOnce(&mut App, &Estado, &mut Option<LinhaTempo>)) {
        let Some(meta) = segmentacao::meta_por_id(self, destino, id) else { return };
        let mut e = match edicao::ler_estado(self, destino, &meta) {
            Ok(e) => e,
            Err(erro) => { self.status = erro; return; }
        };
        let pos = e.linhas_tempo.iter().position(|l| l.coluna == coluna);
        let mut atual = pos.map(|i| e.linhas_tempo.remove(i));
        let antes = atual.clone();
        mudar(self, &e, &mut atual);
        if atual == antes && pos.is_some() { return; }
        if let Some(l) = atual { e.linhas_tempo.insert(pos.unwrap_or(e.linhas_tempo.len()), l); }
        self.aplicar_sem_mover(destino, &e);
    }

    /// Os cartões de linha do tempo das dinâmicas da aba ativa.
    pub(super) fn cartoes_tempo(&self, lista: &[Json]) -> Vec<CartaoTempo> {
        let mut cartoes = Vec::new();
        for meta in lista {
            if meta["linhasTempo"].as_array().is_none_or(|a| a.is_empty()) { continue; }
            let Ok(e) = edicao::ler_estado(self, self.folha, meta) else { continue };
            for l in e.linhas_tempo {
                let nome = e.nomes.get(l.coluna.saturating_sub(e.origem.c0 as usize)).cloned().unwrap_or_default();
                cartoes.push(CartaoTempo { destino: self.folha, id: meta["id"].as_str().unwrap_or_default().to_owned(), nome, linha: l });
            }
        }
        cartoes
    }

    /// `FEATHERSHEET_CAPTURAR_DINAMICA=tempo`: vendas de 2025 a 2026 por loja, dinâmica com linha do tempo em
    /// meses, de fevereiro a abril de 2026 escolhidos.
    pub(in crate::app) fn preparar_captura_linha_tempo(&mut self) {
        use feathersheet_core::celulas::estilo_parcial::Campo;
        let lojas = ["Centro", "Norte", "Sul"];
        self.workbook.editar(self.folha, |s| {
            s.set_cell(0, 0, "Data"); s.set_cell(0, 1, "Loja"); s.set_cell(0, 2, "Vendas");
            for i in 0..120u32 {
                let data = ymd_to_serial(2025, 1, 1) + f64::from(i * 5);
                s.set_cell(i + 1, 0, &data.to_string());
                s.set_cell(i + 1, 1, lojas[(i % 3) as usize]);
                s.set_cell(i + 1, 2, &((i * 37) % 90 + 10).to_string());
            }
            s.set_style(&Rect::new(1, 0, 120, 0), &[("format".to_string(), Campo::Texto("date".into()))].into());
        });
        self.trocar_selecao(0, 0);
        self.fim_selecao = (120, 2);
        self.evento_dinamica(Evento::Abrir);
        if let Some(e) = self.dinamica.as_mut() {
            e.linhas = vec![1];
            e.valores = vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }];
        }
        self.evento_dinamica(Evento::Criar);
        self.evento_dinamica(Evento::Fechar);
        let destino = self.folha;
        self.trocar_selecao(1, 0);
        self.evento_dinamica(Evento::LinhaTempoInserir);
        let Some((_, id, _)) = self.segmentacao.escolher_tempo.clone() else { return };
        self.evento_dinamica(Evento::LinhaTempoCampo(0));
        for m in [2, 4] {
            let p = periodo_contendo(Nivel::Meses, ymd_to_serial(2026, m, 1));
            self.evento_dinamica(Evento::LinhaTempoPeriodo(destino, id.clone(), 0, p.0, p.1));
        }
    }

    /// A escolha do campo de data de uma linha do tempo nova, no meio da janela.
    pub(super) fn sobrepor_escolha_tempo<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some((_, _, campos)) = &self.segmentacao.escolher_tempo else { return base };
        let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(2);
        for (coluna, nome) in campos {
            lista = lista.push(button(row![icone("clock", 13.0), text(nome.clone()).size(12)].spacing(8)
                .align(pelicano::Align::Center)).width(Length::FILL).padding([5, 8])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::LinhaTempoCampo(*coluna))));
        }
        let janela = container(column![
            row![
                text(tr!("Linha do tempo: escolha o campo de data")).size(13).width(Length::FILL),
                button(icone("x", 14.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
                    .on_press(Msg::Dinamica(Evento::LinhaTempoFecharEscolha)),
            ].spacing(6).align(pelicano::Align::Center),
            scrollable(lista).height(Length::Shrink),
        ].spacing(8)).padding(12).width(Length::Fixed(300.0)).max_height(320.0).style(estilo_popup_menu);
        pelicano::stack![base, container(janela).center()].height(Length::FILL).into()
    }
}

pub(super) fn cartao<'a>(c: CartaoTempo) -> Element<'a, Msg> {
    let l = &c.linha;
    let ev = |e: Evento| Msg::Dinamica(e);
    let escolhido = match l.escolha {
        None => tr!("Todos os períodos").to_string(),
        Some((de, ate)) => {
            let (a, b) = (rotulo_periodo(l.nivel, de), rotulo_periodo(l.nivel, periodo_contendo(l.nivel, ate).0));
            if a == b { a } else { format!("{a} – {b}") }
        }
    };
    let mut niveis = row(Vec::<pelicano::Element<_>>::new()).spacing(2);
    for n in Nivel::TODOS {
        niveis = niveis.push(button(text(n.rotulo()).size(11)).padding([3, 6]).style(estilo_botao_pequeno(n == l.nivel))
            .on_press(ev(Evento::LinhaTempoNivel(c.destino, c.id.clone(), l.coluna, n))));
    }
    let mut faixa = row(Vec::<pelicano::Element<_>>::new()).spacing(3);
    // A fileira começa dois períodos antes da escolha, para ela aparecer sem rolar.
    let desde = l.escolha.map_or(l.min, |(de, _)| {
        let mut p = periodo_contendo(l.nivel, de);
        for _ in 0..2 { p = periodo_contendo(l.nivel, p.0 - 1.0); }
        p.0
    });
    for p in periodos(l.nivel, l.min, l.max, desde) {
        let marcado = l.escolha.is_some_and(|(de, ate)| p.0 >= de && p.1 <= ate);
        faixa = faixa.push(button(text(rotulo_periodo(l.nivel, p.0)).size(11.5)).padding([6, 8])
            .style(estilo_botao_pequeno(marcado))
            .on_press(ev(Evento::LinhaTempoPeriodo(c.destino, c.id.clone(), l.coluna, p.0, p.1))));
    }
    let cabecalho = row![
        icone("timeline", 14.0),
        text(c.nome.clone()).size(12.5).width(Length::FILL),
        button(icone("eraser", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
            .on_press_maybe(l.escolha.is_some().then(|| ev(Evento::LinhaTempoLimpar(c.destino, c.id.clone(), l.coluna)))),
        button(icone("x", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
            .on_press(ev(Evento::LinhaTempoRemover(c.destino, c.id.clone(), l.coluna))),
    ].spacing(5).align(pelicano::Align::Center);
    container(column![
        cabecalho,
        text(escolhido).size(11.5).color(COR_TEXTO_MUTED()),
        niveis,
        scrollable(faixa).direction(pelicano::Direction::Horizontal).scrollbar_width(3.0).height(Length::Fixed(38.0)),
    ].spacing(6))
        .padding(8).width(Length::Fixed(LARGURA_CARTAO))
        .style(|_| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_PAINEL())),
            border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn periodos_por_nivel_e_clique_que_estende() {
        let d = |y, m, dd| ymd_to_serial(y, m, dd);
        assert_eq!(periodo_contendo(Nivel::Meses, d(2026, 2, 14) + 0.5), (d(2026, 2, 1), d(2026, 2, 28)));
        assert_eq!(periodo_contendo(Nivel::Trimestres, d(2026, 12, 31)), (d(2026, 10, 1), d(2026, 12, 31)));
        assert_eq!(periodo_contendo(Nivel::Anos, d(2024, 2, 29)), (d(2024, 1, 1), d(2024, 12, 31)));
        let meses = periodos(Nivel::Meses, d(2025, 11, 20), d(2026, 2, 3), d(2025, 11, 20));
        assert_eq!(meses.len(), 4, "nov, dez, jan, fev");
        assert_eq!(meses[1], (d(2025, 12, 1), d(2025, 12, 31)));
        assert_eq!(periodos(Nivel::Dias, d(2000, 1, 1), d(2030, 1, 1), d(2000, 1, 1)).len(), MAX_PERIODOS);
        // Clique: escolhe, estende a partir de um período sozinho, recomeça, limpa.
        let (jan, mar) = (periodo_contendo(Nivel::Meses, d(2026, 1, 5)), periodo_contendo(Nivel::Meses, d(2026, 3, 5)));
        let um = clicar(Nivel::Meses, None, mar);
        assert_eq!(um, Some(mar));
        let faixa = clicar(Nivel::Meses, um, jan);
        assert_eq!(faixa, Some((jan.0, mar.1)), "estende para trás também");
        assert_eq!(clicar(Nivel::Meses, faixa, jan), Some(jan), "com faixa, o clique recomeça");
        assert_eq!(clicar(Nivel::Meses, Some(jan), jan), None);
        assert_eq!(rotulo_periodo(Nivel::Trimestres, d(2026, 5, 1)), "T2 2026");
        assert_eq!(rotulo_periodo(Nivel::Meses, d(2026, 5, 1)), "mai 2026");
        let l = LinhaTempo { coluna: 3, nivel: Nivel::Dias, min: 1.0, max: 9.0, escolha: Some((2.0, 4.0)) };
        assert_eq!(LinhaTempo::de_json(&l.json()), Some(l));
    }

    /// A linha do tempo na dinâmica: escolher março deixa só as vendas de março; estender a fevereiro soma
    /// as duas; limpar volta tudo. A data da origem é serial com formato de data.
    #[test]
    fn linha_do_tempo_filtra_a_dinamica_pelo_periodo() {
        use feathersheet_core::celulas::estilo_parcial::Campo;
        let mut app = App::default();
        let d = |m, dd| ymd_to_serial(2026, m, dd);
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Data"); s.set_cell(0, 1, "Loja"); s.set_cell(0, 2, "Valor");
            for (r, (data, loja, v)) in [(d(1, 10), "A", "1"), (d(2, 10), "A", "2"), (d(3, 10), "B", "4"), (d(3, 20), "A", "8")].iter().enumerate() {
                s.set_cell(r as u32 + 1, 0, &data.to_string()); s.set_cell(r as u32 + 1, 1, loja); s.set_cell(r as u32 + 1, 2, v);
            }
            s.set_style(&Rect::new(1, 0, 4, 0), &[("format".to_string(), Campo::Texto("date".into()))].into());
        });
        app.fim_selecao = (4, 2);
        app.evento_dinamica(Evento::Abrir);
        if let Some(e) = app.dinamica.as_mut() {
            e.linhas = vec![1];
            e.valores = vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }];
        }
        app.evento_dinamica(Evento::Criar);
        app.dinamica = None;
        let destino = app.folha;
        let total = |app: &App| (0..8).find_map(|r| {
            let s = &app.workbook.aba(destino).unwrap().sheet;
            (s.get_raw(r, 0) == tr!("Total Geral")).then(|| s.get_raw(r, 1))
        });
        assert_eq!(total(&app).as_deref(), Some("15"));
        app.evento_dinamica(Evento::LinhaTempoInserir);
        let (_, id, campos) = app.segmentacao.escolher_tempo.clone().expect("escolha do campo");
        assert_eq!(campos.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![0], "só a Data é campo de data");
        app.evento_dinamica(Evento::LinhaTempoCampo(0));
        let marco = periodo_contendo(Nivel::Meses, d(3, 1));
        app.evento_dinamica(Evento::LinhaTempoPeriodo(destino, id.clone(), 0, marco.0, marco.1));
        assert_eq!(total(&app).as_deref(), Some("12"), "março: 4 + 8");
        let fev = periodo_contendo(Nivel::Meses, d(2, 1));
        app.evento_dinamica(Evento::LinhaTempoPeriodo(destino, id.clone(), 0, fev.0, fev.1));
        assert_eq!(total(&app).as_deref(), Some("14"), "fevereiro a março");
        let lista: Vec<Json> = serde_json::from_str(app.workbook.aba(destino).unwrap().sheet.dn.as_deref().unwrap()).unwrap();
        let cartoes = app.cartoes_tempo(&lista);
        assert_eq!(cartoes.len(), 1);
        assert_eq!(cartoes[0].nome, "Data");
        let _: Element<'_, Msg> = cartao(cartoes[0].clone());
        app.evento_dinamica(Evento::LinhaTempoLimpar(destino, id.clone(), 0));
        assert_eq!(total(&app).as_deref(), Some("15"));
        app.evento_dinamica(Evento::LinhaTempoRemover(destino, id, 0));
        assert!(app.cartoes_tempo(&serde_json::from_str::<Vec<Json>>(app.workbook.aba(destino).unwrap().sheet.dn.as_deref().unwrap()).unwrap()).is_empty());
        assert!(feather_ui::icones::existe("timeline") && feather_ui::icones::existe("clock"));
    }
}
