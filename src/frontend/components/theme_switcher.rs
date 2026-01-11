use leptos::prelude::*;

use crate::theme::{CatppuccinTheme, apply_theme};

#[component]
pub fn ThemeSwitcher(
    /// Current theme signal
    theme: RwSignal<CatppuccinTheme>,
) -> impl IntoView {
    let show_menu = RwSignal::new(false);

    // Get theme icon
    let theme_icon = move || match theme.get() {
        CatppuccinTheme::Latte => "☀️",
        _ => "🌙",
    };

    // Get theme label
    let theme_label = move || match theme.get() {
        CatppuccinTheme::Latte => "Light",
        CatppuccinTheme::Frappe => "Frappé",
        CatppuccinTheme::Macchiato => "Macchiato",
        CatppuccinTheme::Mocha => "Mocha",
    };

    // Handle theme selection
    let select_theme = move |selected: CatppuccinTheme| {
        theme.set(selected);
        apply_theme(selected);
        show_menu.set(false);
    };

    // Close menu when clicking outside
    let close_menu = move |_| show_menu.set(false);

    view! {
        <div class="relative">
            <button
                class="px-3 py-1.5 rounded-lg text-sm font-medium bg-ctp-surface0 text-ctp-text border border-ctp-surface1 hover:bg-ctp-surface1 transition-colors flex items-center gap-2"
                on:click=move |_| show_menu.update(|v| *v = !*v)
            >
                <span class="text-base">{theme_icon}</span>
                <span>{theme_label}</span>
                <svg
                    class="w-4 h-4 text-ctp-subtext0 transition-transform"
                    class:rotate-180=move || show_menu.get()
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                >
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                </svg>
            </button>

            <Show when=move || show_menu.get()>
                <div
                    class="fixed inset-0 z-40"
                    on:click=close_menu
                />
                <div class="absolute right-0 mt-2 w-44 rounded-lg shadow-xl bg-ctp-surface0 border border-ctp-surface1 overflow-hidden z-50">
                    {[
                        (CatppuccinTheme::Latte, "☀️", "Latte", "Light theme"),
                        (CatppuccinTheme::Frappe, "🌙", "Frappé", "Dark pastel"),
                        (CatppuccinTheme::Macchiato, "🌙", "Macchiato", "Dark warm"),
                        (CatppuccinTheme::Mocha, "🌙", "Mocha", "Dark cool"),
                    ]
                        .iter()
                        .map(|(t, icon, name, desc)| {
                            let is_active = move || theme.get() == *t;
                            let theme_val = *t;
                            view! {
                                <button
                                    class="w-full px-3 py-2.5 text-left text-sm transition-colors flex items-center gap-3 group"
                                    class:bg-ctp-surface1=is_active
                                    class:text-ctp-text=move || is_active()
                                    class:text-ctp-subtext1=move || !is_active()
                                    class:hover:bg-ctp-surface1=move || !is_active()
                                    class:hover:text-ctp-text=move || !is_active()
                                    on:click=move |_| select_theme(theme_val)
                                >
                                    <span class="text-base">{*icon}</span>
                                    <div class="flex-1 min-w-0">
                                        <div class="font-medium">{*name}</div>
                                        <div class="text-xs text-ctp-overlay0 group-hover:text-ctp-subtext0">{*desc}</div>
                                    </div>
                                    <Show when=is_active>
                                        <svg
                                            class="w-4 h-4 text-ctp-blue flex-shrink-0"
                                            fill="none"
                                            stroke="currentColor"
                                            viewBox="0 0 24 24"
                                        >
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                                        </svg>
                                    </Show>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </Show>
        </div>
    }
}
