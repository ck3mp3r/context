use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, notes, projects, repos, task_lists};
use crate::components::{NoteCard, NoteDetailModal, Pagination, TaskListCard, TaskListDetailModal};
use crate::models::{Note, Paginated, Project, Repo, TaskList};

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
    let task_list_filter = RwSignal::new(String::new()); // TODO: Add debouncing
    let note_filter_input = RwSignal::new(String::new()); // Raw input
    let note_filter = RwSignal::new(String::new()); // Debounced (uses backend FTS5 API)
    let repo_filter = RwSignal::new(String::new()); // TODO: Add debouncing

    // Pagination state for task lists
    let (task_list_page, set_task_list_page) = signal(0usize);
    const TASK_LIST_PAGE_SIZE: usize = 12;

    // Show archived toggle
    let show_archived_task_lists = RwSignal::new(false);

    // Note detail modal state
    let note_modal_open = RwSignal::new(false);
    let selected_note_id = RwSignal::new(String::new());

    // Task list detail modal state
    let task_list_modal_open = RwSignal::new(false);
    let selected_task_list = RwSignal::new(None::<TaskList>);

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

    // Debounce search inputs (300ms delay)
    Effect::new(move || {
        let input = note_filter_input.get();
        use wasm_bindgen::JsCast;
        use wasm_bindgen::prelude::*;
        let callback = Closure::once(move || {
            note_filter.set(input.clone());
        });
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                300,
            )
            .unwrap();
        callback.forget();
    });

    // Reset pagination when archived toggle changes
    Effect::new(move || {
        show_archived_task_lists.get();
        set_task_list_page.set(0);
    });

    // Fetch task lists for this project (with archived toggle and pagination)
    Effect::new(move || {
        let id = project_id();
        let show_archived = show_archived_task_lists.get();
        let current_page = task_list_page.get();
        if !id.is_empty() {
            spawn_local(async move {
                let status = if show_archived { None } else { Some("active") };
                let offset = current_page * TASK_LIST_PAGE_SIZE;
                let result =
                    task_lists::list(Some(TASK_LIST_PAGE_SIZE), Some(offset), Some(id), status)
                        .await;
                set_task_lists_data.set(Some(result));
            });
        }
    });

    // Fetch notes for this project (with FTS5 search support)
    Effect::new(move || {
        let id = project_id();
        let search = note_filter.get();
        if !id.is_empty() {
            spawn_local(async move {
                let search_query = if search.trim().is_empty() {
                    None
                } else {
                    Some(search)
                };
                let result = notes::list(Some(50), None, search_query, Some(id)).await;
                set_notes_data.set(Some(result));
            });
        }
    });

    // Fetch repos for this project
    Effect::new(move || {
        let id = project_id();
        if !id.is_empty() {
            spawn_local(async move {
                let result = repos::list(Some(50), None, Some(id)).await;
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
                                    <h2 class="text-3xl font-bold text-ctp-text">{project.title.clone()}</h2>
                                    <a
                                        href="/"
                                        class="text-ctp-blue hover:text-ctp-lavender text-sm"
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
                                            <div class="flex flex-wrap gap-2">
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
                                                    // Filter input with clear button
                                                    <div class="mb-4 relative">
                                                        <input
                                                            type="text"
                                                            placeholder="Filter task lists..."
                                                            prop:value=move || task_list_filter.get()
                                                            on:input=move |ev| {
                                                                task_list_filter.set(event_target_value(&ev));
                                                            }

                                                            class="w-full px-4 py-2 pr-10 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text focus:outline-none focus:border-ctp-blue"
                                                        />
                                                        {move || {
                                                            if !task_list_filter.get().is_empty() {
                                                                Some(
                                                                    view! {
                                                                        <button
                                                                            on:click=move |_| task_list_filter.set(String::new())
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
                                                            let search = task_list_filter.get().to_lowercase();
                                                            let filtered: Vec<TaskList> = paginated
                                                                .items
                                                                .iter()
                                                                .filter(|list| {
                                                                    search.is_empty()
                                                                        || list.name.to_lowercase().contains(&search)
                                                                        || list
                                                                            .description
                                                                            .as_ref()
                                                                            .map(|d| d.to_lowercase().contains(&search))
                                                                            .unwrap_or(false)
                                                                })
                                                                .cloned()
                                                                .collect();

                                                            let total_pages = paginated.total.div_ceil(TASK_LIST_PAGE_SIZE);

                                                            if filtered.is_empty() {
                                                                view! {
                                                                <p class="text-ctp-subtext0">
                                                                    "No task lists for this project yet"
                                                                </p>
                                                            }
                                                                .into_any()
                                                            } else {
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                             {filtered
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
                                                    // Filter input with clear button
                                                    <div class="mb-4 relative">
                                                        <input
                                                            type="text"
                                                            placeholder="Search notes..."
                                                            prop:value=move || note_filter_input.get()
                                                            on:input=move |ev| {
                                                                note_filter_input.set(event_target_value(&ev));
                                                            }

                                                            class="w-full px-4 py-2 pr-10 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text placeholder-ctp-overlay0 focus:outline-none focus:border-ctp-blue"
                                                        />
                                                        {move || {
                                                            if !note_filter_input.get().is_empty() {
                                                                Some(
                                                                    view! {
                                                                        <button
                                                                            on:click=move |_| {
                                                                                note_filter_input.set(String::new());
                                                                                note_filter.set(String::new());
                                                                            }
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

                                                    {move || match notes_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            // Backend already filtered with FTS5, just display results
                                                            if paginated.items.is_empty() {
                                                                view! { <p class="text-ctp-subtext0">"No notes found"</p> }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 auto-rows-fr">
                                                                        {paginated
                                                                            .items
                                                                            .iter()
                                                                            .map(|note| {
                                                                                view! {
                                                                                     <NoteCard
                                                                                        note=note.clone()
                                                                                        on_click=Callback::new(move |note_id: String| {
                                                                                            selected_note_id.set(note_id);
                                                                                            note_modal_open.set(true);
                                                                                        })
                                                                                    />
                                                                                }
                                                                            })
                                                                            .collect::<Vec<_>>()}
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
                                                                view! {
                                                                    <div class="space-y-4">
                                                                        {filtered
                                                                            .iter()
                                                                            .map(|repo| {
                                                                                view! {
                                                                                    <div class="bg-ctp-surface0 rounded-lg p-4 border border-ctp-surface1">
                                                                                        <h3 class="font-semibold text-ctp-text">{repo.remote.clone()}</h3>
                                                                                    </div>
                                                                                }
                                                                            })
                                                                            .collect::<Vec<_>>()}
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
