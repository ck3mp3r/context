//! The route classifier: what a pathname means to the tab shell.
//!
//! Three kinds matter. A chrome-global path (`/skills`, `/skills/:id`, `/settings`) hides every tab
//! and shows its own list. The projects home (`/`) shows the projects overview beside the opened
//! project tabs, in row one only. The notes home (`/notes`) shows the notes list beside the notes
//! bar, and no project row. The repos home (`/repos`) shows the repos list beside the repos bar, and
//! no project row. A tab surface is the path a tab names, so that tab paints. Any other path is
//! unknown, and the shell does nothing with it: a URL the app did not write never moves the active
//! tab, which is what keeps `pushState` and Back inert.
//!
//! This module is pure. It holds no DOM and no reactive runtime, so the host test binary compiles it
//! directly and pins the table without a browser.

use super::session::boot_surface;

/// What the shell does with the pathname the address bar holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    /// The projects overview: row one of the strip renders, with no active chip and no row two.
    ProjectsHome,
    /// The notes list: the notes bar renders, with no active chip and no project row.
    NotesHome,
    /// The repos list: the repos bar renders, with no active chip and no project row.
    ReposHome,
    /// A chrome list or the settings page: every tab hides and the list renders alone.
    ChromeGlobal,
    /// A surface a tab names: the tab paints and the chrome views render nothing.
    TabSurface,
    /// A path the app has no route for. The shell leaves the registry alone.
    Unknown,
}

impl RouteKind {
    /// Whether this route is chrome: the strip is not a tab surface, so no tab paints.
    ///
    /// Every chrome route deactivates the registry, both homes included.
    pub fn is_chrome(self) -> bool {
        matches!(
            self,
            Self::ProjectsHome | Self::NotesHome | Self::ReposHome | Self::ChromeGlobal
        )
    }
}

/// Classify a pathname.
///
/// The tab-surface arm delegates to [`boot_surface`], so the paths a tab writes and the paths this
/// classifier accepts cannot drift apart. A chrome route names no tab surface, so the chrome arms
/// come first.
pub fn route_kind(pathname: &str) -> RouteKind {
    let segments: Vec<&str> = pathname
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match segments.as_slice() {
        [] => RouteKind::ProjectsHome,
        ["notes"] => RouteKind::NotesHome,
        ["repos"] => RouteKind::ReposHome,
        ["skills"] | ["settings"] => RouteKind::ChromeGlobal,
        ["skills", _] => RouteKind::ChromeGlobal,
        _ if boot_surface(pathname).is_some() => RouteKind::TabSurface,
        _ => RouteKind::Unknown,
    }
}
