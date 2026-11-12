use vpn_manager_core::backend::{Backend, BackendError, ErrorKind, Operation};
use vpn_manager_core::settings::{OpenVpnProfile, ProfileChanges, ProfileField};
use vpn_manager_core::{ConnectionState, Profile, sort_profiles};
mod events;
mod inventory;
mod management;
pub mod vault;
pub use management::detect_file;

pub struct NetworkManager;

impl Backend for NetworkManager {
    fn id(&self) -> &'static str {
        "networkmanager"
    }

    async fn capabilities(&self) -> Result<vpn_manager_core::backend::BackendReport, BackendError> {
        inventory::report().await
    }

    fn subscribe(
        &self,
        callback: impl Fn(vpn_manager_core::backend::BackendEvent) + 'static,
    ) -> Result<Box<dyn vpn_manager_core::backend::EventSubscription>, BackendError> {
        events::subscribe(callback)
    }

    fn supports(&self, operation: Operation, kind: &str) -> bool {
        match operation {
            Operation::Discover => true,
            Operation::Cancel => matches!(kind, "vpn" | "wireguard"),
            Operation::Create => kind == "openvpn",
            Operation::Import => matches!(kind, "openvpn" | "wireguard" | "vpn-manager"),
            Operation::Export => kind == "vpn",
            _ => matches!(kind, "vpn" | "wireguard"),
        }
    }

    async fn profiles(&self) -> Result<Vec<Profile>, BackendError> {
        discover_profiles().await
    }

    async fn connect(&self, id: &str) -> Result<(), BackendError> {
        management::require_vpn(id).await?;
        execute(&["connection", "up", "uuid", id]).await.map(|_| ())
    }

    async fn disconnect(&self, id: &str) -> Result<(), BackendError> {
        management::require_vpn(id).await?;
        execute(&["connection", "down", "uuid", id])
            .await
            .map(|_| ())
    }

    async fn cancel(&self, id: &str) -> Result<(), BackendError> {
        management::cancel(id).await
    }
    async fn state(&self, id: &str) -> Result<ConnectionState, BackendError> {
        let kind = management::require_vpn(id).await?;
        // Inactive profiles have no active connection state.
        let active = execute(&["-t", "-f", "UUID", "connection", "show", "--active"]).await?;
        if !active.lines().any(|uuid| uuid == id) {
            return Ok(ConnectionState::Disconnected);
        }
        active_state(id, &kind).await
    }
    async fn read(&self, id: &str, field: ProfileField) -> Result<String, BackendError> {
        management::read(id, field).await
    }
    async fn update(&self, id: &str, changes: &ProfileChanges) -> Result<(), BackendError> {
        management::update(id, changes).await
    }
    async fn create(&self, profile: &OpenVpnProfile) -> Result<String, BackendError> {
        management::create(profile).await
    }
    async fn import(&self, path: &std::path::Path) -> Result<(), BackendError> {
        management::import(path).await
    }
    async fn export(&self, id: &str, path: &std::path::Path) -> Result<(), BackendError> {
        management::export(id, path).await
    }
    async fn duplicate(&self, id: &str, name: &str) -> Result<(), BackendError> {
        ProfileChanges(vec![(ProfileField::Name, name.into())])
            .validate()
            .map_err(management::invalid)?;
        management::require_vpn(id).await?;
        execute(&["connection", "clone", "uuid", id, name])
            .await
            .map(|_| ())
    }
    async fn remove(&self, id: &str) -> Result<(), BackendError> {
        management::require_vpn(id).await?;
        execute(&["connection", "delete", "uuid", id])
            .await
            .map(|_| ())
    }
}

// GENERAL.STATE describes the active connection, not the base network device.
pub fn connection_state(value: &str) -> ConnectionState {
    match value.trim() {
        "activating" | "1" => ConnectionState::Connecting,
        "activated" | "2" => ConnectionState::Connected,
        "deactivating" | "3" => ConnectionState::Disconnecting,
        "deactivated" | "4" => ConnectionState::Disconnected,
        _ => ConnectionState::Unknown,
    }
}

// NMVpnConnectionState is separate from the general active-connection enum.
fn vpn_connection_state(value: u32) -> ConnectionState {
    match value {
        1 => ConnectionState::Preparing,
        2 => ConnectionState::Authenticating,
        3 | 4 => ConnectionState::Connecting,
        5 => ConnectionState::Connected,
        6 => ConnectionState::Failed,
        7 => ConnectionState::Disconnected,
        _ => ConnectionState::Unknown,
    }
}

fn combine_active_state(general: ConnectionState, vpn: Option<u32>) -> ConnectionState {
    if matches!(
        general,
        ConnectionState::Disconnecting | ConnectionState::Disconnected | ConnectionState::Unknown
    ) {
        return general;
    }
    match (
        general,
        vpn.map(vpn_connection_state)
            .unwrap_or(ConnectionState::Unknown),
    ) {
        (ConnectionState::Connected, ConnectionState::Connected) => ConnectionState::Connected,
        (ConnectionState::Connecting, ConnectionState::Connected | ConnectionState::Unknown) => {
            ConnectionState::Connecting
        }
        (_, other) => other,
    }
}

async fn active_state(id: &str, kind: &str) -> Result<ConnectionState, BackendError> {
    let fields = if kind == "vpn" {
        "GENERAL.STATE,GENERAL.DBUS-PATH"
    } else {
        "GENERAL.STATE"
    };
    let text = execute(&[
        "--escape",
        "no",
        "-g",
        fields,
        "connection",
        "show",
        "uuid",
        id,
    ])
    .await?;
    if text.trim().is_empty() {
        return Ok(ConnectionState::Disconnected);
    }
    let mut lines = text.lines();
    let general = connection_state(lines.next().unwrap_or_default());
    if kind != "vpn"
        || !matches!(
            general,
            ConnectionState::Connected | ConnectionState::Connecting
        )
    {
        return Ok(general);
    }
    let path = lines.next().unwrap_or_default();
    let vpn = read_vpn_state(path).await;
    Ok(combine_active_state(general, vpn))
}

async fn read_vpn_state(path: &str) -> Option<u32> {
    use gio::glib::variant::ToVariant;
    if !path
        .strip_prefix("/org/freedesktop/NetworkManager/ActiveConnection/")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let bus = gio::bus_get_future(gio::BusType::System).await.ok()?;
    let reply = bus
        .call_future(
            Some("org.freedesktop.NetworkManager"),
            path,
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(&("org.freedesktop.NetworkManager.VPN.Connection", "VpnState").to_variant()),
            Some(gio::glib::VariantTy::new("(v)").expect("constant variant type")),
            gio::DBusCallFlags::NONE,
            3000,
        )
        .await
        .ok()?;
    vpn_state_reply(&reply)
}

fn vpn_state_reply(reply: &gio::glib::Variant) -> Option<u32> {
    reply.get::<(gio::glib::Variant,)>()?.0.get::<u32>()
}

// nmcli escapes both delimiters and backslashes in terse output.
pub fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            out.last_mut().unwrap().push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    if escaped {
        out.last_mut().unwrap().push('\\');
    }
    out
}

pub async fn command(args: &[&str]) -> Result<String, String> {
    execute(args).await.map_err(|e| e.to_string())
}

pub async fn execute(args: &[&str]) -> Result<String, BackendError> {
    let mut argv = vec!["env", "LC_ALL=C", "nmcli", "--wait", "40"];
    argv.extend_from_slice(args);
    let process = gio::Subprocess::newv(
        &argv.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>(),
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    )
    .map_err(|e| BackendError {
        kind: ErrorKind::Unavailable,
        message: e.to_string(),
    })?;
    complete(process).await
}

async fn complete(process: gio::Subprocess) -> Result<String, BackendError> {
    let (out, err) = process
        .communicate_utf8_future(None)
        .await
        .map_err(|e| BackendError {
            kind: ErrorKind::Failed,
            message: e.to_string(),
        })?;
    if process.is_successful() {
        Ok(out.map(|s| s.to_string()).unwrap_or_default())
    } else {
        Err(BackendError {
            kind: if process.has_exited() {
                exit_kind(process.exit_status())
            } else {
                ErrorKind::Failed
            },
            message: err
                .map(|s| s.to_string())
                .unwrap_or_else(|| "O NetworkManager recusou a operação.".into()),
        })
    }
}

fn exit_kind(code: i32) -> ErrorKind {
    match code {
        2 => ErrorKind::InvalidInput,
        3 => ErrorKind::Timeout,
        8 | 127 => ErrorKind::Unavailable,
        10 => ErrorKind::NotFound,
        _ => ErrorKind::Failed,
    }
}

pub async fn profiles() -> Result<Vec<Profile>, String> {
    NetworkManager.profiles().await.map_err(|e| e.to_string())
}

async fn discover_profiles() -> Result<Vec<Profile>, BackendError> {
    let text = execute(&["-t", "-f", "UUID,NAME,TYPE,ACTIVE", "connection", "show"]).await?;
    let mut profiles = Vec::new();
    for line in text.lines() {
        let f = fields(line);
        if f.len() == 4 && (f[2] == "vpn" || f[2] == "wireguard") {
            profiles.push(Profile {
                uuid: f[0].clone(),
                name: f[1].clone(),
                kind: f[2].clone(),
                protocol: if f[2] == "vpn" {
                    let service = execute(&[
                        "--escape",
                        "no",
                        "-g",
                        "vpn.service-type",
                        "connection",
                        "show",
                        "uuid",
                        &f[0],
                    ])
                    .await?;
                    service
                        .trim()
                        .strip_prefix("org.freedesktop.NetworkManager.")
                        .unwrap_or(service.trim())
                        .to_owned()
                } else {
                    "wireguard".into()
                },
                state: if f[3] == "yes" {
                    active_state(&f[0], &f[2]).await?
                } else {
                    ConnectionState::Disconnected
                },
            });
        }
    }
    sort_profiles(&mut profiles);
    Ok(profiles)
}

pub async fn property(uuid: &str, key: &str) -> Result<String, String> {
    command(&["--escape", "no", "-g", key, "connection", "show", uuid])
        .await
        .map(|v| v.trim_end().to_owned())
}

async fn activate_with_password(
    uuid: &str,
    password: Option<&str>,
) -> Result<String, BackendError> {
    management::require_vpn(uuid).await?;
    if let Some(password) = password {
        if password.contains(['\n', '\r']) {
            return Err(management::invalid(
                "Senha contém uma quebra de linha inválida.",
            ));
        }
        if execute(&[
            "--escape",
            "no",
            "-g",
            "vpn.service-type",
            "connection",
            "show",
            "uuid",
            uuid,
        ])
        .await?
        .trim()
            != "org.freedesktop.NetworkManager.openvpn"
        {
            return Err(BackendError {
                kind: ErrorKind::Unsupported,
                message: "Este plugin deve autenticar pelo agente de segredos do sistema.".into(),
            });
        }
        complete(spawn_with_password(
            &["nmcli", "--wait", "40", "connection", "up", "uuid", uuid],
            password,
        )?)
        .await
    } else {
        execute(&["connection", "up", "uuid", uuid]).await
    }
}

#[cfg(target_os = "linux")]
fn spawn_with_password(argv: &[&str], password: &str) -> Result<gio::Subprocess, BackendError> {
    use std::{
        io::{Seek, Write},
        os::fd::AsRawFd,
        os::unix::fs::PermissionsExt,
    };
    if password.contains(['\n', '\r', '\0']) {
        return Err(management::invalid("Senha contém caracteres inválidos."));
    }
    // An unlinked 0600 file survives only through inherited descriptors.
    // No named credential file is left behind if the GUI exits unexpectedly.
    let mut file = tempfile::tempfile()
        .map_err(|_| management::invalid("Não foi possível criar credenciais privadas."))?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|_| {
            management::invalid("Não foi possível restringir as permissões das credenciais.")
        })?;
    writeln!(file, "vpn.secrets.password:{password}")
        .map_err(|_| management::invalid("Não foi possível escrever credenciais privadas."))?;
    file.rewind()
        .map_err(|_| management::invalid("Não foi possível preparar credenciais privadas."))?;
    let path = format!("/proc/self/fd/{}", file.as_raw_fd());
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    );
    launcher.setenv("LC_ALL", "C", true);
    launcher.take_fd(
        file.try_clone()
            .map_err(|_| management::invalid("Não foi possível transferir credenciais privadas."))?
            .into(),
        &file,
    );
    let mut args: Vec<_> = argv.iter().map(std::ffi::OsStr::new).collect();
    args.extend([
        std::ffi::OsStr::new("passwd-file"),
        std::ffi::OsStr::new(&path),
    ]);
    launcher.spawn(&args).map_err(|_| BackendError {
        kind: ErrorKind::Unavailable,
        message: "Não foi possível iniciar o cliente VPN.".into(),
    })
}

#[cfg(not(target_os = "linux"))]
fn spawn_with_password(_: &[&str], _: &str) -> Result<gio::Subprocess, BackendError> {
    Err(BackendError {
        kind: ErrorKind::Unsupported,
        message: "Credenciais por descritor requerem o adaptador Linux.".into(),
    })
}

pub async fn connect_cancellable(
    uuid: &str,
    password: Option<&str>,
    token: &vpn_manager_core::backend::CancellationToken,
) -> Result<(), BackendError> {
    token.check()?;
    let result = activate_with_password(uuid, password).await;
    if token.is_cancelled() {
        NetworkManager.cancel(uuid).await?;
        token.check()?;
    }
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_vpn_states_without_confusing_the_two_nm_enums() {
        for (value, expected) in [
            (1, ConnectionState::Preparing),
            (2, ConnectionState::Authenticating),
            (3, ConnectionState::Connecting),
            (4, ConnectionState::Connecting),
            (5, ConnectionState::Connected),
            (6, ConnectionState::Failed),
            (7, ConnectionState::Disconnected),
            (0, ConnectionState::Unknown),
            (99, ConnectionState::Unknown),
        ] {
            assert_eq!(vpn_connection_state(value), expected);
        }
    }

    #[test]
    fn requires_consistent_activation_and_preserves_transitions() {
        use ConnectionState::*;
        assert_eq!(combine_active_state(Connecting, Some(2)), Authenticating);
        assert_eq!(combine_active_state(Connecting, Some(5)), Connecting);
        assert_eq!(combine_active_state(Connected, Some(5)), Connected);
        assert_eq!(combine_active_state(Connected, None), Unknown);
        assert_eq!(combine_active_state(Connected, Some(6)), Failed);
        assert_eq!(combine_active_state(Disconnecting, Some(5)), Disconnecting);
        assert_eq!(combine_active_state(Unknown, Some(5)), Unknown);
    }

    #[test]
    fn rejects_wrong_dbus_property_types() {
        use gio::glib::variant::ToVariant;
        assert_eq!(vpn_state_reply(&(5u32.to_variant(),).to_variant()), Some(5));
        assert_eq!(
            vpn_state_reply(&("invalid".to_variant(),).to_variant()),
            None
        );
        assert_eq!(vpn_state_reply(&5u32.to_variant()), None);
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn password_is_private_anonymous_and_absent_from_arguments() {
        gio::glib::MainContext::new().block_on(async {
            let script = "import os,stat,sys; p=sys.argv[-1]; assert b'synthetic-secret' not in open('/proc/self/cmdline','rb').read(); f=open(p); assert stat.S_IMODE(os.fstat(f.fileno()).st_mode)==0o600; assert os.fstat(f.fileno()).st_nlink==0; assert f.read()=='vpn.secrets.password:synthetic-secret\\n'; print('private-fd-ok')";
            let directory = tempfile::tempdir().unwrap();
            let reader = directory.path().join("reader.py");
            std::fs::write(&reader, script).unwrap();
            let process = spawn_with_password(&["python3", reader.to_str().unwrap()], "synthetic-secret").unwrap();
            assert_eq!(complete(process).await.unwrap().trim(), "private-fd-ok");
        });
    }
    #[test]
    fn adapter_declares_unsupported_operations_and_typed_failures() {
        assert!(!NetworkManager.supports(Operation::Export, "wireguard"));
        assert!(NetworkManager.supports(Operation::Cancel, "vpn"));
        assert!(!NetworkManager.supports(Operation::Create, "fortinet"));
        assert_eq!(exit_kind(3), ErrorKind::Timeout);
        assert_eq!(exit_kind(8), ErrorKind::Unavailable);
        assert_eq!(exit_kind(10), ErrorKind::NotFound);
        assert_eq!(exit_kind(2), ErrorKind::InvalidInput);
    }
    #[test]
    fn only_backend_activation_confirmation_means_connected() {
        assert_eq!(connection_state("activating"), ConnectionState::Connecting);
        assert_eq!(connection_state("activated\n"), ConnectionState::Connected);
        assert_eq!(
            connection_state("deactivating"),
            ConnectionState::Disconnecting
        );
        assert_eq!(
            connection_state("deactivated"),
            ConnectionState::Disconnected
        );
        assert_eq!(connection_state("future-state"), ConnectionState::Unknown);
    }
    #[test]
    fn escaped_profile_names() {
        assert_eq!(
            fields(r"uuid:Empresa\: produção\\VPN:vpn:yes"),
            vec!["uuid", "Empresa: produção\\VPN", "vpn", "yes"]
        );
    }
    #[test]
    fn empty_fields() {
        assert_eq!(fields("a::vpn:no"), vec!["a", "", "vpn", "no"]);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    #[ignore = "requires a running NetworkManager and permission to manage user connections"]
    fn profile_lifecycle() {
        struct Cleanup(Vec<String>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for name in &self.0 {
                    let _ = std::process::Command::new("nmcli")
                        .args(["connection", "delete", "id", name])
                        .output();
                }
            }
        }
        let name = format!("vpn-manager-test-{}", std::process::id());
        let copy = format!("{name}-copy");
        let _cleanup = Cleanup(vec![name.clone(), copy.clone()]);
        glib_context(async {
            command(&[
                "connection",
                "add",
                "type",
                "vpn",
                "vpn-type",
                "openvpn",
                "con-name",
                &name,
                "ifname",
                "*",
                "connection.autoconnect",
                "no",
                "vpn.data",
                "connection-type=password,remote=127.0.0.1,port=1",
            ])
            .await
            .unwrap();
            let uuid = property(&name, "connection.uuid").await.unwrap();
            command(&[
                "connection",
                "modify",
                &uuid,
                "ipv4.dns",
                "10.0.0.53",
                "ipv4.never-default",
                "yes",
            ])
            .await
            .unwrap();
            assert_eq!(property(&uuid, "ipv4.dns").await.unwrap(), "10.0.0.53");
            assert_eq!(property(&uuid, "ipv4.never-default").await.unwrap(), "yes");
            assert!(
                profiles()
                    .await
                    .unwrap()
                    .iter()
                    .any(|p| p.uuid == uuid && !p.state.is_connected())
            );
            command(&["connection", "clone", &uuid, &copy])
                .await
                .unwrap();
            let copy_uuid = property(&copy, "connection.uuid").await.unwrap();
            assert_ne!(uuid, copy_uuid);
            assert_eq!(property(&copy_uuid, "ipv4.dns").await.unwrap(), "10.0.0.53");
            command(&["connection", "delete", "uuid", &copy_uuid])
                .await
                .unwrap();
            command(&["connection", "delete", "uuid", &uuid])
                .await
                .unwrap();
            assert!(
                !profiles()
                    .await
                    .unwrap()
                    .iter()
                    .any(|p| p.uuid == uuid || p.uuid == copy_uuid)
            );
        });
    }

    fn glib_context(future: impl std::future::Future<Output = ()>) {
        gio::glib::MainContext::new().block_on(future);
    }
}
