//! Fuso de `AGORA()`/`HOJE()`: o motor não sabe o fuso do sistema (o `std`
//! não diz e o `feathersheet-core` não tem dependência), então o app informa.

/// Minutos a leste de UTC no fuso do sistema (Brasília: -180).
pub(crate) fn do_sistema() -> i32 {
    #[cfg(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos"))]
    {
        let agora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as libc::time_t;
        // SAFETY: `localtime_r` só escreve na `tm` que passamos.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&agora, &mut tm) }.is_null() {
            return (tm.tm_gmtoff / 60) as i32;
        }
    }
    // shortcut: no Windows ainda é UTC; ler o fuso pelo GetTimeZoneInformation.
    0
}

/// Escolhido nas Configurações, ou o do sistema.
pub(crate) fn aplicar(escolhido: Option<i32>) {
    feathersheet_core::funcoes::fn_date::definir_fuso_minutos(escolhido.unwrap_or_else(do_sistema));
}

/// "UTC−03:00".
pub(crate) fn rotulo(minutos: i32) -> String {
    let sinal = if minutos < 0 { '−' } else { '+' };
    format!("UTC{sinal}{:02}:{:02}", minutos.abs() / 60, minutos.abs() % 60)
}

#[cfg(test)]
mod testes {
    #[test]
    fn rotulo_do_fuso() {
        assert_eq!(super::rotulo(-180), "UTC−03:00");
        assert_eq!(super::rotulo(330), "UTC+05:30");
        assert_eq!(super::rotulo(0), "UTC+00:00");
    }
}
