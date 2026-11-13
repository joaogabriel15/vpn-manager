#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
fn main() -> gtk::glib::ExitCode {
    linux::run()
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    vpn_manager_native::gui::run()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn main() {
    eprintln!("Plataforma não suportada nesta versão.");
    std::process::exit(1);
}
