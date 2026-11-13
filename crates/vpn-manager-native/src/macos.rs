use crate::{command, error, state, windows::valid_uuid};
use vpn_manager_core::{
    Profile,
    backend::{BackendError, ErrorKind},
};

pub(crate) fn profiles() -> Result<Vec<Profile>, BackendError> {
    parse(&command::run_text("/usr/sbin/scutil", &["--nc", "list"])?)
}

fn parse(text: &str) -> Result<Vec<Profile>, BackendError> {
    let mut profiles = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('*').trim_start();
        if !line.starts_with('(') {
            continue;
        }
        let (status, rest) = line[1..].split_once(')').ok_or_else(invalid)?;
        let rest = rest.trim_start();
        let (id, rest) = rest.split_once(char::is_whitespace).ok_or_else(invalid)?;
        if !valid_uuid(id) {
            return Err(invalid());
        }
        // scutil prints: UUID TYPE "name" [subtype]. Preserve quotes in names.
        let first = rest.find('"').ok_or_else(invalid)?;
        let last = rest
            .rfind('"')
            .filter(|last| *last > first)
            .ok_or_else(invalid)?;
        let name = &rest[first + 1..last];
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(invalid());
        }
        let protocol = rest[last + 1..].trim().trim_matches(['[', ']']);
        // scutil may also list PPP dial-up services. Only expose VPN families.
        if !(protocol == "IPSec"
            || protocol == "PPP:L2TP"
            || protocol == "VPN"
            || protocol.starts_with("VPN:"))
        {
            continue;
        }
        profiles.push(Profile {
            uuid: id.into(),
            name: name.into(),
            kind: "macos-vpn".into(),
            protocol: protocol.into(),
            state: state(status),
        });
    }
    Ok(profiles)
}

fn invalid() -> BackendError {
    error(ErrorKind::Failed, "Inventário VPN do macOS inválido.")
}

pub(crate) fn control(profile: &Profile, disconnect: bool) -> Result<(), BackendError> {
    command::run(
        "/usr/sbin/scutil",
        &[
            "--nc",
            if disconnect { "stop" } else { "start" },
            &profile.uuid,
        ],
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_enabled_disabled_unicode_and_unknown_states() {
        let profiles = parse("Available network connection services in the current set (*=enabled):\n* (Connected) 12345678-1234-1234-1234-123456789abc PPP --> IPSec \"VPN \\\"São Paulo\\\"\" [IPSec]\n  (Disconnected) 12345678-1234-1234-1234-123456789abd VPN \"Example\" [VPN:IKEv2]\n* (FutureState) 12345678-1234-1234-1234-123456789abe VPN \"Pending\" [VPN:IKEv2]\n").unwrap();
        assert_eq!(profiles.len(), 3);
        assert!(profiles[0].state.is_connected());
        assert_eq!(profiles[1].protocol, "VPN:IKEv2");
        assert_eq!(
            profiles[2].state,
            vpn_manager_core::ConnectionState::Unknown
        );
        assert!(
            parse("Available network connection services:")
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn rejects_bad_identity_without_exposing_configuration() {
        let error = parse("* (Connected) --help VPN \"private-gateway\" [VPN:IKEv2]").unwrap_err();
        assert!(!error.message.contains("private-gateway"));
    }
}
