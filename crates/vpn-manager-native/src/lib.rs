//! Desktop adapters for VPN profiles managed by Windows/macOS.
//! No tunnel engines or credentials are bundled. System policy remains authoritative.
mod command;
mod macos;
mod windows;

#[cfg(feature = "gui")]
pub mod gui;

use vpn_manager_core::{
    ConnectionState, Profile,
    backend::{BackendError, ErrorKind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
}

impl Platform {
    pub fn current() -> Result<Self, BackendError> {
        if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else if cfg!(target_os = "macos") {
            Ok(Self::MacOs)
        } else {
            Err(error(
                ErrorKind::Unsupported,
                "Este adaptador requer Windows ou macOS.",
            ))
        }
    }

    pub fn profiles(self) -> Result<Vec<Profile>, BackendError> {
        let mut profiles = match self {
            Self::Windows => windows::profiles()?,
            Self::MacOs => macos::profiles()?,
        };
        vpn_manager_core::sort_profiles(&mut profiles);
        Ok(profiles)
    }

    /// Request activation; callers must read the backend state for confirmation.
    pub fn request_connect(self, id: &str) -> Result<(), BackendError> {
        let profile = self.require_profile(id)?;
        if profile.state.is_connected() {
            return Ok(());
        }
        match self {
            Self::Windows => windows::control(&profile, false),
            Self::MacOs => macos::control(&profile, false),
        }
    }

    pub fn disconnect(self, id: &str) -> Result<(), BackendError> {
        let profile = self.require_profile(id)?;
        match self {
            Self::Windows => windows::control(&profile, true),
            Self::MacOs => macos::control(&profile, true),
        }
    }

    fn require_profile(self, id: &str) -> Result<Profile, BackendError> {
        self.profiles()?
            .into_iter()
            .find(|p| p.uuid == id)
            .ok_or_else(|| {
                error(
                    ErrorKind::NotFound,
                    "Perfil VPN não encontrado. Atualize a lista.",
                )
            })
    }

    pub fn open_settings(self) -> Result<(), BackendError> {
        match self {
            Self::Windows => windows::open_settings(),
            Self::MacOs => command::run(
                "/usr/bin/open",
                &["x-apple.systempreferences:com.apple.Network-Settings.extension"],
            )
            .map(|_| ()),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Windows => {
                "VPNs nativas do usuário Windows. Configure credenciais e certificados nas configurações do sistema. Perfis globais, OpenVPN e WireGuard externos ainda não são gerenciados."
            }
            Self::MacOs => {
                "VPNs expostas pelo macOS em scutil --nc. Configure credenciais e certificados no sistema. Apps externos e extensões de túnel podem não aparecer."
            }
        }
    }
}

fn state(value: &str) -> ConnectionState {
    match value {
        "Connected" => ConnectionState::Connected,
        "Disconnected" => ConnectionState::Disconnected,
        "Connecting" => ConnectionState::Connecting,
        "Disconnecting" => ConnectionState::Disconnecting,
        _ => ConnectionState::Unknown,
    }
}

fn error(kind: ErrorKind, message: &str) -> BackendError {
    BackendError {
        kind,
        message: message.into(),
    }
}
