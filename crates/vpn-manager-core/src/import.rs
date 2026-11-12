//! Conservative format detection. This is not a protocol configuration parser.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    OpenVpn,
    WireGuard,
    ManagerProfile,
}

impl ImportFormat {
    pub fn backend_name(self) -> &'static str {
        match self {
            Self::OpenVpn => "openvpn",
            Self::WireGuard => "wireguard",
            Self::ManagerProfile => "vpn-manager",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionError {
    Unknown,
    Ambiguous,
}

pub fn detect_import(text: &str) -> Result<ImportFormat, DetectionError> {
    if crate::profile::ProfileDocument::from_toml(text).is_ok() {
        return Ok(ImportFormat::ManagerProfile);
    }
    let mut interface = false;
    let mut peer = false;
    let mut openvpn = false;
    let mut inline = None;
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        // Certificate/key blocks can contain arbitrary lines: ignore their data.
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
        interface |= line == "[Interface]";
        peer |= line == "[Peer]";
        let mut words = line.split_whitespace();
        let directive = words.next().unwrap_or_default().trim_start_matches("--");
        openvpn |= matches!(directive, "client" | "tls-client")
            || (directive == "remote" && words.next().is_some());
    }
    match (openvpn, interface && peer) {
        (true, false) => Ok(ImportFormat::OpenVpn),
        (false, true) => Ok(ImportFormat::WireGuard),
        (true, true) => Err(DetectionError::Ambiguous),
        (false, false) => Err(DetectionError::Unknown),
    }
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
