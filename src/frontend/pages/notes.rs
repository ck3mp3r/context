use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api::notes;
use crate::models::Note;

#[component]
pub fn Notes() -> impl IntoView {
    // Check if we have a note ID in the URL params
    let params = use_params_map();
    let note_id = move || params.read().get("id").unwrap_or_default();

    view! {
        {move || {
            if note_id().is_empty() {
                view! { <NotesList/> }.into_any()
            } else {
                view! { <NoteDetail id=note_id()/> }.into_any()
            }
        }}
    }
}

#[component]
fn NotesList() -> impl IntoView {
    let notes_resource = LocalResource::new(|| async move { notes::list(Some(50), None).await });

    view! {
        <div class="container mx-auto p-6">
            <h2 class="text-3xl font-bold text-ctp-text mb-6">"Notes"</h2>

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }
            }>
                {move || {
                    notes_resource
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(paginated) => {
                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            "No notes found. Create one to get started!"
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="grid gap-4">
                                            {paginated
                                                .items
                                                .iter()
                                                .map(|note| {
                                                    view! { <NoteCard note=note.clone()/> }
                                                })
                                                .collect::<Vec<_>>()}
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
                        })
                }}

            </Suspense>
        </div>
    }
}

#[component]
fn NoteCard(note: Note) -> impl IntoView {
    // Create a preview of the content (first 200 chars)
    let preview = if note.content.len() > 200 {
        format!("{}...", &note.content[..200])
    } else {
        note.content.clone()
    };

    view! {
        <a
            href=format!("/notes/{}", note.id)
            class="block bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors"
        >
            <div class="flex justify-between items-start mb-2">
                <h3 class="text-xl font-semibold text-ctp-text">{note.title.clone()}</h3>
                <span class="text-xs text-ctp-overlay0 ml-2 flex-shrink-0">{note.id.clone()}</span>
            </div>

            <p class="text-ctp-subtext0 text-sm mb-3 line-clamp-3">{preview}</p>

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

            <div class="flex justify-between text-xs text-ctp-overlay0 mt-3">
                <span>"Created: " {note.created_at}</span>
                <span>"Updated: " {note.updated_at}</span>
            </div>
        </a>
    }
}

#[component]
fn NoteDetail(id: String) -> impl IntoView {
    let note_resource = LocalResource::new(move || {
        let note_id = id.clone();
        async move { notes::get(&note_id).await }
    });

    view! {
        <div class="container mx-auto p-6">
            <a
                href="/notes"
                class="inline-flex items-center text-ctp-blue hover:text-ctp-lavender mb-6"
            >
                "← Back to Notes"
            </a>

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading note..."</p> }
            }>
                {move || {
                    note_resource
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(note) => {
                                view! {
                                    <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-6">
                                        <div class="mb-6">
                                            <h2 class="text-3xl font-bold text-ctp-text mb-2">
                                                {note.title.clone()}
                                            </h2>
                                            <div class="flex justify-between text-sm text-ctp-overlay0">
                                                <span>"ID: " {note.id.clone()}</span>
                                                <span>"Updated: " {note.updated_at.clone()}</span>
                                            </div>
                                        </div>

                                        {(!note.tags.is_empty())
                                            .then(|| {
                                                view! {
                                                    <div class="flex flex-wrap gap-2 mb-6">
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
                        })
                }}

            </Suspense>
        </div>
    }
}

#[component]
fn MarkdownContent(content: String) -> impl IntoView {
    use pulldown_cmark::{Options, Parser, html};

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
