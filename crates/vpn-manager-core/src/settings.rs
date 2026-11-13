use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileField {
    Name,
    Dns4,
    Search4,
    Routes4,
    Dns6,
    Search6,
    Routes6,
    Split4,
    Split6,
    IgnoreDns4,
    IgnoreDns6,
    Username,
    PluginOptions,
}

#[derive(Clone, Debug, Default)]
pub struct ProfileChanges(pub Vec<(ProfileField, String)>);

impl ProfileChanges {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in &self.0 {
            if value.contains(['\0', '\r', '\n']) {
                return Err(
                    "Campos de configuração não podem conter caracteres de controle.".into(),
                );
            }
            match field {
                ProfileField::Name if value.trim().is_empty() => {
                    return Err("Informe um nome para a VPN.".into());
                }
                ProfileField::Split4
                | ProfileField::Split6
                | ProfileField::IgnoreDns4
                | ProfileField::IgnoreDns6
                    if !matches!(value.as_str(), "yes" | "no") =>
                {
                    return Err("Opção de rede inválida.".into());
                }
                ProfileField::Dns4 | ProfileField::Dns6 => {
                    for dns in value.split([',', ' ']).filter(|v| !v.is_empty()) {
                        let ip: std::net::IpAddr =
                            dns.parse().map_err(|_| "Endereço DNS inválido.")?;
                        if matches!(field, ProfileField::Dns4) != ip.is_ipv4() {
                            return Err(
                                "A família do endereço DNS não corresponde ao campo.".into()
                            );
                        }
                    }
                }
                ProfileField::Routes4 | ProfileField::Routes6 => {
                    validate_routes(value, matches!(field, ProfileField::Routes4))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

// Validate the common route syntax without interpreting backend-specific attributes.
// Attribute support and values are checked by the installed backend.
fn validate_routes(value: &str, ipv4: bool) -> Result<(), String> {
    let invalid = || {
        "Rota inválida: use destino[/prefixo], gateway opcional e métrica opcional da mesma família IP.".to_owned()
    };
    if value.trim().is_empty() {
        return Ok(()); // Explicitly clearing the route list is supported.
    }
    for route in value.split(',') {
        let mut parts = route.split_whitespace().peekable();
        let destination = parts.next().ok_or_else(invalid)?;
        let (address, prefix) = destination
            .split_once('/')
            .map_or((destination, None), |(address, prefix)| {
                (address, Some(prefix))
            });
        let address: std::net::IpAddr = address.parse().map_err(|_| invalid())?;
        if address.is_ipv4() != ipv4 {
            return Err(invalid());
        }
        if let Some(prefix) = prefix {
            let prefix: u8 = prefix.parse().map_err(|_| invalid())?;
            if prefix > if ipv4 { 32 } else { 128 } {
                return Err(invalid());
            }
        }
        if let Some(gateway) = parts
            .peek()
            .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        {
            if gateway.is_ipv4() != ipv4 {
                return Err(invalid());
            }
            parts.next();
        }
        if let Some(metric) = parts.peek()
            && !metric.contains('=')
        {
            metric.parse::<u32>().map_err(|_| invalid())?;
            parts.next();
        }
        for attribute in parts {
            let (name, value) = attribute.split_once('=').ok_or_else(invalid)?;
            if name.is_empty() || value.is_empty() {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct OpenVpnProfile {
    pub name: String,
    pub remote: String,
    pub port: u16,
    pub username: String,
    pub ca: PathBuf,
    pub tcp: bool,
}

impl OpenVpnProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.remote.trim().is_empty() || self.port == 0 {
            return Err("Informe nome, servidor e uma porta entre 1 e 65535.".into());
        }
        if self.name.chars().any(char::is_control) || self.username.chars().any(char::is_control) {
            return Err("Nome e usuário não podem conter caracteres de controle.".into());
        }
        let ca = self.ca.to_str().ok_or("Caminho de certificado inválido.")?;
        if !self.ca.is_absolute()
            || [self.remote.as_str(), ca]
                .iter()
                .any(|v| v.contains([',', '=', '\n', '\r', '\0']))
        {
            return Err("Informe um caminho absoluto de CA; servidor e caminho não podem conter vírgula, = ou caracteres de controle.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_dns_family_and_accepts_clearing_dns() {
        assert!(
            ProfileChanges(vec![(ProfileField::Dns4, "::1".into())])
                .validate()
                .is_err()
        );
        assert!(
            ProfileChanges(vec![
                (ProfileField::Dns6, "2001:db8::53".into()),
                (ProfileField::Dns4, "".into())
            ])
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_flags_names_and_control_characters() {
        for field in [ProfileField::Split4, ProfileField::IgnoreDns6] {
            assert!(
                ProfileChanges(vec![(field, "maybe".into())])
                    .validate()
                    .is_err()
            );
        }
        assert!(
            ProfileChanges(vec![(ProfileField::Name, "  ".into())])
                .validate()
                .is_err()
        );
        assert!(
            ProfileChanges(vec![(ProfileField::Username, "a\nb".into())])
                .validate()
                .is_err()
        );
    }

    #[test]
    fn accepts_routes_with_optional_gateway_metric_and_attributes() {
        for (field, routes) in [
            (ProfileField::Routes4, ""),
            (ProfileField::Routes6, "  "),
            (ProfileField::Routes4, "192.0.2.1, 0.0.0.0/0 192.0.2.1 0"),
            (ProfileField::Routes4, "192.0.2.0/32 4294967295 table=100"),
            (ProfileField::Routes4, "192.0.2.0/24 table=100 onlink=true"),
            (
                ProfileField::Routes6,
                "::/0 2001:db8::1 10, 2001:db8::1/128",
            ),
            (ProfileField::Routes6, "2001:db8::/32 from=2001:db8:1::/48"),
        ] {
            assert!(
                ProfileChanges(vec![(field, routes.into())])
                    .validate()
                    .is_ok(),
                "{routes}"
            );
        }
    }

    #[test]
    fn rejects_malformed_routes_without_echoing_private_values() {
        for (field, routes) in [
            (ProfileField::Routes4, "private-gateway.invalid/24"),
            (ProfileField::Routes4, "192.0.2.0/33"),
            (ProfileField::Routes6, "2001:db8::/129"),
            (ProfileField::Routes4, "192.0.2.0/-1"),
            (ProfileField::Routes4, "192.0.2.0/24/1"),
            (ProfileField::Routes4, "2001:db8::/32"),
            (ProfileField::Routes6, "192.0.2.0/24"),
            (ProfileField::Routes4, "192.0.2.0/24 2001:db8::1"),
            (ProfileField::Routes6, "2001:db8::/32 192.0.2.1"),
            (ProfileField::Routes4, "192.0.2.0/24 -1"),
            (ProfileField::Routes4, "192.0.2.0/24 4294967296"),
            (ProfileField::Routes4, "192.0.2.0/24 1 2"),
            (ProfileField::Routes4, "192.0.2.0/24 table="),
            (ProfileField::Routes4, "192.0.2.0/24 =100"),
            (ProfileField::Routes4, "192.0.2.0/24,"),
            (ProfileField::Routes6, ","),
        ] {
            let error = ProfileChanges(vec![(field, routes.into())])
                .validate()
                .unwrap_err();
            assert_eq!(
                error,
                validate_routes("private-gateway.invalid", true).unwrap_err()
            );
            assert!(!error.contains("private-gateway.invalid"));
        }
    }
}
