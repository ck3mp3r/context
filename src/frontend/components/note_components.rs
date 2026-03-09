use leptos::prelude::*;
use pulldown_cmark::{Options, Parser, html};

use crate::api::QueryBuilder;
use crate::components::CopyableId;
use crate::models::{Note, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn NoteCard(
    note: Note,
    #[prop(optional)] on_click: Option<Callback<String>>,
    #[prop(optional)] project_id: Option<String>,
    #[prop(optional)] current_query: Option<String>,
    #[prop(optional)] breadcrumb_name: Option<String>,
) -> impl IntoView {
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

    // Check if note has subnotes for stacked effect
    let has_subnotes = note.subnote_count.unwrap_or(0) > 0;

    let page_state = use_context::<crate::breadcrumb_state::BreadcrumbPageState>();

    let note_id = note.id.clone();
    let href = if on_click.is_some() {
        "#".to_string()
    } else if let Some(proj_id) = project_id {
        format!("/projects/{}/notes/{}", proj_id, note.id)
    } else {
        format!("/notes/{}", note.id)
    };

    view! {
        // Wrapper - natural sizing
        <div class="relative w-full">
            // Background layers - ALL same size: (W-8px) × (H-8px), positioned with offset
            // Layer 2 (BACK) at (8px, 8px): left=8px, right=0 → width = W-8px ✓
            {has_subnotes.then(|| view! {
                <div class="absolute bg-ctp-surface2 border border-ctp-overlay0 rounded-lg opacity-40 pointer-events-none" style="z-index: 0; top: 8px; left: 8px; right: 0; bottom: 0;"></div>
            })}
            // Layer 1 (MIDDLE) at (4px, 4px): left=4px, right=4px → width = W-8px ✓
            {has_subnotes.then(|| view! {
                <div class="absolute bg-ctp-surface1 border border-ctp-surface2 rounded-lg opacity-60 pointer-events-none" style="z-index: 1; top: 4px; left: 4px; right: 4px; bottom: 4px;"></div>
            })}

            // Main card (FRONT) at (0, 0), size (W-8px) × (H-8px)
            <div class="relative bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col"
                 style={if has_subnotes { "z-index: 2; width: calc(100% - 8px); height: calc(100% - 8px);" } else { "z-index: 2; width: 100%; height: 100%;" }}>
            <a
                href=href
                on:click=move |ev| {
                    // Store query state before navigation
                    if let (Some(state), Some(query), Some(name)) =
                        (page_state.as_ref(), &current_query, &breadcrumb_name) {
                        state.set_query(name, query);
                    }

                    // Existing on_click logic
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
pub fn NoteStackSidebar(parent_note: Note, on_note_select: Callback<String>) -> impl IntoView {
    use leptos::task::spawn_local;

    let (subnotes, set_subnotes) = signal(Vec::<Note>::new());
    let (offset, set_offset) = signal(0);
    let (loading, set_loading) = signal(false);
    let (total_count, set_total_count) = signal(0);

    // Internal selection state - starts with parent selected
    let (_selected_note_id, _set_selected_note_id) = signal(parent_note.id.clone());

    let parent_id = parent_note.id.clone();
    let parent_id_for_fetch = parent_note.id.clone();

    // WebSocket updates - refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);
    let ws_updates = use_websocket_updates();

    // Watch for WebSocket note updates - refetch if any note in this stack changes
    Effect::new(move || {
        if let Some(
            UpdateMessage::NoteUpdated { note_id: _ }
            | UpdateMessage::NoteDeleted { note_id: _ }
            | UpdateMessage::NoteCreated { note_id: _ },
        ) = ws_updates.get()
        {
            // Refetch the entire sidebar to catch any changes to subnotes
            web_sys::console::log_1(&"Note updated via WebSocket, refetching sidebar...".into());
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Initial fetch of subnotes + refetch on WebSocket updates
    Effect::new(move || {
        let _ = refetch_trigger.get(); // Track refetch trigger
        let parent_id = parent_id_for_fetch.clone();
        spawn_local(async move {
            let result = QueryBuilder::<Note>::new()
                .limit(12)
                .offset(0)
                .param("type", "subnote")
                .param("parent_id", parent_id)
                .fetch()
                .await;

            if let Ok(paginated) = result {
                set_subnotes.set(paginated.items);
                set_total_count.set(paginated.total);
            }
        });
    });

    let parent_id_for_load = parent_id.clone();
    let load_more = move |_| {
        set_loading.set(true);
        let parent_id = parent_id_for_load.clone();
        let current_offset = offset.get();
        let new_offset = current_offset + 12;

        spawn_local(async move {
            let result = QueryBuilder::<Note>::new()
                .limit(12)
                .offset(new_offset)
                .param("type", "subnote")
                .param("parent_id", parent_id)
                .fetch()
                .await;

            if let Ok(paginated) = result {
                set_subnotes.update(|notes| notes.extend(paginated.items));
                set_offset.set(new_offset);
            }
            set_loading.set(false);
        });
    };

    let scroll_ref = NodeRef::<leptos::html::Div>::new();

    // Helper to update selection in DOM (non-reactive)
    let update_selection = {
        move |note_id: String| {
            use wasm_bindgen::JsCast;
            if let Some(container) = scroll_ref.get() {
                let element: &web_sys::Element = container.unchecked_ref();
                // Remove selected from all cards
                if let Ok(cards) = element.query_selector_all("[data-note-id]") {
                    for i in 0..cards.length() {
                        if let Some(node) = cards.item(i)
                            && let Some(card) = node.dyn_ref::<web_sys::Element>()
                        {
                            let _ = card.set_attribute("data-selected", "false");
                        }
                    }
                }
                // Add selected to the clicked card
                let selector = format!("[data-note-id='{}']", note_id);
                if let Ok(Some(card)) = element.query_selector(&selector) {
                    let _ = card.set_attribute("data-selected", "true");
                }
            }
        }
    };

    // Set initial highlight on mount
    {
        let scroll_ref_for_init = scroll_ref;
        let update_selection_for_init = update_selection;
        let parent_id_for_init = parent_note.id.clone();
        Effect::new(move || {
            if scroll_ref_for_init.get().is_some() {
                update_selection_for_init(parent_id_for_init.clone());
            }
        });
    }

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
            let displayed = subnotes.get().len();
            if scroll_top + client_height >= scroll_height - 200.0 && displayed < total_count.get()
            {
                load_more(());
            }
        }
    };

    view! {
        <div class="note-stack-sidebar p-3" node_ref=scroll_ref on:scroll=on_scroll>
                // Parent note preview (static, doesn't re-render)
                {
                    let parent_id = parent_note.id.clone();
                    let update_selection_parent = update_selection;
                    view! {
                        <div
                            class="note-stack-card rounded-lg p-2 cursor-pointer transition-colors mb-2 flex flex-col overflow-hidden"
                            style="height: 200px; width: 150px;"
                            data-note-id=parent_id.clone()
                            data-selected="false"
                            on:click=move |e| {
                                e.prevent_default();
                                let id = parent_note.id.clone();
                                update_selection_parent(id.clone());
                                on_note_select.run(id);
                            }
                        >
                            <h4 class="text-sm font-semibold text-ctp-text mb-1 truncate">
                                {parent_note.title.clone()}
                            </h4>
                            <div class="text-xs text-ctp-subtext0 line-clamp-4 mb-2 flex-1">
                                {if parent_note.content.chars().count() > 150 {
                                    format!("{}...", parent_note.content.chars().take(150).collect::<String>())
                                } else {
                                    parent_note.content.clone()
                                }}
                            </div>
                            {(!parent_note.tags.is_empty()).then(|| {
                                view! {
                                    <div class="flex flex-wrap gap-1 overflow-hidden">
                                        {parent_note.tags.iter().take(3).map(|tag| {
                                            view! {
                                                <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-1.5 py-0.5 rounded">
                                                    {tag.clone()}
                                                </span>
                                            }
                                        }).collect::<Vec<_>>()}
                                        {(parent_note.tags.len() > 3).then(|| {
                                            view! {
                                                <span class="text-xs text-ctp-overlay0">
                                                    "+"{parent_note.tags.len() - 3}
                                                </span>
                                            }
                                        })}
                                    </div>
                                }
                            })}
                        </div>
                    }
                }

                // Subnotes (use For with stable keys to prevent re-render)
                <For
                    each=move || subnotes.get()
                    key=|note| note.id.clone()
                    children=move |note| {
                        let note_id = note.id.clone();
                        let note_clone = note.clone();
                        let update_selection_child = update_selection;
                        view! {
                            <div
                                class="note-stack-card ml-2 rounded-lg p-2 cursor-pointer transition-colors mb-2 flex flex-col overflow-hidden"
                                style="height: 200px; width: 150px;"
                                data-note-id=note_id.clone()
                                data-selected="false"
                                on:click=move |e| {
                                    e.prevent_default();
                                    let id = note_id.clone();
                                    update_selection_child(id.clone());
                                    on_note_select.run(id);
                                }
                            >
                                <h4 class="text-sm font-semibold text-ctp-text mb-1 truncate">
                                    {note_clone.title.clone()}
                                </h4>
                                <div class="text-xs text-ctp-subtext0 line-clamp-4 mb-2 flex-1">
                                    {if note_clone.content.chars().count() > 150 {
                                        format!("{}...", note_clone.content.chars().take(150).collect::<String>())
                                    } else {
                                        note_clone.content.clone()
                                    }}
                                </div>
                                {(!note_clone.tags.is_empty()).then(|| {
                                    view! {
                                        <div class="flex flex-wrap gap-1 overflow-hidden">
                                            {note_clone.tags.iter().take(3).map(|tag| {
                                                view! {
                                                    <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-1.5 py-0.5 rounded">
                                                        {tag.clone()}
                                                    </span>
                                                }
                                            }).collect::<Vec<_>>()}
                                            {(note_clone.tags.len() > 3).then(|| {
                                                view! {
                                                    <span class="text-xs text-ctp-overlay0">
                                                        "+"{note_clone.tags.len() - 3}
                                                    </span>
                                                }
                                            })}
                                        </div>
                                    }
                                })}
                            </div>
                        }
                    }
                />

                // Loading indicator
                {move || {
                    loading.get().then(|| {
                        view! {
                            <div class="py-2 text-center">
                                <span class="text-ctp-subtext0 text-xs">"Loading more..."</span>
                            </div>
                        }
                    })
                }}
        </div>
    }
}
