use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::*;

use crate::api::{ApiClientError, task_lists, tasks};
use crate::components::CopyableId;
use crate::models::{Task, TaskList, TaskStats};

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
                    .filter(|task| task.parent_id.is_none())
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
            <div class=format!("relative mb-4 p-4 bg-ctp-surface0 rounded-lg border-l-4 {}", status_color)>
                // Task ID in top-right corner
                <div class="absolute top-2 right-2">
                    <CopyableId id=task.id.clone()/>
                </div>
                // Task description - prominent
                <div class="text-base text-ctp-text whitespace-pre-wrap break-words mb-4 pr-20">
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
                                <div class="max-h-[40vh] overflow-y-auto">
                                <Accordion open_items=open_items collapsible=true>
                                    {tasks_list.into_iter().map(|subtask| {
                                        let subtask_id = subtask.id.clone();
                                        let subtask_content_header = subtask.content.clone();
                                        let subtask_content_body = subtask.content.clone();
                                        let subtask_status = subtask.status.clone();
                                        view! {
                                            <AccordionItem value=subtask_id.clone()>
                                                <AccordionHeader slot>
                                                    <div class="flex items-center gap-2 w-full">
                                                        <span class="flex-1 truncate text-sm">
                                                            {
                                                                if subtask_content_header.len() > 60 {
                                                                    format!("{}...", &subtask_content_header[..60])
                                                                } else {
                                                                    subtask_content_header
                                                                }
                                                            }
                                                        </span>
                                                        <div class="flex items-center gap-1 flex-shrink-0">
                                                            {subtask.priority.map(|p| {
                                                                let priority_color = match p {
                                                                    1 => "bg-ctp-red text-ctp-base",
                                                                    2 => "bg-ctp-peach text-ctp-base",
                                                                    3 => "bg-ctp-yellow text-ctp-base",
                                                                    4 => "bg-ctp-blue text-ctp-base",
                                                                    5 => "bg-ctp-overlay0 text-ctp-base",
                                                                    _ => "bg-ctp-surface1 text-ctp-text",
                                                                };
                                                                view! {
                                                                    <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", priority_color)>
                                                                        "P"{p}
                                                                    </span>
                                                                }
                                                            })}
                                                            {
                                                                let status_color = match subtask_status.as_str() {
                                                                    "backlog" => "bg-ctp-overlay0/20 text-ctp-overlay0",
                                                                    "todo" => "bg-ctp-blue/20 text-ctp-blue",
                                                                    "in_progress" => "bg-ctp-yellow/20 text-ctp-yellow",
                                                                    "review" => "bg-ctp-mauve/20 text-ctp-mauve",
                                                                    "done" => "bg-ctp-green/20 text-ctp-green",
                                                                    "cancelled" => "bg-ctp-red/20 text-ctp-red",
                                                                    _ => "bg-ctp-surface1 text-ctp-text",
                                                                };
                                                                let status_label = match subtask_status.as_str() {
                                                                    "in_progress" => "In Progress".to_string(),
                                                                    status => {
                                                                        let mut s = status.to_string();
                                                                        if let Some(first_char) = s.get_mut(0..1) {
                                                                            first_char.make_ascii_uppercase();
                                                                        }
                                                                        s
                                                                    }
                                                                };
                                                                view! {
                                                                    <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", status_color)>
                                                                        {status_label}
                                                                    </span>
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                </AccordionHeader>
                                                <div class="relative space-y-2 text-sm">
                                                    <div class="absolute top-0 right-0">
                                                        <CopyableId id=subtask_id.clone()/>
                                                    </div>
                                                    <div class="text-ctp-text break-words pr-20">
                                                        {subtask_content_body}
                                                    </div>
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
    view! {
        <Dialog open=open>
            <DialogSurface class="max-w-3xl max-h-[60vh] overflow-hidden flex flex-col">
                <DialogBody class="flex flex-col overflow-hidden">
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
        <div class="relative bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[280px]">
            <div class="absolute top-2 right-2">
                <CopyableId id=task_list.id.clone()/>
            </div>
            <a
                href=href
                on:click=move |ev| {
                    if let Some(callback) = on_click {
                        ev.prevent_default();
                        callback.run(list_id.clone());
                    }
                }

                class="flex flex-col h-full"
            >
                <h3 class="text-xl font-semibold text-ctp-text mb-2 pr-20">{task_list.name.clone()}</h3>

            {task_list
                .description
                .as_ref()
                .map(|desc| {
                    view! { <p class="text-ctp-subtext0 text-sm mb-3">{desc.clone()}</p> }
                })}

            <div class="flex-grow"></div>

            <div class="mt-auto">
            {(!task_list.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-3">
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

            // Task stats badges - compact with icons, always show all statuses
            {move || {
                stats.get().and_then(|result| {
                    match result {
                        Ok(s) => {
                            Some(view! {
                                <div class="flex gap-1.5 flex-wrap text-xs mb-3">
                                    // Backlog
                                    <Tooltip content="Backlog">
                                        <span class="bg-ctp-overlay0/20 text-ctp-overlay0 px-2 py-0.5 rounded">
                                            "📦 " {s.backlog}
                                        </span>
                                    </Tooltip>

                                    // Todo
                                    <Tooltip content="Todo">
                                        <span class="bg-ctp-blue/20 text-ctp-blue px-2 py-0.5 rounded">
                                            "📋 " {s.todo}
                                        </span>
                                    </Tooltip>

                                    // In Progress
                                    <Tooltip content="In Progress">
                                        <span class="bg-ctp-yellow/20 text-ctp-yellow px-2 py-0.5 rounded">
                                            "⚙️ " {s.in_progress}
                                        </span>
                                    </Tooltip>

                                    // Review
                                    <Tooltip content="Review">
                                        <span class="bg-ctp-mauve/20 text-ctp-mauve px-2 py-0.5 rounded">
                                            "👀 " {s.review}
                                        </span>
                                    </Tooltip>

                                    // Done
                                    <Tooltip content="Done">
                                        <span class="bg-ctp-green/20 text-ctp-green px-2 py-0.5 rounded">
                                            "✓ " {s.done}
                                        </span>
                                    </Tooltip>

                                    // Cancelled
                                    <Tooltip content="Cancelled">
                                        <span class="bg-ctp-red/20 text-ctp-red px-2 py-0.5 rounded">
                                            "✗ " {s.cancelled}
                                        </span>
                                    </Tooltip>
                                </div>
                            })
                        },
                        Err(_) => None,
                    }
                })
            }}

            <div class="flex justify-between text-xs text-ctp-overlay0">
                <span>"Created: " {task_list.created_at}</span>
                <span>"Updated: " {task_list.updated_at}</span>
            </div>
            </div>
            </a>
        </div>
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
                                                    <CopyableId id=tl.id.clone()/>
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
