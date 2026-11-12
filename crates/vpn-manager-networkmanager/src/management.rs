use crate::{NetworkManager, execute};
use gio::prelude::*;
use std::path::Path;
use vpn_manager_core::{
    backend::{Backend, BackendError, ErrorKind},
    import::detect_import,
    settings::{OpenVpnProfile, ProfileChanges, ProfileField},
};

pub(crate) fn invalid(message: impl Into<String>) -> BackendError {
    BackendError {
        kind: ErrorKind::InvalidInput,
        message: message.into(),
    }
}

pub(crate) async fn require_vpn(id: &str) -> Result<String, BackendError> {
    let kind = execute(&[
        "--escape",
        "no",
        "-g",
        "connection.type",
        "connection",
        "show",
        "uuid",
        id,
    ])
    .await?;
    match kind.trim() {
        "vpn" | "wireguard" => Ok(kind.trim().into()),
        _ => Err(invalid("O identificador não pertence a um perfil VPN.")),
    }
}

pub(crate) fn field_name(field: ProfileField) -> &'static str {
    match field {
        ProfileField::Name => "connection.id",
        ProfileField::Dns4 => "ipv4.dns",
        ProfileField::Search4 => "ipv4.dns-search",
        ProfileField::Routes4 => "ipv4.routes",
        ProfileField::Dns6 => "ipv6.dns",
        ProfileField::Search6 => "ipv6.dns-search",
        ProfileField::Routes6 => "ipv6.routes",
        ProfileField::Split4 => "ipv4.never-default",
        ProfileField::Split6 => "ipv6.never-default",
        ProfileField::IgnoreDns4 => "ipv4.ignore-auto-dns",
        ProfileField::IgnoreDns6 => "ipv6.ignore-auto-dns",
        ProfileField::Username => "vpn.user-name",
        ProfileField::PluginOptions => "vpn.data",
    }
}

pub(crate) async fn read(id: &str, field: ProfileField) -> Result<String, BackendError> {
    require_vpn(id).await?;
    execute(&[
        "--escape",
        "no",
        "-g",
        field_name(field),
        "connection",
        "show",
        "uuid",
        id,
    ])
    .await
    .map(|s| s.trim_end().into())
}

pub(crate) async fn update(id: &str, changes: &ProfileChanges) -> Result<(), BackendError> {
    changes.validate().map_err(invalid)?;
    require_vpn(id).await?;
    if changes.0.is_empty() {
        return Ok(());
    }
    let mut args = vec![
        "connection".to_owned(),
        "modify".into(),
        "uuid".into(),
        id.into(),
    ];
    for (key, value) in &changes.0 {
        args.push(field_name(*key).into());
        args.push(value.clone());
    }
    execute(&args.iter().map(String::as_str).collect::<Vec<_>>())
        .await
        .map(|_| ())
}

pub(crate) async fn create(profile: &OpenVpnProfile) -> Result<String, BackendError> {
    profile.validate().map_err(invalid)?;
    let file = gio::File::for_path(&profile.ca);
    let info = file
        .query_info_future(
            "standard::type",
            gio::FileQueryInfoFlags::NONE,
            gio::glib::Priority::DEFAULT,
        )
        .await
        .map_err(|_| invalid("O certificado CA não está acessível."))?;
    if info.file_type() != gio::FileType::Regular {
        return Err(invalid("Selecione um certificado CA existente."));
    }
    let data = format!(
        "connection-type=password,remote={},port={},ca={},password-flags=2,proto-tcp={}",
        profile.remote.trim(),
        profile.port,
        profile
            .ca
            .to_str()
            .ok_or_else(|| invalid("Caminho inválido."))?,
        if profile.tcp { "yes" } else { "no" }
    );
    let id = gio::glib::uuid_string_random();
    execute(&[
        "connection",
        "add",
        "type",
        "vpn",
        "vpn-type",
        "openvpn",
        "connection.uuid",
        &id,
        "con-name",
        &profile.name,
        "ifname",
        "*",
        "vpn.data",
        &data,
        "vpn.user-name",
        &profile.username,
        "connection.autoconnect",
        "no",
    ])
    .await
    .map(|_| id.to_string())
}

pub async fn detect_file(
    path: &Path,
) -> Result<vpn_manager_core::import::ImportFormat, BackendError> {
    let text = read_configuration(path).await?;
    detect_import(&text).map_err(|e| invalid(match e {
        vpn_manager_core::import::DetectionError::Unknown => "Formato não reconhecido. Selecione uma configuração OpenVPN, WireGuard ou VPN Manager.",
        vpn_manager_core::import::DetectionError::Ambiguous => "O arquivo mistura indicadores de OpenVPN e WireGuard. Revise a configuração.",
    }))
}

async fn read_configuration(path: &Path) -> Result<String, BackendError> {
    const MAX: usize = 4 * 1024 * 1024;
    let file = gio::File::for_path(path);
    let stream = file
        .read_future(gio::glib::Priority::DEFAULT)
        .await
        .map_err(|_| invalid("Não foi possível abrir a configuração."))?;
    let mut bytes = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(
                (MAX + 1 - bytes.len()).min(65536),
                gio::glib::Priority::DEFAULT,
            )
            .await
            .map_err(|_| invalid("Não foi possível ler a configuração."))?;
        if chunk.is_empty() {
            break;
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX {
            return Err(invalid(
                "O arquivo excede o limite de 4 MiB para importação.",
            ));
        }
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| invalid("Selecione uma configuração em texto UTF-8."))?;
    Ok(text.into())
}

pub(crate) async fn import(path: &Path) -> Result<(), BackendError> {
    let format = detect_file(path).await?;
    if format == vpn_manager_core::import::ImportFormat::ManagerProfile {
        let document =
            vpn_manager_core::profile::ProfileDocument::from_toml(&read_configuration(path).await?)
                .map_err(invalid)?;
        return NetworkManager.create_profile(&document).await.map(|_| ());
    }
    execute(&[
        "connection",
        "import",
        "type",
        format.backend_name(),
        "file",
        path.to_str().ok_or_else(|| invalid("Caminho inválido."))?,
    ])
    .await
    .map(|_| ())
}

pub(crate) async fn export(id: &str, path: &Path) -> Result<(), BackendError> {
    if require_vpn(id).await? == "wireguard" {
        return Err(BackendError {
            kind: ErrorKind::Unsupported,
            message: "A exportação de WireGuard não está disponível neste adaptador.".into(),
        });
    }
    let data = execute(&["connection", "export", "uuid", id]).await?;
    let path = path.to_owned();
    gio::spawn_blocking(move || {
        use std::io::Write;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| invalid("Não foi possível criar o arquivo de exportação."))?;
        file.write_all(data.as_bytes())
            .map_err(|_| invalid("Não foi possível escrever a exportação."))?;
        file.persist(&path)
            .map_err(|_| invalid("Não foi possível salvar a exportação."))?;
        Ok(())
    })
    .await
    .map_err(|_| invalid("Falha ao salvar a exportação."))?
}

pub(crate) async fn cancel(id: &str) -> Result<(), BackendError> {
    if NetworkManager.state(id).await? == vpn_manager_core::ConnectionState::Disconnected {
        return Ok(());
    }
    NetworkManager.disconnect(id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, io::Write};

    #[test]
    #[ignore = "requires NetworkManager, OpenVPN plugin and permission to manage synthetic profiles"]
    fn typed_lifecycle_preserves_network_families_and_private_export() {
        struct Cleanup(RefCell<Vec<String>>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for id in self.0.borrow().iter() {
                    let _ = std::process::Command::new("nmcli")
                        .args(["connection", "delete", "uuid", id])
                        .output();
                }
            }
        }
        let cleanup = Cleanup(RefCell::new(Vec::new()));
        let mut ca = tempfile::NamedTempFile::new().unwrap();
        writeln!(ca, "synthetic CA fixture, never used for a tunnel").unwrap();
        let export_dir = tempfile::tempdir().unwrap();
        gio::glib::MainContext::new().block_on(async {
            let name = format!("vpn-manager-typed-{}", gio::glib::uuid_string_random());
            let id = NetworkManager
                .create(&OpenVpnProfile {
                    name: name.clone(),
                    remote: "127.0.0.1".into(),
                    port: 1,
                    username: "synthetic".into(),
                    ca: ca.path().into(),
                    tcp: false,
                })
                .await
                .unwrap();
            cleanup.0.borrow_mut().push(id.clone());
            assert_eq!(
                NetworkManager.state(&id).await.unwrap(),
                vpn_manager_core::ConnectionState::Disconnected
            );
            assert_eq!(
                crate::property(&id, "connection.autoconnect")
                    .await
                    .unwrap(),
                "no"
            );
            NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![
                        (ProfileField::Dns4, "192.0.2.53".into()),
                        (ProfileField::Dns6, "2001:db8::53".into()),
                        (ProfileField::Split4, "yes".into()),
                        (ProfileField::Split6, "no".into()),
                    ]),
                )
                .await
                .unwrap();
            NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![(ProfileField::Split4, "no".into())]),
                )
                .await
                .unwrap();
            assert_eq!(
                NetworkManager.read(&id, ProfileField::Dns6).await.unwrap(),
                "2001:db8::53"
            );
            assert_eq!(
                NetworkManager
                    .read(&id, ProfileField::Split6)
                    .await
                    .unwrap(),
                "no"
            );
            let invalid = NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![(ProfileField::Dns4, "invalid".into())]),
                )
                .await
                .unwrap_err();
            assert_eq!(invalid.kind, ErrorKind::InvalidInput);
            assert_eq!(
                NetworkManager.read(&id, ProfileField::Dns4).await.unwrap(),
                "192.0.2.53"
            );
            let copy_name = format!("{name}-copy");
            NetworkManager.duplicate(&id, &copy_name).await.unwrap();
            let copy = crate::property(&copy_name, "connection.uuid")
                .await
                .unwrap();
            cleanup.0.borrow_mut().push(copy.clone());
            assert_eq!(
                NetworkManager
                    .read(&copy, ProfileField::Dns6)
                    .await
                    .unwrap(),
                "2001:db8::53"
            );
            let path = export_dir.path().join("profile.ovpn");
            NetworkManager.export(&id, &path).await.unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
            }
            assert_eq!(
                detect_file(&path).await.unwrap(),
                vpn_manager_core::import::ImportFormat::OpenVpn
            );
            NetworkManager.cancel(&id).await.unwrap();
            let token = vpn_manager_core::backend::CancellationToken::default();
            token.cancel();
            assert_eq!(
                NetworkManager
                    .connect_cancellable(&id, &token)
                    .await
                    .unwrap_err()
                    .kind,
                ErrorKind::Cancelled
            );
            assert_eq!(
                NetworkManager.state(&id).await.unwrap(),
                vpn_manager_core::ConnectionState::Disconnected
            );
            NetworkManager.remove(&copy).await.unwrap();
            NetworkManager.remove(&id).await.unwrap();
            let rollback_name = format!("{name}-rollback");
            let document = vpn_manager_core::profile::ProfileDocument {
                version: vpn_manager_core::profile::SCHEMA_VERSION,
                name: rollback_name.clone(),
                backend: "networkmanager".into(),
                backend_id: String::new(),
                configuration: Some(vpn_manager_core::profile::TunnelConfiguration::OpenVpn {
                    remote: "127.0.0.1".into(),
                    port: 1,
                    username: String::new(),
                    ca: ca.path().into(),
                    tcp: false,
                }),
                network: Some(vpn_manager_core::profile::NetworkConfiguration {
                    ipv4: vpn_manager_core::profile::IpConfiguration {
                        routes: vec!["not-a-route".into()],
                        ..Default::default()
                    },
                    ipv6: Default::default(),
                }),
            };
            assert!(NetworkManager.create_profile(&document).await.is_err());
            let leftovers: Vec<_> = NetworkManager
                .profiles()
                .await
                .unwrap()
                .into_iter()
                .filter(|p| p.name == rollback_name)
                .collect();
            for profile in &leftovers {
                cleanup.0.borrow_mut().push(profile.uuid.clone());
            }
            assert!(
                leftovers.is_empty(),
                "new profile must be removed after network update fails"
            );
        });
    }
}
