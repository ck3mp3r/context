use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::{use_location, use_navigate},
    path,
};
use leptos_use::core::ConnectionReadyState;
use thaw::*;

use crate::components::{SyncIndicator, ThemeSwitcher, UpdatePill};
use crate::models::{UpdateCheckOutcome, UpdateMessage, UpdateStatusPayload};
use crate::pages::{Notes, Projects, Repos, Settings, Skills};
use crate::sync_status::SyncIndicator as SyncState;
use crate::tabs::{
    TabContainer, TabKey, TabRegistry, TabStrip, TabTitles, adopt_owned_url, boot_surface,
    finish_boot, restore_or_boot, route_kind, start_keyboard, start_session_persistence,
    start_window_title, surface_url, write_owned_url,
};
use crate::theme::{CatppuccinTheme, apply_theme, load_theme_from_storage};
use crate::websocket::{WebSocketProvider, use_websocket_connection, use_websocket_updates};

#[component]
pub fn App() -> impl IntoView {
    // Load saved theme from localStorage or use default (Mocha)
    let catppuccin_theme = RwSignal::new(load_theme_from_storage());

    // Apply initial theme immediately (before rendering)
    apply_theme(catppuccin_theme.get_untracked());

    // Sync theme changes
    Effect::new(move || {
        let theme = catppuccin_theme.get();
        apply_theme(theme);
    });

    // Map Catppuccin theme to Thaw UI theme
    let thaw_theme = RwSignal::new(catppuccin_theme.get_untracked().to_thaw_theme());

    // Sync Catppuccin theme changes to Thaw theme
    Effect::new(move || {
        thaw_theme.set(catppuccin_theme.get().to_thaw_theme());
    });

    // One non-manual check per mount: the service skips it while auto-update
    // is off, so this stays quiet unless the user asked for checks.
    Effect::new(move |_| {
        spawn_local(async move {
            if let Err(error) = crate::api::update::check(false).await {
                web_sys::console::error_1(
                    &format!("Failed to trigger the update check: {error}").into(),
                );
            }
        });
    });

    view! {
        <ConfigProvider theme=thaw_theme>
            <WebSocketProvider>
            <AppContent catppuccin_theme/>
            </WebSocketProvider>
        </ConfigProvider>
    }
}

#[component]
fn AppContent(catppuccin_theme: RwSignal<CatppuccinTheme>) -> impl IntoView {
    view! {
        <Router>
            <NavAndContent catppuccin_theme/>
        </Router>
    }
}

#[component]
fn NavAndContent(catppuccin_theme: RwSignal<CatppuccinTheme>) -> impl IntoView {
    // WebSocket connection status (from context)
    let ws_state = use_websocket_connection();
    let location = use_location();

    // Update state shared by the pill and the settings check.
    let update_status = RwSignal::new(None::<UpdateStatusPayload>);
    provide_context(update_status);

    // Outcome of a manual check that reported no state of its own: the pill renders it, so a click
    // that finds nothing or fails no longer ends silently.
    let check_outcome = RwSignal::new(None::<UpdateCheckOutcome>);
    provide_context(check_outcome);

    // Git-sync state shared by the header indicator and the settings sync section: one state
    // machine drives both surfaces. `None` means the app does not know the state yet, or sync is
    // not set up, so no surface renders.
    let sync_state = RwSignal::new(None::<SyncState>);
    provide_context(sync_state);

    // One tab registry for the whole shell: the cards open tabs into it and the container renders
    // them, so both read the same state.
    let tabs = TabRegistry::new();
    provide_context(tabs);

    // The label of every open tab: restored from the session, re-resolved by id (decision 14).
    let titles = TabTitles::new();
    provide_context(titles);

    // Boot: one storage read, one plan, one install, before the first render. Only the active tab
    // mounts, so a restored strip costs one label request per tab and no surface fetch. The boot
    // surface opens once its eligibility is known: a `/notes/{id}` deep link names a tab only when
    // the note is top-level, so that fetch comes first. Nothing here writes a URL; the one writer
    // below adopts the boot URL as the URL it already owns.
    let boot = boot_surface(&location.pathname.get_untracked());
    let pending = restore_or_boot(tabs, titles, boot);
    spawn_local(async move { finish_boot(tabs, titles, pending).await });
    start_session_persistence(tabs, titles);

    // The one native seam (decisions P and 18): the window title follows the active tab through one
    // best-effort `setTitle` call. Nothing else of the shell crosses into the native window.
    start_window_title(tabs, titles);

    // The deletion reaction: a surface deleted over the websocket closes its tab. Cascade and
    // neighbour activation are the model's close rules; this reaction writes no URL, because the
    // one writer below names whatever tab survives. Every other update falls through to the tab's
    // own refetch handler.
    let ws_updates = use_websocket_updates();
    Effect::new(move |_| {
        let Some(update) = ws_updates.get() else {
            return;
        };
        let key = match update {
            UpdateMessage::NoteDeleted { note_id } => TabKey::note(note_id),
            UpdateMessage::RepoDeleted { repo_id } => TabKey::repo(repo_id),
            UpdateMessage::TaskListDeleted { task_list_id } => TabKey::task_list(task_list_id),
            _ => return,
        };
        // The lookup and the close are untracked, so this effect reacts to a new message only: a
        // surface the user reopens after its deletion stays open.
        untrack(|| {
            if let Some((id, _)) = tabs
                .descriptors()
                .into_iter()
                .find(|(_, open)| open == &key)
            {
                tabs.close(id);
            }
        });
    });

    // Chrome navigation deactivates the strip: the tab set stays open in the registry, no tab
    // paints, and the chrome view below renders alone, so exactly one surface shows (decisions 5 and
    // 6). Only a known chrome route deactivates, so a URL the app did not write — `pushState`, Back —
    // moves no active tab. The first run is the boot mount, and it is skipped, so a session-restored
    // active tab survives the boot URL (decision 14).
    //
    // Clearing focus writes no URL here. The writer below sees no active tab and a strip that is not
    // empty, and leaves the address bar on the chrome view the user asked for. The app adopts that
    // chrome URL instead, so returning to a tab writes the tab's URL again.
    let owned = RwSignal::new(location.pathname.get_untracked());
    let mut booted = false;
    Effect::new(move |_| {
        let path = location.pathname.get();
        if !booted {
            booted = true;
            return;
        }
        if !route_kind(&path).is_chrome() {
            return;
        }
        tabs.deactivate();
        adopt_owned_url(&owned, &path);
    });

    // The one URL writer: the active tab names the URL, written once with `replace` (decisions 11
    // and 21). Nothing here reads the address bar, so a mutation the app did not make cannot move
    // the active tab.
    //
    // An empty strip names no surface, so the last close writes chrome home. A strip that is merely
    // deactivated names no surface either, but it is not empty: chrome navigation keeps every tab, so
    // that transition writes nothing and the user stays on the chrome view they chose.
    //
    // The URL the app owns starts as the boot URL, read once (decision 11), so a restore writes no
    // navigation and cannot overwrite a deep link (decision 14).
    let url_writer = use_navigate();
    let mut had_active = false;
    Effect::new(move |_| {
        let Some(key) = tabs.active_key() else {
            if had_active {
                had_active = false;
                if tabs.is_empty() {
                    write_owned_url(&url_writer, &owned, "/");
                }
            }
            return;
        };
        had_active = true;
        let url = surface_url(&key, tabs.active_project().as_deref());
        write_owned_url(&url_writer, &owned, &url);
    });

    // The one keyboard transport (decisions 9, 10, 16, 17): installed once for the window, so the
    // ring, the tab close, and the chrome shortcut all read the same registry as the strip.
    let keyboard = start_keyboard(tabs, use_navigate());
    on_cleanup(move || keyboard.remove());

    // Determine active tab based on current path
    let is_active = move |path: &str| {
        let current = location.pathname.get();
        if path == "/" {
            current == "/" || current.starts_with("/projects")
        } else {
            current.starts_with(path)
        }
    };

    view! {
        <main class="min-h-screen bg-ctp-base flex flex-col">
            // WebSocket connection status bar (top of page)
            <Tooltip content=move || {
                match ws_state.get() {
                    ConnectionReadyState::Open => "Connected",
                    ConnectionReadyState::Connecting => "Connecting...",
                    ConnectionReadyState::Closing => "Closing...",
                    ConnectionReadyState::Closed => "Disconnected",
                }
            }>
                <div class="h-1 cursor-help"
                    class:bg-ctp-green=move || matches!(ws_state.get(), ConnectionReadyState::Open)
                    class:bg-ctp-yellow=move || matches!(ws_state.get(), ConnectionReadyState::Connecting | ConnectionReadyState::Closing)
                    class:bg-ctp-red=move || matches!(ws_state.get(), ConnectionReadyState::Closed)>
                </div>
            </Tooltip>
            <UpdatePill status=update_status outcome=check_outcome/>
            <nav class="bg-ctp-surface0 border-b border-ctp-surface1 relative">
                <div class="container mx-auto flex justify-between items-center px-6 py-4">
                    <a href="/" class="flex items-center gap-2 hover:opacity-80 transition-opacity">
                        <h1 class="text-3xl font-bold bg-gradient-to-r from-ctp-mauve to-ctp-blue bg-clip-text text-transparent">
                            "context"
                        </h1>
                        <span class="text-xs text-ctp-subtext0 font-mono">
                            {env!("CARGO_PKG_VERSION")}
                        </span>
                    </a>
                    <div class="flex gap-4 items-center">
                        <div class="flex gap-2 items-center">
                            <a href="/"
                                class="px-4 py-2 rounded-lg font-medium transition-colors"
                                class:bg-ctp-surface2=move || is_active("/")
                                class:text-ctp-text=move || is_active("/")
                                class:text-ctp-subtext1=move || !is_active("/")
                                class:hover:bg-ctp-surface1=move || !is_active("/")
                                class:hover:text-ctp-text=move || !is_active("/")>
                                "Projects"
                            </a>
                            <a href="/notes"
                                class="px-4 py-2 rounded-lg font-medium transition-colors"
                                class:bg-ctp-surface2=move || is_active("/notes")
                                class:text-ctp-text=move || is_active("/notes")
                                class:text-ctp-subtext1=move || !is_active("/notes")
                                class:hover:bg-ctp-surface1=move || !is_active("/notes")
                                class:hover:text-ctp-text=move || !is_active("/notes")>
                                "Notes"
                            </a>
                            <a href="/skills"
                                class="px-4 py-2 rounded-lg font-medium transition-colors"
                                class:bg-ctp-surface2=move || is_active("/skills")
                                class:text-ctp-text=move || is_active("/skills")
                                class:text-ctp-subtext1=move || !is_active("/skills")
                                class:hover:bg-ctp-surface1=move || !is_active("/skills")
                                class:hover:text-ctp-text=move || !is_active("/skills")>
                                "Skills"
                            </a>
                            <a href="/repos"
                                class="px-4 py-2 rounded-lg font-medium transition-colors"
                                class:bg-ctp-surface2=move || is_active("/repos")
                                class:text-ctp-text=move || is_active("/repos")
                                class:text-ctp-subtext1=move || !is_active("/repos")
                                class:hover:bg-ctp-surface1=move || !is_active("/repos")
                                class:hover:text-ctp-text=move || !is_active("/repos")>
                                "Repos"
                            </a>
                            <a href="/settings"
                                class="px-4 py-2 rounded-lg font-medium transition-colors flex items-center"
                                title="Settings"
                                aria-label="Settings"
                                class:bg-ctp-surface2=move || is_active("/settings")
                                class:text-ctp-text=move || is_active("/settings")
                                class:text-ctp-subtext1=move || !is_active("/settings")
                                class:hover:bg-ctp-surface1=move || !is_active("/settings")
                                class:hover:text-ctp-text=move || !is_active("/settings")>
                                <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                                    <circle cx="12" cy="12" r="3"></circle>
                                    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
                                </svg>
                            </a>
                            <SyncIndicator/>
                        </div>
                        <ThemeSwitcher theme=catppuccin_theme/>
                    </div>
                </div>
            </nav>

            <div class="flex-1">
                // The strip sits above the surfaces it names. Its route decides what it shows:
                // nothing on a chrome list, row one on the projects overview, both rows on a tab
                // surface.
                <TabStrip registry=tabs titles=titles/>

                // Tab surfaces render through the container, so every tab keeps one stable in-tree
                // wrapper. Exactly one surface paints: chrome navigation deactivates every wrapper,
                // and a tab URL matches no detail route, so the fallback below renders nothing under
                // a tab.
                <TabContainer registry=tabs/>

                <div data-routes-area="">
                    <Routes fallback=|| ()>
                        <Route path=path!("/") view=Projects/>
                        <Route path=path!("/notes") view=Notes/>
                        <Route path=path!("/repos") view=Repos/>
                        <Route path=path!("/skills") view=Skills/>
                        <Route path=path!("/skills/:id") view=Skills/>
                        <Route path=path!("/settings") view=Settings/>
                    </Routes>
                </div>
            </div>

            <footer class="screen-only py-3 px-6 border-t border-ctp-surface1 bg-ctp-surface0">
                <div class="container mx-auto text-center text-xs text-ctp-overlay0">
                    "© " {
                        let date = web_sys::js_sys::Date::new_0();
                        date.get_full_year()
                    } " Christian Kemper. Licensed under "
                    <a href="https://www.gnu.org/licenses/old-licenses/gpl-2.0.html" target="_blank" rel="noopener noreferrer"
                        class="text-ctp-blue hover:text-ctp-lavender underline">
                        "GPL v2"
                    </a>
                    "."
                </div>
            </footer>
        </main>
    }
}
