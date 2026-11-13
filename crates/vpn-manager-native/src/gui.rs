use crate::Platform;
use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use vpn_manager_core::Profile;

enum Action {
    Refresh,
    Connect(String),
    Disconnect(String),
    Settings,
}
type Response = Result<(Vec<Profile>, String), String>;

struct App {
    platform: Platform,
    profiles: Vec<Profile>,
    search: String,
    message: String,
    pending: Option<Receiver<Response>>,
}

impl App {
    fn submit(&mut self, ctx: &egui::Context, action: Action) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.message = "Consultando o sistema…".into();
        let ctx = ctx.clone();
        let platform = self.platform;
        std::thread::spawn(move || {
            let result = (|| {
                let message = match action {
                    Action::Refresh => String::new(),
                    Action::Connect(id) => {
                        platform.request_connect(&id).map_err(|e| e.to_string())?;
                        "Solicitação de conexão enviada. Atualize para confirmar o estado.".into()
                    }
                    Action::Disconnect(id) => {
                        platform.disconnect(&id).map_err(|e| e.to_string())?;
                        "Solicitação de desligamento enviada. Atualize para confirmar o estado."
                            .into()
                    }
                    Action::Settings => {
                        platform.open_settings().map_err(|e| e.to_string())?;
                        "Configure a VPN no sistema e atualize a lista.".into()
                    }
                };
                Ok((platform.profiles().map_err(|e| e.to_string())?, message))
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn receive(&mut self) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok((profiles, message)) => {
                            self.profiles = profiles;
                            self.message = message;
                        }
                        Err(message) => {
                            self.profiles.clear();
                            self.message = message;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.profiles.clear();
                    self.message = "A operação foi interrompida. Atualize o estado.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.receive();
        let busy = self.pending.is_some();
        let mut action = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("VPN Manager");
            ui.label(self.platform.description());
            ui.add_space(12.0);
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Buscar VPN"));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Atualizar"))
                    .clicked()
                {
                    action = Some(Action::Refresh);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Configurar VPN no sistema"))
                    .clicked()
                {
                    action = Some(Action::Settings);
                }
                if busy {
                    ui.spinner();
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.profiles.is_empty() && !busy {
                    ui.label(
                        "Nenhuma VPN disponível. Cadastre um perfil nas configurações do sistema.",
                    );
                }
                for profile in &self.profiles {
                    if !profile
                        .name
                        .to_lowercase()
                        .contains(&self.search.to_lowercase())
                    {
                        continue;
                    }
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(&profile.name).strong());
                        ui.label(format!("{} · {}", profile.protocol, profile.state.label()));
                        let active =
                            profile.state.is_connected() || profile.state.is_transitioning();
                        let text = if active { "Desconectar" } else { "Conectar" };
                        if ui
                            .add_enabled(
                                !busy
                                    && profile.state != vpn_manager_core::ConnectionState::Unknown,
                                egui::Button::new(text),
                            )
                            .clicked()
                        {
                            action = Some(if active {
                                Action::Disconnect(profile.uuid.clone())
                            } else {
                                Action::Connect(profile.uuid.clone())
                            });
                        }
                    });
                }
            });
            if !self.message.is_empty() {
                ui.separator();
                ui.label(&self.message);
            }
        });
        if let Some(action) = action {
            self.submit(ctx, action);
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let platform = Platform::current()?;
    let icon = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../../packaging/icons/vpn-manager-256.png"
    ))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 560.0])
            .with_min_inner_size([360.0, 320.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "VPN Manager",
        options,
        Box::new(move |cc| {
            let mut app = App {
                platform,
                profiles: Vec::new(),
                search: String::new(),
                message: String::new(),
                pending: None,
            };
            app.submit(&cc.egui_ctx, Action::Refresh);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> App {
        App {
            platform: Platform::Windows,
            profiles: vec![Profile {
                uuid: "synthetic-id".into(),
                name: "Synthetic".into(),
                kind: "windows-vpn".into(),
                protocol: "Ikev2".into(),
                state: vpn_manager_core::ConnectionState::Connected,
            }],
            search: String::new(),
            message: String::new(),
            pending: None,
        }
    }

    #[test]
    fn a_failed_or_interrupted_refresh_cannot_leave_a_stale_connected_indicator() {
        for interrupted in [false, true] {
            let mut app = fixture();
            let (tx, rx) = mpsc::channel();
            app.pending = Some(rx);
            if !interrupted {
                tx.send(Err("Synthetic failure".into())).unwrap();
            }
            drop(tx);
            app.receive();
            assert!(app.profiles.is_empty());
            assert!(app.pending.is_none());
            assert!(!app.message.is_empty());
        }
    }

    #[test]
    fn a_pending_operation_prevents_duplicate_connection_requests() {
        let mut app = fixture();
        let (tx, rx) = mpsc::channel();
        app.pending = Some(rx);
        app.submit(
            &egui::Context::default(),
            Action::Connect("synthetic-id".into()),
        );
        // The original receiver must remain installed; no second worker starts.
        tx.send(Ok((Vec::new(), "Original operation".into())))
            .unwrap();
        app.receive();
        assert_eq!(app.message, "Original operation");
    }
}
