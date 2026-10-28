use super::*;

/// Uma planilha de mentira: `(linha, coluna) -> valor`.
struct Plan(HashMap<(u32, u32), Value>);

impl Plan {
    fn nova() -> Self {
        Plan(HashMap::new())
    }
    fn txt(&mut self, r: u32, c: u32, s: &str) {
        self.0.insert((r, c), Value::Text(s.to_string()));
    }
    fn num(&mut self, r: u32, c: u32, n: f64) {
        self.0.insert((r, c), Value::Number(n));
    }
    fn dados(&self, config: &ChartConfig) -> ChartData {
        extract_chart_data_with(&|r, c| self.0.get(&(r, c)).cloned().unwrap_or(Value::Empty), config)
    }
}

fn config(range: (u32, u32, u32, u32)) -> ChartConfig {
    ChartConfig {
        range: ChartRange { r0: range.0, c0: range.1, r1: range.2, c1: range.3 },
        series_in_columns: true,
        has_headers: true,
        has_category_labels: true,
        ..Default::default()
    }
}

/// A planilha de `chart-agregacao.test.ts`: ALTO tem 2,4,6,8; NORMAL 10,30.
fn planilha_agregacao() -> Plan {
    let mut p = Plan::nova();
    p.txt(0, 0, "Status");
    p.txt(0, 1, "ValorA");
    p.txt(0, 2, "ValorB");
    for (i, (cat, a)) in [("ALTO", 2.0), ("ALTO", 4.0), ("NORMAL", 10.0), ("ALTO", 6.0), ("NORMAL", 30.0), ("ALTO", 8.0)]
        .into_iter()
        .enumerate()
    {
        let r = i as u32 + 1;
        p.txt(r, 0, cat);
        p.num(r, 1, a);
        p.num(r, 2, 1.0);
    }
    p
}

fn agregado(conta: Option<AggregateFn>) -> ChartConfig {
    ChartConfig { aggregate: true, aggregate_fn: conta, ..config((0, 0, 6, 2)) }
}

// --- chart.test.ts

#[test]
fn series_em_colunas() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Mês");
    p.txt(0, 1, "Vendas");
    p.txt(0, 2, "Lucro");
    p.txt(1, 0, "Jan");
    p.num(1, 1, 100.0);
    p.num(1, 2, 20.0);
    p.txt(2, 0, "Fev");
    p.num(2, 1, 150.0);
    p.num(2, 2, 35.0);
    let d = p.dados(&ChartConfig { title: "Desempenho".into(), ..config((0, 0, 2, 2)) });
    assert_eq!(d.title, "Desempenho");
    assert_eq!(d.categories, ["Jan", "Fev"]);
    assert_eq!(d.series[0], ChartSeries { name: "Vendas".into(), values: vec![100.0, 150.0] });
    assert_eq!(d.series[1], ChartSeries { name: "Lucro".into(), values: vec![20.0, 35.0] });
}

#[test]
fn series_em_linhas() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Produto");
    p.txt(0, 1, "Q1");
    p.txt(0, 2, "Q2");
    p.txt(1, 0, "Alfa");
    p.num(1, 1, 50.0);
    p.num(1, 2, 80.0);
    let d = p.dados(&ChartConfig { series_in_columns: false, ..config((0, 0, 1, 2)) });
    assert_eq!(d.categories, ["Q1", "Q2"]);
    assert_eq!(d.series, [ChartSeries { name: "Alfa".into(), values: vec![50.0, 80.0] }]);
}

#[test]
fn linha_3d_sem_escolha_pula_a_descricao_e_agrupa_por_ela() {
    // O caso do usuário (24/09/2026): Descr, A, B, C com a/b alternados.
    let mut p = Plan::nova();
    for (c, h) in ["Descr", "A", "B", "C"].iter().enumerate() {
        p.txt(0, c as u32, h);
    }
    for r in 1..=4u32 {
        p.txt(r, 0, if r % 2 == 1 { "a" } else { "b" });
        p.num(r, 1, f64::from(r));
        p.num(r, 2, f64::from(r) + 1.0);
        p.num(r, 3, f64::from(r) + 2.0);
    }
    let d = p.dados(&ChartConfig { tipo: ChartType::Line3d, ..config((0, 0, 4, 3)) });
    assert_eq!(d.x_values, Some(vec![1.0, 2.0, 3.0, 4.0]), "X é a coluna A, não a descrição");
    assert_eq!(d.z_values, Some(vec![3.0, 4.0, 5.0, 6.0]));
    let nomes: Vec<&str> = d.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(nomes, ["a", "b"], "uma linha para cada descrição");
    assert_eq!(d.series[0].values[0], 2.0);
    assert!(d.series[0].values[1].is_nan(), "a linha 'a' não tem ponto nas linhas 'b'");
    assert_eq!(d.series[1].values[1], 3.0);
    assert_eq!(d.categories, ["a", "b", "a", "b"]);
}

#[test]
fn linha_3d_usa_as_colunas_escolhidas() {
    // Colunas: Nome, Z, X, Y — fora de ordem, com texto na primeira.
    let mut p = Plan::nova();
    for (c, h) in ["Nome", "Z", "X", "Y"].iter().enumerate() {
        p.txt(0, c as u32, h);
    }
    for (r, (z, x, y)) in [(7.0, 1.0, 10.0), (8.0, 2.0, 20.0)].iter().enumerate() {
        let r = r as u32 + 1;
        p.txt(r, 0, "a");
        p.num(r, 1, *z);
        p.num(r, 2, *x);
        p.num(r, 3, *y);
    }
    let d = p.dados(&ChartConfig {
        tipo: ChartType::Line3d,
        eixos_3d: Some(Eixos3d { x: 2, y: 3, z: Some(1), grupo: None }),
        ..config((0, 0, 2, 3))
    });
    assert_eq!(d.x_values, Some(vec![1.0, 2.0]));
    assert_eq!(d.series[0].values, [10.0, 20.0]);
    assert_eq!(d.series[0].name, "Y");
    assert_eq!(d.z_values, Some(vec![7.0, 8.0]));
    // Sem Z: profundidade pela ordem (o desenho decide).
    let d = p.dados(&ChartConfig {
        tipo: ChartType::Line3d,
        eixos_3d: Some(Eixos3d { x: 2, y: 3, z: None, grupo: None }),
        ..config((0, 0, 2, 3))
    });
    assert_eq!(d.z_values, None);
    let lido: ChartConfig = serde_json::from_str(r#"{"type":"line-3d","eixos3d":{"x":2,"y":3}}"#).unwrap();
    assert_eq!(lido.eixos_3d, Some(Eixos3d { x: 2, y: 3, z: None, grupo: None }));
}

#[test]
fn linha_3d_separa_x_e_z() {
    let mut p = Plan::nova();
    for (c, t) in ["X", "Y", "Z"].iter().enumerate() {
        p.txt(0, c as u32, t);
    }
    for (r, linha) in [[10.0, 20.0, 30.0], [15.0, 25.0, 35.0]].iter().enumerate() {
        for (c, &v) in linha.iter().enumerate() {
            p.num(r as u32 + 1, c as u32, v);
        }
    }
    let d = p.dados(&ChartConfig {
        tipo: ChartType::Line3d,
        has_category_labels: false,
        ..config((0, 0, 2, 2))
    });
    assert_eq!(d.x_values, Some(vec![10.0, 15.0]));
    assert_eq!(d.z_values, Some(vec![30.0, 35.0]));
    assert_eq!(d.series.len(), 1);
    assert_eq!(d.series[0].values, [20.0, 25.0]);
    // O tipo sobrevive à ida e volta do arquivo (não vira `Outro`).
    assert_eq!(serde_json::to_string(&ChartType::Line3d).unwrap(), "\"line-3d\"");
}

#[test]
fn dispersao_separa_o_eixo_x() {
    let mut p = Plan::nova();
    p.txt(0, 0, "X");
    p.txt(0, 1, "Y");
    p.num(1, 0, 1.0);
    p.num(1, 1, 10.0);
    p.num(2, 0, 2.0);
    p.num(2, 1, 25.0);
    let d = p.dados(&ChartConfig {
        tipo: ChartType::Scatter,
        has_category_labels: false,
        ..config((0, 0, 2, 1))
    });
    assert_eq!(d.x_values, Some(vec![1.0, 2.0]));
    assert_eq!(d.series.len(), 1);
    assert_eq!(d.series[0].values, [10.0, 25.0]);
}

#[test]
fn coluna_do_eixo_x_escolhida() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Vendedor");
    p.txt(0, 1, "Mês");
    p.txt(0, 2, "Vendas");
    p.txt(1, 0, "Ana");
    p.txt(1, 1, "Jan");
    p.num(1, 2, 100.0);
    p.txt(2, 0, "Bruno");
    p.txt(2, 1, "Fev");
    p.num(2, 2, 150.0);
    let d = p.dados(&ChartConfig { category_column_index: Some(1), ..config((0, 0, 2, 2)) });
    assert_eq!(d.categories, ["Jan", "Fev"]);
    assert_eq!(d.series.len(), 1, "a coluna de texto (Vendedor) não vira série");
    assert_eq!(d.series[0].name, "Vendas");
}

#[test]
fn agrupa_e_soma_por_categoria() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Tipo");
    p.txt(0, 1, "Valor");
    p.txt(1, 0, "Alimentação");
    p.num(1, 1, 100.0);
    p.txt(2, 0, "Alimentação");
    p.num(2, 1, 200.0);
    p.txt(3, 0, "Transporte");
    p.num(3, 1, 150.0);
    let d = p.dados(&ChartConfig { aggregate: true, ..config((0, 0, 3, 1)) });
    assert_eq!(d.categories, ["Alimentação", "Transporte"]);
    assert_eq!(d.series, [ChartSeries { name: "Valor".into(), values: vec![300.0, 150.0] }]);
}

#[test]
fn hierarquia_de_datas() {
    // No TypeScript `setCell('15/01/2024')` já guarda o serial da data.
    let mut p = Plan::nova();
    p.txt(0, 0, "Data");
    p.txt(0, 1, "Vendas");
    for (r, serial, v) in [(1, 45306.0, 100.0), (2, 45311.0, 150.0), (3, 45327.0, 200.0)] {
        p.num(r, 0, serial);
        p.num(r, 1, v);
    }
    let com = |h| ChartConfig { style: ChartStyle { date_hierarchy: Some(h), ..Default::default() }, ..config((0, 0, 3, 1)) };

    let mes = p.dados(&com(DateHierarchy::Month));
    assert_eq!(mes.categories, ["01/2024", "02/2024"]);
    assert_eq!(mes.series[0].values, [250.0, 200.0]);

    let ano = p.dados(&com(DateHierarchy::Year));
    assert_eq!(ano.categories, ["2024"]);
    assert_eq!(ano.series[0].values, [450.0]);

    let dia = p.dados(&com(DateHierarchy::Day));
    assert_eq!(dia.categories, ["15/01/2024", "20/01/2024", "05/02/2024"]);
    assert_eq!(dia.series[0].values, [100.0, 150.0, 200.0]);
}

#[test]
fn data_em_texto() {
    assert_eq!(parse_category_date(&Value::Text("15/01/2024".into())), Some((2024, 1, 15)));
    assert_eq!(parse_category_date(&Value::Text("5.2.24".into())), Some((2024, 2, 5)));
    assert_eq!(parse_category_date(&Value::Text("2024-02-05".into())), Some((2024, 2, 5)));
    // O Date.UTC do JavaScript rola o dia que passa do mês: 31/02 é 02/03
    // em 2024 (bissexto). Traduzido, não consertado.
    assert_eq!(parse_category_date(&Value::Text("31/02/2024".into())), Some((2024, 3, 2)));
    assert_eq!(parse_category_date(&Value::Text("32/01/2024".into())), None);
    assert_eq!(parse_category_date(&Value::Text("15/13/2024".into())), None);
    assert_eq!(parse_category_date(&Value::Number(999.0)), None);
}

#[test]
fn data_em_texto_livre_nao_vira_data() {
    // Diferença CONSCIENTE com o TypeScript, que caía no `Date.parse` do
    // navegador: o que ele aceita muda de motor para motor.
    assert_eq!(parse_category_date(&Value::Text("Jan 15 2024".into())), None);
}

#[test]
fn tendencia_linear() {
    let t = compute_linear_trendline(&[1.0, 2.0, 3.0, 4.0, 5.0], &[2.0, 4.0, 6.0, 8.0, 10.0]).unwrap();
    assert_eq!(t.slope, 2.0);
    assert_eq!(t.intercept, 0.0);
    assert_eq!(t.r_squared, 1.0);
    assert_eq!(compute_linear_trendline(&[1.0], &[1.0]), None);
}

#[test]
fn cor_da_serie() {
    let oceano = ChartStyle { palette: Some(PaletteName::Ocean), ..Default::default() };
    assert_eq!(get_series_color(&oceano, 0), "#0284c7");
    let mut propria = ChartStyle::default();
    propria.series_colors.insert(0, "#ff0000".into());
    assert_eq!(get_series_color(&propria, 0), "#ff0000");
    assert_eq!(get_series_color(&ChartStyle::default(), 9), "#10b981", "a paleta volta ao começo");
}

#[test]
fn empilhamento_pelo_tipo() {
    assert_eq!(get_stack_mode(&ChartConfig { tipo: ChartType::ColumnStacked100, ..Default::default() }), StackMode::Percent);
    assert_eq!(get_stack_mode(&ChartConfig { tipo: ChartType::BarStacked, ..Default::default() }), StackMode::Stacked);
    assert_eq!(get_stack_mode(&ChartConfig::default()), StackMode::None);
}

// --- chart-agregacao.test.ts

#[test]
fn series_escondidas_nao_aparecem_no_grafico_agrupado() {
    let p = planilha_agregacao();
    let nomes = |esc: Vec<u32>| {
        let mut c = agregado(None);
        c.style.hidden_series = esc;
        p.dados(&c).series.into_iter().map(|s| s.name).collect::<Vec<_>>()
    };
    assert_eq!(nomes(vec![]), ["ValorA", "ValorB"]);
    assert_eq!(nomes(vec![0]), ["ValorB"]);
    assert_eq!(nomes(vec![1]), ["ValorA"]);
}

#[test]
fn cada_conta_da_o_seu_numero() {
    let p = planilha_agregacao();
    let valor = |conta: Option<AggregateFn>, cat: &str| {
        let d = p.dados(&agregado(conta));
        let i = d.categories.iter().position(|c| c == cat).unwrap();
        d.series[0].values[i]
    };
    assert_eq!(valor(None, "ALTO"), 20.0, "sem dizer nada, soma");
    assert_eq!(valor(Some(AggregateFn::Avg), "ALTO"), 5.0);
    assert_eq!(valor(Some(AggregateFn::Count), "ALTO"), 4.0);
    assert_eq!(valor(Some(AggregateFn::Min), "ALTO"), 2.0);
    assert_eq!(valor(Some(AggregateFn::Max), "ALTO"), 8.0);
    assert!((valor(Some(AggregateFn::Stddev), "ALTO") - (20.0f64 / 3.0).sqrt()).abs() < 1e-9);
    assert_eq!(valor(Some(AggregateFn::Sum), "NORMAL"), 40.0);
    assert_eq!(valor(Some(AggregateFn::Avg), "NORMAL"), 20.0);
    assert_eq!(valor(Some(AggregateFn::Stddev), "NORMAL"), 200.0f64.sqrt());
}

#[test]
fn desvio_de_uma_amostra_so_e_zero() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Cat");
    p.txt(0, 1, "Valor");
    p.txt(1, 0, "X");
    p.num(1, 1, 42.0);
    let d = p.dados(&ChartConfig { aggregate: true, aggregate_fn: Some(AggregateFn::Stddev), ..config((0, 0, 1, 1)) });
    assert_eq!(d.series[0].values, [0.0]);
}

#[test]
fn cada_serie_tem_a_sua_conta_e_o_nome_diz_qual() {
    let p = planilha_agregacao();
    let mut c = agregado(None);
    c.aggregate_by_series = BTreeMap::from([(0, AggregateFn::Max), (1, AggregateFn::Count)]);
    let d = p.dados(&c);
    assert_eq!(d.categories, ["ALTO", "NORMAL"]);
    assert_eq!(d.series[0], ChartSeries { name: "Máximo de ValorA".into(), values: vec![8.0, 30.0] });
    assert_eq!(d.series[1], ChartSeries { name: "Contagem de ValorB".into(), values: vec![4.0, 2.0] });
}

#[test]
fn serie_sem_conta_propria_segue_a_de_todas() {
    let p = planilha_agregacao();
    let mut c = agregado(Some(AggregateFn::Avg));
    c.aggregate_by_series = BTreeMap::from([(1, AggregateFn::Sum)]);
    let d = p.dados(&c);
    assert_eq!(d.series[0], ChartSeries { name: "ValorA".into(), values: vec![5.0, 20.0] });
    assert_eq!(d.series[1], ChartSeries { name: "Soma de ValorB".into(), values: vec![4.0, 2.0] });
}

#[test]
fn moda_mais_repetido_e_no_empate_o_primeiro() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Status");
    p.txt(0, 1, "Nota");
    let linhas = [
        ("ALTO", 7.0), ("NORMAL", 3.0), ("ALTO", 5.0), ("NORMAL", 9.0),
        ("ALTO", 5.0), ("NORMAL", 9.0), ("NORMAL", 3.0), ("BAIXO", 1.0), ("BAIXO", 2.0),
    ];
    for (i, (cat, v)) in linhas.iter().enumerate() {
        p.txt(i as u32 + 1, 0, cat);
        p.num(i as u32 + 1, 1, *v);
    }
    let d = p.dados(&ChartConfig {
        aggregate: true,
        aggregate_fn: Some(AggregateFn::Mode),
        ..config((0, 0, linhas.len() as u32, 1))
    });
    assert_eq!(d.categories, ["ALTO", "NORMAL", "BAIXO"]);
    assert_eq!(d.series[0].values, [5.0, 3.0, 0.0]);
    assert_eq!(d.aviso, None);
}

#[test]
fn chip_mostra_o_nome_sem_a_conta() {
    let p = planilha_agregacao();
    let mut c = agregado(None);
    c.aggregate_by_series = BTreeMap::from([(0, AggregateFn::Avg)]);
    c.style.hidden_series = vec![1];
    let chips = extract_raw_chart_series_with(&|r, col| p.0.get(&(r, col)).cloned().unwrap_or(Value::Empty), &c);
    assert_eq!(chips, [(0, "ValorA".to_string()), (1, "ValorB".to_string())]);
}

// --- chart-amostragem.test.ts

const LINHAS: u32 = 50_000;

fn sem_rotulos(range: (u32, u32, u32, u32)) -> ChartConfig {
    ChartConfig { tipo: ChartType::Line, has_headers: false, has_category_labels: false, ..config(range) }
}

#[test]
fn intervalo_enorme_e_amostrado_e_anunciado_com_o_ultimo_ponto() {
    let mut p = Plan::nova();
    p.num(0, 0, 1.0);
    p.num(LINHAS - 1, 0, 999.0);
    let d = p.dados(&sem_rotulos((0, 0, LINHAS - 1, 0)));
    let n = d.series[0].values.len();
    assert!(n <= MAX_PONTOS_POR_SERIE + 1 && n > 100, "vieram {n} pontos");
    assert!(d.categories.len() <= MAX_PONTOS_POR_SERIE + 1);
    assert_eq!(*d.series[0].values.last().unwrap(), 999.0, "o último valor real aparece");
    let a = d.amostrado.expect("a amostragem precisa ser anunciada");
    assert_eq!(a.disponiveis, LINHAS as usize);
    assert!(a.usados < LINHAS as usize && a.usados > 100);
}

#[test]
fn intervalo_pequeno_nao_e_amostrado() {
    let mut p = Plan::nova();
    for r in 0..10 {
        p.num(r, 0, (r + 1) as f64);
    }
    let d = p.dados(&sem_rotulos((0, 0, 9, 0)));
    assert_eq!(d.series[0].values, (1..=10).map(f64::from).collect::<Vec<_>>());
    assert_eq!(d.amostrado, None);
}

#[test]
fn series_em_linhas_nao_vira_um_milhao_de_series() {
    let mut p = Plan::nova();
    for r in 0..LINHAS {
        p.num(r, 1, (r + 1) as f64);
    }
    let d = p.dados(&ChartConfig { series_in_columns: false, ..sem_rotulos((0, 0, LINHAS - 1, 3)) });
    assert!(d.series.len() <= MAX_PONTOS_POR_SERIE + 1, "vieram {} séries", d.series.len());
}

#[test]
fn agrupar_soma_todas_as_linhas_nao_a_amostra() {
    // O bug de 22/09/2026: a barra saía 489 vezes menor que o rodapé.
    let mut p = Plan::nova();
    p.txt(0, 0, "Status");
    p.txt(0, 1, "Valor");
    let n = 20_000;
    for r in 1..=n {
        p.txt(r, 0, if r % 2 == 0 { "ALTO" } else { "NORMAL" });
        p.num(r, 1, 1.0);
    }
    let d = p.dados(&ChartConfig { aggregate: true, ..config((0, 0, n, 1)) });
    assert_eq!(d.series[0].values, [10_000.0, 10_000.0]);
    assert_eq!(d.amostrado, None);
}

// --- a leitura da célula, que o TypeScript fazia com parseFloat e String()

#[test]
fn numero_de_texto_segue_o_parse_float() {
    assert_eq!(parse_float_js("12abc"), 12.0);
    assert_eq!(parse_float_js("  -3.5e2x"), -350.0);
    assert_eq!(parse_float_js("1e"), 1.0);
    assert_eq!(parse_float_js(".5"), 0.5);
    assert!(parse_float_js("R$ 5").is_nan());
    assert!(parse_float_js(".").is_nan());
    assert_eq!(numero_da_celula(&Value::Text("1,5".into())), 1.5);
    assert_eq!(numero_da_celula(&Value::Text("abc".into())), 0.0);
    assert_eq!(numero_da_celula(&Value::Bool(true)), 1.0);
    assert_eq!(texto_da_celula(&Value::Number(1e21)), "1e+21");
    assert_eq!(texto_da_celula(&Value::Bool(false)), "false");
}

#[test]
fn forma_e_tamanho_do_marcador_preservam_prioridade_do_typescript() {
    let estilo: ChartStyle = serde_json::from_str(r#"{
        "pointShape":"triangle", "pointSize":8,
        "seriesPointShapes":{"0":"none","2":"diamond"},
        "seriesPointSizes":{"0":0,"2":12.5}
    }"#).unwrap();
    assert_eq!(get_series_point_shape(&estilo, 0), "none");
    assert_eq!(get_series_point_size(&estilo, 0), 0.0);
    assert_eq!(get_series_point_shape(&estilo, 1), "triangle");
    assert_eq!(get_series_point_size(&estilo, 1), 8.0);
    assert_eq!(get_series_point_shape(&estilo, 2), "diamond");
    assert_eq!(get_series_point_size(&estilo, 2), 12.5);
    let padrao = ChartStyle::default();
    assert_eq!(get_series_point_shape(&padrao, 33), "circle");
    assert_eq!(get_series_point_size(&padrao, 33), 5.0);
}

#[test]
fn marcador_especifico_vazio_recua_para_forma_geral() {
    let mut estilo = ChartStyle {
        point_shape: Some("square".into()), ..Default::default()
    };
    estilo.series_point_shapes.insert(2, "".into());
    assert_eq!(get_series_point_shape(&estilo, 2), "square");
    estilo.point_shape = None;
    assert_eq!(get_series_point_shape(&estilo, 2), "circle");
}

#[test]
fn media_movel_como_o_typescript() {
    assert_eq!(compute_moving_average(&[1.0, 2.0, 3.0, 4.0], 2), vec![None, Some(1.5), Some(2.5), Some(3.5)]);
    assert_eq!(compute_moving_average(&[1.0, 2.0], 3), vec![None, None]);
}

/// Pedido de 16/10: série de texto só conta. "Contar" conta as linhas da
/// categoria; "Contar distintos" conta os valores diferentes (vazio fora).
#[test]
fn serie_de_texto_conta_e_conta_distintos() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Região");
    p.txt(0, 1, "Cliente");
    for (i, (regiao, cliente)) in [("Sul", "Ana"), ("Sul", "Bia"), ("Sul", "Ana"), ("Norte", "Caio"), ("Sul", "")]
        .into_iter()
        .enumerate()
    {
        p.txt(i as u32 + 1, 0, regiao);
        if !cliente.is_empty() {
            p.txt(i as u32 + 1, 1, cliente);
        }
    }
    let mut c = ChartConfig { aggregate: true, ..config((0, 0, 5, 1)) };
    c.aggregate_by_series = BTreeMap::from([(0, AggregateFn::CountDistinct)]);
    let d = p.dados(&c);
    assert_eq!(d.categories, ["Sul", "Norte"]);
    assert_eq!(d.series[0].values, [2.0, 1.0]);
    assert_eq!(d.series[0].name, "Contagem distinta de Cliente");
    c.aggregate_by_series = BTreeMap::from([(0, AggregateFn::Count)]);
    assert_eq!(p.dados(&c).series[0].values, [4.0, 1.0]);
    // O nome gravado no arquivo.
    assert_eq!(serde_json::to_value(AggregateFn::CountDistinct).unwrap(), "countDistinct");
    assert!(AggregateFn::CountDistinct.serve_para_texto() && !AggregateFn::Sum.serve_para_texto());
}

#[test]
fn plot_vis_only_pula_linhas_ocultas() {
    let mut p = Plan::nova();
    p.txt(0, 0, "Item");
    p.txt(0, 1, "Valor");
    for (r, (cat, n)) in [("a", 1.0), ("b", 2.0), ("c", 3.0)].into_iter().enumerate() {
        p.txt(r as u32 + 1, 0, cat);
        p.num(r as u32 + 1, 1, n);
    }
    let ler = |r: u32, c: u32| p.0.get(&(r, c)).cloned().unwrap_or(Value::Empty);
    let oculta = |r: u32| r == 2;
    let mut c = config((0, 0, 3, 1));
    // Sem pedir, a linha oculta continua (os gráficos feitos aqui não mudam).
    assert_eq!(extract_chart_data_visiveis(&ler, &oculta, &c).categories, ["a", "b", "c"]);
    c.plot_vis_only = true;
    let d = extract_chart_data_visiveis(&ler, &oculta, &c);
    assert_eq!((d.categories, d.series[0].values.clone()), (vec!["a".to_string(), "c".into()], vec![1.0, 3.0]));
    c.aggregate = true;
    assert_eq!(extract_chart_data_visiveis(&ler, &oculta, &c).categories, ["a", "c"]);
}
