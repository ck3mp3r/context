use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use thaw::Tooltip;
use wasm_bindgen::prelude::*;

use crate::api::{ApiClientError, graph, projects, repos};
use crate::components::{Breadcrumb, BreadcrumbItem, CopyableId};
use crate::models::{Project, Repo, UpdateMessage};
use crate::utils::extract_repo_name;
use crate::websocket::use_websocket_updates;

#[wasm_bindgen(
    inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }"
)]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

#[component]
pub fn RepoDetail() -> impl IntoView {
    let params = use_params_map();
    let repo_id = move || params.read().get("id").unwrap_or_default();
    let project_id = move || params.read().get("project_id");

    let (repo_data, set_repo_data) = signal(None::<Result<Repo, ApiClientError>>);
    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);

    // WebSocket updates - refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);
    let ws_updates = use_websocket_updates();

    // Fetch project if we have a project_id (coming from project context)
    Effect::new(move || {
        if let Some(proj_id) = project_id() {
            spawn_local(async move {
                let result = projects::get(&proj_id).await;
                set_project_data.set(Some(result));
            });
        }
    });

    // Watch for WebSocket repo updates
    Effect::new(move || {
        if let Some(msg) = ws_updates.get() {
            let current_id = repo_id();
            if !current_id.is_empty() {
                match &msg {
                    UpdateMessage::RepoUpdated {
                        repo_id: updated_id,
                    } if *updated_id == current_id => {
                        set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                    }
                    UpdateMessage::RepoDeleted {
                        repo_id: deleted_id,
                    } if *deleted_id == current_id => {
                        let navigate = leptos_router::hooks::use_navigate();
                        if let Some(proj_id) = project_id() {
                            navigate(&format!("/projects/{}/repos", proj_id), Default::default());
                        } else {
                            navigate("/repos", Default::default());
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // Fetch repo when params change or refetch trigger fires
    Effect::new(move || {
        let id = repo_id();
        let _ = refetch_trigger.get();

        if !id.is_empty() {
            spawn_local(async move {
                let result = repos::get(&id).await;
                set_repo_data.set(Some(result));
            });
        }
    });

    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]">
            // Breadcrumb navigation
            {move || {
                match repo_data.get() {
                    Some(Ok(ref repo)) => {
                        let display_name = extract_repo_name(&repo.remote);

                        let items = if let Some(Ok(ref project)) = project_data.get() {
                            vec![
                                BreadcrumbItem::new("Projects")
                                    .with_href("/")
                                    .with_name("projects"),
                                BreadcrumbItem::new(project.title.clone())
                                    .with_id(project.id.clone())
                                    .with_href(format!("/projects/{}/repos", project.id))
                                    .with_name(project.id.clone()),
                                BreadcrumbItem::new(display_name)
                                    .with_id(repo.id.clone()),
                            ]
                        } else if project_id().is_some() {
                            return None;
                        } else {
                            vec![
                                BreadcrumbItem::new("Repos")
                                    .with_href("/repos")
                                    .with_name("repos"),
                                BreadcrumbItem::new(display_name)
                                    .with_id(repo.id.clone()),
                            ]
                        };

                        Some(view! { <Breadcrumb items=items/> })
                    }
                    _ => None,
                }
            }}

            <div class="container mx-auto px-6 py-6 flex-1">
                {move || {
                    match repo_data.get() {
                        Some(Ok(repo)) => {
                            view! { <RepoDetailContent repo=repo/> }.into_any()
                        }
                        Some(Err(err)) => {
                            view! {
                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                    <p class="text-ctp-red font-semibold">"Error loading repository"</p>
                                    <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                </div>
                            }
                                .into_any()
                        }
                        None => {
                            view! { <p class="text-ctp-subtext0">"Loading..."</p> }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn RepoDetailContent(repo: Repo) -> impl IntoView {
    let display_name = extract_repo_name(&repo.remote);
    let remote_url = repo.remote.clone();
    let remote_for_link = repo.remote.clone();

    view! {
        <div class="space-y-6">
            // Header
            <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-6">
                <div class="flex items-start gap-3">
                    <CopyableId id=repo.id.clone()/>
                    <div class="flex-1 min-w-0">
                        <div class="flex items-start justify-between mb-2">
                            <div class="flex items-center gap-3">
                                <h2 class="text-2xl font-bold text-ctp-text truncate">
                                    {display_name}
                                </h2>
                                <a
                                    href=remote_for_link
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="flex-shrink-0 text-ctp-overlay0 hover:text-ctp-blue transition-colors"
                                    title="Open remote"
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor">
                                        <path d="M11 3a1 1 0 100 2h2.586l-6.293 6.293a1 1 0 101.414 1.414L15 6.414V9a1 1 0 102 0V4a1 1 0 00-1-1h-5z"/>
                                        <path d="M5 5a2 2 0 00-2 2v8a2 2 0 002 2h8a2 2 0 002-2v-3a1 1 0 10-2 0v3H5V7h3a1 1 0 000-2H5z"/>
                                    </svg>
                                </a>
                            </div>
                            <span class="text-xs text-ctp-overlay0 flex-shrink-0">{repo.created_at}</span>
                        </div>

                        <p class="text-ctp-subtext0 text-sm font-mono mb-3">{remote_url}</p>

                        {repo.path.map(|p| {
                            view! {
                                <p class="text-ctp-subtext0 text-sm font-mono">
                                    <span class="text-ctp-overlay1">"Path: "</span>
                                    {p}
                                </p>
                            }
                        })}

                        {(!repo.tags.is_empty())
                            .then(|| {
                                view! {
                                    <div class="flex flex-wrap gap-2 mt-3">
                                        {repo
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
                    </div>
                </div>
            </div>

            // Graph
            <GraphViewer repo_id=repo.id.clone()/>
        </div>
    }
}

// =============================================================================
// JS bridge bindings
// =============================================================================

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = initGraph)]
    fn init_graph(container_id: &str, graph_data_json: &str) -> bool;

    #[wasm_bindgen(js_name = destroyGraph)]
    fn destroy_graph(container_id: &str);

    #[wasm_bindgen(js_name = graphZoomToFit)]
    fn zoom_to_fit(container_id: &str);

    #[wasm_bindgen(js_name = graphFilterKinds)]
    fn filter_kinds(container_id: &str, kinds_json: &str);

    #[wasm_bindgen(js_name = graphGetKinds)]
    fn get_kinds(container_id: &str) -> String;

    #[wasm_bindgen(js_name = graphGetLanguages)]
    fn get_languages(container_id: &str) -> String;

    #[wasm_bindgen(js_name = graphGetEdgeTypes)]
    fn get_edge_types(container_id: &str) -> String;

    #[wasm_bindgen(js_name = graphSearchNodes)]
    fn search_nodes(container_id: &str, query: &str) -> u32;

    #[wasm_bindgen(js_name = graphFilterEdgeTypes)]
    fn filter_edge_types(container_id: &str, types_json: &str);

    #[wasm_bindgen(js_name = graphFilterTests)]
    fn filter_tests(container_id: &str, hide: bool);

    #[wasm_bindgen(js_name = graphFilterLanguage)]
    fn filter_language(container_id: &str, language: &str);

    #[wasm_bindgen(js_name = graphSetFocusDepth)]
    fn js_set_focus_depth(container_id: &str, depth: u32);

    #[wasm_bindgen(js_name = graphOnNodeSelect)]
    fn on_node_select(container_id: &str, callback: &Closure<dyn Fn(String)>);

    #[wasm_bindgen(js_name = graphOnFocusChange)]
    fn on_focus_change(container_id: &str, callback: &Closure<dyn Fn(bool)>);

    #[wasm_bindgen(js_name = graphIsLayoutRunning)]
    fn is_layout_running(container_id: &str) -> bool;
}

// =============================================================================
// GraphViewer component
// =============================================================================

#[component]
fn GraphViewer(repo_id: String) -> impl IntoView {
    let container_id = "graph-canvas";
    let (graph_state, set_graph_state) = signal(GraphState::Loading);
    let (active_kinds, set_active_kinds) = signal(std::collections::HashSet::<String>::new());
    let (active_edges, set_active_edges) = signal(std::collections::HashSet::<String>::new());
    let (hide_tests, set_hide_tests) = signal(true);
    let (selected_language, set_selected_language) = signal(String::new());
    let (legend_kinds, set_legend_kinds) = signal(Vec::<KindInfo>::new());
    let (legend_edge_types, set_legend_edge_types) = signal(Vec::<KindInfo>::new());
    let (available_languages, set_available_languages) = signal(Vec::<String>::new());
    let (search_query, set_search_query) = signal(String::new());
    let (search_count, set_search_count) = signal(0u32);
    let (focus_depth, set_focus_depth) = signal(1u32);
    let (is_focused, set_is_focused) = signal(false);
    let (selected_node, set_selected_node) = signal(None::<SelectedNodeInfo>);
    let (layout_running, set_layout_running) = signal(false);

    // Fetch graph data once
    let repo_id_for_fetch = repo_id.clone();
    Effect::new(move || {
        let repo_id = repo_id_for_fetch.clone();
        spawn_local(async move {
            match graph::get_repo_graph(&repo_id).await {
                Ok(Some(json_data)) => {
                    set_graph_state.set(GraphState::Ready(json_data));
                }
                Ok(None) => {
                    set_graph_state.set(GraphState::NoAnalysis);
                }
                Err(e) => {
                    set_graph_state.set(GraphState::Error(e.to_string()));
                }
            }
        });
    });

    // Initialize graph renderer after Ready state is set and DOM has updated
    Effect::new(move || {
        if let GraphState::Ready(ref json_data) = graph_state.get() {
            let data = json_data.clone();
            set_timeout(
                move || {
                    if init_graph(container_id, &data) {
                        let kinds_json = get_kinds(container_id);
                        if let Ok(kinds) = serde_json::from_str::<Vec<KindInfo>>(&kinds_json) {
                            set_legend_kinds.set(kinds);
                        }
                        let edge_types_json = get_edge_types(container_id);
                        if let Ok(edge_types) =
                            serde_json::from_str::<Vec<KindInfo>>(&edge_types_json)
                        {
                            set_legend_edge_types.set(edge_types);
                        }
                        let langs_json = get_languages(container_id);
                        if let Ok(langs) = serde_json::from_str::<Vec<String>>(&langs_json) {
                            set_available_languages.set(langs);
                        }
                        set_active_kinds.set(std::collections::HashSet::new());
                        set_active_edges.set(std::collections::HashSet::new());
                        set_search_query.set(String::new());
                        // Apply initial test filter
                        filter_tests(container_id, hide_tests.get_untracked());
                        // Register node selection callback
                        let select_closure = Closure::new(move |json: String| {
                            if json.is_empty() {
                                set_selected_node.set(None);
                            } else if let Ok(info) = serde_json::from_str::<SelectedNodeInfo>(&json)
                            {
                                set_selected_node.set(Some(info));
                            }
                        });
                        on_node_select(container_id, &select_closure);
                        select_closure.forget();
                        // Register focus change callback
                        let focus_closure = Closure::new(move |focused: bool| {
                            set_is_focused.set(focused);
                        });
                        on_focus_change(container_id, &focus_closure);
                        focus_closure.forget();
                        set_layout_running.set(true);
                        set_graph_state.set(GraphState::Loaded);
                        // Poll layout status via recursive setTimeout
                        fn poll_layout(
                            container_id: &'static str,
                            set_layout_running: WriteSignal<bool>,
                        ) {
                            set_timeout(
                                move || {
                                    let running = is_layout_running(container_id);
                                    set_layout_running.set(running);
                                    if running {
                                        poll_layout(container_id, set_layout_running);
                                    }
                                },
                                std::time::Duration::from_millis(200),
                            );
                        }
                        poll_layout(container_id, set_layout_running);
                    } else {
                        set_graph_state.set(GraphState::Error(
                            "Failed to initialize graph renderer".to_string(),
                        ));
                    }
                },
                std::time::Duration::ZERO,
            );
        }
    });

    // Cleanup on unmount
    on_cleanup(move || {
        destroy_graph(container_id);
    });

    // Push kind filter to JS when active_kinds changes
    Effect::new(move || {
        let kinds = active_kinds.get();
        let kinds_vec: Vec<&str> = kinds.iter().map(|s| s.as_str()).collect();
        let json = serde_json::to_string(&kinds_vec).unwrap_or_else(|_| "[]".to_string());
        filter_kinds(container_id, &json);
    });

    // Push edge type filter to JS when active_edges changes
    Effect::new(move || {
        let edges = active_edges.get();
        let edges_vec: Vec<&str> = edges.iter().map(|s| s.as_str()).collect();
        let json = serde_json::to_string(&edges_vec).unwrap_or_else(|_| "[]".to_string());
        filter_edge_types(container_id, &json);
    });

    // Push test filter to JS when hide_tests changes
    Effect::new(move || {
        let hide = hide_tests.get();
        filter_tests(container_id, hide);
    });

    // Push language filter to JS when selected_language changes
    Effect::new(move || {
        let lang = selected_language.get();
        filter_language(container_id, &lang);
    });

    // Push search query to JS when it changes
    Effect::new(move || {
        let query = search_query.get();
        let count = search_nodes(container_id, &query);
        set_search_count.set(count);
    });

    // Push focus depth to JS when it changes
    Effect::new(move || {
        let depth = focus_depth.get();
        js_set_focus_depth(container_id, depth);
    });

    view! {
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4">
            // Main content: graph (left) + side panel (right)
            <div class="flex gap-4">
                // Graph canvas (square, takes remaining space)
                <div
                    id=container_id
                    class="flex-1 bg-ctp-mantle rounded-lg relative aspect-square max-h-[600px]"
                >
                    {move || match graph_state.get() {
                        GraphState::Loading => {
                            view! {
                                <div class="absolute inset-0 flex items-center justify-center">
                                    <div class="flex items-center gap-2 text-ctp-subtext0">
                                        <span class="inline-block w-3 h-3 rounded-full bg-ctp-blue animate-pulse"></span>
                                        <span class="text-sm">"Loading graph..."</span>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        GraphState::NoAnalysis => {
                            view! {
                                <div class="absolute inset-0 flex items-center justify-center">
                                    <p class="text-ctp-overlay0 text-sm">"No analysis available. Run analysis first."</p>
                                </div>
                            }.into_any()
                        }
                        GraphState::Error(ref msg) => {
                            view! {
                                <div class="absolute inset-0 flex items-center justify-center">
                                    <div class="text-center">
                                        <p class="text-ctp-red text-sm mb-2">"Error loading graph"</p>
                                        <p class="text-ctp-overlay0 text-xs">{msg.clone()}</p>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        _ => {
                            view! { <span></span> }.into_any()
                        }
                    }}
                    // Top-left overlay: search + language filters
                    <div class="absolute top-3 left-3 z-10 flex items-center gap-2">
                        // Search
                        <div class="relative">
                            <input
                                type="text"
                                placeholder="Search..."
                                class="text-xs bg-ctp-surface0 border border-ctp-surface2 rounded px-2 py-1 pl-6 pr-12 text-ctp-text placeholder-ctp-overlay0 focus:outline-none focus:border-ctp-blue transition-colors w-36"
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_search_query.set(value);
                                }
                                prop:value=move || search_query.get()
                            />
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-ctp-overlay0"
                                viewBox="0 0 20 20"
                                fill="currentColor"
                            >
                                <path fill-rule="evenodd" d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" clip-rule="evenodd"/>
                            </svg>
                            {move || {
                                let q = search_query.get();
                                let count = search_count.get();
                                if !q.is_empty() {
                                    view! {
                                        <span class="absolute right-1.5 top-1/2 -translate-y-1/2 flex items-center gap-1">
                                            <span class="text-[10px] text-ctp-overlay0">
                                                {count.to_string()}
                                            </span>
                                            <button
                                                class="text-ctp-overlay0 hover:text-ctp-red transition-colors"
                                                on:click=move |_| set_search_query.set(String::new())
                                                title="Clear search"
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 20 20" fill="currentColor">
                                                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd"/>
                                                </svg>
                                            </button>
                                        </span>
                                    }.into_any()
                                } else {
                                    view! { <span></span> }.into_any()
                                }
                            }}
                        </div>
                        // Language pills (only show if more than one language)
                        {move || {
                            let langs = available_languages.get();
                            if langs.len() <= 1 {
                                return Vec::new();
                            }
                            langs
                                .into_iter()
                                .map(|lang| {
                                    let lang_for_click = lang.clone();
                                    let is_selected = {
                                        let lang_check = lang.clone();
                                        Signal::derive(move || selected_language.get() == lang_check)
                                    };

                                    view! {
                                        <button
                                            class="text-xs px-2 py-0.5 rounded border transition-colors capitalize"
                                            class:bg-ctp-blue=move || is_selected.get()
                                            class:text-ctp-base=move || is_selected.get()
                                            class:border-ctp-blue=move || is_selected.get()
                                            class:bg-ctp-surface0=move || !is_selected.get()
                                            class:text-ctp-subtext0=move || !is_selected.get()
                                            class:border-ctp-surface2=move || !is_selected.get()
                                            class:hover:bg-ctp-surface1=move || !is_selected.get()
                                            on:click=move |_| {
                                                let current = selected_language.get_untracked();
                                                if current == lang_for_click {
                                                    set_selected_language.set(String::new());
                                                } else {
                                                    set_selected_language.set(lang_for_click.clone());
                                                }
                                            }
                                        >
                                            {lang}
                                        </button>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </div>
                    // Top-right overlay: layout indicator + depth controls + fit button
                    <div class="absolute top-3 right-3 z-10 flex items-center gap-2">
                        // Layout running indicator
                        <div
                            class="flex items-center gap-2 bg-ctp-surface0 border border-ctp-surface2 rounded-full px-3 py-1 text-xs text-ctp-subtext0 transition-opacity duration-200"
                            style:opacity=move || if layout_running.get() { "1" } else { "0" }
                            style:pointer-events="none"
                        >
                            <span class="inline-block w-2 h-2 rounded-full bg-ctp-blue animate-pulse"></span>
                            "Layouting..."
                        </div>
                        // Depth controls (only shown when focused)
                        <div
                            class="flex items-center gap-1 bg-ctp-surface0 border border-ctp-surface2 rounded px-2 py-1 transition-opacity duration-200"
                            style:opacity=move || if is_focused.get() { "1" } else { "0" }
                            style:pointer-events=move || if is_focused.get() { "auto" } else { "none" }
                        >
                            <Tooltip content="Neighborhood depth">
                                <span class="text-xs text-ctp-overlay0">"Depth"</span>
                            </Tooltip>
                            {[1u32, 2, 3].into_iter().map(|d| {
                                let is_active = Signal::derive(move || focus_depth.get() == d);
                                view! {
                                    <button
                                        class="text-xs px-1.5 py-0.5 rounded transition-colors"
                                        class:bg-ctp-blue=move || is_active.get()
                                        class:text-ctp-base=move || is_active.get()
                                        class:bg-ctp-surface1=move || !is_active.get()
                                        class:text-ctp-subtext0=move || !is_active.get()
                                        class:hover:bg-ctp-surface2=move || !is_active.get()
                                        on:click=move |_| set_focus_depth.set(d)
                                    >
                                        {d.to_string()}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                        // Fit to canvas button
                        <Tooltip content="Fit to canvas">
                            <button
                                class="p-1.5 rounded bg-ctp-surface0 border border-ctp-surface2 text-ctp-subtext0 hover:text-ctp-blue hover:bg-ctp-surface1 transition-colors"
                                on:click=move |_| zoom_to_fit(container_id)
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
                                    <path d="M3 4a1 1 0 011-1h4a1 1 0 010 2H5v3a1 1 0 01-2 0V4zM16 4a1 1 0 00-1-1h-4a1 1 0 100 2h3v3a1 1 0 102 0V4zM3 16a1 1 0 001 1h4a1 1 0 100-2H5v-3a1 1 0 10-2 0v4zM16 16a1 1 0 01-1 1h-4a1 1 0 110-2h3v-3a1 1 0 112 0v4z"/>
                                </svg>
                            </button>
                        </Tooltip>
                    </div>
                </div>

                // Side panel: detail card + legends
                <div class="w-64 flex flex-col gap-4">
                    // Selected node info card
                    <div class="bg-ctp-base border border-ctp-surface2 rounded-lg p-3">
                        <h4 class="text-xs font-semibold text-ctp-overlay0 mb-2">"Selected Node"</h4>
                        {move || {
                            selected_node.get().map(|info| {
                                view! {
                                    <div class="space-y-1 text-xs font-mono">
                                        <div class="flex items-center gap-2">
                                            <span class="text-ctp-blue font-semibold">{info.kind}</span>
                                            <span class="text-ctp-overlay0">{info.language}</span>
                                        </div>
                                        <p class="text-ctp-text break-all">{info.qualified_name.clone()}</p>
                                        <div class="flex items-center gap-1">
                                            <p class="text-ctp-overlay0 text-[10px] truncate" title=format!("{}:{}", info.file_path, info.start_line)>
                                                {format!("{}:{}", info.file_path, info.start_line)}
                                            </p>
                                            {
                                                let path = format!("{}:{}", info.file_path, info.start_line);
                                                let (path_copied, set_path_copied) = signal(false);
                                                view! {
                                                    <button
                                                        class="text-ctp-overlay0 hover:text-ctp-text flex-shrink-0 transition-colors"
                                                        title="Copy path"
                                                        on:click={
                                                            let path = path.clone();
                                                            move |_| {
                                                                copy_to_clipboard(&path);
                                                                set_path_copied.set(true);
                                                                set_timeout(
                                                                    move || set_path_copied.set(false),
                                                                    std::time::Duration::from_secs(2),
                                                                );
                                                            }
                                                        }
                                                    >
                                                        {move || {
                                                            if path_copied.get() {
                                                                view! {
                                                                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 text-ctp-green" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                                        <polyline points="20 6 9 17 4 12"></polyline>
                                                                    </svg>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                                                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                                                                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                                                                    </svg>
                                                                }.into_any()
                                                            }
                                                        }}
                                                    </button>
                                                }
                                            }
                                        </div>
                                    </div>
                                }.into_any()
                            }).unwrap_or_else(|| {
                                view! {
                                    <p class="text-ctp-overlay0 text-xs italic">"Click a node to see details"</p>
                                }.into_any()
                            })
                        }}
                    </div>

                    // Node kinds legend
                    <div class="bg-ctp-base border border-ctp-surface2 rounded-lg p-3">
                        <div class="flex items-center justify-between mb-2">
                            <h4 class="text-xs font-semibold text-ctp-overlay0">"Node Types"</h4>
                            <Tooltip content=move || if hide_tests.get() { "Show test symbols" } else { "Hide test symbols" }>
                                <button
                                    class="p-1 rounded border transition-colors"
                                    class:bg-ctp-surface1=move || hide_tests.get()
                                    class:border-ctp-surface2=move || hide_tests.get()
                                    class:text-ctp-overlay0=move || hide_tests.get()
                                    class:bg-ctp-surface0=move || !hide_tests.get()
                                    class:border-ctp-yellow=move || !hide_tests.get()
                                    class:text-ctp-yellow=move || !hide_tests.get()
                                    on:click=move |_| set_hide_tests.set(!hide_tests.get_untracked())
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M21 7 6.82 21.18a2.83 2.83 0 0 1-3.99-.01a2.83 2.83 0 0 1 0-4L17 3"/>
                                        <path d="m16 2 6 6"/>
                                        <path d="M12 16H4"/>
                                    </svg>
                                </button>
                            </Tooltip>
                        </div>
                        <div class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-ctp-subtext0">
                            {move || {
                                let kinds = legend_kinds.get();
                                let total = kinds.len();
                                kinds
                                    .into_iter()
                                    .map(|ki| {
                                        let kind = ki.kind.clone();
                                        let color = ki.color.clone();
                                        let kind_for_check = kind.clone();
                                        let kind_for_click = kind.clone();

                                        let is_active = {
                                            let kind_check = kind_for_check.clone();
                                            move || {
                                                let active = active_kinds.get();
                                                active.is_empty() || active.contains(&kind_check)
                                            }
                                        };

                                        view! {
                                            <button
                                                class="flex items-center gap-1 cursor-pointer transition-opacity"
                                                style:opacity=move || if is_active() { "1" } else { "0.3" }
                                                on:click=move |_| {
                                                    let mut kinds = active_kinds.get();
                                                    if kinds.contains(&kind_for_click) {
                                                        kinds.remove(&kind_for_click);
                                                    } else {
                                                        kinds.insert(kind_for_click.clone());
                                                    }
                                                    if kinds.len() == total {
                                                        kinds.clear();
                                                    }
                                                    set_active_kinds.set(kinds);
                                                }
                                            >
                                                <span
                                                    class="inline-block w-2 h-2 rounded-full"
                                                    style:background-color=color
                                                />
                                                {kind}
                                            </button>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </div>
                    </div>

                    // Edge types legend
                    <div class="bg-ctp-base border border-ctp-surface2 rounded-lg p-3">
                        <h4 class="text-xs font-semibold text-ctp-overlay0 mb-2">"Edge Types"</h4>
                        <div class="flex flex-wrap gap-x-3 gap-y-1 text-xs text-ctp-subtext0">
                            {move || {
                                let edge_types = legend_edge_types.get();
                                let total = edge_types.len();
                                edge_types
                                    .into_iter()
                                    .map(|ei| {
                                        let kind = ei.kind.clone();
                                        let color = ei.color.clone();
                                        let kind_for_check = kind.clone();
                                        let kind_for_click = kind.clone();

                                        let is_active = {
                                            let kind_check = kind_for_check.clone();
                                            move || {
                                                let active = active_edges.get();
                                                active.is_empty() || active.contains(&kind_check)
                                            }
                                        };

                                        view! {
                                            <button
                                                class="flex items-center gap-1 cursor-pointer transition-opacity"
                                                style:opacity=move || if is_active() { "1" } else { "0.3" }
                                                on:click=move |_| {
                                                    let mut edges = active_edges.get();
                                                    if edges.contains(&kind_for_click) {
                                                        edges.remove(&kind_for_click);
                                                    } else {
                                                        edges.insert(kind_for_click.clone());
                                                    }
                                                    if edges.len() == total {
                                                        edges.clear();
                                                    }
                                                    set_active_edges.set(edges);
                                                }
                                            >
                                                <span
                                                    class="inline-block w-3 h-0.5 rounded"
                                                    style:background-color=color
                                                />
                                                {kind}
                                            </button>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(Clone)]
enum GraphState {
    Loading,
    Ready(String),
    Loaded,
    NoAnalysis,
    Error(String),
}

#[derive(Clone, serde::Deserialize)]
struct KindInfo {
    kind: String,
    color: String,
}

#[derive(Clone, serde::Deserialize)]
struct SelectedNodeInfo {
    #[allow(dead_code)]
    label: String,
    #[serde(rename = "qualifiedName")]
    qualified_name: String,
    kind: String,
    language: String,
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(rename = "startLine")]
    start_line: i64,
}
