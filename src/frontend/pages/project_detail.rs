use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use std::collections::HashMap;

use crate::api::{ApiClientError, notes, projects, repos, task_lists};
use crate::models::{Note, Paginated, Project, Repo, Task, TaskList};

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
    let (swim_lane_tasks, set_swim_lane_tasks) =
        signal(HashMap::<String, Result<Vec<Task>, ApiClientError>>::new());

    // Search/filter state for each tab
    let task_list_filter = RwSignal::new(String::new());
    let note_filter = RwSignal::new(String::new());
    let repo_filter = RwSignal::new(String::new());

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

    // Fetch task lists for this project
    Effect::new(move || {
        let id = project_id();
        if !id.is_empty() {
            spawn_local(async move {
                // TODO: Backend needs to support filtering by project_id
                // For now, fetch all and filter client-side
                let result = task_lists::list(Some(100), None).await;
                set_task_lists_data.set(Some(result));
            });
        }
    });

    // Fetch notes for this project
    Effect::new(move || {
        let id = project_id();
        if !id.is_empty() {
            spawn_local(async move {
                // TODO: Backend needs to support filtering by project_id
                let result = notes::list(Some(50), None, None).await;
                set_notes_data.set(Some(result));
            });
        }
    });

    // Fetch repos for this project
    Effect::new(move || {
        let id = project_id();
        if !id.is_empty() {
            spawn_local(async move {
                // TODO: Backend needs to support filtering by project
                let result = repos::list(Some(50), None).await;
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
                                    let proj_id = project.id.clone();
                                    match active_tab.get().as_ref() {
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
                                                                    list.project_id == proj_id
                                                                        && (search.is_empty()
                                                                            || list.name.to_lowercase().contains(&search)
                                                                            || list
                                                                                .description
                                                                                .as_ref()
                                                                                .map(|d| d.to_lowercase().contains(&search))
                                                                                .unwrap_or(false))
                                                                })
                                                                .cloned()
                                                                .collect();
                                                            if filtered.is_empty() {
                                                                view! {
                                                                    <p class="text-ctp-subtext0">
                                                                        "No task lists for this project yet"
                                                                    </p>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="space-y-4">
                                                                        {filtered
                                                                            .iter()
                                                                            .map(|list| {
                                                                                view! {
                                                                                    <div class="bg-ctp-surface0 rounded-lg p-4 border border-ctp-surface1">
                                                                                        <h3 class="font-semibold text-ctp-text mb-2">
                                                                                            {list.name.clone()}
                                                                                        </h3>
                                                                                        {list
                                                                                            .description
                                                                                            .as_ref()
                                                                                            .map(|desc| {
                                                                                                view! { <p class="text-sm text-ctp-subtext0">{desc.clone()}</p> }
                                                                                            })}

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
                                        "notes" => {
                                            view! {
                                                <div>
                                                    // Filter input with clear button
                                                    <div class="mb-4 relative">
                                                        <input
                                                            type="text"
                                                            placeholder="Filter notes..."
                                                            prop:value=move || note_filter.get()
                                                            on:input=move |ev| {
                                                                note_filter.set(event_target_value(&ev));
                                                            }

                                                            class="w-full px-4 py-2 pr-10 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text focus:outline-none focus:border-ctp-blue"
                                                        />
                                                        {move || {
                                                            if !note_filter.get().is_empty() {
                                                                Some(
                                                                    view! {
                                                                        <button
                                                                            on:click=move |_| note_filter.set(String::new())
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
                                                            let search = note_filter.get().to_lowercase();
                                                            let filtered: Vec<Note> = paginated
                                                                .items
                                                                .iter()
                                                                .filter(|note| {
                                                                    note.project_ids.contains(&proj_id)
                                                                        && (search.is_empty()
                                                                            || note.title.to_lowercase().contains(&search))
                                                                })
                                                                .cloned()
                                                                .collect();
                                                            if filtered.is_empty() {
                                                                view! { <p class="text-ctp-subtext0">"No notes linked to this project"</p> }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="space-y-4">
                                                                        {filtered
                                                                            .iter()
                                                                            .map(|note| {
                                                                                view! {
                                                                                    <div class="bg-ctp-surface0 rounded-lg p-4 border border-ctp-surface1">
                                                                                        <h3 class="font-semibold text-ctp-text">{note.title.clone()}</h3>
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
                                                                    repo.tags.iter().any(|tag| tag == &proj_id)
                                                                        && (search.is_empty()
                                                                            || repo.remote.to_lowercase().contains(&search))
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

        </div>
    }
}
