use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_location, use_params_map};

use crate::api::{ApiClientError, QueryBuilder, projects};
use crate::breadcrumb_state::BreadcrumbPageState;
use crate::components::{Breadcrumb, BreadcrumbItem, NoteCard, Pagination, SearchInput, SortControls};
use crate::hooks::{use_pagination, use_search, use_sort};
use crate::models::{Note, Paginated, Project, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn ProjectNotes() -> impl IntoView {
    const PAGE_SIZE: usize = 12;

    let params = use_params_map();
    let location = use_location();
    let project_id = move || params.read().get("id").unwrap_or_default();

    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);
    
    // Hooks for search, sort, and pagination
    let pagination = use_pagination();
    let search = use_search();
    let sort = use_sort("last_activity_at", "desc");

    let (notes_data, set_notes_data) = signal(None::<Result<Paginated<Note>, ApiClientError>>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Fetch project details
    Effect::new(move || {
        let id = project_id();
        if !id.is_empty() {
            spawn_local(async move {
                let result = projects::get(&id).await;
                set_project_data.set(Some(result));
            });
        }
    });

    // Watch for WebSocket updates and trigger refetch when notes change
    Effect::new(move || {
        if let Some(
            UpdateMessage::NoteCreated { .. }
            | UpdateMessage::NoteUpdated { .. }
            | UpdateMessage::NoteDeleted { .. },
        ) = ws_updates.get()
        {
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Fetch notes for this project
    Effect::new(move || {
        let id = project_id();
        let current_page = pagination.page.get();
        let current_query = search.search_query.get();
        let current_sort = sort.sort_field.get();
        let current_order = sort.sort_order.get();
        let _ = refetch_trigger.get();

        if id.is_empty() {
            return;
        }

        set_notes_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;

            let mut builder = QueryBuilder::<Note>::new()
                .limit(PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order)
                .param("type", "note")
                .param("project_id", &id);

            if !current_query.trim().is_empty() {
                builder = builder.search(current_query);
            }

            let result = builder.fetch().await;
            set_notes_data.set(Some(result));
        });
    });

    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]">
            // Breadcrumb navigation
            {move || {
                project_data.get().and_then(|result| {
                    result.ok().map(|project| {
                        let items = vec![
                            BreadcrumbItem::new("Projects")
                                .with_href("/")
                                .with_name("projects"),
                            BreadcrumbItem::new(project.title.clone())
                                .with_href(format!("/projects/{}", project.id))
                                .with_id(project.id.clone()),
                            BreadcrumbItem::new("Notes"),
                        ];
                        view! { <Breadcrumb items=items/> }
                    })
                })
            }}

            <div class="container mx-auto p-6 flex-1">
                {move || match project_data.get() {
                    None => {
                        view! {
                            <div class="text-center py-12">
                                <p class="text-ctp-subtext0">"Loading project..."</p>
                            </div>
                        }
                            .into_any()
                    }
                    Some(Err(err)) => {
                        view! {
                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                <p class="text-ctp-red">{err.to_string()}</p>
                            </div>
                        }
                            .into_any()
                    }
                    Some(Ok(project)) => {
                        view! {
                            <div>
                                <h1 class="text-3xl font-bold text-ctp-text mb-6">
                                    {project.title.clone()} " - Notes"
                                </h1>

                                // Search input and sort controls
                                <div class="mb-4 flex gap-4 items-center">
                                    <div class="flex-1">
                                        <SearchInput
                                            value=search.search_input
                                            on_change=search.on_debounced_change
                                            on_immediate_change=search.on_immediate_change
                                            placeholder="Search notes by title, content, or tags..."
                                        />
                                    </div>
                                    <SortControls
                                        sort_field=sort.sort_field
                                        sort_order=sort.sort_order
                                        on_sort_change=sort.on_sort_change
                                        on_order_change=sort.on_order_change
                                        fields=vec![
                                            ("last_activity_at".to_string(), "Last Activity".to_string()),
                                            ("title".to_string(), "Title".to_string()),
                                            ("created_at".to_string(), "Created".to_string()),
                                            ("updated_at".to_string(), "Updated".to_string()),
                                        ]
                                    />
                                </div>

                                {move || match notes_data.get() {
                                    None => {
                                        view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }.into_any()
                                    }
                                    Some(Ok(paginated)) => {
                                        let total_pages = paginated.total.div_ceil(PAGE_SIZE);
                                        if paginated.items.is_empty() {
                                            view! {
                                                <p class="text-ctp-subtext0">
                                                    {if search.search_query.get().trim().is_empty() {
                                                        "No notes found for this project."
                                                    } else {
                                                        "No notes found matching your search."
                                                    }}
                                                </p>
                                            }
                                                .into_any()
                                        } else {
                                            let proj_id = project_id();
                                            view! {
                                                <div>
                                                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                        {paginated
                                                            .items
                                                            .iter()
                                                            .map(|note| {
                                                                let proj_id_clone = proj_id.clone();
                                                                let query_str = location.search.get();
                                                                view! {
                                                                    <NoteCard
                                                                        note=note.clone()
                                                                        project_id=proj_id_clone.clone()
                                                                        current_query=query_str
                                                                        breadcrumb_name=proj_id_clone
                                                                    />
                                                                }
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
                                                        item_name="notes".to_string()
                                                    />
                                                </div>
                                            }
                                                .into_any()
                                        }
                                    }
                                    Some(Err(err)) => {
                                        view! {
                                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                <p class="text-ctp-red">{err.to_string()}</p>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                }}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}
