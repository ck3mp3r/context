//! Tests for the build-profile endpoints and the health probe.

use std::io::{Read, Write};
use std::net::TcpListener;

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
fn debug_profile_uses_dev_port_and_trunk_origin() {
    assert_eq!(api_port(), 3738);
    assert_eq!(spa_origin(), "http://127.0.0.1:8080/");
}

#[cfg(not(debug_assertions))]
#[test]
fn release_profile_uses_production_port_and_embedded_origin() {
    assert_eq!(api_port(), 3737);
    assert_eq!(spa_origin(), "http://127.0.0.1:3737/");
}
