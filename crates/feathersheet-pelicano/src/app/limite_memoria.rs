//! Limite de memória RAM do FeatherSheet: a configuração fica aqui, a vigia
//! é a comum da suíte (`feather_io::memoria`, reexportada pelo
//! `feathersheet_io`). Os leitores conferem o limite entre blocos de
//! trabalho; passou dele, a operação para com uma mensagem e o app continua.
use feathersheet_io::memoria;

/// Valores ilegíveis usam o padrão; zero desliga e valores positivos são limitados.
pub(crate) fn normalizar_limite(valor: f64) -> u16 {
    memoria::normalizar_gb(valor)
}

/// RAM instalada, em GB inteiros (Linux: `MemTotal` do `/proc/meminfo`).
/// `None` quando o sistema não conta: a lista mostra todas as opções.
pub(crate) fn ram_total_gb() -> Option<u16> {
    // Linux (`/proc/meminfo`) e Windows (`GlobalMemoryStatusEx`): a mesma
    // leitura do monitor F12 da suíte (antes era só Linux).
    let kb = feather_ui::monitor::ram_total_kb()?;
    // 32 GB instalados aparecem como ~30,7 GiB (o sistema reserva uma
    // parte): arredonda para o par de cima, o tamanho dos pentes.
    let gib = kb.div_ceil(1024 * 1024);
    Some((gib + gib % 2).min(u16::MAX as u64) as u16)
}

/// As opções do limite que cabem na máquina: até a RAM instalada.
pub(crate) fn opcoes_limite() -> &'static [&'static str] {
    let Some(total) = ram_total_gb() else { return &LIMITES };
    let cabem = LIMITES.iter().take_while(|r| {
        r.split_whitespace().next().and_then(|n| n.parse::<u16>().ok()).is_some_and(|gb| gb <= total)
    }).count();
    &LIMITES[..cabem.max(2)]
}

static LIMITES: [&str; 10] = ["0 GB (desligado)", "2 GB", "4 GB", "6 GB", "8 GB", "12 GB", "16 GB", "32 GB", "64 GB", "128 GB"];

/// Passa o limite das Configurações para a vigia e confere o uso atual.
/// Chamado antes e depois de abrir um arquivo; durante a leitura quem
/// confere é o próprio leitor.
pub(super) fn verificar(limite_gb: u16) -> Result<(), String> {
    memoria::definir_limite_gb(limite_gb);
    memoria::verificar(tr!("A abertura do arquivo"))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normalizacao_equivale_ao_typescript() {
        assert_eq!(normalizar_limite(f64::NAN), 6);
        assert_eq!(normalizar_limite(0.0), 0);
        assert_eq!(normalizar_limite(-2.0), 0);
        assert_eq!(normalizar_limite(0.9), 2);
        assert_eq!(normalizar_limite(4.6), 5);
        assert_eq!(normalizar_limite(900.0), 128);
    }

    #[test]
    fn desligado_nunca_interrompe() {
        assert!(verificar(0).is_ok());
    }
}
