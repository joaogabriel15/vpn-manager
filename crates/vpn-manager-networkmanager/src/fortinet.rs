//! Fortinet SSL-VPN with SSO/SAML login through openfortivpn.
//!
//! NetworkManager's fortisslvpn plugin has no SAML login, so these profiles are
//! kept by the app (name and gateway only, no secrets) and the engine is run
//! with pkexec. Profiles can be discovered from the FortiClient GUI cache.

use gio::glib;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::{ChildStdin, Command, Stdio},
    sync::{Mutex, OnceLock},
};
use vpn_manager_core::{
    ConnectionState, Profile,
    backend::{BackendError, ErrorKind},
};

pub const KIND: &str = "fortinet-sso";

struct Session {
    // Dropping stdin ends the root helper below, which stops the tunnel.
    stdin: Option<ChildStdin>,
    state: ConnectionState,
}

type Listener = Box<dyn Fn() + Send + Sync>;
static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
static LAST_ERROR: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static LISTENER: OnceLock<Listener> = OnceLock::new();

fn sessions() -> std::sync::MutexGuard<'static, HashMap<String, Session>> {
    SESSIONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn notify() {
    if let Some(listener) = LISTENER.get() {
        listener();
    }
}

/// Called from any thread whenever a session changes state.
pub fn set_listener(listener: impl Fn() + Send + Sync + 'static) {
    let _ = LISTENER.set(Box::new(listener));
}

fn error(kind: ErrorKind, message: &str) -> BackendError {
    BackendError {
        kind,
        message: message.into(),
    }
}

fn store_path() -> PathBuf {
    glib::user_config_dir().join("vpn-manager/fortinet.ini")
}

fn load() -> glib::KeyFile {
    let file = glib::KeyFile::new();
    let _ = file.load_from_file(store_path(), glib::KeyFileFlags::NONE);
    file
}

fn persist(file: &glib::KeyFile) -> Result<(), BackendError> {
    let path = store_path();
    let failed = |_| {
        error(
            ErrorKind::Failed,
            "Não foi possível salvar o perfil Fortinet.",
        )
    };
    std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(failed)?;
    file.save_to_file(&path).map_err(|_| {
        error(
            ErrorKind::Failed,
            "Não foi possível salvar o perfil Fortinet.",
        )
    })
}

/// `host` or `host:port`, restricted to characters valid in a DNS name or IP.
pub fn valid_gateway(gateway: &str) -> Option<String> {
    let (host, port) = match gateway.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok().filter(|p| *p > 0)?),
        None => (gateway, 443),
    };
    let ok = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['-', '.'])
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    ok.then(|| format!("{host}:{port}"))
}

pub fn profiles() -> Vec<Profile> {
    let file = load();
    let sessions = sessions();
    file.groups()
        .iter()
        .filter_map(|id| {
            let name = file.string(id, "name").ok()?;
            Some(Profile {
                uuid: id.to_string(),
                name: name.into(),
                kind: KIND.into(),
                protocol: "fortisslvpn".into(),
                state: sessions
                    .get(id.as_str())
                    .map_or(ConnectionState::Disconnected, |s| s.state),
            })
        })
        .collect()
}

/// Save a profile; an existing one with the same name and gateway is reused.
pub fn save(name: &str, gateway: &str) -> Result<String, BackendError> {
    let gateway = valid_gateway(gateway)
        .ok_or_else(|| error(ErrorKind::InvalidInput, "Gateway Fortinet inválido."))?;
    let file = load();
    for id in file.groups() {
        if file.string(&id, "name").ok().as_deref() == Some(name)
            && file.string(&id, "gateway").ok().as_deref() == Some(gateway.as_str())
        {
            return Ok(id.to_string());
        }
    }
    let id = glib::uuid_string_random().to_string();
    file.set_string(&id, "name", name);
    file.set_string(&id, "gateway", &gateway);
    persist(&file)?;
    Ok(id)
}

pub fn remove(id: &str) -> Result<(), BackendError> {
    disconnect(id);
    let file = load();
    file.remove_group(id)
        .map_err(|_| error(ErrorKind::NotFound, "Perfil Fortinet não encontrado."))?;
    persist(&file)
}

pub fn last_error(id: &str) -> Option<String> {
    LAST_ERROR
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .remove(id)
}

pub fn disconnect(id: &str) {
    if let Some(session) = sessions().get_mut(id) {
        session.stdin.take();
        session.state = ConnectionState::Disconnecting;
    }
    notify();
}

pub fn connect(id: &str) -> Result<(), BackendError> {
    if sessions().contains_key(id) {
        return Ok(());
    }
    let file = load();
    let gateway = file
        .string(id, "gateway")
        .ok()
        .and_then(|g| valid_gateway(&g))
        .ok_or_else(|| error(ErrorKind::NotFound, "Perfil Fortinet não encontrado."))?;
    let engine = glib::find_program_in_path("openfortivpn").ok_or_else(|| {
        error(
            ErrorKind::Unavailable,
            "Instale o openfortivpn pelo gerenciador de pacotes para conectar à Fortinet com SSO.",
        )
    })?;
    // shortcut: pkexec asks to run /bin/sh as root on every connect; a packaged
    // helper with its own polkit action would give a clearer prompt.
    // Values are positional parameters, never interpolated into the script.
    let script = r#""$1" "$2" --saml-login & p=$!; read _; kill "$p" 2>/dev/null; wait "$p""#;
    let mut child = Command::new("pkexec")
        .args(["/bin/sh", "-c", script, "sh"])
        .arg(&engine)
        .arg(&gateway)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| error(ErrorKind::Unavailable, "pkexec não está disponível."))?;
    sessions().insert(
        id.to_owned(),
        Session {
            stdin: child.stdin.take(),
            state: ConnectionState::Authenticating,
        },
    );
    notify();
    let login = format!("https://{gateway}/remote/saml/start?redirect=1");
    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    let id = id.to_owned();
    std::thread::spawn(move || {
        let err_id = id.clone();
        let errors = stderr.map(|e| std::thread::spawn(move || watch(&err_id, e, None)));
        if let Some(out) = stdout {
            watch(&id, out, Some(&login));
        }
        if let Some(handle) = errors {
            let _ = handle.join();
        }
        let status = child.wait();
        let was_connected = sessions()
            .remove(&id)
            .is_some_and(|s| s.state == ConnectionState::Connected);
        if !was_connected && !status.is_ok_and(|s| s.success()) {
            let mut errors = LAST_ERROR
                .get_or_init(Default::default)
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            errors.entry(id).or_insert_with(|| {
                "A conexão Fortinet não foi concluída. Confira o login no navegador e a autorização de administrador.".into()
            });
        }
        notify();
    });
    Ok(())
}

/// Track openfortivpn's log; open the browser once the SAML listener is up.
fn watch(id: &str, stream: impl Read, login: Option<&str>) {
    let mut opened = false;
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        let state = if line.contains("Tunnel is up and running") {
            Some(ConnectionState::Connected)
        } else if line.contains("SAML") && line.contains("port") {
            if let (Some(url), false) = (login, opened) {
                opened = true;
                let _ = Command::new("xdg-open")
                    .arg(url)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
            Some(ConnectionState::Authenticating)
        } else if line.contains("Gateway certificate validation failed") {
            remember(
                id,
                "O certificado do gateway não é confiável neste computador. Confirme com a TI antes de prosseguir.",
            );
            None
        } else if line.contains("ERROR") && line.contains("ould not authenticate") {
            remember(id, "O gateway recusou o login SSO.");
            None
        } else {
            None
        };
        let Some(state) = state else { continue };
        let changed = match sessions().get_mut(id) {
            Some(session) if session.state != ConnectionState::Disconnecting => {
                session.state = state;
                true
            }
            _ => false,
        };
        if changed {
            notify();
        }
    }
}

fn remember(id: &str, message: &str) {
    if let Ok(mut errors) = LAST_ERROR.get_or_init(Default::default).lock() {
        errors.insert(id.to_owned(), message.to_owned());
    }
}

pub struct Discovered {
    pub name: String,
    pub gateway: String,
    pub sso: bool,
}

/// SSL-VPN connections shown by the FortiClient GUI, read from its local HTTP
/// cache (`/vpn?name=…` responses). Read-only; the newest entry per name wins.
pub fn discover_forticlient() -> Vec<Discovered> {
    let folder = glib::user_config_dir().join("FortiClient/Cache/Cache_Data");
    let mut found: HashMap<String, (std::time::SystemTime, Discovered)> = HashMap::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() || meta.len() > 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Some(item) = parse_cache_entry(&bytes) else {
            continue;
        };
        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        if found
            .get(&item.name)
            .is_none_or(|(when, _)| *when < modified)
        {
            found.insert(item.name.clone(), (modified, item));
        }
    }
    let mut list: Vec<_> = found.into_values().map(|(_, item)| item).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

fn parse_cache_entry(bytes: &[u8]) -> Option<Discovered> {
    let at = bytes.windows(10).position(|w| w == b"/vpn?name=")?;
    let start = at + bytes[at..].iter().position(|b| *b == b'{')?;
    let text = String::from_utf8_lossy(&bytes[start..]);
    let value: serde_json::Value = serde_json::Deserializer::from_str(&text)
        .into_iter()
        .next()?
        .ok()?;
    if value.get("type")?.as_str()? != "ssl" {
        return None;
    }
    let gateway = value
        .get("remote_gateway")?
        .as_str()?
        .split([',', ';'])
        .next()?
        .trim();
    Some(Discovered {
        name: value.get("connection_name")?.as_str()?.to_owned(),
        gateway: valid_gateway(gateway)?,
        sso: value.get("sso_enabled")?.as_bool()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_validation_rejects_injection_and_defaults_port() {
        assert_eq!(
            valid_gateway("vpn.example.invalid").as_deref(),
            Some("vpn.example.invalid:443")
        );
        assert_eq!(
            valid_gateway("vpn.example.invalid:10443").as_deref(),
            Some("vpn.example.invalid:10443")
        );
        for bad in [
            "",
            "-oProxy",
            "a b",
            "x;rm",
            "host:0",
            "host:99999",
            "$(id)",
            "h/../x",
        ] {
            assert_eq!(valid_gateway(bad), None, "{bad}");
        }
    }

    #[test]
    fn reads_connection_from_forticlient_cache_entry() {
        let entry = b"\x00\x01http://127.0.0.1:41665/vpn?name=Lab\x00{\"type\":\"ssl\",\"connection_name\":\"Lab\",\"remote_gateway\":\"vpn.example.invalid\",\"sso_enabled\":true,\"username\":\"\"}\x00trailing";
        let item = parse_cache_entry(entry).unwrap();
        assert_eq!(
            (item.name.as_str(), item.gateway.as_str(), item.sso),
            ("Lab", "vpn.example.invalid:443", true)
        );
        assert!(parse_cache_entry(b"/vpn?name=X{\"type\":\"ipsec\"}").is_none());
        assert!(parse_cache_entry(b"/vpn?name=X{\"type\":\"ssl\",\"connection_name\":\"X\",\"remote_gateway\":\"$(id)\",\"sso_enabled\":true}").is_none());
    }
}
