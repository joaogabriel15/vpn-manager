//! Own test binary: glib caches the config directory on first use.
use vpn_manager_networkmanager::fortinet;

#[test]
fn auto_import_once_and_never_resurrect_a_removed_profile() {
    let home = tempfile::tempdir().unwrap();
    // SAFETY: set before any other thread or glib call in this binary.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", home.path()) };
    let cache = home.path().join("FortiClient/Cache/Cache_Data");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join("entry_0"),
        b"http://127.0.0.1:1/vpn?name=Lab\0{\"type\":\"ssl\",\"connection_name\":\"Lab\",\"remote_gateway\":\"vpn.example.invalid\",\"sso_enabled\":true}",
    )
    .unwrap();

    assert_eq!(fortinet::auto_import(), ["Lab"]);
    assert!(fortinet::auto_import().is_empty(), "already imported");
    let profiles = fortinet::profiles();
    assert_eq!(profiles.len(), 1);
    fortinet::remove(&profiles[0].uuid).unwrap();
    assert!(
        fortinet::auto_import().is_empty(),
        "removed profile came back"
    );
    assert!(fortinet::profiles().is_empty());
}
