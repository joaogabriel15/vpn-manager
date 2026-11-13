use crate::{NetworkManager, execute};
use gio::prelude::*;
use std::path::Path;
use vpn_manager_core::{
    backend::{Backend, BackendError, ErrorKind},
    import::{detect_import, validate_import_policy},
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

pub(crate) async fn ensure_engine(id: &str) -> Result<(), BackendError> {
    if require_vpn(id).await? != "vpn" {
        return Ok(());
    }
    let service = execute(&[
        "--escape",
        "no",
        "-g",
        "vpn.service-type",
        "connection",
        "show",
        "uuid",
        id,
    ])
    .await?;
    let report = NetworkManager.capabilities().await?;
    if report
        .plugins
        .iter()
        .any(|p| p.service == service.trim() && p.engine_available)
    {
        return Ok(());
    }
    Err(BackendError {
        kind: ErrorKind::Unavailable,
        message: format!(
            "Plugin/engine {} não encontrado. Instale-o pelo gerenciador de pacotes e confira Recursos VPN disponíveis.",
            vpn_manager_core::protocols::label(service.trim())
        ),
    })
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
    let format = detect_import(&text).map_err(|e| invalid(match e {
        vpn_manager_core::import::DetectionError::Unknown => "Formato não reconhecido. Selecione OpenVPN, WireGuard, Cisco PCF ou VPN Manager; para outros protocolos use o editor do Linux.",
        vpn_manager_core::import::DetectionError::Ambiguous => "O arquivo mistura indicadores de formatos VPN diferentes. Revise a configuração.",
    }))?;
    validate_import_policy(&text, format).map_err(invalid)?;
    Ok(format)
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

fn pcf_without_secrets(text: &str) -> Result<String, BackendError> {
    let data = gio::glib::KeyFile::new();
    data.load_from_data(
        text.trim_start_matches('\u{feff}'),
        gio::glib::KeyFileFlags::NONE,
    )
    .map_err(|_| invalid("Arquivo PCF inválido."))?;
    // Rebuild instead of removing keys in place: duplicate secret keys must
    // never survive serialization in any GLib version.
    let clean = gio::glib::KeyFile::new();
    for key in data
        .keys("main")
        .map_err(|_| invalid("Seção PCF principal ausente."))?
    {
        if matches!(
            key.as_str(),
            "UserPassword"
                | "enc_UserPassword"
                | "GroupPwd"
                | "enc_GroupPwd"
                | "SaveUserPassword"
                | "X-NM-SaveGroupPassword"
        ) {
            continue;
        }
        let value = data
            .value("main", &key)
            .map_err(|_| invalid("Opção PCF inválida."))?;
        clean.set_value("main", &key, &value);
    }
    clean.set_string("main", "SaveUserPassword", "0");
    clean.set_string("main", "X-NM-SaveGroupPassword", "0");
    Ok(clean.to_data().into())
}

pub(crate) async fn import(path: &Path) -> Result<(), BackendError> {
    let format = detect_file(path).await?;
    if format == vpn_manager_core::import::ImportFormat::ManagerProfile {
        let document =
            vpn_manager_core::profile::ProfileDocument::from_toml(&read_configuration(path).await?)
                .map_err(invalid)?;
        return NetworkManager.create_profile(&document).await.map(|_| ());
    }
    if format != vpn_manager_core::import::ImportFormat::WireGuard {
        let report = NetworkManager.capabilities().await?;
        let service = format!("org.freedesktop.NetworkManager.{}", format.backend_name());
        if !report
            .plugins
            .iter()
            .any(|p| p.service == service && p.configuration_available)
        {
            return Err(invalid(
                "A biblioteca de importação do plugin VPN não foi encontrada. Instale o plugin e suas ferramentas de configuração pelo gerenciador de pacotes.",
            ));
        }
    }
    // The vpnc importer can invoke cisco-decrypt with encrypted secrets as
    // arguments. Deliberately import connection settings without credentials.
    let private_pcf = if format == vpn_manager_core::import::ImportFormat::CiscoPcf {
        use std::io::Write;
        let text = pcf_without_secrets(&read_configuration(path).await?)?;
        let mut file = tempfile::Builder::new()
            .suffix(".pcf")
            .tempfile()
            .map_err(|_| invalid("Não foi possível preparar PCF privado."))?;
        file.write_all(text.as_bytes())
            .map_err(|_| invalid("Não foi possível preparar PCF privado."))?;
        Some(file)
    } else {
        None
    };
    let path = private_pcf.as_ref().map_or(path, |file| file.path());
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
    .map_err(|error| BackendError { kind: error.kind, message: "O plugin não conseguiu importar o arquivo. Confira o formato e as opções no editor nativo; o conteúdo da configuração foi omitido deste erro.".into() })
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
        file.persist_noclobber(&path)
            .map_err(|_| invalid("Não foi possível salvar a exportação. Escolha um arquivo novo; arquivos existentes não são sobrescritos."))?;
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
    fn pcf_import_removes_passwords_before_handing_the_file_to_the_plugin() {
        let source = "[main]\nHost=vpn.example.invalid\nGroupName=Synthetic\nDescription=Synthetic VPN\nUserPassword=private-duplicate\nUserPassword=private-user\nenc_UserPassword=private-encoded-user\nGroupPwd=private-group\nenc_GroupPwd=private-encoded-group\nSaveUserPassword=1\nX-NM-SaveGroupPassword=1\n";
        let clean = pcf_without_secrets(source).unwrap();
        assert!(!clean.contains("private-"));
        let data = gio::glib::KeyFile::new();
        data.load_from_data(&clean, gio::glib::KeyFileFlags::NONE)
            .unwrap();
        assert_eq!(data.string("main", "Host").unwrap(), "vpn.example.invalid");
        assert_eq!(data.string("main", "GroupName").unwrap(), "Synthetic");
        assert_eq!(data.string("main", "SaveUserPassword").unwrap(), "0");
        assert_eq!(data.string("main", "X-NM-SaveGroupPassword").unwrap(), "0");
    }

    #[test]
    #[ignore = "requires NetworkManager and permission to manage synthetic profiles; never activates tunnels"]
    fn generic_plugin_profiles_keep_their_protocol_and_fail_early_when_engine_is_absent() {
        struct Cleanup(Vec<String>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for id in &self.0 {
                    let _ = std::process::Command::new("nmcli")
                        .args(["connection", "delete", "uuid", id])
                        .output();
                }
            }
        }
        let mut cleanup = Cleanup(Vec::new());
        gio::glib::MainContext::new().block_on(async {
            for protocol in [
                "openconnect",
                "fortisslvpn",
                "strongswan",
                "libreswan",
                "l2tp",
                "sstp",
                "vpnc",
                "pptp",
                "ssh",
                "iodine",
                "synthetic-unavailable",
            ] {
                let id = gio::glib::uuid_string_random().to_string();
                let name = format!("vpn-manager-plugin-test-{id}");
                let service = format!("org.freedesktop.NetworkManager.{protocol}");
                cleanup.0.push(id.clone());
                execute(&[
                    "connection",
                    "add",
                    "type",
                    "vpn",
                    "con-name",
                    &name,
                    "connection.uuid",
                    &id,
                    "vpn-type",
                    &service,
                    "connection.autoconnect",
                    "no",
                ])
                .await
                .unwrap();
                let profile = NetworkManager
                    .profiles()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|p| p.uuid == id)
                    .unwrap();
                assert_eq!(profile.protocol, protocol);
                assert_eq!(
                    profile.state,
                    vpn_manager_core::ConnectionState::Disconnected
                );
                NetworkManager
                    .update(
                        &id,
                        &ProfileChanges(vec![(ProfileField::Dns4, "192.0.2.53".into())]),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    NetworkManager.read(&id, ProfileField::Dns4).await.unwrap(),
                    "192.0.2.53"
                );
                if protocol == "synthetic-unavailable" {
                    assert_eq!(
                        NetworkManager.connect(&id).await.unwrap_err().kind,
                        ErrorKind::Unavailable
                    );
                    assert_eq!(
                        NetworkManager.state(&id).await.unwrap(),
                        vpn_manager_core::ConnectionState::Disconnected
                    );
                }
                NetworkManager.remove(&id).await.unwrap();
            }
        });
    }

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
            NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![
                        (ProfileField::Routes4, "192.0.2.0/24 192.0.2.1 10".into()),
                        (ProfileField::Routes6, "2001:db8::/32 2001:db8::1 20".into()),
                    ]),
                )
                .await
                .unwrap();
            let routes4 = NetworkManager
                .read(&id, ProfileField::Routes4)
                .await
                .unwrap();
            let routes6 = NetworkManager
                .read(&id, ProfileField::Routes6)
                .await
                .unwrap();
            assert!(!routes4.is_empty() && !routes6.is_empty());
            NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![
                        (ProfileField::Routes4, routes4.clone()),
                        (ProfileField::Routes6, routes6.clone()),
                    ]),
                )
                .await
                .unwrap();
            let error = NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![
                        (ProfileField::Dns4, "192.0.2.99".into()),
                        (ProfileField::Routes4, "2001:db8::/32".into()),
                    ]),
                )
                .await
                .unwrap_err();
            assert_eq!(error.kind, ErrorKind::InvalidInput);
            assert_eq!(
                NetworkManager.read(&id, ProfileField::Dns4).await.unwrap(),
                "192.0.2.53"
            );
            assert_eq!(
                NetworkManager
                    .read(&id, ProfileField::Routes4)
                    .await
                    .unwrap(),
                routes4
            );
            NetworkManager
                .update(
                    &id,
                    &ProfileChanges(vec![(ProfileField::Routes4, "".into())]),
                )
                .await
                .unwrap();
            assert!(
                NetworkManager
                    .read(&id, ProfileField::Routes4)
                    .await
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                NetworkManager
                    .read(&id, ProfileField::Routes6)
                    .await
                    .unwrap(),
                routes6
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
            let original = std::fs::read(&path).unwrap();
            assert!(NetworkManager.export(&id, &path).await.is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
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
                        routes: vec!["192.0.2.0/24 unsupported-test-attribute=1".into()],
                        ..Default::default()
                    },
                    ipv6: Default::default(),
                }),
            };
            // Core validation accepts the common syntax. The daemon rejects the
            // unsupported attribute after creation, exercising actual rollback.
            document.validate().unwrap();
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
