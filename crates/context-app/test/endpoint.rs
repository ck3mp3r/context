//! Tests for the build-profile endpoints and the health probe.

use std::io::{Read, Write};
use std::net::TcpListener;

use context_core::{clear_base_path, get_config_dir, get_config_path, set_base_path};
use serial_test::serial;
use std::path::PathBuf;
use tempfile::TempDir;

use super::*;

fn client() -> reqwest::Client {
    health_client().expect("Health probe client should build")
}

fn answer_once(listener: TcpListener) {
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        }
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn probe_health_reports_ok_when_server_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    answer_once(listener);

    assert!(probe_health(&client(), &health_url(port)).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn probe_health_reports_false_when_nothing_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    drop(listener);

    assert!(!probe_health(&client(), &health_url(port)).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn await_health_succeeds_when_server_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    answer_once(listener);

    let (_failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();

    assert!(
        await_health(
            &client(),
            &health_url(port),
            Duration::from_secs(5),
            &mut failure_rx
        )
        .await
        .is_ok()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn await_health_returns_server_failure_before_the_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    drop(listener);

    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();
    failure_tx
        .send("Failed to bind to address 127.0.0.1:3738: Address already in use".to_string())
        .expect("Failure send should succeed");
    drop(failure_tx);

    let started = std::time::Instant::now();
    let result = await_health(
        &client(),
        &health_url(port),
        Duration::from_secs(10),
        &mut failure_rx,
    )
    .await;

    let message = result.expect_err("Server failure should end the wait");
    assert!(
        message.contains("Failed to bind to address 127.0.0.1:3738"),
        "message was: {message}"
    );
    assert!(
        !message.contains("did not answer"),
        "message was: {message}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "wait took {:?}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn await_health_reports_timeout_when_server_never_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind should succeed");
    let port = listener
        .local_addr()
        .expect("Local addr should exist")
        .port();
    drop(listener);

    let (_failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();
    let result = await_health(
        &client(),
        &health_url(port),
        Duration::from_millis(300),
        &mut failure_rx,
    )
    .await;

    let message = result.expect_err("A silent server should time out");
    assert!(message.contains("did not answer"), "message was: {message}");
}

#[test]
fn health_url_targets_loopback() {
    assert_eq!(health_url(3738), "http://127.0.0.1:3738/health");
}

#[cfg(debug_assertions)]
#[test]
#[serial]
fn debug_profile_uses_dev_port_and_trunk_origin() {
    let base = empty_base();

    assert_eq!(api_port(), 3738);
    assert_eq!(spa_origin(), "http://127.0.0.1:8080/");

    clear_base_path();
    drop(base);
}

#[cfg(not(debug_assertions))]
#[test]
#[serial]
fn release_profile_uses_production_port_and_embedded_origin() {
    let base = empty_base();

    assert_eq!(api_port(), 3737);
    assert_eq!(spa_origin(), "http://127.0.0.1:3737/");

    clear_base_path();
    drop(base);
}

// =============================================================================
// Stored settings
// =============================================================================

/// Base path this test alone owns, with no settings file in it.
fn empty_base() -> TempDir {
    let base = TempDir::new().expect("Failed to create base temp dir");
    set_base_path(base.path().to_path_buf());
    base
}

/// Base path this test alone owns, with the settings file written.
fn base_with_config(contents: &str) -> TempDir {
    let base = empty_base();
    let config_dir = get_config_dir();
    std::fs::create_dir_all(&config_dir).expect("config dir creates");
    std::fs::write(get_config_path(), contents).expect("settings file writes");
    base
}

#[test]
#[serial]
fn stored_port_reaches_the_port_decision_point() {
    let base = base_with_config("port = 4000\n");

    let port = api_port();

    clear_base_path();
    drop(base);

    assert_eq!(port, 4000);
}

#[test]
#[serial]
fn stored_values_reach_the_effective_settings() {
    let base = base_with_config(
        "host = \"0.0.0.0\"\nport = 4000\nverbosity = 2\nenable_docs = true\n\
         skills_dir = \"/tmp/stored-skills\"\n",
    );

    let settings = effective_settings();

    clear_base_path();
    drop(base);

    assert_eq!(settings.port, 4000);
    assert_eq!(settings.verbosity, 2);
    assert!(settings.enable_docs);
    assert_eq!(settings.skills_dir, PathBuf::from("/tmp/stored-skills"));
}

#[test]
#[serial]
fn fresh_start_keeps_loopback_and_takes_the_stored_port() {
    let base = base_with_config("host = \"0.0.0.0\"\nport = 4000\nverbosity = 2\n");

    let config = server_config(&effective_settings(), api_port());

    clear_base_path();
    drop(base);

    assert_eq!(config.host.to_string(), "127.0.0.1");
    assert_eq!(config.port, 4000);
    assert_eq!(config.verbosity, 2);
}

#[test]
#[serial]
fn corrupt_settings_file_falls_back_to_the_profile_port() {
    let base = base_with_config("port = \"not-a-number\"\n");

    let port = api_port();

    clear_base_path();
    drop(base);

    assert_eq!(port, profile_api_port());
}

#[test]
#[serial]
fn invalid_stored_port_falls_back_to_the_profile_port() {
    let base = base_with_config("port = 0\n");

    let port = api_port();

    clear_base_path();
    drop(base);

    assert_eq!(port, profile_api_port());
}

#[test]
#[serial]
fn skills_dir_env_beats_the_stored_value() {
    let base = base_with_config("skills_dir = \"/tmp/stored-skills\"\n");
    unsafe { std::env::set_var("C5T_SKILLS_DIR", "/tmp/env-skills") };

    let config = server_config(&effective_settings(), api_port());

    unsafe { std::env::remove_var("C5T_SKILLS_DIR") };
    clear_base_path();
    drop(base);

    assert_eq!(config.skills_dir, PathBuf::from("/tmp/env-skills"));
}

#[cfg(not(debug_assertions))]
#[test]
#[serial]
fn release_spa_origin_follows_the_stored_port() {
    let base = base_with_config("port = 4000\n");

    let origin = spa_origin();

    clear_base_path();
    drop(base);

    assert_eq!(origin, "http://127.0.0.1:4000/");
}
