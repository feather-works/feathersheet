// Nos testes, `app.update(...)` devolve uma `Task` que só o runtime do Iced
// executaria: ignorá-la é o esperado, e o aviso escondia os avisos de verdade.
#![cfg_attr(test, allow(unused_must_use))]
// No Windows, o executável de release é um programa de janela: sem isto o
// sistema abre um console (cmd) junto com o app. `--mcp` continua falando
// pela entrada e saída que quem o chama (o cliente de IA) entrega por pipe.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// Primeiro: as macros tr!/trf! valem para todos os módulos abaixo.
#[macro_use]
mod idioma;
mod vigia;
mod app;
mod configuracao;
mod grade;
mod imagem_celula;
mod caixas;
mod dinamica_visual;
mod condicional;
mod geometria_overlay;
mod bordas_comuns;
mod estilos_comuns;
mod edicao_formula;
mod campo_fx;
mod escopo_app;

use app::App;

/// A janela foi aberta pelo XWayland (ver `preparar_backend_da_caneta`).
pub static CANETA_X11_FORCADO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Canetas de mesa vistas no sysfs ao abrir (para o Console F12).
pub static MESAS_NA_ABERTURA: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Ler a pressão da mesa? `FEATHERSHEET_CANETA_X11=1/0` vale mais que a
/// configuração (para testar sem mexer nas preferências).
pub fn caneta_pressao_pedida(config: &configuracao::Configuracao) -> bool {
    match std::env::var("FEATHERSHEET_CANETA_X11").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => config.caneta_pressao,
    }
}

/// Onde abrir a janela no Linux/Wayland: `FEATHERSHEET_JANELA=auto|x11|wayland`
/// vale mais que a configuração; `FEATHERSHEET_CANETA_X11=1` também força X11.
pub fn preferencia_janela(config: &configuracao::Configuracao) -> feather_ui::caneta::PreferenciaJanela {
    use feather_ui::caneta::PreferenciaJanela;
    if let Some(p) = std::env::var("FEATHERSHEET_JANELA").ok().as_deref().and_then(PreferenciaJanela::de_texto) {
        return p;
    }
    if std::env::var("FEATHERSHEET_CANETA_X11").as_deref() == Ok("1") {
        return PreferenciaJanela::X11;
    }
    PreferenciaJanela::de_texto(&config.janela_linux).unwrap_or(PreferenciaJanela::Automatica)
}

/// No Wayland o winit 0.30 não fala o protocolo de mesa (tablet-v2): a
/// ponta da caneta move o cursor mas não clica em nada, e o Iced não
/// entrega força. Com mesa conectada (modo automático) ou X11 escolhido,
/// a janela nasce no XWayland: sem `WAYLAND_DISPLAY` o winit usa o X11
/// (`DISPLAY`). Tem de ser antes de qualquer thread e do laço de eventos.
fn preparar_backend_da_caneta() {
    use feather_ui::caneta;
    let config = configuracao::Configuracao::carregar();
    let preferencia = preferencia_janela(&config);
    let mesas = caneta::mesas_conectadas();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();
    if caneta::deve_usar_x11(preferencia, !mesas.is_empty(), wayland.as_deref(), display.as_deref()) {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
        CANETA_X11_FORCADO.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let _ = MESAS_NA_ABERTURA.set(mesas);
}

/// Com a sincronia de vídeo (FIFO), cada janela espera o próprio quadro
/// ser exibido; uma janela auxiliar escondida atrás da principal nunca é
/// exibida, e o Iced, que redesenha todas as janelas juntas, travava o app
/// inteiro esperando por ela. `AutoNoVsync` (Mailbox, ou Immediate onde
/// não houver) não espera. `ICED_PRESENT_MODE` definido pelo usuário vale
/// mais. Tem de ser antes de qualquer thread.
fn preparar_modo_de_apresentacao() {
    if std::env::var_os("ICED_PRESENT_MODE").is_none() {
        std::env::set_var("ICED_PRESENT_MODE", "no_vsync");
    }
}

fn main() -> iced::Result {
    #[cfg(feature = "pelicano")]
    if std::env::args().any(|arg| arg == "--pelicano") {
        return app::pelicano::rodar().map_err(|e| iced::Error::WindowCreationFailed(Box::new(e)));
    }
    // `feathersheet --mcp`: servidor MCP por stdio, sem janela (quem instalou
    // só o aplicativo liga uma IA por aqui; ver Configurações → IA (MCP)).
    if std::env::args().skip(1).any(|a| a == "--mcp") {
        feathersheet_mcp::rodar_stdio();
        return Ok(());
    }
    preparar_modo_de_apresentacao();
    preparar_backend_da_caneta();
    escopo_app::entrar_no_escopo_do_app();
    feathersheet_core::paralelismo::ajustar_alocador();
    iced::daemon(App::titulo, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::tema)
        .antialiasing(true)
        .run_with(App::novo)
}
