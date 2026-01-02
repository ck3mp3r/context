use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
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
    // WebSocket connection status (from context)
    let ws_state = use_websocket_connection();

    view! {
            <Router>
                <main class="min-h-screen bg-ctp-base">
                <nav class="bg-ctp-surface0 border-b border-ctp-surface1 px-6 py-4">
                    <div class="container mx-auto flex justify-between items-center">
                        <div class="flex flex-col">
                            <h1 class="text-2xl font-bold text-ctp-text">"c5t"</h1>
                            // WebSocket connection status indicator
                            <span class="text-xs text-ctp-subtext0">
                                {move || {
                                    match ws_state.get() {
                                        ConnectionReadyState::Open => "🟢 Connected",
                                        ConnectionReadyState::Connecting => "🟡 Connecting...",
                                        ConnectionReadyState::Closing => "🟡 Closing...",
                                        ConnectionReadyState::Closed => "🔴 Disconnected",
                                    }
                                }}
                            </span>
                        </div>
                        <div class="flex gap-4 items-center">
                            <a href="/" class="text-ctp-blue hover:text-ctp-lavender">"Projects"</a>
                            <a href="/notes" class="text-ctp-blue hover:text-ctp-lavender">"Notes"</a>
                            <a href="/repos" class="text-ctp-blue hover:text-ctp-lavender">"Repos"</a>
                        </div>
                    </div>
                </nav>

                <Routes fallback=|| view! { <p>"Page not found"</p> }>
                    <Route path=path!("/") view=Projects/>
                    <Route path=path!("/projects/:id") view=ProjectDetail/>
                    <Route path=path!("/notes") view=Notes/>
                    <Route path=path!("/notes/:id") view=Notes/>
                    <Route path=path!("/repos") view=Repos/>
                </Routes>
            </main>
        </Router>
    }
}
