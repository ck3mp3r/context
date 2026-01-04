use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::*;

use crate::api::{ApiClientError, task_lists, tasks};
use crate::components::CopyableId;
use crate::models::{Task, TaskList, TaskStats};

// Helper functions for badge colors and labels (DRY)
fn priority_border_color(priority: Option<i32>) -> &'static str {
    match priority {
        Some(1) => "border-ctp-red",
        Some(2) => "border-ctp-peach",
        Some(3) => "border-ctp-yellow",
        Some(4) => "border-ctp-blue",
        Some(5) => "border-ctp-overlay0",
        _ => "border-ctp-surface1",
    }
}

fn priority_badge_color(priority: i32) -> &'static str {
    match priority {
        1 => "bg-ctp-red text-ctp-base",
        2 => "bg-ctp-peach text-ctp-base",
        3 => "bg-ctp-yellow text-ctp-base",
        4 => "bg-ctp-blue text-ctp-base",
        5 => "bg-ctp-overlay0 text-ctp-base",
        _ => "bg-ctp-surface1 text-ctp-text",
    }
}

fn status_badge_color(status: &str) -> &'static str {
    match status {
        "backlog" => "bg-ctp-overlay0/20 text-ctp-overlay0",
        "todo" => "bg-ctp-blue/20 text-ctp-blue",
        "in_progress" => "bg-ctp-yellow/20 text-ctp-yellow",
        "review" => "bg-ctp-mauve/20 text-ctp-mauve",
        "done" => "bg-ctp-green/20 text-ctp-green",
        "cancelled" => "bg-ctp-red/20 text-ctp-red",
        _ => "bg-ctp-surface1 text-ctp-text",
    }
}

fn status_badge_label(status: &str) -> String {
    match status {
        "in_progress" => "In Progress".to_string(),
        s => {
            let mut result = s.to_string();
            if let Some(first_char) = result.get_mut(0..1) {
                first_char.make_ascii_uppercase();
            }
            result
        }
    }
}

fn status_bg_color(status: &str) -> &'static str {
    match status {
        "backlog" => "bg-ctp-surface0",
        "todo" => "bg-ctp-blue/10",
        "in_progress" => "bg-ctp-yellow/10",
        "review" => "bg-ctp-mauve/10",
        "done" => "bg-ctp-green/10",
        "cancelled" => "bg-ctp-red/10",
        _ => "bg-ctp-surface0",
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

    // Task detail dialog state - store task ID only, not the whole object
    let (selected_task_id, set_selected_task_id) = signal(String::new());
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

    // WebSocket updates - refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);
    let ws_updates = crate::websocket::use_websocket_updates();

    // Watch for WebSocket task updates and trigger refetch
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            use crate::models::UpdateMessage;
            match update {
                UpdateMessage::TaskCreated { .. }
                | UpdateMessage::TaskUpdated { .. }
                | UpdateMessage::TaskDeleted { .. } => {
                    web_sys::console::log_1(
                        &format!(
                            "Task updated via WebSocket for status {}, refetching...",
                            status
                        )
                        .into(),
                    );
                    set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                _ => {} // Ignore non-task updates
            }
        }
    });

    // Determine sort order based on status
    let (sort_field, sort_order) = match status {
        "backlog" | "todo" => ("priority", "asc"), // Priority 1-5, nulls last
        "done" => ("completed_at", "desc"),        // Most recently completed first
        "cancelled" => ("updated_at", "desc"),     // Most recently updated first
        _ => ("updated_at", "desc"), // In progress, review: most recently updated first (parent updated_at cascades when subtask changes)
    };

    // Initial fetch + refetch on WebSocket updates
    Effect::new(move |_| {
        let _ = refetch_trigger.get(); // Track WebSocket refetch trigger
        let list_id = list_id_signal.get_value();
        set_offset.set(0); // Reset offset on refetch
        spawn_local(async move {
            let result = tasks::list_for_task_list(
                &list_id,
                Some(25),
                Some(0),
                Some(status),
                Some(sort_field),
                Some(sort_order),
                None,
                None, // Fetch all tasks - filter orphaned subtasks in UI (lines 136-160)
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
                None, // Fetch all tasks - filter orphaned subtasks in UI (lines 136-160)
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
                    {total_count}
                </span>
            </h3>
            <div
                node_ref=scroll_ref
                on:scroll=on_scroll
                class="space-y-2 overflow-y-auto flex-1 min-h-0"
            >
                {move || {
                    let all_tasks = tasks.get();

                    // Build set of parent IDs that exist in this column
                    let parent_ids: std::collections::HashSet<String> = all_tasks
                        .iter()
                        .filter(|t| t.parent_id.is_none())
                        .map(|p| p.id.clone())
                        .collect();

                    // Filter to parent tasks + orphaned subtasks
                    let display_tasks: Vec<_> = all_tasks
                        .into_iter()
                        .filter(|t| {
                            // Include if: parent task OR orphaned subtask
                            t.parent_id.is_none() || {
                                // Orphaned = has parent_id but parent not in this column
                                t.parent_id.as_ref().is_some_and(|pid| !parent_ids.contains(pid))
                            }
                        })
                        .collect();

                    // Track last parent_id to avoid duplicate mini parent cards
                    let last_parent_id = StoredValue::new(None::<String>);

                    view! {
                        <For
                            each=move || display_tasks.clone()
                            key=|task| task.id.clone()
                            children=move |task| {
                                let is_orphaned = task.parent_id.is_some();
                                let task_parent_id = task.parent_id.clone();
                                let (parent_task, set_parent_task) = signal(None::<Task>);

                                // Fetch parent task if this is orphaned and we haven't shown this parent yet
                                if is_orphaned && let Some(parent_id) = &task_parent_id {
                                    let current_last = last_parent_id.get_value();
                                    let is_new_parent = current_last.as_ref() != Some(parent_id);

                                    if is_new_parent {
                                        last_parent_id.set_value(Some(parent_id.clone()));
                                        let parent_id = parent_id.clone();

                                        spawn_local(async move {
                                            if let Ok(parent) = tasks::get(&parent_id).await {
                                                set_parent_task.set(Some(parent));
                                            }
                                        });
                                    }
                                }

                                view! {
                                    <div>
                                        {move || {
                                            parent_task.get().map(|parent| {
                                                view! {
                                                    <MiniParentCard
                                                        parent_task=parent.clone()
                                                        on_click=Callback::new(move |t: Task| {
                                                            set_selected_task_id.set(t.id.clone());
                                                            set_initial_open_subtask.set(None);
                                                            dialog_open.set(true);
                                                        })
                                                    />
                                                }
                                            })
                                        }}
                                        <div class={if is_orphaned { "ml-3 border-l-2 border-ctp-surface1 pl-2" } else { "" }}>
                                            <TaskCard
                                                task=task.clone()
                                                show_subtasks_inline=true
                                                on_click=Callback::new(move |t: Task| {
                                                    set_selected_task_id.set(t.id.clone());
                                                    set_initial_open_subtask.set(None);
                                                    dialog_open.set(true);
                                                })
                                                on_subtask_click=Callback::new(move |clicked_subtask: Task| {
                                                    if let Some(parent_id) = clicked_subtask.parent_id.clone() {
                                                        let clicked_id = clicked_subtask.id.clone();
                                                        set_selected_task_id.set(parent_id);
                                                        set_initial_open_subtask.set(Some(clicked_id));
                                                        dialog_open.set(true);
                                                    } else {
                                                        // Fallback: show subtask directly
                                                        set_selected_task_id.set(clicked_subtask.id.clone());
                                                        set_initial_open_subtask.set(None);
                                                        dialog_open.set(true);
                                                    }
                                                })
                                            />
                                        </div>
                                    </div>
                                }
                            }
                        />
                    }
                }}

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
            <TaskDetailDialog
                task_id=selected_task_id
                open=dialog_open
                initial_open_subtask_id=initial_open_subtask.into()
            />
        </div>
    }
}

/// SubtaskStackItem - Individual collapsible subtask in the stack
#[component]
pub fn SubtaskStackItem(
    subtask: Task,
    is_open: Signal<bool>,
    on_click: Callback<String>,
) -> impl IntoView {
    let subtask_id = subtask.id.clone();
    let subtask_id_for_click = subtask.id.clone();
    let border_color = priority_border_color(subtask.priority);
    let is_expanded = move || is_open.get();

    view! {
        <div
            class=format!(
                "relative mb-2 p-3 bg-ctp-surface0 rounded-lg border-l-4 {} cursor-pointer hover:bg-ctp-surface0/80 transition-colors",
                border_color
            )
            on:click=move |ev: ev::MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                on_click.run(subtask_id_for_click.clone());
            }
        >
            // Header: CopyableId + Title (left) | Badges (right: Priority → Status)
            <div class="flex items-center justify-between gap-4">
                // Left side: CopyableId + Title
                <div class="flex items-center gap-2 flex-1 min-w-0">
                    <div class="flex-shrink-0" on:click=|ev: ev::MouseEvent| {
                        ev.stop_propagation();
                    }>
                        <CopyableId id=subtask_id.clone()/>
                    </div>
                    <div class="flex-1 min-w-0 text-sm text-ctp-text break-words font-medium">
                        {move || {
                            let title = &subtask.title;
                            let truncated = !is_expanded() && title.len() > 60;
                            if truncated {
                                format!("{}...", &title[..60])
                            } else {
                                title.clone()
                            }
                        }}
                    </div>
                </div>

                // Right side: Priority → Status badges
                <div class="flex items-center gap-1 flex-shrink-0">
                    {subtask.priority.map(|p| {
                        view! {
                            <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", priority_badge_color(p))>
                                "P"{p}
                            </span>
                        }
                    })}
                    <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", status_badge_color(&subtask.status))>
                        {status_badge_label(&subtask.status)}
                    </span>
                </div>
            </div>

            // Description (only when expanded, with separator)
            {move || {
                (is_expanded() && subtask.description.is_some()).then(|| {
                    view! {
                        <div class="mt-2 pt-2 border-t border-ctp-surface1">
                            <div class="text-ctp-subtext0 text-xs prose prose-invert prose-xs max-w-none">
                                <crate::components::note_components::MarkdownContent content=subtask.description.clone().unwrap_or_default()/>
                            </div>
                        </div>
                    }
                })
            }}

            // Metadata section (only when expanded)
            {move || is_expanded().then(|| {
                view! {
                    <div class="pt-3 mt-3 border-t border-ctp-surface1">
                        // Timestamps (right-aligned)
                        <div class="flex flex-col items-end gap-1 text-xs text-ctp-overlay0 text-right mb-2">
                            <div>
                                <span class="text-ctp-overlay1">"Created: "</span>
                                <span>{subtask.created_at.clone()}</span>
                            </div>

                            {subtask.started_at.clone().map(|started| {
                                view! {
                                    <div>
                                        <span class="text-ctp-overlay1">"Started: "</span>
                                        <span>{started}</span>
                                    </div>
                                }
                            })}

                            {subtask.completed_at.clone().map(|completed| {
                                view! {
                                    <div>
                                        <span class="text-ctp-overlay1">"Completed: "</span>
                                        <span>{completed}</span>
                                    </div>
                                }
                            })}
                        </div>

                        // Tags (below, left-aligned)
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
                }
            })}
        </div>
    }
}

#[component]
pub fn TaskCard(
    task: Task,
    #[prop(optional, default = false)] show_subtasks_inline: bool,
    #[prop(optional)] on_click: Option<Callback<Task>>,
    #[prop(optional)] on_subtask_click: Option<Callback<Task>>,
    #[prop(optional, default = false)] show_status_badge: bool,
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
                None,           // task_type - we want subtasks here
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

    let task_id_for_list = task.id.clone();
    let list_id_for_list = task.list_id.clone();
    let task_status_for_list = task.status.to_string();

    let task_for_click = task.clone();
    let task_for_subtask_click = task.clone();
    let handle_card_click = move |_| {
        // If this is an orphaned subtask (has parent_id AND show_subtasks_inline is true),
        // use subtask callback behavior to fetch and open parent
        if task_for_click.parent_id.is_some() && show_subtasks_inline {
            if let Some(callback) = on_subtask_click {
                callback.run(task_for_subtask_click.clone());
            }
        } else {
            // Regular parent task OR inline subtask (where on_click IS the subtask callback)
            if let Some(callback) = on_click {
                callback.run(task_for_click.clone());
            }
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


                <div class="text-sm text-ctp-text mb-2">
                    <div class="font-medium break-words">
                        <span class="inline-block align-top mr-1">
                            <CopyableId id=task.id.clone()/>
                        </span>
                        {task.title.clone()}
                    </div>
                    {task.description.as_ref().map(|desc| {
                        // Truncate markdown before rendering to HTML
                        let preview_content = if desc.chars().count() > 100 {
                            let truncated: String = desc.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            desc.clone()
                        };

                        // Parse markdown to HTML for preview
                        use pulldown_cmark::{Options, Parser, html};
                        let mut options = Options::empty();
                        options.insert(Options::ENABLE_STRIKETHROUGH);
                        options.insert(Options::ENABLE_TABLES);

                        let parser = Parser::new_ext(&preview_content, options);
                        let mut html_output = String::new();
                        html::push_html(&mut html_output, parser);

                        view! { <div class="text-ctp-subtext0 text-xs mt-1" inner_html=html_output></div> }
                    })}
                </div>

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

                        // Show status badge for inline subtasks
                        {show_status_badge.then(|| {
                            view! {
                                <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", status_badge_color(&task.status))>
                                    {status_badge_label(&task.status)}
                                </span>
                            }
                        })}

                        {move || {
                            (subtask_count.get() > 0).then(|| {
                                view! {
                                    <span class="text-xs text-ctp-overlay1 bg-ctp-surface1 px-2 py-0.5 rounded">
                                        "📁 " {move || subtask_count.get()} " subtask" {move || if subtask_count.get() > 1 { "s" } else { "" }}
                                    </span>
                                }
                            })
                        }}
                    </div>
                </div>
            </div>

            {move || {
                (show_subtasks_inline && subtask_count.get() > 0).then(|| {
                    match on_subtask_click {
                        Some(callback) => view! {
                            <SubtaskList
                                task_id=task_id_for_list.clone()
                                list_id=list_id_for_list.clone()
                                parent_status=task_status_for_list.clone()
                                on_subtask_click=callback
                            />
                        }.into_any(),
                        None => view! {
                            <SubtaskList
                                task_id=task_id_for_list.clone()
                                list_id=list_id_for_list.clone()
                                parent_status=task_status_for_list.clone()
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
    #[prop(into)] parent_status: String,
    #[prop(optional)] on_subtask_click: Option<Callback<Task>>,
) -> impl IntoView {
    let (subtasks, set_subtasks) = signal(Vec::<Task>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);

    let task_id_for_fetch = task_id.clone();
    let list_id_for_fetch = list_id.clone();
    let parent_status_for_fetch = parent_status.clone();

    // Fetch subtasks on mount - only those matching parent's status
    Effect::new(move || {
        let task_id = task_id_for_fetch.clone();
        let list_id = list_id_for_fetch.clone();
        let parent_status = parent_status_for_fetch.clone();
        spawn_local(async move {
            match tasks::list_for_task_list(
                &list_id,
                None,                 // limit - get all matching subtasks
                None,                 // offset
                Some(&parent_status), // status - filter by parent's status
                Some("priority"),     // sort by priority
                Some("asc"),          // ascending
                Some(&task_id),       // parent_id filter
                None,                 // task_type - we want subtasks here
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
                    view! { <div></div> }.into_any()
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

/// MiniParentCard - compact one-liner parent task display for orphaned subtasks
#[component]
pub fn MiniParentCard(
    parent_task: Task,
    #[prop(optional)] on_click: Option<Callback<Task>>,
) -> impl IntoView {
    let parent_for_click = parent_task.clone();

    let handle_click = move |_| {
        if let Some(callback) = on_click {
            callback.run(parent_for_click.clone());
        }
    };

    let truncated_title = if parent_task.title.len() > 50 {
        format!("{}...", &parent_task.title[..50])
    } else {
        parent_task.title.clone()
    };

    let bg_color = status_bg_color(&parent_task.status.to_string());
    let priority_color = priority_border_color(parent_task.priority);

    view! {
        <div
            class=format!(
                "{} rounded px-2 py-1 mb-1 text-xs border-l-2 {} hover:bg-ctp-surface1 transition-colors cursor-pointer",
                bg_color,
                priority_color
            )
            on:click=handle_click
        >
            <div class="flex items-center gap-1">
                <CopyableId id=parent_task.id.clone() />
                <span class="text-ctp-text font-medium truncate">{truncated_title}</span>
            </div>
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
                None, // task_type - we want subtasks here
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

    // Determine priority color for left border (matching kanban cards)
    let priority_color = match task.priority {
        Some(1) => "border-l-ctp-red",      // P1 - Highest
        Some(2) => "border-l-ctp-peach",    // P2 - High
        Some(3) => "border-l-ctp-yellow",   // P3 - Medium
        Some(4) => "border-l-ctp-blue",     // P4 - Low
        Some(5) => "border-l-ctp-overlay0", // P5 - Lowest
        _ => "border-l-ctp-surface1",       // No priority
    };

    view! {
        <div>
            // Main task - title and description first, metadata secondary
            <div class=format!("mb-4 p-4 bg-ctp-surface0 rounded-lg border-l-4 {}", priority_color)>
                // CopyableId + Task title (left-aligned)
                <div class="flex items-start gap-2 mb-4 pb-4 border-b border-ctp-surface1">
                    <div class="flex-shrink-0">
                        <CopyableId id=task.id.clone()/>
                    </div>
                    <h2 class="flex-1 min-w-0 break-words text-xl font-semibold text-ctp-text">
                        {task.title.clone()}
                    </h2>
                </div>

                // Task description (if present)
                {task.description.as_ref().map(|desc| {
                    view! {
                        <div class="mb-4 prose prose-invert prose-sm max-w-none">
                            <crate::components::note_components::MarkdownContent content=desc.clone()/>
                        </div>
                    }
                })}

                // Metadata - compact and less prominent
                <div class="pt-3 border-t border-ctp-surface1">
                    // Top row: Badges (left) | Timestamps (right)
                    <div class="flex justify-between items-center gap-4 mb-3">
                        // LEFT: Badges section
                        <div class="flex items-center gap-2">
                            // Priority badge (if exists)
                            {task.priority.map(|p| {
                                view! {
                                    <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", priority_badge_color(p))>
                                        "P"{p}
                                    </span>
                                }
                            })}

                            // Status badge
                            <span class=format!("text-xs px-1.5 py-0.5 rounded font-medium {}", status_badge_color(&task.status))>
                                {status_badge_label(&task.status)}
                            </span>
                        </div>

                        // RIGHT: Timestamps section - right-aligned
                        <div class="flex flex-col items-end gap-1 text-xs text-ctp-overlay0 text-right">
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

                    // External reference link (if present)
                    {task.external_ref.as_ref().filter(|s| !s.is_empty()).map(|ext_ref| {
                        view! {
                            <div class="mt-2">
                                <ExternalRefLink external_ref=ext_ref.clone() />
                            </div>
                        }
                    })}
                </div>
            </div>

            // SubtaskStack - custom collapsible stack (no accordion arrow, no duplicate content)
            {move || {
                    let tasks_list = subtasks.get();
                    if !tasks_list.is_empty() {
                        // Track which subtask is currently open (only one at a time)
                        let open_subtask_id = RwSignal::new(initial_open_subtask_id.clone());

                        let handle_click = move |id: String| {
                            open_subtask_id.update(|current| {
                                if current.as_ref() == Some(&id) {
                                    *current = None; // Close if already open
                                } else {
                                    *current = Some(id); // Open clicked item
                                }
                            });
                        };

                        Some(view! {
                            <div class="mb-4">
                                <h3 class="text-sm font-semibold text-ctp-subtext0 mb-2">
                                    {format!("Subtasks ({})", tasks_list.len())}
                                </h3>
                                <div class="max-h-[40vh] overflow-y-auto">
                                    <For
                                        each=move || subtasks.get()
                                        key=|task| task.id.clone()
                                        children=move |subtask| {
                                            let subtask_id = subtask.id.clone();
                                            let is_open = Signal::derive(move || {
                                                open_subtask_id.get().as_ref() == Some(&subtask_id)
                                            });

                                            view! {
                                                <SubtaskStackItem
                                                    subtask=subtask
                                                    is_open=is_open
                                                    on_click=Callback::new(handle_click)
                                                />
                                            }
                                        }
                                    />
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
/// Fetches fresh data from API using task_id
#[component]
pub fn TaskDetailDialog(
    task_id: ReadSignal<String>,
    open: RwSignal<bool>,
    #[prop(optional, default = Signal::derive(|| None))] initial_open_subtask_id: Signal<
        Option<String>,
    >,
) -> impl IntoView {
    let task_resource = LocalResource::new(move || {
        let id = task_id.get();
        async move {
            if id.is_empty() {
                Err(crate::api::ApiClientError::Network(
                    "No task selected".to_string(),
                ))
            } else {
                tasks::get(&id).await
            }
        }
    });

    view! {
        <Dialog open=open>
            <DialogSurface class="max-w-3xl max-h-[60vh] overflow-hidden flex flex-col">
                <DialogBody class="flex flex-col overflow-hidden">
                    <DialogContent class="flex-1 overflow-y-auto">
                        <Suspense fallback=move || {
                            view! { <p class="text-ctp-subtext0">"Loading task..."</p> }
                        }>
                            {move || {
                                task_resource
                                    .get()
                                    .map(|result| {
                                        match result {
                                            Ok(task) => {
                                                match initial_open_subtask_id.get() {
                                                    Some(id) => view! {
                                                        <TaskDetailContent task=task initial_open_subtask_id=id />
                                                    }.into_any(),
                                                    None => view! {
                                                        <TaskDetailContent task=task />
                                                    }.into_any(),
                                                }
                                            }
                                            Err(e) => {
                                                view! {
                                                    <div class="p-4 text-ctp-red">
                                                        "Error loading task: " {e.to_string()}
                                                    </div>
                                                }.into_any()
                                            }
                                        }
                                    })
                            }}
                        </Suspense>
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
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[280px]">
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
                <div class="flex items-start gap-2 mb-2">
                    <div class="flex-shrink-0">
                        <CopyableId id=task_list.id.clone()/>
                    </div>
                    <h3 class="flex-1 min-w-0 break-words text-xl font-semibold text-ctp-text">{task_list.title.clone()}</h3>
                </div>

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

            // External reference link (if present)
            {task_list.external_ref.as_ref().filter(|s| !s.is_empty()).map(|ext_ref| {
                view! {
                    <div class="mb-3">
                        <ExternalRefLink external_ref=ext_ref.clone() />
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

    // WebSocket updates - refetch trigger for stats
    let (stats_refetch_trigger, set_stats_refetch_trigger) = signal(0u32);
    let ws_updates = crate::websocket::use_websocket_updates();

    // Watch for WebSocket task updates and trigger stats refetch
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            use crate::models::UpdateMessage;
            match update {
                UpdateMessage::TaskCreated { .. }
                | UpdateMessage::TaskUpdated { .. }
                | UpdateMessage::TaskDeleted { .. } => {
                    web_sys::console::log_1(
                        &"Task updated via WebSocket, refetching stats...".into(),
                    );
                    set_stats_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
                _ => {} // Ignore non-task updates
            }
        }
    });

    // Fetch stats when modal opens or task list changes or WebSocket updates
    Effect::new(move || {
        let list = task_list.get();
        let is_open = open.get();
        let _ = stats_refetch_trigger.get(); // Track WebSocket refetch trigger

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
                                                    <CopyableId id=tl.id.clone()/>
                                                    <h2 class="text-2xl font-bold text-ctp-text">
                                                        {tl.title.clone()}
                                                    </h2>
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

                                                // External reference link (if present)
                                                {tl.external_ref.as_ref().filter(|s| !s.is_empty()).map(|ext_ref| {
                                                    view! {
                                                        <div class="mt-2">
                                                            <ExternalRefLink external_ref=ext_ref.clone() />
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

/// ExternalRefLink component - renders external references as clickable links
/// Intelligently parses different formats:
/// - Full URLs: https://jira.company.com/browse/PROJ-123 → displays "PROJ-123", links to full URL
/// - GitHub issues: owner/repo#123 → displays "owner/repo#123", links to GitHub
/// - Short references: PROJ-123 → displays as non-clickable badge
#[component]
pub fn ExternalRefLink(external_ref: String) -> impl IntoView {
    // Parse the external_ref intelligently
    let (url, display_text, icon, label_class, is_link) =
        if external_ref.starts_with("http://") || external_ref.starts_with("https://") {
            // Full URL - extract ticket ID from last path segment
            let display = external_ref
                .split('/')
                .next_back()
                .unwrap_or(&external_ref)
                .to_string();
            (
                external_ref.clone(),
                display,
                "🔗",
                "bg-ctp-blue/10 text-ctp-blue border border-ctp-blue/30",
                true,
            )
        } else if external_ref.contains('/') && external_ref.contains('#') {
            // GitHub issue format: owner/repo#123
            let url = format!(
                "https://github.com/{}",
                external_ref.replace('#', "/issues/")
            );
            (
                url,
                external_ref.clone(),
                "🔗",
                "bg-ctp-blue/10 text-ctp-blue border border-ctp-blue/30",
                true,
            )
        } else {
            // Short reference: PROJ-123, etc. - not a clickable link
            (
                String::new(),
                external_ref.clone(),
                "📋",
                "bg-ctp-mauve/10 text-ctp-mauve border border-ctp-mauve/30",
                false,
            )
        };

    if is_link {
        view! {
            <a
                href=url
                target="_blank"
                rel="noopener noreferrer"
                on:click=|ev| {
                    ev.stop_propagation(); // Prevent parent link from triggering
                }
                class=format!("inline-flex items-center gap-1.5 text-xs px-2 py-1 rounded hover:opacity-80 transition-opacity {}", label_class)
            >
                <span>{icon}</span>
                <span class="font-medium">{display_text}</span>
                <span class="opacity-60">"↗"</span>
            </a>
        }.into_any()
    } else {
        // Non-clickable badge for short references
        view! {
            <span
                on:click=|ev| {
                    ev.stop_propagation(); // Prevent parent link from triggering
                }
                class=format!("inline-flex items-center gap-1.5 text-xs px-2 py-1 rounded {}", label_class)
                title="External reference"
            >
                <span>{icon}</span>
                <span class="font-medium">{display_text}</span>
            </span>
        }.into_any()
    }
}
