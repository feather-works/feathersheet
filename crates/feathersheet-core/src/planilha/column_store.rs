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

/// Um bloco novo de `CHUNK_SIZE` cópias de `v`. `Arc<Vec>` e não `Arc<[T]>`: o
/// `Arc::from(vec)` copiava o bloco e encostava em todas as páginas, enquanto o
/// `vec![0; n]` sozinho vem zerado do sistema e só ocupa RAM onde se escreve.
/// Uma coluna com 20 números passou a custar 512 KB em vez de 4 KB (29/10/2026).
fn bloco<T: Copy>(v: T, n: usize) -> Arc<Vec<T>> {
    Arc::new(vec![v; n])
}

/// Os valores de um bloco: um `T` por linha, ou compacto ([`Compacto`]: 1, 2 ou 4
/// bytes por linha). Só os blocos que voltam iguais, bit a bit, são compactados.
#[derive(Clone)]
enum Bloco<T> {
    Cru(Arc<Vec<T>>),
    Compacto(Arc<Compacto>),
}

impl<T: Valor> Bloco<T> {
    fn get(&self, o: usize) -> T {
        match self {
            Bloco::Cru(v) => v[o],
            Bloco::Compacto(c) => T::ler(c, o),
        }
    }

    /// Os valores para escrever: o compacto volta a cru (só este bloco) antes.
    fn cru_mut(&mut self) -> &mut Vec<T> {
        if let Bloco::Compacto(c) = self {
            *self = Bloco::Cru(Arc::new((0..CHUNK_SIZE).map(|o| T::ler(c, o)).collect()));
        }
        match self {
            Bloco::Cru(v) => Arc::make_mut(v),
            Bloco::Compacto(_) => unreachable!("acabou de virar cru"),
        }
    }

    /// Os valores para escrever até a posição `ate`. O bloco cru cresce conforme o uso
    /// (64, 128… até `CHUNK_SIZE`): coluna com 30 células não reserva 65.536 linhas. Um
    /// relatório de 9 abas e ~200 colunas curtas gastava ~70 MB só nisso (29/10/2026).
    /// Toda linha presente fica dentro do tamanho, e quem lê só lê linha presente.
    fn ate(&mut self, ate: usize) -> &mut Vec<T> {
        let v = self.cru_mut();
        if v.len() <= ate {
            v.resize((ate + 1).next_power_of_two().clamp(64, CHUNK_SIZE), T::default());
        }
        v
    }
}

/// Um bloco guardado como inteiros pequenos: o valor da linha `o` é
/// `(base + dados[o]) / escala`, com `dados[o]` de `largura` bytes. Uma coluna de
/// quilômetros em passos de 0,25 cabe em 2 bytes por linha, em vez de 8.
pub struct Compacto {
    base: i64,
    escala: f64,
    largura: u8,
    dados: Box<[u8]>,
}

impl Compacto {
    fn inteiro(&self, o: usize) -> i64 {
        let d = &self.dados;
        self.base + match self.largura {
            1 => i64::from(d[o]),
            2 => i64::from(u16::from_le_bytes([d[2 * o], d[2 * o + 1]])),
            _ => i64::from(u32::from_le_bytes([d[4 * o], d[4 * o + 1], d[4 * o + 2], d[4 * o + 3]])),
        }
    }

    /// Monta o bloco com os inteiros `valor(o)` das linhas presentes, entre `min` e
    /// `max`; `None` se a distância passa de 4 bytes.
    fn montar(min: i64, max: i64, escala: f64, mascara: &[u8], valor: impl Fn(usize) -> i64) -> Option<Self> {
        let distancia = max.checked_sub(min)?;
        let largura: u8 = if distancia <= 0xff { 1 } else if distancia <= 0xffff { 2 } else if distancia <= 0xffff_ffff { 4 } else { return None };
        let w = largura as usize;
        let mut dados = vec![0u8; CHUNK_SIZE * w].into_boxed_slice();
        for o in presentes(mascara) {
            let d = (valor(o) - min) as u32;
            dados[o * w..o * w + w].copy_from_slice(&d.to_le_bytes()[..w]);
        }
        Some(Compacto { base: min, escala, largura, dados })
    }
}

/// As posições com o bit ligado na máscara.
fn presentes(mascara: &[u8]) -> impl Iterator<Item = usize> + '_ {
    mascara.iter().enumerate().filter(|(_, &b)| b != 0)
        .flat_map(|(i, &b)| (0..8).filter(move |k| b & (1 << k) != 0).map(move |k| i * 8 + k))
}

/// O que pode ir numa coluna e como vira inteiro sem perda.
pub trait Valor: Copy + Default + Send + Sync {
    /// O bloco compacto, ou `None` se algum valor presente não volta igual.
    fn compactar(valores: &[Self], mascara: &[u8]) -> Option<Compacto>;
    fn ler(c: &Compacto, o: usize) -> Self;
}

impl Valor for f64 {
    fn compactar(valores: &[f64], mascara: &[u8]) -> Option<Compacto> {
        // Inteiros, meios, quartos e até 5 casas decimais: a primeira escala em que todo
        // valor volta igual (`k / escala` tem os mesmos bits) é a de menor distância.
        'escala: for escala in [1.0, 2.0, 4.0, 10.0, 100.0, 1_000.0, 10_000.0, 100_000.0] {
            let (mut min, mut max) = (i64::MAX, i64::MIN);
            for o in presentes(mascara) {
                let v = valores[o];
                let k = (v * escala).round();
                if !(k.abs() < 9.0e15) { continue 'escala; }
                // Confere pelo caminho da leitura (inteiro → f64 / escala): o -0,0 volta +0,0.
                let k = k as i64;
                if (k as f64 / escala).to_bits() != v.to_bits() { continue 'escala; }
                min = min.min(k);
                max = max.max(k);
            }
            if min > max { return None; }
            return Compacto::montar(min, max, escala, mascara, |o| (valores[o] * escala).round() as i64);
        }
        None
    }

    fn ler(c: &Compacto, o: usize) -> f64 {
        // Dividir por 1 dá o mesmo número: os inteiros pulam a divisão, que é o que pesa.
        if c.escala == 1.0 { c.inteiro(o) as f64 } else { c.inteiro(o) as f64 / c.escala }
    }
}

impl Valor for i32 {
    fn compactar(valores: &[i32], mascara: &[u8]) -> Option<Compacto> {
        let (min, max) = presentes(mascara).map(|o| i64::from(valores[o]))
            .fold((i64::MAX, i64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        if min > max { return None; }
        Compacto::montar(min, max, 1.0, mascara, |o| i64::from(valores[o]))
    }

    fn ler(c: &Compacto, o: usize) -> i32 {
        c.inteiro(o) as i32
    }
}

/// A coluna como estava num instante, sem copiar os dados: os blocos são os
/// mesmos `Arc` da coluna viva. É o que a gravação em segundo plano lê.
#[derive(Clone)]
pub struct RetratoColuna<T> {
    chunks: Vec<Option<Bloco<T>>>,
    presence: Vec<Option<Arc<Vec<u8>>>>,
}

impl<T> Default for RetratoColuna<T> {
    fn default() -> Self {
        Self { chunks: Vec::new(), presence: Vec::new() }
    }
}

impl<T: Valor> RetratoColuna<T> {
    pub fn get(&self, row: u32) -> Option<T> {
        let i = (row >> CHUNK_SHIFT) as usize;
        let o = (row & CHUNK_MASK) as usize;
        let mask = self.presence.get(i)?.as_ref()?;
        if mask[o >> 3] & (1 << (o & 7)) == 0 { return None; }
        self.chunks[i].as_ref().map(|c| c.get(o))
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
pub struct ChunkedColumn<T: Valor> {
    chunks: Vec<Option<Bloco<T>>>,
    presence: Vec<Option<Arc<Vec<u8>>>>,
    /// `u32::MAX` quando a coluna está vazia — o `Infinity` do TypeScript.
    pub min_row: u32,
    /// `-1` quando a coluna está vazia, como no TypeScript.
    pub max_row: i64,
}

impl<T: Valor> ChunkedColumn<T> {
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
        let chunk = self.chunks[chunk_idx].get_or_insert_with(|| Bloco::Cru(Arc::default()));
        chunk.ate(offset)[offset] = val;
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
        self.chunks[chunk_idx].as_ref().map(|c| c.get(offset))
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
        self.presence.get(idx)?.as_deref().map(Vec::as_slice)
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
            match chunk {
                Bloco::Cru(v) => {
                    let n = quantos.min(v.len());
                    values[inicio..inicio + n].copy_from_slice(&v[..n]);
                }
                Bloco::Compacto(c) => {
                    for (o, v) in values[inicio..inicio + quantos].iter_mut().enumerate() { *v = T::ler(c, o); }
                }
            }
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
            self.chunks[i] = Some(Bloco::Cru(Arc::new(chunk)));
            self.presence[i] = Some(Arc::new(mask));
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
        self.chunks[idx] = Some(Bloco::Cru(Arc::new(valores.into_vec())));
        self.presence[idx] = Some(Arc::new(mascara.into_vec()));
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
        // O compacto fica como está: sem o bit, o valor dali não é lido.
        if let Some(Some(Bloco::Cru(chunk))) = self.chunks.get_mut(chunk_idx) {
            if let Some(v) = Arc::make_mut(chunk).get_mut(offset) { *v = T::default(); }
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
                self.chunks[b] = Some(Bloco::Cru(bloco(val, CHUNK_SIZE)));
                self.presence[b] = Some(bloco(0xff, CHUNK_SIZE >> 3));
                continue;
            }
            self.chunks[b].get_or_insert_with(|| Bloco::Cru(Arc::default())).ate(ate)[de..=ate].fill(val);
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
            if let Some(Bloco::Cru(chunk)) = self.chunks[b].as_mut() {
                let v = Arc::make_mut(chunk);
                let fim = (ate + 1).min(v.len());
                if de < fim { v[de..fim].fill(T::default()); }
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

    /// Compacta os blocos que cabem em menos bytes sem perder nada (ver [`Compacto`]).
    /// Bloco com pouco dado fica cru: o `vec![0; n]` só ocupa as páginas escritas, e o
    /// compacto ocuparia todas. Devolve os bytes de valores antes e depois.
    pub fn compactar(&mut self) -> (usize, usize) {
        let (mut antes, mut depois) = (0, 0);
        for (bloco, mascara) in self.chunks.iter_mut().zip(&self.presence) {
            let (Some(Bloco::Cru(v)), Some(m)) = (bloco.as_ref(), mascara) else { continue };
            let n: usize = m.iter().map(|b| b.count_ones() as usize).sum();
            if n < CHUNK_SIZE / 8 { continue; }
            let cru = v.len() * std::mem::size_of::<T>();
            if let Some(c) = T::compactar(v, m).filter(|c| c.dados.len() < cru) {
                antes += cru;
                depois += c.dados.len();
                *bloco = Some(Bloco::Compacto(Arc::new(c)));
            }
        }
        (antes, depois)
    }

}

impl<T: Valor> Default for ChunkedColumn<T> {
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

    /// Compactar não muda nenhum valor (bit a bit), escolhe a menor largura, deixa
    /// cru o bloco que perderia, e escrever depois descompacta só o bloco mexido.
    #[test]
    fn compactar_e_sem_perda_e_escrever_depois_funciona() {
        let n = 3 * CHUNK_SIZE as u32;
        let mut c = ChunkedFloatColumn::new();
        for r in 0..n {
            let v = match r >> CHUNK_SHIFT {
                0 => 4887.75 + 0.25 * f64::from(r % 4000),   // quartos: 2 bytes
                1 => f64::from(r % 200) - 100.0,             // inteiros pequenos: 1 byte
                _ => 1.0 / f64::from(r + 3),                 // dízimas: fica cru
            };
            if r % 7 != 3 { c.set(r, v); }
        }
        let antes: Vec<Option<u64>> = (0..n).map(|r| c.get(r).map(f64::to_bits)).collect();
        let retrato = c.retrato();
        let (bytes_antes, bytes_depois) = c.compactar();
        assert_eq!(bytes_antes, 2 * CHUNK_SIZE * 8, "os dois primeiros blocos compactam");
        assert_eq!(bytes_depois, CHUNK_SIZE * 2 + CHUNK_SIZE);
        let depois: Vec<Option<u64>> = (0..n).map(|r| c.get(r).map(f64::to_bits)).collect();
        assert!(antes == depois, "algum valor mudou ao compactar");
        assert!((0..n).all(|r| retrato.get(r).map(f64::to_bits) == antes[r as usize]), "o retrato de antes continua igual");
        let (planos, mascara) = c.flatten(n as usize);
        assert!((0..n as usize).all(|r| (mascara[r >> 3] & (1 << (r & 7)) != 0).then(|| planos[r].to_bits()) == antes[r]));
        c.set(10, 0.1);
        c.clear(11);
        assert_eq!(c.get(10), Some(0.1));
        assert_eq!(c.get(11), None);
        assert_eq!(c.get(12).map(f64::to_bits), antes[12]);
        assert_eq!(c.get(CHUNK_SIZE as u32 + 5).map(f64::to_bits), antes[CHUNK_SIZE + 5], "o outro bloco segue compacto");
        let mut t = ChunkedInt32Column::new();
        for r in 0..CHUNK_SIZE as u32 { t.set(r, 70_000 + (r % 300) as i32); }
        assert_eq!(t.compactar(), (CHUNK_SIZE * 4, CHUNK_SIZE * 2));
        assert!((0..CHUNK_SIZE as u32).all(|r| t.get(r) == Some(70_000 + (r % 300) as i32)));
        assert_eq!(f64::compactar(&[-0.0; CHUNK_SIZE], &[0xff; CHUNK_SIZE >> 3]).map(|_| ()), None, "-0 não volta igual: fica cru");
    }

    /// O bloco cresce com o uso: coluna curta não reserva 65.536 linhas, e escrever,
    /// apagar e preencher além do tamanho atual continua certo.
    #[test]
    fn bloco_cresce_conforme_o_uso() {
        let mut c = ChunkedFloatColumn::new();
        c.set(3, 1.5);
        assert!(matches!(&c.chunks[0], Some(Bloco::Cru(v)) if v.len() == 64));
        c.set(1000, 2.5);
        assert!(matches!(&c.chunks[0], Some(Bloco::Cru(v)) if v.len() == 1024));
        c.clear(40_000);
        c.apagar_faixa(2_000, 50_000);
        c.preencher_faixa(5_000, 5_002, 7.0);
        assert_eq!((c.get(3), c.get(1000), c.get(5_001), c.get(4_999)), (Some(1.5), Some(2.5), Some(7.0), None));
        let (v, m) = c.flatten(6_000);
        assert_eq!((v[3], v[1000], v[5_002], m[5_002 >> 3] & (1 << (5_002 & 7)) != 0), (1.5, 2.5, 7.0, true));
        c.apagar_faixa(0, 4_000);
        assert_eq!((c.get(3), c.get(5_000)), (None, Some(7.0)));
    }
}
