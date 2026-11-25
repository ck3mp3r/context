use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_location,
    path,
};
use leptos_use::core::ConnectionReadyState;
use thaw::*;

use crate::components::{ThemeSwitcher, UpdateBadge, UpdateBanner};
use crate::models::UpdateStatusPayload;
use crate::pages::{
    NoteDetail, Notes, ProjectDetail, Projects, RepoDetail, Repos, Settings, Skills, TaskListDetail,
};
use crate::tabs::{TabContainer, TabRegistry};
use crate::theme::{CatppuccinTheme, apply_theme, load_theme_from_storage};
use crate::websocket::{WebSocketProvider, use_websocket_connection};

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

    // Update state shared by the shell banner, the nav badge, and the settings check.
    let update_status = RwSignal::new(None::<UpdateStatusPayload>);
    provide_context(update_status);

    // One tab registry for the whole shell: the cards open tabs into it and the container renders
    // them, so both read the same state.
    let tabs = TabRegistry::new();
    provide_context(tabs);

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
            <UpdateBanner status=update_status/>
            <nav class="bg-ctp-surface0 border-b border-ctp-surface1 relative">
                <div class="container mx-auto flex justify-between items-center px-6 py-4">
                    <a href="/" class="flex items-center gap-2 hover:opacity-80 transition-opacity">
                        <h1 class="text-3xl font-bold bg-gradient-to-r from-ctp-mauve to-ctp-blue bg-clip-text text-transparent">
                            "context"
                        </h1>
                        <span class="text-xs text-ctp-subtext0 font-mono">
                            {env!("CARGO_PKG_VERSION")}
                        </span>
                        <UpdateBadge status=update_status/>
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
                        </div>
                        <ThemeSwitcher theme=catppuccin_theme/>
                    </div>
                </div>
            </nav>

            <div class="flex-1">
                // Tab surfaces render through the container, so every tab keeps one stable in-tree
                // wrapper. Chrome views keep rendering through the routes below.
                <TabContainer registry=tabs/>

                <Routes fallback=|| view! { <p>"Page not found"</p> }>
                    <Route path=path!("/") view=Projects/>
                    <Route path=path!("/projects/:id") view=ProjectDetail/>
                    <Route path=path!("/projects/:id/task-lists") view=ProjectDetail/>
                    <Route path=path!("/projects/:id/notes") view=ProjectDetail/>
                    <Route path=path!("/projects/:id/skills") view=ProjectDetail/>
                    <Route path=path!("/projects/:id/repos") view=ProjectDetail/>
                    <Route path=path!("/projects/:project_id/task-lists/:task_list_id") view=TaskListDetail/>
                    <Route path=path!("/projects/:project_id/notes/:id") view=NoteDetail/>
                    <Route path=path!("/projects/:project_id/repos/:id") view=RepoDetail/>
                    <Route path=path!("/notes") view=Notes/>
                    <Route path=path!("/notes/:id") view=NoteDetail/>
                    <Route path=path!("/repos") view=Repos/>
                    <Route path=path!("/repos/:id") view=RepoDetail/>
                    <Route path=path!("/skills") view=Skills/>
                    <Route path=path!("/skills/:id") view=Skills/>
                    <Route path=path!("/settings") view=Settings/>
                </Routes>
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
