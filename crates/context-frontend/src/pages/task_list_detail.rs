use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, task_lists};
use crate::components::TaskListContent;
use crate::models::{TaskList, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn TaskListDetail() -> impl IntoView {
    let params = use_params_map();

    let (task_list_data, set_task_list_data) = signal(None::<Result<TaskList, ApiClientError>>);

    // WebSocket updates - refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);
    let ws_updates = use_websocket_updates();

    // Watch for WebSocket task list updates
    Effect::new(move || {
        if let Some(UpdateMessage::TaskListUpdated { task_list_id }) = ws_updates.get() {
            let params = params.get();
            if let Some(current_id) = params.get("task_list_id")
                && current_id == task_list_id
            {
                web_sys::console::log_1(&"TaskList updated via WebSocket, refetching...".into());
                set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
            }
        }
    });

    // Fetch task list when params change or refetch trigger fires
    Effect::new(move || {
        let params = params.get();
        let _ = refetch_trigger.get(); // Track refetch trigger

        if let Some(task_list_id) = params.get("task_list_id") {
            spawn_local(async move {
                let result = task_lists::get(&task_list_id).await;
                set_task_list_data.set(Some(result));
            });
        }
    });

    view! {
        <div class="flex flex-col h-[calc(100vh-8rem)]">
            <div class="container mx-auto px-6 py-6 flex-1 h-full overflow-hidden">
                <Suspense fallback=move || {
                    view! { <p class="text-ctp-subtext0">"Loading task list..."</p> }
                }>
                    {move || {
                        match task_list_data.get() {
                            Some(Ok(task_list)) => {
                                let task_list_signal = Signal::derive(move || task_list.clone());
                                view! {
                                    <TaskListContent task_list=task_list_signal/>
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
