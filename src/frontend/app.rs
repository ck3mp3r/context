use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_location,
    path,
};
use leptos_use::core::ConnectionReadyState;
use thaw::*;

use crate::pages::{Notes, ProjectDetail, Projects, Repos};
use crate::websocket::{WebSocketProvider, use_websocket_connection};

#[component]
pub fn App() -> impl IntoView {
    // Set dark theme for Thaw UI components
    let theme = RwSignal::new(Theme::dark());

    view! {
        <ConfigProvider theme>
            <WebSocketProvider>
            <AppContent/>
            </WebSocketProvider>
        </ConfigProvider>
    }
}

#[component]
fn AppContent() -> impl IntoView {
    view! {
        <Router>
            <NavAndContent/>
        </Router>
    }
}

#[component]
fn NavAndContent() -> impl IntoView {
    // WebSocket connection status (from context)
    let ws_state = use_websocket_connection();
    let location = use_location();

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
            <nav class="bg-ctp-surface0 border-b border-ctp-surface1 relative">
                // WebSocket connection status bar (left edge)
                <Tooltip content=move || {
                    match ws_state.get() {
                        ConnectionReadyState::Open => "Connected",
                        ConnectionReadyState::Connecting => "Connecting...",
                        ConnectionReadyState::Closing => "Closing...",
                        ConnectionReadyState::Closed => "Disconnected",
                    }
                }>
                    <div class="absolute left-0 top-0 bottom-0 w-2 cursor-help"
                        class:bg-ctp-green=move || matches!(ws_state.get(), ConnectionReadyState::Open)
                        class:bg-ctp-yellow=move || matches!(ws_state.get(), ConnectionReadyState::Connecting | ConnectionReadyState::Closing)
                        class:bg-ctp-red=move || matches!(ws_state.get(), ConnectionReadyState::Closed)>
                    </div>
                </Tooltip>
                <div class="container mx-auto flex justify-between items-center px-6 py-4">
                    <div class="flex items-center gap-2">
                        <h1 class="text-3xl font-bold bg-gradient-to-r from-ctp-mauve to-ctp-blue bg-clip-text text-transparent">
                            "context"
                        </h1>
                        <span class="text-xs text-ctp-subtext0 font-mono">
                            {env!("CARGO_PKG_VERSION")}
                        </span>
                    </div>
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
                        <a href="/repos"
                            class="px-4 py-2 rounded-lg font-medium transition-colors"
                            class:bg-ctp-surface2=move || is_active("/repos")
                            class:text-ctp-text=move || is_active("/repos")
                            class:text-ctp-subtext1=move || !is_active("/repos")
                            class:hover:bg-ctp-surface1=move || !is_active("/repos")
                            class:hover:text-ctp-text=move || !is_active("/repos")>
                            "Repos"
                        </a>
                    </div>
                </div>
            </nav>

            <div class="flex-1">
                <Routes fallback=|| view! { <p>"Page not found"</p> }>
                    <Route path=path!("/") view=Projects/>
                    <Route path=path!("/projects/:id") view=ProjectDetail/>
                    <Route path=path!("/notes") view=Notes/>
                    <Route path=path!("/notes/:id") view=Notes/>
                    <Route path=path!("/repos") view=Repos/>
                </Routes>
            </div>

            <footer class="py-6 px-6 border-t border-ctp-surface1 bg-ctp-surface0">
                <div class="container mx-auto text-center text-sm text-ctp-subtext0">
                    <p>
                        "© " {
                            let date = web_sys::js_sys::Date::new_0();
                            date.get_full_year()
                        } " Christian Kemper. Licensed under "
                        <a href="https://opensource.org/licenses/MIT" target="_blank" rel="noopener noreferrer"
                            class="text-ctp-blue hover:text-ctp-lavender underline">
                            "MIT License"
                        </a>
                        "."
                    </p>
                    <p class="mt-1 text-xs text-ctp-overlay0">
                        "context v" {env!("CARGO_PKG_VERSION")}
                    </p>
                </div>
            </footer>
        </main>
    }
}
