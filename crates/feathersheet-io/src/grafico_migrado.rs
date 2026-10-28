//! Fronteira Sheet -> motor Rust compartilhado dos 25 gráficos (CHT-018).
//! Não reagrupa dados; não modifica ColumnStore nem JSON FSHZ.
use feather_charts::{
    motor::{renderizar_svg, Entrada, TipoGrafico},
    Serie,
};
use feathersheet_core::grafico::chart::{
    extract_chart_data, get_series_color, ChartConfig, ChartData, ChartType,
};
use feathersheet_core::Sheet;
use crate::grafico_compartilhado::cor_css;

/// Mapeamento completo, sem cair silenciosamente em outro tipo visual.
pub fn tipo_compartilhado(tipo: ChartType) -> Option<TipoGrafico> {
    Some(match tipo {
        ChartType::Column => TipoGrafico::Colunas,
        ChartType::ColumnStacked => TipoGrafico::ColunasEmpilhadas,
        ChartType::ColumnStacked100 => TipoGrafico::Colunas100,
        ChartType::Bar => TipoGrafico::Barras,
        ChartType::BarStacked => TipoGrafico::BarrasEmpilhadas,
        ChartType::BarStacked100 => TipoGrafico::Barras100,
        ChartType::Line => TipoGrafico::Linhas,
        ChartType::Line3d => TipoGrafico::Linha3d,
        ChartType::StepLine => TipoGrafico::Degraus,
        ChartType::StepLineStacked => TipoGrafico::DegrausEmpilhados,
        ChartType::StepLineStacked100 => TipoGrafico::Degraus100,
        ChartType::Area => TipoGrafico::Area,
        ChartType::StepArea => TipoGrafico::AreaDegraus,
        ChartType::StepAreaStacked => TipoGrafico::AreaDegrausEmpilhada,
        ChartType::StepAreaStacked100 => TipoGrafico::AreaDegraus100,
        ChartType::Pie | ChartType::Doughnut => TipoGrafico::Pizza,
        ChartType::Scatter => TipoGrafico::Dispersao,
        ChartType::Bubble => TipoGrafico::Bolhas,
        ChartType::Combo => TipoGrafico::Combinado,
        ChartType::Heatmap => TipoGrafico::MapaCalor,
        ChartType::Waterfall => TipoGrafico::Cascata,
        ChartType::Histogram => TipoGrafico::Histograma,
        ChartType::Radar => TipoGrafico::Radar,
        ChartType::Candlestick => TipoGrafico::Candlestick,
        ChartType::Gantt => TipoGrafico::Gantt,
        ChartType::Outro => return None,
    })
}

/// O motor recebe os valores EXATOS já extraídos pelo FeatherSheet.
/// Formatos/estilos desconhecidos continuam preservados na configuração FSHZ.
pub fn renderizar_dados(
    config: &ChartConfig, dados: &ChartData, largura: f32, altura: f32,
) -> Result<String, String> {
    let tipo = tipo_compartilhado(config.tipo)
        .ok_or("O tipo do gráfico ainda não é reconhecido pelo motor Rust.")?;
    let series = dados.series.iter().enumerate().map(|(i, s)| {
        let cor = get_series_color(&config.style, i as u32);
        let cor = cor_css(&cor)
            .ok_or_else(|| format!("Cor de série inválida: {cor}"))?;
        Ok(Serie {
            nome: s.name.clone(),
            valores: s.values.clone(),
            cor,
        })
    }).collect::<Result<Vec<_>,String>>()?;
    let entrada = Entrada {
        titulo: dados.title.clone(),
        categorias: dados.categories.clone(),
        series,
        x_values: dados.x_values.clone(),
        z_values: dados.z_values.clone(),
    };
    renderizar_svg(tipo, &entrada, largura, altura)
        .map_err(|e| format!("Gráfico {}: {e:?}", tipo.id()))
}

/// Extrai a partir de Sheet, preservando locale/agrupamentos e totais originais.
pub fn renderizar_sheet(
    sheet: &Sheet, config: &ChartConfig, largura: f32, altura: f32,
) -> Result<String, String> {
    let dados = extract_chart_data(sheet, config);
    renderizar_dados(config, &dados, largura, altura)
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::grafico::chart::ChartSeries;

    fn data() -> ChartData {
        ChartData {
            title: "A".into(),
            categories: vec!["1".into(), "2".into()],
            series: vec![ChartSeries {
                name: "S".into(), values: vec![10., 20.],
            }],
            x_values: Some(vec![1.,2.]), z_values: Some(vec![2.,3.]),
            amostrado: None, aviso: None,
        }
    }
    #[test]
    fn mapeia_25_tipos_da_configuracao_legada() {
        let mut encontrados=std::collections::HashSet::new();
        for (id,tipo) in [
            ("column",ChartType::Column),("column-stacked",ChartType::ColumnStacked),
            ("column-stacked-100",ChartType::ColumnStacked100),
            ("bar",ChartType::Bar),("bar-stacked",ChartType::BarStacked),
            ("bar-stacked-100",ChartType::BarStacked100),
            ("line",ChartType::Line),("line-3d",ChartType::Line3d),
            ("step-line",ChartType::StepLine),("step-line-stacked",ChartType::StepLineStacked),
            ("step-line-stacked-100",ChartType::StepLineStacked100),
            ("area",ChartType::Area),("step-area",ChartType::StepArea),
            ("step-area-stacked",ChartType::StepAreaStacked),
            ("step-area-stacked-100",ChartType::StepAreaStacked100),
            ("pie",ChartType::Pie),("scatter",ChartType::Scatter),
            ("bubble",ChartType::Bubble),("combo",ChartType::Combo),
            ("heatmap",ChartType::Heatmap),("waterfall",ChartType::Waterfall),
            ("histogram",ChartType::Histogram),("radar",ChartType::Radar),
            ("candlestick",ChartType::Candlestick),("gantt",ChartType::Gantt),
        ] {
            let atual=tipo_compartilhado(tipo).unwrap();
            assert_eq!(atual.id(),id);
            assert!(encontrados.insert(atual));
        }
        assert_eq!(encontrados.len(),25);
    }
    #[test]
    fn svg_sem_refazer_agregacao_no_sheet() {
        let cfg=ChartConfig {tipo:ChartType::Column,..Default::default()};
        let svg=renderizar_dados(&cfg,&data(),480.,300.).unwrap();
        assert!(svg.contains("<svg"));
        assert!(!svg.contains("NaN"));
    }
}
