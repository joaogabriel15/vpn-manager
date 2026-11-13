//! Delegate protocol-specific configuration and interactive login to system tools.
use crate::{NetworkManager, inventory, management::invalid};
use vpn_manager_core::backend::{Backend, BackendError, ErrorKind};

fn spawn(args: &[String]) -> Result<gio::Subprocess, BackendError> {
    gio::Subprocess::newv(&args.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>(),
        gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_SILENCE)
        .map_err(|_| BackendError { kind: ErrorKind::Unavailable, message: "Não foi possível abrir a ferramenta do Linux. Instale nm-connection-editor ou as configurações de rede do KDE pelo gerenciador de pacotes.".into() })
}

pub fn editor_available() -> bool {
    ["nm-connection-editor", "kcmshell6", "kcmshell5"]
        .iter()
        .any(|tool| gio::glib::find_program_in_path(tool).is_some())
}

fn editor_arguments(
    service: Option<&str>,
    id: Option<&str>,
    has_nm_editor: bool,
) -> Result<Vec<String>, BackendError> {
    if let Some(service) = service
        && service != "wireguard"
        && !inventory::valid_service(service)
    {
        return Err(invalid("Tipo de VPN inválido."));
    }
    if let Some(id) = id {
        validate_uuid(id)?;
    }
    if !has_nm_editor {
        return Ok(vec!["kcmshell6".into(), "kcm_networkmanagement".into()]);
    }
    let mut args = vec!["nm-connection-editor".into()];
    if let Some(id) = id {
        args.extend(["--edit".into(), id.into()]);
    } else {
        args.push("--create".into());
        args.push(match service {
            Some("wireguard") => "--type=wireguard".into(),
            Some(service) => format!("--type=vpn:{service}"),
            None => "--type=vpn".into(),
        });
    }
    Ok(args)
}

pub async fn open_editor(
    service: Option<&str>,
    id: Option<&str>,
) -> Result<gio::Subprocess, BackendError> {
    if let Some(id) = id {
        crate::management::require_vpn(id).await?;
    }
    let normalized = service.map(|service| {
        if service == "wireguard" || inventory::valid_service(service) { Ok(service.to_owned()) }
        else if let Some(protocol) = vpn_manager_core::protocols::protocol(service) { Ok(format!("org.freedesktop.NetworkManager.{}", protocol.id)) }
        else { Err(invalid("Tipo de VPN inválido. Use um protocolo do catálogo ou o serviço completo do plugin instalado.")) }
    }).transpose()?;
    let service = normalized.as_deref();
    if let Some(service) = service
        && service != "wireguard"
    {
        let report = NetworkManager.capabilities().await?;
        if !report
            .plugins
            .iter()
            .any(|p| p.service == service && p.engine_available)
        {
            return Err(invalid(
                "Instale o plugin VPN selecionado e sua engine pelo gerenciador de pacotes do Linux.",
            ));
        }
    }
    let has_nm = gio::glib::find_program_in_path("nm-connection-editor").is_some();
    let mut args = editor_arguments(service, id, has_nm)?;
    if !has_nm && gio::glib::find_program_in_path("kcmshell6").is_none() {
        args[0] = "kcmshell5".into();
    }
    spawn(&args)
}

fn validate_uuid(id: &str) -> Result<(), BackendError> {
    if id.len() == 36
        && id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        Ok(())
    } else {
        Err(invalid("Identificador VPN inválido."))
    }
}

fn terminal_arguments(tool: &str, id: &str) -> Result<Vec<String>, BackendError> {
    validate_uuid(id)?;
    let mut args: Vec<String> = match tool {
        "konsole" => vec![tool.into(), "--separate".into(), "-e".into()],
        "gnome-terminal" => vec![tool.into(), "--wait".into(), "--".into()],
        "xterm" => vec![tool.into(), "-e".into()],
        _ => return Err(invalid("Terminal não suportado.")),
    };
    args.extend(
        [
            "nmcli",
            "--ask",
            "--wait",
            "120",
            "connection",
            "up",
            "uuid",
            id,
        ]
        .map(str::to_owned),
    );
    Ok(args)
}

pub fn terminal_available() -> bool {
    ["konsole", "gnome-terminal", "xterm"]
        .iter()
        .any(|tool| gio::glib::find_program_in_path(tool).is_some())
}

/// The returned process represents the terminal, not tunnel confirmation.
pub async fn interactive_login(id: &str) -> Result<gio::Subprocess, BackendError> {
    crate::management::ensure_engine(id).await?;
    let tool = ["konsole", "gnome-terminal", "xterm"].into_iter()
        .find(|tool| gio::glib::find_program_in_path(tool).is_some())
        .ok_or_else(|| invalid("Instale Konsole, GNOME Terminal ou xterm, ou use vpn-manager-cli connect <uuid> --ask em um terminal."))?;
    spawn(&terminal_arguments(tool, id)?)
}

/// For CLI callers already attached to a terminal. Secrets stay in the system
/// agent/nmcli, never in this process's argument list or credential model.
pub async fn connect_ask(id: &str) -> Result<(), BackendError> {
    crate::management::ensure_engine(id).await?;
    let process = gio::Subprocess::newv(
        &[
            "nmcli",
            "--ask",
            "--wait",
            "120",
            "connection",
            "up",
            "uuid",
            id,
        ]
        .map(std::ffi::OsStr::new),
        gio::SubprocessFlags::STDIN_INHERIT,
    )
    .map_err(|_| invalid("Não foi possível iniciar nmcli."))?;
    process.wait_check_future().await.map_err(|_| BackendError { kind: ErrorKind::Failed,
        message: "Login interativo não concluído. Consulte a mensagem do nmcli no terminal e atualize o estado.".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "11111111-1111-4111-8111-111111111111";
    #[test]
    fn editor_selects_the_exact_plugin_and_never_interpolates_a_shell() {
        let service = "org.freedesktop.NetworkManager.fortisslvpn";
        assert_eq!(
            editor_arguments(Some(service), None, true).unwrap(),
            [
                "nm-connection-editor",
                "--create",
                "--type=vpn:org.freedesktop.NetworkManager.fortisslvpn"
            ]
        );
        assert_eq!(
            editor_arguments(None, Some(ID), true).unwrap(),
            ["nm-connection-editor", "--edit", ID]
        );
        assert!(editor_arguments(Some("vpn; private-command"), None, true).is_err());
        assert!(editor_arguments(None, Some("--create"), true).is_err());
    }
    #[test]
    fn interactive_login_only_passes_a_validated_id_and_no_secrets() {
        for tool in ["konsole", "gnome-terminal", "xterm"] {
            let args = terminal_arguments(tool, ID).unwrap();
            assert_eq!(
                &args[args.len() - 8..],
                [
                    "nmcli",
                    "--ask",
                    "--wait",
                    "120",
                    "connection",
                    "up",
                    "uuid",
                    ID
                ]
            );
            assert!(terminal_arguments(tool, "$(private-command)").is_err());
        }
    }
}
