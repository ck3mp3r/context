use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, QueryBuilder, projects};
use crate::components::{
    CopyableId, ExternalRefLink, NoteCard, NoteDetailModal, Pagination, RepoCard, SearchInput,
    SortControls, TaskListCard, TaskListDetailModal,
};
use crate::hooks::{use_pagination, use_search, use_sort};
use crate::models::{Note, Paginated, Project, Repo, TaskList, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let params = use_params_map();
    let project_id = move || params.read().get("id").unwrap_or_default();

    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);
    let (active_tab, set_active_tab) = signal("task-lists");
    let (task_lists_data, set_task_lists_data) =
        signal(None::<Result<Paginated<TaskList>, ApiClientError>>);
    let (notes_data, set_notes_data) = signal(None::<Result<Paginated<Note>, ApiClientError>>);
    let (repos_data, set_repos_data) = signal(None::<Result<Paginated<Repo>, ApiClientError>>);

    const TASK_LIST_PAGE_SIZE: usize = 12;
    const NOTE_PAGE_SIZE: usize = 12;
    const REPO_PAGE_SIZE: usize = 12;

    // Hooks for task lists tab
    let task_list_pagination = use_pagination();
    let task_list_search = use_search(Callback::new(move |_| {
        task_list_pagination.set_page.set(0);
    }));
    let task_list_sort = use_sort(
        "updated_at",
        "desc",
        Callback::new(move |_| {
            task_list_pagination.set_page.set(0);
        }),
    );

    // Hooks for notes tab
    let note_pagination = use_pagination();
    let note_search = use_search(Callback::new(move |_| {
        note_pagination.set_page.set(0);
    }));
    let note_sort = use_sort(
        "last_activity_at",
        "desc",
        Callback::new(move |_| {
            note_pagination.set_page.set(0);
        }),
    );

    // Hooks for repos tab
    let repo_pagination = use_pagination();
    let repo_search = use_search(Callback::new(move |_| {
        repo_pagination.set_page.set(0);
    }));
    let repo_sort = use_sort(
        "created_at",
        "desc",
        Callback::new(move |_| {
            repo_pagination.set_page.set(0);
        }),
    );

    // Show archived toggle
    let show_archived_task_lists = RwSignal::new(false);

    // Note detail modal state
    let note_modal_open = RwSignal::new(false);
    let selected_note_id = RwSignal::new(String::new());
    let selected_note_has_subnotes = RwSignal::new(false);

    // Task list detail modal state
    let task_list_modal_open = RwSignal::new(false);
    let selected_task_list = RwSignal::new(None::<TaskList>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Triggers to force refetch
    let (note_refetch_trigger, set_note_refetch_trigger) = signal(0u32);
    let (task_list_refetch_trigger, set_task_list_refetch_trigger) = signal(0u32);
    let (repo_refetch_trigger, set_repo_refetch_trigger) = signal(0u32);
    let (project_refetch_trigger, set_project_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates and trigger refetch when anything changes
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            match update {
                UpdateMessage::NoteCreated { .. }
                | UpdateMessage::NoteUpdated { .. }
                | UpdateMessage::NoteDeleted { .. } => {
                    web_sys::console::log_1(
                        &"Note updated via WebSocket, refetching project notes...".into(),
                    );
                    set_note_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                UpdateMessage::TaskListCreated { .. }
                | UpdateMessage::TaskListUpdated { .. }
                | UpdateMessage::TaskListDeleted { .. } => {
                    web_sys::console::log_1(
                        &"TaskList updated via WebSocket, refetching project task lists...".into(),
                    );
                    set_task_list_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                UpdateMessage::RepoCreated { .. }
                | UpdateMessage::RepoUpdated { .. }
                | UpdateMessage::RepoDeleted { .. } => {
                    web_sys::console::log_1(
                        &"Repo updated via WebSocket, refetching project repos...".into(),
                    );
                    set_repo_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                UpdateMessage::ProjectUpdated { .. } => {
                    web_sys::console::log_1(
                        &"Project updated via WebSocket, refetching project...".into(),
                    );
                    set_project_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                _ => {} // Ignore other updates (ProjectCreated, ProjectDeleted not relevant for detail page)
            }
        }
    });

    // Fetch project details
    Effect::new(move || {
        let id = project_id();
        let _ = project_refetch_trigger.get(); // Track refetch trigger from WebSocket updates
        if !id.is_empty() {
            spawn_local(async move {
                let result = projects::get(&id).await;
                set_project_data.set(Some(result));
            });
        }
    });

    // Reset pagination when archived toggle changes
    Effect::new(move || {
        show_archived_task_lists.get();
        task_list_pagination.set_page.set(0);
    });

    // Fetch task lists for this project (with archived toggle, search, and pagination)
    Effect::new(move || {
        let id = project_id();
        let show_archived = show_archived_task_lists.get();
        let search_query = task_list_search.search_query.get();
        let current_page = task_list_pagination.page.get();
        let current_sort = task_list_sort.sort_field.get();
        let current_order = task_list_sort.sort_order.get();
        let _ = task_list_refetch_trigger.get();
        if !id.is_empty() {
            spawn_local(async move {
                let status = if show_archived { None } else { Some("active") };
                let offset = current_page * TASK_LIST_PAGE_SIZE;
                let search_opt = if search_query.trim().is_empty() {
                    None
                } else {
                    Some(search_query)
                };
                let mut builder = QueryBuilder::<TaskList>::new()
                    .limit(TASK_LIST_PAGE_SIZE)
                    .offset(offset)
                    .sort(current_sort)
                    .order(current_order)
                    .param("project_id", id);

                if let Some(stat) = status {
                    builder = builder.param("status", stat);
                }

                if let Some(search) = search_opt {
                    builder = builder.search(search);
                }

                let result = builder.fetch().await;
                set_task_lists_data.set(Some(result));
            });
        }
    });

    // Fetch notes for this project (with FTS5 search support and pagination)
    Effect::new(move || {
        let id = project_id();
        let search = note_search.search_query.get();
        let current_page = note_pagination.page.get();
        let current_sort = note_sort.sort_field.get();
        let current_order = note_sort.sort_order.get();
        let _ = note_refetch_trigger.get();
        if !id.is_empty() {
            spawn_local(async move {
                let search_query = if search.trim().is_empty() {
                    None
                } else {
                    Some(search)
                };
                let offset = current_page * NOTE_PAGE_SIZE;

                let mut builder = QueryBuilder::<Note>::new()
                    .limit(NOTE_PAGE_SIZE)
                    .offset(offset)
                    .sort(current_sort)
                    .order(current_order)
                    .param("project_id", id)
                    .param("type", "note");

                if let Some(search) = search_query {
                    builder = builder.search(search);
                }

                let result = builder.fetch().await;
                set_notes_data.set(Some(result));
            });
        }
    });

    // Fetch repos for this project (with pagination)
    Effect::new(move || {
        let id = project_id();
        let current_page = repo_pagination.page.get();
        let current_sort = repo_sort.sort_field.get();
        let current_order = repo_sort.sort_order.get();
        let current_search = repo_search.search_query.get();
        let _ = repo_refetch_trigger.get();
        if !id.is_empty() {
            spawn_local(async move {
                let offset = current_page * REPO_PAGE_SIZE;

                let mut builder = QueryBuilder::<Repo>::new()
                    .limit(REPO_PAGE_SIZE)
                    .offset(offset)
                    .sort(current_sort)
                    .order(current_order)
                    .param("project_id", id);

                if !current_search.trim().is_empty() {
                    builder = builder.search(current_search);
                }

                let result = builder.fetch().await;
                set_repos_data.set(Some(result));
            });
        }
    });

    view! {
        <div class="container mx-auto p-6">
            {move || match project_data.get() {
                None => {
                    view! {
                        <div class="text-center py-12">
                            <p class="text-ctp-subtext0">"Loading project..."</p>
                        </div>
                    }
                        .into_any()
                }
                Some(Ok(project)) => {
                    view! {
                        <div>
                            // Project Header
                            <div class="mb-8">
                                <div class="flex items-center justify-between mb-4">
                                    <div class="flex items-center gap-2 flex-1 min-w-0">
                                        <div class="flex-shrink-0">
                                            <CopyableId id=project.id.clone() />
                                        </div>
                                        <h2 class="flex-1 min-w-0 break-words text-3xl font-bold text-ctp-text">{project.title.clone()}</h2>
                                    </div>
                                    <a
                                        href="/"
                                        class="text-ctp-blue hover:text-ctp-lavender text-sm whitespace-nowrap flex-shrink-0"
                                    >
                                        "← Back to Projects"
                                    </a>
                                </div>

                                {project
                                    .description
                                    .as_ref()
                                    .map(|desc| {
                                        view! { <p class="text-ctp-subtext0 mb-4">{desc.clone()}</p> }
                                    })}

                                {(!project.tags.is_empty())
                                    .then(|| {
                                        view! {
                                            <div class="flex flex-wrap gap-2 mb-3">
                                                {project
                                                    .tags
                                                    .iter()
                                                    .map(|tag| {
                                                        view! {
                                                            <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-3 py-1 rounded">
                                                                {tag.clone()}
                                                            </span>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>
                                        }
                                    })}

                                {(!project.external_refs.is_empty())
                                    .then(|| {
                                        view! {
                                            <div class="flex flex-wrap gap-1">
                                                {project
                                                    .external_refs
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

                            </div>

                            // Tab Navigation
                            <div class="border-b border-ctp-surface1 mb-6">
                                <div class="flex gap-6">
                                    <button
                                        on:click=move |_| set_active_tab.set("task-lists")
                                        class=move || {
                                            if active_tab.get() == "task-lists" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >

                                        "Task Lists"
                                    </button>
                                    <button
                                        on:click=move |_| set_active_tab.set("notes")
                                        class=move || {
                                            if active_tab.get() == "notes" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >

                                        "Notes"
                                    </button>
                                    <button
                                        on:click=move |_| set_active_tab.set("repos")
                                        class=move || {
                                            if active_tab.get() == "repos" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >

                                        "Repos"
                                    </button>
                                </div>
                            </div>

                            // Tab Content
                            <div>
                                {move || {
                                    let _proj_id = project.id.clone();
                                    match active_tab.get() {
                                        "task-lists" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=task_list_search.search_input
                                                                on_change=task_list_search.on_debounced_change
                                                                on_immediate_change=task_list_search.on_immediate_change
                                                                placeholder="Search task lists..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=task_list_sort.sort_field
                                                            sort_order=task_list_sort.sort_order
                                                            on_sort_change=task_list_sort.on_sort_change
                                                            on_order_change=task_list_sort.on_order_change
                                                            fields=vec![
                                                                ("title".to_string(), "Title".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                                ("updated_at".to_string(), "Updated".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    // Show archived toggle
                                                    <div class="mb-4">
                                                        <label class="flex items-center gap-2 text-ctp-text cursor-pointer">
                                                            <input
                                                                type="checkbox"
                                                                prop:checked=move || show_archived_task_lists.get()
                                                                on:change=move |ev| {
                                                                    show_archived_task_lists.set(event_target_checked(&ev));
                                                                }
                                                                class="w-4 h-4 rounded bg-ctp-surface0 border-ctp-surface1 text-ctp-blue focus:ring-ctp-blue"
                                                            />
                                                            <span class="text-sm">"Show archived task lists"</span>
                                                        </label>
                                                    </div>

                                                    {move || match task_lists_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading task lists..."</p> }
                                                                .into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            let total_pages = paginated.total.div_ceil(TASK_LIST_PAGE_SIZE);

                                                            if paginated.items.is_empty() {
                                                                view! {
                                                                <p class="text-ctp-subtext0">
                                                                    {if task_list_search.search_query.get().trim().is_empty() {
                                                                        "No task lists for this project yet"
                                                                    } else {
                                                                        "No task lists found matching your search"
                                                                    }}
                                                                </p>
                                                            }
                                                                .into_any()
                                                            } else {
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                             {paginated.items
                                                                                .into_iter()
                                                                                .map(|task_list| {
                                                                                    let tl_clone = task_list.clone();
                                                                                    view! {
                                                                                        <TaskListCard
                                                                                            task_list=task_list
                                                                                            on_click=Callback::new(move |_list_id: String| {
                                                                                                selected_task_list.set(Some(tl_clone.clone()));
                                                                                                task_list_modal_open.set(true);
                                                                                            })
                                                                                        />
                                                                                    }
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        // Pagination
                                                                        <Pagination
                                                                            current_page=task_list_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=task_list_pagination.on_prev
                                                                            on_next=task_list_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=TASK_LIST_PAGE_SIZE
                                                                            item_name="task lists".to_string()
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
                                        "notes" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=note_search.search_input
                                                                on_change=note_search.on_debounced_change
                                                                on_immediate_change=note_search.on_immediate_change
                                                                placeholder="Search notes..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=note_sort.sort_field
                                                            sort_order=note_sort.sort_order
                                                            on_sort_change=note_sort.on_sort_change
                                                            on_order_change=note_sort.on_order_change
                                                            fields=vec![
                                                                ("title".to_string(), "Title".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                                ("last_activity_at".to_string(), "Updated".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match notes_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            let total_pages = paginated.total.div_ceil(NOTE_PAGE_SIZE);

                                                            // Backend already filtered with FTS5, just display results
                                                            if paginated.items.is_empty() {
                                                                view! { <p class="text-ctp-subtext0">"No notes found"</p> }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                            {paginated
                                                                                .items
                                                                                .iter()
                                                                                .map(|note| {
                                                                                    view! {
                                                                                         <NoteCard
                                                                                            note=note.clone()
                                                                                            on_click=Callback::new({
                                                                                                let has_subs = note.subnote_count.unwrap_or(0) > 0;
                                                                                                move |note_id: String| {
                                                                                                    selected_note_id.set(note_id);
                                                                                                    selected_note_has_subnotes.set(has_subs);
                                                                                                    note_modal_open.set(true);
                                                                                                }
                                                                                            })
                                                                                        />
                                                                                    }
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        // Pagination
                                                                        <Pagination
                                                                            current_page=note_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=note_pagination.on_prev
                                                                            on_next=note_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=NOTE_PAGE_SIZE
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
                                        "repos" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=repo_search.search_input
                                                                on_change=repo_search.on_debounced_change
                                                                on_immediate_change=repo_search.on_immediate_change
                                                                placeholder="Search repositories by remote URL or tags..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=repo_sort.sort_field
                                                            sort_order=repo_sort.sort_order
                                                            on_sort_change=repo_sort.on_sort_change
                                                            on_order_change=repo_sort.on_order_change
                                                            fields=vec![
                                                                ("remote".to_string(), "Remote".to_string()),
                                                                ("path".to_string(), "Path".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match repos_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading repos..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            if paginated.items.is_empty() {
                                                                view! {
                                                                    <p class="text-ctp-subtext0">
                                                                        {if repo_search.search_query.get().trim().is_empty() {
                                                                            "No repositories found. Add one to get started!"
                                                                        } else {
                                                                            "No repositories found matching your search."
                                                                        }}
                                                                    </p>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                let total_pages = paginated.total.div_ceil(REPO_PAGE_SIZE);
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
                                                                            current_page=repo_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=repo_pagination.on_prev
                                                                            on_next=repo_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=REPO_PAGE_SIZE
                                                                            item_name="repositories".to_string()
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
                                        _ => view! { <div></div> }.into_any(),
                                    }
                                }}

                            </div>
                        </div>
                    }
                        .into_any()
                }
                Some(Err(err)) => {
                    view! {
                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                            <p class="text-ctp-red font-semibold">"Error loading project"</p>
                            <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                        </div>
                    }
                        .into_any()
                }
            }}

            // Note detail modal - only render when open
            {move || {
                if note_modal_open.get() {
                    Some(view! {
                        <NoteDetailModal
                            note_id=selected_note_id.read_only()
                            open=note_modal_open
                            has_subnotes=selected_note_has_subnotes.get()
                        />
                    })
                } else {
                    None
                }
            }}

            // Task list detail modal - only render when open
            {move || {
                if task_list_modal_open.get() {
                    Some(view! {
                        <TaskListDetailModal
                            task_list=selected_task_list.read_only()
                            open=task_list_modal_open
                        />
                    })
                } else {
                    None
                }
            }}

        </div>
    }
}
