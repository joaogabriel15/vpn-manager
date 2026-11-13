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

fn find_file(value: &str, directory: &Path, needs_exec: bool) -> bool {
    if value.is_empty() {
        return false;
    }
    let path = Path::new(value);
    let check = |path: &Path| {
        if needs_exec {
            executable(path)
        } else {
            path.is_file()
        }
    };
    if path.is_absolute() {
        return check(path);
    }
    if needs_exec && gio::glib::find_program_in_path(value).is_some() {
        return true;
    }
    [
        directory.to_owned(),
        directory.parent().unwrap_or(directory).to_owned(),
        "/usr/lib/NetworkManager".into(),
        "/usr/lib64/NetworkManager".into(),
        "/usr/lib/x86_64-linux-gnu/NetworkManager".into(),
        "/usr/lib/aarch64-linux-gnu/NetworkManager".into(),
        "/usr/libexec".into(),
    ]
    .iter()
    .any(|dir| check(&dir.join(path)))
}

pub fn valid_service(service: &str) -> bool {
    service.starts_with("org.freedesktop.NetworkManager.")
        && service.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
}

pub(crate) fn scan(directories: &[PathBuf]) -> Vec<PluginInfo> {
    let mut plugins = BTreeMap::new();
    for dir in directories {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
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
            if !valid_service(service.as_str()) {
                continue;
            }
            let Ok(name) = keyfile.string("VPN Connection", "name") else {
                continue;
            };
            let program = keyfile
                .string("VPN Connection", "program")
                .unwrap_or_default();
            let auth = keyfile.string("GNOME", "auth-dialog").unwrap_or_default();
            let configuration = keyfile.string("libnm", "plugin").unwrap_or_default();
            plugins.insert(
                service.to_string(),
                PluginInfo {
                    name: name.to_string(),
                    service: service.to_string(),
                    engine_available: find_file(program.as_str(), dir, true),
                    auth_dialog_available: find_file(auth.as_str(), dir, true),
                    configuration_available: find_file(configuration.as_str(), dir, false),
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
        let fixture = "[VPN Connection]\nname=synthetic\nservice=org.freedesktop.NetworkManager.synthetic\nprogram=/nonexistent/synthetic\n";
        std::fs::write(dir.path().join("one.name"), fixture).unwrap();
        std::fs::write(dir.path().join("two.name"), fixture).unwrap();
        std::fs::write(dir.path().join("invalid.name"), "not a keyfile").unwrap();
        let plugins = scan(&[dir.path().into()]);
        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].service,
            "org.freedesktop.NetworkManager.synthetic"
        );
        assert!(!plugins[0].engine_available);
        assert!(!plugins[0].auth_dialog_available);
    }
    #[test]
    fn finds_relative_helpers_and_configuration_without_loading_or_running_them() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let helper = dir.path().join("synthetic-helper");
        std::fs::write(&helper, "must never be executed").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(
            dir.path().join("libnm-synthetic.so"),
            "not a shared library",
        )
        .unwrap();
        std::fs::write(dir.path().join("plugin.name"), "[VPN Connection]\nname=synthetic\nservice=org.freedesktop.NetworkManager.synthetic\nprogram=synthetic-helper\n[GNOME]\nauth-dialog=synthetic-helper\n[libnm]\nplugin=libnm-synthetic.so\n").unwrap();
        std::fs::write(dir.path().join("unsafe.name"), "[VPN Connection]\nname=unsafe\nservice=org.freedesktop.NetworkManager.synthetic;command\n").unwrap();
        let plugins = scan(&[dir.path().into()]);
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].engine_available);
        assert!(plugins[0].configuration_available);
        assert!(plugins[0].auth_dialog_available);
    }
}
