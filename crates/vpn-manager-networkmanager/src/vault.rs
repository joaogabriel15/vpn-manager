//! Optional Secret Service integration through libsecret's secret-tool.
use std::path::PathBuf;
use vpn_manager_core::{
    auth::{Secret, SecretStore},
    backend::{BackendError, ErrorKind},
};

pub struct SecretService {
    tool: Option<PathBuf>,
}

pub fn profile_key(profile: &str, username: &str) -> String {
    let account = gio::glib::compute_checksum_for_string(gio::glib::ChecksumType::Sha256, username)
        .expect("SHA-256 is supported by GLib");
    format!("{profile}:{account}")
}

impl Default for SecretService {
    fn default() -> Self {
        Self {
            tool: gio::glib::find_program_in_path("secret-tool"),
        }
    }
}

impl SecretService {
    async fn run(
        &self,
        operation: &str,
        profile: &str,
        secret: Option<&Secret>,
    ) -> Result<Option<Secret>, BackendError> {
        let tool = self.tool.as_ref().ok_or_else(|| error(ErrorKind::Unavailable, "Instale libsecret/secret-tool e configure um cofre Secret Service para lembrar senhas."))?;
        if profile.is_empty() || profile.chars().any(char::is_control) {
            return Err(error(
                ErrorKind::InvalidInput,
                "Identificador de perfil inválido.",
            ));
        }
        let mut args = vec![tool.as_os_str(), std::ffi::OsStr::new(operation)];
        if operation == "store" {
            args.push(std::ffi::OsStr::new("--label=VPN Manager — OpenVPN"));
        }
        args.extend(
            [
                "application",
                "br.com.pacheco.VpnManager",
                "profile",
                profile,
                "field",
                "openvpn-password",
            ]
            .into_iter()
            .map(std::ffi::OsStr::new),
        );
        let process = gio::Subprocess::newv(
            &args,
            gio::SubprocessFlags::STDIN_PIPE
                | gio::SubprocessFlags::STDOUT_PIPE
                | gio::SubprocessFlags::STDERR_PIPE,
        )
        .map_err(|_| {
            error(
                ErrorKind::Unavailable,
                "Não foi possível abrir o cofre do sistema.",
            )
        })?;
        let (out, err) = process
            .communicate_utf8_future(secret.map(|s| s.expose().to_owned()))
            .await
            .map_err(|_| error(ErrorKind::Failed, "O cofre não concluiu a operação."))?;
        if process.is_successful() {
            return Ok(if operation == "lookup" {
                Some(Secret::new(out.map(|s| s.to_string()).unwrap_or_default()))
            } else {
                None
            });
        }
        // libsecret returns 1 with no output when no item matches lookup/clear.
        if matches!(operation, "lookup" | "clear")
            && process.has_exited()
            && process.exit_status() == 1
            && err.as_ref().is_none_or(|s| s.is_empty())
        {
            return Ok(None);
        }
        // Never propagate tool output: providers may include sensitive values.
        Err(error(
            ErrorKind::Failed,
            "Não foi possível acessar o cofre. Verifique se ele está disponível e desbloqueado.",
        ))
    }
}

fn error(kind: ErrorKind, message: &str) -> BackendError {
    BackendError {
        kind,
        message: message.into(),
    }
}

impl SecretStore for SecretService {
    fn available(&self) -> bool {
        self.tool.is_some()
    }
    async fn lookup_openvpn(&self, profile: &str) -> Result<Option<Secret>, BackendError> {
        self.run("lookup", profile, None).await
    }
    async fn save_openvpn(&self, profile: &str, secret: &Secret) -> Result<(), BackendError> {
        if secret.is_empty() || secret.expose().contains(['\n', '\r', '\0']) {
            return Err(error(
                ErrorKind::InvalidInput,
                "Senha vazia ou com caracteres inválidos.",
            ));
        }
        self.run("store", profile, Some(secret)).await.map(|_| ())
    }
    async fn clear_openvpn(&self, profile: &str) -> Result<(), BackendError> {
        self.run("clear", profile, None).await.map(|_| ())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn vault_uses_stdin_not_arguments_and_does_not_leak_tool_errors() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("secret-tool-fixture");
        std::fs::write(
            &tool,
            r#"#!/usr/bin/env python3
import pathlib, sys
assert 'synthetic-password' not in sys.argv
store = pathlib.Path(__file__).with_name('secret')
if sys.argv[1] == 'store':
    store.write_text(sys.stdin.read())
elif sys.argv[1] == 'lookup':
    if not store.exists(): sys.exit(1)
    sys.stdout.write(store.read_text())
elif sys.argv[1] == 'clear':
    store.unlink(missing_ok=True)
else:
    sys.stderr.write('synthetic-password')
    sys.exit(2)
"#,
        )
        .unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o700)).unwrap();
        let vault = SecretService { tool: Some(tool) };
        gio::glib::MainContext::new().block_on(async {
            assert!(vault.lookup_openvpn("fixture-id").await.unwrap().is_none());
            vault
                .save_openvpn("fixture-id", &Secret::new("synthetic-password".into()))
                .await
                .unwrap();
            assert_eq!(
                vault
                    .lookup_openvpn("fixture-id")
                    .await
                    .unwrap()
                    .unwrap()
                    .expose(),
                "synthetic-password"
            );
            assert!(
                !vault
                    .run("invalid", "fixture-id", None)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("synthetic-password")
            );
            vault.clear_openvpn("fixture-id").await.unwrap();
            assert!(vault.lookup_openvpn("fixture-id").await.unwrap().is_none());
        });
    }
}
