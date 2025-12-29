use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::ApiClientError;
use crate::api::notes;
use crate::components::{NoteCard, NoteDetailModal, Pagination};
use crate::models::{Note, Paginated};

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
    let (search_query, set_search_query) = signal(String::new());
    let (notes_data, set_notes_data) = signal(None::<Result<Paginated<Note>, ApiClientError>>);

    // Note detail modal state
    let note_modal_open = RwSignal::new(false);
    let selected_note_id = RwSignal::new(String::new());

    // Use Effect to fetch when dependencies change
    Effect::new(move || {
        let current_page = page.get();
        let current_query = search_query.get();

        // Log for debugging
        web_sys::console::log_1(
            &format!(
                "Fetching page {} with query '{}'",
                current_page, current_query
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

            let result = notes::list(Some(PAGE_SIZE), Some(offset), query_opt, None).await;

            match &result {
                Ok(data) => web_sys::console::log_1(
                    &format!("Got {} notes, total {}", data.items.len(), data.total).into(),
                ),
                Err(e) => web_sys::console::log_1(&format!("Error: {}", e).into()),
            }

            set_notes_data.set(Some(result));
        });
    });

    // Handle search input change
    let on_search = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        set_search_query.set(value);
        set_page.set(0); // Reset to first page on new search
    };

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
                    placeholder="Search notes (FTS5)..."
                    value=move || search_query.get()
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
                                                                on_click=Callback::new(move |note_id: String| {
                                                                    selected_note_id.set(note_id);
                                                                    note_modal_open.set(true);
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
                        />
                    })
                } else {
                    None
                }
            }}

        </div>
    }
}
