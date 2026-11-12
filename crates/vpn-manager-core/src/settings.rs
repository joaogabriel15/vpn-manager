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
                _ => {}
            }
        }
        Ok(())
    }
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
}
