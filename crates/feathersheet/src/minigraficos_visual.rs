//! Minigráficos desenhados dentro da célula (Inserir ▸ Minigráficos). A lista
//! da aba é lida uma vez por quadro e os valores ao vivo, só das células com
//! minigráfico que estão na tela: não há cache para invalidar quando um dado muda.
//! As contas (escala, barras, destaques) ficam no motor
//! (`feathersheet_core::operacoes::minigraficos`); aqui só vira desenho.
use std::collections::HashMap;

use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::minigraficos::{self as mg, Cor, Grupo, Marca};
use feathersheet_core::planilha::workbook::Workbook;
use pelicano::{Color, Point, Size};

use crate::compat::canvas::{self, Frame};

/// Teto de pontos lidos por minigráfico: a célula não tem pixels para mais.
const MAX_PONTOS: usize = 4096;

pub(crate) struct Visiveis {
    grupos: Vec<Grupo>,
    onde: HashMap<(u32, u32), (usize, usize)>,
    /// (mín, máx) do grupo inteiro, só nos grupos com "mesmo eixo".
    escalas: Vec<Option<(f64, f64)>>,
}

/// Os números da faixa, na ordem de leitura; vazio, texto e erro viram lacuna.
fn valores(wb: &Workbook, aba: usize, dados: &str) -> Vec<Option<f64>> {
    let Some((nome, r)) = mg::partir_dados(dados) else { return Vec::new() };
    let Some(indice) = (match nome { Some(n) => wb.indice_por_nome(&n), None => Some(aba) }) else { return Vec::new() };
    let mut v = Vec::new();
    'fora: for linha in r.r0..=r.r1 {
        for coluna in r.c0..=r.c1 {
            if v.len() >= MAX_PONTOS { break 'fora; }
            v.push(match wb.valor(indice, linha, coluna) {
                Value::Number(n) if n.is_finite() => Some(n),
                _ => None,
            });
        }
    }
    v
}

impl Visiveis {
    /// `None` quando a aba não tem minigráfico (o caso comum: custo zero no quadro).
    pub(crate) fn da_aba(wb: &Workbook, aba: usize) -> Option<Self> {
        let grupos = feathersheet_io::minigraficos_aba::ler(wb.aba(aba)?.sheet.mg.as_deref());
        if grupos.is_empty() { return None; }
        let mut onde = HashMap::new();
        for (g, grupo) in grupos.iter().enumerate() {
            for (i, m) in grupo.itens.iter().enumerate() {
                onde.insert((m.linha, m.coluna), (g, i));
            }
        }
        let escalas = grupos.iter().map(|g| {
            if !g.eixo_igual { return None; }
            let todos: Vec<Option<f64>> = g.itens.iter().flat_map(|m| valores(wb, aba, &m.dados)).collect();
            mg::limites(&todos)
        }).collect();
        Some(Visiveis { grupos, onde, escalas })
    }

    /// Desenha o minigráfico da célula, se houver, no retângulo dela.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn desenhar(&self, f: &mut Frame, wb: &Workbook, aba: usize, linha: u32, coluna: u32, canto: Point, tamanho: Size, escuro: bool) {
        let Some(&(g, i)) = self.onde.get(&(linha, coluna)) else { return };
        let grupo = &self.grupos[g];
        let marcas = mg::desenhar(grupo, &valores(wb, aba, &grupo.itens[i].dados), self.escalas[g]);
        // Folga de 3 px nas laterais e 2 em cima e embaixo, como no Excel.
        let (x0, y0) = (canto.x + 3.0, canto.y + 2.0);
        let (w, h) = ((tamanho.width - 6.0).max(1.0), (tamanho.height - 4.0).max(1.0));
        let ponto = |fx: f64, fy: f64| Point::new(x0 + fx as f32 * w, y0 + fy as f32 * h);
        let cor = |c: Cor| {
            let hex = match c {
                Cor::Serie => &grupo.cor, Cor::Negativo => &grupo.cor_negativo, Cor::Marcadores => &grupo.cor_marcadores,
                Cor::Alto => &grupo.cor_alto, Cor::Baixo => &grupo.cor_baixo,
                Cor::Primeiro => &grupo.cor_primeiro, Cor::Ultimo => &grupo.cor_ultimo,
            };
            if feather_ui::seletor_cor::hex_valido(hex) {
                let [r, g, b, _] = feather_ui::seletor_cor::rgba_de(hex);
                Color::rgb8(r, g, b)
            } else {
                Color::rgb8(0x37, 0x60, 0x92)
            }
        };
        for m in marcas {
            match m {
                Marca::Linha(pontos) => {
                    let caminho = canvas::Path::new(|p| {
                        let mut solto = true;
                        for pt in &pontos {
                            match pt {
                                Some((fx, fy)) if solto => { p.move_to(ponto(*fx, *fy)); solto = false; }
                                Some((fx, fy)) => p.line_to(ponto(*fx, *fy)),
                                None => solto = true,
                            }
                        }
                    });
                    f.stroke(&caminho, canvas::Stroke::default().with_color(cor(Cor::Serie)).with_width(1.25)
                        .with_line_cap(canvas::LineCap::Round).with_line_join(canvas::LineJoin::Round));
                }
                Marca::Barra { x, largura, topo, base, cor: c } => {
                    let a = ponto(x, topo);
                    f.fill_rectangle(a, Size::new((largura as f32 * w).max(1.0), ((base - topo) as f32 * h).max(1.0)), cor(c));
                }
                Marca::Ponto { x, y, cor: c } => f.fill(&canvas::Path::circle(ponto(x, y), 2.0), cor(c)),
                Marca::Eixo(y) => {
                    let eixo = if escuro { Color::rgb8(120, 128, 140) } else { Color::rgb8(150, 156, 165) };
                    f.fill_rectangle(Point::new(x0, ponto(0.0, y).y - 0.5), Size::new(w, 1.0), eixo);
                }
            }
        }
    }
}
