use vpn_manager_native::Platform;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if args.is_empty() || matches!(args.as_slice(), ["--help" | "-h"]) {
        println!(
            "VPN Manager {}\n\nvpn-manager-cli list\nvpn-manager-cli connect <id>\nvpn-manager-cli disconnect <id>\nvpn-manager-cli settings\nvpn-manager-cli capabilities\n\nWindows/macOS: controla perfis VPN cadastrados no sistema, com credenciais já configuradas no sistema. A conexão deve ser confirmada consultando list. Não inclui engines OpenVPN/WireGuard.",
            env!("CARGO_PKG_VERSION")
        );
        return std::process::ExitCode::SUCCESS;
    }
    if args == ["--version"] {
        println!("vpn-manager-cli {}", env!("CARGO_PKG_VERSION"));
        return std::process::ExitCode::SUCCESS;
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let platform = Platform::current()?;
        match args.as_slice() {
            ["list"] => {
                for profile in platform.profiles()? {
                    println!(
                        "{}\t{}\t{}\t{}",
                        profile.uuid,
                        profile.state.label(),
                        profile.protocol.escape_default(),
                        profile.name.escape_default()
                    );
                }
            }
            ["connect", id] => {
                platform.request_connect(id)?;
                println!("Solicitação enviada. Consulte list para confirmar o estado.");
            }
            ["disconnect", id] => {
                platform.disconnect(id)?;
                println!("Solicitação enviada. Consulte list para confirmar o estado.");
            }
            ["settings"] => platform.open_settings()?,
            ["capabilities"] => println!(
                "{}\nOperações: listar, solicitar conexão, solicitar desligamento, abrir configurações. Atualização manual; sem agente MFA, bandeja ou importação nesta versão.",
                platform.description()
            ),
            _ => return Err("Argumentos inválidos. Consulte --help.".into()),
        }
        Ok(())
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
