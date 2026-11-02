//! Auditoria de fórmulas: quem a célula lê (precedentes) e quem lê a célula
//! (dependentes), para as setas de Fórmulas → Rastrear.
//!
//! Precedentes saem do texto da fórmula (vale para avulsa e empacotada).
//! Dependentes saem do grafo (avulsas, inclusive por intervalo) e da dedução
//! por padrão das colunas empacotadas — o mesmo caminho da cascata de "sujo".
use super::{mesmo_nome, Sheet};
use crate::celulas::refs::Rect;
use crate::formula::ast::Node;
use crate::formula::parser::parse_formula;

/// Uma referência lida pela fórmula: o retângulo e, se for de outra aba, o nome dela.
#[derive(Clone, Debug, PartialEq)]
pub struct Precedente {
    pub area: Rect,
    pub aba: Option<String>,
}

impl Sheet {
    /// As referências que a fórmula em (row, col) lê, na ordem do texto,
    /// sem repetição. Célula sem fórmula: lista vazia.
    pub fn precedentes(&self, row: u32, col: u32) -> Vec<Precedente> {
        let raw = self.get_raw(row, col);
        let Some(corpo) = raw.strip_prefix('=') else { return Vec::new() };
        let Ok(arvore) = parse_formula(corpo) else { return Vec::new() };
        let mut saida: Vec<Precedente> = Vec::new();
        let local = |s: &Option<String>| s.as_deref().filter(|s| !mesmo_nome(s, &self.name)).map(str::to_owned);
        arvore.walk(&mut |n| {
            let p = match n {
                Node::Ref { row, col, sheet } => Precedente { area: Rect::new(*row, *col, *row, *col), aba: local(sheet) },
                Node::Range { r0, c0, r1, c1, sheet } => Precedente { area: Rect::new(*r0, *c0, *r1, *c1), aba: local(sheet) },
                _ => return,
            };
            if !saida.contains(&p) {
                saida.push(p);
            }
        });
        saida
    }

    /// As fórmulas desta aba que leem (row, col) diretamente.
    pub fn dependentes(&self, row: u32, col: u32) -> Vec<(u32, u32)> {
        let k = Self::cell_key(row, col);
        let mut chaves = Vec::new();
        self.deps.step_dependents(k, |d| chaves.push(d));
        self.step_pattern_dependents(k, &self.formulas.columns_in_use(), &mut |d| chaves.push(d), None);
        chaves.sort_unstable();
        chaves.dedup();
        chaves.into_iter().filter(|&d| d != k).map(Self::key_to_coord).collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn precedentes_e_dependentes_avulsas_e_empacotadas() {
        let mut s = Sheet::new("Plan1");
        for r in 0..5 {
            s.set_cell_raw(r, 0, format!("{}", r + 1), None);
            s.set_cell_raw(r, 1, format!("=A{}*2", r + 1), None); // empacotadas
        }
        s.set_cell_raw(0, 3, "=SOMA(A1:A5)+B2+Dados!C3".into(), None); // avulsa
        assert_eq!(s.precedentes(0, 3), vec![
            Precedente { area: Rect::new(0, 0, 4, 0), aba: None },
            Precedente { area: Rect::new(1, 1, 1, 1), aba: None },
            Precedente { area: Rect::new(2, 2, 2, 2), aba: Some("Dados".into()) },
        ]);
        assert_eq!(s.precedentes(1, 1), vec![Precedente { area: Rect::new(1, 0, 1, 0), aba: None }]);
        assert!(s.precedentes(0, 0).is_empty());
        // A2 é lido por B2 (padrão) e por D1 (intervalo).
        assert_eq!(s.dependentes(1, 0), vec![(0, 3), (1, 1)]);
        // B2 é lido por D1.
        assert_eq!(s.dependentes(1, 1), vec![(0, 3)]);
        assert!(s.dependentes(0, 3).is_empty());
    }
}
