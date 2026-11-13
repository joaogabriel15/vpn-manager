use crate::error;
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use vpn_manager_core::backend::{BackendError, ErrorKind};

// Avoid blocking the GUI, inherited console windows, unbounded output, and
// forwarding OS diagnostics that can contain private configuration.
pub(crate) fn run_text(program: &str, args: &[&str]) -> Result<String, BackendError> {
    String::from_utf8(run(program, args)?)
        .map_err(|_| error(ErrorKind::Failed, "Resposta do sistema não está em UTF-8."))
}

pub(crate) fn run(program: &str, args: &[&str]) -> Result<Vec<u8>, BackendError> {
    run_with_timeout(program, args, Duration::from_secs(45))
}

fn run_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<Vec<u8>, BackendError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().map_err(|_| {
        error(
            ErrorKind::Unavailable,
            "Não foi possível executar a ferramenta do sistema.",
        )
    })?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + timeout;
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(error(
                        ErrorKind::Failed,
                        "O sistema recusou a operação VPN. Verifique autenticação, permissões e o perfil nas configurações do sistema.",
                    ))
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                break Err(error(
                    ErrorKind::Timeout,
                    "A ferramenta do sistema excedeu 45 segundos. Atualize o estado antes de tentar novamente.",
                ));
            }
            Err(_) => {
                break Err(error(
                    ErrorKind::Failed,
                    "Não foi possível consultar a ferramenta do sistema.",
                ));
            }
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result?;
    // A descendant may retain its parent's pipe after the tool exits. Do not
    // wait indefinitely for EOF in that case.
    let bytes = rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| {
            error(
                ErrorKind::Timeout,
                "A resposta do sistema não foi encerrada. Atualize o estado.",
            )
        })?
        .map_err(|_| error(ErrorKind::Failed, "Falha ao ler resposta do sistema."))?;
    if bytes.len() > 1024 * 1024 {
        return Err(error(
            ErrorKind::Failed,
            "Resposta do sistema excede o limite.",
        ));
    }
    Ok(bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn process_failures_hide_private_output() {
        let error = run(
            "/bin/sh",
            &[
                "-c",
                "printf private-gateway; printf private-secret >&2; exit 1",
            ],
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Failed);
        assert!(!error.message.contains("private"));
    }

    #[test]
    fn hung_commands_are_killed_and_report_timeout() {
        let start = Instant::now();
        let error = run_with_timeout(
            "/bin/sh",
            &["-c", "exec sleep 10"],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Timeout);
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn names_are_passed_as_one_argument_without_shell_evaluation() {
        let name = "VPN ' ; $(private-gateway) 中文";
        assert_eq!(run_text("/usr/bin/printf", &["%s", name]).unwrap(), name);
    }
}
