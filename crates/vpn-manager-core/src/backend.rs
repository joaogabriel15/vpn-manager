use crate::Profile;
use crate::{
    ConnectionState,
    settings::{OpenVpnProfile, ProfileChanges, ProfileField},
};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), BackendError> {
        if self.is_cancelled() {
            Err(BackendError {
                kind: ErrorKind::Cancelled,
                message: "Conexão cancelada.".into(),
            })
        } else {
            Ok(())
        }
    }
}
use std::{fmt, future::Future};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Unavailable,
    PermissionDenied,
    Timeout,
    NotFound,
    InvalidInput,
    Unsupported,
    Failed,
    Cancelled,
}

#[derive(Debug)]
pub struct BackendError {
    pub kind: ErrorKind,
    pub message: String,
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BackendError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Discover,
    Connect,
    Disconnect,
    Cancel,
    Import,
    Create,
    Edit,
    Duplicate,
    Remove,
    Export,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInfo {
    pub name: String,
    pub service: String,
    pub engine_available: bool,
    pub auth_dialog_available: bool,
}

#[derive(Clone, Debug)]
pub struct BackendReport {
    pub backend: String,
    pub version: String,
    pub running: bool,
    pub plugins: Vec<PluginInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    ProfilesChanged,
    MonitorStopped,
}

pub trait EventSubscription {
    fn stop(&self);
}

/// Capabilities describe this adapter's implementation, not server compatibility.
/// Export availability can require a plugin-specific check at execution time.
pub trait Backend {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> impl Future<Output = Result<BackendReport, BackendError>>;
    fn subscribe(
        &self,
        callback: impl Fn(BackendEvent) + 'static,
    ) -> Result<Box<dyn EventSubscription>, BackendError>;
    fn supports(&self, operation: Operation, kind: &str) -> bool;
    fn profiles(&self) -> impl Future<Output = Result<Vec<Profile>, BackendError>>;
    fn connect(&self, id: &str) -> impl Future<Output = Result<(), BackendError>>;
    fn connect_cancellable(
        &self,
        id: &str,
        token: &CancellationToken,
    ) -> impl Future<Output = Result<(), BackendError>> {
        async move {
            token.check()?;
            let result = self.connect(id).await;
            if token.is_cancelled() {
                // Await the original activation before the final deactivation,
                // so a delayed activation cannot succeed after cancellation.
                self.cancel(id).await?;
                token.check()?;
            }
            result
        }
    }
    fn disconnect(&self, id: &str) -> impl Future<Output = Result<(), BackendError>>;
    fn cancel(&self, id: &str) -> impl Future<Output = Result<(), BackendError>>;
    fn state(&self, id: &str) -> impl Future<Output = Result<ConnectionState, BackendError>>;
    fn read(
        &self,
        id: &str,
        field: ProfileField,
    ) -> impl Future<Output = Result<String, BackendError>>;
    fn update(
        &self,
        id: &str,
        changes: &ProfileChanges,
    ) -> impl Future<Output = Result<(), BackendError>>;
    fn create(
        &self,
        profile: &OpenVpnProfile,
    ) -> impl Future<Output = Result<String, BackendError>>;
    fn create_profile(
        &self,
        document: &crate::profile::ProfileDocument,
    ) -> impl Future<Output = Result<String, BackendError>> {
        async move {
            document.validate().map_err(|message| BackendError {
                kind: ErrorKind::InvalidInput,
                message,
            })?;
            if document.backend != self.id() {
                return Err(BackendError {
                    kind: ErrorKind::Unsupported,
                    message: "Backend do documento não está disponível.".into(),
                });
            }
            let profile = document.openvpn().map_err(|message| BackendError {
                kind: ErrorKind::InvalidInput,
                message,
            })?;
            let id = self.create(&profile).await?;
            if let Some(network) = &document.network
                && let Err(error) = self.update(&id, &network.changes()).await
            {
                if let Err(cleanup) = self.remove(&id).await {
                    return Err(BackendError {
                        kind: ErrorKind::Failed,
                        message: format!(
                            "Falha ao configurar rede: {error}. O perfil {id} também não pôde ser removido: {cleanup}"
                        ),
                    });
                }
                return Err(error);
            }
            Ok(id)
        }
    }
    fn import(&self, path: &Path) -> impl Future<Output = Result<(), BackendError>>;
    fn export(&self, id: &str, path: &Path) -> impl Future<Output = Result<(), BackendError>>;
    fn duplicate(&self, id: &str, name: &str) -> impl Future<Output = Result<(), BackendError>>;
    fn remove(&self, id: &str) -> impl Future<Output = Result<(), BackendError>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        task::{Context, Poll, Waker},
    };

    struct DelayedActivation {
        token: CancellationToken,
        connected: Cell<bool>,
        started: Cell<bool>,
        fail_cancel: bool,
    }

    fn unsupported() -> BackendError {
        BackendError {
            kind: ErrorKind::Unsupported,
            message: "not implemented by test adapter".into(),
        }
    }

    macro_rules! unused_operation {
        ($name:ident ($($arg:ident: $ty:ty),*) -> $output:ty) => {
            async fn $name(&self, $($arg: $ty),*) -> Result<$output, BackendError> {
                $(let _ = $arg;)*
                Err(unsupported())
            }
        };
    }

    impl Backend for DelayedActivation {
        fn id(&self) -> &'static str {
            "test"
        }
        fn supports(&self, _: Operation, _: &str) -> bool {
            false
        }
        fn subscribe(
            &self,
            _: impl Fn(BackendEvent) + 'static,
        ) -> Result<Box<dyn EventSubscription>, BackendError> {
            Err(unsupported())
        }
        async fn connect(&self, _: &str) -> Result<(), BackendError> {
            self.started.set(true);
            self.token.cancel();
            // Simulate a daemon completing activation after cancellation arrived.
            self.connected.set(true);
            Ok(())
        }
        async fn cancel(&self, _: &str) -> Result<(), BackendError> {
            if self.fail_cancel {
                return Err(BackendError {
                    kind: ErrorKind::PermissionDenied,
                    message: "synthetic permission failure".into(),
                });
            }
            self.connected.set(false);
            Ok(())
        }
        unused_operation!(capabilities() -> BackendReport);
        unused_operation!(profiles() -> Vec<Profile>);
        unused_operation!(disconnect(id: &str) -> ());
        unused_operation!(state(id: &str) -> ConnectionState);
        unused_operation!(read(id: &str, field: ProfileField) -> String);
        unused_operation!(update(id: &str, changes: &ProfileChanges) -> ());
        unused_operation!(create(profile: &OpenVpnProfile) -> String);
        unused_operation!(import(path: &Path) -> ());
        unused_operation!(export(id: &str, path: &Path) -> ());
        unused_operation!(duplicate(id: &str, name: &str) -> ());
        unused_operation!(remove(id: &str) -> ());
    }

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(result) => result,
            Poll::Pending => panic!("synthetic adapter unexpectedly blocked"),
        }
    }

    fn fixture(fail_cancel: bool) -> DelayedActivation {
        DelayedActivation {
            token: CancellationToken::default(),
            connected: Cell::new(false),
            started: Cell::new(false),
            fail_cancel,
        }
    }

    #[test]
    fn pre_cancelled_requests_never_activate() {
        let adapter = fixture(false);
        adapter.token.cancel();
        assert_eq!(
            ready(adapter.connect_cancellable("synthetic", &adapter.token))
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
        assert!(!adapter.started.get());
    }

    #[test]
    fn late_activation_is_disconnected_before_cancellation_completes() {
        let adapter = fixture(false);
        assert_eq!(
            ready(adapter.connect_cancellable("synthetic", &adapter.token))
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
        assert!(adapter.started.get());
        assert!(!adapter.connected.get());
    }

    #[test]
    fn failed_cancellation_is_reported_without_claiming_disconnection() {
        let adapter = fixture(true);
        assert_eq!(
            ready(adapter.connect_cancellable("synthetic", &adapter.token))
                .unwrap_err()
                .kind,
            ErrorKind::PermissionDenied
        );
        assert!(adapter.connected.get());
    }
}
