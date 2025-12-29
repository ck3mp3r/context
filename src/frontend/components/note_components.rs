use leptos::prelude::*;
use pulldown_cmark::{Options, Parser, html};
use thaw::*;

use crate::api::notes;
use crate::components::CopyableId;
use crate::models::Note;

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

    view! {
        <div class="relative bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[220px]">
            <div class="absolute top-2 right-2">
                <CopyableId id=note.id.clone()/>
            </div>
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
                <h3 class="text-xl font-semibold text-ctp-text mb-2 pr-20">{note.title.clone()}</h3>

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
    let note_resource = LocalResource::new(move || {
        let id = note_id.get();
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
                                let result_clone = (*result).clone();
                                match result_clone {
                                    Ok(note) => {
                                        view! {
                                            <div class="space-y-4">
                                                <div class="flex justify-between items-start mb-4">
                                                    <h2 class="text-2xl font-bold text-ctp-text">
                                                        {note.title.clone()}
                                                    </h2>
                                                    <button
                                                        on:click=move |_| open.set(false)
                                                        class="text-ctp-overlay0 hover:text-ctp-text text-2xl leading-none px-2"
                                                    >
                                                        "✕"
                                                    </button>
                                                </div>
                                                <div class="flex justify-between text-sm text-ctp-overlay0">
                                                    <span>"ID: " {note.id.clone()}</span>
                                                    <span>"Updated: " {note.updated_at.clone()}</span>
                                                </div>

                                                        {(!note.tags.is_empty())
                                                            .then(|| {
                                                                view! {
                                                                    <div class="flex flex-wrap gap-2">
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

                                                <div class="prose prose-invert max-w-none">
                                                    <MarkdownContent content=note.content.clone()/>
                                                </div>
                                            </div>
                                    }
                                            .into_any()
                                        }
                                        Err(err) => {
                                            view! {
                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                    <p class="text-ctp-red font-semibold">"Error loading note"</p>
                                                    <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
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
