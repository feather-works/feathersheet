//! Fronteira FeatherSheet -> FeatherWorks Charts (CHT-005).
//! A extração, filtros, datas e agregações continuam em feathersheet-core.
//! Nunca reconstrói totais a partir de amostras, nem altera o formato .fshz.

use std::collections::HashSet;

use feather_charts::{
    AmostraDados, ContratoGrafico, ErroGrafico, Grafico, MetadadosSerie, Serie, Tipo,
};
use feather_draw::Cor;
use feathersheet_core::grafico::chart::{
    conta_da_serie, extract_chart_data, extract_chart_data_with, get_series_color, ChartConfig,
    ChartData, ChartType, DateHierarchy,
};
use feathersheet_core::{Sheet, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroAdaptacaoGrafico {
    /// A fonte conhece mais tipos do que o backend de desenho comum atual.
    TipoNaoSuportado(ChartType),
    EixoXIndisponivel,
    CorNaoSuportada { indice_origem: u32, valor: String },
    DadosInvalidos(ErroGrafico),
}

fn tipo_comum(tipo: ChartType) -> Result<Tipo, ErroAdaptacaoGrafico> {
    Ok(match tipo {
        ChartType::Column => Tipo::Colunas,
        ChartType::Bar => Tipo::Barras,
        ChartType::Line => Tipo::Linhas,
        ChartType::Area => Tipo::Area,
        ChartType::StepLine => Tipo::Degraus,
        ChartType::Pie | ChartType::Doughnut => Tipo::Pizza,
        ChartType::Scatter => Tipo::Dispersao,
        outro => return Err(ErroAdaptacaoGrafico::TipoNaoSuportado(outro)),
    })
}

/// Cores RGB/RGBA hexadecimais; não aplica fallback silencioso a estilos
/// CSS que o motor vetorial ainda não representa.
pub(crate) fn cor_css(valor: &str) -> Option<Cor> {
    let hex = valor.strip_prefix('#')?;
    // Não dividir UTF-8 por offsets de byte sem validar a entrada.
    if !hex.is_ascii() {
        return None;
    }
    let byte = |s: &str| u8::from_str_radix(s, 16).ok();
    match hex.len() {
        3 => {
            let r = byte(&hex[0..1])?;
            let g = byte(&hex[1..2])?;
            let b = byte(&hex[2..3])?;
            Some(Cor::rgb(r * 17, g * 17, b * 17))
        }
        4 => {
            let r = byte(&hex[0..1])?;
            let g = byte(&hex[1..2])?;
            let b = byte(&hex[2..3])?;
            let a = byte(&hex[3..4])?;
            Some(Cor::rgba(r * 17, g * 17, b * 17, a * 17))
        }
        6 => Some(Cor::rgb(byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?)),
        8 => Some(Cor::rgba(
            byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?, byte(&hex[6..8])?,
        )),
        _ => None,
    }
}

/// Adapta os tipos efetivamente desenhados por Charts (incluindo Área e Degraus).
/// Qualquer dado ausente/não finito é recusado explicitamente; o chamador
/// deve manter o renderizador legado para estes casos.
pub fn preparar_grafico(
    config: &ChartConfig,
    dados: ChartData,
) -> Result<ContratoGrafico, ErroAdaptacaoGrafico> {
    let tipo = tipo_comum(config.tipo)?;
    let categorias = if tipo == Tipo::Dispersao {
        let xs = dados.x_values.as_ref().ok_or(ErroAdaptacaoGrafico::EixoXIndisponivel)?;
        if xs.len() != dados.categories.len() || xs.iter().any(|x| !x.is_finite()) {
            return Err(ErroAdaptacaoGrafico::EixoXIndisponivel);
        }
        xs.iter().map(f64::to_string).collect()
    } else {
        dados.categories
    };
    let ocultas: HashSet<u32> = config.style.hidden_series.iter().copied().collect();
    let mut proximo = 0u32;
    let mut series = Vec::with_capacity(dados.series.len());
    let mut metadados_series = Vec::with_capacity(dados.series.len());
    let agregou = config.aggregate
        || config.style.date_hierarchy.unwrap_or_default() != DateHierarchy::None;
    for serie in dados.series {
        while ocultas.contains(&proximo) {
            proximo = proximo.checked_add(1)
                .ok_or(ErroAdaptacaoGrafico::DadosInvalidos(ErroGrafico::ValorInvalido))?;
        }
        let indice_origem = proximo;
        proximo = proximo.checked_add(1)
            .ok_or(ErroAdaptacaoGrafico::DadosInvalidos(ErroGrafico::ValorInvalido))?;
        let css = get_series_color(&config.style, indice_origem);
        let cor = cor_css(&css).ok_or(ErroAdaptacaoGrafico::CorNaoSuportada {
            indice_origem, valor: css,
        })?;
        let agregacao = agregou.then(|| conta_da_serie(config, indice_origem).rotulo().to_string());
        metadados_series.push(MetadadosSerie {
            indice_origem, nome: serie.name.clone(), cor, agregacao,
        });
        series.push(Serie { nome: serie.name, valores: serie.values, cor });
    }
    let grafico = Grafico {
        titulo: dados.title,
        tipo,
        categorias,
        series,
    };
    let contrato = ContratoGrafico {
        grafico, metadados_series,
        amostra: dados.amostrado.map(|a| AmostraDados {
            disponiveis: a.disponiveis, usados: a.usados,
        }),
        avisos: dados.aviso.into_iter().collect(),
    };
    contrato.validar().map_err(ErroAdaptacaoGrafico::DadosInvalidos)?;
    Ok(contrato)
}

/// Fluxo real de uma planilha para a cena comum: o ChartData (inclusive
/// aggregações exatas) continua vindo do próprio motor FeatherSheet.
pub fn extrair_grafico(
    sheet: &Sheet,
    config: &ChartConfig,
) -> Result<ContratoGrafico, ErroAdaptacaoGrafico> {
    preparar_grafico(config, extract_chart_data(sheet, config))
}

/// Versão com callback usada por consumidores com armazenamento colunar.
pub fn extrair_grafico_com(
    celula: &dyn Fn(u32, u32) -> Value,
    config: &ChartConfig,
) -> Result<ContratoGrafico, ErroAdaptacaoGrafico> {
    preparar_grafico(config, extract_chart_data_with(celula, config))
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::grafico::chart::{
        AggregateFn, ChartRange, ChartStyle, DateHierarchy, PaletteName,
    };
    use std::collections::{BTreeMap, HashMap};

    fn valores() -> HashMap<(u32, u32), Value> {
        let mut m = HashMap::new();
        for (c, nome) in ["Grupo", "Vendas", "Meta"].into_iter().enumerate() {
            m.insert((0, c as u32), Value::Text(nome.into()));
        }
        for (r, (grupo, vendas, meta)) in [
            ("A", 10., 1.), ("B", 20., 2.), ("A", 30., 3.),
        ].into_iter().enumerate() {
            let r = r as u32 + 1;
            m.insert((r, 0), Value::Text(grupo.into()));
            m.insert((r, 1), Value::Number(vendas));
            m.insert((r, 2), Value::Number(meta));
        }
        m
    }

    fn configuracao() -> ChartConfig {
        ChartConfig {
            tipo: ChartType::Column, title: "Receita".into(),
            range: ChartRange { r0: 0, c0: 0, r1: 3, c1: 2 },
            series_in_columns: true, has_headers: true,
            has_category_labels: true, aggregate: true,
            ..Default::default()
        }
    }

    fn extrair(m: &HashMap<(u32, u32), Value>, c: &ChartConfig) -> ContratoGrafico {
        extrair_grafico_com(
            &|r, col| m.get(&(r, col)).cloned().unwrap_or(Value::Empty), c,
        ).unwrap()
    }

    #[test]
    fn agregacao_exata_por_serie_e_renderizacao_independente() {
        let m = valores();
        let mut config = configuracao();
        config.aggregate_by_series.insert(1, AggregateFn::Avg);
        let contrato = extrair(&m, &config);
        assert_eq!(contrato.grafico.categorias, ["A", "B"]);
        assert_eq!(contrato.grafico.series[0].valores, [40., 20.]);
        assert_eq!(contrato.grafico.series[1].valores, [2., 2.]);
        assert_eq!(contrato.metadados_series[1].agregacao.as_deref(), Some("Média"));
        assert_eq!(contrato.amostra, None);
        assert!(contrato.cena(640., 360.).is_ok());
        assert_eq!(contrato.grafico.series[0].valores, [40., 20.]);
    }

    #[test]
    fn ocultacao_preserva_indice_original_cor_e_ordem() {
        let m = valores();
        let mut config = configuracao();
        config.style = ChartStyle {
            hidden_series: vec![0],
            palette: Some(PaletteName::Ocean),
            series_colors: BTreeMap::from([(1, "#ff8000".into())]),
            ..Default::default()
        };
        let contrato = extrair(&m, &config);
        assert_eq!(contrato.metadados_series.len(), 1);
        assert_eq!(contrato.metadados_series[0].indice_origem, 1);
        assert_eq!(contrato.grafico.series[0].nome, "Meta");
        assert_eq!(contrato.grafico.series[0].cor, Cor::rgb(255, 128, 0));
        assert_eq!(contrato.grafico.series[0].valores, [4., 2.]);
    }

    #[test]
    fn dispersao_usa_x_numerico_da_fonte_e_nao_rotulo_da_categoria() {
        let m = valores();
        let mut config = configuracao();
        config.tipo = ChartType::Scatter;
        config.aggregate = false;
        config.has_category_labels = false;
        // As colunas de números são Vendas (X) e Meta (Y).
        let contrato = extrair(&m, &config);
        assert_eq!(contrato.grafico.tipo, Tipo::Dispersao);
        assert_eq!(contrato.grafico.categorias, ["10", "20", "30"]);
        assert_eq!(contrato.grafico.series[0].valores, [1., 2., 3.]);
        assert!(contrato.cena(500., 300.).is_ok());
    }

    #[test]
    fn area_e_degraus_usam_agregacao_original_do_sheet_e_rendem_svg() {
        let origem = valores();
        for (chart_type, tipo) in [
            (ChartType::Area, Tipo::Area),
            (ChartType::StepLine, Tipo::Degraus),
        ] {
            let mut config = configuracao();
            config.tipo = chart_type;
            let contrato = extrair(&origem, &config);
            assert_eq!(contrato.grafico.tipo, tipo);
            assert_eq!(contrato.grafico.categorias, ["A", "B"]);
            assert_eq!(contrato.grafico.series[0].valores, [40., 20.]);
            assert_eq!(contrato.grafico.series[1].valores, [4., 2.]);
            assert!(contrato.cena(800., 450.).is_ok());
            assert!(contrato.grafico.exportar_svg(800., 450.).unwrap().contains("<path"));
        }
    }

    #[test]
    fn nao_converte_tipo_especial_ou_nan_silenciosamente() {
        let mut config = configuracao();
        config.tipo = ChartType::Combo;
        assert_eq!(
            preparar_grafico(&config, extract_chart_data_with(&|_, _| Value::Empty, &config)),
            Err(ErroAdaptacaoGrafico::TipoNaoSuportado(ChartType::Combo))
        );
        config.tipo = ChartType::Column;
        let m = valores();
        let mut dados = extract_chart_data_with(
            &|r, c| m.get(&(r, c)).cloned().unwrap_or(Value::Empty), &config,
        );
        dados.series[0].values[0] = f64::NAN;
        assert_eq!(
            preparar_grafico(&config, dados),
            Err(ErroAdaptacaoGrafico::DadosInvalidos(ErroGrafico::ValorInvalido))
        );
    }

    #[test]
    fn hierarquia_de_datas_delega_agrupamento_ao_sheet() {
        let mut m = valores();
        for (r, date) in [(1, "01/01/2026"), (2, "02/01/2026"), (3, "05/02/2026")] {
            m.insert((r, 0), Value::Text(date.into()));
        }
        let mut config = configuracao();
        config.aggregate = false;
        config.style.date_hierarchy = Some(DateHierarchy::Month);
        let contrato = extrair(&m, &config);
        assert_eq!(contrato.grafico.categorias, ["01/2026", "02/2026"]);
        assert_eq!(contrato.grafico.series[0].valores, [30., 30.]);
        assert_eq!(contrato.metadados_series[0].agregacao.as_deref(), Some("Soma"));
    }

    #[test]
    fn preserva_todas_as_contas_do_motor_sheet_sem_reagregar() {
        let m = valores();
        let casos = [
            (AggregateFn::Sum, 40.0, 20.0),
            (AggregateFn::Avg, 20.0, 20.0),
            (AggregateFn::Count, 2.0, 1.0),
            (AggregateFn::Min, 10.0, 20.0),
            (AggregateFn::Max, 30.0, 20.0),
            (AggregateFn::Stddev, 200.0_f64.sqrt(), 0.0),
            (AggregateFn::Mode, 0.0, 0.0),
        ];
        for (conta, primeiro, segundo) in casos {
            let mut config = configuracao();
            config.aggregate_fn = Some(conta);
            let contrato = extrair(&m, &config);
            let v = &contrato.grafico.series[0].valores;
            assert!((v[0] - primeiro).abs() < 0.00001, "{conta:?}: {v:?}");
            assert!((v[1] - segundo).abs() < 0.00001, "{conta:?}: {v:?}");
        }
    }

    #[test]
    fn avisos_e_amostra_sao_passados_sem_perder_dados() {
        let m = valores();
        let config = configuracao();
        let mut dados = extract_chart_data_with(
            &|r, c| m.get(&(r, c)).cloned().unwrap_or(Value::Empty),
            &config,
        );
        dados.amostrado = Some(feathersheet_core::grafico::chart::Amostra {
            disponiveis: 12000, usados: 4096,
        });
        dados.aviso = Some("Amostra reduzida pela origem".into());
        let contrato = preparar_grafico(&config, dados).unwrap();
        assert_eq!(contrato.amostra.unwrap().usados, 4096);
        assert_eq!(contrato.avisos, ["Amostra reduzida pela origem"]);
        assert_eq!(contrato.grafico.series[0].valores, [40.0, 20.0]);
    }

    #[test]
    fn rejeita_cor_css_incompativel_sem_esconder_configuracao() {
        let m = valores();
        let mut config = configuracao();
        config.style.series_colors.insert(0, "linear-gradient(red, blue)".into());
        assert!(matches!(
            extrair_grafico_com(
                &|r, c| m.get(&(r, c)).cloned().unwrap_or(Value::Empty), &config,
            ),
            Err(ErroAdaptacaoGrafico::CorNaoSuportada { indice_origem: 0, .. })
        ));
    }
}
