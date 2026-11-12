use gio::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    ffi::OsStr,
    rc::Rc,
};
use vpn_manager_core::backend::{BackendError, BackendEvent, ErrorKind, EventSubscription};

struct Subscription {
    process: gio::Subprocess,
    stopped: Rc<Cell<bool>>,
    bus: Rc<RefCell<Option<gio::SignalSubscription>>>,
}

impl EventSubscription for Subscription {
    fn stop(&self) {
        if !self.stopped.replace(true) {
            self.process.force_exit();
        }
        self.bus.borrow_mut().take();
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn subscribe(
    callback: impl Fn(BackendEvent) + 'static,
) -> Result<Box<dyn EventSubscription>, BackendError> {
    let process = gio::Subprocess::newv(
        &[
            OsStr::new("env"),
            OsStr::new("LC_ALL=C"),
            OsStr::new("nmcli"),
            OsStr::new("monitor"),
        ],
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .map_err(|_| BackendError {
        kind: ErrorKind::Unavailable,
        message: "Não foi possível iniciar o monitor do NetworkManager.".into(),
    })?;
    let stopped = Rc::new(Cell::new(false));
    let bus_subscription = Rc::new(RefCell::new(None));
    let subscription = Subscription {
        process: process.clone(),
        stopped: stopped.clone(),
        bus: bus_subscription.clone(),
    };
    let callback = Rc::new(callback);
    let signal_callback = callback.clone();
    let signal_stopped = stopped.clone();
    glib::spawn_future_local(async move {
        if let Ok(bus) = gio::bus_get_future(gio::BusType::System).await {
            if signal_stopped.get() {
                return;
            }
            let subscription = bus.subscribe_to_signal(
                Some("org.freedesktop.NetworkManager"),
                Some("org.freedesktop.NetworkManager.VPN.Connection"),
                Some("VpnStateChanged"),
                None,
                None,
                gio::DBusSignalFlags::NONE,
                move |_| {
                    if !signal_stopped.get() {
                        signal_callback(BackendEvent::ProfilesChanged);
                    }
                },
            );
            *bus_subscription.borrow_mut() = Some(subscription);
        }
    });
    glib::spawn_future_local(async move {
        let stream =
            gio::DataInputStream::new(&process.stdout_pipe().expect("stdout pipe requested"));
        while let Ok(Some(_)) = stream.read_line_future(glib::Priority::DEFAULT).await {
            if stopped.get() {
                break;
            }
            // nmcli text may contain private connection names. Only emit a hint.
            callback(BackendEvent::ProfilesChanged);
        }
        let _ = process.wait_future().await;
        if !stopped.replace(true) {
            callback(BackendEvent::MonitorStopped);
        }
    });
    Ok(Box::new(subscription))
}
