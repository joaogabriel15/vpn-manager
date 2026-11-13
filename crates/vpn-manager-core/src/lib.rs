//! Shared VPN domain models, independent of the UI and platform adapters.
//!
//! Profile identifiers and kinds are opaque values supplied by the adapter.
//! This discovery model does not store credentials or represent a portable
//! configuration file. Versioned references are available in `profile`;
//! portable tunnel configurations remain a separate roadmap item.

pub mod auth;
pub mod backend;
pub mod import;
pub mod profile;
pub mod protocols;
pub mod settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Unknown,
    Disconnected,
    Preparing,
    Authenticating,
    Connecting,
    Connected,
    Reconnecting,
    Disconnecting,
    Failed,
}

impl ConnectionState {
    pub fn is_connected(self) -> bool {
        self == Self::Connected
    }

    pub fn is_transitioning(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::Authenticating
                | Self::Connecting
                | Self::Reconnecting
                | Self::Disconnecting
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Estado desconhecido",
            Self::Disconnected => "Desconectada",
            Self::Preparing => "Preparando…",
            Self::Authenticating => "Autenticando…",
            Self::Connecting => "Conectando…",
            Self::Connected => "Conectada",
            Self::Reconnecting => "Reconectando…",
            Self::Disconnecting => "Desconectando…",
            Self::Failed => "Falha na conexão",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub uuid: String,
    pub name: String,
    pub kind: String,
    pub protocol: String,
    pub state: ConnectionState,
}

/// Show active connections first, then names in case-insensitive order.
pub fn sort_profiles(profiles: &mut [Profile]) {
    profiles.sort_by_key(|profile| (!profile.state.is_connected(), profile.name.to_lowercase()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_connections_precede_names_and_equal_names_keep_adapter_order() {
        let profile = |id: &str, name: &str, active| Profile {
            uuid: id.into(),
            name: name.into(),
            kind: "adapter-specific".into(),
            protocol: "adapter-specific".into(),
            state: if active {
                ConnectionState::Connected
            } else {
                ConnectionState::Disconnected
            },
        };
        let mut profiles = vec![
            profile("1", "beta", false),
            profile("2", "Zulu", true),
            profile("3", "Alpha", false),
            profile("4", "alpha", false),
            profile("5", "alpha", true),
        ];
        sort_profiles(&mut profiles);
        assert_eq!(
            profiles.iter().map(|p| p.uuid.as_str()).collect::<Vec<_>>(),
            ["5", "2", "3", "4", "1"]
        );
    }
}
