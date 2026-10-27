//! Impressão da aba em PDF de várias páginas — o que o `window.print()` de
//! uma `<table>` HTML fazia no TypeScript (`print-dialog.ts`, `executePrint`):
//! cabeçalho "Planilha: nome / data", rodapé, linhas de grade e cabeçalhos
//! A/B/1/2 opcionais, escala, orientação e papel.
//!
//! Tudo em pontos (1 px da planilha = 0,75 pt). Origem no canto superior
//! esquerdo da página, y para baixo. Só Helvetica (fontes padrão do PDF).

use feather_draw::pdf::{largura_texto, DocumentoPdf, FontePdf, PaginaPdf};
use feather_draw::Cor;
use feathersheet_core::celulas::refs::{col_name, Rect};
use feathersheet_core::celulas::style::{effective_align, format_number, Align, Style};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::impressao::{AreaImpressao, ConfigPagina, Escala, Margens};
use feathersheet_core::planilha::workbook::Workbook;

const PX: f32 = 0.75;
const LARGURA_PADRAO_PX: f32 = 122.0;
const ALTURA_PADRAO_PX: f32 = 28.0;
/// `font-size: 10pt` do CSS de impressão.
const FONTE: f32 = 10.0;
/// `padding: 5px 8px` das células.
const PAD_X: f32 = 8.0 * PX;
/// Altura reservada ao cabeçalho e ao rodapé da página, em pt.
const ALTURA_CABECALHO: f32 = 26.0;
const ALTURA_RODAPE: f32 = 26.0;
/// Altura da linha de cabeçalhos de coluna (A, B, ...).
const ALTURA_CAB_COLUNA: f32 = 10.0 * 1.2 + 10.0 * PX;

const VERDE: Cor = Cor::rgb(0x0d, 0x65, 0x2d);
const TEXTO: Cor = Cor::rgb(0x1f, 0x1f, 0x1f);
const GRADE: Cor = Cor::rgb(0xb0, 0xb0, 0xb0);
const CAB_FUNDO: Cor = Cor::rgb(0xf1, 0xf3, 0xf4);
const CAB_TEXTO: Cor = Cor::rgb(0x44, 0x44, 0x44);
const CINZA_RODAPE: Cor = Cor::rgb(0x77, 0x77, 0x77);

/// Uma faixa de linhas ou colunas visíveis: índice, posição e tamanho (pt,
/// antes da escala).
type Eixo = Vec<(u32, f32)>;

/// Gera o PDF da área da aba `folha`.
pub fn gerar(
    wb: &Workbook,
    folha: usize,
    area: AreaImpressao,
    config: &ConfigPagina,
) -> Result<Vec<u8>, String> {
    gerar_varias(wb, folha, &[area], config)
}

/// As páginas de uma área: o que mede e como quebra.
struct Plano {
    colunas: Eixo,
    linhas: Eixo,
    mesclas: Vec<Rect>,
    larg_cab_linha: f32,
    alt_cab_coluna: f32,
    escala: f32,
    faixas_col: Vec<std::ops::Range<usize>>,
    faixas_lin: Vec<std::ops::Range<usize>>,
}

fn plano_da_area(wb: &Workbook, folha: usize, area: AreaImpressao, config: &ConfigPagina) -> Result<Plano, String> {
    let aba = wb.aba(folha).ok_or_else(|| format!("aba {folha} não existe"))?;
    let sheet = &aba.sheet;
    let (r0, r1) = (area.linha0.min(area.linha1), area.linha0.max(area.linha1));
    let (c0, c1) = (area.coluna0.min(area.coluna1), area.coluna0.max(area.coluna1));

    // Eixos visíveis (tamanho 0 = oculto). Ocultas "até o fim" cortam o
    // intervalo antes de percorrer: a área pode ir até a última linha.
    let c_fim = sheet.colunas_ocultas_desde.map_or(c1, |n| c1.min(n.saturating_sub(1)));
    let r_fim = sheet.linhas_ocultas_desde.map_or(r1, |n| r1.min(n.saturating_sub(1)));
    let colunas: Eixo = (c0..=c_fim)
        .filter(|&c| !sheet.coluna_oculta(c))
        .filter_map(|c| {
            let w = sheet.col_widths.get(&c).map(|v| *v as f32).unwrap_or(LARGURA_PADRAO_PX);
            (w > 0.0).then_some((c, w * PX))
        })
        .collect();
    let linhas: Eixo = (r0..=r_fim)
        .filter(|&r| !sheet.linha_oculta(r))
        .filter_map(|r| {
            let h = sheet.row_heights.get(&r).map(|v| *v as f32).unwrap_or(ALTURA_PADRAO_PX);
            (h > 0.0).then_some((r, h * PX))
        })
        .collect();

    // Mesclas que tocam a área.
    let mesclas: Vec<Rect> = sheet
        .mc
        .as_deref()
        .and_then(|j| serde_json::from_str::<Vec<Rect>>(j).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.normalized())
        .filter(|m| m.r0 <= r1 && m.r1 >= r0 && m.c0 <= c1 && m.c1 >= c0)
        .collect();

    let cabecalhos = config.cabecalhos;
    let larg_cab_linha = if cabecalhos {
        largura_texto(&(r1 + 1).to_string(), FontePdf::Negrito, FONTE) + 2.0 * PAD_X
    } else {
        0.0
    };
    let alt_cab_coluna = if cabecalhos { ALTURA_CAB_COLUNA } else { 0.0 };

    // Área útil da folha.
    let (util_l, util_a) = config.area_util_pt();
    let util_a = util_a - ALTURA_CABECALHO - ALTURA_RODAPE;
    if util_l <= 1.0 || util_a <= 1.0 {
        return Err("margens maiores que a folha".into());
    }

    // Escala.
    let total_l = larg_cab_linha + colunas.iter().map(|c| c.1).sum::<f32>();
    let total_a = alt_cab_coluna + linhas.iter().map(|l| l.1).sum::<f32>();
    let escala = match config.escala {
        Escala::Porcentagem(p) => (p as f32 / 100.0).clamp(0.1, 4.0),
        Escala::Paginas { largura, altura } => {
            let limite = |n: u16, util: f32, total: f32| if n == 0 { 1.0 } else { util * n as f32 / total.max(1.0) };
            limite(largura, util_l, total_l).min(limite(altura, util_a, total_a)).min(1.0)
        }
    }
    .max(0.05);

    // Quebra em páginas: descer, depois direita.
    let faixas_col = quebrar(&colunas, (util_l / escala - larg_cab_linha).max(1.0));
    let faixas_lin = quebrar(&linhas, (util_a / escala - alt_cab_coluna).max(1.0));

    Ok(Plano { colunas, linhas, mesclas, larg_cab_linha, alt_cab_coluna, escala, faixas_col, faixas_lin })
}

/// Gera o PDF de várias áreas da aba `folha` (as áreas de impressão do
/// Excel): cada área começa numa página nova, na ordem, e a numeração das
/// páginas segue de uma para a outra.
pub fn gerar_varias(
    wb: &Workbook,
    folha: usize,
    areas: &[AreaImpressao],
    config: &ConfigPagina,
) -> Result<Vec<u8>, String> {
    let aba = wb.aba(folha).ok_or_else(|| format!("aba {folha} não existe"))?;
    let sheet = &aba.sheet;
    if areas.is_empty() {
        return Err("nenhuma área para imprimir".into());
    }
    let planos: Vec<Plano> = areas.iter().map(|a| plano_da_area(wb, folha, *a, config)).collect::<Result<_, _>>()?;
    let (pl, pa) = {
        let (l, a) = config.papel.tamanho_pt();
        match config.orientacao {
            feathersheet_core::planilha::impressao::Orientacao::Retrato => (l, a),
            feathersheet_core::planilha::impressao::Orientacao::Paisagem => (a, l),
        }
    };
    let Margens { esquerda_mm, topo_mm, .. } = config.margens;
    let mm = 72.0 / 25.4;
    let (margem_x, margem_y) = (esquerda_mm * mm, topo_mm * mm);
    let (util_l, util_a) = config.area_util_pt();
    let util_a = util_a - ALTURA_CABECALHO - ALTURA_RODAPE;
    let total_paginas: usize = planos.iter().map(|p| p.faixas_col.len() * p.faixas_lin.len()).sum();

    let nome = if sheet.name.is_empty() { "Sem título" } else { sheet.name.as_str() };
    let data = data_hoje();
    let mut doc = DocumentoPdf::novo();
    doc.substituir_nao_suportados(true);

    let mut n = 0usize;
    for plano in &planos {
        for fc in &plano.faixas_col {
            for fl in &plano.faixas_lin {
                n += 1;
                let p = doc.adicionar_pagina(pl, pa);
                let ctx = Ctx {
                    wb,
                    folha,
                    config,
                    escala: plano.escala,
                    ox: margem_x,
                    oy: margem_y + ALTURA_CABECALHO,
                    larg_cab: plano.larg_cab_linha,
                    alt_cab: plano.alt_cab_coluna,
                    mesclas: &plano.mesclas,
                };
                cabecalho_pagina(p, config, nome, &data, margem_x, margem_y, util_l);
                desenhar_tabela(p, &ctx, &plano.colunas[fc.clone()], &plano.linhas[fl.clone()], util_l, util_a);
                let ry = margem_y + ALTURA_CABECALHO + util_a + 10.0;
                rodape_pagina(p, config, n, total_paginas, margem_x, ry, util_l);
            }
        }
    }
    doc.finalizar().map_err(|e| format!("{e:?}"))
}

/// Parte o eixo em faixas que cabem em `disponivel` (ao menos um item).
fn quebrar(eixo: &Eixo, disponivel: f32) -> Vec<std::ops::Range<usize>> {
    let mut faixas = Vec::new();
    let (mut ini, mut soma) = (0usize, 0f32);
    for (i, (_, t)) in eixo.iter().enumerate() {
        if i > ini && soma + t > disponivel + 0.01 {
            faixas.push(ini..i);
            ini = i;
            soma = 0.0;
        }
        soma += t;
    }
    faixas.push(ini..eixo.len());
    faixas
}

fn cabecalho_pagina(p: &mut PaginaPdf, cfg: &ConfigPagina, nome: &str, data: &str, x: f32, y: f32, l: f32) {
    let esq = if cfg.cabecalho.trim().is_empty() {
        format!("Planilha: {nome}")
    } else {
        cfg.cabecalho.clone()
    };
    p.texto(x, y + 10.0, &esq, FontePdf::Negrito, FONTE, VERDE);
    let w = largura_texto(data, FontePdf::Negrito, FONTE);
    p.texto(x + l - w, y + 10.0, data, FontePdf::Negrito, FONTE, VERDE);
    p.linha(x, y + 16.0, x + l, y + 16.0, VERDE, 1.5, false);
}

fn rodape_pagina(p: &mut PaginaPdf, cfg: &ConfigPagina, n: usize, total: usize, x: f32, y: f32, l: f32) {
    p.linha(x, y, x + l, y, Cor::rgb(0xcc, 0xcc, 0xcc), 0.75, true);
    let esq = if cfg.rodape.trim().is_empty() { "FeatherSheet Document" } else { cfg.rodape.as_str() };
    p.texto(x, y + 14.0, esq, FontePdf::Helvetica, 9.0, CINZA_RODAPE);
    let dir = format!("Página {n} de {total}");
    let w = largura_texto(&dir, FontePdf::Helvetica, 9.0);
    p.texto(x + l - w, y + 14.0, &dir, FontePdf::Helvetica, 9.0, CINZA_RODAPE);
}

struct Ctx<'a> {
    wb: &'a Workbook,
    folha: usize,
    config: &'a ConfigPagina,
    escala: f32,
    ox: f32,
    oy: f32,
    larg_cab: f32,
    alt_cab: f32,
    mesclas: &'a [Rect],
}

fn desenhar_tabela(p: &mut PaginaPdf, ctx: &Ctx, cols: &[(u32, f32)], lins: &[(u32, f32)], util_l: f32, util_a: f32) {
    let s = ctx.escala;
    // Posições acumuladas (já em pt de página, escaladas).
    let x0 = ctx.ox + ctx.larg_cab * s;
    let y0 = ctx.oy + ctx.alt_cab * s;
    let mut xs = Vec::with_capacity(cols.len() + 1);
    let mut acc = x0;
    xs.push(acc);
    for (_, w) in cols {
        acc += w * s;
        xs.push(acc);
    }
    let mut ys = Vec::with_capacity(lins.len() + 1);
    let mut acc = y0;
    ys.push(acc);
    for (_, h) in lins {
        acc += h * s;
        ys.push(acc);
    }
    let (fim_x, fim_y) = (*xs.last().unwrap(), *ys.last().unwrap());
    let corte = (ctx.ox, ctx.oy, util_l, util_a);
    p.recorte(corte.0, corte.1, corte.2 + 0.5, corte.3 + 0.5);

    let sheet = &ctx.wb.aba(ctx.folha).unwrap().sheet;
    let em_mescla = |r: u32, c: u32| ctx.mesclas.iter().any(|m| m.contains(r, c));
    let grade = ctx.config.linhas_de_grade;

    // 1. fundos das células normais
    for (i, (r, _)) in lins.iter().enumerate() {
        for (j, (c, _)) in cols.iter().enumerate() {
            if em_mescla(*r, *c) {
                continue;
            }
            if let Some(cor) = sheet.estilo_resolvido(*r, *c).and_then(|e| e.bg.as_deref().and_then(cor_css)) {
                p.retangulo_preenchido(xs[j], ys[i], xs[j + 1] - xs[j], ys[i + 1] - ys[i], cor);
            }
        }
    }

    // 2. cabeçalhos
    if ctx.config.cabecalhos {
        let borda = if grade { Some(GRADE) } else { None };
        let fs = FONTE * s;
        p.retangulo_preenchido(ctx.ox, ctx.oy, ctx.larg_cab * s, ctx.alt_cab * s, CAB_FUNDO);
        for (j, (c, _)) in cols.iter().enumerate() {
            let (x, w) = (xs[j], xs[j + 1] - xs[j]);
            p.retangulo_preenchido(x, ctx.oy, w, ctx.alt_cab * s, CAB_FUNDO);
            texto_centro(p, x, ctx.oy, w, ctx.alt_cab * s, &col_name(*c), fs);
        }
        for (i, (r, _)) in lins.iter().enumerate() {
            let (y, h) = (ys[i], ys[i + 1] - ys[i]);
            p.retangulo_preenchido(ctx.ox, y, ctx.larg_cab * s, h, CAB_FUNDO);
            texto_centro(p, ctx.ox, y, ctx.larg_cab * s, h, &(r + 1).to_string(), fs);
        }
        if let Some(b) = borda {
            // contorno dos cabeçalhos
            p.linha(ctx.ox, ctx.oy, fim_x, ctx.oy, b, 0.5, false);
            p.linha(ctx.ox, ctx.oy, ctx.ox, fim_y, b, 0.5, false);
            p.linha(ctx.ox, y0, fim_x, y0, b, 0.5, false);
            p.linha(x0, ctx.oy, x0, fim_y, b, 0.5, false);
            for x in &xs[1..] {
                p.linha(*x, ctx.oy, *x, y0, b, 0.5, false);
            }
            for y in &ys[1..] {
                p.linha(ctx.ox, *y, x0, *y, b, 0.5, false);
            }
        }
    }

    // 3. linhas de grade (sobre os fundos normais; as mesclas cobrem depois)
    if grade {
        p.linha(x0, y0, fim_x, y0, GRADE, 0.5, false);
        p.linha(x0, y0, x0, fim_y, GRADE, 0.5, false);
        for x in &xs[1..] {
            p.linha(*x, y0, *x, fim_y, GRADE, 0.5, false);
        }
        for y in &ys[1..] {
            p.linha(x0, *y, fim_x, *y, GRADE, 0.5, false);
        }
    }

    // 4. mesclas que aparecem nesta página
    for m in ctx.mesclas {
        let js: Vec<usize> = cols.iter().enumerate().filter(|(_, (c, _))| *c >= m.c0 && *c <= m.c1).map(|(j, _)| j).collect();
        let is: Vec<usize> = lins.iter().enumerate().filter(|(_, (r, _))| *r >= m.r0 && *r <= m.r1).map(|(i, _)| i).collect();
        if js.is_empty() || is.is_empty() {
            continue;
        }
        let (xa, xb) = (xs[*js.first().unwrap()], xs[js.last().unwrap() + 1]);
        let (ya, yb) = (ys[*is.first().unwrap()], ys[is.last().unwrap() + 1]);
        let fundo = sheet.estilo_resolvido(m.r0, m.c0).and_then(|e| e.bg.as_deref().and_then(cor_css));
        p.retangulo_preenchido(xa, ya, xb - xa, yb - ya, fundo.unwrap_or(Cor::rgb(255, 255, 255)));
        if grade {
            p.retangulo_contorno(xa, ya, xb - xa, yb - ya, GRADE, 0.5);
        }
    }

    // 5. bordas e textos
    for (i, (r, _)) in lins.iter().enumerate() {
        for (j, (c, _)) in cols.iter().enumerate() {
            if em_mescla(*r, *c) {
                continue;
            }
            celula(p, ctx, *r, *c, xs[j], ys[i], xs[j + 1] - xs[j], ys[i + 1] - ys[i]);
        }
    }
    for m in ctx.mesclas {
        let js: Vec<usize> = cols.iter().enumerate().filter(|(_, (c, _))| *c >= m.c0 && *c <= m.c1).map(|(j, _)| j).collect();
        let is: Vec<usize> = lins.iter().enumerate().filter(|(_, (r, _))| *r >= m.r0 && *r <= m.r1).map(|(i, _)| i).collect();
        if js.is_empty() || is.is_empty() {
            continue;
        }
        let (xa, xb) = (xs[*js.first().unwrap()], xs[js.last().unwrap() + 1]);
        let (ya, yb) = (ys[*is.first().unwrap()], ys[is.last().unwrap() + 1]);
        celula(p, ctx, m.r0, m.c0, xa, ya, xb - xa, yb - ya);
    }
    p.fim_recorte();
}

/// Texto, bordas e decoração de uma célula (o fundo já foi pintado).
fn celula(p: &mut PaginaPdf, ctx: &Ctx, r: u32, c: u32, x: f32, y: f32, w: f32, h: f32) {
    let s = ctx.escala;
    let sheet = &ctx.wb.aba(ctx.folha).unwrap().sheet;
    let estilo: Option<Style> = sheet.estilo_resolvido(r, c);
    let valor = ctx.wb.valor(ctx.folha, r, c);
    let texto = texto_exibido(&valor, estilo.as_ref());

    if let Some(b) = estilo.as_ref().and_then(|e| e.borders.as_ref()) {
        let cor = b.color.as_deref().and_then(cor_css).unwrap_or(Cor::rgb(0x44, 0x44, 0x44));
        let e = 1.1 * s;
        if b.top { p.linha(x, y, x + w, y, cor, e, false); }
        if b.bottom { p.linha(x, y + h, x + w, y + h, cor, e, false); }
        if b.left { p.linha(x, y, x, y + h, cor, e, false); }
        if b.right { p.linha(x + w, y, x + w, y + h, cor, e, false); }
    }
    if texto.is_empty() {
        return;
    }
    let negrito = estilo.as_ref().is_some_and(|e| e.bold);
    let italico = estilo.as_ref().is_some_and(|e| e.italic);
    let fonte = match (negrito, italico) {
        (false, false) => FontePdf::Helvetica,
        (true, false) => FontePdf::Negrito,
        (false, true) => FontePdf::Italico,
        (true, true) => FontePdf::NegritoItalico,
    };
    let cor = estilo.as_ref().and_then(|e| e.color.as_deref().and_then(cor_css)).unwrap_or(TEXTO);
    let fs = FONTE * s;
    let tw = largura_texto(&texto, fonte, fs);
    let pad = PAD_X * s;
    let tx = match effective_align(&valor, estilo.as_ref()) {
        Align::Right => x + w - pad - tw,
        Align::Center => x + (w - tw) / 2.0,
        _ => x + pad,
    };
    let base = y + h / 2.0 + fs * 0.35;
    let corta = tw > w - 2.0 * pad;
    if corta {
        p.recorte(x, y, w, h);
    }
    p.texto(tx, base, &texto, fonte, fs, cor);
    if estilo.as_ref().is_some_and(|e| e.underline) {
        p.linha(tx, base + fs * 0.12, tx + tw, base + fs * 0.12, cor, (0.07 * fs).max(0.3), false);
    }
    if estilo.as_ref().is_some_and(|e| e.strike) {
        p.linha(tx, base - fs * 0.3, tx + tw, base - fs * 0.3, cor, (0.07 * fs).max(0.3), false);
    }
    if corta {
        p.fim_recorte();
    }
}

fn texto_centro(p: &mut PaginaPdf, x: f32, y: f32, w: f32, h: f32, t: &str, fs: f32) {
    let tw = largura_texto(t, FontePdf::Negrito, fs);
    p.texto(x + (w - tw) / 2.0, y + h / 2.0 + fs * 0.35, t, FontePdf::Negrito, fs, CAB_TEXTO);
}

/// O mesmo texto que a grade mostra (`grade/auxiliares.rs::texto`).
fn texto_exibido(valor: &Value, estilo: Option<&Style>) -> String {
    let t = match valor {
        Value::Number(n) => format_number(*n, estilo),
        Value::Empty => String::new(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
        Value::Error(e) => e.code.as_str().into(),
    };
    t.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// `#rgb` ou `#rrggbb`.
fn cor_css(s: &str) -> Option<Cor> {
    let h = s.trim().strip_prefix('#')?;
    // Não dividir UTF-8 por offsets de byte sem validar a entrada.
    if !h.is_ascii() {
        return None;
    }
    let v = |a: &str| u8::from_str_radix(a, 16).ok();
    match h.len() {
        6 => Some(Cor::rgb(v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?)),
        3 => {
            let d = |i: usize| v(&h[i..i + 1]).map(|x| x * 17);
            Some(Cor::rgb(d(0)?, d(1)?, d(2)?))
        }
        _ => None,
    }
}

/// Data de hoje (UTC) em dd/mm/aaaa.
fn data_hoje() -> String {
    let segs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = segs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!("{dia:02}/{mes:02}/{ano}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cor_com_acento_nao_entra_em_panico() {
        assert!(cor_css("#1é234").is_none());
        assert!(cor_css("#aé").is_none());
        assert!(cor_css("#ff8800").is_some());
    }

    fn pasta(linhas: u32) -> Workbook {
        let mut wb = Workbook::new();
        if wb.aba(0).is_none() {
            wb.criar_aba("Dados");
        }
        let s = &mut wb.aba_mut(0).unwrap().sheet;
        for r in 0..linhas {
            s.set_cell(r, 0, &format!("Linha {r} ação"));
            s.set_cell(r, 1, &format!("{}", r as f64 * 1.5));
        }
        wb
    }

    fn contar(pdf: &[u8]) -> usize {
        let t = String::from_utf8_lossy(pdf);
        let i = t.find("/Count ").unwrap() + 7;
        t[i..].split_whitespace().next().unwrap().parse().unwrap()
    }

    fn area(l: u32, c: u32) -> AreaImpressao {
        AreaImpressao { linha0: 0, coluna0: 0, linha1: l, coluna1: c }
    }

    #[test]
    fn planilha_pequena_uma_pagina() {
        let wb = pasta(5);
        let pdf = gerar(&wb, 0, area(4, 1), &ConfigPagina::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(contar(&pdf), 1);
    }

    #[test]
    fn duzentas_linhas_varias_paginas() {
        let wb = pasta(200);
        let mut cfg = ConfigPagina::default();
        cfg.linhas_de_grade = true;
        cfg.cabecalhos = true;
        let pdf = gerar(&wb, 0, area(199, 1), &cfg).unwrap();
        assert!(contar(&pdf) > 3, "{}", contar(&pdf));
        let pdf = gerar(&wb, 0, area(199, 1), &ConfigPagina { escala: Escala::Paginas { largura: 1, altura: 1 }, ..cfg.clone() }).unwrap();
        assert_eq!(contar(&pdf), 1);
    }

    #[test]
    fn colunas_largas_quebram_para_a_direita() {
        let mut wb = pasta(3);
        for c in 0..12 {
            wb.aba_mut(0).unwrap().sheet.col_widths.insert(c, 200);
        }
        let pdf = gerar(&wb, 0, area(2, 11), &ConfigPagina::default()).unwrap();
        assert!(contar(&pdf) >= 4);
        let pdf = gerar(&wb, 0, area(2, 11), &ConfigPagina { escala: Escala::Paginas { largura: 1, altura: 0 }, ..Default::default() }).unwrap();
        assert_eq!(contar(&pdf), 1);
        let pdf = gerar(&wb, 0, area(2, 11), &ConfigPagina { escala: Escala::Paginas { largura: 2, altura: 0 }, ..Default::default() }).unwrap();
        assert_eq!(contar(&pdf), 2);
    }

    #[test]
    fn oculta_e_aba_inexistente() {
        let mut wb = pasta(3);
        wb.aba_mut(0).unwrap().sheet.row_heights.insert(1, 0);
        assert!(gerar(&wb, 0, area(2, 1), &ConfigPagina::default()).is_ok());
        assert!(gerar(&wb, 9, area(2, 1), &ConfigPagina::default()).is_err());
    }

    #[test]
    fn ocultas_ate_o_fim_ficam_fora() {
        let mut wb = pasta(50);
        wb.aba_mut(0).unwrap().sheet.linhas_ocultas_desde = Some(10);
        wb.aba_mut(0).unwrap().sheet.colunas_ocultas_desde = Some(1);
        let plano = plano_da_area(&wb, 0, area(49, 1), &ConfigPagina::default()).unwrap();
        assert_eq!(plano.linhas.len(), 10);
        assert_eq!(plano.colunas.len(), 1);
        // Tudo oculto: c0 já passa do corte.
        wb.aba_mut(0).unwrap().sheet.linhas_ocultas_desde = Some(0);
        assert!(gerar(&wb, 0, area(49, 1), &ConfigPagina::default()).is_ok());
    }

    #[test]
    fn grava_arquivo_para_conferir() {
        let wb = pasta(200);
        let mut cfg = ConfigPagina::default();
        cfg.linhas_de_grade = true;
        cfg.cabecalhos = true;
        let pdf = gerar(&wb, 0, area(199, 1), &cfg).unwrap();
        if let Ok(dir) = std::env::var("FEATHERSHEET_PDF_SAIDA") {
            std::fs::write(format!("{dir}/impressao.pdf"), &pdf).unwrap();
        }
    }
}
