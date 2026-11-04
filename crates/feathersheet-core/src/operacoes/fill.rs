//! A alça de preenchimento nativa; legado TypeScript em `descarte/feathersheet/src/core/fill.ts`.

use crate::celulas::estilo_parcial::EstiloParcial;
use crate::formula::formula_text::{format_ref, map_references};
use crate::celulas::number::to_js_string;
use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::planilha::sheet::Sheet;
use crate::celulas::values::Value;

#[derive(Debug, Clone)]
pub struct SourceCell {
    pub raw: String,
    pub value: Value,
    /// O estilo que vale na célula de origem; vai junto para a nova.
    pub style: Option<EstiloParcial>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillCell {
    pub row: u32,
    pub col: u32,
    pub raw: String,
    pub style: Option<EstiloParcial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillAxis {
    Down,
    Up,
    Right,
    Left,
}

/// Desloca as referências de uma fórmula em (dr, dc), preservando o que estiver preso com "$"
pub fn shift_formula(raw: &str, dr: i32, dc: i32) -> String {
    if !raw.starts_with('=') || (dr == 0 && dc == 0) {
        return raw.to_string();
    }
    map_references(raw, |r| {
        use crate::celulas::refs::{MAX_COLS, MAX_ROWS};
        let col=i64::from(r.col) + if r.col_lock {0} else {i64::from(dc)};
        let row=i64::from(r.row) + if r.row_lock {0} else {i64::from(dr)};
        if !(0..i64::from(MAX_COLS)).contains(&col)
            || !(0..i64::from(MAX_ROWS)).contains(&row) {
            return "#REF!".into();
        }
        format_ref(&crate::formula::formula_text::ParsedRef {
            sheet:r.sheet,
            col_lock:r.col_lock,
            row_lock:r.row_lock,
            col:col as i32,
            row:row as i32,
        })
    })
}

#[cfg(test)]
mod testes_limites_do_shift {
    use super::shift_formula;
    #[test]
    fn nao_emite_referencias_fora_dos_limites_de_excel() {
        assert_eq!(shift_formula("=XFD1+\"XFD1\"",0,1),"=#REF!+\"XFD1\"");
        assert_eq!(shift_formula("=A100000000",1,0),"=#REF!");
        assert_eq!(shift_formula("=A1",-1,0),"=#REF!");
        assert_eq!(shift_formula("=$A$1+XFD1",0,1),"=$A$1+#REF!");
        assert_eq!(shift_formula("=A1+$B2+C$3",1,2),"=C2+$B3+E$3");
        assert_eq!(shift_formula("A1",1,0),"A1");
    }
}

struct TrailingNumber {
    prefix: String,
    /// `Number(m[2])` do TypeScript: número de muitos dígitos vira ponto
    /// flutuante lá, e aqui também — com `i64` ele caía em cópia.
    value: f64,
    digits: usize,
}

fn trailing_number(text: &str) -> Option<TrailingNumber> {
    let mut end = text.len();
    while end > 0 && text.as_bytes()[end - 1].is_ascii_digit() {
        end -= 1;
    }
    if end == text.len() {
        return None;
    }
    let prefix = text[..end].to_string();
    let num_str = &text[end..];
    let value = num_str.parse::<f64>().ok()?;
    Some(TrailingNumber {
        prefix,
        value,
        digits: num_str.len(),
    })
}

fn step_of(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let step = values[1] - values[0];
    for i in 2..values.len() {
        if (values[i] - values[i - 1] - step).abs() > 1e-9 {
            return None;
        }
    }
    Some(step)
}

pub fn axis_of(source: &Rect, target: &Rect) -> Option<FillAxis> {
    if target.r1 > source.r1 {
        Some(FillAxis::Down)
    } else if target.r0 < source.r0 {
        Some(FillAxis::Up)
    } else if target.c1 > source.c1 {
        Some(FillAxis::Right)
    } else if target.c0 < source.c0 {
        Some(FillAxis::Left)
    } else {
        None
    }
}

pub fn fill_target(source: &Rect, row: u32, col: u32) -> Rect {
    // Eventos da UI fora da grade não podem criar coordenadas inválidas.
    let row = row.min(MAX_ROWS - 1);
    let col = col.min(MAX_COLS - 1);
    let d_row = if row > source.r1 {
        row - source.r1
    } else {
        source.r0.saturating_sub(row)
    };
    let d_col = if col > source.c1 {
        col - source.c1
    } else {
        source.c0.saturating_sub(col)
    };
    if d_row == 0 && d_col == 0 {
        return source.clone();
    }
    let mut res = source.clone();
    if d_row >= d_col {
        if row > source.r1 {
            res.r1 = row;
        } else {
            res.r0 = row;
        }
    } else {
        if col > source.c1 {
            res.c1 = col;
        } else {
            res.c0 = col;
        }
    }
    res
}

pub fn plan_fill<F>(read: F, source: &Rect, target: &Rect) -> Vec<FillCell>
where
    F: Fn(u32, u32) -> SourceCell,
{
    plan_fill_ctrl(read, source, target, false)
}

/// Como [`plan_fill`]; `ctrl` = alça arrastada com Ctrl, que inverte como no Excel:
/// um número sozinho vira sequência de 1 em 1, e série (ou "Lote 1") vira cópia.
pub fn plan_fill_ctrl<F>(read: F, source: &Rect, target: &Rect, ctrl: bool) -> Vec<FillCell>
where
    F: Fn(u32, u32) -> SourceCell,
{
    let axis = match axis_of(source, target) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let vertical = axis == FillAxis::Down || axis == FillAxis::Up;
    let forward = axis == FillAxis::Down || axis == FillAxis::Right;

    let count = if vertical {
        if forward {
            target.r1.saturating_sub(source.r1)
        } else {
            source.r0.saturating_sub(target.r0)
        }
    } else {
        if forward {
            target.c1.saturating_sub(source.c1)
        } else {
            source.c0.saturating_sub(target.c0)
        }
    };
    if count == 0 {
        return Vec::new();
    }

    let line_start = if vertical { source.c0 } else { source.r0 };
    let line_end = if vertical { source.c1 } else { source.r1 };
    let pattern_length = if vertical {
        source.r1 - source.r0 + 1
    } else {
        source.c1 - source.c0 + 1
    };

    let mut out = Vec::new();

    for line in line_start..=line_end {
        struct PatternItem {
            cell: SourceCell,
            row: u32,
            col: u32,
        }
        let mut pattern = Vec::with_capacity(pattern_length as usize);
        for k in 0..pattern_length {
            let offset = if forward {
                k
            } else {
                pattern_length - 1 - k
            };
            let row = if vertical { source.r0 + offset } else { line };
            let col = if vertical { line } else { source.c0 + offset };
            pattern.push(PatternItem {
                cell: read(row, col),
                row,
                col,
            });
        }

        let mut numbers = Vec::new();
        for p in &pattern {
            if let Value::Number(n) = p.cell.value {
                numbers.push(n);
            }
        }
        let all_numbers = numbers.len() == pattern.len()
            && pattern.iter().all(|p| !p.cell.raw.starts_with('='));
        let step = match (all_numbers, ctrl, numbers.len()) {
            (true, true, 1) => Some(1.0),
            (true, false, _) => step_of(&numbers),
            _ => None,
        };
        let last = if all_numbers && !numbers.is_empty() {
            numbers[numbers.len() - 1]
        } else {
            0.0
        };

        let single = if pattern.len() == 1 {
            Some(&pattern[0])
        } else {
            None
        };
        let seed = if let Some(s) = single.filter(|_| !ctrl) {
            if !s.cell.raw.starts_with('=') && !matches!(s.cell.value, Value::Number(_)) {
                trailing_number(&s.cell.raw)
            } else {
                None
            }
        } else {
            None
        };

        for i in 0..count {
            let delta = i + 1;
            let row = if vertical {
                if forward {
                    source.r1 + delta
                } else {
                    source.r0 - delta
                }
            } else {
                line
            };
            let col = if vertical {
                line
            } else {
                if forward {
                    source.c1 + delta
                } else {
                    source.c0 - delta
                }
            };

            let origin = &pattern[(i as usize) % (pattern_length as usize)];
            let raw = if origin.cell.raw.starts_with('=') {
                shift_formula(
                    &origin.cell.raw,
                    row as i32 - origin.row as i32,
                    col as i32 - origin.col as i32,
                )
            } else if let Some(st) = step {
                format_series(last + st * (delta as f64))
            } else if let Some(ref sd) = seed {
                // `String(v).padStart(digits, '0')`.
                let n = to_js_string(sd.value + delta as f64);
                let falta = sd.digits.saturating_sub(n.chars().count());
                format!("{}{}{}", sd.prefix, "0".repeat(falta), n)
            } else {
                origin.cell.raw.clone()
            };

            out.push(FillCell { row, col, raw, style: origin.cell.style.clone() });
        }
    }

    out
}

/// `String(Number(n.toPrecision(12)))`: evita que 0.1 + 0.2 apareça como
/// 0.30000000000000004 na sequência. Doze algarismos significativos, e o
/// número escrito como o JavaScript escreveria.
pub(crate) fn format_series(n: f64) -> String {
    if !n.is_finite() {
        return to_js_string(n);
    }
    let arredondado: f64 = format!("{n:.11e}").parse().unwrap_or(n);
    to_js_string(arredondado)
}

/// Duplo clique na alça: até que linha a seleção desce — o fim do bloco de
/// dados de uma coluna vizinha. Tradução do `fillHandleDoubleClick` de
/// `ui/grade/arrastar.ts`, na mesma ordem de procura: a coluna logo à
/// esquerda, a logo à direita, e depois as vizinhas até cinco de distância
/// (esquerda antes da direita, a cada distância). `None` quando nenhuma passa
/// da última linha da seleção.
///
/// `tem_dado` é o `hasContent || getRaw !== ''` de lá; `max_row` o `maxRow`
/// da aba (a procura para em `max_row + 100`, como lá); `colunas` o
/// `colAxis.length` da grade, o limite à direita.
///
/// Pelo TypeScript, com a planilha no Rust, cada pergunta era uma ida pela
/// ponte (um bloco a cada 64 linhas): descer uma coluna de dois milhões de
/// linhas eram 31 mil blocos por duplo clique.
pub fn fim_do_duplo_clique(tem_dado: impl Fn(u32, u32) -> bool, sel: &Rect, max_row: u32, colunas: u32) -> Option<u32> {
    if sel.r1 >= MAX_ROWS - 1 {
        return None; // Não existe linha abaixo da última linha da grade.
    }
    let colunas = colunas.min(MAX_COLS);
    let inicio = sel.r1;
    // `acharFim`: -1 do TypeScript vira `None`. Com dado só na própria linha
    // (e não na de baixo) ele devolve a linha da seleção — que não serve, mas
    // conta como "achou" para a ordem da procura, como lá.
    let achar_fim = |c: u32| -> Option<u32> {
        if !tem_dado(inicio + 1, c) && !tem_dado(inicio, c) {
            return None;
        }
        let teto = max_row.saturating_add(100).min(MAX_ROWS);
        let mut r = inicio + 1;
        while r < teto && tem_dado(r, c) {
            r += 1;
        }
        Some(r - 1)
    };
    let passou = |f: Option<u32>| f.filter(|&r| r > inicio);
    // 1. A coluna à esquerda.
    let mut alvo = if sel.c0 > 0 { achar_fim(sel.c0 - 1) } else { None };
    // 2. Não passou da seleção: a coluna à direita.
    if passou(alvo).is_none() && sel.c1.saturating_add(1) < colunas {
        alvo = achar_fim(sel.c1 + 1);
    }
    // 3. As mais distantes, até cinco.
    if passou(alvo).is_none() {
        for delta in 2..=5u32 {
            if sel.c0 >= delta {
                if let Some(f) = passou(achar_fim(sel.c0 - delta)) {
                    return Some(f);
                }
            }
            if sel.c1.saturating_add(delta) < colunas {
                if let Some(f) = passou(achar_fim(sel.c1 + delta)) {
                    return Some(f);
                }
            }
        }
    }
    passou(alvo)
}

impl Sheet {
    /// O duplo clique na alça inteiro: acha o fim ([`fim_do_duplo_clique`])
    /// e preenche até lá, num Ctrl+Z só. Devolve o destino e quantas células,
    /// ou `None` quando não há até onde descer. Com `so_planejar`, não
    /// escreve: é para a interface conferir a proteção do destino antes.
    pub fn preencher_ate_o_fim(&mut self, sel: &Rect, colunas: u32, so_planejar: bool) -> Option<(Rect, usize)> {
        let sel = sel.normalized();
        let max_row = self.max_row_ts();
        let fim = fim_do_duplo_clique(
            |r, c| self.has_content(r, c) || !self.get_raw(r, c).is_empty(),
            &sel,
            max_row,
            colunas,
        )?;
        let destino = Rect::new(sel.r0, sel.c0, fim, sel.c1);
        if so_planejar {
            return Some((destino, 0));
        }
        let feitas = self.preencher(&sel, &destino);
        Some((destino, feitas))
    }

    /// A alça de preenchimento inteira: planeja e escreve, numa transação só
    /// (um Ctrl+Z desfaz), com o estilo de cada origem. Pela ponte, o
    /// `applyFill` da grade escrevia célula a célula — arrastar mil linhas
    /// eram duas mil idas. Devolve quantas células preencheu.
    pub fn preencher(&mut self, origem: &Rect, destino: &Rect) -> usize {
        self.preencher_com(origem, destino, false)
    }

    /// [`Self::preencher`] com a alça arrastada com Ctrl ([`plan_fill_ctrl`]).
    pub fn preencher_com(&mut self, origem: &Rect, destino: &Rect, ctrl: bool) -> usize {
        let plano = plan_fill_ctrl(
            |r, c| {
                let raw = self.get_raw(r, c);
                // Fórmula não entra na conta da série: não precisa calcular.
                let value = if raw.starts_with('=') { Value::Empty } else { self.get_value(r, c) };
                SourceCell { raw, value, style: self.estilo_mesclado(r, c) }
            },
            origem,
            destino,
            ctrl,
        );
        if plano.is_empty() {
            return 0;
        }
        self.abrir_transacao();
        for cel in &plano {
            self.set_cell(cel.row, cel.col, &cel.raw);
            if let Some(estilo) = &cel.style {
                self.set_style(&Rect::new(cel.row, cel.col, cel.row, cel.col), estilo);
            }
        }
        self.fechar_transacao();
        plano.len()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ler<'a>(celulas: &'a [((u32, u32), &'a str)]) -> impl Fn(u32, u32) -> SourceCell + 'a {
        move |r, c| {
            let raw = celulas.iter().find(|(k, _)| *k == (r, c)).map(|(_, v)| v.to_string()).unwrap_or_default();
            let value = raw.parse::<f64>().map(Value::Number).unwrap_or(Value::Number(0.0));
            SourceCell { raw, value, style: None }
        }
    }

    fn crus(p: &[FillCell]) -> Vec<&str> {
        p.iter().map(|c| c.raw.as_str()).collect()
    }

    // Os casos do `test/core/workbook.test.ts`.
    #[test]
    fn sequencia_numerica_continua_a_progressao() {
        let cel = [((0, 0), "1"), ((1, 0), "3")];
        let p = plan_fill(ler(&cel), &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 4, 0));
        assert_eq!(crus(&p), ["5", "7", "9"]);
        // Com Ctrl a série é copiada (Excel).
        let p = plan_fill_ctrl(ler(&cel), &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 4, 0), true);
        assert_eq!(crus(&p), ["1", "3", "1"]);
    }

    #[test]
    fn uma_celula_so_e_copiada_e_texto_com_numero_vira_sequencia() {
        let sete = |_: u32, _: u32| SourceCell { raw: "7".into(), value: Value::Number(7.0), style: None };
        assert_eq!(crus(&plan_fill(&sete, &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 2, 0))), ["7", "7"]);
        // Ctrl inverte, como no Excel: o número sozinho conta 8, 9; "Semana 1" é copiada.
        assert_eq!(crus(&plan_fill_ctrl(&sete, &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 2, 0), true)), ["8", "9"]);
        let semana = |_: u32, _: u32| SourceCell { raw: "Semana 1".into(), value: Value::Text("Semana 1".into()), style: None };
        assert_eq!(crus(&plan_fill(semana, &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 2, 0))), ["Semana 2", "Semana 3"]);
    }

    #[test]
    fn arrastar_para_cima_extrapola_para_tras() {
        let cel = [((5, 0), "10"), ((6, 0), "20")];
        let p = plan_fill(ler(&cel), &Rect::new(5, 0, 6, 0), &Rect::new(3, 0, 6, 0));
        let pares: Vec<(u32, &str)> = p.iter().map(|c| (c.row, c.raw.as_str())).collect();
        assert_eq!(pares, [(4, "0"), (3, "-10")]);
    }

    /// Divergências do porte antigo: a série era escrita com 10 casas fixas
    /// (o TypeScript usa 12 algarismos significativos, escritos como o
    /// JavaScript), e o número de muitos dígitos no fim do texto caía em cópia.
    #[test]
    fn serie_e_texto_numerado_escritos_como_no_typescript() {
        let cel = [((0, 0), "0,1"), ((1, 0), "0,2")];
        let lido = |r: u32, c: u32| {
            let v = if (r, c) == (0, 0) { 0.1 } else { 0.2 };
            SourceCell { raw: cel.iter().find(|(k, _)| *k == (r, c)).unwrap().1.into(), value: Value::Number(v), style: None }
        };
        assert_eq!(crus(&plan_fill(lido, &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 2, 0))), ["0.3"]);
        let micro = |r: u32, _: u32| SourceCell { raw: "x".into(), value: Value::Number((r + 1) as f64 * 1e-7), style: None };
        assert_eq!(crus(&plan_fill(micro, &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 2, 0))), ["3e-7"]);
        let id = |_: u32, _: u32| SourceCell { raw: "ID 99999999999999999999".into(), value: Value::Text("x".into()), style: None };
        assert_eq!(crus(&plan_fill(id, &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 1, 0))), ["ID 100000000000000000000"]);
        let zero = |_: u32, _: u32| SourceCell { raw: "Lote 007".into(), value: Value::Text("x".into()), style: None };
        assert_eq!(crus(&plan_fill(zero, &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 1, 0))), ["Lote 008"]);
    }

    #[test]
    fn preencher_na_planilha_leva_formula_estilo_e_desfaz_num_ctrl_z() {
        let mut s = Sheet::new("A");
        s.set_cell(0, 0, "1");
        s.set_cell(1, 0, "2");
        s.set_cell(0, 1, "=A1*10");
        let negrito: EstiloParcial = [("bold".to_string(), crate::celulas::estilo_parcial::Campo::Logico(true))].into_iter().collect();
        s.set_style(&Rect::new(0, 1, 0, 1), &negrito);
        let feitas = s.preencher(&Rect::new(0, 0, 1, 1), &Rect::new(0, 0, 3, 1));
        assert_eq!(feitas, 4);
        assert_eq!(s.get_raw(3, 0), "4");
        assert_eq!(s.get_raw(2, 1), "=A3*10");
        assert_eq!(s.get_value(2, 1), Value::Number(30.0));
        assert_eq!(s.get_raw(3, 1), "", "a origem tinha B2 vazia: o padrão repete o vazio");
        assert_eq!(s.estilo_mesclado(2, 1), Some(negrito), "o estilo da origem vai junto");
        assert!(s.undo());
        assert_eq!(s.get_raw(3, 0), "");
        assert_eq!(s.get_raw(2, 1), "");
    }

    /// O caso do `ui.test.ts` ("duplo clique na alça…"): A1:A10 com dado,
    /// fórmulas em B1:C1 — desce até a linha 10.
    #[test]
    fn duplo_clique_desce_ate_o_fim_da_coluna_vizinha() {
        let mut s = Sheet::new("A");
        for r in 0..10 {
            s.set_cell(r, 0, &(r + 1).to_string());
        }
        s.set_cell(0, 1, "=A1*2");
        s.set_cell(0, 2, "=A1*3");
        let (destino, feitas) = s.preencher_ate_o_fim(&Rect::new(0, 1, 0, 2), 26, false).unwrap();
        assert_eq!(destino, Rect::new(0, 1, 9, 2));
        assert_eq!(feitas, 18);
        assert_eq!(s.get_raw(9, 2), "=A10*3");
        assert_eq!(s.get_raw(10, 1), "");
        assert!(s.undo(), "um Ctrl+Z só");
        assert_eq!(s.get_raw(9, 1), "");
    }

    #[test]
    fn duplo_clique_procura_a_direita_e_depois_mais_longe() {
        let ler = |celulas: &'static [(u32, u32)]| move |r: u32, c: u32| celulas.contains(&(r, c));
        // Nada à esquerda (coluna 0 é a seleção): a da direita manda.
        let dir: &[(u32, u32)] = &[(0, 1), (1, 1), (2, 1)];
        assert_eq!(fim_do_duplo_clique(ler(dir), &Rect::new(0, 0, 0, 0), 2, 26), Some(2));
        // Vizinha à esquerda com dado só na linha da seleção: devolve a própria
        // linha, e a procura segue — a de três colunas adiante tem até a 4.
        let longe: &[(u32, u32)] = &[(0, 2), (0, 6), (1, 6), (2, 6), (3, 6), (4, 6)];
        assert_eq!(fim_do_duplo_clique(ler(longe), &Rect::new(0, 3, 0, 3), 4, 26), Some(4));
        // A esquerda ganha da direita na mesma distância.
        let empate: &[(u32, u32)] = &[(1, 1), (2, 1), (1, 5), (2, 5), (3, 5)];
        assert_eq!(fim_do_duplo_clique(ler(empate), &Rect::new(0, 3, 0, 3), 3, 26), Some(2));
        // Seis colunas de distância já não conta; e sem nada, nada.
        let fora: &[(u32, u32)] = &[(1, 9), (2, 9)];
        assert_eq!(fim_do_duplo_clique(ler(fora), &Rect::new(0, 3, 0, 3), 2, 26), None);
        // O limite à direita é o da grade.
        assert_eq!(fim_do_duplo_clique(ler(dir), &Rect::new(0, 0, 0, 0), 2, 1), None);
    }

    /// A procura para em `max_row + 100`, como no TypeScript — mesmo que o
    /// "tem dado" diga sim para sempre.
    #[test]
    fn duplo_clique_tem_teto() {
        assert_eq!(fim_do_duplo_clique(|_, c| c == 0, &Rect::new(0, 1, 0, 1), 50, 26), Some(149));
    }

    #[test]
    fn alca_respeita_limites_da_grade_de_dez_milhoes_de_linhas() {
        let ultima = MAX_ROWS - 1;
        let fim = fim_do_duplo_clique(
            |r, c| r >= ultima - 2 && r < MAX_ROWS && c == 0,
            &Rect::new(ultima - 2, 1, ultima - 2, 1),
            ultima,
            MAX_COLS,
        );
        assert_eq!(fim, Some(ultima));
        assert_eq!(
            fim_do_duplo_clique(
                |_r, _c| true,
                &Rect::new(ultima, 1, ultima, 1),
                ultima,
                MAX_COLS,
            ),
            None,
        );

        let origem = Rect::new(8, 2, 8, 2);
        assert_eq!(
            fill_target(&origem, MAX_ROWS + 200, 2),
            Rect::new(8, 2, ultima, 2),
        );
        assert_eq!(
            fill_target(&origem, 8, MAX_COLS + 200),
            Rect::new(8, 2, 8, MAX_COLS - 1),
        );
    }
}
