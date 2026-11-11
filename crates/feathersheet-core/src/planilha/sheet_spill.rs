//! Derramamento de matriz (spill) e `#SPILL!`.
//!
//! Migração de `descarte/feathersheet/src/core/sheet-spill.ts`.
//!
//! Uma fórmula como `=SEQUÊNCIA(3)` devolve uma matriz, não um valor só. A
//! célula onde ela está escrita é a **âncora**; o resto da matriz "derrama"
//! para as células vizinhas, que não têm fórmula nenhuma escrita — só um
//! valor emprestado. [`SpillManager`] é quem sabe, para cada célula vizinha,
//! de qual âncora ela deriva.
//!
//! ## Duas adaptações de forma, não de comportamento
//!
//! O TypeScript injeta `getCell`/`updateMaxDimensions` como fechos ligados à
//! `Sheet` que os criou; aqui o [`SpillManager`] é genérico sobre os dois,
//! do mesmo jeito. Só que `getCell` devolve `Option<CellData>` **por
//! valor**, não uma referência — `CellData` já deriva `Clone`
//! ([`crate::planilha::sheet::CellData`]) e o valor só é lido (`raw`), nunca guardado;
//! evita a Rust não ter (ainda) onde amarrar o tempo de vida de uma
//! referência dentro de um `HashMap` que também está mudando. O resultado
//! observável é o mesmo — só a posse do dado muda de mãos.
//!
//! [`ChangeSnapshot`] aqui só tem `k` (a chave da célula que mudou). O
//! TypeScript carrega também `before`/`after` (cada um um `CellSnapshot`,
//! para o desfazer) — mas [`SpillManager::spill_needs_review`], a única
//! função que lê um `ChangeSnapshot`, nunca olha para esses dois campos,
//! nem no original. Como o desfazer ainda não tem tradução em Rust (não
//! existe `CellSnapshot` para copiar), incluí só o que já tem uso real; os
//! dois campos voltam junto quando o desfazer for portado.

use std::collections::{HashMap, HashSet};

use crate::formula::ast::Node;
use crate::funcoes::functions;
use crate::celulas::refs::{key, key_col, key_row, Rect, MAX_COLS, MAX_ROWS};
use crate::planilha::sheet::CellData;
use crate::celulas::values::{CellError, ErrorCode, Matrix, Value};

/// Acima disto a matriz derramada é grande demais para valer a pena.
const TAMANHO_MAXIMO: usize = 200_000;

/// Se a árvore desta fórmula pode produzir uma matriz — e por isso precisa
/// ser avaliada com `eval_array` em vez de `eval` na hora de recalcular.
///
/// Devolve `true` em dois casos: a fórmula chama uma função dinâmica (ou tem
/// um literal de matriz) em qualquer profundidade; ou a fórmula é uma conta
/// (binária, unária, ou uma função que "levanta" matriz — `SE`, `ANO`…)
/// que lê um intervalo com mais de uma célula, porque aí a conta se espalha
/// item a item.
pub fn maybe_matrix(ast: &Node) -> bool {
    let mut dynamic = false;
    ast.walk(&mut |n| match n {
        Node::Call { name, .. } if functions::is_dynamic(name) => dynamic = true,
        Node::Array { .. } => dynamic = true,
        _ => {}
    });
    if dynamic {
        return true;
    }
    let spreads = match ast {
        Node::Binary { .. } | Node::Unary { .. } => true,
        Node::Call { name, .. } => functions::array_lifting(name),
        _ => false,
    };
    if !spreads {
        return false;
    }
    let mut wide_range = false;
    ast.walk(&mut |n| {
        if let Node::Range { r0, c0, r1, c1, .. } = n {
            if r1 > r0 || c1 > c0 {
                wide_range = true;
            }
        }
    });
    wide_range
}

/// Uma célula que mudou — o bastante para [`SpillManager::spill_needs_review`]
/// decidir se um derramamento precisa ser reconferido. Ver a nota de topo do
/// arquivo sobre o que falta aqui (e por quê).
pub struct ChangeSnapshot {
    pub k: u64,
}

pub struct SpillManager<GetCell, UpdateMax>
where
    GetCell: Fn(u64) -> Option<CellData>,
    UpdateMax: FnMut(u32, u32),
{
    pub spills: HashMap<u64, Matrix>,
    pub spill_index: HashMap<u64, u64>,
    get_cell: GetCell,
    update_max_dimensions: UpdateMax,
}

impl<GetCell, UpdateMax> SpillManager<GetCell, UpdateMax>
where
    GetCell: Fn(u64) -> Option<CellData>,
    UpdateMax: FnMut(u32, u32),
{
    pub fn new(get_cell: GetCell, update_max_dimensions: UpdateMax) -> Self {
        SpillManager { spills: HashMap::new(), spill_index: HashMap::new(), get_cell, update_max_dimensions }
    }

    pub fn clear(&mut self) {
        self.spills.clear();
        self.spill_index.clear();
    }

    pub fn has(&self, k: u64) -> bool {
        self.spills.contains_key(&k)
    }

    pub fn has_spills(&self) -> bool {
        !self.spills.is_empty()
    }

    pub fn for_each_spilled_in_range(
        &self,
        r0: u32,
        r1: u32,
        c0: u32,
        c1: u32,
        mut visit: impl FnMut(&Value, u32, u32),
    ) {
        for (&anchor, matrix) in &self.spills {
            let row0 = key_row(anchor);
            let col0 = key_col(anchor);
            let row1 = row0 + matrix.height() as u32 - 1;
            let col1 = col0 + matrix.width() as u32 - 1;
            let start_r = r0.max(row0);
            let end_r = r1.min(row1);
            let start_c = c0.max(col0);
            let end_c = c1.min(col1);
            if start_r > end_r || start_c > end_c {
                continue;
            }
            for r in start_r..=end_r {
                for c in start_c..=end_c {
                    if r == row0 && c == col0 {
                        continue;
                    }
                    let val = matrix.at((r - row0) as usize, (c - col0) as usize);
                    if !matches!(val, Value::Empty) {
                        visit(val, r, c);
                    }
                }
            }
        }
    }

    pub fn get(&self, k: u64) -> Option<&Matrix> {
        self.spills.get(&k)
    }

    /// Derrama `matrix` a partir da âncora. Devolve o valor da própria
    /// âncora (o `matrix.first`) quando dá certo, ou o erro (`#N/A`,
    /// `#NUM!`, `#REF!`, `#SPILL!`) quando não cabe ou algo está no caminho.
    pub fn spill(&mut self, anchor: u64, matrix: Matrix) -> Value {
        // Não deixar que uma nova âncora sobrescreva a área derramada por outra.
        // O get_cell só enxerga conteúdo real: células virtuais de spill têm raw vazio.
        if self.spill_index.contains_key(&anchor) {
            return Value::Error(CellError::com_motivo(ErrorCode::Spill, crate::tr!("Há outra matriz neste endereço")));
        }
        self.clear_spill(anchor);
        if matrix.size() == 0 {
            return Value::Error(CellError::com_motivo(ErrorCode::Na, "A matriz veio vazia"));
        }
        if matrix.size() > TAMANHO_MAXIMO {
            return Value::Error(CellError::com_motivo(ErrorCode::Num, "Matriz grande demais"));
        }

        let row0 = key_row(anchor);
        let col0 = key_col(anchor);
        let height = matrix.height() as u32;
        let width = matrix.width() as u32;
        if row0 + height > MAX_ROWS || col0 + width > MAX_COLS {
            return Value::Error(CellError::com_motivo(ErrorCode::Ref, crate::tr!("A matriz não cabe na planilha")));
        }

        for r in 0..height {
            for c in 0..width {
                if r == 0 && c == 0 {
                    continue;
                }
                let covered = key(row0 + r, col0 + c);
                // A âncora de outra matriz não está no índice das virtuais: conferir à parte.
                if self.spill_index.contains_key(&covered) || self.spills.contains_key(&covered) {
                    return Value::Error(CellError::com_motivo(ErrorCode::Spill, crate::tr!("Há outra matriz no caminho")));
                }
                if let Some(occupant) = (self.get_cell)(covered) {
                    if !occupant.raw.is_empty() {
                        return Value::Error(CellError::com_motivo(ErrorCode::Spill, crate::tr!("Há conteúdo no caminho da matriz")));
                    }
                }
            }
        }

        let primeiro = matrix.first().clone();
        self.spills.insert(anchor, matrix);
        // Releu do mapa em vez de reusar `matrix` (movida acima) — mesmo
        // `height`/`width` já calculados, sem precisar clonar a matriz só
        // para varrer de novo.
        for r in 0..height {
            for c in 0..width {
                if r == 0 && c == 0 {
                    continue;
                }
                self.spill_index.insert(key(row0 + r, col0 + c), anchor);
            }
        }
        (self.update_max_dimensions)(row0 + height - 1, col0 + width - 1);
        primeiro
    }

    pub fn clear_spill(&mut self, anchor: u64) {
        let Some(matrix) = self.spills.get(&anchor) else {
            return;
        };
        let row0 = key_row(anchor);
        let col0 = key_col(anchor);
        for r in 0..matrix.height() as u32 {
            for c in 0..matrix.width() as u32 {
                self.spill_index.remove(&key(row0 + r, col0 + c));
            }
        }
        self.spills.remove(&anchor);
    }

    pub fn spilled_value(&self, row: u32, col: u32) -> Value {
        let Some(&anchor) = self.spill_index.get(&key(row, col)) else {
            return Value::Empty;
        };
        let Some(matrix) = self.spills.get(&anchor) else {
            return Value::Empty;
        };
        matrix.at((row - key_row(anchor)) as usize, (col - key_col(anchor)) as usize).clone()
    }

    pub fn spill_rect(&self, row: u32, col: u32) -> Option<Rect> {
        let k = key(row, col);
        let anchor = if self.spills.contains_key(&k) { Some(k) } else { self.spill_index.get(&k).copied() }?;
        let matrix = self.spills.get(&anchor)?;
        let r0 = key_row(anchor);
        let c0 = key_col(anchor);
        Some(Rect::new(r0, c0, r0 + matrix.height() as u32 - 1, c0 + matrix.width() as u32 - 1))
    }

    /// Se um derramamento precisa ser reconferido: a própria célula está em
    /// `#SPILL!` (algo apareceu no caminho e nunca foi tentado de novo), ou
    /// alguma das mudanças cai dentro da área que ele ocupa.
    pub fn spill_needs_review(&self, anchor: u64, cell_value: &Value, changes: &[ChangeSnapshot]) -> bool {
        if changes.is_empty() {
            return false;
        }
        if matches!(cell_value, Value::Error(e) if e.code == ErrorCode::Spill) {
            return true;
        }
        let Some(matrix) = self.spills.get(&anchor) else {
            return false;
        };
        let row0 = key_row(anchor);
        let col0 = key_col(anchor);
        let row1 = row0 + matrix.height() as u32 - 1;
        let col1 = col0 + matrix.width() as u32 - 1;
        changes.iter().any(|change| {
            let r = key_row(change.k);
            let c = key_col(change.k);
            r >= row0 && r <= row1 && c >= col0 && c <= col1
        })
    }
}

/// Marca, em `into`, todas as chaves que a matriz derramada a partir de
/// `anchor` ocupa (incluindo a própria âncora).
pub fn mark_spill_area(matrix: Option<&Matrix>, anchor: u64, into: &mut HashSet<u64>) {
    let Some(matrix) = matrix else {
        return;
    };
    let row0 = key_row(anchor);
    let col0 = key_col(anchor);
    for r in 0..matrix.height() as u32 {
        for c in 0..matrix.width() as u32 {
            into.insert(key(row0 + r, col0 + c));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::parser::parse_formula;

    fn matriz_2x1() -> Matrix {
        Matrix::new(vec![vec![Value::Number(1.0)], vec![Value::Number(2.0)]])
    }

    fn gerenciador_vazio() -> SpillManager<impl Fn(u64) -> Option<CellData>, impl FnMut(u32, u32)> {
        SpillManager::new(|_k: u64| None, |_r: u32, _c: u32| {})
    }

    #[test]
    fn sequencia_e_matriz_literal_sao_dinamicas() {
        assert!(maybe_matrix(&parse_formula("SEQUÊNCIA(3)").unwrap()));
        assert!(maybe_matrix(&parse_formula("{1;2;3}").unwrap()));
        assert!(!maybe_matrix(&parse_formula("SOMA(A1:A3)").unwrap()));
    }

    #[test]
    fn conta_com_intervalo_largo_espalha() {
        // A1:A3 tem mais de uma célula — a soma item a item espalha.
        assert!(maybe_matrix(&parse_formula("A1:A3+1").unwrap()));
        // Célula única não espalha nada.
        assert!(!maybe_matrix(&parse_formula("A1+1").unwrap()));
    }

    #[test]
    fn se_levanta_matriz_só_com_intervalo_largo() {
        assert!(maybe_matrix(&parse_formula("SE(A1:A3>2;1;0)").unwrap()));
        assert!(!maybe_matrix(&parse_formula("SE(A1>2;1;0)").unwrap()));
    }

    #[test]
    fn derrama_e_le_de_volta() {
        let mut gm = gerenciador_vazio();
        let anchor = key(0, 3); // D1
        let v = gm.spill(anchor, matriz_2x1());
        assert_eq!(v, Value::Number(1.0)); // a âncora fica com o primeiro valor
        assert_eq!(gm.spilled_value(1, 3), Value::Number(2.0)); // D2, derramada
        assert_eq!(gm.spill_rect(0, 3), Some(Rect::new(0, 3, 1, 3)));
        assert_eq!(gm.spill_rect(1, 3), Some(Rect::new(0, 3, 1, 3)));
    }

    #[test]
    fn matriz_vazia_e_erro_na() {
        let mut gm = gerenciador_vazio();
        let v = gm.spill(key(0, 0), Matrix::new(vec![]));
        assert!(matches!(v, Value::Error(e) if e.code == ErrorCode::Na));
    }

    #[test]
    fn matriz_grande_demais_e_erro_num() {
        let mut gm = gerenciador_vazio();
        let linha = vec![Value::Number(1.0); TAMANHO_MAXIMO + 1];
        let v = gm.spill(key(0, 0), Matrix::new(vec![linha]));
        assert!(matches!(v, Value::Error(e) if e.code == ErrorCode::Num));
    }

    #[test]
    fn conteudo_no_caminho_vira_spill() {
        let mut gm = SpillManager::new(
            |k: u64| {
                if k == key(1, 3) {
                    Some(CellData { raw: "ocupado".to_string(), formula: None, value: Value::Empty, style_idx: None })
                } else {
                    None
                }
            },
            |_r: u32, _c: u32| {},
        );
        let v = gm.spill(key(0, 3), matriz_2x1());
        assert!(matches!(v, Value::Error(e) if e.code == ErrorCode::Spill));
        // Não deveria ter registrado nada — o derramamento falhou.
        assert!(!gm.has(key(0, 3)));
    }

    #[test]
    fn nao_sobrepoe_duas_matrizes_derrubadas() {
        let mut gm = gerenciador_vazio();
        let primeira = key(1, 1); // B2:B3
        assert_eq!(gm.spill(primeira, matriz_2x1()), Value::Number(1.0));

        // B1:B2 cruza B2; não pode substituir a matriz que já existe.
        let segunda = key(0, 1);
        let conflito = gm.spill(segunda, matriz_2x1());
        assert!(matches!(conflito, Value::Error(e) if e.code == ErrorCode::Spill));
        assert!(!gm.has(segunda));
        assert!(gm.has(primeira));
        assert_eq!(gm.spilled_value(2, 1), Value::Number(2.0));

        // B3 já é uma célula virtual da matriz anterior; não pode virar âncora.
        let conflito_na_ancora = gm.spill(key(2, 1), matriz_2x1());
        assert!(matches!(conflito_na_ancora, Value::Error(e) if e.code == ErrorCode::Spill));
        assert_eq!(gm.spilled_value(2, 1), Value::Number(2.0));

        // Depois de liberar a matriz anterior, a região pode ser reaproveitada.
        gm.clear_spill(primeira);
        assert_eq!(gm.spill(segunda, matriz_2x1()), Value::Number(1.0));
        assert_eq!(gm.spilled_value(1, 1), Value::Number(2.0));
    }

    #[test]
    fn clear_spill_libera_a_area() {
        let mut gm = gerenciador_vazio();
        let anchor = key(0, 3);
        gm.spill(anchor, matriz_2x1());
        assert!(gm.has(anchor));
        gm.clear_spill(anchor);
        assert!(!gm.has(anchor));
        assert_eq!(gm.spilled_value(1, 3), Value::Empty);
    }

    #[test]
    fn spill_needs_review_pega_mudanca_dentro_da_area_e_erro_spill() {
        let mut gm = gerenciador_vazio();
        let anchor = key(0, 3);
        gm.spill(anchor, matriz_2x1());

        assert!(!gm.spill_needs_review(anchor, &Value::Number(1.0), &[]));
        // B7 não cruza com D1:D2.
        assert!(!gm.spill_needs_review(anchor, &Value::Number(1.0), &[ChangeSnapshot { k: key(6, 1) }]));
        // D2 está dentro da área derramada.
        assert!(gm.spill_needs_review(anchor, &Value::Number(1.0), &[ChangeSnapshot { k: key(1, 3) }]));
        // Célula em #SPILL! sempre pede reconferência, mudança ou não.
        let erro_spill = Value::Error(CellError::new(ErrorCode::Spill));
        assert!(gm.spill_needs_review(anchor, &erro_spill, &[ChangeSnapshot { k: key(99, 99) }]));
    }

    #[test]
    fn mark_spill_area_marca_a_ancora_e_o_resto() {
        let mut into = HashSet::new();
        mark_spill_area(Some(&matriz_2x1()), key(0, 3), &mut into);
        assert_eq!(into.len(), 2);
        assert!(into.contains(&key(0, 3)));
        assert!(into.contains(&key(1, 3)));
    }
}
