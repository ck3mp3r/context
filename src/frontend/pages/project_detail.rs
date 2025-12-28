use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, projects};
use crate::models::Project;

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let params = use_params_map();
    let project_id = move || params.read().get("id").unwrap_or_default();

    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);
    let (active_tab, set_active_tab) = signal("task-lists");

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
                                {move || match active_tab.get().as_ref() {
                                    "task-lists" => {
                                        view! {
                                            <div>
                                                <p class="text-ctp-subtext0">
                                                    "Task lists for this project will appear here"
                                                </p>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    "notes" => {
                                        view! {
                                            <div>
                                                <p class="text-ctp-subtext0">"Notes for this project will appear here"</p>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    "repos" => {
                                        view! {
                                            <div>
                                                <p class="text-ctp-subtext0">"Repos for this project will appear here"</p>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                    _ => view! { <div></div> }.into_any(),
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
