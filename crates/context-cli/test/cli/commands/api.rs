// =============================================================================
// Unit Tests - Focused on testable logic
// =============================================================================
//
// NOTE: The `run()` function starts a long-running API server which cannot be
// easily tested in unit tests. The core API server functionality is tested in:
// - src/api/* (API endpoint tests)
// - Integration tests via CLI commands (project, repo, note, task, sync tests)
//
// These tests focus on:
// 1. Config structure validation
// 2. Path resolution logic (via get_db_path)
// 3. Input validation
//
// Full server startup is covered by:
// - Manual testing
// - Integration tests (all other CLI command tests use the API server)
// =============================================================================

use context_server::api::Config;
use std::net::IpAddr;

use super::api::{ApiFlags, effective_config, stored_settings, verbosity_from_count};
use context_core::settings::Settings;
use context_core::{clear_base_path, get_config_dir, get_config_path, set_base_path};
use serial_test::serial;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(debug_assertions)]
const PROFILE_PORT: u16 = 3738;
#[cfg(not(debug_assertions))]
const PROFILE_PORT: u16 = 3737;

/// Stored settings carrying every field.
fn stored_settings_fixture() -> Settings {
    Settings {
        host: "0.0.0.0".parse().expect("host parses"),
        port: 4000,
        verbosity: 2,
        enable_docs: true,
        skills_dir: PathBuf::from("/tmp/stored-skills"),
        start_at_login: true,
    }
}

/// Base path this test alone owns, with the settings file written.
fn base_with_config(contents: &str) -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());

    let config_dir = get_config_dir();
    std::fs::create_dir_all(&config_dir).expect("config dir creates");
    std::fs::write(get_config_path(), contents).expect("settings file writes");

    base
}

#[test]
fn test_config_structure() {
    // Test that Config can be created with valid values
    let config = Config {
        host: "127.0.0.1".parse::<IpAddr>().unwrap(),
        port: 3000,
        verbosity: 0,
        enable_docs: false,
        skills_dir: std::path::PathBuf::from("/tmp/skills"),
    };

    assert_eq!(config.host.to_string(), "127.0.0.1");
    assert_eq!(config.port, 3000);
    assert_eq!(config.verbosity, 0);
    assert!(!config.enable_docs);
}

#[test]
fn test_config_with_docs_enabled() {
    // Test Config with docs enabled
    let config = Config {
        host: "0.0.0.0".parse::<IpAddr>().unwrap(),
        port: 8080,
        verbosity: 2,
        enable_docs: true,
        skills_dir: std::path::PathBuf::from("/tmp/skills"),
    };

    assert_eq!(config.host.to_string(), "0.0.0.0");
    assert_eq!(config.port, 8080);
    assert_eq!(config.verbosity, 2);
    assert!(config.enable_docs);
}

#[test]
fn test_ipv4_address_parsing() {
    // Test that IPv4 addresses can be parsed
    let ipv4: IpAddr = "192.168.1.100".parse().unwrap();
    assert!(ipv4.is_ipv4());
    assert_eq!(ipv4.to_string(), "192.168.1.100");
}

#[test]
fn test_ipv6_address_parsing() {
    // Test that IPv6 addresses can be parsed
    let ipv6: IpAddr = "::1".parse().unwrap();
    assert!(ipv6.is_ipv6());
    assert_eq!(ipv6.to_string(), "::1");
}

#[test]
fn test_localhost_address() {
    // Test localhost address
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(localhost.is_loopback());
}

#[test]
fn test_invalid_ip_address() {
    // Test that invalid IP addresses fail to parse
    let result = "999.999.999.999".parse::<IpAddr>();
    assert!(result.is_err());
}

#[test]
fn test_invalid_ipv6_address() {
    // Test that invalid IPv6 addresses fail to parse
    let result = "gggg::1".parse::<IpAddr>();
    assert!(result.is_err());
}

#[test]
fn test_port_ranges() {
    // Test that various port numbers are valid
    let valid_ports = vec![80, 443, 3000, 8080, 65535];

    for port in valid_ports {
        let config = Config {
            host: "127.0.0.1".parse().unwrap(),
            port,
            verbosity: 0,
            enable_docs: false,
            skills_dir: std::path::PathBuf::from("/tmp/skills"),
        };
        assert_eq!(config.port, port);
    }
}

#[test]
fn test_verbosity_levels() {
    // Test different verbosity levels
    let levels = vec![0, 1, 2, 3, 4, 5];

    for level in levels {
        let config = Config {
            host: "127.0.0.1".parse().unwrap(),
            port: 3000,
            verbosity: level,
            enable_docs: false,
            skills_dir: std::path::PathBuf::from("/tmp/skills"),
        };
        assert_eq!(config.verbosity, level);
    }
}

#[test]
fn test_get_db_path_with_custom_home() {
    // Test get_db_path with custom home directory
    use context_core::{get_db_path, set_base_path};
    use std::path::PathBuf;

    let custom_home = PathBuf::from("/custom/home");
    set_base_path(custom_home.clone());
    let db_path = get_db_path();

    assert!(db_path.starts_with(custom_home));
    assert!(db_path.to_string_lossy().contains("c5t"));
}

#[test]
fn test_get_db_path_without_home() {
    // Test get_db_path with default home directory
    use context_core::get_db_path;

    let db_path = get_db_path();

    // Should use default path
    assert!(db_path.to_string_lossy().contains("c5t"));
    assert!(db_path.to_string_lossy().ends_with("context.db"));
}

// =============================================================================
// Precedence: flags > C5T_SKILLS_DIR > stored > built-in
// =============================================================================

#[test]
fn no_flags_keep_every_stored_value() {
    let stored = stored_settings_fixture();
    let config = effective_config(&ApiFlags::default(), &stored);

    assert_eq!(config.host, stored.host);
    assert_eq!(config.port, 4000);
    assert_eq!(config.verbosity, 2);
    assert!(config.enable_docs);
    assert_eq!(config.skills_dir, stored.skills_dir);
}

#[test]
fn flags_override_every_stored_value() {
    let stored = stored_settings_fixture();
    let flags = ApiFlags {
        host: Some("127.0.0.1".parse().expect("host parses")),
        port: Some(5000),
        skills_dir: Some(PathBuf::from("/tmp/flag-skills")),
        verbosity: Some(3),
        enable_docs: true,
    };

    let config = effective_config(&flags, &stored);

    assert_eq!(config.host.to_string(), "127.0.0.1");
    assert_eq!(config.port, 5000);
    assert_eq!(config.verbosity, 3);
    assert!(config.enable_docs);
    assert_eq!(config.skills_dir, PathBuf::from("/tmp/flag-skills"));
}

#[test]
fn docs_flag_enables_docs_without_a_stored_request() {
    let stored = Settings {
        enable_docs: false,
        ..stored_settings_fixture()
    };
    let flags = ApiFlags {
        enable_docs: true,
        ..ApiFlags::default()
    };

    assert!(effective_config(&flags, &stored).enable_docs);
}

#[test]
fn absent_verbosity_flag_is_no_override() {
    assert_eq!(verbosity_from_count(0), None);
    assert_eq!(verbosity_from_count(2), Some(2));
}

#[test]
#[serial]
fn skills_dir_env_beats_the_stored_value() {
    let base = base_with_config("skills_dir = \"/tmp/stored-skills\"\n");
    unsafe { std::env::set_var("C5T_SKILLS_DIR", "/tmp/env-skills") };

    let config = effective_config(&ApiFlags::default(), &stored_settings());

    unsafe { std::env::remove_var("C5T_SKILLS_DIR") };
    clear_base_path();
    drop(base);

    assert_eq!(config.skills_dir, PathBuf::from("/tmp/env-skills"));
}

#[test]
#[serial]
fn skills_dir_flag_beats_env_and_stored() {
    let base = base_with_config("skills_dir = \"/tmp/stored-skills\"\n");
    unsafe { std::env::set_var("C5T_SKILLS_DIR", "/tmp/env-skills") };
    let flags = ApiFlags {
        skills_dir: Some(PathBuf::from("/tmp/flag-skills")),
        ..ApiFlags::default()
    };

    let config = effective_config(&flags, &stored_settings());

    unsafe { std::env::remove_var("C5T_SKILLS_DIR") };
    clear_base_path();
    drop(base);

    assert_eq!(config.skills_dir, PathBuf::from("/tmp/flag-skills"));
}

#[test]
#[serial]
fn stored_settings_read_the_file_at_the_base_path() {
    let base = base_with_config("host = \"127.0.0.1\"\nport = 4000\nverbosity = 2\n");

    let stored = stored_settings();

    clear_base_path();
    drop(base);

    assert_eq!(stored.host.to_string(), "127.0.0.1");
    assert_eq!(stored.port, 4000);
    assert_eq!(stored.verbosity, 2);
}

#[test]
#[serial]
fn absent_file_keeps_the_built_in_defaults() {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());

    let stored = stored_settings();

    clear_base_path();
    drop(base);

    assert_eq!(stored.port, PROFILE_PORT);
    assert_eq!(stored.verbosity, 0);
    assert!(!stored.enable_docs);
}

#[test]
#[serial]
fn corrupt_file_falls_back_to_the_built_in_defaults() {
    let base = base_with_config("port = \"not-a-number\"\n");

    let stored = stored_settings();

    clear_base_path();
    drop(base);

    assert_eq!(stored.port, PROFILE_PORT);
    assert_eq!(stored.verbosity, 0);
}

#[test]
#[serial]
fn invalid_stored_values_fall_back_to_the_built_in_defaults() {
    let base = base_with_config("port = 0\nverbosity = 4\n");

    let stored = stored_settings();

    clear_base_path();
    drop(base);

    assert_eq!(stored.port, PROFILE_PORT);
    assert_eq!(stored.verbosity, 0);
}
