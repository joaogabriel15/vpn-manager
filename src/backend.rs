use gtk::gio;

#[derive(Clone, Debug)]
pub struct Profile {
    pub uuid: String,
    pub name: String,
    pub kind: String,
    pub active: bool,
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
    let mut argv = vec!["env", "LC_ALL=C", "nmcli", "--wait", "40"];
    argv.extend_from_slice(args);
    let process = gio::Subprocess::newv(
        &argv.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>(),
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    )
    .map_err(|e| e.to_string())?;
    let (out, err) = process
        .communicate_utf8_future(None)
        .await
        .map_err(|e| e.to_string())?;
    if process.is_successful() {
        Ok(out.map(|s| s.to_string()).unwrap_or_default())
    } else {
        Err(err
            .map(|s| s.to_string())
            .unwrap_or_else(|| "O NetworkManager recusou a operação.".into()))
    }
}

pub async fn profiles() -> Result<Vec<Profile>, String> {
    let text = command(&["-t", "-f", "UUID,NAME,TYPE,ACTIVE", "connection", "show"]).await?;
    let mut profiles = Vec::new();
    for line in text.lines() {
        let f = fields(line);
        if f.len() == 4 && (f[2] == "vpn" || f[2] == "wireguard") {
            profiles.push(Profile {
                uuid: f[0].clone(),
                name: f[1].clone(),
                kind: f[2].clone(),
                active: f[3] == "yes",
            });
        }
    }
    profiles.sort_by_key(|p| (!p.active, p.name.to_lowercase()));
    Ok(profiles)
}

pub async fn property(uuid: &str, key: &str) -> Result<String, String> {
    command(&["--escape", "no", "-g", key, "connection", "show", uuid])
        .await
        .map(|v| v.trim_end().to_owned())
}

pub async fn connect(uuid: &str, password: Option<&str>) -> Result<String, String> {
    if let Some(password) = password {
        use std::io::Write;
        if password.contains(['\n', '\r']) {
            return Err("Senha contém uma quebra de linha inválida.".into());
        }
        // tempfile creates a private 0600 file. It lives only until nmcli completes.
        let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        writeln!(file, "vpn.secrets.password:{password}").map_err(|e| e.to_string())?;
        command(&[
            "connection",
            "up",
            "uuid",
            uuid,
            "passwd-file",
            file.path().to_str().ok_or("Caminho inválido")?,
        ])
        .await
    } else {
        command(&["connection", "up", "uuid", uuid]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    .any(|p| p.uuid == uuid && !p.active)
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
        gtk::glib::MainContext::new().block_on(future);
    }
}
