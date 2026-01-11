use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, projects};
use crate::components::{CopyableId, ExternalRefLink, Pagination, SearchInput};
use crate::models::{Paginated, Project, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn Projects() -> impl IntoView {
    const PAGE_SIZE: usize = 12;

    // State management
    let (page, set_page) = signal(0usize);
    let (search_input, set_search_input) = signal(String::new()); // Raw input
    let (search_query, set_search_query) = signal(String::new()); // Debounced search
    let (projects_data, set_projects_data) =
        signal(None::<Result<Paginated<Project>, ApiClientError>>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates
    Effect::new(move || {
        if let Some(
            UpdateMessage::ProjectCreated { .. }
            | UpdateMessage::ProjectUpdated { .. }
            | UpdateMessage::ProjectDeleted { .. },
        ) = ws_updates.get()
        {
            web_sys::console::log_1(
                &"Project updated via WebSocket, refetching projects list...".into(),
            );
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

    // Fetch projects when dependencies change
    Effect::new(move || {
        let current_page = page.get();
        let current_query = search_query.get();
        let _ = refetch_trigger.get(); // Track refetch trigger

        // Reset to loading state immediately
        set_projects_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;
            let query_opt = if current_query.trim().is_empty() {
                None
            } else {
                Some(current_query)
            };

            let result = projects::list(Some(PAGE_SIZE), Some(offset), query_opt).await;
            set_projects_data.set(Some(result));
        });
    });

    // Pagination handlers
    let go_to_page = move |new_page: usize| {
        set_page.set(new_page);
    };

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Projects"</h2>
            </div>

            // Search bar
            <div class="mb-6">
                <SearchInput
                    value=search_input
                    on_change=on_debounced_change
                    on_immediate_change=on_immediate_change
                    placeholder="Search projects..."
                />
            </div>

            {move || match projects_data.get() {
                None => {
                    view! {
                        <div class="text-center py-12">
                            <p class="text-ctp-subtext0">"Loading projects..."</p>
                        </div>
                    }
                        .into_any()
                }
                Some(Ok(paginated)) => {
                    let total_pages = paginated.total.div_ceil(PAGE_SIZE);

                    if paginated.items.is_empty() {
                        view! {
                            <p class="text-ctp-subtext0">
                                {if search_query.get().trim().is_empty() {
                                    "No projects found. Create one to get started!"
                                } else {
                                    "No projects found matching your search."
                                }}
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div>
                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 auto-rows-fr mb-6">
                                {paginated
                                    .items
                                    .iter()
                                    .map(|project| {
                                        let project_id = project.id.clone();
                                        let project_title = project.title.clone();
                                        let project_description = project.description.clone();
                                        let project_tags = project.tags.clone();
                                        let project_external_refs = project.external_refs.clone();
                                        view! {
                                            <div class="bg-ctp-surface0 rounded-lg p-6 border border-ctp-surface1 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[280px]">
                                                <a
                                                    href=format!("/projects/{}", project_id)
                                                    class="flex flex-col h-full"
                                                >
                                                    <div class="flex items-start gap-2 mb-2">
                                                        <div class="flex-shrink-0">
                                                            <CopyableId id=project_id.clone()/>
                                                        </div>
                                                        <h3 class="flex-1 min-w-0 break-words text-xl font-semibold text-ctp-text">
                                                            {project_title}
                                                        </h3>
                                                    </div>
                                                {project_description
                                                    .as_ref()
                                                    .map(|desc| {
                                                        view! {
                                                            <p class="text-sm text-ctp-subtext0 mb-4">
                                                                {desc.clone()}
                                                            </p>
                                                        }
                                                    })}

                                                <div class="flex-grow"></div>

                                                {(!project_tags.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <div class="flex flex-wrap gap-2 mt-auto">
                                                                {project_tags
                                                                    .iter()
                                                                    .map(|tag| {
                                                                        view! {
                                                                            <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-2 py-1 rounded">
                                                                                {tag.clone()}
                                                                            </span>
                                                                        }
                                                                    })
                                                                    .collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}

                                                {(!project_external_refs.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <div class="mt-2 flex flex-wrap gap-1">
                                                                {project_external_refs
                                                                    .iter()
                                                                    .map(|ext_ref| {
                                                                        view! {
                                                                            <ExternalRefLink external_ref=ext_ref.clone()/>
                                                                        }
                                                                    })
                                                                    .collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}
                                                </a>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </div>

                            // Pagination
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
                                item_name="projects".to_string()
                            />
                            </div>
                        }
                            .into_any()
                    }
                }
                Some(Err(err)) => {
                    view! {
                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                            <p class="text-ctp-red font-semibold">"Error loading projects"</p>
                            <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                        </div>
                    }
                        .into_any()
                }
            }}

        </div>
    }
}
