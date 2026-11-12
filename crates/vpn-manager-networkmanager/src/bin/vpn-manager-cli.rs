use vpn_manager_core::{backend::Backend, profile::ProfileDocument};
use vpn_manager_networkmanager::NetworkManager;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "vpn-manager-cli list\nvpn-manager-cli validate-profile <arquivo.toml>\nvpn-manager-cli create-profile <arquivo.toml>\nvpn-manager-cli capabilities\nvpn-manager-cli cancel <uuid>\nvpn-manager-cli connect <uuid>\nvpn-manager-cli disconnect <uuid>\nvpn-manager-cli save-reference <uuid> <arquivo.toml>\nvpn-manager-cli connect-reference <arquivo.toml>\n\nRequer NetworkManager. Login usa o agente de segredos do sistema. Referências não incluem configuração do túnel; documentos v2 podem criar OpenVPN básico sem credenciais."
        );
        return std::process::ExitCode::SUCCESS;
    }
    match gio::glib::MainContext::new().block_on(run(&args)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(args: &[String]) -> Result<(), String> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["capabilities"] => {
            let report = NetworkManager
                .capabilities()
                .await
                .map_err(|e| e.to_string())?;
            println!("{}\nrunning={}", report.version, report.running);
            for plugin in report.plugins {
                println!(
                    "{}\tengine={}\tauth-dialog={}",
                    plugin.name.escape_default(),
                    plugin.engine_available,
                    plugin.auth_dialog_available
                );
            }
        }
        ["cancel", id] => {
            require_vpn(id).await?;
            NetworkManager.cancel(id).await.map_err(|e| e.to_string())?;
        }
        ["validate-profile", path] => {
            read_document(path)?;
            println!("Perfil válido.");
        }
        ["create-profile", path] => {
            let document = read_document(path)?;
            let id = NetworkManager
                .create_profile(&document)
                .await
                .map_err(|e| e.to_string())?;
            println!("{id}");
        }
        ["list"] => {
            for profile in NetworkManager.profiles().await.map_err(|e| e.to_string())? {
                println!(
                    "{}\t{}\t{}",
                    profile.uuid,
                    profile.state.label(),
                    profile.name.escape_default()
                );
            }
        }
        ["connect", id] => {
            require_vpn(id).await?;
            NetworkManager
                .connect(id)
                .await
                .map_err(|e| e.to_string())?;
        }
        ["disconnect", id] => {
            require_vpn(id).await?;
            NetworkManager
                .disconnect(id)
                .await
                .map_err(|e| e.to_string())?;
        }
        ["save-reference", id, path] => {
            let profile = NetworkManager
                .profiles()
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|p| p.uuid == *id)
                .ok_or("Perfil não encontrado.")?;
            let text = ProfileDocument::reference(NetworkManager.id(), &profile).to_toml()?;
            let path = std::path::Path::new(path);
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            use std::io::Write;
            let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
            file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            file.persist_noclobber(path).map_err(|e| e.to_string())?;
        }
        ["connect-reference", path] => {
            let profile = read_document(path)?;
            if profile.configuration.is_some() {
                return Err("Use create-profile para criar uma configuração de túnel.".into());
            }
            if profile.backend != NetworkManager.id() {
                return Err("Backend da referência não está disponível neste cliente.".into());
            }
            // Validate the ID against VPN profiles, never activate arbitrary Wi-Fi/Ethernet.
            let profiles = NetworkManager.profiles().await.map_err(|e| e.to_string())?;
            if !profiles.iter().any(|p| p.uuid == profile.backend_id) {
                return Err(
                    "A referência não corresponde a uma VPN cadastrada neste sistema.".into(),
                );
            }
            NetworkManager
                .connect(&profile.backend_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        _ => return Err("Argumentos inválidos. Consulte --help.".into()),
    }
    Ok(())
}

async fn require_vpn(id: &str) -> Result<(), String> {
    if NetworkManager
        .profiles()
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .any(|p| p.uuid == id)
    {
        Ok(())
    } else {
        Err("UUID não corresponde a uma VPN cadastrada neste sistema.".into())
    }
}

fn read_document(path: &str) -> Result<ProfileDocument, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(65537)
        .read_to_string(&mut text)
        .map_err(|_| "Não foi possível ler o documento UTF-8.")?;
    ProfileDocument::from_toml(&text)
}
