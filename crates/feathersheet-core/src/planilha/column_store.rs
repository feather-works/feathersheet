//! Armazenamento colunar por blocos.
//!
//! Tradução de `src/core/column-store.ts`. Lá existem duas classes coladas
//! uma na outra, `ChunkedFloatColumn` e `ChunkedInt32Column`, trocando só
//! `Float64Array` por `Int32Array` — cópia e cola deliberada, porque
//! TypeScript não tem genérico sobre `TypedArray` de graça. Em Rust isso vira
//! um `ChunkedColumn<T>` só, e os dois nomes do TypeScript continuam
//! existindo como sinônimos (`pub type`) para bater o código lado a lado. Não
//! é uma melhoria de comportamento — os dois produzem exatamente a mesma
//! sequência de bytes; é só o compilador cobrindo os dois tipos de uma vez.
//!
//! Cada bloco tem `CHUNK_SIZE` linhas e só é alocado quando alguma delas tem
//! dado — é o que permite uma coluna com 10 milhões de linhas custar memória
//! só onde tem célula preenchida.

pub const CHUNK_SIZE: usize = 65_536;
pub const CHUNK_SHIFT: u32 = 16;
pub const CHUNK_MASK: u32 = 0xffff;

use std::sync::Arc;

/// Um bloco novo de `CHUNK_SIZE` cópias de `v`.
fn bloco<T: Copy>(v: T, n: usize) -> Arc<[T]> {
    Arc::from(vec![v; n])
}

/// A coluna como estava num instante, sem copiar os dados: os blocos são os
/// mesmos `Arc` da coluna viva. É o que a gravação em segundo plano lê.
#[derive(Clone, Default)]
pub struct RetratoColuna<T> {
    chunks: Vec<Option<Arc<[T]>>>,
    presence: Vec<Option<Arc<[u8]>>>,
}

impl<T: Copy> RetratoColuna<T> {
    pub fn get(&self, row: u32) -> Option<T> {
        let i = (row >> CHUNK_SHIFT) as usize;
        let o = (row & CHUNK_MASK) as usize;
        let mask = self.presence.get(i)?.as_ref()?;
        if mask[o >> 3] & (1 << (o & 7)) == 0 { return None; }
        self.chunks[i].as_ref().map(|c| c[o])
    }

    /// Quantas linhas abaixo de `linhas` têm valor.
    pub fn contar(&self, linhas: u32) -> usize {
        let mut n = 0;
        for (i, m) in self.presence.iter().enumerate() {
            let Some(m) = m else { continue };
            let base = (i as u64) << CHUNK_SHIFT;
            if base >= linhas as u64 { break; }
            let ate = ((linhas as u64 - base).min(CHUNK_SIZE as u64)) as usize;
            n += m[..ate >> 3].iter().map(|b| b.count_ones() as usize).sum::<usize>();
            if ate & 7 != 0 { n += (m[ate >> 3] & ((1u8 << (ate & 7)) - 1)).count_ones() as usize; }
        }
        n
    }
}

/// Uma coluna esparsa de `T`, guardada em blocos de [`CHUNK_SIZE`] linhas.
///
/// `presence` é a máscara de bits (1 bit por linha, menos significativo
/// primeiro, mesma convenção do Arrow) que diz se a posição em `values` é
/// dado de verdade ou só o valor-padrão de `T` deixado por trás.
/// Os blocos são `Arc`: o retrato para gravar ([`Self::retrato`]) só conta
/// referências, e escrever num bloco que está sendo gravado copia só ele
/// (`Arc::make_mut`). Antes a gravação copiava a coluna inteira: 39 colunas de
/// 13,7 milhões de linhas eram 4,3 GB a mais ao salvar (28/10/2026).
pub struct ChunkedColumn<T: Copy + Default> {
    chunks: Vec<Option<Arc<[T]>>>,
    presence: Vec<Option<Arc<[u8]>>>,
    /// `u32::MAX` quando a coluna está vazia — o `Infinity` do TypeScript.
    pub min_row: u32,
    /// `-1` quando a coluna está vazia, como no TypeScript.
    pub max_row: i64,
}

impl<T: Copy + Default> ChunkedColumn<T> {
    pub fn new() -> Self {
        Self { chunks: Vec::new(), presence: Vec::new(), min_row: u32::MAX, max_row: -1 }
    }

    fn ensure_chunk(&mut self, idx: usize) {
        if self.chunks.len() <= idx {
            self.chunks.resize_with(idx + 1, || None);
            self.presence.resize_with(idx + 1, || None);
        }
    }

    pub fn set(&mut self, row: u32, val: T) {
        let chunk_idx = (row >> CHUNK_SHIFT) as usize;
        let offset = (row & CHUNK_MASK) as usize;
        self.ensure_chunk(chunk_idx);
        let chunk = self.chunks[chunk_idx].get_or_insert_with(|| bloco(T::default(), CHUNK_SIZE));
        Arc::make_mut(chunk)[offset] = val;
        let mask = self.presence[chunk_idx].get_or_insert_with(|| bloco(0u8, CHUNK_SIZE >> 3));
        Arc::make_mut(mask)[offset >> 3] |= 1 << (offset & 7);
        if row < self.min_row {
            self.min_row = row;
        }
        if row as i64 > self.max_row {
            self.max_row = row as i64;
        }
    }

    pub fn get(&self, row: u32) -> Option<T> {
        let chunk_idx = (row >> CHUNK_SHIFT) as usize;
        let mask = self.presence.get(chunk_idx)?.as_ref()?;
        let offset = (row & CHUNK_MASK) as usize;
        if mask[offset >> 3] & (1 << (offset & 7)) == 0 {
            return None;
        }
        self.chunks[chunk_idx].as_ref().map(|c| c[offset])
    }

    pub fn has(&self, row: u32) -> bool {
        let chunk_idx = (row >> CHUNK_SHIFT) as usize;
        let Some(Some(mask)) = self.presence.get(chunk_idx) else {
            return false;
        };
        let offset = (row & CHUNK_MASK) as usize;
        mask[offset >> 3] & (1 << (offset & 7)) != 0
    }

    /// A máscara de presença do bloco `idx` (`None` quando o bloco não tem
    /// nenhuma linha). Quem precisa saber onde há dado em milhões de linhas
    /// lê 8 linhas por byte, em vez de perguntar [`Self::has`] uma a uma.
    /// O retrato da coluna agora, sem copiar dados (ver [`RetratoColuna`]).
    pub fn retrato(&self) -> RetratoColuna<T> {
        RetratoColuna { chunks: self.chunks.clone(), presence: self.presence.clone() }
    }

    pub fn mascara_do_bloco(&self, idx: usize) -> Option<&[u8]> {
        self.presence.get(idx)?.as_deref()
    }

    /// Copia a coluna para buffers contíguos — os valores e a máscara de
    /// presença — do índice 0 até `length`. É o formato que o arquivo grava
    /// (equivalente ao que o `Arrow` consumia direto no lado TypeScript).
    pub fn flatten(&self, length: usize) -> (Vec<T>, Vec<u8>) {
        let mut values = vec![T::default(); length];
        let mut bitmap = vec![0u8; (length + 7) >> 3];
        let blocos = self.chunks.len().min((length + CHUNK_MASK as usize) >> CHUNK_SHIFT);
        for i in 0..blocos {
            let (Some(chunk), Some(mask)) = (&self.chunks[i], &self.presence[i]) else {
                continue;
            };
            let inicio = i << CHUNK_SHIFT;
            if inicio >= length {
                break;
            }
            let quantos = CHUNK_SIZE.min(length - inicio);
            values[inicio..inicio + quantos].copy_from_slice(&chunk[..quantos]);
            // A máscara copia byte a byte porque o bloco começa sempre em
            // múltiplo de 65.536, que é múltiplo de 8 — os bits nunca ficam
            // desalinhados.
            let bytes = (quantos + 7) >> 3;
            bitmap[(inicio >> 3)..(inicio >> 3) + bytes].copy_from_slice(&mask[..bytes]);
        }
        (values, bitmap)
    }

    /// Preenche a coluna a partir dos buffers contíguos que o [`Self::flatten`] produz.
    pub fn load_flat(&mut self, values: &[T], bitmap: &[u8], length: usize) {
        self.chunks.clear();
        self.presence.clear();
        self.min_row = u32::MAX;
        self.max_row = -1;
        let blocos = (length + CHUNK_MASK as usize) >> CHUNK_SHIFT;
        self.chunks.resize_with(blocos, || None);
        self.presence.resize_with(blocos, || None);
        for i in 0..blocos {
            let inicio = i << CHUNK_SHIFT;
            let quantos = CHUNK_SIZE.min(length - inicio);
            let mut mask = vec![0u8; CHUNK_SIZE >> 3];
            let bytes = (quantos + 7) >> 3;
            let origem = inicio >> 3;
            mask[..bytes].copy_from_slice(&bitmap[origem..origem + bytes]);
            if mask.iter().all(|&b| b == 0) {
                continue;
            }
            let mut chunk = vec![T::default(); CHUNK_SIZE];
            chunk[..quantos].copy_from_slice(&values[inicio..inicio + quantos]);
            self.chunks[i] = Some(Arc::from(chunk));
            self.presence[i] = Some(Arc::from(mask));
        }
        for r in 0..length as u32 {
            if self.has(r) {
                self.min_row = r;
                break;
            }
        }
        for r in (0..length as u32).rev() {
            if self.has(r) {
                self.max_row = r as i64;
                break;
            }
        }
    }

    /// Carga de arquivo por blocos: põe um bloco pronto (`CHUNK_SIZE` valores
    /// e a máscara de `CHUNK_SIZE / 8` bytes) na posição `idx`, sem copiar.
    /// O bloco tem de estar vazio — é para a aba que acabou de nascer.
    /// Devolve a primeira e a última linha presentes, se houver alguma.
    pub fn colocar_bloco(&mut self, idx: usize, valores: Box<[T]>, mascara: Box<[u8]>) -> Option<(u32, u32)> {
        debug_assert_eq!(valores.len(), CHUNK_SIZE);
        debug_assert_eq!(mascara.len(), CHUNK_SIZE >> 3);
        let primeiro = mascara.iter().position(|&b| b != 0)?;
        let ultimo = mascara.iter().rposition(|&b| b != 0)?;
        let base = (idx << CHUNK_SHIFT) as u32;
        let primeira = base + (primeiro * 8) as u32 + mascara[primeiro].trailing_zeros();
        let ultima = base + (ultimo * 8) as u32 + 7 - mascara[ultimo].leading_zeros();
        self.ensure_chunk(idx);
        self.chunks[idx] = Some(Arc::from(valores));
        self.presence[idx] = Some(Arc::from(mascara));
        self.min_row = self.min_row.min(primeira);
        self.max_row = self.max_row.max(ultima as i64);
        Some((primeira, ultima))
    }

    /// Permuta as linhas `first..first + ordem.len()`: a linha `ordem[i]` vai
    /// para `first + i` (é o ordenar). `ordem` tem de ser uma permutação desse
    /// mesmo trecho. Mexe direto nos blocos e acerta os limites uma vez no
    /// fim — pelo [`Self::clear`], que procura a nova última linha a cada
    /// chamada, dois milhões de linhas virariam uma conta quadrática.
    pub fn permutar(&mut self, first: u32, ordem: &[u32]) {
        let Some(last) = (first as u64 + ordem.len() as u64).checked_sub(1) else {
            return;
        };
        if self.max_row < first as i64 || (self.min_row as u64) > last {
            return;
        }
        let velhos: Vec<Option<T>> = ordem.iter().map(|&r| self.get(r)).collect();
        for (i, v) in velhos.into_iter().enumerate() {
            let row = first + i as u32;
            match v {
                Some(x) => self.set(row, x),
                None => self.apagar_sem_limites(row),
            }
        }
        self.acertar_limites();
    }

    fn apagar_sem_limites(&mut self, row: u32) {
        let chunk_idx = (row >> CHUNK_SHIFT) as usize;
        let Some(Some(mask)) = self.presence.get_mut(chunk_idx) else {
            return;
        };
        let offset = (row & CHUNK_MASK) as usize;
        Arc::make_mut(mask)[offset >> 3] &= !(1 << (offset & 7));
        if let Some(Some(chunk)) = self.chunks.get_mut(chunk_idx) {
            Arc::make_mut(chunk)[offset] = T::default();
        }
    }

    /// Encontra a primeira/última linha presente entre `de` e `ate`.
    /// Percorre bytes do bitmap (8 linhas por byte), pulando blocos ausentes,
    /// em vez de chamar `has` milhões de vezes na exclusão do último valor.
    fn procurar_presente(&self, de: u32, ate: u32, inverso: bool) -> Option<u32> {
        if de > ate { return None; }
        let primeiro_bloco = (de >> CHUNK_SHIFT) as usize;
        let ultimo_bloco = (ate >> CHUNK_SHIFT) as usize;
        let mut bloco = if inverso { ultimo_bloco } else { primeiro_bloco };
        loop {
            if let Some(Some(mascara)) = self.presence.get(bloco) {
                let inicio = if bloco == primeiro_bloco {
                    (de & CHUNK_MASK) as usize
                } else { 0 };
                let fim = if bloco == ultimo_bloco {
                    (ate & CHUNK_MASK) as usize
                } else { CHUNK_SIZE - 1 };
                let primeiro_byte = inicio >> 3;
                let ultimo_byte = fim >> 3;
                let mut byte = if inverso { ultimo_byte } else { primeiro_byte };
                loop {
                    let mut bits = mascara[byte];
                    if byte == primeiro_byte {
                        bits &= (0xffu16 << (inicio & 7)) as u8;
                    }
                    if byte == ultimo_byte {
                        bits &= (0xffu16 >> (7 - (fim & 7))) as u8;
                    }
                    if bits != 0 {
                        let bit = if inverso {
                            7 - bits.leading_zeros()
                        } else {
                            bits.trailing_zeros()
                        } as usize;
                        return Some(((bloco << CHUNK_SHIFT) + (byte << 3) + bit) as u32);
                    }
                    let borda = if inverso { primeiro_byte } else { ultimo_byte };
                    if byte == borda { break; }
                    if inverso { byte -= 1; } else { byte += 1; }
                }
            }
            let ultimo = if inverso { primeiro_bloco } else { ultimo_bloco };
            if bloco == ultimo { break; }
            if inverso { bloco -= 1; } else { bloco += 1; }
        }
        None
    }

    /// Atualiza os limites sem varrer todas as linhas entre eles.
    fn acertar_limites(&mut self) {
        if self.max_row < 0 { return; }
        let (de, ate) = (self.min_row, self.max_row as u32);
        match self.procurar_presente(de, ate, false) {
            None => {
                self.min_row = u32::MAX;
                self.max_row = -1;
            }
            Some(min) => {
                self.min_row = min;
                self.max_row = self.procurar_presente(min, ate, true)
                    .unwrap_or(min) as i64;
            }
        }
    }

    /// Apaga o valor e atualiza os limites se alguma extremidade foi removida.
    /// Antes, remover somente a primeira linha deixava `min_row` incorreto.
    /// `r0..=r1` passam a valer `val`. Bloco coberto inteiro nasce pronto
    /// (sem passar linha a linha): é o colar de um valor em 100 milhões de
    /// linhas (17/10/2026). As pontas vão linha a linha.
    pub fn preencher_faixa(&mut self, r0: u32, r1: u32, val: T) {
        if r0 > r1 { return; }
        let (b0, b1) = ((r0 >> CHUNK_SHIFT) as usize, (r1 >> CHUNK_SHIFT) as usize);
        self.ensure_chunk(b1);
        for b in b0..=b1 {
            let base = (b as u32) << CHUNK_SHIFT;
            let (de, ate) = ((r0.max(base) - base) as usize, (r1.min(base + CHUNK_MASK) - base) as usize);
            if de == 0 && ate == CHUNK_MASK as usize {
                self.chunks[b] = Some(bloco(val, CHUNK_SIZE));
                self.presence[b] = Some(bloco(0xff, CHUNK_SIZE >> 3));
                continue;
            }
            Arc::make_mut(self.chunks[b].get_or_insert_with(|| bloco(T::default(), CHUNK_SIZE)))[de..=ate].fill(val);
            let mask = Arc::make_mut(self.presence[b].get_or_insert_with(|| bloco(0u8, CHUNK_SIZE >> 3)));
            for o in de..=ate { mask[o >> 3] |= 1 << (o & 7); }
        }
        self.min_row = self.min_row.min(r0);
        self.max_row = self.max_row.max(r1 as i64);
    }

    /// Esvazia `r0..=r1` bloco a bloco e acerta os limites uma vez só. O
    /// [`Self::clear`] linha a linha podia procurar a nova ponta a cada
    /// chamada: desfazer o colar de 100 milhões de linhas não acabava.
    pub fn apagar_faixa(&mut self, r0: u32, r1: u32) {
        if r0 > r1 || self.max_row < r0 as i64 || self.min_row > r1 { return; }
        let r1 = r1.min(self.max_row as u32);
        let (b0, b1) = ((r0 >> CHUNK_SHIFT) as usize, (r1 >> CHUNK_SHIFT) as usize);
        for b in b0..=b1.min(self.chunks.len().saturating_sub(1)) {
            let base = (b as u32) << CHUNK_SHIFT;
            let (de, ate) = ((r0.max(base) - base) as usize, (r1.min(base + CHUNK_MASK) - base) as usize);
            if de == 0 && ate == CHUNK_MASK as usize {
                self.chunks[b] = None;
                self.presence[b] = None;
                continue;
            }
            if let Some(mask) = self.presence[b].as_mut().map(Arc::make_mut) {
                for o in de..=ate { mask[o >> 3] &= !(1 << (o & 7)); }
            }
            if let Some(chunk) = self.chunks[b].as_mut().map(Arc::make_mut) {
                chunk[de..=ate].fill(T::default());
            }
        }
        self.acertar_limites();
    }

    pub fn clear(&mut self, row: u32) {
        if !self.has(row) { return; }
        self.apagar_sem_limites(row);
        if row == self.min_row || row as i64 == self.max_row {
            self.acertar_limites();
        }
    }

}

impl<T: Copy + Default> Default for ChunkedColumn<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub type ChunkedFloatColumn = ChunkedColumn<f64>;
pub type ChunkedInt32Column = ChunkedColumn<i32>;

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn guarda_e_le() {
        let mut col: ChunkedFloatColumn = ChunkedColumn::new();
        assert!(!col.has(5));
        col.set(5, 3.5);
        assert!(col.has(5));
        assert_eq!(col.get(5), Some(3.5));
        assert_eq!(col.min_row, 5);
        assert_eq!(col.max_row, 5);
    }

    #[test]
    fn atravessa_blocos() {
        let mut col: ChunkedInt32Column = ChunkedColumn::new();
        col.set(10, 1);
        col.set(CHUNK_SIZE as u32 + 20, 2);
        assert_eq!(col.get(10), Some(1));
        assert_eq!(col.get(CHUNK_SIZE as u32 + 20), Some(2));
        assert_eq!(col.min_row, 10);
        assert_eq!(col.max_row, CHUNK_SIZE as i64 + 20);
    }

    #[test]
    fn limpar_recalcula_max_row() {
        let mut col: ChunkedFloatColumn = ChunkedColumn::new();
        col.set(3, 1.0);
        col.set(7, 2.0);
        col.set(9, 3.0);
        col.clear(9);
        assert_eq!(col.max_row, 7);
        assert!(!col.has(9));
        col.clear(7);
        assert_eq!(col.max_row, 3);
        col.clear(3);
        assert_eq!(col.max_row, -1);
        assert_eq!(col.min_row, u32::MAX);
    }

    #[test]
    fn limpar_uma_unica_celula() {
        let mut col: ChunkedFloatColumn = ChunkedColumn::new();
        col.set(42, 9.0);
        col.clear(42);
        assert_eq!(col.max_row, -1);
        assert_eq!(col.min_row, u32::MAX);
        assert!(!col.has(42));
    }

    #[test]
    fn remover_primeira_linha_atualiza_minimo() {
        let mut col: ChunkedFloatColumn = ChunkedColumn::new();
        col.set(2, 10.0);
        col.set(7, 20.0);
        col.set(9, 30.0);
        col.clear(2);
        assert_eq!(col.min_row, 7);
        assert_eq!(col.max_row, 9);
        assert_eq!(col.get(7), Some(20.0));
        col.clear(2); // idempotente: não altera extremos.
        assert_eq!(col.min_row, 7);
        col.clear(7);
        assert_eq!(col.min_row, 9);
        col.clear(9);
        assert_eq!(col.min_row, u32::MAX);
        assert_eq!(col.max_row, -1);
    }

    #[test]
    fn procurar_presente_pula_blocos_e_respeita_bits_de_borda() {
        let mut col: ChunkedInt32Column = ChunkedColumn::new();
        let distante = CHUNK_SIZE as u32 * 100 + 3;
        col.set(7, 1);
        col.set(8, 2);
        col.set(CHUNK_SIZE as u32 - 1, 3);
        col.set(distante, 4);
        assert_eq!(col.procurar_presente(8, distante, false), Some(8));
        assert_eq!(col.procurar_presente(9, distante, false),
            Some(CHUNK_SIZE as u32 - 1));
        assert_eq!(col.procurar_presente(8, distante - 1, true),
            Some(CHUNK_SIZE as u32 - 1));
        assert_eq!(col.procurar_presente(CHUNK_SIZE as u32, distante - 1, false), None);
        col.clear(7);
        col.clear(8);
        col.clear(CHUNK_SIZE as u32 - 1);
        assert_eq!(col.min_row, distante);
        assert_eq!(col.max_row, distante as i64);
        col.clear(distante);
        assert_eq!(col.min_row, u32::MAX);
        assert_eq!(col.max_row, -1);
    }

    #[test]
    fn flatten_e_load_flat_ida_e_volta() {
        let mut col: ChunkedFloatColumn = ChunkedColumn::new();
        col.set(0, 1.0);
        col.set(100, 2.0);
        col.set(CHUNK_SIZE as u32 + 5, 3.0);
        let length = CHUNK_SIZE + 10;
        let (values, bitmap) = col.flatten(length);

        let mut outra: ChunkedFloatColumn = ChunkedColumn::new();
        outra.load_flat(&values, &bitmap, length);
        assert_eq!(outra.get(0), Some(1.0));
        assert_eq!(outra.get(100), Some(2.0));
        assert_eq!(outra.get(CHUNK_SIZE as u32 + 5), Some(3.0));
        assert!(!outra.has(1));
        assert_eq!(outra.min_row, 0);
        assert_eq!(outra.max_row, CHUNK_SIZE as i64 + 5);
    }

    #[test]
    fn coluna_vazia_nao_atravanca_flatten() {
        let col: ChunkedInt32Column = ChunkedColumn::new();
        let (values, bitmap) = col.flatten(100);
        assert_eq!(values.len(), 100);
        assert!(bitmap.iter().all(|&b| b == 0));
    }
}
