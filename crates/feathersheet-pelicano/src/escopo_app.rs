//! Faz o Linux reconhecer o processo como o aplicativo FeatherSheet.
//!
//! O GNOME (Recursos, Monitor do sistema) liga um processo ao `.desktop` — e
//! daí ao nome e à logo — pelo grupo de controle do systemd em que ele roda:
//! `app-gnome-feathersheet-<pid>.scope`. Quando o painel abre o app ele já
//! nasce nesse grupo; aberto pelo gerenciador de arquivos ou por outro
//! programa, cai no grupo de quem o lançou (`dbus.service`, por exemplo) e
//! aparece como um processo solto, sem ícone.
//!
//! Na partida, se não estiver num grupo `app-…`, o próprio processo pede ao
//! systemd do usuário um escopo com esse nome, pelo `busctl` (vem com o
//! systemd; sem ele, ou fora do Linux, nada acontece).

/// Nome do escopo no padrão que o GNOME Shell usa ao abrir um app.
#[cfg(any(target_os = "linux", test))]
fn nome_do_escopo(pid: u32) -> String {
    format!("app-gnome-feathersheet-{pid}.scope")
}

/// Se o grupo atual já é de um aplicativo (`/proc/self/cgroup`).
#[cfg(any(target_os = "linux", test))]
fn ja_em_escopo_de_app(cgroup: &str) -> bool {
    cgroup
        .lines()
        .filter_map(|l| l.rsplit('/').next())
        .any(|ultimo| ultimo.starts_with("app-") && ultimo.ends_with(".scope"))
}

/// Pede o escopo numa thread à parte, para não atrasar a janela.
#[cfg(target_os = "linux")]
pub fn entrar_no_escopo_do_app() {
    let Ok(cgroup) = std::fs::read_to_string("/proc/self/cgroup") else {
        return;
    };
    if ja_em_escopo_de_app(&cgroup) || std::env::var_os("FEATHERSHEET_SEM_ESCOPO").is_some() {
        return;
    }
    std::thread::spawn(|| {
        let pid = std::process::id();
        let _ = std::process::Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
                "StartTransientUnit",
                "ssa(sv)a(sa(sv))",
                &nome_do_escopo(pid),
                "fail",
                "1",
                "PIDs",
                "au",
                "1",
                &pid.to_string(),
                "0",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

#[cfg(not(target_os = "linux"))]
pub fn entrar_no_escopo_do_app() {}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn reconhece_grupo_de_app() {
        assert!(ja_em_escopo_de_app(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-feathersheet-42.scope\n"
        ));
        assert!(!ja_em_escopo_de_app(
            "0::/user.slice/user-1000.slice/user@1000.service/session.slice/dbus.service\n"
        ));
    }

    #[test]
    fn nome_segue_o_padrao_do_gnome() {
        assert_eq!(nome_do_escopo(7), "app-gnome-feathersheet-7.scope");
    }
}
