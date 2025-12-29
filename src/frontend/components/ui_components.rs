use leptos::prelude::*;

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
