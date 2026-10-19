//! A tabela de células de uma aba: onde o valor de cada célula mora.
//!
//! Substitui `descarte/feathersheet/src/core/data-table.ts`. Cada coluna com dado vira uma
//! entrada no mapa `columns`, guardando um [`ChunkedFloatColumn`] (números)
//! e/ou um [`ChunkedInt32Column`] (texto e booleano, como id no
//! [`StringPool`]) — nunca os dois preenchidos na mesma linha.

use std::collections::HashMap;

use crate::planilha::column_store::{ChunkedFloatColumn, ChunkedInt32Column};
use crate::celulas::number::to_js_string;
use crate::celulas::pool::StringPool;
use crate::celulas::refs::Rect;
use crate::celulas::values::Value;

/// O conteúdo de uma célula da tabela, sem montar o `Value`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cru {
    Numero(f64),
    /// Id no [`StringPool`] (texto, ou `VERDADEIRO`/`FALSO`).
    Texto(i32),
}

#[derive(Default)]
struct ColumnEntry {
    float_col: Option<ChunkedFloatColumn>,
    text_col: Option<ChunkedInt32Column>,
}

#[derive(Default)]
pub struct DataTable {
    columns: HashMap<u32, ColumnEntry>,
    tracked_max_row: u32,
    tracked_max_col: u32,
}

impl DataTable {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_or_create_column(&mut self, col: u32) -> &mut ColumnEntry {
        self.columns.entry(col).or_default()
    }

    /// Exporta todas as colunas de números para gravação no formato colunar `.fshz`.
    pub fn exportar_colunas_num(&self, linhas: u32) -> Vec<(u32, Vec<f64>, Vec<u8>)> {
        let cols = self.colunas_com(|e| e.float_col.as_ref().is_some_and(|fc| fc.max_row >= 0));
        // Uma coluna por núcleo: 46 colunas de 13,7 milhões de linhas eram
        // 2,1 s de cópia na thread da tela a cada Ctrl+S (18/10/2026).
        crate::paralelismo::por_coluna(&cols, |col| {
            let (values, bitmap) = self.columns[&col].float_col.as_ref().unwrap().flatten(linhas as usize);
            (col, values, bitmap)
        })
    }

    /// Exporta todas as colunas de texto para gravação no formato colunar `.fshz`.
    pub fn exportar_colunas_txt(&self, linhas: u32) -> Vec<(u32, Vec<i32>, Vec<u8>)> {
        self.colunas_txt().into_iter().filter_map(|col| self.exportar_coluna_txt(col, linhas).map(|(i, b)| (col, i, b))).collect()
    }

    /// As colunas com texto, em ordem.
    pub fn colunas_txt(&self) -> Vec<u32> {
        self.colunas_com(|e| e.text_col.as_ref().is_some_and(|tc| tc.max_row >= 0))
    }

    /// Os índices de texto (no `StringPool`) e a máscara de uma coluna.
    pub fn exportar_coluna_txt(&self, col: u32, linhas: u32) -> Option<(Vec<i32>, Vec<u8>)> {
        let tc = self.columns.get(&col)?.text_col.as_ref().filter(|tc| tc.max_row >= 0)?;
        Some(tc.flatten(linhas as usize))
    }

    fn colunas_com(&self, tem: impl Fn(&ColumnEntry) -> bool) -> Vec<u32> {
        let mut cols: Vec<u32> = self.columns.iter().filter(|(_, e)| tem(e)).map(|(&c, _)| c).collect();
        cols.sort_unstable();
        cols
    }

    /// Carga de arquivo: a coluna inteira de números de uma vez, nos buffers
    /// contíguos que o `.fshz` traz (valores + máscara de presença), sem
    /// passar célula a célula pelo [`Self::set_cell_value`]. A coluna de
    /// texto da mesma posição, se existir, fica como está — uma célula nunca
    /// está nas duas.
    pub fn carregar_coluna_num(&mut self, col: u32, values: &[f64], bitmap: &[u8], linhas: u32) {
        let linhas = (linhas as usize).min(values.len()).min(bitmap.len() * 8);
        if linhas == 0 {
            return;
        }
        let entry = self.get_or_create_column(col);
        let fc = entry.float_col.get_or_insert_with(ChunkedFloatColumn::new);
        fc.load_flat(values, bitmap, linhas);
        let max_row = fc.max_row;
        if max_row >= 0 {
            self.tracked_max_row = self.tracked_max_row.max(max_row as u32);
            self.tracked_max_col = self.tracked_max_col.max(col);
        }
    }

    /// Carga por blocos (CSV): um bloco de números pronto, ver
    /// [`ChunkedColumn::colocar_bloco`]. Devolve as linhas da ponta.
    pub fn carregar_bloco_num(&mut self, col: u32, idx: usize, valores: Box<[f64]>, mascara: Box<[u8]>) -> Option<(u32, u32)> {
        let pontas = self.get_or_create_column(col).float_col.get_or_insert_with(ChunkedFloatColumn::new)
            .colocar_bloco(idx, valores, mascara)?;
        self.tracked_max_row = self.tracked_max_row.max(pontas.1);
        self.tracked_max_col = self.tracked_max_col.max(col);
        Some(pontas)
    }

    /// Carga por blocos (CSV): um bloco de ids de texto **já no pool**.
    pub fn carregar_bloco_txt(&mut self, col: u32, idx: usize, ids: Box<[i32]>, mascara: Box<[u8]>) -> Option<(u32, u32)> {
        let pontas = self.get_or_create_column(col).text_col.get_or_insert_with(ChunkedInt32Column::new)
            .colocar_bloco(idx, ids, mascara)?;
        self.tracked_max_row = self.tracked_max_row.max(pontas.1);
        self.tracked_max_col = self.tracked_max_col.max(col);
        Some(pontas)
    }

    /// Carga de arquivo: a coluna inteira de texto/booleano de uma vez, com
    /// os ids **já no pool da aba** (quem chama traduz o dicionário local do
    /// arquivo uma vez por texto distinto, não uma vez por célula).
    pub fn carregar_coluna_txt(&mut self, col: u32, ids: &[i32], bitmap: &[u8], linhas: u32) {
        let linhas = (linhas as usize).min(ids.len()).min(bitmap.len() * 8);
        if linhas == 0 {
            return;
        }
        let entry = self.get_or_create_column(col);
        let tc = entry.text_col.get_or_insert_with(ChunkedInt32Column::new);
        tc.load_flat(ids, bitmap, linhas);
        let max_row = tc.max_row;
        if max_row >= 0 {
            self.tracked_max_row = self.tracked_max_row.max(max_row as u32);
            self.tracked_max_col = self.tracked_max_col.max(col);
        }
    }

    /// Quantidade de células com dado. Custa uma volta pelas colunas — não é
    /// campo guardado, igual no TypeScript.
    pub fn size(&self) -> u64 {
        let mut count: u64 = 0;
        for entry in self.columns.values() {
            if let Some(fc) = &entry.float_col {
                if fc.max_row >= 0 {
                    count += fc.max_row as u64 + 1;
                }
            }
            if let Some(tc) = &entry.text_col {
                if tc.max_row >= 0 {
                    count += tc.max_row as u64 + 1;
                }
            }
        }
        count
    }

    fn ultima_linha_com_dado(col: &ChunkedFloatColumn) -> Option<u32> {
        if col.max_row < 0 {
            return None;
        }
        let mut r = col.max_row;
        while r >= 0 {
            if col.has(r as u32) {
                return Some(r as u32);
            }
            r -= 1;
        }
        None
    }

    fn ultima_linha_com_dado_i32(col: &ChunkedInt32Column) -> Option<u32> {
        if col.max_row < 0 {
            return None;
        }
        let mut r = col.max_row;
        while r >= 0 {
            if col.has(r as u32) {
                return Some(r as u32);
            }
            r -= 1;
        }
        None
    }

    pub fn max_row(&self) -> u32 {
        let mut r_max: u32 = 0;
        for entry in self.columns.values() {
            if let Some(fc) = &entry.float_col {
                if let Some(r) = Self::ultima_linha_com_dado(fc) {
                    if r > r_max {
                        r_max = r;
                    }
                }
            }
            if let Some(tc) = &entry.text_col {
                if let Some(r) = Self::ultima_linha_com_dado_i32(tc) {
                    if r > r_max {
                        r_max = r;
                    }
                }
            }
        }
        r_max
    }

    pub fn max_col(&self) -> u32 {
        let mut c_max: u32 = 0;
        for (&col, entry) in &self.columns {
            let mut has_data = entry.float_col.as_ref().is_some_and(|fc| Self::ultima_linha_com_dado(fc).is_some());
            if !has_data {
                has_data = entry.text_col.as_ref().is_some_and(|tc| Self::ultima_linha_com_dado_i32(tc).is_some());
            }
            if has_data && col > c_max {
                c_max = col;
            }
        }
        c_max
    }

    /// Texto "VERDADEIRO"/"FALSO" do pool vira booleano; qualquer outro
    /// texto fica como está. Mesma regra do `getCellValue` do TypeScript.
    fn texto_ou_booleano(texto: &str) -> Value {
        match texto {
            "VERDADEIRO" => Value::Bool(true),
            "FALSO" => Value::Bool(false),
            _ => Value::Text(texto.to_string()),
        }
    }

    pub fn get_cell_value(&self, row: u32, col: u32, pool: &StringPool) -> Value {
        let Some(entry) = self.columns.get(&col) else {
            return Value::Empty;
        };
        if let Some(fc) = &entry.float_col {
            if fc.has(row) {
                if let Some(v) = fc.get(row) {
                    return Value::Number(v);
                }
            }
        }
        if let Some(tc) = &entry.text_col {
            if tc.has(row) {
                if let Some(id) = tc.get(row) {
                    if id >= 0 {
                        return Self::texto_ou_booleano(pool.get(id));
                    }
                }
            }
        }
        Value::Empty
    }

    pub fn get_raw(&self, row: u32, col: u32, pool: &StringPool) -> String {
        let Some(entry) = self.columns.get(&col) else {
            return String::new();
        };
        if let Some(fc) = &entry.float_col {
            if fc.has(row) {
                if let Some(v) = fc.get(row) {
                    return to_js_string(v);
                }
            }
        }
        if let Some(tc) = &entry.text_col {
            if tc.has(row) {
                if let Some(id) = tc.get(row) {
                    if id >= 0 {
                        return pool.get(id).to_string();
                    }
                }
            }
        }
        String::new()
    }

    /// `Value::Empty` ou texto vazio limpam a célula — o mesmo `null`,
    /// `undefined` e `''` que o TypeScript trata igual antes de olhar o tipo.
    pub fn set_cell_value(&mut self, row: u32, col: u32, val: &Value, pool: &mut StringPool) {
        let vazio = matches!(val, Value::Empty) || matches!(val, Value::Text(s) if s.is_empty());
        if vazio {
            self.clear_cell(row, col);
            return;
        }

        if row > self.tracked_max_row {
            self.tracked_max_row = row;
        }
        if col > self.tracked_max_col {
            self.tracked_max_col = col;
        }
        let entry = self.get_or_create_column(col);
        match val {
            Value::Number(n) => {
                let fc = entry.float_col.get_or_insert_with(ChunkedFloatColumn::new);
                fc.set(row, *n);
                if let Some(tc) = &mut entry.text_col {
                    tc.clear(row);
                }
            }
            Value::Bool(b) => {
                let texto = if *b { "VERDADEIRO" } else { "FALSO" };
                let id = pool.get_or_create(texto);
                if id >= 0 {
                    let tc = entry.text_col.get_or_insert_with(ChunkedInt32Column::new);
                    tc.set(row, id);
                    if let Some(fc) = &mut entry.float_col {
                        fc.clear(row);
                    }
                }
            }
            Value::Text(s) => {
                let id = pool.get_or_create(s);
                if id >= 0 {
                    let tc = entry.text_col.get_or_insert_with(ChunkedInt32Column::new);
                    tc.set(row, id);
                    if let Some(fc) = &mut entry.float_col {
                        fc.clear(row);
                    }
                }
            }
            // `Value::Error` nunca chega aqui na prática — a DataTable guarda
            // valor de célula, não resultado de fórmula — mas, se chegar, o
            // TypeScript também não faz nada (nenhum `typeof` bate) e a
            // coluna fica criada vazia. Replicado aqui por fidelidade.
            Value::Empty | Value::Error(_) => {}
        }
    }

    /// O valor cru de uma célula, sem montar texto: o número, ou o id do
    /// texto no pool. Mesma ordem do [`Self::get_cell_value`]. Serve quem lê
    /// milhões de células e pode guardar o trabalho por texto distinto (o
    /// exportador de `.xlsx`).
    pub fn cru_em(&self, row: u32, col: u32) -> Option<Cru> {
        let entry = self.columns.get(&col)?;
        if let Some(fc) = &entry.float_col {
            if fc.has(row) {
                if let Some(v) = fc.get(row) {
                    return Some(Cru::Numero(v));
                }
            }
        }
        let tc = entry.text_col.as_ref()?;
        if !tc.has(row) {
            return None;
        }
        tc.get(row).filter(|&id| id >= 0).map(Cru::Texto)
    }

    pub fn has_cell(&self, row: u32, col: u32) -> bool {
        let Some(entry) = self.columns.get(&col) else {
            return false;
        };
        entry.float_col.as_ref().is_some_and(|c| c.has(row))
            || entry.text_col.as_ref().is_some_and(|c| c.has(row))
    }

    /// As máscaras de presença (número e texto) do bloco `idx` da coluna —
    /// ver [`crate::planilha::column_store::ChunkedColumn::mascara_do_bloco`].
    pub fn mascaras_do_bloco(&self, col: u32, idx: usize) -> [Option<&[u8]>; 2] {
        let Some(entry) = self.columns.get(&col) else { return [None, None] };
        [entry.float_col.as_ref().and_then(|c| c.mascara_do_bloco(idx)),
         entry.text_col.as_ref().and_then(|c| c.mascara_do_bloco(idx))]
    }

    pub fn clear_cell(&mut self, row: u32, col: u32) {
        let Some(entry) = self.columns.get_mut(&col) else {
            return;
        };
        if let Some(fc) = &mut entry.float_col {
            fc.clear(row);
        }
        if let Some(tc) = &mut entry.text_col {
            tc.clear(row);
        }
    }

    /// Permuta as linhas `first..first + ordem.len()` da coluna: a linha
    /// `ordem[i]` vai para `first + i`. Ver [`crate::planilha::column_store::ChunkedColumn::permutar`].
    pub fn permutar_linhas(&mut self, col: u32, first: u32, ordem: &[u32]) {
        let Some(entry) = self.columns.get_mut(&col) else {
            return;
        };
        if let Some(fc) = &mut entry.float_col {
            fc.permutar(first, ordem);
        }
        if let Some(tc) = &mut entry.text_col {
            tc.permutar(first, ordem);
        }
    }

    /// Desloca as colunas para excluir ou inserir, **sem tocar nos dados**.
    ///
    /// ⚠ O armazenamento aqui é `HashMap<coluna, blocos>`: mover a coluna E
    /// para o lugar da D é trocar a chave, não copiar célula nenhuma. Custa
    /// O(colunas ocupadas), que são dezenas.
    ///
    /// Medido em 22/09/2026, com 10 milhões de células: o caminho anterior
    /// (`Sheet::delete_cols` extraía todos os literais para um `Vec`, limpava
    /// a tabela e reinseria um a um) levava 374 ms e fazia a memória do
    /// processo saltar de 277 para 541 MB. É o mesmo erro que o `ops.ts` do
    /// TypeScript cometia, e foi encontrado aqui medindo o motor novo com a
    /// planilha do usuário.
    pub fn shift_columns(&mut self, at: u32, count: u32, removing: bool) {
        if count == 0 {
            return;
        }
        let mut proximo: HashMap<u32, ColumnEntry> = HashMap::with_capacity(self.columns.len());
        for (col, entry) in self.columns.drain() {
            if removing {
                // A coluna excluída vai embora com tudo que tinha dentro.
                if col >= at && col < at + count {
                    continue;
                }
                let destino = if col >= at + count { col - count } else { col };
                proximo.insert(destino, entry);
            } else {
                let destino = if col >= at { col + count } else { col };
                proximo.insert(destino, entry);
            }
        }
        self.columns = proximo;
        self.tracked_max_col = if removing {
            self.tracked_max_col.saturating_sub(count)
        } else {
            self.tracked_max_col.saturating_add(count)
        };
    }

    pub fn clear(&mut self) {
        self.columns.clear();
        self.tracked_max_row = 0;
        self.tracked_max_col = 0;
    }

    /// Visita cada célula com dado dentro do retângulo. Estilo de retorno por
    /// função (`&mut dyn FnMut`) igual ao `EvalContext::for_each_in_range` do
    /// [`crate::formula::evaluator`], em vez do gerador (`function*`) que o TypeScript
    /// usava — Rust não tem geradores prontos sem um crate a mais, e esse
    /// estilo já é o padrão do resto do crate.
    pub fn for_each_in_range(&self, rect: &Rect, pool: &StringPool, visit: &mut dyn FnMut(&Value, u32, u32)) {
        let min_c = rect.c0;
        let max_c = rect.c1.min(self.tracked_max_col);
        if min_c > max_c {
            return;
        }
        for col in min_c..=max_c {
            let Some(entry) = self.columns.get(&col) else {
                continue;
            };
            if let Some(fc) = &entry.float_col {
                if fc.max_row >= rect.r0 as i64 {
                    let start_r = rect.r0.max(fc.min_row);
                    let end_r = rect.r1.min(fc.max_row as u32);
                    if start_r <= end_r {
                        for r in start_r..=end_r {
                            if let Some(v) = fc.get(r) {
                                visit(&Value::Number(v), r, col);
                            }
                        }
                    }
                }
            }
            if let Some(tc) = &entry.text_col {
                if tc.max_row >= rect.r0 as i64 {
                    let start_r = rect.r0.max(tc.min_row);
                    let end_r = rect.r1.min(tc.max_row as u32);
                    if start_r <= end_r {
                        for r in start_r..=end_r {
                            if let Some(id) = tc.get(r) {
                                if id >= 0 {
                                    let val = Self::texto_ou_booleano(pool.get(id));
                                    visit(&val, r, col);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Última linha realmente ocupada de uma coluna; consulta os limites
    /// das colunas compactadas sem percorrer as linhas virtuais.
    pub fn ultima_linha_coluna(&self, col: u32) -> Option<u32> {
        let entry = self.columns.get(&col)?;
        let maximo = entry.float_col.as_ref().map(|c| c.max_row).unwrap_or(-1)
            .max(entry.text_col.as_ref().map(|c| c.max_row).unwrap_or(-1));
        u32::try_from(maximo).ok()
    }

    /// Se a coluna PODE ter algo nas linhas `r0..=r1` (pelos limites de cada
    /// pedaço da coluna — falso só quando com certeza está vazia ali).
    pub fn pode_ter_na_faixa(&self, col: u32, r0: u32, r1: u32) -> bool {
        let Some(entry) = self.columns.get(&col) else { return false };
        let cruza = |min: u32, max: i64| max >= 0 && min <= r1 && max as u32 >= r0;
        entry.float_col.as_ref().is_some_and(|c| cruza(c.min_row, c.max_row as i64))
            || entry.text_col.as_ref().is_some_and(|c| cruza(c.min_row, c.max_row as i64))
    }

    /// Esvazia as linhas `r0..=r1` de uma coluna (o desfazer em lote).
    pub fn limpar_faixa(&mut self, col: u32, r0: u32, r1: u32) {
        let Some(entry) = self.columns.get_mut(&col) else {
            return;
        };
        // Bloco a bloco (era `clear` linha a linha: 100 milhões de linhas).
        if let Some(fc) = &mut entry.float_col { fc.apagar_faixa(r0, r1); }
        if let Some(tc) = &mut entry.text_col { tc.apagar_faixa(r0, r1); }
    }

    /// `r0..=r1` da coluna `col` passam a valer `val` (número, texto ou
    /// booleano), bloco a bloco. `false` para vazio e erro (não vão assim).
    pub fn preencher_faixa(&mut self, col: u32, r0: u32, r1: u32, val: &Value, pool: &mut StringPool) -> bool {
        let id_texto = match val {
            Value::Number(_) => None,
            Value::Bool(b) => Some(pool.get_or_create(if *b { "VERDADEIRO" } else { "FALSO" })),
            Value::Text(t) if !t.is_empty() => Some(pool.get_or_create(t)),
            _ => return false,
        };
        if id_texto.is_some_and(|id| id < 0) { return false; }
        let entry = self.get_or_create_column(col);
        match (val, id_texto) {
            (Value::Number(n), _) => {
                entry.float_col.get_or_insert_with(ChunkedFloatColumn::new).preencher_faixa(r0, r1, *n);
                if let Some(tc) = &mut entry.text_col { tc.apagar_faixa(r0, r1); }
            }
            (_, Some(id)) => {
                entry.text_col.get_or_insert_with(ChunkedInt32Column::new).preencher_faixa(r0, r1, id);
                if let Some(fc) = &mut entry.float_col { fc.apagar_faixa(r0, r1); }
            }
            _ => return false,
        }
        self.tracked_max_row = self.tracked_max_row.max(r1);
        self.tracked_max_col = self.tracked_max_col.max(col);
        true
    }

    /// As colunas que têm alguma coisa guardada.
    pub fn colunas(&self) -> Vec<u32> {
        self.columns.keys().copied().collect()
    }

    /// O [`Self::for_each_cru`] de uma coluna só — é o pedaço que a busca
    /// entrega a cada thread.
    pub fn for_each_cru_da_coluna(&self, col: u32, visit: &mut dyn FnMut(u32, Cru)) {
        let Some(entry) = self.columns.get(&col) else {
            return;
        };
        if let Some(fc) = &entry.float_col {
            if fc.max_row >= 0 {
                for r in fc.min_row..=(fc.max_row as u32) {
                    if let Some(v) = fc.get(r) {
                        visit(r, Cru::Numero(v));
                    }
                }
            }
        }
        if let Some(tc) = &entry.text_col {
            if tc.max_row >= 0 {
                for r in tc.min_row..=(tc.max_row as u32) {
                    if let Some(id) = tc.get(r) {
                        if id >= 0 {
                            visit(r, Cru::Texto(id));
                        }
                    }
                }
            }
        }
    }

    /// Como [`Self::for_each`], sem montar o `Value`: o número cru ou o id do
    /// texto no pool. É o que a busca usa — com o id, cada texto distinto é
    /// conferido uma vez só, e não uma vez por célula.
    pub fn for_each_cru(&self, visit: &mut dyn FnMut(u32, u32, Cru)) {
        for (&col, entry) in &self.columns {
            if let Some(fc) = &entry.float_col {
                if fc.max_row >= 0 {
                    for r in fc.min_row..=(fc.max_row as u32) {
                        if let Some(v) = fc.get(r) {
                            visit(r, col, Cru::Numero(v));
                        }
                    }
                }
            }
            if let Some(tc) = &entry.text_col {
                if tc.max_row >= 0 {
                    for r in tc.min_row..=(tc.max_row as u32) {
                        if let Some(id) = tc.get(r) {
                            if id >= 0 {
                                visit(r, col, Cru::Texto(id));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Visita toda célula com dado da tabela, em qualquer ordem de coluna.
    /// Equivalente ao `entries()` do TypeScript.
    pub fn for_each(&self, pool: &StringPool, visit: &mut dyn FnMut(u32, u32, &Value)) {
        for (&col, entry) in &self.columns {
            if let Some(fc) = &entry.float_col {
                if fc.max_row >= 0 {
                    for r in fc.min_row..=(fc.max_row as u32) {
                        if let Some(v) = fc.get(r) {
                            visit(r, col, &Value::Number(v));
                        }
                    }
                }
            }
            if let Some(tc) = &entry.text_col {
                if tc.max_row >= 0 {
                    for r in tc.min_row..=(tc.max_row as u32) {
                        if let Some(id) = tc.get(r) {
                            if id >= 0 {
                                let val = Self::texto_ou_booleano(pool.get(id));
                                visit(r, col, &val);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn guarda_numero_texto_e_booleano() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(0, 0, &Value::Number(42.0), &mut pool);
        tabela.set_cell_value(1, 0, &Value::Text("olá".to_string()), &mut pool);
        tabela.set_cell_value(2, 0, &Value::Bool(true), &mut pool);

        assert_eq!(tabela.get_cell_value(0, 0, &pool), Value::Number(42.0));
        assert_eq!(tabela.get_cell_value(1, 0, &pool), Value::Text("olá".to_string()));
        assert_eq!(tabela.get_cell_value(2, 0, &pool), Value::Bool(true));
        assert_eq!(tabela.get_cell_value(9, 9, &pool), Value::Empty);
    }

    #[test]
    fn escrever_string_vazia_limpa_a_celula() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(0, 0, &Value::Number(1.0), &mut pool);
        assert!(tabela.has_cell(0, 0));
        tabela.set_cell_value(0, 0, &Value::Text(String::new()), &mut pool);
        assert!(!tabela.has_cell(0, 0));
    }

    #[test]
    fn trocar_numero_por_texto_limpa_a_coluna_numerica() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(5, 2, &Value::Number(1.0), &mut pool);
        tabela.set_cell_value(5, 2, &Value::Text("agora é texto".to_string()), &mut pool);
        assert_eq!(tabela.get_cell_value(5, 2, &pool), Value::Text("agora é texto".to_string()));
        assert_eq!(tabela.get_raw(5, 2, &pool), "agora é texto");
    }

    #[test]
    fn max_row_e_max_col_ignoram_celula_limpa() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(10, 3, &Value::Number(1.0), &mut pool);
        assert_eq!(tabela.max_row(), 10);
        assert_eq!(tabela.max_col(), 3);
        tabela.clear_cell(10, 3);
        assert_eq!(tabela.max_row(), 0);
        assert_eq!(tabela.max_col(), 0);
    }

    #[test]
    fn for_each_in_range_visita_so_o_retangulo() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(0, 0, &Value::Number(1.0), &mut pool);
        tabela.set_cell_value(5, 0, &Value::Number(2.0), &mut pool);
        tabela.set_cell_value(5, 1, &Value::Text("fora".to_string()), &mut pool);

        let rect = Rect { r0: 0, c0: 0, r1: 5, c1: 0, sheet: None };
        let mut vistos = Vec::new();
        tabela.for_each_in_range(&rect, &pool, &mut |val, row, col| {
            vistos.push((row, col, val.clone()));
        });
        vistos.sort_by_key(|(r, c, _)| (*r, *c));
        assert_eq!(vistos, vec![(0, 0, Value::Number(1.0)), (5, 0, Value::Number(2.0))]);
    }

    #[test]
    fn get_raw_de_numero_usa_formatacao_javascript() {
        let mut tabela = DataTable::new();
        let mut pool = StringPool::new();
        tabela.set_cell_value(0, 0, &Value::Number(0.1 + 0.2), &mut pool);
        assert_eq!(tabela.get_raw(0, 0, &pool), "0.30000000000000004");
    }

    #[test]
    fn deslocar_coluna_nao_copia_dado() {
        let mut pool = StringPool::new();
        let mut t = DataTable::new();
        t.set_cell_value(0, 0, &Value::Number(1.0), &mut pool);
        t.set_cell_value(0, 1, &Value::Text("b".into()), &mut pool);
        t.set_cell_value(0, 2, &Value::Number(3.0), &mut pool);
        t.set_cell_value(5, 2, &Value::Number(30.0), &mut pool);

        t.shift_columns(1, 1, true);

        assert_eq!(t.get_cell_value(0, 0, &pool), Value::Number(1.0));
        assert_eq!(
            t.get_cell_value(0, 1, &pool),
            Value::Number(3.0),
            "a coluna C não tomou o lugar da B"
        );
        assert_eq!(
            t.get_cell_value(5, 1, &pool),
            Value::Number(30.0),
            "a coluna inteira tinha de andar, não só a primeira linha"
        );
        assert_eq!(t.get_cell_value(0, 2, &pool), Value::Empty, "sobrou lixo na coluna antiga");
    }

    #[test]
    fn inserir_empurra_para_a_direita() {
        let mut pool = StringPool::new();
        let mut t = DataTable::new();
        t.set_cell_value(0, 0, &Value::Number(1.0), &mut pool);
        t.set_cell_value(0, 1, &Value::Number(2.0), &mut pool);

        t.shift_columns(1, 1, false);

        assert_eq!(t.get_cell_value(0, 0, &pool), Value::Number(1.0));
        assert_eq!(t.get_cell_value(0, 1, &pool), Value::Empty, "a coluna nova nasce vazia");
        assert_eq!(t.get_cell_value(0, 2, &pool), Value::Number(2.0));
    }

    #[test]
    fn deslocar_varias_de_uma_vez() {
        let mut pool = StringPool::new();
        let mut t = DataTable::new();
        for c in 0..5 {
            t.set_cell_value(0, c, &Value::Number(c as f64), &mut pool);
        }
        t.shift_columns(1, 3, true);
        assert_eq!(t.get_cell_value(0, 0, &pool), Value::Number(0.0));
        assert_eq!(t.get_cell_value(0, 1, &pool), Value::Number(4.0));
        assert_eq!(t.get_cell_value(0, 2, &pool), Value::Empty);
    }

    #[test]
    fn armazenamento_esparso_preserva_linha_alta_sem_criar_celulas_intermediarias() {
        let mut pool = StringPool::new();
        let mut tabela = DataTable::new();
        tabela.set_cell_value(999_999, 12, &Value::Text("fim".into()), &mut pool);
        assert_eq!(tabela.get_cell_value(999_999, 12, &pool), Value::Text("fim".into()));
        assert_eq!(tabela.get_cell_value(500_000, 12, &pool), Value::Empty);
        assert_eq!(tabela.get_cell_value(999_999, 11, &pool), Value::Empty);
        assert_eq!(tabela.max_row(), 999_999);
        assert_eq!(tabela.max_col(), 12);
        tabela.clear_cell(999_999, 12);
        assert_eq!(tabela.get_cell_value(999_999, 12, &pool), Value::Empty);
    }
}
