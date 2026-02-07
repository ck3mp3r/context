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

use crate::api::Config;
use std::net::IpAddr;

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
    use crate::sync::{get_db_path, set_base_path};
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
    use crate::sync::get_db_path;

    let db_path = get_db_path();

    // Should use default path
    assert!(db_path.to_string_lossy().contains("c5t"));
    assert!(db_path.to_string_lossy().ends_with("context.db"));
}
