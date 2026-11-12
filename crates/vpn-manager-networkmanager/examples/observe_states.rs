//! Integration probe driven exclusively by tests/states.py's private bus.
use gio::glib::{self, variant::ToVariant};
use std::{cell::Cell, rc::Rc, time::Duration};
use vpn_manager_core::{
    ConnectionState,
    backend::{Backend, BackendEvent},
};
use vpn_manager_networkmanager::NetworkManager;

fn main() {
    assert_eq!(
        std::env::var("VPN_MANAGER_TEST_FIXTURE").as_deref(),
        Ok("1"),
        "Run with python3 tests/states.py; never point this probe at a real system bus"
    );
    let context = glib::MainContext::new();
    context.with_thread_default(|| context.block_on(async {
        let changes = Rc::new(Cell::new(0));
        let seen = changes.clone();
        let subscription = NetworkManager.subscribe(move |event| {
            assert_eq!(event, BackendEvent::ProfilesChanged);
            seen.set(seen.get() + 1);
        }).unwrap();
        let bus = gio::bus_get_future(gio::BusType::System).await.unwrap();
        // Wait for an observed signal, rather than assuming a registration delay.
        for _ in 0..50 {
            set_state(&bus, 5).await;
            glib::timeout_future(Duration::from_millis(20)).await;
            if changes.get() > 0 { break; }
        }
        assert!(changes.get() > 0, "VPN state subscription must initialize");
        for (value, expected) in [
            (1u32, ConnectionState::Preparing), (2, ConnectionState::Authenticating),
            (3, ConnectionState::Connecting), (4, ConnectionState::Connecting),
            (5, ConnectionState::Connected), (6, ConnectionState::Failed),
            (7, ConnectionState::Disconnected), (99, ConnectionState::Unknown),
        ] {
            let before = changes.get();
            set_state(&bus, value).await;
            for _ in 0..50 {
                if changes.get() > before { break; }
                glib::timeout_future(Duration::from_millis(20)).await;
            }
            assert!(changes.get() > before, "VpnStateChanged must wake the adapter without nmcli output");
            let profiles = NetworkManager.profiles().await.unwrap();
            let profile = profiles.iter().find(|p| p.uuid == "11111111-1111-4111-8111-111111111111").unwrap();
            assert_eq!(profile.state, expected);
            assert_eq!(NetworkManager.state(&profile.uuid).await.unwrap(), expected);
        }
        drop(subscription);
        let before = changes.get();
        set_state(&bus, 5).await;
        glib::timeout_future(Duration::from_millis(100)).await;
        assert_eq!(changes.get(), before, "dropped subscription must stop delivering events");
        println!("VPN_STATES_OK: preparation, authentication, connection, failure, unknown states, signals and unsubscribe");
    })).unwrap();
}

async fn set_state(bus: &gio::DBusConnection, value: u32) {
    bus.call_future(
        Some("org.freedesktop.NetworkManager"),
        "/org/freedesktop/NetworkManager/ActiveConnection/1",
        "org.example.VpnManager.Fixture",
        "SetState",
        Some(&(value,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
    )
    .await
    .unwrap();
}
