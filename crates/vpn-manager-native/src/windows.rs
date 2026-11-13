use crate::{command, error, state};
use serde::Deserialize;
use vpn_manager_core::{
    Profile,
    backend::{BackendError, ErrorKind},
};

// A fixed script; profile names are never interpolated into PowerShell source.
const INVENTORY: &str = r#"$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $p=@(Get-VpnConnection | ForEach-Object { [PSCustomObject]@{ Name=$_.Name; Id=$_.Guid.ToString(); Protocol=$_.TunnelType.ToString(); Status=$_.ConnectionStatus.ToString() } }); ConvertTo-Json -InputObject $p -Compress"#;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Entry {
    name: String,
    id: String,
    protocol: String,
    status: String,
}

fn system_tool(name: &str) -> Result<String, BackendError> {
    let root = std::env::var_os("SystemRoot")
        .ok_or_else(|| error(ErrorKind::Unavailable, "SystemRoot indisponível."))?;
    Ok(std::path::PathBuf::from(root)
        .join("System32")
        .join(name)
        .to_string_lossy()
        .into_owned())
}

pub(crate) fn profiles() -> Result<Vec<Profile>, BackendError> {
    parse(&command::run_text(
        &system_tool("WindowsPowerShell/v1.0/powershell.exe")?,
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            INVENTORY,
        ],
    )?)
}

fn parse(text: &str) -> Result<Vec<Profile>, BackendError> {
    let entries: Vec<Entry> = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| error(ErrorKind::Failed, "Inventário VPN do Windows inválido."))?;
    entries
        .into_iter()
        .map(|entry| {
            if entry.name.is_empty()
                || entry.name.starts_with(['/', '-'])
                || entry.name.chars().any(char::is_control)
                || !valid_uuid(&entry.id)
            {
                return Err(error(
                    ErrorKind::Failed,
                    "Identidade de perfil Windows inválida.",
                ));
            }
            Ok(Profile {
                uuid: entry.id,
                name: entry.name,
                kind: "windows-vpn".into(),
                protocol: entry.protocol,
                state: state(&entry.status),
            })
        })
        .collect()
}

pub(crate) fn valid_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

pub(crate) fn control(profile: &Profile, disconnect: bool) -> Result<(), BackendError> {
    let tool = system_tool("rasdial.exe")?;
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| error(ErrorKind::Unavailable, "APPDATA indisponível."))?;
    let phonebook =
        std::path::PathBuf::from(appdata).join("Microsoft/Network/Connections/Pbk/rasphone.pbk");
    if !phonebook.is_file() {
        return Err(error(
            ErrorKind::NotFound,
            "Catálogo VPN do usuário não encontrado.",
        ));
    }
    let phonebook_arg = format!("/phonebook:{}", phonebook.to_string_lossy());
    let mut args = vec![profile.name.as_str(), phonebook_arg.as_str()];
    if disconnect {
        args.push("/disconnect");
    }

    command::run(&tool, &args).map(|_| ())
}

pub(crate) fn open_settings() -> Result<(), BackendError> {
    command::run(
        &system_tool("WindowsPowerShell/v1.0/powershell.exe")?,
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Process 'ms-settings:network-vpn'",
        ],
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_preserves_names_and_never_guesses_connected() {
        let profiles = parse(r#"[{"Name":"VPN ' ; $() 中文","Id":"12345678-1234-1234-1234-123456789abc","Protocol":"Ikev2","Status":"Connected"},{"Name":"Pending","Id":"12345678-1234-1234-1234-123456789abd","Protocol":"Sstp","Status":"Unexpected"}]"#).unwrap();
        assert_eq!(profiles[0].name, "VPN ' ; $() 中文");
        assert!(profiles[0].state.is_connected());
        assert_eq!(
            profiles[1].state,
            vpn_manager_core::ConnectionState::Unknown
        );
        assert!(parse("[]").unwrap().is_empty());
    }
    #[test]
    fn rejects_malformed_inventory_and_option_names_without_echoing() {
        for text in [
            "private-gateway",
            r#"[{"Name":"/disconnect","Id":"12345678-1234-1234-1234-123456789abc","Protocol":"Ikev2","Status":"Connected"}]"#,
        ] {
            let error = parse(text).unwrap_err();
            assert!(!error.message.contains("private-gateway"));
        }
        assert!(!valid_uuid("--help"));
    }
}
