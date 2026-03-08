use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, QueryBuilder, notes, projects};
use crate::components::{
    Breadcrumb, BreadcrumbItem, CopyableId, MarkdownContent, NoteStackSidebar,
};
use crate::models::{Note, Project, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn NoteDetail() -> impl IntoView {
    let params = use_params_map();
    let note_id = move || params.read().get("id").unwrap_or_default();
    let project_id = move || params.read().get("project_id");

    let (note_data, set_note_data) = signal(None::<Result<Note, ApiClientError>>);
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Fetch project if we have a project_id (coming from project context)
    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);

    Effect::new(move || {
        if let Some(proj_id) = project_id() {
            spawn_local(async move {
                let result = projects::get(&proj_id).await;
                set_project_data.set(Some(result));
            });
        }
    });

    // Fetch subnotes to determine if we should show sidebar
    let (subnotes_data, set_subnotes_data) = signal(None::<Result<Vec<Note>, ApiClientError>>);
    let (subnotes_refetch_trigger, _set_subnotes_refetch_trigger) = signal(0u32);

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Watch for WebSocket updates for THIS note
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            let current_note_id = note_id();
            if !current_note_id.is_empty() {
                match update {
                    UpdateMessage::NoteUpdated {
                        note_id: updated_id,
                    } => {
                        if updated_id == current_note_id {
                            web_sys::console::log_1(
                                &format!(
                                    "Note {} updated via WebSocket, refetching...",
                                    updated_id
                                )
                                .into(),
                            );
                            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                        }
                    }
                    UpdateMessage::NoteDeleted {
                        note_id: updated_id,
                    } => {
                        if updated_id == current_note_id {
                            // Note was deleted - navigate back to notes list
                            web_sys::console::log_1(
                                &format!(
                                    "Note {} deleted via WebSocket, navigating back...",
                                    updated_id
                                )
                                .into(),
                            );
                            leptos_router::hooks::use_navigate()("/notes", Default::default());
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // Fetch note when ID or refetch trigger changes
    Effect::new(move || {
        let id = note_id();
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
        let id = note_id();
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

    // Callback for note selection from sidebar
    let on_note_select = Callback::new(move |id: String| {
        selected_note_id.set(id);
    });

    // Watch for WebSocket updates for the SELECTED note
    Effect::new(move || {
        if let Some(update) = ws_updates.get() {
            let current_selected = selected_note_id.get();
            let parent_note_id = note_id();
            if !current_selected.is_empty() {
                match update {
                    UpdateMessage::NoteUpdated {
                        note_id: updated_id,
                    } => {
                        if updated_id == current_selected {
                            web_sys::console::log_1(
                                &format!(
                                    "Selected note {} updated via WebSocket, refetching...",
                                    updated_id
                                )
                                .into(),
                            );
                            set_selected_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                        }
                    }
                    UpdateMessage::NoteDeleted {
                        note_id: updated_id,
                    } => {
                        if updated_id == current_selected && current_selected != parent_note_id {
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
                    }
                    _ => {}
                }
            }
        }
    });

    // Resource for fetching the selected note (for stack navigation)
    let (selected_note_data, set_selected_note_data) = signal(None::<Result<Note, ApiClientError>>);

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
            // Breadcrumb navigation - different based on context
            {move || {
                // Check if we have project context
                if let Some(_proj_id) = project_id() {
                    // Coming from project - show Projects → Project → Note
                    match (project_data.get(), note_data.get()) {
                        (Some(Ok(project)), Some(Ok(note))) => {
                            let has_subs = subnotes_data.get()
                                .and_then(|r| r.ok())
                                .map(|items| !items.is_empty())
                                .unwrap_or(false);

                            let (title, id) = if has_subs {
                                // Use selected note for breadcrumb
                                selected_note_data.get()
                                    .and_then(|r| r.ok())
                                    .map(|n| (n.title.clone(), n.id.clone()))
                                    .unwrap_or_else(|| (note.title.clone(), note.id.clone()))
                            } else {
                                (note.title.clone(), note.id.clone())
                            };

                            let items = vec![
                                BreadcrumbItem::new("Projects")
                                    .with_href("/")
                                    .with_name("projects"),
                                BreadcrumbItem::new(project.title.clone())
                                    .with_id(project.id.clone())
                                    .with_href(format!("/projects/{}/notes", project.id))
                                    .with_name(project.id.clone()),
                                BreadcrumbItem::new(title)
                                    .with_id(id),
                            ];
                            Some(view! { <Breadcrumb items=items/> })
                        }
                        _ => None
                    }
                } else {
                    // Coming from Notes page - show Notes → Note
                    let has_subs = subnotes_data.get()
                        .and_then(|r| r.ok())
                        .map(|items| !items.is_empty())
                        .unwrap_or(false);

                    let breadcrumb_data = if has_subs {
                        // Has subnotes - use selected note for breadcrumb
                        selected_note_data.get().and_then(|result| {
                            result.ok().map(|note| (note.title.clone(), note.id.clone()))
                        })
                    } else {
                        // No subnotes - use parent note for breadcrumb
                        note_data.get().and_then(|result| {
                            result.ok().map(|note| (note.title.clone(), note.id.clone()))
                        })
                    };

                    breadcrumb_data.map(|(title, id)| {
                        let items = vec![
                            BreadcrumbItem::new("Notes")
                                .with_href("/notes")
                                .with_name("notes"),
                            BreadcrumbItem::new(title)
                                .with_id(id),
                        ];
                        view! { <Breadcrumb items=items/> }
                    })
                }
            }}

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
                                            <div class="flex gap-6 h-[calc(100vh-12rem)]">
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
                                                                            <div class="flex flex-col h-full">
                                                                                // Header: title, tags, metadata
                                                                                <div class="flex-shrink-0 pb-4 border-b border-ctp-surface1">
                                                                                    <div class="flex items-center gap-3 mb-4">
                                                                                        <CopyableId id=selected_note.id.clone()/>
                                                                                        <h2 class="text-2xl font-bold text-ctp-text">
                                                                                            {selected_note.title.clone()}
                                                                                        </h2>
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
                                        }.into_any()
                                    } else {
                                        // Full-width layout for single notes
                                        view! {
                                            <div class="flex flex-col h-[calc(100vh-12rem)]">
                                                // Header: title, tags, metadata
                                                <div class="flex-shrink-0 pb-4 border-b border-ctp-surface1">
                                                    <div class="flex items-center gap-3 mb-4">
                                                        <CopyableId id=note.id.clone()/>
                                                        <h2 class="text-2xl font-bold text-ctp-text">
                                                            {note.title.clone()}
                                                        </h2>
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
