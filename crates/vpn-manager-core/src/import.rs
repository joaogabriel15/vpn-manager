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

/// Directives whose argument names a file that OpenVPN can also take inline.
const INLINE_FILES: [&str; 8] = [
    "ca",
    "cert",
    "key",
    "dh",
    "tls-auth",
    "tls-crypt",
    "tls-crypt-v2",
    "extra-certs",
];

/// OpenVPN client configuration found in a vendor bundle: a plain `.ovpn` or
/// text embedded in an installer script, which is read and never executed.
/// Files referenced by name are inlined from the bundle so the result is
/// self-contained; `auth-user-pass <file>` becomes an interactive login.
pub fn openvpn_from_bundle(files: &[(String, Vec<u8>)]) -> Result<String, DetectionError> {
    let mut found: Vec<String> = Vec::new();
    for (name, bytes) in files {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        let script = text.starts_with("#!") || name.ends_with(".sh");
        let blocks = if script {
            embedded_blocks(text)
        } else {
            vec![text.to_owned()]
        };
        for block in blocks {
            if detect_import(&block) == Ok(ImportFormat::OpenVpn) && !found.contains(&block) {
                found.push(block);
            }
        }
    }
    // Installers often ship per-distro variants of one profile; if one variant
    // contains every line of the others, it is the most complete choice.
    let superset = found.iter().find(|candidate| {
        let all = config_lines(candidate);
        found
            .iter()
            .all(|other| config_lines(other).is_subset(&all))
    });
    match (found.as_slice(), superset) {
        ([], _) => Err(DetectionError::Unknown),
        (_, Some(config)) => Ok(self_contained(config, files)),
        _ => Err(DetectionError::Ambiguous),
    }
}

/// Double-quoted strings and heredoc bodies of a shell script.
fn embedded_blocks(script: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut chars = script.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut block = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some(next @ ('"' | '\\' | '$' | '`')) => block.push(next),
                    Some(next) => {
                        block.push('\\');
                        block.push(next);
                    }
                    None => {}
                },
                _ => block.push(c),
            }
        }
        if block.contains('\n') {
            blocks.push(block);
        }
    }
    let mut lines = script.lines();
    while let Some(line) = lines.next() {
        let Some((_, marker)) = line.split_once("<<") else {
            continue;
        };
        let marker = marker
            .trim_start_matches('-')
            .trim()
            .trim_matches(['\'', '"']);
        if marker.is_empty() || marker.contains(char::is_whitespace) {
            continue;
        }
        let body: Vec<&str> = lines.by_ref().take_while(|l| l.trim() != marker).collect();
        blocks.push(body.join("\n"));
    }
    blocks
}

fn self_contained(config: &str, files: &[(String, Vec<u8>)]) -> String {
    let inline_tags: Vec<&str> = config
        .lines()
        .filter_map(|l| l.trim().strip_prefix('<')?.strip_suffix('>'))
        .collect();
    let has_key_direction = config
        .lines()
        .any(|l| l.trim_start().starts_with("key-direction"));
    let mut out = Vec::new();
    let mut inside = false;
    for line in config.lines() {
        let trimmed = line.trim();
        if let Some(tag) = trimmed.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            inside = !tag.starts_with('/');
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        match words.as_slice() {
            ["auth-user-pass", _, ..] if !inside => out.push("auth-user-pass".to_owned()),
            [directive, file, rest @ ..] if !inside && INLINE_FILES.contains(directive) => {
                if !has_key_direction
                    && *directive == "tls-auth"
                    && let Some(dir) = rest.first()
                {
                    out.push(format!("key-direction {dir}"));
                }
                if inline_tags.contains(directive) {
                    continue;
                }
                let base = file
                    .trim_matches(['\'', '"'])
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(file);
                match files
                    .iter()
                    .find(|(name, _)| name.rsplit('/').next() == Some(base))
                    .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
                {
                    Some(content) => out.push(format!(
                        "<{directive}>\n{}\n</{directive}>",
                        content.trim_end()
                    )),
                    None => out.push(line.to_owned()),
                }
            }
            _ => out.push(line.to_owned()),
        }
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod bundle_tests {
    use super::*;

    fn file(name: &str, text: &str) -> (String, Vec<u8>) {
        (name.into(), text.as_bytes().to_vec())
    }

    #[test]
    fn extracts_config_embedded_in_installer_script_without_running_it() {
        let script = "#!/bin/bash\nusuario=$(zenity --entry)\necho \"$usuario\" > /opt/x/login.conf\necho \"dev tun\nclient\nremote vpn.example.invalid 1194 udp\nauth-user-pass /opt/x/login.conf\nca lab-ca.crt\ntls-auth lab-tls.key 1\" > /opt/x/vpn.conf\n";
        let files = [
            file("pkg/install-a.sh", script),
            file("pkg/install-b.sh", script),
            file(
                "pkg/inner/lab-ca.crt",
                "-----BEGIN CERTIFICATE-----\nSYNTHETIC\n-----END CERTIFICATE-----\n",
            ),
            file(
                "pkg/inner/lab-tls.key",
                "-----BEGIN OpenVPN Static key V1-----\nSYNTHETIC\n-----END OpenVPN Static key V1-----\n",
            ),
            ("pkg/demo.webm".into(), vec![0, 159, 146, 150]),
        ];
        let config = openvpn_from_bundle(&files).unwrap();
        assert!(config.contains("remote vpn.example.invalid 1194 udp"));
        assert!(config.lines().any(|l| l == "auth-user-pass"));
        assert!(config.contains("<ca>\n-----BEGIN CERTIFICATE-----\nSYNTHETIC"));
        assert!(config.contains("<tls-auth>\n-----BEGIN OpenVPN Static key V1-----"));
        assert!(config.contains("key-direction 1"));
        assert!(!config.contains("zenity") && !config.contains("login.conf"));
        assert_eq!(detect_import(&config), Ok(ImportFormat::OpenVpn));
    }

    #[test]
    fn inline_blocks_win_over_file_references_and_heredocs_are_read() {
        let script = "#!/bin/sh\ncat > vpn.ovpn <<'EOF'\nclient\nremote vpn.example.invalid 443 tcp\nca missing.crt\n<ca>\nINLINE\n</ca>\nEOF\n";
        let config = openvpn_from_bundle(&[file("setup.sh", script)]).unwrap();
        assert!(!config.contains("missing.crt"));
        assert!(config.contains("<ca>\nINLINE\n</ca>"));
    }

    #[test]
    fn picks_the_most_complete_per_distro_variant() {
        let base = "client\nremote vpn.example.invalid 1194 udp\n";
        let newer = format!("{base}data-ciphers-fallback AES-128-CBC\n");
        let config =
            openvpn_from_bundle(&[file("old.ovpn", base), file("new.ovpn", &newer)]).unwrap();
        assert!(config.contains("data-ciphers-fallback"));
    }

    #[test]
    fn rejects_bundles_without_or_with_conflicting_configs() {
        assert_eq!(
            openvpn_from_bundle(&[file("readme.txt", "hello")]),
            Err(DetectionError::Unknown)
        );
        assert_eq!(
            openvpn_from_bundle(&[
                file("a.ovpn", "client\nremote a.example.invalid 1194\n"),
                file("b.ovpn", "client\nremote b.example.invalid 1194\n"),
            ]),
            Err(DetectionError::Ambiguous)
        );
    }
}

fn config_lines(config: &str) -> std::collections::HashSet<&str> {
    config.lines().map(str::trim).collect()
}
