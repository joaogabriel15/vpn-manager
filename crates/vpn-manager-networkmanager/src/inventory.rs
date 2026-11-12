use gio::glib;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use vpn_manager_core::backend::{BackendError, BackendReport, ErrorKind, PluginInfo};

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub(crate) fn scan(directories: &[PathBuf]) -> Vec<PluginInfo> {
    let mut plugins = BTreeMap::new();
    for dir in directories {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|v| v != "name") {
                continue;
            }
            let keyfile = glib::KeyFile::new();
            if keyfile
                .load_from_file(&path, glib::KeyFileFlags::NONE)
                .is_err()
            {
                continue;
            }
            let Ok(service) = keyfile.string("VPN Connection", "service") else {
                continue;
            };
            let Ok(name) = keyfile.string("VPN Connection", "name") else {
                continue;
            };
            let program = keyfile
                .string("VPN Connection", "program")
                .unwrap_or_default();
            let auth = keyfile.string("GNOME", "auth-dialog").unwrap_or_default();
            plugins.insert(
                service.to_string(),
                PluginInfo {
                    name: name.to_string(),
                    service: service.to_string(),
                    engine_available: executable(Path::new(program.as_str())),
                    auth_dialog_available: executable(Path::new(auth.as_str())),
                },
            );
        }
    }
    plugins.into_values().collect()
}

pub(crate) async fn report() -> Result<BackendReport, BackendError> {
    let version = crate::execute(&["--version"]).await?.trim().to_owned();
    let running = crate::execute(&["-t", "-f", "RUNNING", "general"]).await?;
    let plugins = gio::spawn_blocking(|| {
        scan(&[
            "/usr/lib/NetworkManager/VPN".into(),
            "/usr/lib64/NetworkManager/VPN".into(),
            "/usr/lib/x86_64-linux-gnu/NetworkManager/VPN".into(),
            "/usr/lib/aarch64-linux-gnu/NetworkManager/VPN".into(),
            "/usr/share/NetworkManager/VPN".into(),
            "/etc/NetworkManager/VPN".into(),
        ])
    })
    .await
    .map_err(|_| BackendError {
        kind: ErrorKind::Failed,
        message: "Não foi possível consultar plugins VPN.".into(),
    })?;
    Ok(BackendReport {
        backend: "networkmanager".into(),
        version,
        running: running.trim() == "running",
        plugins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_metadata_without_executing_plugins_and_deduplicates_service() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = "[VPN Connection]\nname=synthetic\nservice=example.synthetic\nprogram=/nonexistent/synthetic\n";
        std::fs::write(dir.path().join("one.name"), fixture).unwrap();
        std::fs::write(dir.path().join("two.name"), fixture).unwrap();
        std::fs::write(dir.path().join("invalid.name"), "not a keyfile").unwrap();
        let plugins = scan(&[dir.path().into()]);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].service, "example.synthetic");
        assert!(!plugins[0].engine_available);
        assert!(!plugins[0].auth_dialog_available);
    }
}
