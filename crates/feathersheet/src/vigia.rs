//! Vigia da tela: anota no F12 toda vez que a thread da janela fica presa
//! mais que [`LIMITE_MS`] — numa mensagem do `update` ou no desenho da
//! grade. "Não está respondendo" nunca pode aparecer (pedido do usuário,
//! 17/10/2026); o vigia acha quem segura a tela sem precisar reproduzir.
use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::Duration;

pub const LIMITE_MS: u128 = 200;

static TRAVADAS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Guarda até `max` caracteres e para de formatar no resto: o `Debug` de
/// `Msg::Colar` com 13 milhões de células não pode ser montado inteiro.
struct Curto { texto: String, max: usize }

impl std::fmt::Write for Curto {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for c in s.chars() {
            if self.texto.chars().count() >= self.max {
                self.texto.push('…');
                return Err(std::fmt::Error);
            }
            self.texto.push(c);
        }
        Ok(())
    }
}

/// O começo do `Debug` de `v`, barato mesmo quando `v` é enorme.
pub fn nome_curto(v: &impl std::fmt::Debug) -> String {
    let mut curto = Curto { texto: String::new(), max: 80 };
    let _ = write!(curto, "{v:?}");
    curto.texto
}

/// Anota `o_que` se `tempo` passou do limite.
pub fn conferir(o_que: impl FnOnce() -> String, tempo: Duration) {
    let ms = tempo.as_millis();
    if ms >= LIMITE_MS {
        if let Ok(mut fila) = TRAVADAS.lock() {
            fila.push(format!("[tela] presa {ms} ms: {}", o_que()));
        }
    }
}

/// As travadas anotadas desde a última vez.
pub fn recolher() -> Vec<String> {
    TRAVADAS.lock().map(|mut f| std::mem::take(&mut *f)).unwrap_or_default()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_anota_acima_do_limite_e_corta_o_nome() {
        let _ = recolher();
        conferir(|| "rápido".into(), Duration::from_millis(10));
        let enorme = "x".repeat(10_000_000);
        conferir(|| nome_curto(&enorme), Duration::from_millis(900));
        let anotadas = recolher();
        assert_eq!(anotadas.len(), 1);
        assert!(anotadas[0].starts_with("[tela] presa 900 ms") && anotadas[0].chars().count() < 130, "{}", anotadas[0]);
    }
}
