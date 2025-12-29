use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;
use thaw::*;

use crate::api::{ApiClientError, task_lists, tasks};
use crate::models::{Task, TaskList, TaskStats};

// Context for accordion state - shared across all swim lanes (DEPRECATED - keeping for compatibility)
#[derive(Clone, Copy)]
pub struct AccordionContext {
    pub expanded_id: ReadSignal<Option<String>>,
    pub set_expanded_id: WriteSignal<Option<String>>,
}

#[component]
pub fn SwimLane(
    task_list: TaskList,
    tasks_map: ReadSignal<HashMap<String, Result<Vec<Task>, ApiClientError>>>,
) -> impl IntoView {
    // Get accordion state from context
    let AccordionContext {
        expanded_id,
        set_expanded_id,
    } = expect_context();

    let list_id = task_list.id.clone();
    let list_id_for_click = list_id.clone();
    let list_id_for_icon = list_id.clone();
    let list_id_for_summary = list_id.clone();
    let list_id_for_expand_check = list_id.clone();

    let statuses = vec![
        ("backlog", "Backlog"),
        ("todo", "Todo"),
        ("in_progress", "In Progress"),
        ("review", "Review"),
        ("done", "Done"),
        ("cancelled", "Cancelled"),
    ];

    let list_id_for_counts = task_list.id.clone();
    let list_id_for_columns = task_list.id.clone();
    let statuses_for_counts = statuses.clone();
    let statuses_for_columns = statuses.clone();

    // Calculate task counts for summary
    let task_counts = move || {
        let list_id = list_id_for_counts.clone();
        match tasks_map.get().get(&list_id) {
            Some(Ok(tasks)) => {
                let total = tasks.len();
                let by_status: Vec<(String, usize)> = statuses_for_counts
                    .iter()
                    .map(|(status, _)| {
                        let count = tasks.iter().filter(|t| t.status == *status).count();
                        (status.to_string(), count)
                    })
                    .filter(|(_, count)| *count > 0)
                    .collect();
                (total, by_status)
            }
            _ => (0, vec![]),
        }
    };

    view! {
        <div class="border border-ctp-surface1 rounded-lg overflow-hidden">
            // Swim Lane Header (Clickable Accordion)
            <button
                on:click=move |_| {
                    let current_expanded = expanded_id.get();
                    if current_expanded.as_ref() == Some(&list_id_for_click) {
                        // If this lane is expanded, collapse it
                        set_expanded_id.set(None);
                    } else {
                        // Otherwise, expand this lane (closing any other)
                        set_expanded_id.set(Some(list_id_for_click.clone()));
                    }
                }

                class="w-full bg-ctp-surface0 px-4 py-3 hover:bg-ctp-surface1 transition-colors"
            >
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3 flex-1 text-left">
                        // Expand/Collapse Icon
                        <span class="text-ctp-blue text-xl">
                            {move || {
                                if expanded_id.get().as_ref() == Some(&list_id_for_icon) { "▼" } else { "▶" }
                            }}
                        </span>

                        <div class="flex-1">
                            <h3 class="font-semibold text-ctp-text">{task_list.name.clone()}</h3>
                            {task_list
                                .description
                                .as_ref()
                                .map(|desc| {
                                    view! {
                                        <p class="text-sm text-ctp-subtext0 mt-0.5">
                                            {desc.chars().take(100).collect::<String>()}
                                        </p>
                                    }
                                })}

                            // Task Summary (when collapsed)
                            {move || {
                                if expanded_id.get().as_ref() != Some(&list_id_for_summary) {
                                    let (total, by_status) = task_counts();
                                    Some(
                                        view! {
                                            <div class="flex gap-2 mt-2 text-xs">
                                                <span class="text-ctp-subtext1">
                                                    {total} " tasks"
                                                </span>
                                                {by_status
                                                    .into_iter()
                                                    .map(|(status, count)| {
                                                        let color = match status.as_str() {
                                                            "backlog" => "text-ctp-overlay0",
                                                            "todo" => "text-ctp-blue",
                                                            "in_progress" => "text-ctp-yellow",
                                                            "review" => "text-ctp-mauve",
                                                            "done" => "text-ctp-green",
                                                            "cancelled" => "text-ctp-red",
                                                            _ => "text-ctp-subtext0",
                                                        };
                                                        view! {
                                                            <span class=color>
                                                                {status.replace('_', " ")} ": " {count}
                                                            </span>
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}

                                            </div>
                                        },
                                    )
                                } else {
                                    None
                                }
                            }}

                        </div>
                    </div>

                    {(!task_list.tags.is_empty())
                        .then(|| {
                            view! {
                                <div class="flex flex-wrap gap-1">
                                    {task_list
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

                </div>
            </button>

            // Status Columns (Only when expanded)
            {move || {
                if expanded_id.get().as_ref() != Some(&list_id_for_expand_check) {
                    return view! { <div></div> }.into_any();
                }
                let list_id = list_id_for_columns.clone();
                match tasks_map.get().get(&list_id) {
                    None => {
                        view! {
                            <div class="p-8 text-center text-ctp-subtext0">"Loading tasks..."</div>
                        }
                            .into_any()
                    }
                    Some(Ok(tasks)) => {
                        view! {
                            <div class="border-t border-ctp-surface1">
                                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
                                    {statuses_for_columns
                                        .clone()
                                        .into_iter()
                                        .map(|(status, label)| {
                                            let column_tasks: Vec<Task> = tasks
                                                .iter()
                                                .filter(|t| t.status == status)
                                                .cloned()
                                                .collect();
                                            // Simple inline column for deprecated SwimLane component
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
                                                <div class=format!("{} rounded-lg p-4 min-h-[200px]", bg_color)>
                                                    <h3 class="font-semibold text-ctp-text mb-4 flex justify-between items-center">
                                                        <span>{label}</span>
                                                        <span class="text-xs bg-ctp-surface1 px-2 py-1 rounded">{column_tasks.len()}</span>
                                                    </h3>
                                                    <div class="space-y-2 overflow-y-auto max-h-[300px]">
                                                        {column_tasks
                                                            .into_iter()
                                                            .map(|task| view! { <TaskCard task=task/> })
                                                            .collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                    Some(Err(err)) => {
                        view! {
                            <div class="p-4 bg-ctp-red/10 border-t border-ctp-surface1">
                                <p class="text-ctp-red text-sm">
                                    "Error loading tasks: " {err.to_string()}
                                </p>
                            </div>
                        }
                            .into_any()
                    }
                }
            }}

        </div>
    }
}

#[component]
pub fn KanbanColumn(
    status: &'static str,
    label: &'static str,
    list_id: String,
    total_count: usize,
) -> impl IntoView {
    let (tasks, set_tasks) = signal(Vec::<Task>::new());
    let (offset, set_offset) = signal(0);
    let (loading, set_loading) = signal(false);

    // Task detail dialog state
    let (selected_task, set_selected_task) = signal(None::<Task>);
    let dialog_open = RwSignal::new(false);
    let (initial_open_subtask, set_initial_open_subtask) = signal(None::<String>);

    // Reset initial_open_subtask when dialog closes
    Effect::new(move || {
        if !dialog_open.get() {
            set_initial_open_subtask.set(None);
        }
    });

    // Store list_id in a signal so it can be shared across closures
    let list_id_signal = StoredValue::new(list_id.clone());

    // Determine sort order based on status
    let (sort_field, sort_order) = match status {
        "backlog" | "todo" => ("priority", "asc"), // Priority 1-5, nulls last
        "done" | "cancelled" => ("created_at", "desc"), // Newest first (use completed_at when backend supports it)
        _ => ("created_at", "desc"),                    // In progress, review: newest first
    };

    // Initial fetch
    Effect::new(move |_| {
        let list_id = list_id_signal.get_value();
        spawn_local(async move {
            let result = tasks::list_for_task_list(
                &list_id,
                Some(25),
                Some(0),
                Some(status),
                Some(sort_field),
                Some(sort_order),
                None,
            )
            .await;
            if let Ok(paginated) = result {
                set_tasks.set(paginated.items);
            }
        });
    });

    let load_more = move |_| {
        set_loading.set(true);
        let list_id = list_id_signal.get_value();
        let current_offset = offset.get();
        let new_offset = current_offset + 25;

        spawn_local(async move {
            let result = tasks::list_for_task_list(
                &list_id,
                Some(25),
                Some(new_offset),
                Some(status),
                Some(sort_field),
                Some(sort_order),
                None,
            )
            .await;
            if let Ok(paginated) = result {
                set_tasks.update(|t| t.extend(paginated.items));
                set_offset.set(new_offset);
            }
            set_loading.set(false);
        });
    };

    let bg_color = match status {
        "backlog" => "bg-ctp-surface0",
        "todo" => "bg-ctp-blue/10",
        "in_progress" => "bg-ctp-yellow/10",
        "review" => "bg-ctp-mauve/10",
        "done" => "bg-ctp-green/10",
        "cancelled" => "bg-ctp-red/10",
        _ => "bg-ctp-surface0",
    };

    let scroll_ref = NodeRef::<leptos::html::Div>::new();

    // Infinite scroll: load more when scrolling near bottom
    let on_scroll = move |_| {
        if loading.get() {
            return;
        }

        if let Some(el) = scroll_ref.get() {
            let scroll_height = el.scroll_height() as f64;
            let scroll_top = el.scroll_top() as f64;
            let client_height = el.client_height() as f64;

            // Load more when within 200px of bottom
            let displayed = tasks.get().len();
            if scroll_top + client_height >= scroll_height - 200.0 && displayed < total_count {
                load_more(());
            }
        }
    };

    view! {
        <div class=format!("{} rounded-lg p-4 flex flex-col h-full overflow-hidden", bg_color)>
            <h3 class="font-semibold text-ctp-text mb-4 flex justify-between items-center flex-shrink-0">
                <span>{label}</span>
                <span class="text-xs bg-ctp-surface1 px-2 py-1 rounded">
                    {move || {
                        let displayed = tasks.get().len();
                        if displayed < total_count {
                            format!("{} of {}", displayed, total_count)
                        } else {
                            format!("{}", total_count)
                        }
                    }}
                </span>
            </h3>
            <div
                node_ref=scroll_ref
                on:scroll=on_scroll
                class="space-y-2 overflow-y-auto flex-1 min-h-0"
            >
                {move || tasks.get()
                    .into_iter()
                    .map(|task| {
                        view! {
                            <TaskCard
                                task=task
                                show_subtasks_inline=true
                                on_click=Callback::new(move |t: Task| {
                                    set_selected_task.set(Some(t.clone()));
                                    set_initial_open_subtask.set(None);
                                    dialog_open.set(true);
                                })
                                on_subtask_click=Callback::new(move |clicked_subtask: Task| {
                                    if let Some(parent_id) = clicked_subtask.parent_id.clone() {
                                        let clicked_id = clicked_subtask.id.clone();
                                        spawn_local(async move {
                                            match tasks::get(&parent_id).await {
                                                Ok(parent_task) => {
                                                    set_selected_task.set(Some(parent_task));
                                                    set_initial_open_subtask.set(Some(clicked_id));
                                                    dialog_open.set(true);
                                                }
                                                Err(_) => {
                                                    // Fallback: show subtask directly
                                                    set_selected_task.set(Some(clicked_subtask));
                                                    set_initial_open_subtask.set(None);
                                                    dialog_open.set(true);
                                                }
                                            }
                                        });
                                    }
                                })
                            />
                        }
                    })
                    .collect::<Vec<_>>()}

                {move || {
                    loading.get().then(|| {
                        view! {
                            <div class="py-4 text-center">
                                <span class="text-ctp-subtext0 text-sm">"Loading more..."</span>
                            </div>
                        }
                    })
                }}
            </div>

            // Task detail dialog
            {move || {
                selected_task.get().map(|task| {
                    match initial_open_subtask.get() {
                        Some(subtask_id) => view! {
                            <TaskDetailDialog
                                task=task
                                open=dialog_open
                                initial_open_subtask_id=subtask_id
                            />
                        }.into_any(),
                        None => view! {
                            <TaskDetailDialog
                                task=task
                                open=dialog_open
                            />
                        }.into_any(),
                    }
                })
            }}
        </div>
    }
}

#[component]
pub fn TaskCard(
    task: Task,
    #[prop(optional, default = false)] show_subtasks_inline: bool,
    #[prop(optional)] on_click: Option<Callback<Task>>,
    #[prop(optional)] on_subtask_click: Option<Callback<Task>>,
) -> impl IntoView {
    let priority_color = match task.priority {
        Some(1) => "border-l-ctp-red",
        Some(2) => "border-l-ctp-peach",
        Some(3) => "border-l-ctp-yellow",
        Some(4) => "border-l-ctp-blue",
        Some(5) => "border-l-ctp-overlay0",
        _ => "border-l-ctp-surface1",
    };

    // Fetch subtask count
    let (subtask_count, set_subtask_count) = signal(0usize);
    let (subtasks_expanded, set_subtasks_expanded) = signal(false);
    let task_id = task.id.clone();
    let list_id = task.list_id.clone();

    Effect::new(move || {
        let task_id = task_id.clone();
        let list_id = list_id.clone();
        spawn_local(async move {
            // Fetch tasks where parent_id == this task's id
            match tasks::list_for_task_list(
                &list_id,
                Some(1),        // limit 1 - we only need the count
                None,           // offset
                None,           // status
                None,           // sort
                None,           // order
                Some(&task_id), // parent_id filter
            )
            .await
            {
                Ok(paginated) => {
                    set_subtask_count.set(paginated.total);
                }
                Err(_) => {
                    // Silently fail - subtask count is non-critical
                    set_subtask_count.set(0);
                }
            }
        });
    });

    let toggle_subtasks = move |e: ev::MouseEvent| {
        e.stop_propagation(); // Don't trigger parent task click
        set_subtasks_expanded.update(|v| *v = !*v);
    };

    let task_id_for_list = task.id.clone();
    let list_id_for_list = task.list_id.clone();

    let task_for_click = task.clone();
    let handle_card_click = move |_| {
        if let Some(callback) = on_click {
            callback.run(task_for_click.clone());
        }
    };

    view! {
        <div>
            <div
                class=move || {
                    format!(
                        "bg-ctp-base border-l-4 {} rounded p-3 hover:shadow-lg transition-shadow cursor-pointer {}",
                        priority_color,
                        if subtask_count.get() > 0 { "task-card-parent" } else { "" },
                    )
                }
                on:click=handle_card_click
            >
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

                <div class="flex items-center justify-between mt-2">
                    <div class="flex items-center gap-2">
                        {task
                            .priority
                            .map(|p| {
                                view! {
                                    <div class="text-xs text-ctp-overlay0">"P" {p}</div>
                                }
                            })}

                        {move || {
                            (show_subtasks_inline && subtask_count.get() > 0).then(|| {
                                view! {
                                    <button
                                        on:click=toggle_subtasks
                                        class="flex items-center gap-1 text-xs text-ctp-brand hover:text-ctp-blue transition-colors"
                                    >
                                        <span>{if subtasks_expanded.get() { "▼" } else { "▶" }}</span>
                                        <Badge size=BadgeSize::Small appearance=BadgeAppearance::Outline color=BadgeColor::Brand>
                                            "📁 " {subtask_count.get()} " subtask" {if subtask_count.get() > 1 { "s" } else { "" }}
                                        </Badge>
                                    </button>
                                }
                            })
                        }}

                        {move || {
                            (!show_subtasks_inline && subtask_count.get() > 0).then(|| {
                                view! {
                                    <Badge size=BadgeSize::Small appearance=BadgeAppearance::Outline color=BadgeColor::Brand>
                                        "📁 " {subtask_count.get()} " subtask" {if subtask_count.get() > 1 { "s" } else { "" }}
                                    </Badge>
                                }
                            })
                        }}
                    </div>
                </div>
            </div>

            {move || {
                (show_subtasks_inline && subtasks_expanded.get() && subtask_count.get() > 0).then(|| {
                    match on_subtask_click {
                        Some(callback) => view! {
                            <SubtaskList
                                task_id=task_id_for_list.clone()
                                list_id=list_id_for_list.clone()
                                on_subtask_click=callback
                            />
                        }.into_any(),
                        None => view! {
                            <SubtaskList
                                task_id=task_id_for_list.clone()
                                list_id=list_id_for_list.clone()
                            />
                        }.into_any(),
                    }
                })
            }}
        </div>
    }
}

/// SubtaskList component - displays subtasks for a parent task
/// Constraint: 1 level deep only (does not recursively show sub-subtasks)
#[component]
pub fn SubtaskList(
    #[prop(into)] task_id: String,
    #[prop(into)] list_id: String,
    #[prop(optional)] on_subtask_click: Option<Callback<Task>>,
) -> impl IntoView {
    let (subtasks, set_subtasks) = signal(Vec::<Task>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);

    let task_id_for_fetch = task_id.clone();
    let list_id_for_fetch = list_id.clone();

    // Fetch subtasks on mount
    Effect::new(move || {
        let task_id = task_id_for_fetch.clone();
        let list_id = list_id_for_fetch.clone();
        spawn_local(async move {
            match tasks::list_for_task_list(
                &list_id,
                None,             // limit - get all subtasks
                None,             // offset
                None,             // status
                Some("priority"), // sort by priority
                Some("asc"),      // ascending
                Some(&task_id),   // parent_id filter
            )
            .await
            {
                Ok(paginated) => {
                    set_subtasks.set(paginated.items);
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(format!("Failed to load subtasks: {}", e)));
                    set_loading.set(false);
                }
            }
        });
    });

    view! {
        <div class="ml-3 border-l-2 border-ctp-surface1 pl-2 mt-3">
            {move || {
                if loading.get() {
                    view! { <p class="text-xs text-ctp-overlay0">"Loading subtasks..."</p> }.into_any()
                } else if let Some(err) = error.get() {
                    view! { <p class="text-xs text-ctp-red">{err}</p> }.into_any()
                } else if subtasks.get().is_empty() {
                    view! { <p class="text-xs text-ctp-overlay0">"No subtasks"</p> }.into_any()
                } else {
                    match on_subtask_click {
                        Some(callback) => view! {
                            <For
                                each=move || subtasks.get()
                                key=|task| task.id.clone()
                                let:subtask
                            >
                                <div class="my-2">
                                    <TaskCard task=subtask.clone() on_click=callback />
                                </div>
                            </For>
                        }.into_any(),
                        None => view! {
                            <For
                                each=move || subtasks.get()
                                key=|task| task.id.clone()
                                let:subtask
                            >
                                <div class="my-2">
                                    <TaskCard task=subtask.clone() />
                                </div>
                            </For>
                        }.into_any(),
                    }
                }
            }}
        </div>
    }
}

/// TaskDetailContent component - reusable task detail content (used in both drawer and split view)
#[component]
pub fn TaskDetailContent(
    task: Task,
    #[prop(optional)] initial_open_subtask_id: Option<String>,
) -> impl IntoView {
    // Fetch subtasks
    let (subtasks, set_subtasks) = signal(Vec::<Task>::new());
    let task_id_for_fetch = task.id.clone();
    let list_id_for_fetch = task.list_id.clone();

    Effect::new(move || {
        let task_id = task_id_for_fetch.clone();
        let list_id = list_id_for_fetch.clone();
        spawn_local(async move {
            match tasks::list_for_task_list(
                &list_id,
                None,
                None,
                None,
                Some("priority"),
                Some("asc"),
                Some(&task_id),
            )
            .await
            {
                Ok(paginated) => {
                    set_subtasks.set(paginated.items);
                }
                Err(_) => {
                    set_subtasks.set(Vec::new());
                }
            }
        });
    });

    // Determine status color for left border (matching kanban columns)
    let status_color = match task.status.as_str() {
        "backlog" => "border-l-ctp-surface1",
        "todo" => "border-l-ctp-blue",
        "in_progress" => "border-l-ctp-yellow",
        "review" => "border-l-ctp-mauve",
        "done" => "border-l-ctp-green",
        "cancelled" => "border-l-ctp-red",
        _ => "border-l-ctp-surface1",
    };

    view! {
        <div>
            // Main task - description first, metadata secondary
            <div class=format!("mb-4 p-4 bg-ctp-surface0 rounded-lg border-l-4 {}", status_color)>
                // Task description - prominent
                <div class="text-base text-ctp-text whitespace-pre-wrap break-words mb-4">
                    {task.content.clone()}
                </div>

                // Metadata - compact and less prominent
                <div class="pt-3 border-t border-ctp-surface1">
                    <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ctp-overlay0">
                        <div>
                            <span class="text-ctp-overlay1">"Status: "</span>
                            <span>{format!("{:?}", task.status)}</span>
                        </div>

                        {task.priority.map(|p| {
                            view! {
                                <div>
                                    <span class="text-ctp-overlay1">"Priority: "</span>
                                    <span>"P"{p}</span>
                                </div>
                            }
                        })}

                        <div>
                            <span class="text-ctp-overlay1">"Created: "</span>
                            <span>{task.created_at.clone()}</span>
                        </div>

                        {task.started_at.clone().map(|started| {
                            view! {
                                <div>
                                    <span class="text-ctp-overlay1">"Started: "</span>
                                    <span>{started}</span>
                                </div>
                            }
                        })}

                        {task.completed_at.clone().map(|completed| {
                            view! {
                                <div>
                                    <span class="text-ctp-overlay1">"Completed: "</span>
                                    <span>{completed}</span>
                                </div>
                            }
                        })}
                    </div>

                    {(!task.tags.is_empty()).then(|| {
                        view! {
                            <div class="flex flex-wrap gap-1 mt-2">
                                {task.tags.iter().map(|tag| {
                                    view! {
                                        <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-2 py-0.5 rounded">
                                            {tag.clone()}
                                        </span>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }
                    })}
                </div>
            </div>

                // Accordion for subtasks - each subtask is an accordion item
                // Only ONE can be open at a time (no multiple=true)
                {move || {
                    let tasks_list = subtasks.get();
                    if !tasks_list.is_empty() {
                        // Pre-populate accordion with initial open item if provided
                        let open_items = RwSignal::new({
                            let mut set = std::collections::HashSet::new();
                            if let Some(id) = initial_open_subtask_id.clone() {
                                set.insert(id);
                            }
                            set
                        });

                        Some(view! {
                            <div class="mb-4">
                                <h3 class="text-sm font-semibold text-ctp-subtext0 mb-2">
                                    {format!("Subtasks ({})", tasks_list.len())}
                                </h3>
                                <Accordion open_items=open_items collapsible=true>
                                    {tasks_list.into_iter().map(|subtask| {
                                        let subtask_id = subtask.id.clone();
                                        view! {
                                            <AccordionItem value=subtask_id>
                                                <AccordionHeader slot>{subtask.content.clone()}</AccordionHeader>
                                                <div class="space-y-2 text-sm">
                                                    <div>
                                                        <span class="text-ctp-subtext0">"Status: "</span>
                                                        <span class="text-ctp-text">{format!("{:?}", subtask.status)}</span>
                                                    </div>
                                                    {subtask.priority.map(|p| {
                                                        view! {
                                                            <div>
                                                                <span class="text-ctp-subtext0">"Priority: "</span>
                                                                <span class="text-ctp-text">"P"{p}</span>
                                                            </div>
                                                        }
                                                    })}
                                                    {(!subtask.tags.is_empty()).then(|| {
                                                        view! {
                                                            <div class="flex flex-wrap gap-1">
                                                                {subtask.tags.iter().map(|tag| {
                                                                    view! {
                                                                        <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-2 py-0.5 rounded">
                                                                            {tag.clone()}
                                                                        </span>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}
                                                </div>
                                            </AccordionItem>
                                        }
                                    }).collect::<Vec<_>>()}
                                </Accordion>
                            </div>
                        })
                    } else {
                        None
                    }
                }}
        </div>
    }
}

/// TaskDetailDialog component - shows full task details in a centered modal dialog
#[component]
pub fn TaskDetailDialog(
    task: Task,
    open: RwSignal<bool>,
    #[prop(optional, default = None)] initial_open_subtask_id: Option<String>,
) -> impl IntoView {
    let task_id = task.id.clone();

    view! {
        <Dialog open>
            <DialogSurface class="max-w-3xl max-h-[80vh] overflow-hidden flex flex-col">
                <DialogBody class="flex flex-col overflow-hidden">
                    <DialogTitle class="flex items-center justify-between">
                        <span
                            class="text-xs text-ctp-overlay0 font-mono bg-ctp-surface0 px-2 py-1 rounded select-all cursor-pointer hover:bg-ctp-surface1"
                            title="Click to select, then copy"
                        >
                            {task_id}
                        </span>
                        <button
                            on:click=move |_| open.set(false)
                            class="text-ctp-overlay0 hover:text-ctp-text transition-colors text-xl ml-auto"
                        >
                            "✕"
                        </button>
                    </DialogTitle>
                    <DialogContent class="flex-1 overflow-y-auto">
                        {match initial_open_subtask_id {
                            Some(id) => view! {
                                <TaskDetailContent task=task initial_open_subtask_id=id />
                            }.into_any(),
                            None => view! {
                                <TaskDetailContent task=task />
                            }.into_any(),
                        }}
                    </DialogContent>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}

#[component]
pub fn TaskListCard(
    task_list: TaskList,
    #[prop(optional)] on_click: Option<Callback<String>>,
) -> impl IntoView {
    let list_id = task_list.id.clone();
    let list_id_for_stats = task_list.id.clone();
    let href = if on_click.is_some() {
        "#".to_string()
    } else {
        format!("/task-lists/{}", task_list.id)
    };

    // Fetch stats for this task list
    let (stats, set_stats) = signal(None::<Result<TaskStats, ApiClientError>>);

    Effect::new(move || {
        let id = list_id_for_stats.clone();
        spawn_local(async move {
            let result = task_lists::get_stats(&id).await;
            set_stats.set(Some(result));
        });
    });

    view! {
        <a
            href=href
            on:click=move |ev| {
                if let Some(callback) = on_click {
                    ev.prevent_default();
                    callback.run(list_id.clone());
                }
            }

            class="block bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors"
        >
            <div class="flex justify-between items-start mb-2">
                <h3 class="text-xl font-semibold text-ctp-text">{task_list.name.clone()}</h3>
                <span class="text-xs text-ctp-overlay0 ml-2 flex-shrink-0">{task_list.id.clone()}</span>
            </div>

            {task_list
                .description
                .as_ref()
                .map(|desc| {
                    view! { <p class="text-ctp-subtext0 text-sm mb-3">{desc.clone()}</p> }
                })}

            {(!task_list.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-2">
                            {task_list
                                .tags
                                .iter()
                                .map(|tag| {
                                    view! {
                                        <span class="bg-ctp-surface1 text-ctp-subtext1 text-xs px-2 py-1 rounded">
                                            {tag.clone()}
                                        </span>
                                    }
                                })
                                .collect::<Vec<_>>()}
                        </div>
                    }
                })}

            // Task stats badges - show all statuses (same order as kanban board)
            {move || {
                stats.get().and_then(|result| {
                    match result {
                        Ok(s) => {
                            Some(view! {
                                <div class="flex gap-2 flex-wrap mb-2">
                                    // Backlog
                                    <span class="bg-ctp-overlay0/20 text-ctp-overlay0 text-xs px-2 py-1 rounded">
                                        {s.backlog} " backlog"
                                    </span>

                                    // Todo
                                    <span class="bg-ctp-blue/20 text-ctp-blue text-xs px-2 py-1 rounded">
                                        {s.todo} " todo"
                                    </span>

                                    // In Progress
                                    <span class="bg-ctp-yellow/20 text-ctp-yellow text-xs px-2 py-1 rounded">
                                        {s.in_progress} " in progress"
                                    </span>

                                    // Review
                                    <span class="bg-ctp-mauve/20 text-ctp-mauve text-xs px-2 py-1 rounded">
                                        {s.review} " review"
                                    </span>

                                    // Done
                                    <span class="bg-ctp-green/20 text-ctp-green text-xs px-2 py-1 rounded">
                                        {s.done} " done"
                                    </span>

                                    // Cancelled
                                    <span class="bg-ctp-red/20 text-ctp-red text-xs px-2 py-1 rounded">
                                        {s.cancelled} " cancelled"
                                    </span>
                                </div>
                            })
                        },
                        Err(_) => None,
                    }
                })
            }}

            <div class="flex justify-between text-xs text-ctp-overlay0 mt-3">
                <span>"Created: " {task_list.created_at}</span>
                <span>"Updated: " {task_list.updated_at}</span>
            </div>
        </a>
    }
}

#[component]
pub fn TaskListDetailModal(
    task_list: ReadSignal<Option<TaskList>>,
    open: RwSignal<bool>,
) -> impl IntoView {
    let (stats_data, set_stats_data) = signal(None::<Result<TaskStats, ApiClientError>>);

    // Fetch stats when modal opens or task list changes
    Effect::new(move || {
        let list = task_list.get();
        let is_open = open.get();

        if let Some(tl) = list
            && is_open
        {
            let id = tl.id.clone();

            // Fetch stats only - columns will fetch their own tasks
            spawn_local(async move {
                let result = task_lists::get_stats(&id).await;
                set_stats_data.set(Some(result));
            });
        }
    });

    let statuses = vec![
        ("backlog", "Backlog"),
        ("todo", "Todo"),
        ("in_progress", "In Progress"),
        ("review", "Review"),
        ("done", "Done"),
        ("cancelled", "Cancelled"),
    ];

    view! {
        <OverlayDrawer
            open
            position=DrawerPosition::Right
            class="task-list-detail-drawer"
        >
            <DrawerBody>
                <Suspense fallback=move || {
                    view! { <p class="text-ctp-subtext0">"Loading task list..."</p> }
                }>
                    {move || {
                        let list = task_list.get();
                        let stats_result = stats_data.get();

                        match (list, stats_result) {
                            (Some(tl), Some(Ok(stats))) => {
                                view! {
                                    <div class="flex flex-col" style="height: calc(100vh - 4rem)">
                                        <div class="flex justify-between items-start mb-4 flex-shrink-0">
                                            <div class="flex-1">
                                                <div class="flex items-center gap-3 mb-1">
                                                    <h2 class="text-2xl font-bold text-ctp-text">
                                                        {tl.name.clone()}
                                                    </h2>
                                                    <span
                                                        class="text-ctp-overlay0 text-xs font-mono bg-ctp-surface0 px-2 py-1 rounded cursor-pointer hover:bg-ctp-surface1 select-all"
                                                        title="Click to select, then copy"
                                                    >
                                                        {tl.id.clone()}
                                                    </span>
                                                </div>
                                                {tl.description.as_ref().map(|desc| {
                                                    view! { <p class="text-ctp-subtext0 text-sm mt-1">{desc.clone()}</p> }
                                                })}

                                                {(!tl.tags.is_empty()).then(|| {
                                                    view! {
                                                        <div class="flex flex-wrap gap-2 mt-2">
                                                            {tl.tags.iter().map(|tag| {
                                                                view! {
                                                                    <span class="bg-ctp-surface1 text-ctp-subtext1 text-xs px-2 py-1 rounded">
                                                                        {tag.clone()}
                                                                    </span>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                            <button
                                                on:click=move |_| open.set(false)
                                                class="text-ctp-overlay0 hover:text-ctp-text text-2xl leading-none px-2"
                                            >
                                                "✕"
                                            </button>
                                        </div>

                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-4 flex-1 min-h-0">
                                            {statuses
                                                .clone()
                                                .into_iter()
                                                .map(|(status, label)| {
                                                    // Get total count for this status from stats
                                                    let total = match status {
                                                        "backlog" => stats.backlog,
                                                        "todo" => stats.todo,
                                                        "in_progress" => stats.in_progress,
                                                        "review" => stats.review,
                                                        "done" => stats.done,
                                                        "cancelled" => stats.cancelled,
                                                        _ => 0,
                                                    };

                                                    view! {
                                                        <KanbanColumn
                                                            status=status
                                                            label=label
                                                            list_id=tl.id.clone()
                                                            total_count=total
                                                        />
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </div>
                            }
                                .into_any()
                            }
                            (_, Some(Err(err))) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading stats"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                            _ => {
                                view! { <p class="text-ctp-subtext0">"Loading..."</p> }
                                    .into_any()
                            }
                        }
                    }}

                </Suspense>
            </DrawerBody>
        </OverlayDrawer>
    }
}
