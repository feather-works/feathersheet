//! Inserir e excluir **células** com deslocamento (Página Inicial → Inserir ▾ /
//! Excluir ▾ → Inserir células… / Excluir células…, Ctrl++ e Ctrl+-).
//!
//! Diferente de inserir linha ou coluna inteira, só as colunas (ou linhas)
//! da seleção andam. As duas operações são o mesmo [`Workbook::mover_intervalo`]
//! do arrastar pela borda, sobre o bloco que vai da seleção até o fim da
//! planilha:
//!
//! - **inserir para baixo**: o bloco que começa na seleção desce tantas linhas
//!   quantas a seleção tem; quem lia uma célula deslocada passa a ler a nova
//!   posição;
//! - **excluir para cima**: o bloco abaixo da seleção sobe por cima dela; a
//!   referência a uma célula excluída vira `#REF!` (é a regra "destino
//!   sobrescrito" do mover).
//!
//! Um Ctrl+Z desfaz na aba mexida. Como no mover, um intervalo só anda quando
//! está inteiro no bloco: `SOMA(A1:A10)` com células inseridas em `A5` fica
//! como está (o Excel o estica). Mesclas e tabelas no caminho são recusadas
//! por quem chama, que é quem lê esses metadados.
use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::planilha::workbook::{Mudanca, Workbook};

/// Para onde as células andam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deslocamento {
    /// Inserir: para baixo. Excluir: para cima.
    Vertical,
    /// Inserir: para a direita. Excluir: para a esquerda.
    Horizontal,
}

impl Deslocamento {
    /// Quantas linhas (colunas) a seleção tem no sentido do deslocamento.
    fn passo(self, area: &Rect) -> u32 {
        match self {
            Self::Vertical => area.r1 - area.r0 + 1,
            Self::Horizontal => area.c1 - area.c0 + 1,
        }
    }
}

/// O bloco que anda: da seleção (ou logo depois dela) até a borda da
/// planilha, nas colunas (linhas) da seleção.
pub fn bloco_deslocado(area: &Rect, sentido: Deslocamento, inserir: bool) -> Option<Rect> {
    let a = area.normalized();
    let n = sentido.passo(&a);
    Some(match (sentido, inserir) {
        (Deslocamento::Vertical, true) => Rect::new(a.r0, a.c0, MAX_ROWS.checked_sub(1 + n)?, a.c1),
        (Deslocamento::Vertical, false) => Rect::new(a.r1.checked_add(1).filter(|&r| r < MAX_ROWS)?, a.c0, MAX_ROWS - 1, a.c1),
        (Deslocamento::Horizontal, true) => Rect::new(a.r0, a.c0, a.r1, MAX_COLS.checked_sub(1 + n)?),
        (Deslocamento::Horizontal, false) => Rect::new(a.r0, a.c1.checked_add(1).filter(|&c| c < MAX_COLS)?, a.r1, MAX_COLS - 1),
    })
}

impl Workbook {
    /// Insere células vazias no lugar de `area`, empurrando o que está nela
    /// e depois dela para baixo ou para a direita.
    pub fn inserir_celulas(&mut self, indice: usize, area: Rect, sentido: Deslocamento) -> Result<Mudanca, String> {
        let a = area.normalized();
        let n = sentido.passo(&a);
        let bloco = bloco_deslocado(&a, sentido, true).ok_or_else(|| crate::tr!("A seleção é grande demais para deslocar."))?;
        let sheet = &self.aba(indice).ok_or("Aba inexistente.")?.sheet;
        // O que está na borda da planilha cairia para fora.
        let borda = match sentido {
            Deslocamento::Vertical => Rect::new(MAX_ROWS - n, a.c0, MAX_ROWS - 1, a.c1),
            Deslocamento::Horizontal => Rect::new(a.r0, MAX_COLS - n, a.r1, MAX_COLS - 1),
        };
        if !sheet.chaves_no_retangulo(&borda).is_empty() {
            return Err(crate::tr!("Não cabe: há dados no fim da planilha que seriam empurrados para fora.").into());
        }
        if sheet.chaves_no_retangulo(&bloco).is_empty() {
            return Ok(Mudanca { versao: self.aba(indice).map_or(0, |x| x.versao), sujos: Vec::new() });
        }
        let (dr, dc) = match sentido { Deslocamento::Vertical => (n as i32, 0), Deslocamento::Horizontal => (0, n as i32) };
        self.mover_intervalo(indice, bloco, dr, dc).ok_or_else(|| crate::tr!("Não consegui deslocar as células.").into())
    }

    /// Exclui as células de `area`, puxando o que está depois delas para
    /// cima ou para a esquerda. Quem lia uma célula excluída fica com `#REF!`.
    pub fn excluir_celulas(&mut self, indice: usize, area: Rect, sentido: Deslocamento) -> Result<Mudanca, String> {
        let a = area.normalized();
        let n = sentido.passo(&a);
        let sheet = &self.aba(indice).ok_or("Aba inexistente.")?.sheet;
        let (dr, dc) = match sentido { Deslocamento::Vertical => (-(n as i32), 0), Deslocamento::Horizontal => (0, -(n as i32)) };
        let bloco = bloco_deslocado(&a, sentido, false);
        let tem_depois = bloco.as_ref().is_some_and(|b| !sheet.chaves_no_retangulo(b).is_empty());
        if tem_depois {
            return self.mover_intervalo(indice, bloco.expect("conferido"), dr, dc)
                .ok_or_else(|| crate::tr!("Não consegui deslocar as células.").into());
        }
        // Nada depois: só apagar a seleção, com as referências a ela virando #REF!.
        // É o mesmo mover, de um bloco vazio que cai por cima dela.
        let vazio = match sentido {
            Deslocamento::Vertical => Rect::new(MAX_ROWS - n, a.c0, MAX_ROWS - 1, a.c1),
            Deslocamento::Horizontal => Rect::new(a.r0, MAX_COLS - n, a.r1, MAX_COLS - 1),
        };
        let (dr, dc) = match sentido {
            Deslocamento::Vertical => (a.r0 as i32 - vazio.r0 as i32, 0),
            Deslocamento::Horizontal => (0, a.c0 as i32 - vazio.c0 as i32),
        };
        self.mover_intervalo(indice, vazio, dr, dc).ok_or_else(|| crate::tr!("Não consegui excluir as células.").into())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::Value;

    fn pasta(celulas: &[(u32, u32, &str)]) -> Workbook {
        let mut w = Workbook::new();
        w.editar(0, |s| for &(r, c, t) in celulas { s.set_cell(r, c, t); });
        w
    }
    fn raw(w: &Workbook, r: u32, c: u32) -> String { w.aba(0).unwrap().sheet.get_raw(r, c) }

    #[test]
    fn inserir_para_baixo_so_desloca_as_colunas_da_selecao() {
        let mut w = pasta(&[(0, 0, "a1"), (1, 0, "a2"), (2, 0, "a3"), (1, 1, "b2"), (5, 3, "=A2&A3")]);
        w.inserir_celulas(0, Rect::new(1, 0, 2, 0), Deslocamento::Vertical).unwrap();
        assert_eq!((raw(&w, 0, 0), raw(&w, 1, 0), raw(&w, 3, 0), raw(&w, 4, 0)), ("a1".into(), "".into(), "a2".into(), "a3".into()));
        assert_eq!(raw(&w, 1, 1), "b2", "a coluna B não anda");
        assert_eq!(raw(&w, 5, 3), "=A4&A5", "quem lia acompanha");
        assert_eq!(w.valor(0, 5, 3), Value::Text("a2a3".into()));
        assert!(w.editar(0, |s| { s.undo(); }).is_some());
        assert_eq!(raw(&w, 1, 0), "a2", "um Ctrl+Z volta");
    }

    #[test]
    fn excluir_para_cima_puxa_e_referencia_excluida_vira_ref() {
        let mut w = pasta(&[(0, 0, "1"), (1, 0, "2"), (2, 0, "3"), (0, 2, "=A2"), (1, 2, "=A3*10")]);
        w.excluir_celulas(0, Rect::new(1, 0, 1, 0), Deslocamento::Vertical).unwrap();
        assert_eq!((raw(&w, 0, 0), raw(&w, 1, 0), raw(&w, 2, 0)), ("1".into(), "3".into(), "".into()));
        assert!(raw(&w, 0, 2).contains("#REF!"), "{}", raw(&w, 0, 2));
        assert_eq!(raw(&w, 1, 2), "=A2*10");
        assert_eq!(w.valor(0, 1, 2), Value::Number(30.));
    }

    #[test]
    fn horizontal_e_excluir_sem_nada_depois() {
        let mut w = pasta(&[(0, 0, "x"), (0, 1, "y"), (0, 2, "z"), (3, 0, "=C1")]);
        w.inserir_celulas(0, Rect::new(0, 1, 0, 1), Deslocamento::Horizontal).unwrap();
        assert_eq!((raw(&w, 0, 1), raw(&w, 0, 2), raw(&w, 0, 3)), ("".into(), "y".into(), "z".into()));
        assert_eq!(raw(&w, 3, 0), "=D1");
        w.excluir_celulas(0, Rect::new(0, 3, 0, 3), Deslocamento::Horizontal).unwrap();
        assert_eq!(raw(&w, 0, 3), "");
        assert!(raw(&w, 3, 0).contains("#REF!"));
    }

    #[test]
    fn recusa_empurrar_dado_para_fora_da_planilha() {
        let mut w = pasta(&[(MAX_ROWS - 1, 0, "fim")]);
        assert!(w.inserir_celulas(0, Rect::cell(0, 0), Deslocamento::Vertical).unwrap_err().contains("Não cabe"));
    }
}
