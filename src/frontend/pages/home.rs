use leptos::prelude::*;

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <div class="container mx-auto p-6">
            <h2 class="text-3xl font-bold text-ctp-text mb-4">"Welcome to c5t"</h2>
            <p class="text-ctp-subtext0">"Your task and note management system"</p>
        </div>
    }
}
