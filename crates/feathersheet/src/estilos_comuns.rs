//! TBL-013 — projeção leve do estilo do Sheet para o motor Tables.
//!
//! Não copia valores, fórmulas, referências, ranges nem atributos globais;
//! somente os atributos visuais da célula que o Canvas já vai desenhar.
//! Cores condicionais são sobreposições efêmeras: não alteram o .fshz.
use feather_ui::seletor_cor::{hex_valido, rgba_de};
use feathersheet_core::celulas::style::{Align, Style, VAlign};
use feathersheet_core::operacoes::rules::CondStyle;
use feather_tables::{
    AlinhamentoHorizontal, AlinhamentoVertical, EstiloCelula, EstiloCompleto, EstiloLayout,
};

#[derive(Clone, Debug, PartialEq)]
pub struct EstiloVisualSheet {
    /// Tipografia, preenchimentos, alinhamento e quebra de linha comuns.
    pub comum: EstiloCompleto,
    /// A API básica Tables ainda não possui tachado; mantido separado até
    /// uma migração compatível com os outros apps.
    pub riscado: bool,
}

fn cor(input: Option<&str>) -> Option<[u8; 4]> {
    input.filter(|c| hex_valido(c)).map(rgba_de)
}

/// Adapta um estilo *resolvido* da grade esparsa: custo O(1) por célula
/// visível, nenhuma varredura da planilha ou cache de milhões de células.
/// Não converte formatos numéricos: format_number continua em Sheet core.
pub fn projetar_estilo(
    estilo: Option<&Style>,
    condicional: Option<&CondStyle>,
) -> EstiloVisualSheet {
    let base = estilo.cloned().unwrap_or_default();
    let mut basico = EstiloCelula {
        fonte: base.font_family,
        tamanho: base.font_size.filter(|v| v.is_finite() && *v > 0.0),
        negrito: base.bold,
        italico: base.italic,
        sublinhado: base.underline,
        fundo: cor(base.bg.as_deref()),
        texto: cor(base.color.as_deref()),
        quebrar_linha: base.wrap,
        ..EstiloCelula::default()
    };
    let layout = EstiloLayout {
        horizontal: match base.align {
            Align::Auto | Align::Left => AlinhamentoHorizontal::Inicio,
            Align::Center => AlinhamentoHorizontal::Centro,
            Align::Right => AlinhamentoHorizontal::Fim,
        },
        vertical: match base.valign {
            VAlign::Top => AlinhamentoVertical::Superior,
            VAlign::Middle => AlinhamentoVertical::Centro,
            VAlign::Bottom => AlinhamentoVertical::Inferior,
        },
        ..EstiloLayout::default()
    };
    let mut riscado = base.strike;
    if let Some(c) = condicional {
        // false explícito desliga negrito/itálico/sublinhado/tachado legados.
        if let Some(v) = c.bold { basico.negrito = v; }
        if let Some(v) = c.italic { basico.italico = v; }
        if let Some(v) = c.underline { basico.sublinhado = v; }
        if let Some(v) = c.strike { riscado = v; }
        if let Some(v) = cor(c.bg.as_deref()) { basico.fundo = Some(v); }
        if let Some(v) = cor(c.color.as_deref()) { basico.texto = Some(v); }
    }
    EstiloVisualSheet { comum: EstiloCompleto { basico, layout }, riscado }
}

/// A letra do texto da célula no pelicano, pelas mesmas regras da grade.
pub fn fonte_da_celula(visual: &EstiloVisualSheet, padrao: &str) -> pelicano::Font {
    let familia = visual.comum.basico.fonte.as_deref().unwrap_or(padrao);
    pelicano::Font {
        family: if familia.to_ascii_lowercase().contains("mono") {
            Some("monospace")
        } else if familia.contains("serif") || familia.contains("Georgia") || familia.contains("Times New Roman") {
            Some("serif")
        } else { None },
        weight: if visual.comum.basico.negrito { 700 } else { 400 },
        italic: visual.comum.basico.italico,
        ..pelicano::Font::DEFAULT
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn estilo_ausente_vira_padroes_sem_apagar_valor_nem_engine() {
        let estilo = projetar_estilo(None, None);
        assert_eq!(estilo.comum.basico, EstiloCelula::default());
        assert_eq!(estilo.comum.layout.vertical, AlinhamentoVertical::Centro);
        assert_eq!(estilo.comum.layout.horizontal, AlinhamentoHorizontal::Inicio);
        assert!(!estilo.riscado);
    }

    #[test]
    fn tipografia_alinhamento_cores_unicode_e_espacamento_comuns() {
        let base = Style {
            bold: true, italic: true, underline: true, strike: true,
            font_family: Some("Noto Sans".into()), font_size: Some(16.0),
            wrap: true, align: Align::Right, valign: VAlign::Bottom,
            bg: Some("#11223344".into()), color: Some("#aabbcc".into()),
            ..Style::default()
        };
        let antes = base.clone();
        let p = projetar_estilo(Some(&base), None);
        assert_eq!(base, antes);
        assert_eq!(p.comum.basico.fonte.as_deref(), Some("Noto Sans"));
        assert_eq!(p.comum.basico.tamanho, Some(16.0));
        assert!(p.comum.basico.negrito && p.comum.basico.italico);
        assert!(p.comum.basico.sublinhado && p.comum.basico.quebrar_linha);
        assert!(p.riscado);
        assert_eq!(p.comum.basico.fundo, Some([17, 34, 51, 68]));
        assert_eq!(p.comum.basico.texto, Some([170, 187, 204, 255]));
        assert_eq!(p.comum.layout.horizontal, AlinhamentoHorizontal::Fim);
        assert_eq!(p.comum.layout.vertical, AlinhamentoVertical::Inferior);
    }

    #[test]
    fn condicional_sobrepoe_sem_alterar_estilo_original_e_false_vence() {
        let base = Style {
            bold: true, italic: true, underline: true, strike: true,
            color: Some("#111111".into()), bg: Some("#eeeeee".into()),
            ..Style::default()
        };
        let cond = CondStyle {
            bold: Some(false), italic: Some(false),
            underline: Some(false), strike: Some(false),
            color: Some("#12345678".into()), bg: Some("#010203".into()),
        };
        let before = (base.clone(), cond.clone());
        let p = projetar_estilo(Some(&base), Some(&cond));
        assert!(!p.comum.basico.negrito && !p.comum.basico.italico);
        assert!(!p.comum.basico.sublinhado && !p.riscado);
        assert_eq!(p.comum.basico.texto, Some([18, 52, 86, 120]));
        assert_eq!(p.comum.basico.fundo, Some([1, 2, 3, 255]));
        assert_eq!(before, (base, cond));
    }

    #[test]
    fn cores_invalidas_nao_apagam_cor_existente() {
        let base = Style {
            bg: Some("#123456".into()), color: Some("#abcdef".into()),
            ..Style::default()
        };
        let cond = CondStyle {
            bg: Some("não hexadecimal".into()),
            color: Some("#1234zz".into()),
            ..CondStyle::default()
        };
        let p = projetar_estilo(Some(&base), Some(&cond));
        assert_eq!(p.comum.basico.fundo, Some([18, 52, 86, 255]));
        assert_eq!(p.comum.basico.texto, Some([171, 205, 239, 255]));
    }

    #[test]
    fn fonte_invalida_retorna_ao_padrao_e_alinhamento_automatico_sem_inferir_tipo() {
        let base = Style {
            font_size: Some(f32::NAN),
            align: Align::Auto, valign: VAlign::Top,
            ..Style::default()
        };
        let p = projetar_estilo(Some(&base), None);
        assert_eq!(p.comum.basico.tamanho, None);
        assert_eq!(p.comum.layout.horizontal, AlinhamentoHorizontal::Inicio);
        assert_eq!(p.comum.layout.vertical, AlinhamentoVertical::Superior);
    }

    #[test]
    fn integra_estilo_da_ultima_linha_real_sem_materializar_milhoes() {
        use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
        use feathersheet_core::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
        use feathersheet_core::planilha::workbook::Workbook;

        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 1, 1, "=2+3".into());
        let mut patch = EstiloParcial::new();
        patch.insert("bold".into(), Campo::Logico(true));
        patch.insert("color".into(), Campo::Texto("#123456".into()));
        // A última célula existe apenas na estrutura esparsa do Sheet:
        // projetá-la não percorre 100 milhões de linhas nem 16.384 colunas.
        let row = MAX_ROWS - 1;
        let col = MAX_COLS - 1;
        pasta.editar(0, |s| s.set_style(&Rect::cell(row, col), &patch));
        let sheet = &pasta.aba(0).unwrap().sheet;
        let style = sheet.estilo_resolvido(row, col).unwrap();
        let before = (sheet.get_raw(1, 1), sheet.get_raw(row, col));
        let projected = projetar_estilo(Some(&style), None);
        assert!(projected.comum.basico.negrito);
        assert_eq!(projected.comum.basico.texto, Some([18, 52, 86, 255]));
        assert_eq!((sheet.get_raw(1, 1), sheet.get_raw(row, col)), before);
        assert_eq!(sheet.get_raw(1, 1), "=2+3");
    }

    #[test]
    fn estilos_compartilhados_nao_dependem_de_instanciar_tabela_dense() {
        // Grandeza de linha/coluna afeta somente a grade virtual: o projetor
        // recebe exclusivamente 2 estilos visíveis, independentemente dos
        // limites de 100 milhões de linhas do engine.
        let base = Style { bold: true, ..Style::default() };
        let first = projetar_estilo(Some(&base), None);
        let last = projetar_estilo(Some(&base), None);
        assert_eq!(first, last);
        assert_eq!(std::mem::size_of::<EstiloVisualSheet>(),
            std::mem::size_of::<EstiloVisualSheet>());
    }
}
