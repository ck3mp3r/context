use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, projects, task_lists};
use crate::components::{CopyableId, TaskListContent};
use crate::models::{Project, TaskList};

#[component]
pub fn TaskListDetail() -> impl IntoView {
    let params = use_params_map();

    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);
    let (task_list_data, set_task_list_data) = signal(None::<Result<TaskList, ApiClientError>>);

    // Fetch project and task list when params change
    Effect::new(move || {
        let params = params.get();
        if let (Some(project_id), Some(task_list_id)) =
            (params.get("project_id"), params.get("task_list_id"))
        {
            let project_id = project_id.to_string();
            let task_list_id = task_list_id.to_string();

            // Fetch project
            spawn_local({
                let project_id = project_id.clone();
                async move {
                    let result = projects::get(&project_id).await;
                    set_project_data.set(Some(result));
                }
            });

            // Fetch task list
            spawn_local(async move {
                let result = task_lists::get(&task_list_id).await;
                set_task_list_data.set(Some(result));
            });
        }
    });

    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]">
            // Breadcrumb navigation
            <div class="bg-ctp-surface0 border-b border-ctp-surface1 py-2 px-6">
                <div class="container mx-auto flex items-center gap-2 text-sm">
                    {move || {
                        match (project_data.get(), task_list_data.get()) {
                            (Some(Ok(project)), Some(Ok(task_list))) => {
                                view! {
                                    <a
                                        href=format!("/projects/{}", project.id)
                                        class="text-ctp-blue hover:text-ctp-sapphire transition-colors flex items-center gap-2"
                                    >
                                        <span class="font-medium">{project.title.clone()}</span>
                                        <CopyableId id=project.id.clone()/>
                                    </a>
                                    <span class="text-ctp-overlay0">"/"</span>
                                    <div class="flex items-center gap-2">
                                        <span class="text-ctp-text font-medium">{task_list.title.clone()}</span>
                                        <CopyableId id=task_list.id.clone()/>
                                    </div>
                                }
                                    .into_any()
                            }
                            _ => {
                                view! {
                                    <span class="text-ctp-overlay0">"Loading..."</span>
                                }
                                    .into_any()
                            }
                        }
                    }}
                </div>
            </div>

            <div class="container mx-auto px-6 py-6 flex-1">
                <Suspense fallback=move || {
                    view! { <p class="text-ctp-subtext0">"Loading task list..."</p> }
                }>
                    {move || {
                        match task_list_data.get() {
                            Some(Ok(task_list)) => {
                                let task_list_signal = Signal::derive(move || task_list.clone());
                                view! {
                                    <TaskListContent
                                        task_list=task_list_signal
                                        show_close_button=false
                                    />
                                }
                                    .into_any()
                            }
                            Some(Err(err)) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading task list"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                            None => {
                                view! { <p class="text-ctp-subtext0">"Loading..."</p> }
                                    .into_any()
                            }
                        }
                    }}
                </Suspense>
            </div>
        </div>
    }
}
