use leptos::prelude::*;
use thaw::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(
    inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }"
)]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

/// Copyable ID component - icon-only with tooltip showing "ID: <id>" and title "Copy to clipboard"
#[component]
pub fn CopyableId(id: String) -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let id_clone = id.clone();
    let tooltip_text = format!("ID: {}", id);

    let do_copy = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        copy_to_clipboard(&id_clone);
        set_copied.set(true);

        // Reset after 2 seconds
        set_timeout(
            move || {
                set_copied.set(false);
            },
            std::time::Duration::from_secs(2),
        );
    };

    view! {
        <Tooltip content=tooltip_text>
            <button
                on:click=do_copy
                class="inline-flex items-center justify-center w-4 h-4 text-ctp-overlay0 hover:text-ctp-text transition-colors cursor-pointer align-text-top"
                title="Copy to clipboard"
            >
                {move || {
                    if copied.get() {
                        view! {
                            <svg xmlns="http://www.w3.org/2000/svg" class="w-2.5 h-2.5 text-ctp-green" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <polyline points="20 6 9 17 4 12"></polyline>
                            </svg>
                        }.into_any()
                    } else {
                        view! {
                            <svg xmlns="http://www.w3.org/2000/svg" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                            </svg>
                        }.into_any()
                    }
                }}
            </button>
        </Tooltip>
    }
}

#[component]
pub fn Pagination(
    current_page: ReadSignal<usize>,
    total_pages: usize,
    on_prev: Callback<()>,
    on_next: Callback<()>,
    #[prop(optional)] show_summary: Option<bool>,
    #[prop(optional)] total_items: Option<usize>,
    #[prop(optional)] page_size: Option<usize>,
    #[prop(optional)] item_name: Option<String>,
) -> impl IntoView {
    let show_summary = show_summary.unwrap_or(false);
    let item_name = item_name.unwrap_or_else(|| "items".to_string());

    view! {
        <div>
            {show_summary
                .then(|| {
                    if let (Some(total), Some(size)) = (total_items, page_size) {
                        let offset = move || current_page.get() * size;
                        let end = move || (offset() + size).min(total);
                        Some(
                            view! {
                                <div class="text-sm text-ctp-overlay0 mb-4">
                                    "Showing " {move || offset() + 1} " - " {end} " of " {total} " "
                                    {item_name.clone()}
                                </div>
                            },
                        )
                    } else {
                        None
                    }
                })}

            {(total_pages > 1)
                .then(|| {
                    view! {
                        <div class="flex justify-center items-center gap-2">
                            <button
                                on:click=move |_| {
                                    if current_page.get() > 0 {
                                        on_prev.run(());
                                    }
                                }

                                disabled=move || current_page.get() == 0
                                class="px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded text-ctp-text disabled:opacity-50 disabled:cursor-not-allowed hover:border-ctp-blue"
                            >
                                "← Previous"
                            </button>

                            <span class="text-ctp-subtext0">
                                "Page " {move || current_page.get() + 1} " of " {total_pages}
                            </span>

                            <button
                                on:click=move |_| {
                                    if current_page.get() < total_pages - 1 {
                                        on_next.run(());
                                    }
                                }
                                disabled=move || {
                                    current_page.get() >= total_pages - 1
                                }
                                class="px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded text-ctp-text disabled:opacity-50 disabled:cursor-not-allowed hover:border-ctp-blue"
                            >
                                "Next →"
                            </button>
                        </div>
                    }
                })}
        </div>
    }
}

