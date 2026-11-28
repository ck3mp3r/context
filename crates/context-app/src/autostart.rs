//! Start-at-login: a launchd LaunchAgent for the service.
//!
//! The service is a GUI-session agent, so a per-user LaunchAgent in
//! `~/Library/LaunchAgents` registers it: `RunAtLoad` starts it at login, and the
//! absence of `KeepAlive` leaves restarts to the user. The stored
//! `start_at_login` field is the switch, applied when the service starts — the
//! same restart boundary every other stored setting uses.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Canonical label of the agent: the bundle identity plus the service suffix.
pub const LAUNCH_AGENT_LABEL: &str = "wtf.kemper.c5t.service";

/// Path of the agent launcher.
const LAUNCHCTL: &str = "/bin/launchctl";

#[cfg(test)]
#[path = "../test/autostart.rs"]
mod autostart_test;

// region:    --- Paths and content

/// Path of the agent plist under the user's LaunchAgents directory.
///
/// `HOME` decides the directory, so a test can point it at a temp directory and
/// keep the real login session untouched.
pub fn launch_agent_plist_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;

    Ok(Path::new(&home)
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist")))
}

/// Plist that starts `program` at login.
///
/// Minimal on purpose: label, program, and `RunAtLoad`. No `KeepAlive`, so the
/// system never respawns the service behind the user's back.
pub fn plist_content(program: &Path) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n",
            "<dict>\n",
            "    <key>Label</key>\n",
            "    <string>{}</string>\n",
            "    <key>Program</key>\n",
            "    <string>{}</string>\n",
            "    <key>RunAtLoad</key>\n",
            "    <true/>\n",
            "</dict>\n",
            "</plist>\n"
        ),
        escape_xml(LAUNCH_AGENT_LABEL),
        escape_xml(&program.display().to_string())
    )
}

/// Escape the characters an XML text node cannot carry literally.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// endregion: --- Paths and content

// region:    --- Sync

/// Sync the LaunchAgent with the stored `start_at_login` preference.
///
/// Enabled writes the plist and registers the agent; disabled unregisters it and
/// removes the plist. The caller logs a failure and keeps serving: a login item
/// must never block the server.
pub fn sync_start_at_login(enabled: bool) -> Result<(), String> {
    sync(enabled, register, unregister)
}

/// Sync the files and let `register`/`unregister` do the launchd work.
///
/// The two functions are the seam for tests: a test passes no-ops, so it writes
/// and removes the plist without touching the login session of the machine.
pub(crate) fn sync(
    enabled: bool,
    register: impl Fn(&Path) -> Result<(), String>,
    unregister: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    if enabled {
        let plist = launch_agent_plist_path()?;
        let program = std::env::current_exe()
            .map_err(|error| format!("Failed to locate the service binary: {error}"))?;

        write_plist(&plist, &plist_content(&program))?;
        register(&plist)
    } else {
        unregister()?;
        remove_plist(&launch_agent_plist_path()?)
    }
}

// endregion: --- Sync

// region:    --- Files

/// Write `content` to `path`, creating the LaunchAgents directory.
fn write_plist(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    std::fs::write(path, content)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

/// Remove `path`; a missing file is already the wanted state.
fn remove_plist(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to remove {}: {error}", path.display())),
    }
}

// endregion: --- Files

// region:    --- Launchd

/// The per-user GUI domain of this login session.
#[cfg(target_os = "macos")]
fn gui_domain() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

/// Register the agent.
///
/// A bootstrap on an agent that is already registered fails; that is the state
/// the caller wants, so the failure is checked against `launchctl print`.
#[cfg(target_os = "macos")]
fn register(plist: &Path) -> Result<(), String> {
    let output = Command::new(LAUNCHCTL)
        .args(["bootstrap", &gui_domain()])
        .arg(plist)
        .output()
        .map_err(|error| format!("Failed to run launchctl: {error}"))?;

    if output.status.success() || is_registered() {
        return Ok(());
    }

    Err(format!(
        "launchctl bootstrap failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Unregister the agent; an agent that is already gone is the wanted state.
#[cfg(target_os = "macos")]
fn unregister() -> Result<(), String> {
    let service = format!("{}/{}", gui_domain(), LAUNCH_AGENT_LABEL);

    let output = Command::new(LAUNCHCTL)
        .args(["bootout", &service])
        .output()
        .map_err(|error| format!("Failed to run launchctl: {error}"))?;

    if output.status.success() || !is_registered() {
        return Ok(());
    }

    Err(format!(
        "launchctl bootout failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Whether launchd knows the agent in this session.
#[cfg(target_os = "macos")]
fn is_registered() -> bool {
    Command::new(LAUNCHCTL)
        .args(["print", &format!("{}/{}", gui_domain(), LAUNCH_AGENT_LABEL)])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Start-at-login is a macOS login item: other hosts keep the plist out of the
/// way and change nothing at runtime.
#[cfg(not(target_os = "macos"))]
fn register(_plist: &Path) -> Result<(), String> {
    Ok(())
}

/// Start-at-login is a macOS login item: nothing to unregister elsewhere.
#[cfg(not(target_os = "macos"))]
fn unregister() -> Result<(), String> {
    Ok(())
}

// endregion: --- Launchd
