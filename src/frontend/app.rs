use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use crate::pages::{Home, Notes, ProjectDetail, Repos};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <main class="min-h-screen bg-ctp-base">
                <nav class="bg-ctp-surface0 border-b border-ctp-surface1 px-6 py-4">
                    <div class="container mx-auto flex justify-between items-center">
                        <h1 class="text-2xl font-bold text-ctp-text">"c5t"</h1>
                        <div class="flex gap-4">
                            <a href="/" class="text-ctp-blue hover:text-ctp-lavender">"Home"</a>
                            <a href="/notes" class="text-ctp-blue hover:text-ctp-lavender">"Notes"</a>
                            <a href="/repos" class="text-ctp-blue hover:text-ctp-lavender">"Repos"</a>
                        </div>
                    </div>
                </nav>

                <Routes fallback=|| view! { <p>"Page not found"</p> }>
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/projects/:id") view=ProjectDetail/>
                    <Route path=path!("/notes") view=Notes/>
                    <Route path=path!("/notes/:id") view=Notes/>
                    <Route path=path!("/repos") view=Repos/>
                </Routes>
            </main>
        </Router>
    }
}
