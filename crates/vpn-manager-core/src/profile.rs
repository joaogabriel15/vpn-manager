//! Versioned references and basic OpenVPN configurations, without credentials.
use crate::settings::{OpenVpnProfile, ProfileChanges, ProfileField};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDocument {
    pub version: u32,
    pub name: String,
    pub backend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub backend_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<TunnelConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkConfiguration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "lowercase", deny_unknown_fields)]
pub enum TunnelConfiguration {
    OpenVpn {
        remote: String,
        port: u16,
        #[serde(default)]
        username: String,
        ca: std::path::PathBuf,
        #[serde(default)]
        tcp: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IpConfiguration {
    pub dns: Vec<String>,
    pub search: Vec<String>,
    pub routes: Vec<String>,
    pub split: bool,
    pub ignore_auto_dns: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfiguration {
    pub ipv4: IpConfiguration,
    pub ipv6: IpConfiguration,
}

impl NetworkConfiguration {
    pub fn changes(&self) -> ProfileChanges {
        let mut changes = ProfileChanges::default();
        for (ip, fields) in [
            (
                &self.ipv4,
                [
                    ProfileField::Dns4,
                    ProfileField::Search4,
                    ProfileField::Routes4,
                    ProfileField::Split4,
                    ProfileField::IgnoreDns4,
                ],
            ),
            (
                &self.ipv6,
                [
                    ProfileField::Dns6,
                    ProfileField::Search6,
                    ProfileField::Routes6,
                    ProfileField::Split6,
                    ProfileField::IgnoreDns6,
                ],
            ),
        ] {
            for (field, value) in fields.into_iter().zip([
                ip.dns.join(","),
                ip.search.join(","),
                ip.routes.join(","),
                if ip.split { "yes" } else { "no" }.into(),
                if ip.ignore_auto_dns { "yes" } else { "no" }.into(),
            ]) {
                changes.0.push((field, value));
            }
        }
        changes
    }
}

impl ProfileDocument {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SCHEMA_VERSION {
            return Err(format!("Versão de perfil não suportada: {}.", self.version));
        }
        for (name, value) in [("nome", &self.name), ("backend", &self.backend)] {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                return Err(format!("Campo de perfil inválido: {name}."));
            }
        }
        if self.backend_id.is_empty() == self.configuration.is_none() {
            return Err("O perfil deve conter uma referência ou uma configuração de túnel.".into());
        }
        if self.backend_id.chars().any(char::is_control)
            || (self.configuration.is_none() && self.backend_id.trim().is_empty())
        {
            return Err("Identificador de perfil inválido.".into());
        }
        if self.configuration.is_none() && self.network.is_some() {
            return Err("Uma referência não pode modificar a configuração de rede.".into());
        }
        if self.configuration.is_some() {
            self.openvpn()?.validate()?;
        }
        if let Some(network) = &self.network {
            network.changes().validate()?;
        }
        Ok(())
    }

    pub fn openvpn(&self) -> Result<OpenVpnProfile, String> {
        match &self.configuration {
            Some(TunnelConfiguration::OpenVpn {
                remote,
                port,
                username,
                ca,
                tcp,
            }) => Ok(OpenVpnProfile {
                name: self.name.clone(),
                remote: remote.clone(),
                port: *port,
                username: username.clone(),
                ca: ca.clone(),
                tcp: *tcp,
            }),
            None => Err("Este documento é uma referência, não uma configuração OpenVPN.".into()),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, String> {
        // Do not return parser diagnostics: source lines could contain secrets
        // mistakenly pasted into an unsupported field.
        if text.len() > 65536 {
            return Err("Documento de perfil excede 64 KiB.".into());
        }
        let mut document: Self =
            toml::from_str(text).map_err(|_| "Documento de perfil inválido.".to_owned())?;
        if document.version == 1 {
            if document.configuration.is_some() || document.network.is_some() {
                return Err("A versão 1 aceita somente referências a perfis existentes.".into());
            }
            document.version = SCHEMA_VERSION;
        }
        document.validate()?;
        Ok(document)
    }

    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string(self).map_err(|_| "Não foi possível escrever o perfil.".into())
    }
}

impl ProfileDocument {
    pub fn reference(backend: &str, profile: &crate::Profile) -> Self {
        Self {
            version: SCHEMA_VERSION,
            name: profile.name.clone(),
            backend: backend.into(),
            backend_id: profile.uuid.clone(),
            configuration: None,
            network: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> &'static str {
        "version = 1\nname = 'Synthetic VPN'\nbackend = 'networkmanager'\nbackend_id = 'synthetic-id'\n"
    }

    #[test]
    fn round_trip_retains_backend_identity() {
        let profile = ProfileDocument::from_toml(fixture()).unwrap();
        assert_eq!(
            ProfileDocument::from_toml(&profile.to_toml().unwrap()).unwrap(),
            profile
        );
    }

    #[test]
    fn rejects_future_versions_and_never_echoes_secret_fields() {
        assert!(
            ProfileDocument::from_toml(&fixture().replace("version = 1", "version = 99")).is_err()
        );
        let error =
            ProfileDocument::from_toml(&format!("{}password = 'synthetic-secret'", fixture()))
                .unwrap_err();
        assert!(!error.contains("synthetic-secret"));
        assert!(ProfileDocument::from_toml(&fixture().replace("'synthetic-id'", "''")).is_err());
    }

    #[test]
    fn migrates_old_references_without_changing_backend_identity() {
        let profile = ProfileDocument::from_toml(fixture()).unwrap();
        assert_eq!(profile.version, 2);
        assert_eq!(profile.backend_id, "synthetic-id");
        assert!(profile.configuration.is_none());
    }

    #[test]
    fn validates_portable_configuration_and_rejects_embedded_secrets() {
        let ca = toml::Value::String(
            std::env::temp_dir()
                .join("synthetic-ca.crt")
                .to_string_lossy()
                .into_owned(),
        )
        .to_string();
        let fixture = format!(
            "version=2\nname='Synthetic VPN'\nbackend='networkmanager'\n[configuration]\nprotocol='openvpn'\nremote='example.invalid'\nport=1194\nca={ca}\n"
        );
        let profile = ProfileDocument::from_toml(&fixture).unwrap();
        assert_eq!(
            ProfileDocument::from_toml(&profile.to_toml().unwrap()).unwrap(),
            profile
        );
        assert!(ProfileDocument::from_toml(&fixture.replace("1194", "0")).is_err());
        assert!(ProfileDocument::from_toml(&fixture.replace(&ca, "'relative.crt'")).is_err());
        assert!(
            ProfileDocument::from_toml(&format!("{fixture}password='synthetic-secret'\n")).is_err()
        );
        assert!(ProfileDocument::from_toml(&fixture.replace("version=2", "version=1")).is_err());
        let valid_routes = format!(
            "{fixture}\n[network.ipv4]\nroutes=['192.0.2.0/24 192.0.2.1 10']\n[network.ipv6]\nroutes=['2001:db8::/32']\n"
        );
        let document = ProfileDocument::from_toml(&valid_routes).unwrap();
        assert_eq!(
            ProfileDocument::from_toml(&document.to_toml().unwrap()).unwrap(),
            document
        );
        for invalid in ["192.0.2.0/33", "2001:db8::/32", "private-gateway.invalid"] {
            let error = ProfileDocument::from_toml(
                &valid_routes.replace("192.0.2.0/24 192.0.2.1 10", invalid),
            )
            .unwrap_err();
            assert!(!error.contains(invalid));
        }
    }
}
