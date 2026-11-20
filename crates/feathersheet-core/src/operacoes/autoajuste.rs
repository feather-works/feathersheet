//! Autoajuste de largura de colunas e altura de linhas ao conteúdo. Mede só
//! as linhas/colunas usadas (amostragem limitada); quem grava é o chamador,
//! numa transação (interface e MCP usam a mesma conta).
use crate::celulas::{refs::Rect, values::Value};
use crate::planilha::workbook::Workbook;

/// Teto de linhas medidas por coluna (cabeça + amostra espalhada).
const TETO_AMOSTRA: u32 = 10_000;
/// Acima disto, a seleção é tratada como "eixo inteiro" e limitada ao usado.
const LIMITE_EIXO: u32 = 256;

fn texto_exibido(w: &Workbook, folha: usize, r: u32, c: u32) -> String {
    let estilo = w.aba(folha).and_then(|a| a.sheet.estilo_resolvido(r, c));
    match w.valor(folha, r, c) {
        // O formato personalizado (`"Total: "0`, `0000`) muda a largura, como na grade.
        Value::Number(n) => crate::celulas::style::format_value(&Value::Number(n), estilo.as_ref()),
        Value::Empty => String::new(),
        Value::Text(s) => s,
        Value::Bool(b) => if b { "VERDADEIRO".into() } else { "FALSO".into() },
        Value::Error(e) => e.code.as_str().into(),
    }
}

/// Linhas amostradas: todas se couberem no teto; senão o começo (cabeçalho)
/// e um passo constante pelo resto.
fn linhas_amostradas(ultima: u32) -> Vec<u32> {
    let total = ultima as u64 + 1;
    if total <= TETO_AMOSTRA as u64 { return (0..=ultima).collect(); }
    let cabeca = TETO_AMOSTRA / 2;
    let mut v: Vec<u32> = (0..cabeca).collect();
    let passo = (total / cabeca as u64).max(1) as usize;
    v.extend((cabeca..=ultima).step_by(passo));
    v.push(ultima);
    v
}

/// Célula dentro de uma mesclagem: o Excel não a mede no autoajuste (o
/// título mesclado sobre várias colunas não alarga a primeira).
fn mesclada(mescladas: &[Rect], r: u32, c: u32) -> bool {
    mescladas.iter().any(|m| m.contains(r, c))
}

/// Largura ideal em pixels lógicos de cada coluna de `c0..=c1`; `padrao` é a
/// largura de coluna vazia; células em `mescladas` não contam.
pub fn larguras_ideais(w: &Workbook, folha: usize, c0: u32, c1: u32, padrao: u32, mescladas: &[Rect]) -> Vec<(u32, u32)> {
    let Some(aba) = w.aba(folha) else { return Vec::new() };
    let usado = aba.sheet.used_rect();
    let vazia = aba.sheet.is_empty();
    let c1 = if c1 - c0 + 1 > LIMITE_EIXO { c1.min(usado.c1).max(c0) } else { c1 };
    let linhas = if vazia { Vec::new() } else { linhas_amostradas(usado.r1) };
    (c0..=c1).map(|c| {
        let mut maior = 0usize;
        for &r in &linhas {
            if aba.sheet.get_raw(r, c).is_empty() || mesclada(mescladas, r, c) { continue; }
            let t = texto_exibido(w, folha, r, c);
            maior = maior.max(t.lines().map(|l| l.chars().count()).max().unwrap_or(0));
        }
        let px = if maior == 0 { padrao }
            else { ((maior as f32 * 7.3 + 20.0).ceil() as u32).clamp(48, 520) };
        (c, px)
    }).collect()
}

/// Altura ideal de cada linha de `r0..=r1` (só cresce com quebras de linha);
/// `padrao` é a altura mínima.
pub fn alturas_ideais(w: &Workbook, folha: usize, r0: u32, r1: u32, padrao: u32, mescladas: &[Rect]) -> Vec<(u32, u32)> {
    let Some(aba) = w.aba(folha) else { return Vec::new() };
    let usado = aba.sheet.used_rect();
    let vazia = aba.sheet.is_empty();
    let r1 = if r1 - r0 + 1 > TETO_AMOSTRA { r1.min(usado.r1).max(r0) } else { r1 };
    (r0..=r1).map(|r| {
        let mut linhas = 1usize;
        if !vazia && r <= usado.r1 {
            for c in usado.c0..=usado.c1.min(usado.c0 + 255) {
                if aba.sheet.get_raw(r, c).is_empty() || mesclada(mescladas, r, c) { continue; }
                linhas = linhas.max(texto_exibido(w, folha, r, c).lines().count().max(1));
            }
        }
        (r, ((linhas as f32 * 18.0 + 10.0).max(padrao as f32).ceil() as u32).clamp(22, 400))
    }).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn titulo_mesclado_nao_alarga_a_coluna() {
        let mut w = Workbook::new();
        w.editar(0, |s| {
            s.set_cell(0, 0, "Um título comprido que ocupa três colunas");
            s.set_cell(1, 0, "curto");
        });
        let livre = larguras_ideais(&w, 0, 0, 0, 122, &[])[0].1;
        let mesclado = larguras_ideais(&w, 0, 0, 0, 122, &[Rect::new(0, 0, 0, 2)])[0].1;
        assert!(livre > 250 && mesclado < 100, "{livre} {mesclado}");
    }

    #[test]
    fn formato_personalizado_conta_na_largura() {
        use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
        let mut w = Workbook::new();
        w.editar(0, |s| {
            let mut total = EstiloParcial::new();
            total.insert("numFmt".into(), Campo::Texto("\"Total geral do mês: \"0".into()));
            let i = s.registrar_parcial(total).unwrap();
            s.set_cell_raw(0, 0, "42".into(), Some(i));
            s.set_cell(0, 1, "42");
        });
        let l = larguras_ideais(&w, 0, 0, 1, 122, &[]);
        assert!(l[0].1 > l[1].1 * 2, "{l:?}");
    }
}
