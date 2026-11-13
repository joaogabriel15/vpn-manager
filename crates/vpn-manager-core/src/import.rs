//! Conservative format detection. This is not a protocol configuration parser.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    OpenVpn,
    WireGuard,
    ManagerProfile,
    CiscoPcf,
}

impl ImportFormat {
    pub fn backend_name(self) -> &'static str {
        match self {
            Self::OpenVpn => "openvpn",
            Self::WireGuard => "wireguard",
            Self::ManagerProfile => "vpn-manager",
            Self::CiscoPcf => "vpnc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionError {
    Unknown,
    Ambiguous,
}

fn visible_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut inline = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(ref tag) = inline {
            if line == format!("</{tag}>") {
                inline = None;
            }
            continue;
        }
        if let Some(tag) = line.strip_prefix('<').and_then(|s| s.strip_suffix('>'))
            && matches!(
                tag,
                "ca" | "cert"
                    | "key"
                    | "pkcs12"
                    | "dh"
                    | "secret"
                    | "tls-auth"
                    | "tls-crypt"
                    | "tls-crypt-v2"
                    | "extra-certs"
            )
        {
            inline = Some(tag.to_owned());
            continue;
        }
        lines.push(line);
    }
    lines
}

pub fn detect_import(text: &str) -> Result<ImportFormat, DetectionError> {
    if crate::profile::ProfileDocument::from_toml(text).is_ok() {
        return Ok(ImportFormat::ManagerProfile);
    }
    let (mut interface, mut peer, mut openvpn) = (false, false, false);
    let (mut main_section, mut host, mut group) = (false, false, false);
    for line in visible_lines(text) {
        interface |= line == "[Interface]";
        peer |= line == "[Peer]";
        if line.starts_with('[') {
            main_section = line == "[main]";
        }
        if main_section
            && let Some((key, value)) = line.split_once('=')
            && !value.trim().is_empty()
        {
            host |= key.trim() == "Host";
            group |= key.trim() == "GroupName";
        }
        let mut words = line.split_whitespace();
        let directive = words
            .next()
            .unwrap_or_default()
            .trim_matches(['\'', '"'])
            .trim_start_matches("--");
        openvpn |= matches!(directive, "client" | "tls-client")
            || (directive == "remote" && words.next().is_some());
    }
    let formats: Vec<_> = [
        (openvpn, ImportFormat::OpenVpn),
        (interface && peer, ImportFormat::WireGuard),
        (host && group, ImportFormat::CiscoPcf),
    ]
    .into_iter()
    .filter_map(|(found, format)| found.then_some(format))
    .collect();
    match formats.as_slice() {
        [format] => Ok(*format),
        [] => Err(DetectionError::Unknown),
        _ => Err(DetectionError::Ambiguous),
    }
}

/// The app delegates tunnel execution, but does not silently accept imported
/// scripts/includes. Errors never contain source lines or values.
pub fn validate_import_policy(text: &str, format: ImportFormat) -> Result<(), String> {
    for line in visible_lines(text) {
        match format {
            ImportFormat::OpenVpn => {
                let mut words = line.split_whitespace();
                let directive = words
                    .next()
                    .unwrap_or_default()
                    .trim_matches(['\'', '"'])
                    .trim_start_matches("--");
                if matches!(
                    directive,
                    "up" | "down"
                        | "route-up"
                        | "route-pre-down"
                        | "ipchange"
                        | "tls-verify"
                        | "auth-user-pass-verify"
                        | "client-connect"
                        | "client-disconnect"
                        | "learn-address"
                        | "plugin"
                        | "config"
                        | "iproute"
                        | "engine"
                ) || (directive == "script-security"
                    && !matches!(words.next(), Some("0" | "1" | "2")))
                {
                    return Err("O perfil OpenVPN contém scripts, plugins executáveis ou arquivos de configuração adicionais. Revise essas opções com o administrador e forneça um perfil sem execução externa.".into());
                }
            }
            ImportFormat::WireGuard => {
                if let Some((key, _)) = line.split_once('=')
                    && matches!(
                        key.trim().to_ascii_lowercase().as_str(),
                        "preup" | "postup" | "predown" | "postdown"
                    )
                {
                    return Err("O perfil WireGuard contém scripts de rede. Configure essas operações separadamente com o administrador; o app não importa hooks executáveis.".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_content_with_comments_bom_and_windows_lines() {
        assert_eq!(
            detect_import("\u{feff}# fixture\r\nclient\r\nremote example.invalid 1194"),
            Ok(ImportFormat::OpenVpn)
        );
        assert_eq!(
            detect_import(
                "; fixture\n[Interface]\nAddress=10.0.0.1/32\n[Peer]\nPublicKey=synthetic"
            ),
            Ok(ImportFormat::WireGuard)
        );
    }

    #[test]
    fn rejects_unknown_incomplete_and_conflicting_formats() {
        assert_eq!(
            detect_import("[Interface]\nAddress=10.0.0.1"),
            Err(DetectionError::Unknown)
        );
        assert_eq!(
            detect_import("[unrelated]\nkey=value"),
            Err(DetectionError::Unknown)
        );
        assert_eq!(
            detect_import("client\n[Interface]\n[Peer]"),
            Err(DetectionError::Ambiguous)
        );
    }

    #[test]
    fn ignores_comments_and_inline_certificate_contents() {
        assert_eq!(
            detect_import("# client\n<cert>\nremote example.invalid\n[Interface]\n[Peer]\n</cert>"),
            Err(DetectionError::Unknown)
        );
    }

    #[test]
    fn reads_openvpn_connection_blocks() {
        assert_eq!(
            detect_import("<connection>\nremote example.invalid 1194\n</connection>"),
            Ok(ImportFormat::OpenVpn)
        );
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    #[test]
    fn pcf_detection_uses_signature_and_rejects_mixed_or_incomplete_profiles() {
        let pcf = "[main]\nHost=vpn.example.invalid\nGroupName=Synthetic\nenc_GroupPwd=synthetic-secret\n";
        assert_eq!(detect_import(pcf), Ok(ImportFormat::CiscoPcf));
        assert_eq!(
            detect_import("Host=x\nGroupName=x"),
            Err(DetectionError::Unknown)
        );
        assert_eq!(
            detect_import("[main]\nHost=x"),
            Err(DetectionError::Unknown)
        );
        assert_eq!(
            detect_import(&format!("{pcf}\nclient")),
            Err(DetectionError::Ambiguous)
        );
    }
    #[test]
    fn rejects_executable_configuration_without_echoing_it() {
        for directive in [
            "up",
            "--down",
            "\"plugin\"",
            "config",
            "route-up",
            "tls-verify",
            "script-security 3",
        ] {
            let error = validate_import_policy(
                &format!("client\n{directive} private-secret"),
                ImportFormat::OpenVpn,
            )
            .unwrap_err();
            assert!(!error.contains("private-secret"));
        }
        assert!(
            validate_import_policy(
                "client\n# up script\n<cert>\nup not-a-directive\n</cert>\nscript-security 1",
                ImportFormat::OpenVpn
            )
            .is_ok()
        );
        assert!(
            validate_import_policy(
                "[Interface]\nPostUp=private-command\n[Peer]",
                ImportFormat::WireGuard
            )
            .is_err()
        );
    }
}
