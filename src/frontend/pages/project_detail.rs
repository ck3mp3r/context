use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, QueryBuilder, projects};
use crate::components::{
    CopyableId, ExternalRefLink, NoteCard, NoteDetailModal, Pagination, RepoCard, SearchInput,
    TaskListCard, TaskListDetailModal,
};
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

    // Search/filter state for each tab
    let (task_list_filter_input, set_task_list_filter_input) = signal(String::new()); // Raw input
    let (task_list_filter, set_task_list_filter) = signal(String::new()); // Debounced (uses backend FTS5 API)
    let (note_filter_input, set_note_filter_input) = signal(String::new()); // Raw input
    let (note_filter, set_note_filter) = signal(String::new()); // Debounced (uses backend FTS5 API)
    let repo_filter = RwSignal::new(String::new()); // TODO: Add debouncing

    // Pagination state for task lists
    let (task_list_page, set_task_list_page) = signal(0usize);
    const TASK_LIST_PAGE_SIZE: usize = 12;

    // Pagination state for notes
    let (note_page, set_note_page) = signal(0usize);
    const NOTE_PAGE_SIZE: usize = 12;

    // Pagination state for repos
    let (repo_page, set_repo_page) = signal(0usize);
    const REPO_PAGE_SIZE: usize = 12;

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

    // Callbacks for SearchInput components
    let on_task_list_immediate_change = Callback::new(move |value: String| {
        set_task_list_filter_input.set(value);
    });
    let on_task_list_debounced_change = Callback::new(move |value: String| {
        set_task_list_filter.set(value);
        set_task_list_page.set(0); // Reset to first page on new search
    });

    let on_note_immediate_change = Callback::new(move |value: String| {
        set_note_filter_input.set(value);
    });
    let on_note_debounced_change = Callback::new(move |value: String| {
        set_note_filter.set(value);
        set_note_page.set(0); // Reset to first page on new search
    });

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
        set_task_list_page.set(0);
    });

    // Fetch task lists for this project (with archived toggle, search, and pagination)
    Effect::new(move || {
        let id = project_id();
        let show_archived = show_archived_task_lists.get();
        let search_query = task_list_filter.get();
        let current_page = task_list_page.get();
        let _ = task_list_refetch_trigger.get(); // Track refetch trigger from WebSocket updates
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
        let search = note_filter.get();
        let current_page = note_page.get();
        let _ = note_refetch_trigger.get(); // Track refetch trigger from WebSocket updates
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
        let current_page = repo_page.get();
        let _ = repo_refetch_trigger.get(); // Track refetch trigger from WebSocket updates
        if !id.is_empty() {
            spawn_local(async move {
                let offset = current_page * REPO_PAGE_SIZE;
                let result = QueryBuilder::<Repo>::new()
                    .limit(REPO_PAGE_SIZE)
                    .offset(offset)
                    .param("project_id", id)
                    .fetch()
                    .await;
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
                                                    // Search input
                                                    <div class="mb-4">
                                                        <SearchInput
                                                            value=task_list_filter_input
                                                            on_change=on_task_list_debounced_change
                                                            on_immediate_change=on_task_list_immediate_change
                                                            placeholder="Search task lists..."
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
                                                                    {if task_list_filter.get().trim().is_empty() {
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
                                                                            current_page=task_list_page
                                                                            total_pages=total_pages
                                                                            on_prev=Callback::new(move |_| {
                                                                                let current = task_list_page.get();
                                                                                if current > 0 {
                                                                                    set_task_list_page.set(current - 1);
                                                                                }
                                                                            })
                                                                            on_next=Callback::new(move |_| {
                                                                                let current = task_list_page.get();
                                                                                if current < total_pages - 1 {
                                                                                    set_task_list_page.set(current + 1);
                                                                                }
                                                                            })
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
                                                    // Search input
                                                    <div class="mb-4">
                                                        <SearchInput
                                                            value=note_filter_input
                                                            on_change=on_note_debounced_change
                                                            on_immediate_change=on_note_immediate_change
                                                            placeholder="Search notes..."
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
                                                                            current_page=note_page
                                                                            total_pages=total_pages
                                                                            on_prev=Callback::new(move |_| {
                                                                                let current = note_page.get();
                                                                                if current > 0 {
                                                                                    set_note_page.set(current - 1);
                                                                                }
                                                                            })
                                                                            on_next=Callback::new(move |_| {
                                                                                let current = note_page.get();
                                                                                if current < total_pages - 1 {
                                                                                    set_note_page.set(current + 1);
                                                                                }
                                                                            })
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
                                                    // Filter input with clear button
                                                    <div class="mb-4 relative">
                                                        <input
                                                            type="text"
                                                            placeholder="Filter repos..."
                                                            prop:value=move || repo_filter.get()
                                                            on:input=move |ev| {
                                                                repo_filter.set(event_target_value(&ev));
                                                            }

                                                            class="w-full px-4 py-2 pr-10 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text focus:outline-none focus:border-ctp-blue"
                                                        />
                                                        {move || {
                                                            if !repo_filter.get().is_empty() {
                                                                Some(
                                                                    view! {
                                                                        <button
                                                                            on:click=move |_| repo_filter.set(String::new())
                                                                            class="absolute right-3 top-1/2 -translate-y-1/2 w-5 h-5 rounded-full bg-ctp-overlay0 hover:bg-ctp-overlay1 flex items-center justify-center text-ctp-base text-xs"
                                                                        >
                                                                            "×"
                                                                        </button>
                                                                    },
                                                                )
                                                            } else {
                                                                None
                                                            }
                                                        }}

                                                    </div>

                                                    {move || match repos_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading repos..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            let search = repo_filter.get().to_lowercase();
                                                            let filtered: Vec<Repo> = paginated
                                                                .items
                                                                .iter()
                                                                .filter(|repo| {
                                                                    search.is_empty()
                                                                        || repo.remote.to_lowercase().contains(&search)
                                                                })
                                                                .cloned()
                                                                .collect();
                                                            if filtered.is_empty() {
                                                                view! { <p class="text-ctp-subtext0">"No repos linked to this project"</p> }
                                                                    .into_any()
                                                            } else {
                                                                let total_pages = paginated.total.div_ceil(REPO_PAGE_SIZE);
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                            {filtered
                                                                                .iter()
                                                                                .map(|repo| {
                                                                                    view! { <RepoCard repo=repo.clone()/> }
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        <Pagination
                                                                            current_page=repo_page
                                                                            total_pages=total_pages
                                                                            on_prev=Callback::new(move |_| {
                                                                                let current = repo_page.get();
                                                                                if current > 0 {
                                                                                    set_repo_page.set(current - 1);
                                                                                }
                                                                            })
                                                                            on_next=Callback::new(move |_| {
                                                                                let current = repo_page.get();
                                                                                if current < total_pages - 1 {
                                                                                    set_repo_page.set(current + 1);
                                                                                }
                                                                            })
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
