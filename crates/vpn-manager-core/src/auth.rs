use crate::backend::BackendError;
use std::{fmt, future::Future};
use zeroize::Zeroizing;

pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

/// Vault attributes identify a profile; secrets are never command arguments.
pub trait SecretStore {
    fn available(&self) -> bool;
    fn lookup_openvpn(
        &self,
        profile: &str,
    ) -> impl Future<Output = Result<Option<Secret>, BackendError>>;
    fn save_openvpn(
        &self,
        profile: &str,
        secret: &Secret,
    ) -> impl Future<Output = Result<(), BackendError>>;
    fn clear_openvpn(&self, profile: &str) -> impl Future<Output = Result<(), BackendError>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting_never_exposes_secret() {
        let secret = Secret::new("synthetic-password".into());
        assert!(!format!("{secret:?}").contains("synthetic-password"));
    }
}
