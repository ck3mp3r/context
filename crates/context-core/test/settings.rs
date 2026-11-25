//! Tests for settings.rs.

use crate::settings::{load, migrate_legacy_config, resolve, validate_stored, write};
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

/// Environment variable restored when the guard drops.
struct EnvGuard {
    name: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(name: &'static str, value: &std::ffi::OsStr) -> Self {
        let previous = env::var(name).ok();
        unsafe { env::set_var(name, value) };
        Self { name, previous }
    }

    fn remove(name: &'static str) -> Self {
        let previous = env::var(name).ok();
        unsafe { env::remove_var(name) };
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe { env::set_var(self.name, value) },
            None => unsafe { env::remove_var(self.name) },
        }
    }
}

/// Stored settings with every field set.
fn full_stored() -> StoredSettings {
    StoredSettings {
        host: Some(IpAddr::from([127, 0, 0, 1])),
        port: Some(4040),
        verbosity: Some(3),
        enable_docs: Some(true),
        skills_dir: Some(PathBuf::from("/tmp/settings-skills")),
        start_at_login: Some(false),
        auto_update: Some(false),
        update_channel: Some("pre-release".to_string()),
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
    let xdg = TempDir::new().expect("xdg temp dir");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg.path().as_os_str());
    set_base_path(base.path().to_path_buf());

    let config_dir = get_config_dir();

    clear_base_path();

    assert!(
        config_dir.starts_with(base.path()),
        "base path override should win over XDG_CONFIG_HOME, got {config_dir:?}"
    );
    assert_eq!(
        config_dir,
        base.path().join(PROFILE_DIR),
        "base path override should replace the config root"
    );
    assert!(
        config_dir.to_string_lossy().contains("c5t"),
        "the override path should carry the profile name, got {config_dir:?}"
    );
}

#[test]
#[serial]
fn xdg_config_home_does_not_move_the_config_root() {
    clear_base_path();
    let xdg = TempDir::new().expect("xdg temp dir");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg.path().as_os_str());

    let config_dir = get_config_dir();
    let home = PathBuf::from(env::var("HOME").expect("HOME is set"));

    #[cfg(target_os = "macos")]
    let expected = home.join("Library/Application Support").join(PROFILE_DIR);
    #[cfg(not(target_os = "macos"))]
    let expected = home.join(".config").join(PROFILE_DIR);

    assert!(
        !config_dir.starts_with(xdg.path()),
        "XDG_CONFIG_HOME must not move the config root, got {config_dir:?}"
    );
    assert_eq!(
        config_dir, expected,
        "a set XDG_CONFIG_HOME should leave the native config root in place"
    );
}

#[test]
#[serial]
fn config_resolution_does_not_panic_without_home() {
    clear_base_path();
    let _home = EnvGuard::remove("HOME");
    let _xdg = EnvGuard::remove("XDG_CONFIG_HOME");
    #[cfg(target_os = "windows")]
    let _appdata = EnvGuard::remove("APPDATA");

    let config_dir = try_get_config_dir();
    let config_path = try_get_config_path();
    let fallback = get_config_path();

    assert!(
        config_dir.is_none(),
        "no HOME means no config dir, got {config_dir:?}"
    );
    assert!(
        config_path.is_none(),
        "no HOME means no config path, got {config_path:?}"
    );
    assert_eq!(
        fallback,
        env::temp_dir().join(PROFILE_DIR).join("config.toml"),
        "the infallible lookup should fall back to the temp directory"
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
#[serial]
fn migration_copies_a_legacy_config_once() {
    let native_base = TempDir::new().expect("native temp dir");
    let legacy_home = TempDir::new().expect("legacy temp dir");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", legacy_home.path().as_os_str());
    set_base_path(native_base.path().to_path_buf());

    let legacy_dir = legacy_home.path().join(PROFILE_DIR);
    std::fs::create_dir_all(&legacy_dir).expect("legacy config dir creates");
    std::fs::write(legacy_dir.join("config.toml"), "port = 4040\n").expect("legacy file writes");

    migrate_legacy_config();

    let native = get_config_path();
    let copied = std::fs::read_to_string(&native).expect("the native file exists after migration");

    // A second run must not copy again: a changed native file stays changed.
    std::fs::write(&native, "port = 5050\n").expect("the native file rewrites");
    migrate_legacy_config();
    let after_second = std::fs::read_to_string(&native).expect("the native file survives");

    clear_base_path();

    assert_eq!(
        copied, "port = 4040\n",
        "the migration should copy the legacy contents"
    );
    assert_eq!(
        after_second, "port = 5050\n",
        "a second migration should not re-copy the legacy file"
    );
}

#[test]
#[serial]
fn migration_keeps_the_native_file_when_both_exist() {
    let native_base = TempDir::new().expect("native temp dir");
    let legacy_home = TempDir::new().expect("legacy temp dir");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", legacy_home.path().as_os_str());
    set_base_path(native_base.path().to_path_buf());

    let native = get_config_path();
    std::fs::create_dir_all(native.parent().expect("native config dir"))
        .expect("native dir creates");
    std::fs::write(&native, "port = 5050\n").expect("native file writes");

    let legacy_dir = legacy_home.path().join(PROFILE_DIR);
    std::fs::create_dir_all(&legacy_dir).expect("legacy config dir creates");
    std::fs::write(legacy_dir.join("config.toml"), "port = 4040\n").expect("legacy file writes");

    migrate_legacy_config();

    let native_contents = std::fs::read_to_string(&native).expect("the native file stays readable");

    clear_base_path();

    assert_eq!(
        native_contents, "port = 5050\n",
        "an existing native file must win over the legacy one"
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
    assert!(settings.auto_update);
    assert_eq!(settings.update_channel, "stable");
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
fn validate_stored_accepts_both_update_channels() {
    for channel in ["stable", "pre-release"] {
        let stored = StoredSettings {
            update_channel: Some(channel.to_string()),
            ..StoredSettings::default()
        };

        assert!(
            validate_stored(&stored).is_ok(),
            "{channel:?} is an accepted update channel"
        );
    }
}

#[test]
fn validate_stored_rejects_an_unknown_update_channel() {
    let stored = StoredSettings {
        update_channel: Some("beta".to_string()),
        ..StoredSettings::default()
    };

    match validate_stored(&stored) {
        Err(SettingsError::Validation { message }) => assert!(
            message.contains("update_channel"),
            "validation message {message:?} should name the update channel field"
        ),
        other => panic!("expected a validation error for channel beta, got {other:?}"),
    }
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
        "auto_update",
        "update_channel",
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
fn resolve_defaults_the_update_preferences_when_absent() {
    let resolved = resolve(&StoredSettings::default());

    assert!(resolved.auto_update, "auto-update defaults to on");
    assert_eq!(resolved.update_channel, "stable");
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
        auto_update: false,
        update_channel: "pre-release".to_string(),
    };

    assert_eq!(resolved, expected);
}
