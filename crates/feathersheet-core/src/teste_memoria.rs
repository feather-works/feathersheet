//! MEM-1: alocador dos testes (do motor e, pelo `#[path]`, do `feathersheet-io`) que conta a memória por thread e recusa o pedido
//! que passa do teto. Um teste que estoura falha em milissegundos, com o rótulo e o tamanho
//! do pedido no stderr, em vez de encher a RAM da máquina.
//!
//! ```ignore
//! let teto = crate::teste_memoria::Teto::novo("colar_coluna_inteira", 64 << 20);
//! /* ...o trabalho... */
//! assert!(teto.pico() < 32 << 20);
//! ```
//!
//! shortcut: conta só a thread do teste (o `cargo test` roda testes em paralelo, e um contador
//! global misturaria todos); trabalho mandado ao rayon fica de fora. Se um teste de teto
//! precisar do paralelo, rodar com `RAYON_NUM_THREADS=1` ou contar as threads do pool.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Contador;

thread_local! {
    static EM_USO: Cell<usize> = const { Cell::new(0) };
    static PICO: Cell<usize> = const { Cell::new(0) };
    static TETO: Cell<usize> = const { Cell::new(usize::MAX) };
    static ROTULO: Cell<&'static str> = const { Cell::new("") };
}

/// O pedido cabe? `em_uso + pedido <= teto`, sem estourar a soma.
fn cabe(em_uso: usize, pedido: usize, teto: usize) -> bool {
    em_uso.checked_add(pedido).is_some_and(|total| total <= teto)
}

/// Escreve no stderr sem alocar (o alocador não pode chamar a si mesmo).
fn avisar_e_abortar(rotulo: &str, pedido: usize, em_uso: usize, teto: usize) -> ! {
    use std::io::Write;
    fn numero(buf: &mut [u8; 20], mut n: usize) -> &[u8] {
        let mut i = buf.len();
        loop {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 { break; }
        }
        &buf[i..]
    }
    let (mut a, mut b, mut c) = ([0u8; 20], [0u8; 20], [0u8; 20]);
    let mut e = std::io::stderr().lock();
    for parte in [b"\n[teto de memoria] " as &[u8], rotulo.as_bytes(), b": pedido de ", numero(&mut a, pedido),
        b" bytes recusado (em uso ", numero(&mut b, em_uso), b", teto ", numero(&mut c, teto), b")\n"] {
        let _ = e.write_all(parte);
    }
    std::process::abort()
}

unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // `try_with`: na saída da thread as variáveis locais já podem ter sido destruídas.
        let em_uso = EM_USO.try_with(Cell::get).unwrap_or(0);
        let teto = TETO.try_with(Cell::get).unwrap_or(usize::MAX);
        if !cabe(em_uso, layout.size(), teto) {
            // Desliga o teto antes de escrever: o stderr pode pedir memória.
            let _ = TETO.try_with(|t| t.set(usize::MAX));
            avisar_e_abortar(ROTULO.try_with(Cell::get).unwrap_or(""), layout.size(), em_uso, teto);
        }
        // SAFETY: repassa o mesmo layout ao alocador do sistema.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let agora = em_uso + layout.size();
            let _ = EM_USO.try_with(|c| c.set(agora));
            let _ = PICO.try_with(|c| if agora > c.get() { c.set(agora) });
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Memória pedida em outra thread e solta aqui: satura em zero em vez de dar a volta.
        let _ = EM_USO.try_with(|c| c.set(c.get().saturating_sub(layout.size())));
        // SAFETY: o ponteiro veio deste alocador com este layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALOCADOR: Contador = Contador;

/// Teto de memória da thread atual enquanto existir; ao sair, volta a ser ilimitado.
pub struct Teto { base: usize }

impl Teto {
    /// `bytes` a mais do que a thread já usa agora; `rotulo` sai na mensagem de recusa.
    pub fn novo(rotulo: &'static str, bytes: usize) -> Self {
        let base = EM_USO.with(Cell::get);
        ROTULO.with(|r| r.set(rotulo));
        PICO.with(|p| p.set(base));
        TETO.with(|t| t.set(base.saturating_add(bytes)));
        Teto { base }
    }

    /// O maior uso desde a criação do teto, descontado o que já estava em uso.
    pub fn pico(&self) -> usize {
        PICO.with(Cell::get).saturating_sub(self.base)
    }
}

impl Drop for Teto {
    fn drop(&mut self) {
        TETO.with(|t| t.set(usize::MAX));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pedido_que_passa_do_teto_nao_cabe() {
        assert!(cabe(10, 90, 100));
        assert!(!cabe(10, 91, 100));
        assert!(!cabe(usize::MAX - 1, 5, usize::MAX), "a soma que estoura não cabe");
    }

    #[test]
    fn pico_conta_a_maior_alocacao_e_nao_o_que_ja_existia() {
        let antes: Vec<u8> = vec![1; 1 << 20];
        let teto = Teto::novo("pico_conta_a_maior_alocacao", 16 << 20);
        {
            let grande: Vec<u8> = vec![2; 4 << 20];
            assert_eq!(grande.len(), 4 << 20);
        }
        let pequeno: Vec<u8> = vec![3; 1 << 10];
        let pico = teto.pico();
        assert!((4 << 20..5 << 20).contains(&pico), "pico {pico}");
        drop((antes, pequeno));
    }
}
