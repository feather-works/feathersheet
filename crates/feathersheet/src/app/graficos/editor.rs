//! Galeria inicial, editor com prévia e gráficos persistidos na aba.
use super::*;
use serde_json::json;

fn cartao<'a>(conteudo: impl Into<Element<'a, Msg>>) -> pelicano::Container<'a, Msg> {
    container(conteudo).padding(18).style(|_| pelicano::Style {
        background: Some(pelicano::Background::Color(feather_ui::tema::COR_PAINEL())),
        text_color: Some(feather_ui::tema::COR_TEXTO()),
        border: pelicano::Border {
            color: feather_ui::tema::COR_BORDA(),
            width: 1.,
            radius: [12.0; 4],
        },
        ..Default::default()
    })
}
/// Renderizador único de embeds: obtém os números da engine colunar do
/// FeatherSheet e só aplica estilo/desenho no Charts compartilhado.
/// Não escreve na aba, não armazena caches serializados e não agrega.
/// Quadro no lugar do gráfico que o motor não desenha.
fn previa_indisponivel() -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"200\" viewBox=\"0 0 480 200\">\
         <rect x=\"0.5\" y=\"0.5\" width=\"479\" height=\"199\" fill=\"#ffffff\" stroke=\"#dadce0\"/>\
         <text x=\"240\" y=\"104\" text-anchor=\"middle\" font-size=\"14\" fill=\"#5f6368\" \
         font-family=\"Noto Sans, DejaVu Sans, sans-serif\">{}</text></svg>",
        tr!("Prévia indisponível para esses dados ou tipo.")
    )
}

/// Sem cache: os testes comparam com a prévia lida na hora.
#[cfg(test)]
pub(super) fn svg_do_grafico_salvo(
    sheet: &feathersheet_core::Sheet,
    valor: &serde_json::Value,
) -> Option<String> {
    desenhar_grafico_salvo(sheet, valor, None).map(|d| d.svg)
}

/// O SVG do gráfico salvo, reaproveitando os números já lidos da aba.
pub(super) fn svg_do_grafico_salvo_em(
    sheet: &feathersheet_core::Sheet,
    valor: &serde_json::Value,
    fonte: Option<super::Fonte<'_>>,
) -> Option<String> {
    desenhar_grafico_salvo(sheet, valor, fonte).map(|d| d.svg)
}

/// O desenho de um gráfico salvo e, nos tipos do desenhista novo, o mapa das
/// partes clicáveis e os dados (para a dica do mouse e o clique na legenda).
pub(super) struct Desenhado {
    pub svg: String,
    pub mapa: Option<feather_charts::formatado::MapaGrafico>,
    pub grafico: Option<Grafico>,
    pub estilo: Option<feather_charts::estilo::EstiloGrafico>,
}

/// Troca do desenho ao ligar/desligar um item da legenda, "atravessando o
/// fundo": na 1ª metade o gráfico antigo some sobre o fundo do próprio
/// gráfico; na 2ª o novo aparece. Os dois nunca ficam sobrepostos — antes,
/// com escalas e larguras de barra diferentes, o meio da animação mostrava
/// dois gráficos tremidos um sobre o outro.
fn cruzar(antes: &str, depois: &str, t: f32) -> String {
    let medida = |nome: &str| -> f32 {
        let chave = format!("{nome}=\"");
        depois.find(&chave).and_then(|i| depois[i + chave.len()..].split('"').next())
            .and_then(|v| v.parse().ok()).unwrap_or(480.0)
    };
    let (w, h) = (medida("width"), medida("height"));
    // O fundo do gráfico novo, sem sombra; com degradê (precisa das defs),
    // fica sem fundo fixo.
    let fundo = depois.find("<rect class=\"area-grafico\"")
        .and_then(|i| depois[i..].find("/>").map(|f| &depois[i..i + f + 2]))
        .map(|r| match r.find(" filter=\"url(") {
            Some(i) => format!("{}{}", &r[..i], r[i + 1..].split_once('"').and_then(|(_, x)| x.split_once('"')).map_or("/>", |(_, x)| x)),
            None => r.to_owned(),
        })
        .filter(|r| !r.contains("url("))
        .unwrap_or_default();
    let suave = |x: f32| { let x = x.clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) };
    let (op_antes, op_depois) = if t < 0.5 { (1.0 - suave(t * 2.0), 0.0) } else { (0.0, suave(t * 2.0 - 1.0)) };
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\
{fundo}<g opacity=\"{op_antes:.3}\">{antes}</g><g opacity=\"{op_depois:.3}\">{depois}</g></svg>")
}

/// Itens desligados pela legenda em gráfico sem estilo próprio.
fn aplicar_desligados(e: &mut feather_charts::estilo::EstiloGrafico, valor: &serde_json::Value) {
    let lista: Vec<usize> = serde_json::from_value(valor[super::interacao::CAMPO_DESLIGADOS].clone()).unwrap_or_default();
    for i in lista {
        e.legenda.itens.entry(i).or_default().desligado = true;
    }
}

/// Células que um gráfico lê, ponto a ponto, sem resumir.
pub(super) const MAX_CELULAS_GRAFICO: u64 = 200_000;
/// Resumindo (soma, média, contagem…), o gráfico mostra no máximo 10 mil
/// categorias, mas lê todas as linhas: o teto é o que cabe ler em ~1 s.
pub(super) const MAX_CELULAS_RESUMO: u64 = 100_000_000;

/// Com resumo, o intervalo para na última linha/coluna com dados: a coluna
/// inteira (até a linha 100 milhões) viraria milhões de categorias vazias.
/// Devolve as células que sobram, ou `None` se passam do teto.
pub(super) fn cortar_intervalo(sheet: &feathersheet_core::Sheet, range: &mut ChartRange, resumir: bool) -> Option<u64> {
    if resumir && !sheet.is_empty() {
        range.r1 = range.r1.min(sheet.max_row.max(range.r0));
        range.c1 = range.c1.min(sheet.max_col.max(range.c0));
    }
    let linhas = u64::from(range.r1.checked_sub(range.r0)?) + 1;
    let colunas = u64::from(range.c1.checked_sub(range.c0)?) + 1;
    let celulas = linhas.checked_mul(colunas)?;
    (celulas <= if resumir { MAX_CELULAS_RESUMO } else { MAX_CELULAS_GRAFICO }).then_some(celulas)
}

/// `fonte`: de onde os números vêm, para não reler a aba inteira quando nada
/// mudou (ver [`super::CacheDados`]); `None` lê sempre.
pub(super) fn desenhar_grafico_salvo(
    sheet: &feathersheet_core::Sheet,
    valor: &serde_json::Value,
    fonte: Option<super::Fonte<'_>>,
) -> Option<Desenhado> {
    let mut config = serde_json::from_value::<ChartConfig>(valor.clone()).ok()?;
    let resumir = config.aggregate;
    cortar_intervalo(sheet, &mut config.range, resumir)?;
    // O extractor de pizza antigo pode reduzir a primeira série antes
    // de o painel escolher outra. A extração como colunas preserva TODAS
    // as séries; o Charts só recebe a série selecionada no passo seguinte.
    let pilha = super::empilhamento(config.tipo);
    if let Some((base, _)) = pilha {
        config.tipo = base;
    }
    let pizza = config.tipo == ChartType::Pie;
    if pizza {
        config.tipo = ChartType::Column;
    }
    // Tipos especiais (degraus empilhados, radar, cascata, Gantt…): o mesmo
    // desenhista novo da prévia, no tamanho do quadro.
    if !matches!(config.tipo,
        ChartType::Column | ChartType::Bar | ChartType::Line |
        ChartType::Area | ChartType::StepLine | ChartType::Scatter
    ) && !pizza {
        let aparencia: Aparencia =
            serde_json::from_value(valor["_featherNative"].clone()).unwrap_or_default();
        let mut dados = (*super::dados_do_grafico(sheet, &config, false, fonte)).clone();
        super::renomear_series_dados(&mut dados, &config.style.hidden_series, &aparencia);
        super::ordenar_papeis(&mut dados, config.tipo, &config.style.hidden_series, &aparencia.papeis);
        let medida = |k: &str, padrao: f64| valor["position"][k].as_f64().unwrap_or(padrao).clamp(120., 3200.) as f32;
        // O estilo já com os itens desligados pela legenda, antes do desenho.
        let mut aparencia = aparencia;
        let mut estilo = aparencia.estilo.take()
            .unwrap_or_else(|| super::painel::migrar(&aparencia, valor["_excel"].as_object()));
        aplicar_desligados(&mut estilo, valor);
        aparencia.estilo = Some(estilo);
        let (svg, mapa, g, estilo) = super::desenhar_especial(&config, &dados, &aparencia, None, medida("width", 480.), medida("height", 300.)).ok()?;
        return Some(Desenhado { svg, mapa: Some(mapa), grafico: Some(g), estilo: Some(estilo) });
    }
    // Gráfico do Excel: célula vazia na série é lacuna lá; o motor comum
    // recusa lacuna, então vale 0 (categoria e cabeçalho ficam como estão).
    let lacuna_zero = valor["_excel"].is_object();
    let dados = super::dados_para_desenho(sheet, &config, lacuna_zero, fonte);
    let mut contrato = feathersheet_io::grafico_compartilhado::preparar_grafico(&config, (*dados).clone()).ok()?;
    let aparencia: Aparencia =
        serde_json::from_value(valor["_featherNative"].clone()).unwrap_or_default();
    super::aplicar_nomes_origem(&mut contrato, &aparencia);
    if pizza {
        contrato.grafico.tipo = Tipo::Pizza;
        let indice = valor["_featherPieSeries"]
            .as_str()
            .and_then(|s| s.split(" — ").next())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let escolhida = contrato.grafico.series.get(indice)?.clone();
        contrato.grafico.series = vec![escolhida];
    }
    let nativa = valor["_featherNative"].is_object();
    let aparencia = match pilha {
        Some((_, emp)) => super::aparencia_empilhada(&aparencia, emp),
        None => aparencia,
    };
    if nativa {
        // Reaplica cores primárias, overrides e nomes EXATAMENTE como
        // a prévia. Sem essa chamada, editar um gráfico salvo alterava
        // a aparência durante o redraw, apesar da configuração persistida.
        aplicar_aparencia_grafico(&mut contrato.grafico, &aparencia).ok()?;
    }
    // Todos os tipos básicos saem do desenhista novo, no tamanho do quadro:
    // com o estilo do painel ou, sem ele, o convertido da aparência antiga
    // (fundo, contorno e cores de texto do `.xlsx` inclusive). Assim a dica
    // do mouse e o clique na legenda valem para qualquer gráfico.
    let medida = |k: &str, padrao: f64| valor["position"][k].as_f64().unwrap_or(padrao).clamp(120., 3200.) as f32;
    let (w, h) = (medida("width", 480.), medida("height", 300.));
    let mut estilo = aparencia.estilo.clone()
        .unwrap_or_else(|| super::painel::migrar(&aparencia, valor["_excel"].as_object()));
    aplicar_desligados(&mut estilo, valor);
    let (svg, mapa) = contrato.grafico.exportar_svg_formatado_com_mapa(w, h, &estilo).ok()?;
    Some(Desenhado { svg, mapa: Some(mapa), grafico: Some(contrato.grafico), estilo: Some(estilo) })
}

impl App {
    pub(in crate::app) fn sobrepor_galeria_graficos<'a>(
        &'a self,
        base: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        if !self.graficos.seletor_aberto {
            return base;
        }
        // A galeria abre imediatamente abaixo do botão Gráfico, e NÃO
        // abaixo de toda a Ribbon (que inclui legenda e pista de rolagem).
        // O X do clique continua congelado para o popup não seguir o mouse.
        let topo = feather_ui::cromo::BARRA_ALTURA
            + (if self.config.barra_propria { 2.0 } else { 1.0 });
        let abas = 44.0 + 8.0; // scroll das abas + padding vertical
        let abaixo_botao = crate::geometria_overlay::abaixo_botao_grafico(
            topo, abas, self.config.faixa_recolhida,
        );
        let Some(pos) = crate::geometria_overlay::galeria_graficos(
            self.graficos.ancora_seletor_x,
            self.tamanho_principal.width,
            self.tamanho_principal.height,
            abaixo_botao,
            seletor_tipo::LARGURA_POPOVER,
            seletor_tipo::ALTURA_POPOVER,
        ) else { return base; };
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::Grafico(Evento::FecharSeletor));
        let painel = cartao(seletor_tipo::painel(self.graficos.tipo))
            .padding(6)
            .width(Length::Fixed(pos.largura))
            .height(Length::Fixed(pos.altura));
        pelicano::stack![
            base,
            fora,
            pelicano::column![
                Space::new(0.0, Length::Fixed(pos.y)),
                pelicano::row![
                    Space::new(Length::Fixed(pos.x), 0.0),
                    painel
                ]
            ]
        ]
        .height(Length::FILL)
        .into()
    }
    /// O JSON do gráfico com o que está no editor. Mesclado no gravado
    /// (`graficos_salvos::mesclado`), preserva `_excel`, `sourceSheet` e o resto.
    fn json_do_editor(&self, id: &str) -> Result<serde_json::Value, String> {
        let g = &self.graficos;
        let r = parse_range_ref(&g.intervalo).map(|r| r.normalized())
            .ok_or(tr!("Intervalo inválido. Use A1:C10."))?;
        let tipo = if g.tipo == TipoUi::Pizza {
            "pie".to_owned()
        } else {
            serde_json::to_value(g.tipo.motor()).ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| "column".to_owned())
        };
        let a = &g.aparencia;
        let dimensao = |s: &str| {
            s.parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && (100.0..=3200.0).contains(n))
        };
        let mut valor = json!({"id":id,"type":tipo,"title":g.titulo,"range":r,"seriesInColumns":g.colunas,"hasHeaders":g.cabecalho,"hasCategoryLabels":g.categorias,"aggregate":g.agrupar,"aggregateFn":"sum","style":{"showTitle":a.titulo,"subtitle":a.subtitulo,"legendPosition":if a.legenda{"bottom"}else{"none"},"dataLabels":a.valores,"seriesColors":{"0":a.cor}},"_featherNative":a,"_featherPieSeries":g.serie});
        for (i, c) in &a.cores {
            valor["style"]["seriesColors"][i.to_string()] = json!(c);
        }
        // Aba Configurar: coluna das categorias, séries ocultas e a conta de cada uma.
        valor["categoryColumnIndex"] = json!(g.coluna_categoria);
        valor["style"]["hiddenSeries"] = json!(g.ocultas);
        valor["aggregateBySeries"] = serde_json::to_value(&g.contas).unwrap_or_default();
        if let Some(e) = g.eixos3d {
            valor["eixos3d"] = json!(e);
        }
        if let Some(f) = g.furo.filter(|_| g.tipo == TipoUi::Rosca) {
            valor["holeSize"] = json!(f);
        }
        if let Some(d) = &g.dinamica {
            valor["pivotSource"] = json!(d);
        }
        if let Some(e) = &a.estilo {
            // O gravador `.xlsx` lê as cores daqui: as que se veem (tema/estilo).
            for i in 0..g.series.len() {
                let c = super::painel::cor_resolvida_serie(e, &g.painel.cores_base, i);
                valor["style"]["seriesColors"][i.to_string()] = json!(c.texto());
            }
        }
        let salvo = self.indice_aba_grafico()
            .and_then(|i| self.workbook.aba(i))
            .and_then(|aba| feathersheet_io::graficos_salvos::listar(&aba.sheet).ok())
            .and_then(|l| l.into_iter().find(|v| v["id"].as_str() == Some(id)));
        // Gráfico novo nasce na parte da folha que está à vista (era sempre
        // perto de A1: rolado até a linha 900 mil, ele sumia lá no topo).
        let (x0, y0) = match self.indice_aba_grafico().filter(|&i| i == self.folha).and_then(|i| self.workbook.aba(i)) {
            Some(aba) => super::posicionamento::scroll_em_pixels(&aba.sheet, self.linha_inicio, self.coluna_inicio),
            None => (0.0, 0.0),
        };
        let mut posicao = salvo.as_ref().map(|v| v["position"].clone())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| json!({"x": x0 + 80.0, "y": y0 + 80.0, "width": 480, "height": 300}));
        // Medida fora da faixa (digitando) não estraga o quadro gravado.
        if let (Some(largura), Some(altura)) = (dimensao(&a.largura), dimensao(&a.altura)) {
            posicao["width"] = json!(largura);
            posicao["height"] = json!(altura);
        }
        valor["position"] = posicao;
        Ok(valor)
    }

    /// Índice da aba onde o gráfico fica (a dos dados, se não houver outra).
    pub(super) fn indice_aba_grafico(&self) -> Option<usize> {
        let alvo = self.graficos.aba_grafico.or(self.graficos.aba_id)?;
        (0..self.workbook.len()).find(|&i| self.workbook.aba(i).is_some_and(|a| a.id == alvo))
    }

    /// Grava o editor no gráfico da planilha AO VIVO: cada edição é um passo
    /// de Ctrl+Z; letras seguidas no mesmo campo (`campo`) juntam num passo.
    pub(super) fn salvar_grafico_ao_vivo(&mut self, campo: Option<&'static str>) {
        let Some(id) = self.graficos.id.clone() else { return };
        let juntar = campo.is_some() && campo == self.graficos.ultimo_campo;
        self.graficos.ultimo_campo = campo;
        let Ok(valor) = self.json_do_editor(&id) else { return };
        let Some(indice) = self.indice_aba_grafico() else { return };
        // Só o JSON do gráfico muda: os números já lidos da aba continuam
        // valendo (ver `CacheDados::renovar`).
        let antes = self.geracao_da_aba(indice);
        let Some(aba) = self.workbook.aba_mut(indice) else { return };
        match feathersheet_io::graficos_salvos::mesclado(&aba.sheet, &id, valor) {
            Ok(novo) => {
                if aba.sheet.alterar_graficos(Some(novo), juntar) {
                    self.alterado = true;
                }
            }
            Err(e) => self.graficos.mensagem = e,
        }
        self.renovar_dados_da_aba(indice, antes);
    }

    pub(super) fn inserir_grafico(&mut self) -> Task<Msg> {
        self.atualizar_grafico();
        if self.graficos.svg.is_none() && self.graficos.id.is_none() {
            return Task::none();
        }
        let id = self.graficos.id.clone().unwrap_or_else(|| {
            format!(
                "native-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        });
        let valor = match self.json_do_editor(&id) {
            Ok(v) => v,
            Err(e) => {
                self.graficos.mensagem = e;
                return Task::none();
            }
        };
        let Some(indice) = self.indice_aba_grafico() else {
            self.graficos.mensagem = tr!("A aba de origem foi removida.").into();
            return Task::none();
        };
        let aba = self.workbook.aba_mut(indice).unwrap();
        match feathersheet_io::graficos_salvos::mesclado(&aba.sheet, &id, valor) {
            Ok(novo) => { aba.sheet.alterar_graficos(Some(novo), false); }
            Err(e) => {
                self.graficos.mensagem = e;
                return Task::none();
            }
        }
        self.graficos.ultimo_campo = None;
        self.alterado = true;
        self.status = tr!("Gráfico salvo na planilha.").into();
        self.graficos.id = Some(id);
        self.graficos.aberto = true;
        self.atualizar_grafico();
        Task::none()
    }
    /// Depois de Ctrl+Z/Ctrl+Y com o painel aberto: o painel volta a mostrar
    /// o gráfico gravado (ou fecha, se o gráfico deixou de existir).
    pub(in crate::app) fn recarregar_grafico_aberto(&mut self) {
        if !self.graficos.aberto { return; }
        let Some(id) = self.graficos.id.clone() else { return };
        let existe = self.workbook.aba(self.folha)
            .and_then(|a| feathersheet_io::graficos_salvos::listar(&a.sheet).ok())
            .is_some_and(|l| l.iter().any(|v| v["id"].as_str() == Some(id.as_str())));
        if existe {
            let antes = std::mem::take(&mut self.graficos.painel);
            let _ = self.editar_grafico_salvo(&id);
            let p = &mut self.graficos.painel;
            (p.grupo, p.eixo, p.serie, p.ponto, p.area) = (antes.grupo, antes.eixo, antes.serie, antes.ponto, antes.area);
        } else {
            self.graficos.aberto = false;
            self.graficos.id = None;
            self.graficos.selecionado = None;
        }
    }

    pub(super) fn editar_grafico_salvo(&mut self, id: &str) -> Task<Msg> {
        let Some(aba) = self.workbook.aba(self.folha) else {
            return Task::none();
        };
        let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) else {
            return Task::none();
        };
        let Some(v) = lista.iter().find(|v| v["id"].as_str() == Some(id)) else {
            return Task::none();
        };
        let c = match serde_json::from_value::<ChartConfig>(v.clone()) {
            Ok(c) => c,
            // Antes saía calado: o duplo clique parecia não fazer nada.
            Err(e) => {
                self.status = trf!("Este gráfico não pode ser editado aqui: {}", e);
                return Task::none();
            }
        };
        let tipo = match c.tipo {
            ChartType::Column => TipoUi::Colunas,
            ChartType::ColumnStacked => TipoUi::ColunasEmpilhadas,
            ChartType::ColumnStacked100 => TipoUi::Colunas100,
            ChartType::Bar => TipoUi::Barras,
            ChartType::BarStacked => TipoUi::BarrasEmpilhadas,
            ChartType::BarStacked100 => TipoUi::Barras100,
            ChartType::Line => TipoUi::Linhas,
            ChartType::Line3d => TipoUi::Linha3d,
            ChartType::StepLine => TipoUi::Degraus,
            ChartType::StepLineStacked => TipoUi::DegrausEmpilhados,
            ChartType::StepLineStacked100 => TipoUi::Degraus100,
            ChartType::Area => TipoUi::Area,
            ChartType::StepArea => TipoUi::AreaDegraus,
            ChartType::StepAreaStacked => TipoUi::AreaDegrausEmpilhada,
            ChartType::StepAreaStacked100 => TipoUi::AreaDegraus100,
            ChartType::Pie => TipoUi::Pizza,
            ChartType::Doughnut => TipoUi::Rosca,
            ChartType::Scatter => TipoUi::Dispersao,
            ChartType::Bubble => TipoUi::Bolhas,
            ChartType::Combo => TipoUi::Combinado,
            ChartType::Heatmap => TipoUi::MapaCalor,
            ChartType::Waterfall => TipoUi::Cascata,
            ChartType::Histogram => TipoUi::Histograma,
            ChartType::Radar => TipoUi::Radar,
            ChartType::Candlestick => TipoUi::Candlestick,
            ChartType::Gantt => TipoUi::Gantt,
            ChartType::Outro => {
                self.status = tr!("Tipo de gráfico não reconhecido.").into();
                return Task::none();
            }
        };
        // A alça atualiza position no .fshz; o editor deve mostrar as
        // dimensões persistidas mesmo quando _featherNative é legado.
        let mut aparencia: Aparencia =
            serde_json::from_value(v["_featherNative"].clone()).unwrap_or_default();
        for (campo, destino) in [
            ("width", &mut aparencia.largura),
            ("height", &mut aparencia.altura),
        ] {
            if let Some(n) = v["position"][campo].as_f64().filter(|n| n.is_finite()) {
                *destino = format!("{n:.0}");
            }
        }
        // Gráfico do `.xlsx` com os dados em outra aba (`sourceSheet`): a
        // prévia lê de lá; o gráfico continua gravado na aba onde aparece.
        let aba_dados = v["sourceSheet"].as_str()
            .and_then(|n| self.workbook.indice_por_nome(n))
            .and_then(|i| self.workbook.aba(i))
            .map_or(aba.id, |a| a.id);
        let copiado = self.graficos.copiado.take();
        // O que já foi lido e desenhado continua valendo.
        let dados = std::mem::take(&mut self.graficos.dados);
        let svgs = std::mem::take(&mut self.graficos.svgs);
        self.graficos = Estado {
            copiado,
            dados,
            svgs,
            aberto: true,
            id: Some(id.into()),
            aba_grafico: Some(aba.id),
            intervalo: format_ref(&feathersheet_core::celulas::refs::Rect::new(
                c.range.r0, c.range.c0, c.range.r1, c.range.c1,
            )),
            titulo: c.title,
            tipo,
            cabecalho: c.has_headers,
            categorias: c.has_category_labels,
            colunas: c.series_in_columns,
            agrupar: c.aggregate,
            coluna_categoria: c.category_column_index.unwrap_or(0),
            ocultas: c.style.hidden_series.clone(),
            contas: c.aggregate_by_series.clone(),
            eixos3d: c.eixos_3d,
            furo: c.hole_size,
            aba_id: Some(aba_dados),
            aparencia,
            serie: v["_featherPieSeries"].as_str().map(str::to_string),
            ..Default::default()
        };
        self.graficos.painel.excel = v["_excel"].as_object().cloned();
        self.atualizar_grafico();
        Task::none()
    }
    /// A chave do SVG de um gráfico na camada: aba dos dados, versão dela,
    /// tema e o JSON do gráfico.
    fn chave_svg_camada(&self, indice: usize, v: &serde_json::Value) -> String {
        format!("{indice}|{}|{}|{}", self.workbook.versao(indice), feather_ui::tema::escuro(), v)
    }

    /// Guarda o SVG de um gráfico para a camada não desenhá-lo de novo.
    pub(super) fn guardar_svg_camada(&self, indice: usize, v: &serde_json::Value, svg: &str) {
        let mut cache = self.graficos.svgs.borrow_mut();
        if cache.len() > 64 { cache.clear(); }
        cache.insert(self.chave_svg_camada(indice, v), svg.to_owned());
    }

    /// Os gráficos da aba ativa para a camada de objetos desenhar:
    /// `(z, retângulo, svg)`. Os do `.xlsx` vêm na ordem do desenho do
    /// Excel (`_excel.z`); os criados no app ficam por cima de tudo.
    pub(in crate::app) fn graficos_da_camada(&self) -> Vec<(u64, feathersheet_io::objetos_aba::Retangulo, String)> {
        self.graficos_da_aba(self.folha)
    }

    /// Os gráficos de uma aba qualquer (a ativa é a [`Self::graficos_da_camada`]).
    pub(in crate::app) fn graficos_da_aba(&self, folha: usize) -> Vec<(u64, feathersheet_io::objetos_aba::Retangulo, String)> {
        let Some(aba) = self.workbook.aba(folha) else { return Vec::new() };
        let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) else { return Vec::new() };
        let mut saida = Vec::new();
        for v in lista {
            let Some(quadro) = Quadro::da_posicao(&v["position"]) else { continue };
            let indice = v["sourceSheet"].as_str()
                .and_then(|n| self.workbook.indice_por_nome(n))
                .filter(|i| self.workbook.aba(*i).is_some())
                .unwrap_or(folha);
            let dados = self.workbook.aba(indice).map_or(&aba.sheet, |a| &a.sheet);
            // O editor grava ao vivo: o gráfico aberto se desenha do mesmo
            // jeito que fechado (antes, a prévia do editor lia a aba errada no
            // gráfico do `.xlsx` e ele sumia ao abrir "Editar gráfico…").
            // Desenhar é caro (dados, escalas, SVG): só quando algo mudou, não
            // a cada quadro da rolagem.
            let chave = self.chave_svg_camada(indice, &v);
            let guardado = self.graficos.svgs.borrow().get(&chave).cloned();
            let svg = match guardado {
                Some(svg) => svg,
                None => {
                    let svg = svg_do_grafico_salvo_em(dados, &v, self.fonte_dados(indice))
                        .unwrap_or_else(previa_indisponivel);
                    self.guardar_svg_camada(indice, &v, &svg);
                    svg
                }
            };
            // Liga/desliga pela legenda: o desenho novo aparece por cima do
            // antigo (esmaecimento cruzado), sem o eixo balançar no meio.
            let svg = match v["id"].as_str().and_then(|id| self.transicao_do_grafico(id)) {
                Some((item, t)) if t < 1.0 => {
                    let (antes, _, _) = super::interacao::com_item_alternado(&v, item);
                    // A legenda não muda os números: o cache poupa reler a
                    // aba a cada quadro da transição.
                    match svg_do_grafico_salvo_em(dados, &antes, self.fonte_dados(indice)) {
                        Some(velho) => cruzar(&velho, &svg, t),
                        None => svg,
                    }
                }
                _ => svg,
            };
            let ret = feathersheet_io::objetos_aba::Retangulo::novo(
                quadro.x as f32, quadro.y as f32, quadro.largura as f32, quadro.altura as f32,
            );
            saida.push((v["_excel"]["z"].as_u64().unwrap_or(u64::MAX), ret, svg));
        }
        saida
    }

    pub(in crate::app) fn com_graficos_salvos<'a>(
        &'a self,
        grade: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        if self.pendente.is_some() || self.contexto.is_some() {
            return grade;
        }
        let Some(aba) = self.workbook.aba(self.folha) else {
            return grade;
        };
        let Ok(lista) = feathersheet_io::graficos_salvos::listar(&aba.sheet) else {
            return grade;
        };

        // A posição é absoluta na FOLHA, e não na janela. A grade Iced possui
        // cabeçalhos próprios, scroll por índices de célula e zoom separado.
        let zoom = (self.config.zoom as f64 / 100.).max(0.01);
        let scroll = posicionamento::scroll_em_pixels(
            &aba.sheet, self.linha_inicio, self.coluna_inicio,
        );
        let mut graficos = pelicano::stack![
            Space::new(Length::FILL, Length::FILL)
        ].width(Length::FILL).height(Length::FILL);

        for v in lista {
            let Some(id) = v["id"].as_str().map(str::to_owned) else {
                continue;
            };
            let Some(quadro) = Quadro::da_posicao(&v["position"]) else {
                continue;
            };

            let (x, y) = posicionamento::origem_tela(quadro, scroll, zoom);
            // O pelicano não posiciona filhos em coordenada negativa: com o
            // gráfico cortado pela borda de cima/esquerda, a área de clique
            // cobre só a parte que aparece (#112); o desenho já vem recortado
            // pela camada de objetos.
            let Some((x, y, largura, altura, corte_x, corte_y)) = posicionamento::parte_visivel(
                x, y,
                (quadro.largura * zoom).clamp(120., 3200.),
                (quadro.altura * zoom).clamp(100., 3200.),
            ) else {
                continue;
            };
            let (largura, altura) = (largura as f32, altura as f32);
            let (corte_x, corte_y) = (corte_x as f32, corte_y as f32);

            // O desenho de todo gráfico sai na camada de objetos
            // (`graficos_da_camada`, intercalado com as formas e recortado
            // pela grade); aqui fica só a área de clique: selecionar e
            // arrastar pelo corpo, duplo clique edita, botão direito abre o menu.
            let selecionado = self.graficos.selecionado.as_deref() == Some(id.as_str())
                || (self.graficos.aberto && self.graficos.id.as_deref() == Some(id.as_str()));
            // A roda sobre o gráfico rola a planilha (Ctrl+roda dá zoom),
            // como no Excel: a área de clique esconde o cursor da grade.
            let ctrl = self.ctrl_pressionado;
            let sobre_legenda = self.graficos.interacao.pairar.as_ref()
                .is_some_and(|p| p.id == id && p.legenda.is_some());
            let id_mover = id.clone();
            // O ponto do `Pairar` é relativo ao gráfico (o `on_move` do pelicano dá o da janela).
            let item = pelicano::mouse_area(crate::compat::ao_mover_relativo(
                Space::new(Length::FILL, Length::FILL),
                move |p| Msg::Grafico(Evento::Pairar(id_mover.clone(), pelicano::Point::new(p.x + corte_x, p.y + corte_y))),
            ))
                .on_exit(Msg::Grafico(Evento::SairDoGrafico(id.clone())))
                .on_press(Msg::Grafico(Evento::Arrastar(id.clone())))
                .on_right_press(Msg::Grafico(Evento::Contexto(id.clone())))
                .on_scroll(move |dx, dy, mods| {
                    // Pixels do pelicano; o Iced contava em linhas (≈ 40 px).
                    let y = dy / 40.0;
                    if ctrl && y.abs() >= 0.1 {
                        Msg::Roda(y.signum() as i32)
                    } else {
                        crate::app::botoes_macro::rolar_pela_roda(dx, dy, mods)
                    }
                })
                .cursor_icon(if sobre_legenda { pelicano::CursorIcon::Pointer } else { pelicano::CursorIcon::Grab });

            // Conteúdo opaco e CLIPADO dentro da própria borda do gráfico:
            // SVG não deve ultrapassar o retângulo, nem consumir os handles.
            let painel = container(item)
                .width(Length::Fixed(largura))
                .height(Length::Fixed(altura))
                .style(move |_| pelicano::Style {
                    border: pelicano::Border {
                        color: feather_ui::tema::COR_AZUL_ACENTO(),
                        width: if selecionado { 2. } else { 0. },
                        radius: [0.0; 4],
                    },
                    ..Default::default()
                });

            let conteudo: Element<'_, Msg> = if selecionado {
                // Oito alças (barras nos lados, "L" nos cantos) sobrepostas às
                // bordas sem adicionar largura/altura ao gráfico no .fshz.
                let cantos = pelicano::column![
                    pelicano::row![
                        alca_grafico(&id, Alca::Noroeste),
                        Space::new(Length::FILL, 0.0),
                        alca_grafico(&id, Alca::Norte),
                        Space::new(Length::FILL, 0.0),
                        alca_grafico(&id, Alca::Nordeste),
                    ],
                    Space::new(0.0, Length::FILL),
                    pelicano::row![
                        alca_grafico(&id, Alca::Oeste),
                        Space::new(Length::FILL, 0.0),
                        alca_grafico(&id, Alca::Leste),
                    ].align(pelicano::Align::Center),
                    Space::new(0.0, Length::FILL),
                    pelicano::row![
                        alca_grafico(&id, Alca::Sudoeste),
                        Space::new(Length::FILL, 0.0),
                        alca_grafico(&id, Alca::Sul),
                        Space::new(Length::FILL, 0.0),
                        alca_grafico(&id, Alca::Sudeste),
                    ].align(pelicano::Align::End)
                ].width(Length::Fixed(largura)).height(Length::Fixed(altura));
                pelicano::stack![painel, cantos].into()
            } else {
                painel.into()
            };
            // Dica do ponto sob o mouse, por cima de tudo (sem pegar o clique).
            let conteudo: Element<'_, Msg> = match self.dica_do_grafico(&id, largura, altura, (corte_x, corte_y)) {
                Some(dica) => pelicano::stack![conteudo, dica].into(),
                None => conteudo,
            };
            graficos = graficos.push(
                pelicano::column![
                    Space::new(0.0, Length::Fixed(y as f32)),
                    pelicano::row![
                        Space::new(Length::Fixed(x as f32), 0.0),
                        conteudo
                    ],
                ]
            );
        }

        // A área dos gráficos NÃO inclui os cabeçalhos; a máscara impede que
        // o SVG e as alças invadam bordas da grade, barra de abas e editor.
        // Sem cobrir as barras de rolagem da grade (direita e embaixo): as
        // alças de um gráfico perto da borda ficavam por cima delas.
        let barra = crate::grade::barras::ESPESSURA;
        let celulas = pelicano::column![
            pelicano::row![
                container(graficos).width(Length::FILL).height(Length::FILL).clip(true),
                Space::new(Length::Fixed(barra), 0.0),
            ].height(Length::FILL),
            Space::new(0.0, Length::Fixed(barra)),
        ].width(Length::FILL).height(Length::FILL);
        let cobertura = pelicano::column![
            Space::new(0.0, Length::Fixed(
                crate::grade::CABECALHO_COLUNA * zoom as f32
            )),
            pelicano::row![
                Space::new(Length::Fixed(
                    crate::grade::CABECALHO_LINHA * zoom as f32
                ), 0.0),
                celulas
            ].width(Length::FILL).height(Length::FILL)
        ].width(Length::FILL).height(Length::FILL);
        let sobre_grade = container(pelicano::stack![grade, cobertura])
            .width(Length::FILL)
            .height(Length::FILL)
            .clip(true);
        let mut linha = pelicano::row![sobre_grade].height(Length::FILL);
        if self.graficos.aberto {
            linha = linha.push(self.view_graficos());
        }
        if self.formatar_imagem.aberto {
            linha = linha.push(self.view_formatar_imagem());
        }
        linha.into()
    }
}

/// Alça de redimensionar: barra grossa colada na borda (no meio dos lados)
/// ou "L" grosso no canto, dentro de uma área de captura maior que o traço.
fn alca_grafico<'a>(id: &str, direcao: Alca) -> Element<'a, Msg> {
    use crate::compat::alignment::Horizontal; use crate::compat::alignment::Vertical;
    const ESPESSURA: f32 = 5.0;
    const COMPRIMENTO: f32 = 28.0;
    const CANTO: f32 = 16.0;
    const CAPTURA: f32 = 12.0;
    let barra = |w: f32, h: f32| -> Element<'a, Msg> {
        container(Space::new(Length::Fixed(w), Length::Fixed(h)))
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(feather_ui::tema::COR_AZUL_ACENTO())),
                border: pelicano::Border { radius: [ESPESSURA / 2.0; 4], ..Default::default() },
                ..Default::default()
            })
            .into()
    };
    let colada = |filho: Element<'a, Msg>, w: f32, h: f32, x: Horizontal, y: Vertical| -> Element<'a, Msg> {
        container(filho).width(Length::Fixed(w)).height(Length::Fixed(h))
            .align_x(x).align_y(y).into()
    };
    let (x, y) = match direcao {
        Alca::Noroeste => (pelicano::Align::Start, pelicano::Align::Start),
        Alca::Norte => (pelicano::Align::Center, pelicano::Align::Start),
        Alca::Nordeste => (pelicano::Align::End, pelicano::Align::Start),
        Alca::Oeste => (pelicano::Align::Start, pelicano::Align::Center),
        Alca::Leste => (pelicano::Align::End, pelicano::Align::Center),
        Alca::Sudoeste => (pelicano::Align::Start, pelicano::Align::End),
        Alca::Sul => (pelicano::Align::Center, pelicano::Align::End),
        Alca::Sudeste => (pelicano::Align::End, pelicano::Align::End),
    };
    let (desenho, cursor): (Element<'a, Msg>, _) = match direcao {
        Alca::Norte | Alca::Sul => (
            colada(barra(COMPRIMENTO, ESPESSURA), COMPRIMENTO + 8.0, CAPTURA, x, y),
            pelicano::CursorIcon::NsResize,
        ),
        Alca::Oeste | Alca::Leste => (
            colada(barra(ESPESSURA, COMPRIMENTO), CAPTURA, COMPRIMENTO + 8.0, x, y),
            pelicano::CursorIcon::EwResize,
        ),
        _ => {
            let l = pelicano::stack![
                colada(barra(CANTO, ESPESSURA), CANTO, CANTO, x, y),
                colada(barra(ESPESSURA, CANTO), CANTO, CANTO, x, y),
            ];
            let cursor = if matches!(direcao, Alca::Noroeste | Alca::Sudeste) {
                pelicano::CursorIcon::NwseResize
            } else {
                pelicano::CursorIcon::NeswResize
            };
            (colada(l.into(), CANTO + 4.0, CANTO + 4.0, x, y), cursor)
        }
    };
    pelicano::mouse_area(desenho)
        .on_press(Msg::Grafico(Evento::Redimensionar(id.to_owned(), direcao)))
        .cursor_icon(cursor)
        .into()
}

#[cfg(test)]
mod testes_cruzar {
    use super::cruzar;

    #[test]
    fn troca_da_legenda_nunca_sobrepoe_os_dois_desenhos() {
        let svg = |barra: &str| format!("<svg width=\"200\" height=\"100\"><rect class=\"area-grafico\" x=\"0\" y=\"0\" width=\"200\" height=\"100\" rx=\"0\" fill=\"#ffffff\" filter=\"url(#sombra-grafico)\"/>{barra}</svg>");
        let (antes, depois) = (svg("<rect class=\"a\"/>"), svg("<rect class=\"b\"/>"));
        let opacidades = |t: f32| -> Vec<f32> {
            let s = cruzar(&antes, &depois, t);
            s.match_indices("<g opacity=\"").map(|(i, _)| s[i + 12..].split('"').next().unwrap().parse().unwrap()).collect()
        };
        for t in [0.0, 0.2, 0.49, 0.5, 0.8, 1.0] {
            let o = opacidades(t);
            assert!(o[0] == 0.0 || o[1] == 0.0, "t={t}: {o:?}");
        }
        assert_eq!(opacidades(0.0), [1.0, 0.0]);
        assert_eq!(opacidades(1.0), [0.0, 1.0]);
        // O fundo fixo vem sem a sombra (as defs ficam dentro dos grupos).
        let s = cruzar(&antes, &depois, 0.5);
        let fundo = &s[..s.find("<g ").unwrap()];
        assert!(fundo.contains("area-grafico") && !fundo.contains("url("), "{fundo}");
    }
}
