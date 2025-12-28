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
    const PAGE_SIZE: usize = 20;

    // State management
    let (page, set_page) = signal(0usize);
    let (search_query, set_search_query) = signal(String::new());

    // Fetch notes with current page and search query
    let notes_resource = LocalResource::new(move || async move {
        let offset = page.get() * PAGE_SIZE;
        let query = search_query.get();
        let query_opt = if query.trim().is_empty() {
            None
        } else {
            Some(query)
        };
        notes::list(Some(PAGE_SIZE), Some(offset), query_opt).await
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

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }
            }>
                {move || {
                    notes_resource
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(paginated) => {
                                let total_pages = (paginated.total + PAGE_SIZE - 1) / PAGE_SIZE;
                                let current_page = page.get();

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
                                            // Results summary
                                            <div class="text-sm text-ctp-overlay0 mb-4">
                                                "Showing "
                                                {paginated.offset + 1}
                                                " - "
                                                {(paginated.offset + paginated.items.len()).min(paginated.total)}
                                                " of "
                                                {paginated.total}
                                                " notes"
                                            </div>

                                            // Notes grid
                                            <div class="grid gap-4 mb-6">
                                                {paginated
                                                    .items
                                                    .iter()
                                                    .map(|note| {
                                                        view! { <NoteCard note=note.clone()/> }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>

                                            // Pagination controls
                                            {(total_pages > 1)
                                                .then(|| {
                                                    view! {
                                                        <div class="flex justify-center items-center gap-2">
                                                            // Previous button
                                                            <button
                                                                on:click=move |_| {
                                                                    if current_page > 0 {
                                                                        go_to_page(current_page - 1)
                                                                    }
                                                                }

                                                                disabled=move || current_page == 0
                                                                class="px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded text-ctp-text disabled:opacity-50 disabled:cursor-not-allowed hover:border-ctp-blue"
                                                            >
                                                                "← Previous"
                                                            </button>

                                                            // Page numbers
                                                            <span class="text-ctp-subtext0">
                                                                "Page " {current_page + 1} " of " {total_pages}
                                                            </span>

                                                            // Next button
                                                            <button
                                                                on:click=move |_| {
                                                                    if current_page < total_pages - 1 {
                                                                        go_to_page(current_page + 1)
                                                                    }
                                                                }

                                                                disabled=move || current_page >= total_pages - 1
                                                                class="px-4 py-2 bg-ctp-surface0 border border-ctp-surface1 rounded text-ctp-text disabled:opacity-50 disabled:cursor-not-allowed hover:border-ctp-blue"
                                                            >
                                                                "Next →"
                                                            </button>
                                                        </div>
                                                    }
                                                })}
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
