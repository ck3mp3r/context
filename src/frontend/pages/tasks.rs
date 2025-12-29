use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::{HashMap, HashSet};
use thaw::*;

use crate::api::{ApiClientError, task_lists, tasks};
use crate::models::{Paginated, Task, TaskList};

// Context for accordion state - shared across all swim lanes
#[derive(Clone, Copy)]
struct AccordionContext {
    expanded_id: ReadSignal<Option<String>>,
    set_expanded_id: WriteSignal<Option<String>>,
}

#[component]
pub fn Tasks() -> impl IntoView {
    // State
    let (selected_list_ids, set_selected_list_ids) = signal(HashSet::<String>::new());
    let show_archived = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let (is_search_focused, set_is_search_focused) = signal(false);
    let (expanded_swim_lane_id, set_expanded_swim_lane_id) = signal(None::<String>);
    let (task_lists_data, set_task_lists_data) =
        signal(None::<Result<Paginated<TaskList>, ApiClientError>>);
    let (swim_lane_tasks, set_swim_lane_tasks) =
        signal(HashMap::<String, Result<Vec<Task>, ApiClientError>>::new());

    // Provide accordion context to all child swim lanes
    let accordion_ctx = AccordionContext {
        expanded_id: expanded_swim_lane_id,
        set_expanded_id: set_expanded_swim_lane_id,
    };
    provide_context(accordion_ctx);

    // Fetch task lists on mount
    Effect::new(move || {
        spawn_local(async move {
            let result = task_lists::list(Some(200), None, None).await;
            set_task_lists_data.set(Some(result));
        });
    });

    // Fetch tasks for all selected lists
    Effect::new(move || {
        let list_ids = selected_list_ids.get();
        for list_id in list_ids.iter() {
            let list_id = list_id.clone();
            spawn_local(async move {
                let result = tasks::list_for_task_list(&list_id, Some(200), None).await;
                set_swim_lane_tasks.update(|map| {
                    map.insert(list_id.clone(), result.map(|paginated| paginated.items));
                });
            });
        }
    });

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Tasks"</h2>

                // Show Archived Toggle with Thaw Checkbox
                <label class="flex items-center gap-2 cursor-pointer">
                    <Checkbox checked=show_archived/>
                    <span class="text-sm text-ctp-subtext0">"Show Archived"</span>
                </label>
            </div>

            // Task List Multi-Selector with Search
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
                                    let all_lists = paginated.items.clone();
                                    let all_lists_for_chips = all_lists.clone();
                                    let all_lists_for_search = all_lists.clone();
                                    view! {
                                        <div class="relative">
                                            // Custom search input (Thaw styling incompatible with our design)
                                            <input
                                                type="text"
                                                placeholder="Search task lists to add swim lanes..."
                                                prop:value=move || search_query.get()
                                                on:input=move |ev| {
                                                    search_query.set(event_target_value(&ev));
                                                }
                                                on:focus=move |_| set_is_search_focused.set(true)
                                                on:blur=move |_| set_is_search_focused.set(false)
                                                class="w-full px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text focus:outline-none focus:border-ctp-blue"
                                            />

                                            // Selected Lists Display (Chips)
                                            {move || {
                                                let selected = selected_list_ids.get();
                                                if !selected.is_empty() {
                                                    let selected_lists: Vec<TaskList> = all_lists_for_chips
                                                        .iter()
                                                        .filter(|list| selected.contains(&list.id))
                                                        .cloned()
                                                        .collect();
                                                    Some(
                                                        view! {
                                                            <div class="mt-2 flex flex-wrap gap-2">
                                                                {selected_lists
                                                                    .iter()
                                                                    .map(|list| {
                                                                        let list_id = list.id.clone();
                                                                        let list_name = list.name.clone();
                                                                        view! {
                                                                            <div class="flex items-center gap-2 px-3 py-1 bg-ctp-blue/20 text-ctp-blue rounded-full text-sm font-medium">
                                                                                <span>{list_name.clone()}</span>
                                                                                <button
                                                                                    on:click=move |_| {
                                                                                        set_selected_list_ids
                                                                                            .update(|ids| {
                                                                                                ids.remove(&list_id);
                                                                                            });
                                                                                    }

                                                                                    class="hover:text-ctp-red"
                                                                                >
                                                                                    "×"
                                                                                </button>
                                                                            </div>
                                                                        }
                                                                    })
                                                                    .collect::<Vec<_>>()}

                                                                <button
                                                                    on:click=move |_| {
                                                                        set_selected_list_ids.set(HashSet::new());
                                                                    }

                                                                    class="text-ctp-red hover:text-ctp-maroon text-sm px-2"
                                                                >
                                                                    "Clear All"
                                                                </button>
                                                            </div>
                                                        }
                                                    )
                                                } else {
                                                    None
                                                }
                                            }}

                                            // Filtered Results Dropdown with Checkboxes
                                            {move || {
                                                let query = search_query.get();
                                                let show_archived_val = show_archived.get();
                                                let focused = is_search_focused.get();
                                                if query.is_empty() || !focused {
                                                    return view! { <div></div> }.into_any();
                                                }
                                                let filtered: Vec<TaskList> = all_lists_for_search
                                                    .iter()
                                                    .filter(|list| {
                                                        let matches_query = list
                                                            .name
                                                            .to_lowercase()
                                                            .contains(&query.to_lowercase());
                                                        let matches_status = show_archived_val
                                                            || list.status != "archived";
                                                        matches_query && matches_status
                                                    })
                                                    .cloned()
                                                    .collect();
                                                if filtered.is_empty() {
                                                    view! {
                                                        <div class="absolute z-10 w-full mt-1 bg-ctp-surface0 border border-ctp-surface1 rounded-lg shadow-lg p-4">
                                                            <p class="text-ctp-subtext0 text-sm">"No matching task lists"</p>
                                                        </div>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! {
                                                        <div
                                                            class="absolute z-10 w-full mt-1 bg-ctp-surface0 border border-ctp-surface1 rounded-lg shadow-lg max-h-[400px] overflow-y-auto"
                                                            on:mousedown=move |ev| ev.prevent_default()
                                                        >
                                                            {filtered
                                                                .iter()
                                                                .map(|list| {
                                                                    let list_id = list.id.clone();
                                                                    let list_id_for_checked = list_id.clone();
                                                                    let list_id_for_change = list_id.clone();
                                                                    let _list_name = list.name.clone();
                                                                    let is_archived = list.status == "archived";
                                                                    view! {
                                                                        <label class="flex items-center gap-3 px-4 py-2 hover:bg-ctp-surface1 cursor-pointer transition-colors">
                                                                            <input
                                                                                type="checkbox"
                                                                                prop:checked=move || {
                                                                                    selected_list_ids.get().contains(&list_id_for_checked)
                                                                                }

                                                                                on:change=move |_| {
                                                                                    set_selected_list_ids
                                                                                        .update(|ids| {
                                                                                            if ids.contains(&list_id_for_change) {
                                                                                                ids.remove(&list_id_for_change);
                                                                                            } else {
                                                                                                ids.insert(list_id_for_change.clone());
                                                                                            }
                                                                                        });
                                                                                }

                                                                                class="w-4 h-4 text-ctp-blue bg-ctp-base border-ctp-surface1 rounded"
                                                                            />
                                                                            <div class="flex-1">
                                                                                <div class="font-medium text-ctp-text flex items-center gap-2">
                                                                                    <span>{list.name.clone()}</span>
                                                                                    {is_archived
                                                                                        .then(|| {
                                                                                            view! {
                                                                                                <span class="text-xs px-2 py-0.5 bg-ctp-overlay0/20 text-ctp-overlay0 rounded">
                                                                                                    "Archived"
                                                                                                </span>
                                                                                            }
                                                                                        })}

                                                                                </div>
                                                                                {list
                                                                                    .description
                                                                                    .as_ref()
                                                                                    .map(|desc| {
                                                                                        view! {
                                                                                            <div class="text-sm text-ctp-subtext0 truncate">
                                                                                                {desc.clone()}
                                                                                            </div>
                                                                                        }
                                                                                    })}

                                                                            </div>
                                                                        </label>
                                                                    }
                                                                })
                                                                .collect::<Vec<_>>()}
                                                        </div>
                                                    }
                                                        .into_any()
                                                }
                                            }}

                                        </div>
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

            // Swim Lanes (Each selected task list becomes a row)
            {move || {
                let selected = selected_list_ids.get();
                if selected.is_empty() {
                    return view! {
                        <p class="text-ctp-subtext0 text-center py-12">
                            "Search and select task lists above to add swim lanes"
                        </p>
                    }
                        .into_any();
                }
                match task_lists_data.get() {
                    None => view! { <p class="text-ctp-subtext0">"Loading..."</p> }.into_any(),
                    Some(Ok(paginated)) => {
                        let selected_lists: Vec<TaskList> = paginated
                            .items
                            .iter()
                            .filter(|list| selected.contains(&list.id))
                            .cloned()
                            .collect();
                        view! {
                            <Accordion>
                                {selected_lists
                                    .into_iter()
                                    .map(|task_list| {
                                        view! {
                                            <SwimLane task_list=task_list tasks_map=swim_lane_tasks/>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </Accordion>
                        }
                            .into_any()
                    }
                    Some(Err(err)) => {
                        view! {
                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                <p class="text-ctp-red font-semibold">"Error loading task lists"</p>
                                <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
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
fn SwimLane(
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
                                            view! { <KanbanColumn status=status label=label tasks=column_tasks/> }
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
