use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, QueryBuilder, notes};
use crate::components::{EntityHeader, MarkdownContent, NoteStackSidebar};
use crate::models::{Note, UpdateMessage};
use crate::tabs::{TabScope, should_refetch};
use crate::websocket::use_websocket_updates;

#[component]
pub fn NoteDetail(scope: TabScope) -> impl IntoView {
    // The tab's entity id never changes for the tab's lifetime.
    let note_id = StoredValue::new_local(scope.entity_id().to_string());

    let (note_data, set_note_data) = signal(None::<Result<Note, ApiClientError>>);
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Fetch subnotes to determine if we should show sidebar
    let (subnotes_data, set_subnotes_data) = signal(None::<Result<Vec<Note>, ApiClientError>>);
    let (subnotes_refetch_trigger, _set_subnotes_refetch_trigger) = signal(0u32);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Watch for WebSocket updates for THIS note. The tab's own fetch runs at mount, so loaded data
    // is the gate: a hidden tab keeps its own note live, and another note's message refetches
    // nothing (decision 15).
    Effect::new(move || {
        let Some(update) = ws_updates.get() else {
            return;
        };
        let current_note_id = note_id.get_value();
        if current_note_id.is_empty() {
            return;
        }
        match update {
            UpdateMessage::NoteUpdated {
                note_id: updated_id,
            } if should_refetch(
                note_data.get_untracked().is_some(),
                false,
                updated_id == current_note_id,
            ) =>
            {
                web_sys::console::log_1(
                    &format!("Note {} updated via WebSocket, refetching...", updated_id).into(),
                );
                set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
            }
            UpdateMessage::NoteDeleted {
                note_id: updated_id,
            } if updated_id == current_note_id => {
                // No route exists for this surface any more, and closing the tab is the
                // deletion reaction's job: this handler writes nothing.
                web_sys::console::log_1(
                    &format!("Note {} deleted via WebSocket, tab stays open", updated_id).into(),
                );
            }
            _ => {}
        }
    });

    // Fetch note when ID or refetch trigger changes
    Effect::new(move || {
        let id = note_id.get_value();
        let _ = refetch_trigger.get();

        if id.is_empty() {
            set_note_data.set(Some(Err(ApiClientError::Network(
                "No note ID provided".to_string(),
            ))));
            return;
        }

        set_note_data.set(None); // Loading state

        spawn_local(async move {
            let result = notes::get(&id).await;
            set_note_data.set(Some(result));
        });
    });

    // Fetch subnotes to check if this note has any
    Effect::new(move || {
        let id = note_id.get_value();
        let _ = subnotes_refetch_trigger.get();

        if id.is_empty() {
            return;
        }

        spawn_local(async move {
            let result = QueryBuilder::<Note>::new()
                .limit(1)
                .offset(0)
                .param("type", "subnote")
                .param("parent_id", &id)
                .fetch()
                .await;

            match result {
                Ok(paginated) => {
                    set_subnotes_data.set(Some(Ok(paginated.items)));
                }
                Err(e) => {
                    set_subnotes_data.set(Some(Err(e)));
                }
            }
        });
    });

    // Fetch all subnotes for printing (notestack layout)
    let (print_notes, set_print_notes) = signal(Vec::<Note>::new());

    Effect::new(move || {
        let id = note_id.get_value();
        if id.is_empty() {
            return;
        }

        spawn_local(async move {
            let mut all = vec![];
            let mut offset = 0;
            loop {
                let result = QueryBuilder::<Note>::new()
                    .limit(20)
                    .offset(offset)
                    .param("type", "subnote")
                    .param("parent_id", &id)
                    .param("include_content", "true")
                    .fetch()
                    .await;
                match result {
                    Ok(paginated) => {
                        let count = paginated.items.len();
                        all.extend(paginated.items);
                        if count < 20 {
                            break;
                        }
                        offset += 20;
                    }
                    Err(_) => break,
                }
            }
            set_print_notes.set(all);
        });
    });

    // State for selected note in stack (for notes with subnotes)
    // Initialize with parent note ID
    let selected_note_id = RwSignal::new(String::new());

    // Initialize selected_note_id once parent note loads
    Effect::new(move || {
        if let Some(Ok(note)) = note_data.get()
            && selected_note_id.get().is_empty()
        {
            selected_note_id.set(note.id.clone());
        }
    });

    let (selected_refetch_trigger, set_selected_refetch_trigger) = signal(0u32);

    // Resource for fetching the selected note (for stack navigation)
    let (selected_note_data, set_selected_note_data) = signal(None::<Result<Note, ApiClientError>>);

    // Callback for note selection from sidebar
    let on_note_select = Callback::new(move |id: String| {
        selected_note_id.set(id);
    });

    // Watch for WebSocket updates for the SELECTED note, gated the same way as the note itself.
    Effect::new(move || {
        let Some(update) = ws_updates.get() else {
            return;
        };
        let current_selected = selected_note_id.get_untracked();
        let parent_note_id = note_id.get_value();
        if current_selected.is_empty() {
            return;
        }
        match update {
            UpdateMessage::NoteUpdated {
                note_id: updated_id,
            } if should_refetch(
                selected_note_data.get_untracked().is_some(),
                false,
                updated_id == current_selected,
            ) =>
            {
                web_sys::console::log_1(
                    &format!(
                        "Selected note {} updated via WebSocket, refetching...",
                        updated_id
                    )
                    .into(),
                );
                set_selected_refetch_trigger.update(|n| *n = n.wrapping_add(1));
            }
            UpdateMessage::NoteDeleted {
                note_id: updated_id,
            } if updated_id == current_selected && current_selected != parent_note_id => {
                // Selected subnote was deleted - switch back to parent
                web_sys::console::log_1(
                    &format!(
                        "Selected note {} deleted via WebSocket, switching to parent...",
                        updated_id
                    )
                    .into(),
                );
                selected_note_id.set(parent_note_id);
            }
            _ => {}
        }
    });

    Effect::new(move || {
        let id = selected_note_id.get();
        let _ = selected_refetch_trigger.get();

        if id.is_empty() {
            return;
        }

        set_selected_note_data.set(None);

        spawn_local(async move {
            let result = notes::get(&id).await;
            set_selected_note_data.set(Some(result));
        });
    });

    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]">
            <div class="container mx-auto p-6 flex-1">
                {move || {
                    match note_data.get() {
                        None => view! { <p class="text-ctp-subtext0">"Loading note..."</p> }.into_any(),
                        Some(result) => {
                            match result {
                                Ok(note) => {
                                    // Check if we have subnotes from the fetch
                                    let has_subs = subnotes_data.get()
                                        .and_then(|r| r.ok())
                                        .map(|items| !items.is_empty())
                                        .unwrap_or(false);

                                    web_sys::console::log_1(&format!("Note {} has_subnotes: {}", note.id, has_subs).into());

                                    if has_subs {
                                        web_sys::console::log_1(&"Rendering split-panel layout with sidebar".into());
                                        // Split-panel layout for note stacks
                                        let parent_note_for_sidebar = note.clone();

                                        view! {
                                            <div class="flex gap-6 h-[calc(100vh-12rem)] screen-only">
                                                // Left sidebar - note stack
                                                <div class="border-r border-ctp-surface1 flex-shrink-0 flex flex-col overflow-hidden" style="width: 190px;">
                                                    <div class="overflow-y-auto flex-1 min-h-0">
                                                        <NoteStackSidebar
                                                            parent_note=parent_note_for_sidebar
                                                            on_note_select=on_note_select
                                                        />
                                                    </div>
                                                </div>

                                                // Right side - selected note content
                                                <div class="flex-1 flex flex-col min-h-0 overflow-hidden">
                                                    {move || {
                                                        match selected_note_data.get() {
                                                            None => view! { <p class="text-ctp-subtext0">"Loading..."</p> }.into_any(),
                                                            Some(result) => {
                                                                match result {
                                                                    Ok(selected_note) => {
                                                                        view! {
                                                                            <div class="flex flex-col h-full note-print-area">
                                                                                // Header: title, tags, metadata
                                                                                <div class="flex-shrink-0 pb-4 border-b border-ctp-surface1">
                                                                                    <div class="flex items-center gap-3 mb-4">
                                                                                        <EntityHeader id=selected_note.id.clone() title=selected_note.title.clone()/>
                                                                                        <div class="ml-auto">
                                                            <button
                                                                class="screen-only inline-flex items-center gap-1 text-ctp-subtext0 hover:text-ctp-text text-sm"
                                                                on:click=move |_| {
                                                                    let _ = js_sys::eval("window.printWithLightMermaid && window.printWithLightMermaid()");
                                                                }
                                                            >
                                                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                    <polyline points="6 9 6 2 18 2 18 9"/>
                                                                    <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/>
                                                                    <rect x="6" y="14" width="12" height="8"/>
                                                                </svg>
                                                                "Print"
                                                            </button>
                                                        </div>
                                                    </div>
                                                    <div class="flex justify-between items-start">
                                                        <div class="flex flex-wrap gap-2">
                                                            {(!selected_note.tags.is_empty())
                                                                .then(|| {
                                                                    selected_note.tags
                                                                        .iter()
                                                                        .map(|tag: &String| {
                                                                            view! {
                                                                                <span class="bg-ctp-surface1 text-ctp-subtext1 text-xs px-2 py-1 rounded">
                                                                                    {tag.clone()}
                                                                                </span>
                                                                            }
                                                                        })
                                                                        .collect::<Vec<_>>()
                                                                })}
                                                        </div>
                                                        <div class="flex flex-col gap-1 text-sm text-ctp-overlay0 text-right">
                                                            <span>"Created: " {selected_note.created_at.clone()}</span>
                                                            <span>"Updated: " {selected_note.updated_at.clone()}</span>
                                                        </div>
                                                    </div>
                                                </div>

                                                // Scrollable content
                                                <div class="flex-1 overflow-y-auto min-h-0 pt-6">
                                                    <div class="prose prose-invert max-w-none">
                                                        <MarkdownContent content=selected_note.content.clone()/>
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                    Err(err) => {
                                        view! {
                                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                <p class="text-ctp-red font-semibold">"Error loading note"</p>
                                                <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }
                        }
                    }}
                </div>
            </div>

            // Print-only container with all subnotes
            <div class="print-only">
                {move || {
                    let notes = print_notes.get();
                    let parent = note.clone();
                    let mut all_notes = vec![
                        view! {
                            <div class="note-print-page">
                                <h1 class="text-2xl font-bold mb-4">{parent.title.clone()}</h1>
                                <div class="prose max-w-none">
                                    <MarkdownContent content=parent.content.clone() />
                                </div>
                            </div>
                        }.into_any()
                    ];
                    for subnote in notes {
                        all_notes.push(
                            view! {
                                <div class="note-print-page">
                                    <h1 class="text-2xl font-bold mb-4">{subnote.title.clone()}</h1>
                                    <div class="prose max-w-none">
                                        <MarkdownContent content=subnote.content.clone() />
                                    </div>
                                </div>
                            }.into_any()
                        );
                    }
                    all_notes
                }}
            </div>
        }.into_any()
    } else {
        // Full-width layout for single notes
        view! {
            <div class="flex flex-col h-[calc(100vh-12rem)] note-print-area">
                // Header: title, tags, metadata
                <div class="flex-shrink-0 pb-4 border-b border-ctp-surface1">
                    <div class="flex items-center gap-3 mb-4">
                        <EntityHeader id=note.id.clone() title=note.title.clone()/>
                        <div class="ml-auto">
                            <button
                                class="screen-only inline-flex items-center gap-1 text-ctp-subtext0 hover:text-ctp-text text-sm"
                                on:click=move |_| {
                                    let _ = js_sys::eval("window.printWithLightMermaid && window.printWithLightMermaid()");
                                }
                            >
                                                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                    <polyline points="6 9 6 2 18 2 18 9"/>
                                                                    <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/>
                                                                    <rect x="6" y="14" width="12" height="8"/>
                                                                </svg>
                                                                "Print"
                                                            </button>
                                                        </div>
                                                    </div>
                                                    <div class="flex justify-between items-start">
                                                        <div class="flex flex-wrap gap-2">
                                                            {(!note.tags.is_empty())
                                                                .then(|| {
                                                                    note.tags
                                                                        .iter()
                                                                        .map(|tag: &String| {
                                                                            view! {
                                                                                <span class="bg-ctp-surface1 text-ctp-subtext1 text-xs px-2 py-1 rounded">
                                                                                    {tag.clone()}
                                                                                </span>
                                                                            }
                                                                        })
                                                                        .collect::<Vec<_>>()
                                                                })}
                                                        </div>
                                                        <div class="flex flex-col gap-1 text-sm text-ctp-overlay0 text-right">
                                                            <span>"Created: " {note.created_at.clone()}</span>
                                                            <span>"Updated: " {note.updated_at.clone()}</span>
                                                        </div>
                                                    </div>
                                                </div>

                                                // Scrollable content
                                                <div class="flex-1 overflow-y-auto min-h-0 pt-6">
                                                    <div class="prose prose-invert max-w-none">
                                                        <MarkdownContent content=note.content.clone()/>
                                                    </div>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                }
                                Err(err) => {
                                    view! {
                                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                            <p class="text-ctp-red font-semibold">"Error loading note"</p>
                                            <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                        </div>
                                    }.into_any()
                                }
                            }
                        }
                    }
                }}
            </div>
        </div>
    }
}
