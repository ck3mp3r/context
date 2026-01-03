use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::ApiClientError;
use crate::api::repos;
use crate::components::{Pagination, RepoCard};
use crate::models::{Paginated, Repo, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn Repos() -> impl IntoView {
    view! {
        <ReposList/>
    }
}

#[component]
fn ReposList() -> impl IntoView {
    const PAGE_SIZE: usize = 12;

    // State management
    let (page, set_page) = signal(0usize);
    let (search_input, set_search_input) = signal(String::new()); // Raw input
    let (search_query, set_search_query) = signal(String::new()); // Debounced search
    let (repos_data, set_repos_data) = signal(None::<Result<Paginated<Repo>, ApiClientError>>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Trigger to force refetch (increments when we need to refresh)
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates and trigger refetch when repos change
    Effect::new(move || {
        if let Some(
            UpdateMessage::RepoCreated { .. }
            | UpdateMessage::RepoUpdated { .. }
            | UpdateMessage::RepoDeleted { .. },
        ) = ws_updates.get()
        {
            web_sys::console::log_1(&"Repo updated via WebSocket, refetching...".into());
            // Trigger refetch by incrementing counter
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Store the timeout ID so we can cancel it
    let debounce_timeout = RwSignal::new(None::<i32>);

    // Handle search input change with proper debouncing
    let on_search = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_search_input.set(value.clone());

        use wasm_bindgen::JsCast;
        use wasm_bindgen::prelude::*;

        // Cancel the previous timeout if it exists
        if let Some(timeout_id) = debounce_timeout.get() {
            web_sys::window()
                .unwrap()
                .clear_timeout_with_handle(timeout_id);
        }

        // Set new timeout
        let callback = Closure::once(move || {
            set_search_query.set(value.clone());
            set_page.set(0); // Reset to first page on new search
            debounce_timeout.set(None); // Clear timeout ID after it fires
        });

        let timeout_id = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                500,
            )
            .unwrap();

        debounce_timeout.set(Some(timeout_id));
        callback.forget();
    };

    // Use Effect to fetch when dependencies change (including WebSocket updates)
    Effect::new(move || {
        let current_page = page.get();
        let current_query = search_query.get();
        let _ = refetch_trigger.get(); // Track refetch trigger

        // Reset to loading state immediately
        set_repos_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;
            let search_opt = if current_query.trim().is_empty() {
                None
            } else {
                Some(current_query)
            };

            let result = repos::list(Some(PAGE_SIZE), Some(offset), search_opt, None).await;
            set_repos_data.set(Some(result));
        });
    });

    // Pagination handlers
    let go_to_page = move |new_page: usize| {
        set_page.set(new_page);
    };

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Repositories"</h2>
            </div>

            // Search bar
            <div class="mb-6">
                <input
                    type="text"
                    placeholder="Search repositories by remote URL or tags..."
                    class="w-full px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text placeholder-ctp-overlay0 focus:outline-none focus:border-ctp-blue"
                    on:input=on_search
                    prop:value=move || search_input.get()
                />
            </div>

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading repositories..."</p> }
            }>
                {move || {
                    repos_data
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(paginated) => {
                                let total_pages = paginated.total.div_ceil(PAGE_SIZE);

                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            {if search_query.get().trim().is_empty() {
                                                "No repositories found. Add one to get started!"
                                            } else {
                                                "No repositories found matching your search."
                                            }}
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div>
                                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                {paginated
                                                    .items
                                                    .iter()
                                                    .map(|repo| {
                                                        view! { <RepoCard repo=repo.clone()/> }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>

                                            <Pagination
                                                current_page=page
                                                total_pages=total_pages
                                                on_prev=Callback::new(move |_| {
                                                    let current = page.get();
                                                    if current > 0 {
                                                        go_to_page(current - 1);
                                                    }
                                                })
                                                on_next=Callback::new(move |_| {
                                                    let current = page.get();
                                                    if current < total_pages - 1 {
                                                        go_to_page(current + 1);
                                                    }
                                                })
                                                show_summary=true
                                                total_items=paginated.total
                                                page_size=PAGE_SIZE
                                                item_name="repositories".to_string()
                                            />
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(err) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading repositories"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        })
                }}

            </Suspense>
        </div>
    }
}
