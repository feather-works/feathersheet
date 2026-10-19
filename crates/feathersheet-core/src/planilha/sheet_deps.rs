//! Grafo de dependências entre fórmulas: quem lê quem.
//!
//! Substitui `descarte/feathersheet/src/core/sheet-deps.ts` (motor TS antigo).

use std::collections::{HashMap, HashSet};

use crate::formula::ast::Node;
use crate::celulas::refs::{key_col, key_row, CellKey, Rect};

const BLOCK_SHIFT: u32 = 10;
/// Faixa com mais blocos de linhas que isto (64 mil linhas) e mais blocos
/// que colunas é "alta" (`A:A`, `A2:C1000000`): vai para o índice por
/// coluna. Pelo índice de blocos, `A:A` (100 milhões de linhas) eram ~98 mil
/// entradas por faixa — 70 ms por fórmula, e `MÍNIMOSES(AO:AO;A:A;A2;…)`
/// preenchida em 13,7 milhões de linhas travou o app (17/10/2026).
const BLOCOS_FAIXA_ALTA: u32 = 64;

fn faixa_alta(rect: &Rect) -> bool {
    let blocos = (rect.r1 >> BLOCK_SHIFT).saturating_sub(rect.r0 >> BLOCK_SHIFT) + 1;
    blocos > BLOCOS_FAIXA_ALTA && rect.c1.saturating_sub(rect.c0) + 1 < blocos
}

/// Nomes de função voláteis: recalculam mesmo sem nenhuma dependência ter
/// mudado. As mesmas oito da versão TypeScript, em PT-BR e EN-US.
const VOLATILE_KEYWORDS: &[&str] = &[
    "SUBTOTAL",
    "AGGREGATE",
    "AGREGAR",
    "RAND",
    "RANDBETWEEN",
    "RANDARRAY",
    "MATRIZALEATÓRIA",
    "MATRIZALEATORIA",
    "NOW",
    "TODAY",
    "ALEATÓRIO",
    "ALEATORIO",
    "ALEATÓRIOENTRE",
    "ALEATORIOENTRE",
    "AGORA",
    "HOJE",
    // A cor não é dependência: recalculam com as voláteis.
    "SUMBYCOLOR",
    "COUNTBYCOLOR",
    "SOMAR.POR.COR",
    "CONT.SE.COR",
    // A referência sai de uma conta: o grafo não sabe o que elas leem.
    "OFFSET",
    "DESLOC",
    "INDIRECT",
    "INDIRETO",
    "CELL",
    "CÉL",
    "INFO",
    "INFORMAÇÃO",
];

fn is_ascii_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Se `raw` chama, na raiz ou em qualquer parte do texto, uma das funções
/// voláteis — sem casar com nomes que só têm o prefixo em comum
/// (`RANDOM_THING(` não conta).
///
/// Tradução do `VOLATILE.test(raw)` do TypeScript, que usa uma regex com
/// `\b` (fronteira de palavra) e `\s*\(` depois do nome. O `sheet-core` não
/// tem dependência de fora (nem `regex`, de propósito — ver `Cargo.toml`),
/// então a fronteira de palavra é refeita à mão: mesma regra do JavaScript,
/// que só considera `[A-Za-z0-9_]` como caractere de palavra para o `\b`.
pub fn is_volatile(raw: &str) -> bool {
    let chars: Vec<char> = raw.chars().collect();
    let n = chars.len();
    for i in 0..n {
        let prev_is_word = i > 0 && is_ascii_word_char(chars[i - 1]);
        let cur_is_word = is_ascii_word_char(chars[i]);
        if prev_is_word || !cur_is_word {
            // Sem fronteira de palavra aqui (ou a posição não começa com um
            // caractere de palavra, e nenhum dos nomes começa diferente).
            continue;
        }
        for kw in VOLATILE_KEYWORDS {
            // Evita alocar um vetor novo para cada palavra-chave em cada caractere.
            let mut j = i;
            let mut bate = true;
            for esperado in kw.chars() {
                if j == n || chars[j].to_uppercase().next() != Some(esperado) {
                    bate = false;
                    break;
                }
                j += 1;
            }
            if !bate {
                continue;
            }
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j < n && chars[j] == '(' {
                return true;
            }
        }
    }
    false
}

/// O grafo de dependências entre fórmulas.
///
/// Guarda duas fontes: células simples referenciadas diretamente
/// (`precedents`/`dependents`) e intervalos (`rangeDeps`/`rangeIndex`,
/// indexado por bloco de linhas para não varrer tudo a cada mudança). As
/// fórmulas cuja dependência é deduzida do padrão (coluna preenchida para
/// baixo) não entram aqui — é o `formula-pattern` que cuida delas, e quem
/// chama precisa intercalar as duas fontes.
#[derive(Default)]
pub struct DependencyGraph {
    /// célula -> fórmulas que a referenciam diretamente.
    pub dependents: HashMap<CellKey, HashSet<CellKey>>,
    /// fórmula -> intervalos que ela lê.
    pub range_deps: HashMap<CellKey, Vec<Rect>>,
    /// bloco de linhas -> fórmulas com intervalos que tocam o bloco.
    pub range_index: HashMap<u32, HashSet<CellKey>>,
    /// coluna -> fórmulas com intervalos **altos** ([`faixa_alta`]) que
    /// passam pela coluna. Esses não entram em `range_index`.
    pub range_cols: HashMap<u32, HashSet<CellKey>>,
    /// fórmula -> células simples que ela lê.
    pub precedents: HashMap<CellKey, Vec<CellKey>>,
    pub volatiles: HashSet<CellKey>,
    pub array_cells: HashSet<CellKey>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.dependents.clear();
        self.range_deps.clear();
        self.range_index.clear();
        self.range_cols.clear();
        self.precedents.clear();
        self.volatiles.clear();
        self.array_cells.clear();
    }

    pub fn shift_columns(&mut self, at: u32, count: u32, removing: bool) {
        if count == 0 {
            return;
        }

        let shift_key = |k: CellKey| -> Option<CellKey> {
            let r = key_row(k);
            let c = key_col(k);
            if removing {
                if c >= at && c < at + count {
                    None
                } else if c >= at + count {
                    Some(crate::celulas::refs::key(r, c - count))
                } else {
                    Some(k)
                }
            } else if c >= at {
                Some(crate::celulas::refs::key(r, c + count))
            } else {
                Some(k)
            }
        };

        let mut new_dependents: HashMap<CellKey, HashSet<CellKey>> = HashMap::with_capacity(self.dependents.len());
        for (target, set) in self.dependents.drain() {
            if let Some(new_target) = shift_key(target) {
                let mut new_set = HashSet::with_capacity(set.len());
                for dep in set {
                    if let Some(new_dep) = shift_key(dep) {
                        new_set.insert(new_dep);
                    }
                }
                if !new_set.is_empty() {
                    new_dependents.insert(new_target, new_set);
                }
            }
        }
        self.dependents = new_dependents;

        let mut new_precedents: HashMap<CellKey, Vec<CellKey>> = HashMap::with_capacity(self.precedents.len());
        for (fk, deps) in self.precedents.drain() {
            if let Some(new_fk) = shift_key(fk) {
                let mut new_deps = Vec::with_capacity(deps.len());
                for dep in deps {
                    if let Some(new_dep) = shift_key(dep) {
                        new_deps.push(new_dep);
                    }
                }
                if !new_deps.is_empty() {
                    new_precedents.insert(new_fk, new_deps);
                }
            }
        }
        self.precedents = new_precedents;

        let mut new_range_deps: HashMap<CellKey, Vec<Rect>> = HashMap::with_capacity(self.range_deps.len());
        for (fk, rects) in self.range_deps.drain() {
            if let Some(new_fk) = shift_key(fk) {
                let mut new_rects = Vec::with_capacity(rects.len());
                for rect in rects {
                    let c0 = if removing {
                        if rect.c0 < at {
                            rect.c0
                        } else if rect.c0 < at + count {
                            at
                        } else {
                            rect.c0 - count
                        }
                    } else if rect.c0 >= at {
                        rect.c0 + count
                    } else {
                        rect.c0
                    };

                    let c1 = if removing {
                        if rect.c1 < at {
                            rect.c1
                        } else if rect.c1 < at + count {
                            at.saturating_sub(1)
                        } else {
                            rect.c1 - count
                        }
                    } else if rect.c1 >= at {
                        rect.c1 + count
                    } else {
                        rect.c1
                    };

                    if c0 <= c1 {
                        new_rects.push(Rect { r0: rect.r0, c0, r1: rect.r1, c1, sheet: rect.sheet });
                    }
                }
                if !new_rects.is_empty() {
                    new_range_deps.insert(new_fk, new_rects);
                }
            }
        }
        self.range_deps = new_range_deps;

        self.reindexar_faixas();

        let mut new_volatiles = HashSet::with_capacity(self.volatiles.len());
        for fk in self.volatiles.drain() {
            if let Some(new_fk) = shift_key(fk) {
                new_volatiles.insert(new_fk);
            }
        }
        self.volatiles = new_volatiles;

        let mut new_array_cells = HashSet::with_capacity(self.array_cells.len());
        for fk in self.array_cells.drain() {
            if let Some(new_fk) = shift_key(fk) {
                new_array_cells.insert(new_fk);
            }
        }
        self.array_cells = new_array_cells;
    }

    pub fn shift_rows(&mut self, at: u32, count: u32, removing: bool) {
        if count == 0 {
            return;
        }

        let shift_key = |k: CellKey| -> Option<CellKey> {
            let r = key_row(k);
            let c = key_col(k);
            if removing {
                if r >= at && r < at + count {
                    None
                } else if r >= at + count {
                    Some(crate::celulas::refs::key(r - count, c))
                } else {
                    Some(k)
                }
            } else if r >= at {
                Some(crate::celulas::refs::key(r + count, c))
            } else {
                Some(k)
            }
        };

        let mut new_dependents: HashMap<CellKey, HashSet<CellKey>> = HashMap::with_capacity(self.dependents.len());
        for (target, set) in self.dependents.drain() {
            if let Some(new_target) = shift_key(target) {
                let mut new_set = HashSet::with_capacity(set.len());
                for dep in set {
                    if let Some(new_dep) = shift_key(dep) {
                        new_set.insert(new_dep);
                    }
                }
                if !new_set.is_empty() {
                    new_dependents.insert(new_target, new_set);
                }
            }
        }
        self.dependents = new_dependents;

        let mut new_precedents: HashMap<CellKey, Vec<CellKey>> = HashMap::with_capacity(self.precedents.len());
        for (fk, deps) in self.precedents.drain() {
            if let Some(new_fk) = shift_key(fk) {
                let mut new_deps = Vec::with_capacity(deps.len());
                for dep in deps {
                    if let Some(new_dep) = shift_key(dep) {
                        new_deps.push(new_dep);
                    }
                }
                if !new_deps.is_empty() {
                    new_precedents.insert(new_fk, new_deps);
                }
            }
        }
        self.precedents = new_precedents;

        let mut new_range_deps: HashMap<CellKey, Vec<Rect>> = HashMap::with_capacity(self.range_deps.len());
        for (fk, rects) in self.range_deps.drain() {
            if let Some(new_fk) = shift_key(fk) {
                let mut new_rects = Vec::with_capacity(rects.len());
                for rect in rects {
                    let r0 = if removing {
                        if rect.r0 < at {
                            rect.r0
                        } else if rect.r0 < at + count {
                            at
                        } else {
                            rect.r0 - count
                        }
                    } else if rect.r0 >= at {
                        rect.r0 + count
                    } else {
                        rect.r0
                    };

                    let r1 = if removing {
                        if rect.r1 < at {
                            rect.r1
                        } else if rect.r1 < at + count {
                            at.saturating_sub(1)
                        } else {
                            rect.r1 - count
                        }
                    } else if rect.r1 >= at {
                        rect.r1 + count
                    } else {
                        rect.r1
                    };

                    if r0 <= r1 {
                        new_rects.push(Rect { r0, c0: rect.c0, r1, c1: rect.c1, sheet: rect.sheet });
                    }
                }
                if !new_rects.is_empty() {
                    new_range_deps.insert(new_fk, new_rects);
                }
            }
        }
        self.range_deps = new_range_deps;

        self.reindexar_faixas();

        let mut new_volatiles = HashSet::with_capacity(self.volatiles.len());
        for fk in self.volatiles.drain() {
            if let Some(new_fk) = shift_key(fk) {
                new_volatiles.insert(new_fk);
            }
        }
        self.volatiles = new_volatiles;

        let mut new_array_cells = HashSet::with_capacity(self.array_cells.len());
        for fk in self.array_cells.drain() {
            if let Some(new_fk) = shift_key(fk) {
                new_array_cells.insert(new_fk);
            }
        }
        self.array_cells = new_array_cells;
    }

    /// Lê as referências da árvore `ast` e registra as dependências da
    /// fórmula em `k`. `is_self` diz se uma referência (com aba `sheet`,
    /// quando tem) fala da própria aba; `set_external` marca a fórmula como
    /// tendo pelo menos uma referência para fora dessa aba.
    pub fn link(
        &mut self,
        k: CellKey,
        ast: &Node,
        is_self: &dyn Fn(Option<&str>) -> bool,
        set_external: &mut dyn FnMut(CellKey, bool),
    ) {
        let mut cell_deps: Vec<CellKey> = Vec::new();
        let mut rects: Vec<Rect> = Vec::new();
        let mut external = false;

        ast.walk(&mut |n| match n {
            Node::Ref { row, col, sheet } => {
                if is_self(sheet.as_deref()) {
                    cell_deps.push(crate::celulas::refs::key(*row, *col));
                } else {
                    external = true;
                }
            }
            Node::Range { r0, c0, r1, c1, sheet } => {
                if is_self(sheet.as_deref()) {
                    rects.push(Rect::new(*r0, *c0, *r1, *c1));
                } else {
                    external = true;
                }
            }
            Node::Name(_) => {
                external = true;
            }
            _ => {}
        });

        if external {
            set_external(k, true);
        }
        // Âncora de matriz dinâmica: ver `planilha::sheet::derramamento`.
        if crate::planilha::sheet_spill::maybe_matrix(ast) {
            self.array_cells.insert(k);
        }
        if !cell_deps.is_empty() {
            for dep in &cell_deps {
                self.dependents.entry(*dep).or_default().insert(k);
            }
            self.precedents.insert(k, cell_deps);
        }
        if !rects.is_empty() {
            for rect in &rects {
                self.indexar_faixa(k, rect);
            }
            self.range_deps.insert(k, rects);
        }
    }

    /// Desfaz o que `link` tinha registrado para `k`.
    pub fn unlink(&mut self, k: CellKey, set_external: &mut dyn FnMut(CellKey, bool)) {
        set_external(k, false);
        self.array_cells.remove(&k);

        if let Some(deps) = self.precedents.remove(&k) {
            for dep in deps {
                if let Some(set) = self.dependents.get_mut(&dep) {
                    set.remove(&k);
                    if set.is_empty() {
                        self.dependents.remove(&dep);
                    }
                }
            }
        }

        if let Some(rects) = self.range_deps.remove(&k) {
            for rect in &rects {
                self.desindexar_faixa(k, rect);
            }
        }

        self.volatiles.remove(&k);
    }

    /// Um nível só: avisa `visit` de cada fórmula **guardada aqui** que lê
    /// esta célula. Quem faz a cascata é quem chama, porque a `Sheet`
    /// precisa intercalar esta fonte com a outra — as fórmulas colunares,
    /// cujas dependências são deduzidas do padrão em vez de guardadas.
    pub fn step_dependents(&self, k: CellKey, mut visit: impl FnMut(CellKey)) {
        let row = key_row(k);
        let col = key_col(k);

        if let Some(direct) = self.dependents.get(&k) {
            for &dep in direct {
                visit(dep);
            }
        }

        let contem = |rect: &Rect| row >= rect.r0 && row <= rect.r1 && col >= rect.c0 && col <= rect.c1;
        if let Some(candidates) = self.range_index.get(&(row >> BLOCK_SHIFT)) {
            for &dep in candidates {
                let Some(rects) = self.range_deps.get(&dep) else { continue };
                if rects.iter().any(|r| !faixa_alta(r) && contem(r)) {
                    visit(dep);
                }
            }
        }
        // Cada fórmula uma vez só: pelas altas, só se nenhuma baixa já casou.
        if let Some(candidates) = self.range_cols.get(&col) {
            for &dep in candidates {
                let Some(rects) = self.range_deps.get(&dep) else { continue };
                if rects.iter().any(|r| faixa_alta(r) && contem(r))
                    && !rects.iter().any(|r| !faixa_alta(r) && contem(r)) {
                    visit(dep);
                }
            }
        }
    }

    fn indexar_faixa(&mut self, k: CellKey, rect: &Rect) {
        if faixa_alta(rect) {
            for c in rect.c0..=rect.c1 {
                self.range_cols.entry(c).or_default().insert(k);
            }
        } else {
            for b in (rect.r0 >> BLOCK_SHIFT)..=(rect.r1 >> BLOCK_SHIFT) {
                self.range_index.entry(b).or_default().insert(k);
            }
        }
    }

    fn desindexar_faixa(&mut self, k: CellKey, rect: &Rect) {
        let (indice, de, ate) = if faixa_alta(rect) {
            (&mut self.range_cols, rect.c0, rect.c1)
        } else {
            (&mut self.range_index, rect.r0 >> BLOCK_SHIFT, rect.r1 >> BLOCK_SHIFT)
        };
        for i in de..=ate {
            if let Some(set) = indice.get_mut(&i) {
                set.remove(&k);
                if set.is_empty() {
                    indice.remove(&i);
                }
            }
        }
    }

    /// Refaz os dois índices de faixas a partir de `range_deps`.
    fn reindexar_faixas(&mut self) {
        self.range_index.clear();
        self.range_cols.clear();
        let deps = std::mem::take(&mut self.range_deps);
        for (&fk, rects) in &deps {
            for rect in rects {
                self.indexar_faixa(fk, rect);
            }
        }
        self.range_deps = deps;
    }

    /// Marca como suja, em cascata, toda fórmula que depende (direta ou
    /// indiretamente) de `k`. `mark_dirty` decide se cada uma precisa
    /// entrar na fila — devolve `false` para quem já estava suja, cortando
    /// o ciclo. Devolve a lista de tudo que foi sujado.
    pub fn invalidate_dependents(&self, k: CellKey, mut mark_dirty: impl FnMut(CellKey) -> bool) -> Vec<CellKey> {
        let mut queue: Vec<CellKey> = vec![k];
        let mut dirtied: Vec<CellKey> = Vec::new();
        while let Some(atual) = queue.pop() {
            let mut novos: Vec<CellKey> = Vec::new();
            self.step_dependents(atual, |dep| {
                if mark_dirty(dep) {
                    novos.push(dep);
                }
            });
            for dep in novos {
                queue.push(dep);
                dirtied.push(dep);
            }
        }
        dirtied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::celulas::refs::key;

    #[test]
    fn nao_volatil_para_texto_qualquer() {
        assert!(!is_volatile("=A1+B1"));
        assert!(!is_volatile("=SOMA(A1:A10)"));
    }

    #[test]
    fn detecta_rand_e_variantes() {
        assert!(is_volatile("=RAND()"));
        assert!(is_volatile("=rand()"));
        assert!(is_volatile("=RANDBETWEEN(1;10)"));
        assert!(is_volatile("=NOW()"));
        assert!(is_volatile("=TODAY()"));
        assert!(is_volatile("=AGORA()"));
        assert!(is_volatile("=HOJE()"));
        assert!(is_volatile("=ALEATÓRIO()"));
        assert!(is_volatile("=aleatorio()"));
        assert!(is_volatile("=ALEATÓRIOENTRE(1;10)"));
    }

    #[test]
    fn nao_confunde_prefixo_com_a_funcao() {
        // "RANDOM" não é nenhuma das funções voláteis listadas.
        assert!(!is_volatile("=RANDOM(1)"));
    }

    #[test]
    fn precisa_do_parenteses_logo_depois_ignorando_espaco() {
        assert!(is_volatile("=RAND ()"));
        assert!(!is_volatile("=RANDX()"));
    }

    #[test]
    fn volatil_no_meio_da_formula_tambem_conta() {
        assert!(is_volatile("=1+RAND()"));
    }

    #[test]
    fn link_registra_dependencia_direta_e_stepdependents_acha() {
        let mut g = DependencyGraph::new();
        let ast = Node::Ref { row: 0, col: 0, sheet: None }; // A1
        let formula_key = key(1, 1); // B2
        g.link(formula_key, &ast, &|_sheet| true, &mut |_, _| {});

        let mut achou = Vec::new();
        g.step_dependents(key(0, 0), |dep| achou.push(dep));
        assert_eq!(achou, vec![formula_key]);
    }

    #[test]
    fn link_com_referencia_externa_marca_external_e_nao_guarda_precedente() {
        let mut g = DependencyGraph::new();
        let ast = Node::Ref { row: 0, col: 0, sheet: Some("Outra".to_string()) };
        let formula_key = key(1, 1);
        let mut marcados = Vec::new();
        g.link(formula_key, &ast, &|_sheet| false, &mut |k, ext| marcados.push((k, ext)));

        assert_eq!(marcados, vec![(formula_key, true)]);
        assert!(!g.precedents.contains_key(&formula_key));
    }

    #[test]
    fn unlink_desfaz_o_que_link_registrou() {
        let mut g = DependencyGraph::new();
        let ast = Node::Ref { row: 0, col: 0, sheet: None };
        let formula_key = key(1, 1);
        g.link(formula_key, &ast, &|_sheet| true, &mut |_, _| {});
        assert!(g.dependents.contains_key(&key(0, 0)));

        g.unlink(formula_key, &mut |_, _| {});
        assert!(!g.dependents.contains_key(&key(0, 0)));
        assert!(!g.precedents.contains_key(&formula_key));
    }

    #[test]
    fn faixa_de_coluna_inteira_vai_para_o_indice_por_coluna() {
        let mut grafo = DependencyGraph::new();
        let k = crate::celulas::refs::key(1, 46);
        let ast = crate::formula::parser::parse_formula("MINIFS(AO:AO;A:A;A2;AP2:AP200)").unwrap();
        let inicio = std::time::Instant::now();
        grafo.link(k, &ast, &|_| true, &mut |_, _| {});
        assert!(inicio.elapsed() < std::time::Duration::from_millis(5), "{:?}", inicio.elapsed());
        assert_eq!(grafo.range_cols.len(), 2, "AO e A");
        assert_eq!(grafo.range_index.len(), 1, "AP2:AP200 num bloco só");
        let visitas = |r, c| { let mut v = 0; grafo.step_dependents(crate::celulas::refs::key(r, c), |_| v += 1); v };
        assert_eq!(visitas(50_000_000, 40), 1, "AO lá embaixo");
        assert_eq!(visitas(5, 0), 1, "A");
        assert_eq!(visitas(5, 41), 1, "AP dentro");
        assert_eq!(visitas(500, 41), 0, "AP fora");
        assert_eq!(visitas(5, 1), 0, "B");
        grafo.unlink(k, &mut |_, _| {});
        assert!(grafo.range_cols.is_empty() && grafo.range_index.is_empty());
    }

    #[test]
    fn range_deps_indexa_por_bloco_e_step_dependents_confere_o_retangulo() {
        let mut g = DependencyGraph::new();
        // =SOMA(A1:A5) em B1
        let ast = Node::Range { r0: 0, c0: 0, r1: 4, c1: 0, sheet: None };
        let formula_key = key(0, 1);
        g.link(formula_key, &ast, &|_sheet| true, &mut |_, _| {});

        // A3 está dentro do retângulo: deve achar a fórmula.
        let mut achou = Vec::new();
        g.step_dependents(key(2, 0), |dep| achou.push(dep));
        assert_eq!(achou, vec![formula_key]);

        // B3 está fora do retângulo, mesmo no mesmo bloco de linhas.
        let mut achou2 = Vec::new();
        g.step_dependents(key(2, 1), |dep| achou2.push(dep));
        assert!(achou2.is_empty());
    }

    #[test]
    fn invalidate_dependents_faz_a_cascata_e_corta_ciclo() {
        let mut g = DependencyGraph::new();
        // B1 = A1, C1 = B1
        g.link(key(0, 1), &Node::Ref { row: 0, col: 0, sheet: None }, &|_| true, &mut |_, _| {});
        g.link(key(0, 2), &Node::Ref { row: 0, col: 1, sheet: None }, &|_| true, &mut |_, _| {});

        let mut ja_visitados: HashSet<CellKey> = HashSet::new();
        let dirtied = g.invalidate_dependents(key(0, 0), |dep| ja_visitados.insert(dep));

        assert_eq!(dirtied.len(), 2);
        assert!(dirtied.contains(&key(0, 1)));
        assert!(dirtied.contains(&key(0, 2)));
    }

    #[test]
    fn shift_columns_desloca_chaves_e_rects() {
        let mut g = DependencyGraph::new();
        // C1 (key(0, 2)) reads A1 (key(0, 0))
        g.link(key(0, 2), &Node::Ref { row: 0, col: 0, sheet: None }, &|_| true, &mut |_, _| {});
        assert!(g.dependents.contains_key(&key(0, 0)));

        // Insert 1 column at index 1 (B)
        g.shift_columns(1, 1, false);
        // C1 becomes D1 (key(0, 3)), A1 stays A1 (key(0, 0))
        assert!(g.dependents.contains_key(&key(0, 0)));
        assert_eq!(g.dependents[&key(0, 0)].iter().cloned().collect::<Vec<_>>(), vec![key(0, 3)]);
        assert_eq!(g.precedents[&key(0, 3)], vec![key(0, 0)]);

        // Delete column at index 0 (A)
        g.shift_columns(0, 1, true);
        // A1 was deleted, dependent entry for A1 is gone
        assert!(!g.dependents.contains_key(&key(0, 0)));
    }

    #[test]
    fn shift_rows_desloca_chaves_e_rects() {
        let mut g = DependencyGraph::new();
        // A3 (key(2, 0)) reads A1 (key(0, 0))
        g.link(key(2, 0), &Node::Ref { row: 0, col: 0, sheet: None }, &|_| true, &mut |_, _| {});

        // Insert 1 row at index 1
        g.shift_rows(1, 1, false);
        // A3 becomes A4 (key(3, 0))
        assert_eq!(g.dependents[&key(0, 0)].iter().cloned().collect::<Vec<_>>(), vec![key(3, 0)]);

        // Delete row at index 0
        g.shift_rows(0, 1, true);
        // A1 was deleted, precedence and dependents updated
        assert!(!g.dependents.contains_key(&key(0, 0)));
    }

    #[test]
    fn volatil_nao_casa_prefixos_e_aceita_acentos() {
        assert!(!is_volatile("=1+RANDOM_THING(3)"));
        assert!(!is_volatile("=X_ALEATÓRIO(3)"));
        assert!(!is_volatile("=ALEATÓRIO_VALOR(3)"));
        assert!(!is_volatile("=ALEATÓRIO 42"));
        assert!(is_volatile("=1 + aleatório ( )"));
        assert!(is_volatile("=1 + AleatórioEntre (1; 2)"));
    }

    #[test]
    fn intervalo_entre_blocos_funciona_e_unlink_desindexa() {
        let mut grafo = DependencyGraph::new();
        let formula = key(2, 3);
        let intervalo = Node::Range { r0: 1023, c0: 0, r1: 1025, c1: 1, sheet: None };
        grafo.link(formula, &intervalo, &|_| true, &mut |_, _| {});
        for linha in [1023, 1024, 1025] {
            let mut encontrados = Vec::new();
            grafo.step_dependents(key(linha, 1), |k| encontrados.push(k));
            assert_eq!(encontrados, vec![formula]);
        }
        let mut fora = Vec::new();
        grafo.step_dependents(key(1024, 2), |k| fora.push(k));
        assert!(fora.is_empty());

        grafo.unlink(formula, &mut |_, _| {});
        assert!(grafo.range_index.is_empty());
        assert!(grafo.range_cols.is_empty());
        assert!(grafo.range_deps.is_empty());
    }
}
