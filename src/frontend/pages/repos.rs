use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, QueryBuilder};
use crate::components::{Pagination, RepoCard, SearchInput};
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

    // Callbacks for SearchInput component
    let on_immediate_change = Callback::new(move |value: String| {
        set_search_input.set(value);
    });

    let on_debounced_change = Callback::new(move |value: String| {
        set_search_query.set(value);
        set_page.set(0); // Reset to first page on new search
    });

    // Use Effect to fetch when dependencies change (including WebSocket updates)
    Effect::new(move || {
        let current_page = page.get();
        let current_query = search_query.get();
        let _ = refetch_trigger.get(); // Track refetch trigger

        // Reset to loading state immediately
        set_repos_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;

            let mut builder = QueryBuilder::<Repo>::new().limit(PAGE_SIZE).offset(offset);

            if !current_query.trim().is_empty() {
                builder = builder.search(current_query);
            }

            let result = builder.fetch().await;
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
                <SearchInput
                    value=search_input
                    on_change=on_debounced_change
                    on_immediate_change=on_immediate_change
                    placeholder="Search repositories by remote URL or tags..."
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
