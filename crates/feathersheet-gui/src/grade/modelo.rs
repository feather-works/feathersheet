//! Estrutura da grade e helpers leves, separados da rotina de desenho Canvas.
//! Nunca armazena ou materializa conteúdo de células.
use feathersheet_core::{celulas::refs::Rect, formula::ref_drag::Caixa, planilha::{workbook::Workbook,sheet::Sheet}};
use crate::condicional::Estado as EstadoCondicional;
use std::cell::RefCell;
use super::{LARGURA_PADRAO, ALTURA_PADRAO};

/// Metadados visuais adicionais do painel Formatar Células. Continuam no
/// EstiloParcial e no .fshz mesmo antes da migração integral do modelo Style.
#[derive(Default)]
pub(super) struct AjustesTexto {
    pub(super) vertical: bool,
    pub(super) reduzir: bool,
    /// Quebrar texto automaticamente (faixa "Quebrar", `wrapText` do .xlsx).
    pub(super) quebrar: bool,
    pub(super) recuo: f32,
    pub(super) sobrescrito: bool,
    pub(super) subscrito: bool,
    /// Sublinhado duplo (Página Inicial > Sublinhado ▾): dois traços.
    pub(super) sublinhado_duplo: bool,
}
pub(super) fn ajustes_texto(sheet: &Sheet, r: u32, c: u32) -> AjustesTexto {
    use feathersheet_core::celulas::estilo_parcial::Campo;
    let Some(estilo)=sheet.estilo_mesclado(r,c) else {return AjustesTexto::default()};
    let ativo=|chave:&str| matches!(estilo.get(chave),Some(Campo::Logico(true)));
    let recuo=match estilo.get("indent") {
        Some(Campo::Numero(v)) => (*v as f32).clamp(0.,50.),
        _ => 0.,
    };
    AjustesTexto {
        vertical: ativo("textVertical"),
        reduzir: ativo("shrinkToFit"),
        quebrar: ativo("wrap"),
        recuo,
        sobrescrito: ativo("superscript"),
        subscrito: ativo("subscript"),
        sublinhado_duplo: ativo(feathersheet_core::operacoes::sublinhado::CHAVE_DUPLO),
    }
}

/// Cache do desenho das células da grade (ver `Grade::draw`).
#[derive(Default)]
pub struct CacheGrade {
    pub desenho: iced::widget::canvas::Cache,
    /// Arrastes internos do Canvas no último desenho (referência, alça, divisória).
    pub mouse: RefCell<String>,
}

pub struct Grade<'a> {
    pub workbook: &'a Workbook,
    pub cache: &'a CacheGrade,
    /// Índices esparsos de dimensão, partilhados por desenho e hit-test.
    pub(crate) eixos: &'a RefCell<super::EstadoEixos>,
    pub condicional: &'a RefCell<EstadoCondicional>,
    pub aba: usize,
    pub linha_inicio: u32,
    pub coluna_inicio: u32,
    /// Primeiras linhas/colunas fixadas por aba sem copiar células.
    pub congelar_linhas: u32,
    pub congelar_colunas: u32,
    pub selecionada: (u32, u32),
    pub area: Rect,
    pub escuro: bool,
    pub linhas_grade: bool,
    /// Exibe os textos brutos (inclusive =fórmulas), sem alterar o Workbook.
    pub mostrar_formulas: bool,
    /// Preferências do usuário só para células sem fonte/tamanho explícitos.
    pub fonte_padrao: &'a str,
    pub tamanho_fonte_padrao: u16,
    /// Realça linha e coluna da célula ativa, sem alterar dados ou estilos.
    pub realce_eixos: bool,
    pub zoom: f32,
    /// Ctrl+roda aplica zoom no corpo da grade, sem deslocar as linhas.
    pub ctrl_pressionado: bool,
    pub rascunho: Option<&'a str>,
    pub refs_formula: Vec<Caixa>,
    /// Dropdown de validação da célula ativa (não faz parte dos dados salvos).
    pub lista_aberta: bool,
    /// Primeiro item à vista na lista suspensa (rolagem).
    pub lista_pagina: usize,
    /// Item marcado pelo teclado na lista suspensa.
    pub lista_destaque: usize,
    /// Valores da coluna pedidos com Alt+↓ (usados quando não há validação).
    pub opcoes_coluna: Option<&'a [String]>,
    /// Áreas anteriores da seleção múltipla (Ctrl+clique).
    pub areas_extras: &'a [Rect],
    /// Área de impressão e quebras da aba ativa (só visual, em memória).
    pub impressao: Option<super::ImpressaoVisual>,
    /// Área do Ctrl+C nesta aba: contorno tracejado.
    pub copiada: Option<Rect>,
    /// Setas de Fórmulas → Rastrear precedentes/dependentes da aba ativa.
    pub setas: &'a [crate::app::SetaRastreio],
    /// Balões de comentário à mostra (mouse, Mostrar todos, editor aberto).
    pub comentarios: super::VisaoComentarios,
    /// Células de "Circular dados inválidos" na aba ativa.
    pub circulos_validacao: &'a [(u32, u32)],
}

pub(crate) fn largura(w: &Workbook, aba: usize, coluna: u32) -> f32 {
    w.aba(aba).map_or(LARGURA_PADRAO, |a| if a.sheet.coluna_oculta(coluna) { 0.0 } else {
        a.sheet.col_widths.get(&coluna).map(|&v| v as f32).unwrap_or_else(|| super::largura_padrao_da(&a.sheet))
    })
}
/// Distância, em pixels lógicos, da borda esquerda da coluna A até a
/// coluna `coluna`, com as larguras reais da aba. Botões e formas usam esta
/// régua, a mesma da grade.
pub(crate) fn inicio_x(w: &Workbook, aba: usize, coluna: u32) -> f32 {
    w.aba(aba).map_or(coluna as f32 * LARGURA_PADRAO, |a| inicio_x_da(&a.sheet, coluna))
}

/// [`inicio_x`] direto na aba (para quem só tem o `Sheet`).
pub(crate) fn inicio_x_da(sheet: &feathersheet_core::planilha::sheet::Sheet, coluna: u32) -> f32 {
    let padrao = super::largura_padrao_da(sheet);
    // Colunas ocultas até o fim não somam largura.
    let fim = sheet.colunas_ocultas_desde.map_or(coluna, |n| coluna.min(n));
    (0..fim.min(feathersheet_core::MAX_COLS))
        .map(|c| sheet.col_widths.get(&c).map(|&v| v as f32).unwrap_or(padrao)).sum()
}

/// Como [`inicio_x`], para linhas: a altura padrão vezes as linhas, corrigida
/// só pelas linhas de altura própria (sem varrer um milhão de linhas).
pub(crate) fn inicio_y(w: &Workbook, aba: usize, linha: u32) -> f32 {
    w.aba(aba).map_or(linha as f32 * ALTURA_PADRAO, |a| inicio_y_da(&a.sheet, linha))
}

/// [`inicio_y`] direto na aba.
pub(crate) fn inicio_y_da(sheet: &feathersheet_core::planilha::sheet::Sheet, linha: u32) -> f32 {
    // Linhas ocultas até o fim não somam altura.
    let linha = sheet.linhas_ocultas_desde.map_or(linha, |n| linha.min(n));
    let proprias: f32 = sheet.row_heights.iter()
        .filter(|(r, _)| **r < linha).map(|(_, h)| *h as f32 - ALTURA_PADRAO).sum();
    linha as f32 * ALTURA_PADRAO + proprias
}

pub(crate) fn altura(w: &Workbook, aba: usize, linha: u32) -> f32 {
    match w.aba(aba) {
        Some(a) if a.sheet.linhas_ocultas_desde.is_some_and(|n| linha >= n) => 0.0,
        a => a.and_then(|a| a.sheet.row_heights.get(&linha).copied())
            .map(|v| v as f32).unwrap_or(ALTURA_PADRAO),
    }
}
