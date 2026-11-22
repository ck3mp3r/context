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
