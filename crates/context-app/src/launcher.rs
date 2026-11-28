//! Launch-or-focus of the GUI from the service tray.
//!
//! The service owns the tray and never shows a window. Its Show and Settings
//! items bring the GUI forward: a GUI that already runs is activated through
//! LaunchServices, and a GUI that does not run is spawned as a sibling with the
//! route in argv and then activated through LaunchServices once it appears. A dev
//! build has no registered app, so a dev GUI relies on its own startup
//! activation. An update restart replaces the GUI instead: every GUI process of
//! this profile ends before the replacement starts. Quit ends the GUIs this
//! service spawned, then the service.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// Argument that carries the SPA route into the GUI.
pub const OPEN_ROUTE_FLAG: &str = "--open-route";
/// How long a tracked GUI gets to exit after SIGTERM before Quit moves on.
const TERMINATE_GRACE: Duration = Duration::from_millis(500);
/// How long a replaced GUI gets to exit after SIGTERM before it is killed.
///
/// The whole replace step stays under 0.8 s: the swapped service waits one
/// second before it execs, so the old service must be gone well inside it.
const REPLACE_GRACE: Duration = Duration::from_millis(500);
/// Delay between the polls that wait for a replaced GUI to exit.
const REPLACE_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Polls that wait for a freshly spawned GUI to appear before activating it.
const ACTIVATION_POLLS: usize = 10;
/// Delay between the polls that wait for a freshly spawned GUI.
const ACTIVATION_POLL_INTERVAL: Duration = Duration::from_millis(200);

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

/// File name of the GUI binary for the current build profile.
///
/// `cargo build` names the GUI `context`, and the bundle renames it to
/// `Context` through `mainBinaryName`, so every name-based path, process probe,
/// and LaunchServices lookup follows the profile.
fn gui_binary_name() -> &'static str {
    if cfg!(debug_assertions) {
        "context"
    } else {
        "Context"
    }
}

/// Path of the sibling GUI binary: next to this executable.
///
/// The bundler puts both binaries in `Contents/MacOS`, and `cargo build` puts
/// both in `target/debug`, so the GUI is always a directory away. The file name
/// follows `gui_binary_name()`: `Context` in the bundle, `context` in a dev
/// build.
pub(crate) fn sibling_gui_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate this executable: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| format!("{} has no directory", executable.display()))?;

    Ok(directory.join(gui_binary_name()))
}

/// Pids of the running GUI processes of this profile.
///
/// `pgrep -x` matches the process name exactly, so the probe uses the profile
/// name from `gui_binary_name()`: `Context` in the bundle, `context` in a dev
/// build. This service and the other `context*` helpers never count as a GUI.
/// The pids are what a replace step needs: a GUI that runs is a GUI to end.
pub(crate) fn running_gui_pids() -> Vec<u32> {
    let Ok(output) = Command::new("/usr/bin/pgrep")
        .args(["-x", gui_binary_name()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|pid| pid.trim().parse().ok())
        .collect()
}

/// Whether a GUI process is already running.
pub(crate) fn gui_running() -> bool {
    !running_gui_pids().is_empty()
}

/// Ask LaunchServices to open or focus the bundled GUI.
///
/// A bundled GUI has a registered name, and LaunchServices focuses the running
/// instance instead of starting a second one. The name follows
/// `gui_binary_name()`: the installed bundle registers as `Context`, and a dev
/// build has no bundle, so the command fails and the caller falls back to the
/// sibling spawn.
fn activate_bundled_gui() -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", gui_binary_name()])
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

/// Ask LaunchServices to activate the GUI this service just spawned.
///
/// The spawned GUI is a plain child process, not a LaunchServices launch, so it
/// never comes to the front on its own. The menu click is the user event
/// LaunchServices wants, so the service relays it: a detached thread waits for
/// the child to appear and then activates the bundled GUI once.
fn activate_spawned_gui() {
    std::thread::spawn(|| {
        for _ in 0..ACTIVATION_POLLS {
            if gui_running() {
                break;
            }
            std::thread::sleep(ACTIVATION_POLL_INTERVAL);
        }

        if !activate_bundled_gui() {
            eprintln!("context-service: LaunchServices could not activate the spawned GUI");
        }
    });
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
    /// single-instance callback, and a missing GUI starts fresh on `route` and
    /// asks LaunchServices to activate it once it appears.
    pub fn launch_gui(&self, route: Option<&str>) {
        if gui_running() && activate_bundled_gui() {
            return;
        }

        match spawn_gui(route) {
            Ok(child) => {
                self.track(child);
                activate_spawned_gui();
            }
            Err(error) => eprintln!("context-service: {error}"),
        }
    }

    /// Replace the GUI: end every running GUI of this profile, then spawn one.
    ///
    /// Unlike [`Self::launch_gui`] there is no focus-or-nothing branch in front
    /// of the spawn: an update restart must land the swapped binary, so a GUI
    /// that already runs is the one thing that must not stop it. The tracked
    /// children end first, then the pids [`running_gui_pids`] reports, then
    /// whoever outlived [`REPLACE_GRACE`] is killed.
    ///
    /// The spawned GUI is not tracked: the service exits right after this call,
    /// and a dropped [`Child`] kills nothing.
    pub fn replace_gui(&self) {
        self.replace_gui_with(running_gui_pids, || spawn_gui(None));
    }

    /// Replace the GUI with the process probe and the spawn supplied as seams.
    ///
    /// `discover` names the GUIs this service does not track: the survivor of an
    /// earlier restart, re-parented to launchd, or a GUI the user opened from
    /// Finder. `spawn` keeps a test away from a real GUI binary.
    fn replace_gui_with<D, S>(&self, discover: D, spawn: S)
    where
        D: Fn() -> Vec<u32>,
        S: FnOnce() -> Result<Child, String>,
    {
        let deadline = std::time::Instant::now() + REPLACE_GRACE;
        let mut children = self.take_children();
        let targets = replace_targets(&mut children, discover());

        for pid in &targets {
            terminate(*pid);
        }

        for pid in await_absence(&mut children, &targets, deadline) {
            force_terminate(pid);
        }

        match spawn() {
            Ok(_untracked) => activate_spawned_gui(),
            Err(error) => eprintln!("context-service: {error}"),
        }
    }

    /// Take the tracked children out of the state, so the replace step owns them.
    fn take_children(&self) -> Vec<Child> {
        let Ok(mut children) = self.children.lock() else {
            return Vec::new();
        };

        std::mem::take(&mut children)
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

/// Pids the replace step ends: the tracked GUIs that still run and the ones the
/// probe found.
///
/// A tracked child that already exited is reaped here and never becomes a
/// target. This service's own pid is dropped: the probe matches by name, and a
/// process never signals itself.
fn replace_targets(children: &mut [Child], discovered: Vec<u32>) -> Vec<u32> {
    let own_pid = std::process::id();

    let tracked = children
        .iter_mut()
        .filter_map(|child| match child.try_wait() {
            Ok(None) => Some(child.id()),
            _ => None,
        });

    let mut targets: Vec<u32> = tracked
        .chain(discovered)
        .filter(|pid| *pid != own_pid)
        .collect();
    targets.sort_unstable();
    targets.dedup();

    targets
}

/// Wait until every target has exited, reaping the tracked children on the way.
///
/// Returns the pids that still run when `deadline` passes. Tracked and
/// discovered pids share one budget: a GUI that ignores SIGTERM cannot stretch
/// the step past it.
fn await_absence(
    children: &mut [Child],
    targets: &[u32],
    deadline: std::time::Instant,
) -> Vec<u32> {
    loop {
        for child in children.iter_mut() {
            let _ = child.try_wait();
        }

        let survivors: Vec<u32> = targets
            .iter()
            .copied()
            .filter(|pid| process_running(*pid))
            .collect();

        if survivors.is_empty() || std::time::Instant::now() >= deadline {
            return survivors;
        }

        std::thread::sleep(REPLACE_POLL_INTERVAL);
    }
}

/// Kill `pid` after it outlived the SIGTERM grace.
///
/// The step continues either way: a GUI that cannot be ended must not keep the
/// swapped service from starting.
fn force_terminate(pid: u32) {
    eprintln!("context-service: GUI {pid} outlived the SIGTERM grace; sending SIGKILL");

    #[cfg(unix)]
    {
        let _ = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    }
}

/// Whether `pid` still runs.
///
/// Signal 0 probes for the process without touching it.
#[cfg(unix)]
fn process_running(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Platforms without signals cannot probe a process.
#[cfg(not(unix))]
fn process_running(_pid: u32) -> bool {
    false
}

// endregion: --- ServiceState
