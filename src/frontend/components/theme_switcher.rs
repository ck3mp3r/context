use leptos::prelude::*;

use crate::theme::{CatppuccinTheme, apply_theme};

#[component]
pub fn ThemeSwitcher(
    /// Current theme signal
    theme: RwSignal<CatppuccinTheme>,
) -> impl IntoView {
    // Toggle between light (Latte) and dark (Mocha)
    let toggle_theme = move |_| {
        let new_theme = if theme.get().is_light() {
            CatppuccinTheme::Mocha
        } else {
            CatppuccinTheme::Latte
        };
        theme.set(new_theme);
        apply_theme(new_theme);
    };

    let is_light = move || theme.get().is_light();

    view! {
        <button
            class="relative inline-flex h-8 w-14 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-ctp-blue focus:ring-offset-2 focus:ring-offset-ctp-base"
            class:bg-ctp-surface1=move || !is_light()
            class:bg-ctp-yellow=is_light
            on:click=toggle_theme
            title=move || if is_light() { "Switch to dark mode" } else { "Switch to light mode" }
        >
            <span
                class="inline-flex h-6 w-6 transform items-center justify-center rounded-full transition-transform duration-200 ease-in-out"
                class:translate-x-1=move || !is_light()
                class:translate-x-7=is_light
                class:bg-ctp-surface2=move || !is_light()
                class:bg-ctp-base=is_light
            >
                <span class="text-sm">
                    {move || if is_light() { "☀️" } else { "🌙" }}
                </span>
            </span>
        </button>
    }
}
