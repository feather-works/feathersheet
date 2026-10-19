//! `SOMAR.POR.COR` e `CONT.SE.COR` (`SUMBYCOLOR`/`COUNTBYCOLOR`): somam e
//! contam as células de um intervalo pela cor do preenchimento ou da fonte,
//! tomando a cor de uma célula de amostra. O Excel não tem (no VBA só com
//! laço lendo `Interior.Color`).
//!
//! Lê a cor da formatação da célula (aba, coluna, linha e célula
//! empilhadas), não a da formatação condicional. Só as células com
//! conteúdo entram, como no `SOMA`. As duas são voláteis
//! (`sheet_deps.rs`): trocar a cor não muda valor nenhum, e é o
//! recálculo das voláteis que as atualiza.

use crate::funcoes::fn_helpers::node_rect;
use crate::celulas::refs::Rect;
use crate::celulas::values::{Computed, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, err_value, has};


/// `#RRGGBB` em minúsculas, para comparar `#FF0000` com `#ff0000`.
fn normalizar(cor: Option<String>) -> Option<String> {
    cor.map(|c| c.trim().to_ascii_lowercase()).filter(|c| !c.is_empty())
}

/// O intervalo, a cor procurada e se é a da fonte. A amostra é uma célula
/// (a cor dela) ou um texto `#RRGGBB`; o tipo é 0 (fundo, padrão) ou 1 (fonte).
fn preparar(args: &[Node], ev: &Evaluator<'_>) -> Result<(Rect, Option<String>, bool), Computed> {
    if let Some(e) = arity(args, 2, 3) {
        return Err(Computed::Scalar(Value::Error(e)));
    }
    let Some(rect) = node_rect(&args[0], ev) else { return Err(err_value()) };
    let fonte = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) if n == 0.0 => false,
            Ok(n) if n == 1.0 => true,
            Ok(_) => return Err(err_value()),
            Err(e) => return Err(Computed::Scalar(Value::Error(e))),
        }
    } else {
        false
    };
    let cor = match node_rect(&args[1], ev) {
        Some(a) => ev.ctx.cor_celula(a.r0, a.c0, a.sheet.as_deref(), fonte),
        None => match ev.text(&args[1]) {
            Ok(t) => Some(t),
            Err(e) => return Err(Computed::Scalar(Value::Error(e))),
        },
    };
    Ok((rect, normalizar(cor), fonte))
}

fn percorrer(args: &[Node], ev: &Evaluator<'_>, mut visitar: impl FnMut(&Value)) -> Option<Computed> {
    let (rect, cor, fonte) = match preparar(args, ev) {
        Ok(p) => p,
        Err(e) => return Some(e),
    };
    let aba = rect.sheet.clone();
    ev.ctx.for_each_in_range(&rect, &mut |v, r, c| {
        if normalizar(ev.ctx.cor_celula(r, c, aba.as_deref(), fonte)) == cor {
            visitar(v);
        }
    });
    None
}

/// `=SOMAR.POR.COR(intervalo; célula_cor; [tipo])`
pub fn fn_sumbycolor(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let mut soma = 0.0;
    let mut erro = None;
    if let Some(e) = percorrer(args, ev, |v| match v {
        Value::Number(n) => soma += n,
        Value::Error(e) if erro.is_none() => erro = Some(e.clone()),
        _ => {}
    }) {
        return e;
    }
    match erro {
        Some(e) => Computed::Scalar(Value::Error(e)),
        None => Computed::Scalar(Value::Number(soma)),
    }
}

/// `=CONT.SE.COR(intervalo; célula_cor; [tipo])`
pub fn fn_countbycolor(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let mut n = 0u64;
    if let Some(e) = percorrer(args, ev, |v| {
        if !matches!(v, Value::Empty) {
            n += 1;
        }
    }) {
        return e;
    }
    Computed::Scalar(Value::Number(n as f64))
}

#[cfg(test)]
mod testes {
    use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
    use crate::celulas::refs::Rect;
    use crate::celulas::values::Value;
    use crate::planilha::sheet::Sheet;

    fn pintar(s: &mut Sheet, r: u32, campo: &str, cor: &str) {
        let mut e = EstiloParcial::new();
        e.insert(campo.into(), Campo::Texto(cor.into()));
        s.set_style(&Rect::new(r, 0, r, 0), &e);
    }

    #[test]
    fn soma_e_conta_pela_cor_e_recalcula_ao_pintar() {
        let mut s = Sheet::new("Aba1");
        for (r, v) in ["10", "20", "30", "texto"].iter().enumerate() {
            s.set_cell(r as u32, 0, v);
        }
        pintar(&mut s, 0, "bg", "#FF0000");
        pintar(&mut s, 2, "bg", "#ff0000");
        pintar(&mut s, 3, "bg", "#ff0000");
        pintar(&mut s, 1, "color", "#0000ff");
        s.set_cell(0, 2, "=SOMAR.POR.COR(A1:A4;A1)");
        s.set_cell(1, 2, "=CONT.SE.COR(A1:A4;\"#FF0000\")");
        s.set_cell(2, 2, "=SUMBYCOLOR(A1:A4;A2;1)");
        s.set_cell(3, 2, "=CONT.SE.COR(A1:A4;A1;2)");
        assert_eq!(s.get_value(0, 2), Value::Number(40.0));
        assert_eq!(s.get_value(1, 2), Value::Number(3.0), "o texto pintado também conta");
        assert_eq!(s.get_value(2, 2), Value::Number(20.0), "cor da fonte");
        assert!(matches!(s.get_value(3, 2), Value::Error(_)), "tipo só 0 ou 1");
        // Pintar não muda valor: a fórmula recalcula por ser volátil.
        pintar(&mut s, 1, "bg", "#FF0000");
        assert_eq!(s.get_value(0, 2), Value::Number(60.0));
    }
}
