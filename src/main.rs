// Dialog widgets remain supported in GTK 4; keeping them supports GTK 4.12 desktops.
#![allow(deprecated)]
use vpn_manager_networkmanager as backend;
mod icons;
use gtk::{gio, glib, prelude::*};
use ksni::blocking::TrayMethods;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use vpn_manager_core::Profile;
use vpn_manager_core::auth::{Secret, SecretStore};
use vpn_manager_core::backend::{Backend, CancellationToken, ErrorKind, Operation};
use vpn_manager_core::settings::{OpenVpnProfile, ProfileChanges, ProfileField as Field};

#[derive(Clone)]
enum Event {
    Show,
    Quit,
    Toggle(Profile),
}
struct Tray {
    tx: async_channel::Sender<Event>,
    profiles: Vec<Profile>,
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "vpn-manager".into()
    }
    fn title(&self) -> String {
        "VPN Manager".into()
    }
    fn icon_name(&self) -> String {
        String::new()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icons::tray(self.profiles.iter().any(|p| p.state.is_connected()))
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        let active = self
            .profiles
            .iter()
            .filter(|p| p.state.is_connected())
            .count();
        let unknown = self
            .profiles
            .iter()
            .filter(|p| p.state == vpn_manager_core::ConnectionState::Unknown)
            .count();
        ksni::ToolTip {
            title: "VPN Manager".into(),
            description: if unknown > 0 {
                format!("Estado de {unknown} conexão(ões) desconhecido. Atualize a lista.")
            } else if active > 0 {
                format!("{active} conexão(ões) ativa(s)")
            } else {
                "Nenhuma VPN conectada. Clique para abrir.".into()
            },
            ..Default::default()
        }
    }
    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.tx.try_send(Event::Show);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let mut items = vec![
            StandardItem {
                label: "Abrir VPN Manager".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.try_send(Event::Show);
                }),
                ..Default::default()
            }
            .into(),
        ];
        for p in &self.profiles {
            let profile = p.clone();
            items.push(
                StandardItem {
                    label: format!(
                        "{} {}",
                        if p.state.is_connected() {
                            "Desconectar"
                        } else {
                            "Conectar"
                        },
                        p.name
                    ),
                    enabled: !p.state.is_transitioning()
                        && p.state != vpn_manager_core::ConnectionState::Unknown,
                    activate: Box::new(move |t: &mut Self| {
                        let _ = t.tx.try_send(Event::Toggle(profile.clone()));
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Sair (manter conexões)".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.try_send(Event::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

struct Ui {
    window: gtk::ApplicationWindow,
    list: gtk::Box,
    status: gtk::Label,
    summary: gtk::Label,
    spinner: gtk::Spinner,
    search: gtk::Entry,
    profiles: RefCell<Vec<Profile>>,
    tray: RefCell<Option<ksni::blocking::Handle<Tray>>>,
    busy: Cell<bool>,
    refreshing: Cell<bool>,
    pending_refresh: Cell<bool>,
    cancel: gtk::Button,
    activation: RefCell<Option<(String, CancellationToken)>>,
    monitor: RefCell<Option<Box<dyn vpn_manager_core::backend::EventSubscription>>>,
}

fn main() -> glib::ExitCode {
    // Set before GTK or the tray starts threads. Cairo avoids an idle GPU context.
    if std::env::var_os("GSK_RENDERER").is_none() {
        unsafe {
            std::env::set_var("GSK_RENDERER", "cairo");
        }
    }
    if std::env::args().any(|a| a == "--self-test") {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            std::process::exit(1);
        }));
    }
    glib::set_prgname(Some("br.com.pacheco.VpnManager"));
    glib::set_application_name("VPN Manager");
    let app = gtk::Application::builder()
        .application_id("br.com.pacheco.VpnManager")
        .flags(if std::env::args().any(|a| a == "--self-test") {
            gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::empty()
        })
        .build();
    app.connect_activate(|app| {
        if let Some(w) = app.windows().first() {
            w.present();
            return;
        }
        build(app);
    });
    // Application arguments belong to GTK; --background is consumed here.
    app.run_with_args(&["br.com.pacheco.VpnManager"])
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}
fn icon_button(text: &str, icon: &str, class: &str) -> gtk::Button {
    let button = gtk::Button::new();
    let content = row(8);
    content.set_halign(gtk::Align::Center);
    content.append(&icons::image(
        icon,
        if class == "primary" {
            "#ffffff"
        } else {
            icons::LIGHT
        },
        17,
    ));
    if !text.is_empty() {
        content.append(&gtk::Label::new(Some(text)));
    }
    button.set_child(Some(&content));
    if !text.is_empty() {
        button.update_property(&[gtk::accessible::Property::Label(text)]);
    }
    if !class.is_empty() {
        button.add_css_class(class);
    }
    button
}
fn column(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn row(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn margins(w: &impl IsA<gtk::Widget>, size: i32) {
    w.set_margin_top(size);
    w.set_margin_bottom(size);
    w.set_margin_start(size);
    w.set_margin_end(size);
}
fn launch(ui: &Rc<Ui>, f: impl std::future::Future<Output = Result<String, String>> + 'static) {
    if ui.busy.replace(true) {
        return;
    }
    ui.list.set_sensitive(false);
    ui.status.set_text("Processando solicitação…");
    ui.spinner.start();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let result = f.await;
        ui.activation.borrow_mut().take();
        ui.cancel.set_visible(false);
        ui.cancel.set_sensitive(true);
        ui.busy.set(false);
        ui.spinner.stop();
        ui.list.set_sensitive(true);
        match result {
            Ok(message) => ui.status.set_text(&message),
            Err(error) => {
                ui.status.set_text("A operação não foi concluída.");
                alert(&ui, &error).await;
            }
        }
        refresh(&ui);
    });
}
async fn alert(ui: &Ui, text: &str) {
    let d = message_dialog(ui, "Não foi possível concluir", text);
    dialog_actions(&d, None, "Fechar", false);
    d.run_future().await;
    d.close();
}
async fn confirm(ui: &Ui, text: &str) -> bool {
    let destructive = text.starts_with("Remover");
    let d = message_dialog(
        ui,
        if destructive {
            "Remover VPN"
        } else {
            "Confirmar"
        },
        text,
    );
    dialog_actions(
        &d,
        Some("Cancelar"),
        if destructive { "Remover" } else { "Continuar" },
        destructive,
    );
    let answer = d.run_future().await == gtk::ResponseType::Accept;
    d.close();
    answer
}
fn message_dialog(ui: &Ui, title: &str, text: &str) -> gtk::Dialog {
    let d = gtk::Dialog::builder()
        .title(title)
        .transient_for(&ui.window)
        .modal(true)
        .default_width(440)
        .default_height(240)
        .build();
    modal_header(&d, title);
    let body = label(text, "");
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    body.set_max_width_chars(55);
    body.set_selectable(true);
    body.set_valign(gtk::Align::Start);
    margins(&body, 20);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&body)
        .build();
    d.content_area().append(&scroll);
    d
}
fn dialog(ui: &Ui, title: &str) -> (gtk::Dialog, gtk::Box) {
    let d = gtk::Dialog::builder()
        .title(title)
        .transient_for(&ui.window)
        .modal(true)
        .default_width(560)
        .default_height(if title.starts_with("Conectar") {
            360
        } else {
            620
        })
        .build();
    let action_text = if title.starts_with("Conectar") {
        "Conectar"
    } else if title.starts_with("Configurar") {
        "Salvar alterações"
    } else {
        "Criar VPN"
    };
    modal_header(&d, title);

    let content = column(10);
    margins(&content, 20);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    d.content_area().append(&scroll);
    dialog_actions(&d, Some("Cancelar"), action_text, false);
    (d, content)
}
fn modal_header(d: &gtk::Dialog, title: &str) {
    let header = gtk::HeaderBar::new();
    header.set_show_title_buttons(false);
    let heading = label(title, "section-title");
    heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
    heading.set_max_width_chars(30);
    header.set_title_widget(Some(&heading));
    header.pack_start(&icons::brand(22));
    let close = icon_button("", "close", "ghost");
    close.add_css_class("window-control");
    close.set_tooltip_text(Some("Cancelar e fechar"));
    close.update_property(&[gtk::accessible::Property::Label("Cancelar e fechar")]);
    let weak = d.downgrade();
    close.connect_clicked(move |_| {
        if let Some(d) = weak.upgrade() {
            d.response(gtk::ResponseType::Cancel);
        }
    });
    header.pack_end(&close);
    d.set_titlebar(Some(&header));
}

/// Keep actions outside the scrolling form, with room for focus outlines.
/// FlowBox wraps the buttons instead of forcing a wide minimum dialog size.
fn dialog_actions(d: &gtk::Dialog, cancel: Option<&str>, primary: &str, destructive: bool) {
    let footer = column(0);
    footer.add_css_class("modal-footer");
    let actions = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(2)
        .column_spacing(10)
        .row_spacing(8)
        .halign(gtk::Align::End)
        .build();
    for (text, response) in cancel
        .into_iter()
        .map(|text| (text, gtk::ResponseType::Cancel))
        .chain(std::iter::once((primary, gtk::ResponseType::Accept)))
    {
        let button = gtk::Button::with_label(text);
        button.add_css_class("modal-action");
        button.set_hexpand(true);
        if let Some(label) = button.child().and_then(|w| w.downcast::<gtk::Label>().ok()) {
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
        }
        if response == gtk::ResponseType::Accept {
            button.add_css_class(if destructive { "danger" } else { "primary" });
            d.set_default_widget(Some(&button));
        }
        let weak = d.downgrade();
        button.connect_clicked(move |_| {
            if let Some(d) = weak.upgrade() {
                d.response(response);
            }
        });
        actions.insert(&button, -1);
    }
    footer.append(&actions);
    d.content_area().append(&footer);
    let escape = gtk::EventControllerKey::new();
    let weak = d.downgrade();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(d) = weak.upgrade() {
                d.response(gtk::ResponseType::Cancel);
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    d.add_controller(escape);
}

fn entry(content: &gtk::Box, label: &str, value: &str) -> gtk::Entry {
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    l.add_css_class("form-label");
    l.set_wrap(true);
    content.append(&l);
    let e = gtk::Entry::builder()
        .text(value)
        .hexpand(true)
        .activates_default(true)
        .build();
    e.update_property(&[gtk::accessible::Property::Label(label)]);
    l.set_mnemonic_widget(Some(&e));
    content.append(&e);
    e
}

fn build(app: &gtk::Application) {
    gtk::Window::set_default_icon_name("vpn-manager");
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Display Linux indisponível"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("VPN Manager")
        .default_width(700)
        .default_height(470)
        .icon_name("vpn-manager")
        .hide_on_close(true)
        .build();
    window.connect_realize(|window| {
        if let Some(surface) = window
            .surface()
            .and_then(|surface| surface.dynamic_cast::<gtk::gdk::Toplevel>().ok())
        {
            surface.set_icon_list(&[icons::application_texture()]);
        }
    });
    let titlebar = gtk::HeaderBar::new();
    titlebar.set_show_title_buttons(false);
    titlebar.set_title_widget(Some(&label("VPN Manager", "small")));
    titlebar.pack_start(&icons::brand(19));
    for (name, icon, tooltip) in [
        ("close", "close", "Fechar para a bandeja"),
        ("maximize", "maximize", "Maximizar ou restaurar"),
        ("minimize", "minimize", "Minimizar"),
    ] {
        let tooltip = if name == "close" {
            "Fechar para a bandeja"
        } else {
            tooltip
        };
        let control = icon_button("", icon, "ghost");
        control.add_css_class("window-control");
        if name == "close" {
            control.add_css_class("close-control");
        }
        control.set_tooltip_text(Some(tooltip));
        control.update_property(&[gtk::accessible::Property::Label(tooltip)]);
        let w = window.downgrade();
        control.connect_clicked(move |_| {
            if let Some(w) = w.upgrade() {
                match name {
                    "close" => w.close(),
                    "minimize" => w.minimize(),
                    _ => {
                        if w.is_maximized() {
                            w.unmaximize();
                        } else {
                            w.maximize();
                        }
                    }
                }
            }
        });
        if name == "maximize" {
            let c = control.downgrade();
            window.connect_maximized_notify(move |w| {
                if let Some(c) = c.upgrade() {
                    c.set_child(Some(&icons::image(
                        if w.is_maximized() {
                            "restore"
                        } else {
                            "maximize"
                        },
                        icons::LIGHT,
                        15,
                    )));
                }
            });
        }
        titlebar.pack_end(&control);
    }
    window.set_titlebar(Some(&titlebar));
    let root = column(0);
    let toolbar = row(8);
    margins(&toolbar, 18);
    let search_shell = row(7);
    search_shell.add_css_class("search-shell");
    search_shell.set_hexpand(true);
    search_shell.append(&icons::image("search", icons::MUTED, 17));
    let search = gtk::Entry::builder()
        .placeholder_text("Buscar VPN")
        .hexpand(true)
        .build();
    search_shell.append(&search);
    toolbar.append(&search_shell);
    let add = gtk::MenuButton::new();
    let add_label = row(7);
    add_label.append(&icons::image("plus", icons::LIGHT, 17));
    add_label.append(&label("Adicionar", ""));
    add.set_child(Some(&add_label));
    let add_popover = gtk::Popover::new();
    let add_menu = column(4);
    margins(&add_menu, 5);
    let import = icon_button("Importar arquivo…", "import", "ghost");
    let create = icon_button("Criar OpenVPN…", "plus", "ghost");
    for b in [&import, &create] {
        add_menu.append(b);
        let pop = add_popover.downgrade();
        b.connect_clicked(move |_| {
            if let Some(p) = pop.upgrade() {
                p.popdown();
            }
        });
    }
    add_popover.set_child(Some(&add_menu));
    add.set_popover(Some(&add_popover));
    toolbar.append(&add);
    let options = gtk::MenuButton::new();
    options.set_child(Some(&icons::image("more", icons::LIGHT, 19)));
    options.set_tooltip_text(Some("Opções do aplicativo"));
    options.update_property(&[gtk::accessible::Property::Label("Opções do aplicativo")]);
    let options_popover = gtk::Popover::new();
    let options_menu = column(7);
    margins(&options_menu, 8);
    let diagnostics = icon_button("Diagnóstico de rede", "activity", "ghost");
    options_menu.append(&diagnostics);
    let capabilities = icon_button("Recursos VPN disponíveis", "settings", "ghost");
    options_menu.append(&capabilities);
    let startup = gtk::CheckButton::with_label("Iniciar com o Linux");
    startup.set_active(autostart_path().exists());
    options_menu.append(&startup);
    options_menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let quit = icon_button("Sair do aplicativo", "logout", "ghost");
    let a = app.clone();
    quit.connect_clicked(move |_| a.quit());
    options_menu.append(&quit);
    let pop = options_popover.downgrade();
    diagnostics.connect_clicked(move |_| {
        if let Some(p) = pop.upgrade() {
            p.popdown();
        }
    });
    options_popover.set_child(Some(&options_menu));
    options.set_popover(Some(&options_popover));
    toolbar.append(&options);
    root.append(&toolbar);
    let section = row(8);
    section.set_margin_start(20);
    section.set_margin_end(18);
    section.set_margin_bottom(10);
    let sub = label("Carregando VPNs…", "muted");
    sub.set_hexpand(true);
    section.append(&sub);
    let reload = icon_button("", "refresh", "ghost");
    reload.set_tooltip_text(Some("Atualizar lista"));
    reload.update_property(&[gtk::accessible::Property::Label("Atualizar lista")]);
    section.append(&reload);
    root.append(&section);
    let list = column(0);
    list.add_css_class("connection-list");
    list.set_margin_start(18);
    list.set_margin_end(18);
    list.set_margin_bottom(18);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    root.append(&scroll);
    let footer = row(8);
    footer.add_css_class("footer");
    let status = label("Carregando conexões…", "muted");
    status.set_hexpand(true);
    status.set_wrap(true);
    footer.append(&status);
    let spinner = gtk::Spinner::new();
    footer.append(&spinner);
    let cancel = gtk::Button::with_label("Cancelar conexão");
    cancel.set_visible(false);
    footer.append(&cancel);
    root.append(&footer);
    window.set_child(Some(&root));
    let ui = Rc::new(Ui {
        window,
        list,
        status,
        summary: sub,
        spinner,
        search,
        profiles: RefCell::new(vec![]),
        tray: RefCell::new(None),
        busy: Cell::new(false),
        refreshing: Cell::new(false),
        pending_refresh: Cell::new(false),
        cancel,
        activation: RefCell::new(None),
        monitor: RefCell::new(None),
    });
    let u = ui.clone();
    ui.cancel.connect_clicked(move |_| {
        let Some((id, token)) = u.activation.borrow().clone() else {
            return;
        };
        token.cancel();
        u.cancel.set_sensitive(false);
        u.status.set_text("Cancelando conexão…");
        glib::spawn_future_local(async move {
            // The activation future performs a final cancellation after nmcli
            // completes, covering a request that has not reached the daemon yet.
            let _ = backend::NetworkManager.cancel(&id).await;
        });
    });
    for name in ["show", "hide", "quit"] {
        let action = gio::SimpleAction::new(name, None);
        let window = ui.window.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(window) = window.upgrade() {
                match name {
                    "show" => window.present(),
                    "hide" => window.set_visible(false),
                    _ => {
                        if let Some(app) = window.application() {
                            app.quit();
                        }
                    }
                }
            }
        });
        app.add_action(&action);
    }
    app.set_accels_for_action("app.hide", &["Escape"]);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
    let (tx, rx) = async_channel::unbounded();
    match (Tray {
        tx,
        profiles: vec![],
    })
    .spawn()
    {
        Ok(handle) => *ui.tray.borrow_mut() = Some(handle),
        Err(e) => {
            ui.window.set_hide_on_close(false);
            ui.status.set_text(&format!(
                "Bandeja indisponível: {e}. Fechar encerrará o app."
            ));
        }
    }
    let u = ui.clone();
    glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::Show => u.window.present(),
                Event::Quit => {
                    if let Some(a) = u.window.application() {
                        a.quit();
                    }
                }
                Event::Toggle(p) => toggle(&u, p),
            }
        }
    });
    let u = ui.clone();
    ui.search.connect_changed(move |_| render(&u));
    let u = ui.clone();
    reload.connect_clicked(move |_| refresh(&u));
    let u = ui.clone();
    import.connect_clicked(move |_| import_profile(&u));
    let u = ui.clone();
    create.connect_clicked(move |_| create_profile(&u));
    let u = ui.clone();
    diagnostics.connect_clicked(move |_| {
        let u2 = u.clone();
        glib::spawn_future_local(async move {
            let result = backend::command(&["-f", "GENERAL,IP4,IP6", "device", "show"]).await;
            let d = gtk::Dialog::builder()
                .title("Diagnóstico de rede")
                .transient_for(&u2.window)
                .default_width(700)
                .default_height(500)
                .build();
            modal_header(&d, "Diagnóstico de rede");
            let text = gtk::TextView::builder()
                .editable(false)
                .monospace(true)
                .build();
            text.buffer().set_text(&result.unwrap_or_else(|e| e));
            let scroll = gtk::ScrolledWindow::builder()
                .vexpand(true)
                .child(&text)
                .build();
            d.content_area().append(&scroll);
            dialog_actions(&d, None, "Fechar", false);
            d.run_future().await;
            d.close();
        });
    });
    let u = ui.clone();
    startup.connect_toggled(move |b| {
        if let Err(e) = set_autostart(b.is_active()) {
            u.status.set_text(&e.to_string());
        }
    });
    let u = ui.clone();
    capabilities.connect_clicked(move |_| {
        let u = u.clone();
        glib::spawn_future_local(async move {
            let text = match backend::NetworkManager.capabilities().await {
                Ok(report) => {
                    let mut lines = vec![report.version, format!("NetworkManager em execução: {}", if report.running { "sim" } else { "não" }), "WireGuard: suporte nativo do NetworkManager; requer suporte do sistema.".into()];
                    for plugin in report.plugins {
                        lines.push(format!("{}: engine {}; diálogo de autenticação {}", plugin.name, if plugin.engine_available { "instalada" } else { "não encontrada" }, if plugin.auth_dialog_available { "instalado" } else { "não encontrado" }));
                    }
                    lines.push("Instale plugins pelo gerenciador de pacotes do Linux. A presença do plugin não homologa um servidor ou método de login.".into());
                    lines.join("\n\n")
                }
                Err(e) => e.to_string(),
            };
            let d = message_dialog(&u, "Recursos VPN disponíveis", &text);
            dialog_actions(&d, None, "Fechar", false);
            d.run_future().await;
            d.close();
        });
    });
    refresh(&ui);
    monitor(&ui);
    if std::env::args().any(|a| a == "--self-test") {
        let u = ui.clone();
        glib::spawn_future_local(async move {
            for _ in 0..100 {
                if !u.refreshing.get() {
                    break;
                }
                glib::timeout_future(std::time::Duration::from_millis(50)).await;
            }
            assert!(!u.refreshing.get(), "consulta de perfis não terminou");
            let expected = backend::profiles()
                .await
                .expect("NetworkManager disponível");
            assert_eq!(u.profiles.borrow().len(), expected.len());
            for active in [false, true] {
                let pixmaps = icons::tray(active);
                assert_eq!(pixmaps[0].data.len(), 48 * 48 * 4);
            }
            if let Some(path) = std::env::var_os("VPN_MANAGER_PREVIEW") {
                glib::timeout_future(std::time::Duration::from_millis(250)).await;
                save_preview(u.window.upcast_ref(), &std::path::PathBuf::from(path))
                    .await
                    .expect("salvar prévia da interface");
            }
            if let Some(menu) = descendants(u.list.upcast_ref())
                .into_iter()
                .find_map(|w| w.downcast::<gtk::MenuButton>().ok())
            {
                menu.popup();
                glib::timeout_future(std::time::Duration::from_millis(50)).await;
                assert!(menu.popover().expect("menu de ações").is_visible());
                menu.popdown();
            }

            u.search.set_text("__nenhum_perfil__");
            render(&u);
            u.search.set_text("");
            render(&u);
            u.window.set_visible(false);
            assert!(!u.window.is_visible());
            u.window.present();
            assert!(u.window.is_visible());
            if let Some(p) = expected.first() {
                edit_profile(&u, p.clone());
                for _ in 0..100 {
                    let dialogs: Vec<_> = gtk::Window::list_toplevels()
                        .into_iter()
                        .filter_map(|w| w.downcast::<gtk::Dialog>().ok())
                        .filter(|d| d.is_visible())
                        .collect();
                    if let Some(d) = dialogs.first() {
                        let tabs = descendants(d.upcast_ref())
                            .into_iter()
                            .find_map(|w| w.downcast::<gtk::Notebook>().ok())
                            .expect("abas de configurações");
                        assert_eq!(tabs.n_pages(), 3);

                        if let Some(path) = std::env::var_os("VPN_MANAGER_PREVIEW") {
                            glib::timeout_future(std::time::Duration::from_millis(150)).await;
                            let path = std::path::PathBuf::from(path)
                                .with_file_name("vpn-manager-editor.png");
                            save_preview(d.upcast_ref(), &path)
                                .await
                                .expect("prévia do editor");
                            tabs.set_current_page(Some(1));
                            glib::timeout_future(std::time::Duration::from_millis(80)).await;
                            save_preview(
                                d.upcast_ref(),
                                &path.with_file_name("vpn-manager-network.png"),
                            )
                            .await
                            .expect("prévia de DNS e rotas");
                            tabs.set_current_page(Some(2));
                            glib::timeout_future(std::time::Duration::from_millis(80)).await;
                            save_preview(
                                d.upcast_ref(),
                                &path.with_file_name("vpn-manager-advanced.png"),
                            )
                            .await
                            .expect("prévia avançada");
                            tabs.set_current_page(Some(0));
                            d.set_default_size(300, 420);
                            glib::timeout_future(std::time::Duration::from_millis(180)).await;
                            save_preview(
                                d.upcast_ref(),
                                &path.with_file_name("vpn-manager-modal-narrow.png"),
                            )
                            .await
                            .expect("prévia compacta");
                        }
                        let cancel = descendants(d.upcast_ref())
                            .into_iter()
                            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
                            .find(|button| button.label().as_deref() == Some("Cancelar"))
                            .expect("botão Cancelar no rodapé");
                        cancel.emit_clicked();
                        break;
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                for _ in 0..100 {
                    if !u.busy.get() {
                        break;
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                assert!(!u.busy.get(), "cancelamento do editor não terminou");
            }
            if let Some(p) = expected.iter().find(|p| {
                p.protocol == "openvpn"
                    && p.state == vpn_manager_core::ConnectionState::Disconnected
            }) {
                toggle(&u, p.clone());
                let mut cancelled_password = None;
                for _ in 0..100 {
                    let dialog = gtk::Window::list_toplevels()
                        .into_iter()
                        .filter_map(|w| w.downcast::<gtk::Dialog>().ok())
                        .find(|d| {
                            d.is_visible() && d.title().is_some_and(|t| t.starts_with("Conectar "))
                        });
                    if let Some(d) = dialog {
                        let widgets = descendants(d.upcast_ref());
                        let password = widgets
                            .iter()
                            .find_map(|w| w.clone().downcast::<gtk::PasswordEntry>().ok())
                            .expect("campo de senha");
                        let remember = widgets
                            .iter()
                            .find_map(|w| w.clone().downcast::<gtk::CheckButton>().ok())
                            .expect("opção de cofre");
                        assert!(
                            !remember.is_active(),
                            "salvar senha exige escolha explícita"
                        );
                        if let Some(path) = std::env::var_os("VPN_MANAGER_PREVIEW") {
                            glib::timeout_future(std::time::Duration::from_millis(150)).await;
                            save_preview(
                                d.upcast_ref(),
                                &std::path::PathBuf::from(path)
                                    .with_file_name("vpn-manager-login.png"),
                            )
                            .await
                            .expect("prévia do login sem credenciais");
                        }
                        password.set_text("synthetic-ui-cancellation-fixture");
                        cancelled_password = Some(password);
                        let cancel = widgets
                            .into_iter()
                            .filter_map(|w| w.downcast::<gtk::Button>().ok())
                            .find(|b| b.label().as_deref() == Some("Cancelar"))
                            .expect("cancelar login");
                        cancel.emit_clicked();
                        break;
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                for _ in 0..100 {
                    if !u.busy.get() {
                        break;
                    }
                    glib::timeout_future(std::time::Duration::from_millis(50)).await;
                }
                assert!(!u.busy.get(), "cancelamento do login terminou");
                assert!(
                    cancelled_password
                        .expect("login foi aberto")
                        .text()
                        .is_empty(),
                    "senha transitória deve ser limpa no cancelamento"
                );
                assert!(u.activation.borrow().is_none());
                assert!(!u.cancel.is_visible());
            }
            if std::env::var_os("VPN_MANAGER_IDLE_TEST").is_some() {
                println!("UI_IDLE_READY");
                glib::timeout_future(std::time::Duration::from_secs(15)).await;
            }
            println!(
                "UI_SMOKE_OK profiles={} tray={}",
                expected.len(),
                u.tray.borrow().is_some()
            );
            if let Some(app) = u.window.application() {
                app.quit();
            }
        });
    }
    if !std::env::args().any(|a| a == "--background") || ui.tray.borrow().is_none() {
        ui.window.present();
    }
}

fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![root.clone()];
    let mut index = 0;
    while index < result.len() {
        let mut child = result[index].first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            result.push(widget);
        }
        index += 1;
    }
    result
}

// Capture only this application, without reading the user's desktop.
async fn save_preview(window: &gtk::Window, path: &std::path::Path) -> Result<(), String> {
    // A tab change may resize the native window; wait for GTK's next allocation.
    for _ in 0..20 {
        let paintable = gtk::WidgetPaintable::new(Some(window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        if let Some(node) = snapshot.to_node() {
            let renderer = window.renderer().ok_or("Renderizador indisponível")?;
            return renderer
                .render_texture(&node, None)
                .save_to_png(path)
                .map_err(|e| e.to_string());
        }
        glib::timeout_future(std::time::Duration::from_millis(50)).await;
    }
    Err("Janela ainda não desenhada".into())
}

fn refresh(ui: &Rc<Ui>) {
    if ui.refreshing.replace(true) {
        ui.pending_refresh.set(true);
        return;
    }
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match backend::profiles().await {
            Ok(p) => {
                if let Some(tray) = ui.tray.borrow().as_ref() {
                    tray.update(|t| t.profiles = p.clone());
                }
                *ui.profiles.borrow_mut() = p;
                render(&ui);
                if ui.status.text() == "Carregando conexões…" {
                    ui.status
                        .set_text("Fechar a janela mantém o aplicativo na bandeja.");
                }
            }
            Err(e) => {
                for p in ui.profiles.borrow_mut().iter_mut() {
                    p.state = vpn_manager_core::ConnectionState::Unknown;
                }
                if let Some(tray) = ui.tray.borrow().as_ref() {
                    tray.update(|t| t.profiles = ui.profiles.borrow().clone());
                }
                render(&ui);
                ui.status
                    .set_text(&format!("Não foi possível consultar o NetworkManager: {e}"));
            }
        }
        ui.refreshing.set(false);
        if ui.pending_refresh.replace(false) {
            refresh(&ui);
        }
    });
}
fn render(ui: &Rc<Ui>) {
    while let Some(child) = ui.list.first_child() {
        ui.list.remove(&child);
    }
    let query = ui.search.text().to_lowercase();
    let profiles = ui.profiles.borrow();
    let visible = profiles
        .iter()
        .any(|p| p.name.to_lowercase().contains(&query));
    if !visible {
        let empty = column(12);
        empty.add_css_class("empty");
        let icon = icons::image(
            if query.is_empty() { "shield" } else { "search" },
            icons::MUTED,
            38,
        );
        empty.append(&icon);
        let (title, description) = if !query.is_empty() {
            ("Nenhuma conexão encontrada", "Tente buscar por outro nome.")
        } else {
            (
                "Nenhuma VPN cadastrada",
                "Importe um arquivo .ovpn ou .conf, ou crie uma nova VPN.",
            )
        };
        let title = label(title, "empty-title");
        title.set_halign(gtk::Align::Center);
        empty.append(&title);
        let text = label(description, "muted");
        text.set_wrap(true);
        text.set_halign(gtk::Align::Center);
        empty.append(&text);
        ui.list.append(&empty);
    }
    for p in profiles
        .iter()
        .filter(|p| p.name.to_lowercase().contains(&query))
    {
        let card = column(0);
        card.add_css_class("connection");
        if p.state.is_connected() {
            card.add_css_class("active");
        }
        let top = row(12);
        top.append(&icons::image(
            if p.kind == "wireguard" {
                "wireguard"
            } else {
                "shield"
            },
            if p.state.is_connected() {
                icons::ACCENT
            } else {
                icons::MUTED
            },
            25,
        ));
        let info = column(4);
        info.set_hexpand(true);
        let name = label(&p.name, "profile-name");
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(30);
        name.set_tooltip_text(Some(&p.name));
        info.append(&name);
        let state = label(p.state.label(), "muted");
        if p.state.is_connected() {
            state.add_css_class("connected");
        }
        info.append(&state);
        info.append(&label(
            match p.protocol.as_str() {
                "wireguard" => "WireGuard",
                "openvpn" => "OpenVPN",
                "openconnect" => "OpenConnect",
                "strongswan" => "IPsec / strongSwan",
                "fortisslvpn" => "Fortinet SSL / plugin do sistema",
                other => other,
            },
            "muted",
        ));
        top.append(&info);
        let connect = icon_button(
            if p.state.is_connected() {
                "Desconectar"
            } else {
                "Conectar"
            },
            "power",
            if p.state.is_connected() {
                "disconnect"
            } else {
                "primary"
            },
        );
        connect.set_sensitive(
            !p.state.is_transitioning() && p.state != vpn_manager_core::ConnectionState::Unknown,
        );
        connect.set_valign(gtk::Align::Center);
        top.append(&connect);
        let u = ui.clone();
        let profile = p.clone();
        connect.connect_clicked(move |_| toggle(&u, profile.clone()));
        let actions = row(5);
        let edit = icon_button("", "settings", "ghost");
        edit.set_tooltip_text(Some("Configurar conexão"));
        edit.update_property(&[gtk::accessible::Property::Label("Configurar conexão")]);
        actions.append(&edit);
        let more = gtk::MenuButton::new();
        more.set_child(Some(&icons::image("more", icons::LIGHT, 18)));
        more.add_css_class("ghost");
        more.set_tooltip_text(Some("Mais opções"));
        more.update_property(&[gtk::accessible::Property::Label("Mais opções da conexão")]);
        let popover = gtk::Popover::new();
        let menu = column(4);
        margins(&menu, 6);
        let duplicate = icon_button("Duplicar perfil", "copy", "ghost");
        let export = icon_button("Exportar arquivo", "export", "ghost");
        export.set_sensitive(backend::NetworkManager.supports(Operation::Export, &p.kind));
        if p.kind == "wireguard" {
            export.set_tooltip_text(Some(
                "A exportação de WireGuard não está disponível nesta versão",
            ));
        }
        let delete = icon_button("Remover perfil", "trash", "danger");
        for b in [&duplicate, &export, &delete] {
            menu.append(b);
            let pop = popover.downgrade();
            b.connect_clicked(move |_| {
                if let Some(p) = pop.upgrade() {
                    p.popdown();
                }
            });
        }
        popover.set_child(Some(&menu));
        more.set_popover(Some(&popover));
        actions.append(&more);
        let u = ui.clone();
        let profile = p.clone();
        edit.connect_clicked(move |_| edit_profile(&u, profile.clone()));
        let u = ui.clone();
        let profile = p.clone();
        export.connect_clicked(move |_| export_profile(&u, profile.clone()));
        let u = ui.clone();
        let profile = p.clone();
        delete.connect_clicked(move |_| {
            let u2 = u.clone();
            let p = profile.clone();
            launch(&u, async move {
                if !confirm(
                    &u2,
                    &format!(
                        "Remover a VPN ‘{}’? O perfil será excluído do sistema{}.",
                        p.name,
                        if p.state.is_connected() {
                            " e a conexão será encerrada"
                        } else {
                            ""
                        }
                    ),
                )
                .await
                {
                    return Ok("Remoção cancelada.".into());
                }
                backend::NetworkManager
                    .remove(&p.uuid)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok("Perfil removido.".into())
            });
        });
        let u = ui.clone();
        let profile = p.clone();
        duplicate.connect_clicked(move |_| {
            let p = profile.clone();
            launch(&u, async move {
                backend::NetworkManager
                    .duplicate(&p.uuid, &format!("{} (cópia)", p.name))
                    .await
                    .map_err(|e| e.to_string())?;
                Ok("Perfil duplicado.".into())
            });
        });
        actions.set_valign(gtk::Align::Center);
        top.append(&actions);
        card.append(&top);
        ui.list.append(&card);
    }
    let active = profiles.iter().filter(|p| p.state.is_connected()).count();
    ui.summary.set_text(&format!(
        "{} {} · {} {}",
        profiles.len(),
        if profiles.len() == 1 { "VPN" } else { "VPNs" },
        active,
        if active == 1 {
            "conectada"
        } else {
            "conectadas"
        }
    ));
}

fn toggle(ui: &Rc<Ui>, p: Profile) {
    let u = ui.clone();
    launch(ui, async move {
        if p.state.is_connected() {
            backend::NetworkManager
                .disconnect(&p.uuid)
                .await
                .map_err(|e| e.to_string())?;
            return Ok("VPN desconectada.".into());
        }
        if p.protocol == "openvpn" {
            u.window.present();
            let old_user = backend::property(&p.uuid, "vpn.user-name").await?;
            let (d, content) = dialog(&u, &format!("Conectar {}", p.name));
            d.set_default_height(480);
            let user = entry(&content, "Usuário (se exigido pelo servidor)", &old_user);
            let label = gtk::Label::new(Some(
                "Senha (deixe vazia para usar certificados ou o agente do sistema)",
            ));
            label.set_wrap(true);
            content.append(&label);
            label.add_css_class("form-label");
            let password = gtk::PasswordEntry::builder()
                .show_peek_icon(true)
                .activates_default(true)
                .build();
            content.append(&password);
            let remember = gtk::CheckButton::with_label("Lembrar senha após conectar");
            let vault = backend::vault::SecretService::default();
            remember.set_sensitive(vault.available());
            if !vault.available() {
                remember.set_tooltip_text(Some(
                    "Instale libsecret/secret-tool e configure um cofre Secret Service.",
                ));
            }
            content.append(&remember);
            let vault_actions = row(6);
            let load = gtk::Button::with_label("Usar senha salva");
            let forget = gtk::Button::with_label("Esquecer senha");
            for button in [&load, &forget] {
                button.set_sensitive(vault.available());
                vault_actions.append(button);
            }
            content.append(&vault_actions);
            let vault_status = gtk::Label::new(None);
            vault_status.set_wrap(true);
            vault_status.add_css_class("muted");
            content.append(&vault_status);
            let password_copy = password.clone();
            let remember_copy = remember.clone();
            user.connect_changed(move |_| {
                password_copy.set_text("");
                remember_copy.set_active(false);
            });
            let user_copy = user.clone();
            let password_copy = password.clone();
            let remember_copy = remember.clone();
            let status_copy = vault_status.clone();
            let id = p.uuid.clone();
            let d_weak = d.downgrade();
            load.connect_clicked(move |button| {
                let button = button.clone();
                let password = password_copy.clone();
                let remember = remember_copy.clone();
                let status = status_copy.clone();
                let key = backend::vault::profile_key(&id, user_copy.text().as_str());
                let account = user_copy.text().to_string();
                let user = user_copy.clone();
                let d_weak = d_weak.clone();
                button.set_sensitive(false);
                glib::spawn_future_local(async move {
                    let result = backend::vault::SecretService::default()
                        .lookup_openvpn(&key)
                        .await;
                    if d_weak.upgrade().is_none_or(|d| !d.is_visible())
                        || user.text().as_str() != account
                    {
                        button.set_sensitive(true);
                        return;
                    }
                    match result {
                        Ok(Some(secret)) => {
                            password.set_text(secret.expose());
                            remember.set_active(true);
                            status.set_text("Senha obtida do cofre.");
                        }
                        Ok(None) => status.set_text("Nenhuma senha salva para este usuário."),
                        Err(e) => status.set_text(&e.to_string()),
                    }
                    button.set_sensitive(true);
                });
            });
            let user_copy = user.clone();
            let remember_copy = remember.clone();
            let status_copy = vault_status.clone();
            let id = p.uuid.clone();
            forget.connect_clicked(move |button| {
                let button = button.clone();
                let remember = remember_copy.clone();
                let status = status_copy.clone();
                let key = backend::vault::profile_key(&id, user_copy.text().as_str());
                button.set_sensitive(false);
                glib::spawn_future_local(async move {
                    match backend::vault::SecretService::default()
                        .clear_openvpn(&key)
                        .await
                    {
                        Ok(()) => {
                            remember.set_active(false);
                            status.set_text("Senha removida do cofre.");
                        }
                        Err(e) => status.set_text(&e.to_string()),
                    }
                    button.set_sensitive(true);
                });
            });
            let response = d.run_future().await;
            d.close();
            if response != gtk::ResponseType::Accept {
                password.set_text("");
                return Ok("Conexão cancelada.".into());
            }
            let secret = Secret::new(password.text().to_string());
            password.set_text("");
            if user.text().as_str() != old_user {
                backend::NetworkManager
                    .update(
                        &p.uuid,
                        &ProfileChanges(vec![(Field::Username, user.text().to_string())]),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let result = activate(
                &u,
                &p.uuid,
                if secret.is_empty() {
                    None
                } else {
                    Some(secret.expose())
                },
            )
            .await?;
            if remember.is_active() && !secret.is_empty() && result == "VPN conectada." {
                let key = backend::vault::profile_key(&p.uuid, user.text().as_str());
                if let Err(error) = vault.save_openvpn(&key, &secret).await {
                    return Ok(format!("VPN conectada. A senha não foi salva: {error}"));
                }
            }
            return Ok(result);
        }
        activate(&u, &p.uuid, None).await
    });
}

async fn activate(ui: &Ui, id: &str, password: Option<&str>) -> Result<String, String> {
    let token = CancellationToken::default();
    *ui.activation.borrow_mut() = Some((id.into(), token.clone()));
    ui.cancel.set_visible(true);
    ui.status.set_text("Conectando…");
    let result = backend::connect_cancellable(id, password, &token).await;
    ui.activation.borrow_mut().take();
    ui.cancel.set_visible(false);
    match result {
        Ok(()) => Ok("VPN conectada.".into()),
        Err(e) if e.kind == ErrorKind::Cancelled => Ok("Conexão cancelada.".into()),
        Err(e) => Err(e.to_string()),
    }
}

fn import_profile(ui: &Rc<Ui>) {
    let u = ui.clone();
    launch(ui, async move {
        let chooser = gtk::FileChooserNative::new(
            Some("Importar VPN — OpenVPN, WireGuard ou perfil VPN Manager"),
            Some(&u.window),
            gtk::FileChooserAction::Open,
            Some("Importar"),
            Some("Cancelar"),
        );
        let response = chooser.run_future().await;
        if response != gtk::ResponseType::Accept {
            return Ok("Importação cancelada.".into());
        }
        let path = chooser
            .file()
            .and_then(|f| f.path())
            .ok_or("Selecione um arquivo local.")?;
        let format = backend::detect_file(&path)
            .await
            .map_err(|e| e.to_string())?;
        let kind = format.backend_name();
        if !confirm(
            &u,
            &format!(
                "Importar este arquivo como {kind}? Revise as configurações antes de conectar."
            ),
        )
        .await
        {
            return Ok("Importação cancelada.".into());
        }
        backend::NetworkManager
            .import(&path)
            .await
            .map_err(|e| e.to_string())?;
        Ok("Perfil importado. Revise as configurações antes de conectar.".into())
    });
}

fn wrap_check_label(check: &gtk::CheckButton) {
    let text = gtk::Label::new(check.label().as_deref());
    text.set_wrap(true);
    text.set_xalign(0.0);
    text.set_hexpand(true);
    check.set_child(Some(&text));
}

fn edit_profile(ui: &Rc<Ui>, p: Profile) {
    let u = ui.clone();
    launch(ui, async move {
        let (d, content) = dialog(&u, &format!("Configurar {}", p.name));
        let tabs = gtk::Notebook::new();
        tabs.set_vexpand(true);
        tabs.set_scrollable(true);
        let general = column(10);
        margins(&general, 15);
        let network = column(10);
        margins(&network, 15);
        let advanced = column(10);
        margins(&advanced, 15);
        for (page, name) in [
            (&general, "Geral"),
            (&network, "Rede e DNS"),
            (&advanced, "Avançado"),
        ] {
            tabs.append_page(page, Some(&gtk::Label::new(Some(name))));
        }
        general.append(&label("IDENTIDADE DA CONEXÃO", "eyebrow"));
        network.append(&label("DNS E ROTEAMENTO", "eyebrow"));
        advanced.append(&label("IPV6 E OPÇÕES DO PROTOCOLO", "eyebrow"));
        content.append(&tabs);
        let note = label(
            "As alterações serão aplicadas ao reconectar a VPN.",
            "muted",
        );
        note.add_css_class("small");
        note.set_wrap(true);
        content.append(&note);

        let keys = [
            Field::Name,
            Field::Dns4,
            Field::Search4,
            Field::Routes4,
            Field::Dns6,
            Field::Routes6,
        ];
        let labels = [
            "Nome",
            "DNS IPv4 (separados por vírgula)",
            "Domínios de busca / DNS dividido (ex.: ~empresa.local)",
            "Rotas IPv4 (ex.: 10.0.0.0/8 10.0.0.1)",
            "DNS IPv6",
            "Rotas IPv6",
        ];
        let mut entries = Vec::new();
        for (key, label) in keys.iter().zip(labels) {
            let page = if *key == Field::Name {
                &general
            } else if matches!(key, Field::Dns6 | Field::Routes6 | Field::Search6) {
                &advanced
            } else {
                &network
            };
            entries.push(entry(
                page,
                label,
                &backend::NetworkManager
                    .read(&p.uuid, *key)
                    .await
                    .map_err(|e| e.to_string())?,
            ));
        }
        let split = gtk::CheckButton::with_label(
            "Túnel dividido — usar VPN somente para as rotas configuradas",
        );
        split.set_active(
            backend::NetworkManager
                .read(&p.uuid, Field::Split4)
                .await
                .map_err(|e| e.to_string())?
                == "yes",
        );
        wrap_check_label(&split);
        network.append(&split);
        let dns = gtk::CheckButton::with_label("Ignorar DNS recebido do servidor");
        dns.set_active(
            backend::NetworkManager
                .read(&p.uuid, Field::IgnoreDns4)
                .await
                .map_err(|e| e.to_string())?
                == "yes",
        );
        wrap_check_label(&dns);
        network.append(&dns);
        let split6 =
            gtk::CheckButton::with_label("IPv6: usar VPN somente para as rotas configuradas");
        split6.set_active(
            backend::NetworkManager
                .read(&p.uuid, Field::Split6)
                .await
                .map_err(|e| e.to_string())?
                == "yes",
        );
        wrap_check_label(&split6);
        advanced.append(&split6);
        let dns6 = gtk::CheckButton::with_label("IPv6: ignorar DNS recebido do servidor");
        dns6.set_active(
            backend::NetworkManager
                .read(&p.uuid, Field::IgnoreDns6)
                .await
                .map_err(|e| e.to_string())?
                == "yes",
        );
        wrap_check_label(&dns6);
        advanced.append(&dns6);
        let mut vpn_entries = Vec::new();
        if p.kind == "vpn" {
            for (key, label) in [
                (Field::Username, "Usuário"),
                (
                    Field::PluginOptions,
                    "Opções do plugin (chave=valor, separadas por vírgula)",
                ),
            ] {
                vpn_entries.push((
                    key,
                    entry(
                        if key == Field::Username {
                            &general
                        } else {
                            &advanced
                        },
                        label,
                        &backend::NetworkManager
                            .read(&p.uuid, key)
                            .await
                            .map_err(|e| e.to_string())?,
                    ),
                ));
            }
        }
        let warning = gtk::Label::new(Some(
            "Alterações de rede serão aplicadas na próxima conexão. Certificados, peers e opções específicas podem ser editados nas configurações de rede do Linux.",
        ));
        warning.set_wrap(true);
        warning.add_css_class("muted");
        warning.add_css_class("small");
        advanced.append(&warning);
        let native = icon_button("Abrir configurações do Linux", "settings", "");
        if let Some(text) = native
            .child()
            .and_then(|w| w.last_child())
            .and_then(|w| w.downcast::<gtk::Label>().ok())
        {
            text.set_wrap(true);
            text.set_max_width_chars(22);
        }
        advanced.append(&native);
        native.connect_clicked(|_| {
            let _ = std::process::Command::new("kcmshell6")
                .arg("kcm_networkmanagement")
                .spawn()
                .or_else(|_| std::process::Command::new("nm-connection-editor").spawn());
        });
        let response = d.run_future().await;
        d.close();
        if response != gtk::ResponseType::Accept {
            return Ok("Edição cancelada.".into());
        }
        if entries[0].text().trim().is_empty() {
            return Err("Informe um nome para a VPN.".into());
        }
        let mut changes = ProfileChanges::default();
        for (key, e) in keys.iter().zip(entries) {
            changes.0.push((*key, e.text().to_string()));
        }
        for (key, e) in vpn_entries {
            changes.0.push((key, e.text().to_string()));
        }
        for (key, checkbox) in [
            (Field::Split4, &split),
            (Field::Split6, &split6),
            (Field::IgnoreDns4, &dns),
            (Field::IgnoreDns6, &dns6),
        ] {
            changes
                .0
                .push((key, if checkbox.is_active() { "yes" } else { "no" }.into()));
        }
        backend::NetworkManager
            .update(&p.uuid, &changes)
            .await
            .map_err(|e| e.to_string())?;
        Ok("Configurações salvas. Reconecte a VPN para aplicar.".into())
    });
}

fn create_profile(ui: &Rc<Ui>) {
    let u = ui.clone();
    launch(ui, async move {
        let (d, content) = dialog(&u, "Nova conexão OpenVPN");
        let name = entry(&content, "Nome da conexão", "Minha VPN");
        let host = entry(&content, "Servidor (hostname ou IP)", "");
        let port = entry(&content, "Porta", "1194");
        let user = entry(&content, "Usuário", "");
        let ca = entry(&content, "Caminho absoluto do certificado CA (.crt)", "");
        let tcp = gtk::CheckButton::with_label("Usar TCP (padrão: UDP)");
        content.append(&tcp);
        let note = gtk::Label::new(Some(
            "Para certificados de cliente, TLS-auth ou configurações especiais, prefira importar o .ovpn fornecido pela empresa.",
        ));
        note.set_wrap(true);
        content.append(&note);
        let response = d.run_future().await;
        d.close();
        if response != gtk::ResponseType::Accept {
            return Ok("Criação cancelada.".into());
        }
        let port: u16 = port.text().parse().map_err(|_| "Porta inválida")?;
        backend::NetworkManager
            .create(&OpenVpnProfile {
                name: name.text().to_string(),
                remote: host.text().to_string(),
                port,
                username: user.text().to_string(),
                ca: ca.text().as_str().into(),
                tcp: tcp.is_active(),
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok("VPN criada. Configure as opções adicionais antes de conectar.".into())
    });
}
fn export_profile(ui: &Rc<Ui>, p: Profile) {
    let u = ui.clone();
    launch(ui, async move {
        let chooser = gtk::FileChooserNative::new(
            Some("Exportar perfil VPN"),
            Some(&u.window),
            gtk::FileChooserAction::Save,
            Some("Exportar"),
            Some("Cancelar"),
        );
        chooser.set_current_name("vpn-export.ovpn");
        if chooser.run_future().await != gtk::ResponseType::Accept {
            return Ok("Exportação cancelada.".into());
        }
        let path = chooser
            .file()
            .and_then(|f| f.path())
            .ok_or("Caminho inválido")?;
        backend::NetworkManager
            .export(&p.uuid, &path)
            .await
            .map_err(|e| e.to_string())?;
        Ok("Perfil exportado com permissão privada. Ele pode referenciar certificados externos; preserve esses arquivos.".into())
    });
}
fn autostart_path() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config")
        })
        .join("autostart/vpn-manager.desktop")
}
fn set_autostart(enabled: bool) -> std::io::Result<()> {
    let path = autostart_path();
    if enabled {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let executable = std::env::current_exe()?;
        let escaped = executable
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%");
        std::fs::write(
            path,
            format!(
                "[Desktop Entry]\nType=Application\nName=VPN Manager\nExec=\"{escaped}\" --background\nIcon=vpn-manager\nTerminal=false\n"
            ),
        )
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}
fn monitor(ui: &Rc<Ui>) {
    let u = Rc::downgrade(ui);
    match backend::NetworkManager.subscribe(move |event| {
        if let Some(u) = u.upgrade() {
            match event {
                vpn_manager_core::backend::BackendEvent::ProfilesChanged => refresh(&u),
                vpn_manager_core::backend::BackendEvent::MonitorStopped => u
                    .status
                    .set_text("Monitor encerrado. Use Atualizar para consultar as conexões."),
            }
        }
    }) {
        Ok(subscription) => *ui.monitor.borrow_mut() = Some(subscription),
        Err(e) => ui.status.set_text(&e.to_string()),
    }
    if let Some(app) = ui.window.application() {
        let u = Rc::downgrade(ui);
        app.connect_shutdown(move |_| {
            if let Some(u) = u.upgrade() {
                u.monitor.borrow_mut().take();
            }
        });
    }
}
