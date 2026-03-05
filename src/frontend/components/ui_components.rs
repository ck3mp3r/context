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
                class="inline-flex items-center justify-center w-4 h-4 bg-ctp-surface0/50 border border-ctp-surface1/50 rounded hover:border-ctp-blue hover:bg-ctp-surface0 transition-colors cursor-pointer align-text-top"
                title="Copy to clipboard"
            >
                <span class="text-[10px] text-ctp-overlay0 leading-none">
                    {move || {
                        if copied.get() {
                            "✓"
                        } else {
                            "🔖"
                        }
                    }}
                </span>
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

/// Breadcrumb item data
#[derive(Clone)]
pub struct BreadcrumbItem {
    pub label: String,
    pub id: Option<String>,
    pub href: Option<String>,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            id: None,
            href: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }
}

/// Breadcrumb navigation component with consistent styling
/// Shows: Item [Copy ID] > Item [Copy ID] > ...
#[component]
pub fn Breadcrumb(items: Vec<BreadcrumbItem>) -> impl IntoView {
    let items_len = items.len();
    view! {
        <div class="bg-ctp-surface0 border-b border-ctp-surface1 py-3 px-6">
            <div class="container mx-auto flex items-center gap-3 text-base">
                {items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        let is_last = idx == items_len - 1;
                        let label = item.label.clone();
                        let id = item.id.clone();
                        let href = item.href.clone();

                        view! {
                            <div class="flex items-center gap-3">
                                {if let Some(link) = href {
                                    view! {
                                        <a
                                            href=link
                                            class="flex items-center gap-2 text-ctp-blue hover:text-ctp-sapphire transition-colors"
                                        >
                                            {if let Some(item_id) = id.clone() {
                                                view! { <CopyableId id=item_id/> }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            <span class="font-medium">{label.clone()}</span>
                                        </a>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="flex items-center gap-2">
                                            {if let Some(item_id) = id {
                                                view! { <CopyableId id=item_id/> }.into_any()
                                            } else {
                                                view! { <span></span> }.into_any()
                                            }}
                                            <span class="text-ctp-text font-medium">{label}</span>
                                        </div>
                                    }
                                        .into_any()
                                }}

                                {(!is_last).then(|| {
                                    view! { <span class="text-ctp-overlay0">"/"</span> }
                                })}
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
