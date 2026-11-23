//! Tests for settings.rs.

use crate::settings::{load, resolve, validate_stored, write};
use crate::*;
use serial_test::serial;
use std::env;
use std::net::IpAddr;
use std::path::PathBuf;

use tempfile::TempDir;

#[cfg(debug_assertions)]
const PROFILE_DIR: &str = "c5t-dev";
#[cfg(not(debug_assertions))]
const PROFILE_DIR: &str = "c5t";

/// Stored settings with every field set.
fn full_stored() -> StoredSettings {
    StoredSettings {
        host: Some(IpAddr::from([127, 0, 0, 1])),
        port: Some(4040),
        verbosity: Some(3),
        enable_docs: Some(true),
        skills_dir: Some(PathBuf::from("/tmp/settings-skills")),
        start_at_login: Some(false),
    }
}

#[test]
fn config_path_sits_in_the_profile_config_dir() {
    let path = get_config_path();

    assert_eq!(
        path,
        get_config_dir().join("config.toml"),
        "config path should sit inside the config dir"
    );
    assert!(
        path.ends_with(format!("{PROFILE_DIR}/config.toml")),
        "config path {path:?} should end with {PROFILE_DIR}/config.toml"
    );
}

#[test]
#[serial]
fn base_path_override_replaces_the_config_root() {
    let base = TempDir::new().expect("test temp dir");
    set_base_path(base.path().to_path_buf());

    let config_dir = get_config_dir();

    clear_base_path();

    assert_eq!(
        config_dir,
        base.path().join(PROFILE_DIR),
        "base path override should replace the config root"
    );
}

#[test]
#[serial]
fn xdg_config_home_replaces_the_config_root() {
    clear_base_path();
    unsafe { env::set_var("XDG_CONFIG_HOME", "/tmp/c5t-settings-xdg") };

    let config_dir = get_config_dir();

    unsafe { env::remove_var("XDG_CONFIG_HOME") };

    assert_eq!(
        config_dir,
        PathBuf::from("/tmp/c5t-settings-xdg").join(PROFILE_DIR)
    );
}

#[test]
#[serial]
fn config_root_falls_back_to_the_platform_default() {
    clear_base_path();
    unsafe { env::remove_var("XDG_CONFIG_HOME") };

    let config_dir = get_config_dir();
    let home = PathBuf::from(env::var("HOME").expect("HOME is set"));

    #[cfg(target_os = "macos")]
    let expected = home.join("Library/Application Support").join(PROFILE_DIR);
    #[cfg(not(target_os = "macos"))]
    let expected = home.join(".config").join(PROFILE_DIR);

    assert_eq!(
        config_dir, expected,
        "unset XDG_CONFIG_HOME should fall back to the platform config root"
    );
}

#[test]
fn settings_default_mirrors_the_api_config_defaults() {
    let settings = Settings::default();

    assert_eq!(settings.host, IpAddr::from([0, 0, 0, 0]));
    #[cfg(debug_assertions)]
    assert_eq!(settings.port, 3738);
    #[cfg(not(debug_assertions))]
    assert_eq!(settings.port, 3737);
    assert_eq!(settings.verbosity, 0);
    assert!(!settings.enable_docs);
    assert_eq!(settings.skills_dir, get_data_dir().join("skills"));
    assert!(settings.start_at_login);
}

#[test]
fn validate_stored_rejects_a_zero_port() {
    let stored = StoredSettings {
        port: Some(0),
        ..StoredSettings::default()
    };

    match validate_stored(&stored) {
        Err(SettingsError::Validation { message }) => assert!(
            message.contains("port"),
            "validation message {message:?} should name the port field"
        ),
        other => panic!("expected a validation error for port 0, got {other:?}"),
    }
}

#[test]
fn validate_stored_rejects_verbosity_above_three() {
    let stored = StoredSettings {
        verbosity: Some(4),
        ..StoredSettings::default()
    };

    match validate_stored(&stored) {
        Err(SettingsError::Validation { message }) => assert!(
            message.contains("verbosity"),
            "validation message {message:?} should name the verbosity field"
        ),
        other => panic!("expected a validation error for verbosity 4, got {other:?}"),
    }
}

#[test]
fn validate_stored_accepts_the_field_limits() {
    let upper = StoredSettings {
        port: Some(65535),
        verbosity: Some(3),
        ..StoredSettings::default()
    };
    let lower = StoredSettings {
        port: Some(1),
        verbosity: Some(0),
        ..StoredSettings::default()
    };

    assert!(validate_stored(&upper).is_ok(), "65535 and 3 are valid");
    assert!(validate_stored(&lower).is_ok(), "1 and 0 are valid");
}

#[test]
fn validation_failure_leaves_the_settings_file_absent() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("nested/config.toml");
    let stored = StoredSettings {
        verbosity: Some(4),
        ..StoredSettings::default()
    };

    let outcome = match validate_stored(&stored) {
        Ok(()) => write(&path, &stored),
        Err(error) => Err(error),
    };

    assert!(outcome.is_err(), "verbosity 4 must fail validation");
    assert!(
        !path.exists(),
        "a caller that validates first must not create {path:?}"
    );
}

#[test]
fn load_returns_defaults_when_the_file_is_missing() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("config.toml");

    let stored = load(&path).expect("a missing file loads as defaults");

    assert_eq!(
        stored,
        StoredSettings::default(),
        "a missing file should resolve to all-none settings"
    );
}

#[test]
fn load_reports_parse_errors_with_the_path() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "port = \"not-a-number\"\n").expect("write invalid toml");

    match load(&path) {
        Err(SettingsError::Parse { path: reported, .. }) => {
            assert_eq!(reported, path.display().to_string());
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn write_then_load_roundtrips_every_field() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("nested/config.toml");
    let stored = full_stored();

    write(&path, &stored).expect("write creates parent directories");
    let loaded = load(&path).expect("load returns the written settings");

    assert_eq!(loaded, stored, "roundtrip should preserve every Some field");
}

#[test]
fn write_skips_none_fields() {
    let dir = TempDir::new().expect("test temp dir");
    let path = dir.path().join("config.toml");
    let stored = StoredSettings {
        port: Some(4040),
        ..StoredSettings::default()
    };

    write(&path, &stored).expect("write settings");
    let text = std::fs::read_to_string(&path).expect("read back the settings file");

    assert!(
        text.contains("port"),
        "serialized toml {text:?} should carry port"
    );
    for absent in [
        "host",
        "verbosity",
        "enable_docs",
        "skills_dir",
        "start_at_login",
    ] {
        assert!(
            !text.contains(absent),
            "serialized toml {text:?} should skip {absent}"
        );
    }
}

#[test]
fn resolve_fills_defaults_for_missing_fields() {
    let defaults = Settings::default();
    let partial = StoredSettings {
        port: Some(4040),
        start_at_login: Some(false),
        ..StoredSettings::default()
    };

    let resolved = resolve(&partial);

    assert_eq!(resolved.port, 4040);
    assert!(!resolved.start_at_login);
    assert_eq!(resolved.host, defaults.host);
    assert_eq!(resolved.verbosity, defaults.verbosity);
    assert_eq!(resolved.enable_docs, defaults.enable_docs);
    assert_eq!(resolved.skills_dir, defaults.skills_dir);
}

#[test]
fn resolve_keeps_every_stored_field() {
    let resolved = resolve(&full_stored());
    let expected = Settings {
        host: IpAddr::from([127, 0, 0, 1]),
        port: 4040,
        verbosity: 3,
        enable_docs: true,
        skills_dir: PathBuf::from("/tmp/settings-skills"),
        start_at_login: false,
    };

    assert_eq!(resolved, expected);
}
