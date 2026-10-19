//! Quantas threads de trabalho o FeatherSheet usa para abrir arquivo,
//! localizar e calcular em paralelo (Configurações → Desempenho).
//!
//! Uma thread do processador fica sempre de fora, para a janela e o sistema
//! continuarem respondendo: a escolha vai de 1 até o total − 1, e o padrão é
//! esse máximo. Num processador de um núcleo só, sobra 1 mesmo.

use std::sync::atomic::{AtomicUsize, Ordering};

/// 0 = ninguém escolheu ainda: vale o [`padrao`].
static ESCOLHIDO: AtomicUsize = AtomicUsize::new(0);

/// Threads do processador (núcleos lógicos) que o sistema informa.
pub fn total_do_sistema() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// O máximo que se pode escolher: todas menos a da janela.
pub fn maximo() -> usize {
    total_do_sistema().saturating_sub(1).max(1)
}

/// Todas menos uma (15 num de 16).
pub fn padrao() -> usize {
    maximo()
}

/// Define quantas usar (fora de 1..=máximo é ajustado; 0 volta ao padrão).
pub fn definir(n: usize) {
    let n = if n == 0 { 0 } else { n.clamp(1, maximo()) };
    ESCOLHIDO.store(n, Ordering::Relaxed);
}

/// Quantas threads de trabalho usar agora.
pub fn nucleos() -> usize {
    match ESCOLHIDO.load(Ordering::Relaxed) {
        0 => padrao(),
        n => n.min(maximo()),
    }
}

/// O nome do processador, quando o sistema conta (Linux: `/proc/cpuinfo`;
/// Windows: a variável `PROCESSOR_IDENTIFIER`).
pub fn nome_do_processador() -> Option<String> {
    if let Ok(info) = std::fs::read_to_string("/proc/cpuinfo") {
        let nome = info.lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string());
        if nome.as_deref().is_some_and(|n| !n.is_empty()) {
            return nome;
        }
    }
    std::env::var("PROCESSOR_IDENTIFIER").ok().filter(|n| !n.trim().is_empty())
}

/// Ajusta o alocador do glibc para devolver ao sistema a memória que as
/// threads de trabalho liberam. Chamar no início do programa.
///
/// Depois que um bloco grande é liberado, o glibc sobe sozinho os limites de
/// `mmap` (até 32 MB) e de devolução (até 64 MB), e passa a guardar esse
/// tanto livre no topo da área de **cada thread** — `malloc_trim` não mexe
/// nesse topo. Calculando a Planilha Pesada em 15 threads ficavam 732 MiB;
/// com 4 MB / 8 MB fixos, 364 MiB (o cálculo sob demanda deixava 457), na
/// mesma velocidade; 1 MB / 4 MB atrasava ~0,6 s a abertura do XLSX. Limitar o número de áreas (`M_ARENA_MAX`) também
/// devolvia, mas dobrava o tempo de abrir o XLSX. Fora do Linux com glibc
/// não faz nada.
pub fn ajustar_alocador() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        const M_TRIM_THRESHOLD: i32 = -1;
        const M_MMAP_THRESHOLD: i32 = -3;
        // SAFETY: `mallopt` só muda parâmetros do alocador do glibc, que
        // está ligado ao programa; não recebe nem devolve ponteiro.
        unsafe {
            mallopt(M_MMAP_THRESHOLD, 4 << 20);
            mallopt(M_TRIM_THRESHOLD, 8 << 20);
        }
    }
}

/// Devolve ao sistema as páginas livres no meio das áreas do alocador
/// (complemento de [`ajustar_alocador`], que cuida do topo). Fora do Linux
/// com glibc não faz nada.
pub fn devolver_memoria_ao_sistema() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: `malloc_trim` só libera páginas livres do alocador do
        // glibc, que está ligado ao programa; não recebe ponteiro.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// A frequência máxima do processador em GHz, quando o sistema conta
/// (Linux: `cpufreq`, em kHz). Nem toda máquina virtual expõe.
pub fn frequencia_maxima_ghz() -> Option<f64> {
    let khz: f64 = std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (khz > 0.0).then_some(khz / 1_000_000.0)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escolha_fica_entre_um_e_o_total_menos_um() {
        assert!(padrao() >= 1 && padrao() <= maximo());
        definir(10_000);
        assert_eq!(nucleos(), maximo());
        definir(1);
        assert_eq!(nucleos(), 1);
        definir(0);
        assert_eq!(nucleos(), padrao());
    }
}

/// Roda `f` para cada coluna, em paralelo no aplicativo — uma thread por
/// núcleo, cada uma pegando a próxima coluna livre (colunas têm tamanhos
/// muito diferentes). No `.wasm` dos testes não há thread: em série.
pub fn por_coluna<T: Send>(colunas: &[u32], f: impl Fn(u32) -> T + Sync) -> Vec<T> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = crate::paralelismo::nucleos().min(colunas.len());
        if threads > 1 {
            let proxima = std::sync::atomic::AtomicUsize::new(0);
            let mut feitas: Vec<(usize, T)> = std::thread::scope(|escopo| {
                let alcas: Vec<_> = (0..threads)
                    .map(|_| {
                        escopo.spawn(|| {
                            let mut minhas = Vec::new();
                            loop {
                                let i = proxima.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(&col) = colunas.get(i) else { break };
                                minhas.push((i, f(col)));
                            }
                            minhas
                        })
                    })
                    .collect();
                alcas.into_iter().flat_map(|a| a.join().expect("thread por coluna")).collect()
            });
            feitas.sort_unstable_by_key(|(i, _)| *i);
            return feitas.into_iter().map(|(_, t)| t).collect();
        }
    }
    colunas.iter().map(|&c| f(c)).collect()
}

/// Andamento do cálculo em paralelo (células feitas de quantas), para a
/// barra de abertura da janela. Um cálculo grande por vez: estado global.
pub mod andamento {
    use std::sync::atomic::{AtomicU64, Ordering};
    static FEITO: AtomicU64 = AtomicU64::new(0);
    static TOTAL: AtomicU64 = AtomicU64::new(0);

    pub fn comecar(total: u64) {
        FEITO.store(0, Ordering::Relaxed);
        TOTAL.store(total, Ordering::Relaxed);
    }
    pub fn somar(n: u64) {
        FEITO.fetch_add(n, Ordering::Relaxed);
    }
    /// (feito, total)
    pub fn agora() -> (u64, u64) {
        (FEITO.load(Ordering::Relaxed), TOTAL.load(Ordering::Relaxed))
    }
}
