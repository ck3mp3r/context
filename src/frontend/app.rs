use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use crate::pages::{Home, Notes, Projects, Tasks};

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
                            <a href="/projects" class="text-ctp-blue hover:text-ctp-lavender">"Projects"</a>
                            <a href="/tasks" class="text-ctp-blue hover:text-ctp-lavender">"Tasks"</a>
                            <a href="/notes" class="text-ctp-blue hover:text-ctp-lavender">"Notes"</a>
                        </div>
                    </div>
                </nav>

                <Routes fallback=|| view! { <p>"Page not found"</p> }>
                    <Route path=path!("/") view=Home/>
                    <Route path=path!("/projects") view=Projects/>
                    <Route path=path!("/tasks") view=Tasks/>
                    <Route path=path!("/notes") view=Notes/>
                </Routes>
            </main>
        </Router>
    }
}
