//! Funil e cálculo de contraste/limites dos cabeçalhos.
use super::*;

// Funil vetorial renderizado no próprio Canvas da grade, sem glifos da fonte.
// Preenchido quando existe critério ativo; contorno quando o menu está disponível.
pub(super) fn desenhar_funil(f: &mut Frame, centro: Point, filtrado: bool, cor: Color) {
    let x = centro.x;
    let y = centro.y;
    let contorno = canvas::Path::new(|p| {
        p.move_to(Point::new(x - 5.5, y - 4.5));
        p.line_to(Point::new(x + 5.5, y - 4.5));
        p.line_to(Point::new(x + 1.8, y - 0.2));
        p.line_to(Point::new(x + 1.8, y + 4.2));
        p.line_to(Point::new(x - 1.8, y + 5.8));
        p.line_to(Point::new(x - 1.8, y - 0.2));
        p.close();
    });
    if filtrado {
        f.fill(&contorno, cor);
    } else {
        f.stroke(&contorno, canvas::Stroke::default()
            .with_color(cor).with_width(1.4));
    }
}

/// Escolhe o primeiro plano pelo fundo efetivamente exibido, não apenas pelo
/// fato de a célula ser um cabeçalho. AutoFiltro não pinta a célula como tabela.
pub(super) fn cor_contraste(fundo: Option<Color>, escuro: bool) -> Color {
    let base = if escuro {
        Color::rgb8(34, 37, 42)
    } else {
        Color::WHITE
    };
    let cor = fundo.unwrap_or(base);
    let alpha = cor.a.clamp(0.0, 1.0);
    let misturar = |v: f32, base: f32| v * alpha + base * (1.0 - alpha);
    let linear = |v: f32| {
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let luminancia = 0.2126 * linear(misturar(cor.r, base.r))
        + 0.7152 * linear(misturar(cor.g, base.g))
        + 0.0722 * linear(misturar(cor.b, base.b));
    // Mantém branco nos cabeçalhos coloridos escuros (incluindo o azul padrão).
    if luminancia < 0.24 { Color::WHITE } else { Color::rgb8(30, 35, 40) }
}

/// Coordenada da borda final de um cabeçalho visível; colunas/linhas
/// ocultas ou parcialmente cortadas na janela não criam divisórias falsas.
/// As coordenadas são lógicas; espessura compensa o zoom do Canvas.
pub(super) fn divisoria_visivel(inicio: f32, tamanho: f32, limite: f32, espessura: f32) -> Option<f32> {
    if tamanho <= 0.0 || inicio < 0.0 || inicio + tamanho > limite || espessura <= 0.0 {
        return None;
    }
    Some((inicio + tamanho - espessura).max(inicio))
}


/// Pintura da linha e da coluna da célula ativa: apresentação somente.
/// Nunca seleciona a linha/coluna inteira e não edita o Workbook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RealceCruz {
    Nenhum,
    Eixo,
    Cruzamento,
}

pub(super) fn realce_cruz(celula: (u32, u32), ativa: (u32, u32), habilitado: bool) -> RealceCruz {
    if !habilitado {
        RealceCruz::Nenhum
    } else if celula == ativa {
        RealceCruz::Cruzamento
    } else if celula.0 == ativa.0 || celula.1 == ativa.1 {
        RealceCruz::Eixo
    } else {
        RealceCruz::Nenhum
    }
}

pub(super) fn cor_hex(txt: &str) -> Option<Color> {
    if !feather_ui::seletor_cor::hex_valido(txt) { return None; }
    let [r, g, b, a] = feather_ui::seletor_cor::rgba_de(txt);
    Some(Color::rgb8(r, g, b).alpha(a as f32 / 255.))
}
