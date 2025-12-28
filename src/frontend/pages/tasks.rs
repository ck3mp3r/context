use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, task_lists, tasks};
use crate::models::{Paginated, Task, TaskList};

#[component]
pub fn Tasks() -> impl IntoView {
    // State
    let (selected_list_id, set_selected_list_id) = signal(None::<String>);
    let (task_lists_data, set_task_lists_data) =
        signal(None::<Result<Paginated<TaskList>, ApiClientError>>);
    let (tasks_data, set_tasks_data) = signal(None::<Result<Paginated<Task>, ApiClientError>>);

    // Fetch task lists on mount
    Effect::new(move || {
        spawn_local(async move {
            let result = task_lists::list(Some(100), None).await;
            set_task_lists_data.set(Some(result));
        });
    });

    // Fetch tasks when selected list changes
    Effect::new(move || {
        if let Some(list_id) = selected_list_id.get() {
            set_tasks_data.set(None); // Loading
            spawn_local(async move {
                let result = tasks::list_for_task_list(&list_id, Some(200), None).await;
                set_tasks_data.set(Some(result));
            });
        }
    });

    view! {
        <div class="container mx-auto p-6">
            <h2 class="text-3xl font-bold text-ctp-text mb-6">"Tasks"</h2>

            // Task List Selector
            <div class="mb-6">
                {move || match task_lists_data.get() {
                    None => {
                        view! { <p class="text-ctp-subtext0">"Loading task lists..."</p> }.into_any()
                    }
                    Some(result) => {
                        match result {
                            Ok(paginated) => {
                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">"No task lists found. Create one to get started!"</p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <select
                                            on:change=move |ev| {
                                                let value = event_target_value(&ev);
                                                if !value.is_empty() {
                                                    set_selected_list_id.set(Some(value));
                                                }
                                            }

                                            class="w-full px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text focus:outline-none focus:border-ctp-blue"
                                        >
                                            <option value="">"Select a task list..."</option>
                                            {paginated
                                                .items
                                                .iter()
                                                .map(|list| {
                                                    view! {
                                                        <option value=list.id.clone()>{list.name.clone()}</option>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </select>
                                    }
                                        .into_any()
                                }
                            }
                            Err(err) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading task lists"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    }
                }}

            </div>

            // Kanban Board
            {move || {
                if selected_list_id.get().is_none() {
                    return view! {
                        <p class="text-ctp-subtext0 text-center py-12">
                            "Select a task list to view the kanban board"
                        </p>
                    }
                        .into_any();
                }
                match tasks_data.get() {
                    None => view! { <p class="text-ctp-subtext0">"Loading tasks..."</p> }.into_any(),
                    Some(result) => {
                        match result {
                            Ok(paginated) => {
                                view! { <KanbanBoard tasks=paginated.items/> }.into_any()
                            }
                            Err(err) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading tasks"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    }
                }
            }}

        </div>
    }
}

#[component]
fn KanbanBoard(tasks: Vec<Task>) -> impl IntoView {
    let statuses = vec![
        ("backlog", "Backlog"),
        ("todo", "Todo"),
        ("in_progress", "In Progress"),
        ("review", "Review"),
        ("done", "Done"),
        ("cancelled", "Cancelled"),
    ];

    view! {
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4">
            {statuses
                .into_iter()
                .map(|(status, label)| {
                    let column_tasks: Vec<Task> = tasks
                        .iter()
                        .filter(|t| t.status == status)
                        .cloned()
                        .collect();
                    view! { <KanbanColumn status=status label=label tasks=column_tasks/> }
                })
                .collect::<Vec<_>>()}
        </div>
    }
}

#[component]
fn KanbanColumn(status: &'static str, label: &'static str, tasks: Vec<Task>) -> impl IntoView {
    let bg_color = match status {
        "backlog" => "bg-ctp-surface0",
        "todo" => "bg-ctp-blue/10",
        "in_progress" => "bg-ctp-yellow/10",
        "review" => "bg-ctp-mauve/10",
        "done" => "bg-ctp-green/10",
        "cancelled" => "bg-ctp-red/10",
        _ => "bg-ctp-surface0",
    };

    view! {
        <div class=format!("{} rounded-lg p-4 min-h-[400px] overflow-hidden", bg_color)>
            <h3 class="font-semibold text-ctp-text mb-4 flex justify-between items-center">
                <span>{label}</span>
                <span class="text-xs bg-ctp-surface1 px-2 py-1 rounded">{tasks.len()}</span>
            </h3>
            <div class="space-y-2 overflow-y-auto">
                {tasks
                    .into_iter()
                    .map(|task| view! { <TaskCard task=task/> })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn TaskCard(task: Task) -> impl IntoView {
    let priority_color = match task.priority {
        Some(1) => "border-l-ctp-red",
        Some(2) => "border-l-ctp-peach",
        Some(3) => "border-l-ctp-yellow",
        Some(4) => "border-l-ctp-blue",
        Some(5) => "border-l-ctp-overlay0",
        _ => "border-l-ctp-surface1",
    };

    view! {
        <div class=format!(
            "bg-ctp-base border-l-4 {} rounded p-3 hover:shadow-lg transition-shadow cursor-pointer",
            priority_color,
        )>
            <p class="text-sm text-ctp-text mb-2 break-words">{task.content.clone()}</p>

            {(!task.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-1 mt-2">
                            {task
                                .tags
                                .iter()
                                .map(|tag| {
                                    view! {
                                        <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-2 py-0.5 rounded">
                                            {tag.clone()}
                                        </span>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                    }
                })}

            {task
                .priority
                .map(|p| {
                    view! {
                        <div class="text-xs text-ctp-overlay0 mt-2">"P" {p}</div>
                    }
                })}
        </div>
    }
}
