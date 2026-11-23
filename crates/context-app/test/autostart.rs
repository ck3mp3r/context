//! Tests for the start-at-login LaunchAgent.
//!
//! The plist text is a pure function, so most coverage is exact-string asserts.
//! The file tests run with `HOME` pointed at a temp directory and the launchd
//! seam set to skip, so they never register an agent in the real login session.

use super::*;
use serial_test::serial;
use std::path::PathBuf;
use tempfile::TempDir;

/// Home directory this test alone owns.
fn base_home() -> (TempDir, Option<String>) {
    let home = TempDir::new().expect("Failed to create home temp dir");
    let previous = std::env::var("HOME").ok();
    unsafe { std::env::set_var("HOME", home.path()) };
    (home, previous)
}

/// Sync with launchd inert, so only the plist files move.
fn files_only_sync(enabled: bool) -> Result<(), String> {
    sync(enabled, |_| Ok(()), || Ok(()))
}

/// Put `HOME` back the way it was.
fn restore_home(previous: Option<String>) {
    match previous {
        Some(home) => unsafe { std::env::set_var("HOME", home) },
        None => unsafe { std::env::remove_var("HOME") },
    }
}

#[test]
fn plist_carries_label_program_and_run_at_load() {
    let program = PathBuf::from("/Applications/Context.app/Contents/MacOS/context-service");

    let content = plist_content(&program);

    assert!(
        content.contains("<key>Label</key>\n    <string>wtf.kemper.c5t.service</string>"),
        "label missing, got: {content}"
    );
    assert!(
        content.contains(
            "<key>Program</key>\n    <string>/Applications/Context.app/Contents/MacOS/context-service</string>"
        ),
        "program missing, got: {content}"
    );
    assert!(
        content.contains("<key>RunAtLoad</key>\n    <true/>"),
        "RunAtLoad missing, got: {content}"
    );
}

#[test]
fn plist_has_no_keep_alive() {
    let content = plist_content(&PathBuf::from("/tmp/context-service"));

    assert!(
        !content.contains("KeepAlive"),
        "the system must not respawn the service, got: {content}"
    );
}

#[test]
fn plist_escapes_xml_special_characters() {
    let content = plist_content(&PathBuf::from("/tmp/a&b/<c>"));

    assert!(
        content.contains("<string>/tmp/a&amp;b/&lt;c&gt;</string>"),
        "the program path must be escaped, got: {content}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn generated_plist_passes_plutil() {
    let home = TempDir::new().expect("Failed to create temp dir");
    let plist = home.path().join("agent.plist");
    std::fs::write(
        &plist,
        plist_content(&PathBuf::from("/tmp/context-service")),
    )
    .expect("Plist should write");

    let check = std::process::Command::new("/usr/bin/plutil")
        .arg("-lint")
        .arg(&plist)
        .output()
        .expect("plutil should run");

    assert!(
        check.status.success(),
        "plutil rejected the plist: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
#[serial]
fn plist_path_sits_in_the_launch_agents_directory_of_home() {
    let (home, previous) = base_home();

    let path = launch_agent_plist_path().expect("Path should resolve");
    let expected = home.path().join("Library").join("LaunchAgents");

    restore_home(previous);
    drop(home);

    assert_eq!(
        path.file_name().expect("Plist has a name"),
        "wtf.kemper.c5t.service.plist"
    );
    assert_eq!(path.parent().expect("Plist has a directory"), expected);
}

#[test]
#[serial]
fn enabled_sync_writes_the_plist() {
    let (home, previous) = base_home();

    let outcome = files_only_sync(true);
    let plist = launch_agent_plist_path().expect("Path should resolve");
    let content = std::fs::read_to_string(&plist).unwrap_or_default();
    let program = std::env::current_exe().expect("Current exe should resolve");

    restore_home(previous);
    drop(home);

    assert!(outcome.is_ok(), "sync failed: {outcome:?}");
    assert!(
        content.contains(&format!("<string>{}</string>", program.display())),
        "plist must name the running binary, got: {content}"
    );
}

#[test]
#[serial]
fn disabled_sync_removes_the_plist() {
    let (home, previous) = base_home();

    files_only_sync(true).expect("Writing the plist should succeed");
    let plist = launch_agent_plist_path().expect("Path should resolve");
    let wrote = plist.exists();

    files_only_sync(false).expect("Removing the plist should succeed");
    let remains = plist.exists();

    restore_home(previous);
    drop(home);

    assert!(wrote, "the first sync must create the plist");
    assert!(!remains, "disabling must remove the plist");
}

#[test]
#[serial]
fn disabling_without_a_plist_succeeds() {
    let (home, previous) = base_home();

    let outcome = files_only_sync(false);

    restore_home(previous);
    drop(home);

    assert!(outcome.is_ok(), "a missing plist is the wanted state");
}
