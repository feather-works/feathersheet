//! Derramamento de matriz (matriz dinâmica), ligado ao cálculo da aba.
//!
//! Uma fórmula avulsa que pode devolver matriz (`={1;2}`, `SEQUÊNCIA`,
//! `FILTRO`, `=A1:A3*2`… ver [`maybe_matrix`]) é uma **âncora**: o grafo a
//! guarda em `deps.array_cells`, que já acompanha inserir/excluir linhas e
//! colunas. Quando a âncora calcula uma matriz maior que 1×1, o resto dela
//! "derrama" para as vizinhas vazias, que não têm conteúdo próprio, só um
//! valor emprestado. Vizinha ocupada (ou já coberta por outra matriz): a
//! âncora dá `#SPILL!`, como no Excel.
//!
//! Três regras seguram a coerência com o cálculo preguiçoso:
//!
//! 1. **Ler** uma vizinha vazia procura a âncora no índice e, se ela está
//!    suja, calcula a âncora antes ([`Sheet::valor_derramado`]).
//! 2. Uma leitura de **fora** do avaliador (a tela, a pasta) garante antes
//!    que toda âncora suja foi calculada ([`Sheet::garantir_derramamentos`]).
//!    Só de fora: no meio de outra conta, calcular âncora alheia acusaria
//!    referência circular onde não há.
//! 3. Quando a matriz de uma âncora muda (tamanho ou valores), as vizinhas
//!    antigas e novas passam pela cascata de "sujo": quem as lê recalcula.
//!    Escrever dentro da área (derramada ou bloqueada) suja a âncora.
//!
//! O `SpillManager` de `sheet_spill.rs` é a tradução fiel do TypeScript e
//! continua com os testes dele; este módulo é a ligação com a `Sheet` real.
use std::collections::HashMap;

use super::Sheet;
use crate::celulas::refs::{key, key_col, key_row, CellKey, Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::{CellError, Computed, ErrorCode, Matrix, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;

/// Acima disto a matriz é grande demais para derramar (o mesmo teto do TS).
const TAMANHO_MAXIMO: usize = 200_000;

#[derive(Default)]
pub struct Derrames {
    /// A matriz derramada de cada âncora (a âncora mostra o item [0,0]).
    matrizes: HashMap<CellKey, Matrix>,
    /// Vizinha coberta → âncora dona.
    indice: HashMap<CellKey, CellKey>,
    /// A área que cada âncora quis ocupar da última vez, derramada ou não:
    /// escrever ali suja a âncora (o `#SPILL!` some quando a vizinha esvazia).
    areas: HashMap<CellKey, Rect>,
    /// A geração da aba em que todas as âncoras estavam calculadas.
    garantida: Option<u64>,
    garantindo: bool,
}

impl Derrames {
    pub(crate) fn vazio(&self) -> bool {
        self.areas.is_empty() && self.matrizes.is_empty()
    }

    fn tirar(&mut self, ancora: CellKey) -> Option<(Rect, Matrix)> {
        let area = self.areas.remove(&ancora);
        let matriz = self.matrizes.remove(&ancora)?;
        let r0 = key_row(ancora);
        let c0 = key_col(ancora);
        for r in 0..matriz.height() as u32 {
            for c in 0..matriz.width() as u32 {
                self.indice.remove(&key(r0 + r, c0 + c));
            }
        }
        area.map(|a| (a, matriz))
    }
}

fn area_de(ancora: CellKey, altura: u32, largura: u32) -> Rect {
    let r0 = key_row(ancora);
    let c0 = key_col(ancora);
    Rect::new(r0, c0, r0 + altura.max(1) - 1, c0 + largura.max(1) - 1)
}

fn dentro(a: &Rect, r: u32, c: u32) -> bool {
    r >= a.r0 && r <= a.r1 && c >= a.c0 && c <= a.c1
}

impl Sheet {
    /// Se esta célula tem conteúdo próprio (literal ou fórmula).
    fn tem_conteudo(&self, r: u32, c: u32) -> bool {
        self.cells.contains_key(&Self::cell_key(r, c))
            || self.formulas.has(r, c)
            || self.data.cru_em(r, c).is_some()
    }

    /// Avalia a âncora `k` e acerta o derramamento dela. `provisorio`: o
    /// resultado não vai para o cache (fórmula que lê outra aba calculada
    /// sem a pasta), então a área também não muda.
    pub(super) fn avaliar_ancora(&self, k: CellKey, ev: &Evaluator<'_>, node: &Node, provisorio: bool) -> Value {
        let calculado = ev.eval_array(node);
        if provisorio || super::corrente::voltando() {
            return calculado.scalar();
        }
        let (r0, c0) = (key_row(k), key_col(k));
        let (valor, nova): (Value, Option<(Rect, Matrix)>) = match calculado {
            Computed::Matrix(m) if m.size() > 1 => {
                let (h, w) = (m.height() as u32, m.width() as u32);
                if m.size() > TAMANHO_MAXIMO {
                    (Value::Error(CellError::com_motivo(ErrorCode::Num, "Matriz grande demais para derramar")), None)
                } else if r0 + h > MAX_ROWS || c0 + w > MAX_COLS {
                    (Value::Error(CellError::com_motivo(ErrorCode::Ref, "A matriz não cabe na planilha")), None)
                } else {
                    (m.first().clone(), Some((area_de(k, h, w), m)))
                }
            }
            outro => (outro.scalar(), None),
        };
        // Confere o caminho antes de tirar a matriz antiga desta âncora.
        let bloqueio = nova.as_ref().and_then(|(area, _)| {
            let d = self.derrame.borrow();
            for r in area.r0..=area.r1 {
                for c in area.c0..=area.c1 {
                    if (r, c) == (r0, c0) {
                        continue;
                    }
                    if d.indice.get(&key(r, c)).is_some_and(|&dona| dona != k) || d.matrizes.contains_key(&key(r, c)) {
                        return Some("Há outra matriz no caminho");
                    }
                    if self.tem_conteudo(r, c) {
                        return Some("Há conteúdo no caminho da matriz");
                    }
                }
            }
            None
        });
        let mut d = self.derrame.borrow_mut();
        let antiga = d.tirar(k);
        let (valor, ficou) = match (nova, bloqueio) {
            (Some((area, m)), None) => {
                for r in area.r0..=area.r1 {
                    for c in area.c0..=area.c1 {
                        if (r, c) != (r0, c0) {
                            d.indice.insert(key(r, c), k);
                        }
                    }
                }
                d.areas.insert(k, area.clone());
                d.matrizes.insert(k, m.clone());
                (valor, Some((area, m)))
            }
            (Some((area, _)), Some(motivo)) => {
                d.areas.insert(k, area);
                (Value::Error(CellError::com_motivo(ErrorCode::Spill, motivo)), None)
            }
            (None, _) => (valor, None),
        };
        drop(d);
        if antiga.as_ref().map(|(a, m)| (a, m)) != ficou.as_ref().map(|(a, m)| (a, m)) {
            // Quem lê as vizinhas (antigas e novas) recalcula.
            let mut tocadas: Vec<CellKey> = Vec::new();
            for (area, _) in antiga.iter().chain(ficou.iter()) {
                for r in area.r0..=area.r1 {
                    for c in area.c0..=area.c1 {
                        if (r, c) != (r0, c0) {
                            tocadas.push(key(r, c));
                        }
                    }
                }
            }
            tocadas.sort_unstable();
            tocadas.dedup();
            for t in tocadas {
                self.sujar_quem_le(t);
            }
        }
        valor
    }

    /// Fora do caminho quente da leitura: só existe quando a aba tem âncora.
    #[cold]
    #[inline(never)]
    pub(super) fn vazia_ou_derramada(&self, row: u32, col: u32, host: Option<&dyn super::Hospedeiro>) -> Value {
        self.valor_derramado(row, col, host).unwrap_or(Value::Empty)
    }

    #[cold]
    #[inline(never)]
    pub(super) fn garantir_derramamentos_frio(&self, host: Option<&dyn super::Hospedeiro>) {
        self.garantir_derramamentos(host);
    }

    /// O valor derramado em (row, col), se uma âncora cobre essa vizinha.
    pub(super) fn valor_derramado(&self, row: u32, col: u32, host: Option<&dyn super::Hospedeiro>) -> Option<Value> {
        let k = Self::cell_key(row, col);
        let ancora = *self.derrame.borrow().indice.get(&k)?;
        if self.ancora_pendente(ancora) {
            let (ar, ac) = Self::key_to_coord(ancora);
            let _ = self.get_value_em(ar, ac, host);
        }
        let d = self.derrame.borrow();
        let dona = *d.indice.get(&k)?;
        let m = d.matrizes.get(&dona)?;
        Some(m.at((row - key_row(dona)) as usize, (col - key_col(dona)) as usize).clone())
    }

    /// A âncora precisa ser calculada (suja, nunca calculada, ou volátil),
    /// e não está sendo calculada agora.
    fn ancora_pendente(&self, k: CellKey) -> bool {
        if !self.cells.get(&k).is_some_and(|c| c.formula.is_some()) {
            return false;
        }
        let calc = self.calc.borrow();
        !calc.evaluating.contains(&k)
            && (!calc.cache.contains_key(&k) || calc.dirty.contains(&k))
    }

    /// Calcula toda âncora suja — chamado nas leituras de fora do avaliador,
    /// para que nenhuma vizinha seja lida antes da matriz que a cobre.
    pub(crate) fn garantir_derramamentos(&self, host: Option<&dyn super::Hospedeiro>) {
        {
            let mut d = self.derrame.borrow_mut();
            if d.garantindo || d.garantida == Some(self.geracao.get()) {
                return;
            }
            d.garantindo = true;
        }
        // Uma matriz que muda pode sujar outra âncora que a lê: repete até
        // parar de mudar (com teto, por segurança).
        for _ in 0..16 {
            let geracao = self.geracao.get();
            let pendentes: Vec<CellKey> = self.deps.array_cells.iter().copied()
                .filter(|&k| self.ancora_pendente(k))
                .collect();
            for k in pendentes {
                let (r, c) = Self::key_to_coord(k);
                let _ = self.get_value_em(r, c, host);
            }
            if self.geracao.get() == geracao {
                break;
            }
        }
        // Âncora que deixou de ser fórmula de matriz: a área dela some.
        let orfas: Vec<CellKey> = self.derrame.borrow().areas.keys().copied()
            .filter(|k| !self.deps.array_cells.contains(k))
            .collect();
        for k in orfas {
            self.descartar_ancora(k);
        }
        let mut d = self.derrame.borrow_mut();
        d.garantindo = false;
        d.garantida = Some(self.geracao.get());
    }

    /// `k` tinha área de matriz mas a fórmula dele já não pode derramar
    /// (virou literal, foi apagada, ou a fórmula nova é escalar).
    pub(super) fn deixou_de_ser_ancora(&self, k: CellKey) -> bool {
        !self.deps.array_cells.contains(&k) && self.derrame.borrow().areas.contains_key(&k)
    }

    /// Os valores derramados dentro de `rect` (sem as âncoras, que têm
    /// célula própria): para `SOMA(A1:A10)` e companhia enxergarem a matriz.
    pub(super) fn para_cada_derramado(&self, rect: &Rect, host: Option<&dyn super::Hospedeiro>, visit: &mut dyn FnMut(&Value, u32, u32)) {
        let cruza = |a: &Rect| a.r0 <= rect.r1 && a.r1 >= rect.r0 && a.c0 <= rect.c1 && a.c1 >= rect.c0;
        let mut ancoras: Vec<CellKey> = {
            let d = self.derrame.borrow();
            if d.matrizes.is_empty() {
                return;
            }
            d.matrizes.iter()
                .filter(|(&k, m)| cruza(&area_de(k, m.height() as u32, m.width() as u32)))
                .map(|(&k, _)| k)
                .collect()
        };
        // Ordem fixa entre execuções (o `HashMap` não tem): #116.
        ancoras.sort_unstable();
        for a in ancoras {
            if self.ancora_pendente(a) {
                let (r, c) = Self::key_to_coord(a);
                let _ = self.get_value_em(r, c, host);
            }
            let d = self.derrame.borrow();
            let Some(m) = d.matrizes.get(&a) else { continue };
            let (r0, c0) = (key_row(a), key_col(a));
            for i in 0..m.height() as u32 {
                for j in 0..m.width() as u32 {
                    let (r, c) = (r0 + i, c0 + j);
                    if (i, j) == (0, 0) || !dentro(rect, r, c) {
                        continue;
                    }
                    let v = m.at(i as usize, j as usize);
                    if !matches!(v, Value::Empty) {
                        visit(v, r, c);
                    }
                }
            }
        }
    }

    /// Tira a área da âncora `k` e faz quem lia as vizinhas recalcular.
    pub(super) fn descartar_ancora(&self, k: CellKey) {
        let antiga = self.derrame.borrow_mut().tirar(k);
        if let Some((area, _)) = antiga {
            for r in area.r0..=area.r1 {
                for c in area.c0..=area.c1 {
                    if key(r, c) != k {
                        self.sujar_quem_le(key(r, c));
                    }
                }
            }
        }
    }

    /// Escrever em `k`: se `k` está na área de alguma âncora (derramada ou
    /// bloqueada), a âncora recalcula. Devolve as âncoras afetadas.
    pub(super) fn ancoras_da_area(&self, k: CellKey) -> Vec<CellKey> {
        let d = self.derrame.borrow();
        if d.areas.is_empty() {
            return Vec::new();
        }
        let (r, c) = Self::key_to_coord(k);
        d.areas.iter()
            .filter(|(&a, area)| a != k && dentro(area, r, c))
            .map(|(&a, _)| a)
            .collect()
    }

    /// Esquece todo derramamento (inserir/excluir linhas, recálculo total):
    /// as âncoras recalculam e derramam de novo na próxima leitura.
    pub(crate) fn descartar_derramamentos(&mut self) {
        let d = self.derrame.get_mut();
        d.matrizes.clear();
        d.indice.clear();
        d.areas.clear();
        d.garantida = None;
        let calc = self.calc.get_mut();
        for &k in &self.deps.array_cells {
            calc.cache.remove(&k);
            calc.dirty.insert(k);
        }
    }

    /// As áreas derramadas agora (para o desenho e o cálculo paralelo).
    pub fn areas_derramadas(&self) -> Vec<Rect> {
        let d = self.derrame.borrow();
        d.matrizes.iter().map(|(&k, m)| area_de(k, m.height() as u32, m.width() as u32)).collect()
    }

    /// A área derramada que contém (row, col), âncora incluída.
    pub fn area_derramada_em(&self, row: u32, col: u32) -> Option<Rect> {
        let d = self.derrame.borrow();
        let k = Self::cell_key(row, col);
        let ancora = if d.matrizes.contains_key(&k) { k } else { *d.indice.get(&k)? };
        let m = d.matrizes.get(&ancora)?;
        Some(area_de(ancora, m.height() as u32, m.width() as u32))
    }

    /// Faz a cascata de "sujo" a partir de uma vizinha, como se ela tivesse
    /// sido escrita.
    fn sujar_quem_le(&self, k: CellKey) {
        self.sujar_cascata(k);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::Value;

    fn n(v: f64) -> Value {
        Value::Number(v)
    }

    #[test]
    fn literal_de_matriz_derrama_e_bloqueia() {
        let mut s = Sheet::new("A");
        s.set_cell_raw(0, 0, "={1;2;3}".into(), None);
        assert_eq!(s.get_value(0, 0), n(1.0));
        assert_eq!(s.get_value(1, 0), n(2.0));
        assert_eq!(s.get_value(2, 0), n(3.0));
        assert_eq!(s.get_value(3, 0), Value::Empty);
        // Colunas com '\'.
        s.set_cell_raw(0, 2, "={1\\2}".into(), None);
        assert_eq!(s.get_value(0, 3), n(2.0));
        // Algo no caminho: #SPILL!; esvaziar volta a derramar.
        s.set_cell_raw(1, 0, "x".into(), None);
        assert!(matches!(s.get_value(0, 0), Value::Error(e) if e.code == ErrorCode::Spill));
        assert_eq!(s.get_value(2, 0), Value::Empty);
        s.set_cell_raw(1, 0, "".into(), None);
        assert_eq!(s.get_value(0, 0), n(1.0));
        assert_eq!(s.get_value(2, 0), n(3.0));
    }

    #[test]
    fn quem_le_a_vizinha_recalcula_quando_a_matriz_muda() {
        let mut s = Sheet::new("A");
        s.set_cell_raw(0, 1, "3".into(), None); // B1
        s.set_cell_raw(0, 0, "=SEQUÊNCIA(B1)".into(), None); // A1:A3
        s.set_cell_raw(0, 3, "=SOMA(A1:A5)".into(), None); // D1
        s.set_cell_raw(1, 3, "=A3*10".into(), None); // D2
        assert_eq!(s.get_value(0, 3), n(6.0));
        assert_eq!(s.get_value(1, 3), n(30.0));
        s.set_cell_raw(0, 1, "2".into(), None);
        assert_eq!(s.get_value(0, 3), n(3.0));
        assert_eq!(s.get_value(1, 3), n(0.0));
        assert_eq!(s.get_value(2, 0), Value::Empty);
    }

    #[test]
    fn matriz_some_quando_a_ancora_vira_literal() {
        let mut s = Sheet::new("A");
        s.set_cell_raw(0, 0, "={1;2}".into(), None);
        assert_eq!(s.get_value(1, 0), n(2.0));
        s.set_cell_raw(0, 0, "7".into(), None);
        assert_eq!(s.get_value(0, 0), n(7.0));
        assert_eq!(s.get_value(1, 0), Value::Empty);
        assert!(s.areas_derramadas().is_empty());
    }

    #[test]
    fn duas_matrizes_nao_se_sobrepoem() {
        let mut s = Sheet::new("A");
        s.set_cell_raw(1, 1, "={1;2}".into(), None); // B2:B3
        assert_eq!(s.get_value(2, 1), n(2.0));
        s.set_cell_raw(0, 1, "={5;6}".into(), None); // B1:B2 cruza B2
        assert!(matches!(s.get_value(0, 1), Value::Error(e) if e.code == ErrorCode::Spill));
        assert_eq!(s.get_value(1, 1), n(1.0));
        assert_eq!(s.get_value(2, 1), n(2.0));
    }

    #[test]
    fn inserir_linha_desloca_a_matriz() {
        let mut s = Sheet::new("A");
        s.set_cell_raw(0, 0, "={1;2}".into(), None);
        assert_eq!(s.get_value(1, 0), n(2.0));
        s.insert_rows(0, 1);
        assert_eq!(s.get_value(1, 0), n(1.0));
        assert_eq!(s.get_value(2, 0), n(2.0));
        assert_eq!(s.get_value(0, 0), Value::Empty);
    }
}
