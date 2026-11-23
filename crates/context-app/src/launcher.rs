//! Launch-or-focus of the GUI from the service tray.
//!
//! The service owns the tray and never shows a window. Its Show and Settings
//! items bring the GUI forward: a GUI that already runs is activated through
//! LaunchServices, and a GUI that does not run is spawned as a sibling with the
//! route in argv. Quit ends the GUIs this service spawned, then the service.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// Argument that carries the SPA route into the GUI.
pub const OPEN_ROUTE_FLAG: &str = "--open-route";
/// How long a tracked GUI gets to exit after SIGTERM before Quit moves on.
const TERMINATE_GRACE: Duration = Duration::from_millis(500);

#[cfg(test)]
#[path = "../test/launcher.rs"]
mod launcher_test;

// region:    --- Routes

/// Arguments that make the GUI land on `route`.
///
/// No route means no argument: the GUI opens at the SPA root.
pub fn gui_launch_args(route: Option<&str>) -> Vec<String> {
    match route {
        Some(route) => vec![OPEN_ROUTE_FLAG.to_string(), route.to_string()],
        None => Vec::new(),
    }
}

/// Route the GUI was launched with, read from its arguments.
///
/// Only on-origin paths are accepted: a route starts with `/` and not with
/// `//`, which a URL join would read as another host.
pub fn open_route(arguments: impl IntoIterator<Item = String>) -> Option<String> {
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        if argument == OPEN_ROUTE_FLAG {
            return arguments.next().filter(|route| is_valid_route(route));
        }
    }

    None
}

/// Whether `route` stays on the SPA origin.
fn is_valid_route(route: &str) -> bool {
    route.starts_with('/') && !route.starts_with("//")
}

// endregion: --- Routes

// region:    --- Launch

/// Path of the sibling GUI binary: next to this executable.
///
/// The bundler puts both binaries in `Contents/MacOS`, and `cargo build` puts
/// both in `target/debug`, so the GUI is always a directory away.
pub(crate) fn sibling_gui_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate this executable: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| format!("{} has no directory", executable.display()))?;

    Ok(directory.join("context"))
}

/// Whether a GUI process is already running.
///
/// `pgrep -x` matches the process name exactly, so this service and the other
/// `context*` helpers never count as a GUI.
pub(crate) fn gui_running() -> bool {
    Command::new("/usr/bin/pgrep")
        .args(["-x", "context"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Ask LaunchServices to open or focus the bundled GUI.
///
/// A bundled GUI has a registered name, and LaunchServices focuses the running
/// instance instead of starting a second one. A dev build has no bundle, so the
/// command fails and the caller falls back to the sibling spawn.
fn activate_bundled_gui() -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", "context"])
        .status()
        .is_ok_and(|status| status.success())
}

/// Spawn the sibling GUI, optionally landing it on `route`.
fn spawn_gui(route: Option<&str>) -> Result<Child, String> {
    let gui = sibling_gui_path()?;

    let mut command = Command::new(&gui);
    command.args(gui_launch_args(route));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", gui.display()))
}

// endregion: --- Launch

// region:    --- ServiceState

/// GUIs the service launched, so Quit can end them.
///
/// Only children of this service are tracked. A GUI the user opened from Finder
/// or a terminal is not a child: the service cannot signal it, and that GUI shows
/// the connection-lost state once the service is gone.
#[derive(Default)]
pub struct ServiceState {
    children: Mutex<Vec<Child>>,
}

impl ServiceState {
    /// Empty state: no GUI launched yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bring the GUI forward: focus a running one, else spawn it on `route`.
    ///
    /// A bundled GUI activates through LaunchServices, which also focuses the
    /// running instance. Without a bundle — a dev build — that path fails, so
    /// the spawn path takes over: the running GUI focuses through its own
    /// single-instance callback, and a missing GUI starts fresh on `route`.
    pub fn launch_gui(&self, route: Option<&str>) {
        if gui_running() && activate_bundled_gui() {
            return;
        }

        match spawn_gui(route) {
            Ok(child) => self.track(child),
            Err(error) => eprintln!("context-service: {error}"),
        }
    }

    /// Remember a GUI process, so Quit can end it.
    fn track(&self, child: Child) {
        if let Ok(mut children) = self.children.lock() {
            children.push(child);
        }
    }

    /// Send SIGTERM to every spawned GUI that still runs.
    ///
    /// A child that already exited is skipped. The tracked set is emptied: the
    /// service quits right after this call.
    pub fn terminate_children(&self) {
        let Ok(mut children) = self.children.lock() else {
            return;
        };

        for child in children.iter_mut() {
            terminate_child(child);
        }

        children.clear();
    }
}

/// Send SIGTERM to `child` and reap it within [`TERMINATE_GRACE`].
fn terminate_child(child: &mut Child) {
    if !matches!(child.try_wait(), Ok(None)) {
        return;
    }

    terminate(child.id());

    let deadline = std::time::Instant::now() + TERMINATE_GRACE;
    while std::time::Instant::now() < deadline {
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Send SIGTERM to `pid`.
///
/// SIGTERM is the clean quit signal; `Child::kill` would send SIGKILL and skip
/// the GUI's own teardown.
#[cfg(unix)]
fn terminate(pid: u32) {
    // A kill on a process that is already gone is a harmless failure.
    let _ = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
}

/// Platforms without SIGTERM cannot signal a child politely.
#[cfg(not(unix))]
fn terminate(_pid: u32) {}

// endregion: --- ServiceState
