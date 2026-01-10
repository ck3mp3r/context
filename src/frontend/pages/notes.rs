use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::ApiClientError;
use crate::api::notes;
use crate::components::{NoteCard, NoteDetailModal, Pagination};
use crate::models::{Note, Paginated, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn Notes() -> impl IntoView {
    view! {
        <NotesList/>
    }
}

#[component]
fn NotesList() -> impl IntoView {
    const PAGE_SIZE: usize = 12;

    // State management
    let (page, set_page) = signal(0usize);
    let (search_input, set_search_input) = signal(String::new()); // Raw input
    let (search_query, set_search_query) = signal(String::new()); // Debounced search
    let (notes_data, set_notes_data) = signal(None::<Result<Paginated<Note>, ApiClientError>>);

    // Note detail modal state
    let note_modal_open = RwSignal::new(false);
    let selected_note_id = RwSignal::new(String::new());
    let selected_note_has_subnotes = RwSignal::new(false);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Trigger to force refetch (increments when we need to refresh)
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates and trigger refetch when notes change
    Effect::new(move || {
        if let Some(
            UpdateMessage::NoteCreated { .. }
            | UpdateMessage::NoteUpdated { .. }
            | UpdateMessage::NoteDeleted { .. },
        ) = ws_updates.get()
        {
            web_sys::console::log_1(&"Note updated via WebSocket, refetching...".into());
            // Trigger refetch by incrementing counter
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Store the timeout ID so we can cancel it
    let debounce_timeout = RwSignal::new(None::<i32>);

    // Handle search input change with proper debouncing
    let on_search = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_search_input.set(value.clone());

        use wasm_bindgen::JsCast;
        use wasm_bindgen::prelude::*;

        // Cancel the previous timeout if it exists
        if let Some(timeout_id) = debounce_timeout.get() {
            web_sys::window()
                .unwrap()
                .clear_timeout_with_handle(timeout_id);
        }

        // Set new timeout
        let callback = Closure::once(move || {
            set_search_query.set(value.clone());
            set_page.set(0); // Reset to first page on new search
            debounce_timeout.set(None); // Clear timeout ID after it fires
        });

        let timeout_id = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                500,
            )
            .unwrap();

        debounce_timeout.set(Some(timeout_id));
        callback.forget();
    };

    // Use Effect to fetch when dependencies change (including WebSocket updates)
    Effect::new(move || {
        let current_page = page.get();
        let current_query = search_query.get();
        let trigger = refetch_trigger.get();

        // Log for debugging with all dependency values
        web_sys::console::log_1(
            &format!(
                "FETCH TRIGGERED: page={}, query='{}', trigger={}",
                current_page, current_query, trigger
            )
            .into(),
        );

        // Reset to loading state immediately
        set_notes_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;
            let query_opt = if current_query.trim().is_empty() {
                None
            } else {
                Some(current_query)
            };

            web_sys::console::log_1(
                &format!("API call: offset={}, limit={}", offset, PAGE_SIZE).into(),
            );

            let result = notes::list(
                Some(PAGE_SIZE),
                Some(offset),
                query_opt,
                None,
                Some("note"),
                None,
            )
            .await;

            match &result {
                Ok(data) => web_sys::console::log_1(
                    &format!("Got {} notes, total {}", data.items.len(), data.total).into(),
                ),
                Err(e) => web_sys::console::log_1(&format!("Error: {}", e).into()),
            }

            set_notes_data.set(Some(result));
        });
    });

    // Pagination handlers
    let go_to_page = move |new_page: usize| {
        set_page.set(new_page);
    };

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Notes"</h2>
            </div>

            // Search bar
            <div class="mb-6">
                <input
                    type="text"
                    placeholder="Search notes..."
                    value=move || search_input.get()
                    on:input=on_search
                    class="w-full px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded-lg text-ctp-text placeholder-ctp-overlay0 focus:outline-none focus:border-ctp-blue"
                />
            </div>

            {move || {
                match notes_data.get() {
                    None => view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }.into_any(),
                    Some(result) => {
                                        match result {
                            Ok(paginated) => {
                                let total_pages = paginated.total.div_ceil(PAGE_SIZE);

                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            {if search_query.get().trim().is_empty() {
                                                "No notes found. Create one to get started!"
                                            } else {
                                                "No notes found matching your search."
                                            }}
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div>
                                            // Notes grid
                                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
                                                {paginated
                                                    .items
                                                    .iter()
                                                    .map(|note| {
                                                        view! {
                                                            <NoteCard
                                                                note=note.clone()
                                                                on_click=Callback::new({
                                                                    let has_subs = note.subnote_count.unwrap_or(0) > 0;
                                                                    move |note_id: String| {
                                                                        selected_note_id.set(note_id);
                                                                        selected_note_has_subnotes.set(has_subs);
                                                                        note_modal_open.set(true);
                                                                    }
                                                                })
                                                            />
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>

                                            // Pagination
                                            <Pagination
                                                current_page=page
                                                total_pages=total_pages
                                                on_prev=Callback::new(move |_| {
                                                    let current = page.get();
                                                    if current > 0 {
                                                        go_to_page(current - 1);
                                                    }
                                                })
                                                on_next=Callback::new(move |_| {
                                                    let current = page.get();
                                                    if current < total_pages - 1 {
                                                        go_to_page(current + 1);
                                                    }
                                                })
                                                show_summary=true
                                                total_items=paginated.total
                                                page_size=PAGE_SIZE
                                                item_name="notes".to_string()
                                            />
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(err) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading notes"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    }
                }
            }}

            // Note detail modal - only render when open
            {move || {
                if note_modal_open.get() {
                    Some(view! {
                        <NoteDetailModal
                            note_id=selected_note_id.read_only()
                            open=note_modal_open
                            has_subnotes=selected_note_has_subnotes.get()
                        />
                    })
                } else {
                    None
                }
            }}

        </div>
    }
}
