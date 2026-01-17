use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, QueryBuilder};
use crate::components::{Pagination, RepoCard, SearchInput, SortControls};
use crate::hooks::{use_pagination, use_search, use_sort};
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

    // Hooks for search, sort, and pagination
    let pagination = use_pagination();
    let search = use_search(Callback::new(move |_| {
        pagination.set_page.set(0); // Reset to first page on new search
    }));
    let sort = use_sort(
        "created_at",
        "desc",
        Callback::new(move |_| {
            pagination.set_page.set(0); // Reset to first page on sort change
        }),
    );

    let (repos_data, set_repos_data) = signal(None::<Result<Paginated<Repo>, ApiClientError>>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();
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
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Fetch when dependencies change
    Effect::new(move || {
        let current_page = pagination.page.get();
        let current_query = search.search_query.get();
        let current_sort = sort.sort_field.get();
        let current_order = sort.sort_order.get();
        let _ = refetch_trigger.get();

        set_repos_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;

            let mut builder = QueryBuilder::<Repo>::new()
                .limit(PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order);

            if !current_query.trim().is_empty() {
                builder = builder.search(current_query);
            }

            let result = builder.fetch().await;
            set_repos_data.set(Some(result));
        });
    });

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Repositories"</h2>
            </div>

            // Search bar and sort controls
            <div class="mb-6 flex gap-4 items-center">
                <div class="flex-1">
                    <SearchInput
                        value=search.search_input
                        on_change=search.on_debounced_change
                        on_immediate_change=search.on_immediate_change
                        placeholder="Search repositories by remote URL or tags..."
                    />
                </div>
                <SortControls
                    sort_field=sort.sort_field
                    sort_order=sort.sort_order
                    on_sort_change=sort.on_sort_change
                    on_order_change=sort.on_order_change
                    fields=vec![
                        ("remote".to_string(), "Remote".to_string()),
                        ("path".to_string(), "Path".to_string()),
                        ("created_at".to_string(), "Created".to_string()),
                    ]
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
                                            {if search.search_query.get().trim().is_empty() {
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
                                                current_page=pagination.page
                                                total_pages=total_pages
                                                on_prev=pagination.on_prev
                                                on_next=pagination.on_next
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
