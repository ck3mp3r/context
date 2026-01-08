use leptos::prelude::*;
use pulldown_cmark::{Options, Parser, html};
use thaw::*;

use crate::api::notes;
use crate::components::CopyableId;
use crate::models::{Note, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn NoteCard(note: Note, #[prop(optional)] on_click: Option<Callback<String>>) -> impl IntoView {
    // Create a preview of the content (first 300 chars for markdown, UTF-8 safe)
    let preview_content = if note.content.chars().count() > 300 {
        let truncated: String = note.content.chars().take(300).collect();
        format!("{}...", truncated)
    } else {
        note.content.clone()
    };

    // Parse markdown to HTML for preview
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(&preview_content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    let note_id = note.id.clone();
    let href = if on_click.is_some() {
        "#".to_string()
    } else {
        format!("/notes/{}", note.id)
    };

    // Check if note has subnotes for stacked effect
    let has_subnotes = note.subnote_count.unwrap_or(0) > 0;

    view! {
        // Wrapper for stacked effect - only visible when has_subnotes
        <div class="relative">
            // Stack layer 2 (furthest back) - only show if has subnotes
            {has_subnotes.then(|| view! {
                <div class="absolute inset-0 bg-ctp-surface1 border border-ctp-surface2 rounded-lg translate-x-2 translate-y-2 -z-20 opacity-40"></div>
            })}
            // Stack layer 1 (middle) - only show if has subnotes
            {has_subnotes.then(|| view! {
                <div class="absolute inset-0 bg-ctp-surface1 border border-ctp-surface2 rounded-lg translate-x-1 translate-y-1 -z-10 opacity-60"></div>
            })}
            // Main card (front)
            <div class="relative bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[220px]">
            <a
                href=href
                on:click=move |ev| {
                    if let Some(callback) = on_click {
                        ev.prevent_default();
                        callback.run(note_id.clone());
                    }
                }

                class="flex flex-col h-full"
            >
                <div class="flex items-start gap-2 mb-2">
                    <div class="flex-shrink-0">
                        <CopyableId id=note.id.clone()/>
                    </div>
                    <h3 class="flex-1 min-w-0 break-words text-xl font-semibold text-ctp-text">{note.title.clone()}</h3>
                </div>

            <div class="relative flex-grow mb-4">
                <div class="text-ctp-subtext0 text-sm leading-relaxed note-preview overflow-hidden" style="max-height: 8rem;" inner_html=html_output></div>
                <div class="absolute bottom-0 left-0 right-0 h-16 bg-gradient-to-t from-ctp-surface0 to-transparent pointer-events-none"></div>
            </div>

            <div class="mt-auto">
            {(!note.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-2">
                            {note
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

            <div class="flex justify-between text-xs text-ctp-overlay0">
                <span>"Created: " {note.created_at}</span>
                <span>"Updated: " {note.updated_at}</span>
            </div>
            </div>
            </a>
            </div>
            // End main card
        </div>
        // End wrapper
    }
}

#[component]
pub fn MarkdownContent(content: String) -> impl IntoView {
    // Parse markdown to HTML
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    view! { <div inner_html=html_output></div> }
}

#[component]
pub fn NoteDetailModal(note_id: ReadSignal<String>, open: RwSignal<bool>) -> impl IntoView {
    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Trigger to force refetch when this specific note is updated
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates for THIS note
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            let current_note_id = note_id.get();
            if !current_note_id.is_empty() {
                match update {
                    UpdateMessage::NoteUpdated {
                        note_id: updated_id,
                    }
                    | UpdateMessage::NoteDeleted {
                        note_id: updated_id,
                    } => {
                        if updated_id == current_note_id {
                            web_sys::console::log_1(
                                &format!(
                                    "Note {} updated via WebSocket, refetching detail...",
                                    updated_id
                                )
                                .into(),
                            );
                            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    let note_resource = LocalResource::new(move || {
        let id = note_id.get();
        let _ = refetch_trigger.get(); // Track refetch trigger
        async move {
            if id.is_empty() {
                Err(crate::api::ApiClientError::Network(
                    "No note selected".to_string(),
                ))
            } else {
                notes::get(&id).await
            }
        }
    });

    view! {
        <OverlayDrawer
            open
            position=DrawerPosition::Right
            class="note-detail-drawer"
        >
            <DrawerBody>
                <Suspense fallback=move || {
                    view! { <p class="text-ctp-subtext0">"Loading note..."</p> }
                }>
                    {move || {
                        note_resource
                            .get()
                            .map(|result| {
                                match result {
                                    Ok(note) => {
                                        view! {
                                            <div class="space-y-4">
                                                <div class="flex justify-between items-start mb-4">
                                                    <div class="flex items-center gap-3">
                                                        <CopyableId id=note.id.clone()/>
                                                        <h2 class="text-2xl font-bold text-ctp-text">
                                                            {note.title.clone()}
                                                        </h2>
                                                    </div>
                                                    <button
                                                        on:click=move |_| open.set(false)
                                                        class="text-ctp-overlay0 hover:text-ctp-text text-2xl leading-none px-2"
                                                    >
                                                        "✕"
                                                    </button>
                                                </div>
                                                <div class="flex justify-between items-start mb-4">
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

                                                <div class="prose prose-invert max-w-none">
                                                    <MarkdownContent content=note.content.clone()/>
                                                </div>
                                            </div>
                                    }
                                            .into_any()
                                        }
                                        Err(err) => {
                                            let err_msg = err.to_string();
                                            view! {
                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                    <p class="text-ctp-red font-semibold">"Error loading note"</p>
                                                    <p class="text-ctp-subtext0 text-sm mt-2">{err_msg}</p>
                                                </div>
                                            }
                                                .into_any()
                                        }
                                    }
                                })
                        }}

                    </Suspense>
            </DrawerBody>
        </OverlayDrawer>
    }
}
