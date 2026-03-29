use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::prelude::*;

use crate::api::{ApiClientError, graph, projects, repos};
use crate::components::{Breadcrumb, BreadcrumbItem, CopyableId, PillColor, PillToggle};
use crate::models::{Project, Repo, UpdateMessage};
use crate::utils::extract_repo_name;
use crate::websocket::use_websocket_updates;

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
                        // Navigate back to project repos tab or standalone repos list
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
                            // Project context: Projects > Project > Repos > repo-name
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
                            // Project context but project not loaded yet - show minimal
                            return None;
                        } else {
                            // Standalone: Repos > repo-name
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
                        <div class="flex items-center gap-3 mb-2">
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

                        <div class="text-xs text-ctp-overlay0 mt-4">
                            <span>"Created: " {repo.created_at}</span>
                        </div>
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
}

// =============================================================================
// GraphViewer component
// =============================================================================

#[component]
fn GraphViewer(repo_id: String) -> impl IntoView {
    let container_id = "graph-canvas";
    let (graph_state, set_graph_state) = signal(GraphState::Loading);
    let (active_edges, set_active_edges) = signal(std::collections::HashSet::<String>::new()); // empty = all
    let (include_tests, set_include_tests) = signal(false);
    let (active_kinds, set_active_kinds) = signal(std::collections::HashSet::<String>::new());
    let (legend_kinds, set_legend_kinds) = signal(Vec::<KindInfo>::new());
    let (legend_edge_types, set_legend_edge_types) = signal(Vec::<KindInfo>::new());
    let (available_languages, set_available_languages) = signal(Vec::<String>::new());
    let (selected_language, set_selected_language) = signal(String::new()); // empty = all
    let (is_fetching, set_is_fetching) = signal(false);
    let (search_query, set_search_query) = signal(String::new());
    let (search_count, set_search_count) = signal(0u32);

    let repo_id_for_fetch = repo_id.clone();

    // Fetch graph data and initialize Sigma.js
    Effect::new(move || {
        let edges = active_edges.get();
        let tests = include_tests.get();
        let lang = selected_language.get();
        let repo_id = repo_id_for_fetch.clone();
        let lang_param = if lang.is_empty() { None } else { Some(lang) };
        let edges_param = if edges.is_empty() {
            None
        } else {
            Some(edges.into_iter().collect::<Vec<_>>().join(","))
        };

        set_is_fetching.set(true);
        spawn_local(async move {
            match graph::get_repo_graph(
                &repo_id,
                edges_param.as_deref(),
                tests,
                lang_param.as_deref(),
            )
            .await
            {
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
                        // Extract kinds from the graph for the dynamic legend
                        let kinds_json = get_kinds(container_id);
                        if let Ok(kinds) = serde_json::from_str::<Vec<KindInfo>>(&kinds_json) {
                            set_legend_kinds.set(kinds);
                        }
                        // Extract edge types from the graph for the edge legend
                        let edge_types_json = get_edge_types(container_id);
                        if let Ok(edge_types) =
                            serde_json::from_str::<Vec<KindInfo>>(&edge_types_json)
                        {
                            set_legend_edge_types.set(edge_types);
                        }
                        // Extract available languages (only when unfiltered)
                        if selected_language.get_untracked().is_empty() {
                            let langs_json = get_languages(container_id);
                            if let Ok(langs) = serde_json::from_str::<Vec<String>>(&langs_json) {
                                set_available_languages.set(langs);
                            }
                        }
                        // Reset active filter on new data
                        set_active_kinds.set(std::collections::HashSet::new());
                        set_search_query.set(String::new());
                        set_graph_state.set(GraphState::Loaded);
                        set_is_fetching.set(false);
                    } else {
                        set_graph_state.set(GraphState::Error(
                            "Failed to initialize graph renderer".to_string(),
                        ));
                        set_is_fetching.set(false);
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
        zoom_to_fit(container_id);
    });

    // Push search query to JS when it changes
    Effect::new(move || {
        let query = search_query.get();
        let count = search_nodes(container_id, &query);
        set_search_count.set(count);
    });

    let is_visible = move || {
        !matches!(
            graph_state.get(),
            GraphState::NoAnalysis | GraphState::Loading
        )
    };

    view! {
        <div
            class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-6"
            style:display=move || if is_visible() { "" } else { "none" }
        >
            // Controls row
            <div class="flex flex-col gap-3 mb-4">
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-3">
                        <h3 class="text-lg font-semibold text-ctp-text">"Code Graph"</h3>
                        // Search input
                        <div class="relative">
                            <input
                                type="text"
                                placeholder="Search nodes..."
                                class="text-xs bg-ctp-base border border-ctp-surface2 rounded-full px-3 py-1 pl-7 pr-14 text-ctp-text placeholder-ctp-overlay0 focus:outline-none focus:border-ctp-blue transition-colors w-44"
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_search_query.set(value);
                                }
                                prop:value=move || search_query.get()
                            />
                            // Search icon
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3 h-3 text-ctp-overlay0"
                                viewBox="0 0 20 20"
                                fill="currentColor"
                            >
                                <path fill-rule="evenodd" d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" clip-rule="evenodd"/>
                            </svg>
                            // Match count + clear button
                            {move || {
                                let q = search_query.get();
                                let count = search_count.get();
                                if !q.is_empty() {
                                    view! {
                                        <span class="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
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
                    </div>
                    // Fit button — icon with subtle background
                    <button
                        class="p-1.5 rounded bg-ctp-surface1 text-ctp-subtext0 hover:text-ctp-blue hover:bg-ctp-surface2 transition-colors flex-shrink-0"
                        on:click=move |_| zoom_to_fit(container_id)
                        title="Fit to canvas"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
                            <path d="M3 4a1 1 0 011-1h4a1 1 0 010 2H5v3a1 1 0 01-2 0V4zM16 4a1 1 0 00-1-1h-4a1 1 0 100 2h3v3a1 1 0 102 0V4zM3 16a1 1 0 001 1h4a1 1 0 100-2H5v-3a1 1 0 10-2 0v4zM16 16a1 1 0 01-1 1h-4a1 1 0 110-2h3v-3a1 1 0 112 0v4z"/>
                        </svg>
                    </button>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    // Edge type toggle pills
                    <span class="text-xs text-ctp-overlay0">"Edges:"</span>
                    {[
                        ("Calls", "Calls", PillColor::Blue, "Function/method call relationships"),
                        ("Uses", "Uses", PillColor::Yellow, "Symbol usage references (e.g. constants, types)"),
                        ("Returns", "Ret", PillColor::Green, "Function return type relationships"),
                        ("Accepts", "Acc", PillColor::Teal, "Function parameter type relationships"),
                        ("FieldType", "Field", PillColor::Pink, "Struct/type field type relationships"),
                        ("TypeAnnotation", "Type", PillColor::Peach, "Type annotation references (e.g. interface methods)"),
                        ("Inherits", "Inh", PillColor::Mauve, "Trait/interface implementation relationships"),
                        ("Import", "Imp", PillColor::Sapphire, "Import/use dependencies between modules"),
                        ("Contains", "Cont", PillColor::Blue, "Parent-child containment (e.g. struct contains method)"),
                    ].into_iter().map(|(value, label, color, tooltip)| {
                        let value = value.to_string();
                        let value_for_click = value.clone();
                        let is_active = Signal::derive({
                            let v = value.clone();
                            move || {
                                let edges = active_edges.get();
                                edges.is_empty() || edges.contains(&v)
                            }
                        });
                        view! {
                            <PillToggle
                                label=label
                                active=is_active
                                color=color
                                tooltip=tooltip.to_string()
                                on_click=move |_| {
                                    let mut edges = active_edges.get();
                                    if edges.is_empty() {
                                        // All are shown — click one to solo it
                                        edges.insert(value_for_click.clone());
                                    } else if edges.contains(&value_for_click) && edges.len() == 1 {
                                        // Already solo — reset to all
                                        edges.clear();
                                    } else if edges.contains(&value_for_click) {
                                        // Remove this edge type
                                        edges.remove(&value_for_click);
                                    } else {
                                        // Add this edge type
                                        edges.insert(value_for_click.clone());
                                    }
                                    set_active_edges.set(edges);
                                }
                            />
                        }
                    }).collect::<Vec<_>>()}

                    // Tests toggle pill
                    <PillToggle
                        label="Tests"
                        active=Signal::derive(move || include_tests.get())
                        color=PillColor::Mauve
                        on_click=move |_| set_include_tests.set(!include_tests.get_untracked())
                    />

                    // Language pills (only if multiple languages)
                    {move || {
                        let langs = available_languages.get();
                        if langs.len() > 1 {
                            let mut items = vec![("".to_string(), "All".to_string())];
                            items.extend(langs.into_iter().map(|l| (l.clone(), l)));
                            view! {
                                <span class="text-xs text-ctp-overlay0 ml-1">"Lang:"</span>
                                {items.into_iter().map(|(value, label)| {
                                    let value_for_click = value.clone();
                                    let is_active = Signal::derive({
                                        let v = value.clone();
                                        move || selected_language.get() == v
                                    });
                                    view! {
                                        <PillToggle
                                            label=label
                                            active=is_active
                                            color=PillColor::Green
                                            on_click=move |_| set_selected_language.set(value_for_click.clone())
                                        />
                                    }
                                }).collect::<Vec<_>>()}
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }
                    }}
                </div>
            </div>

            // Graph canvas container (always in DOM so initGraph can find it)
            <div
                id=container_id
                class="w-full bg-ctp-mantle rounded-lg relative"
                style="height: 500px;"
            >
                // Activity indicator
                <div
                    class="absolute top-3 right-3 z-10 flex items-center gap-2 bg-ctp-surface0 border border-ctp-surface2 rounded-full px-3 py-1 text-xs text-ctp-subtext0 transition-opacity duration-200"
                    style:opacity=move || if is_fetching.get() { "1" } else { "0" }
                    style:pointer-events="none"
                >
                    <span class="inline-block w-2 h-2 rounded-full bg-ctp-blue animate-pulse"></span>
                    "Loading..."
                </div>
                {move || match graph_state.get() {
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
            </div>

            // Legend: node kinds
            <div class="flex flex-wrap gap-x-4 gap-y-1 mt-3 text-xs text-ctp-subtext0">
                <span class="text-ctp-overlay0 font-medium">"Nodes:"</span>
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
                                    class="flex items-center gap-1.5 cursor-pointer transition-opacity"
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
                                        class="inline-block w-2.5 h-2.5 rounded-full"
                                        style:background-color=color
                                    />
                                    {kind}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
            // Legend: edge types
            <div class="flex flex-wrap gap-x-4 gap-y-1 mt-1 text-xs text-ctp-subtext0">
                <span class="text-ctp-overlay0 font-medium">"Edges:"</span>
                {move || {
                    legend_edge_types.get()
                        .into_iter()
                        .map(|ei| {
                            let color = ei.color.clone();
                            view! {
                                <span class="flex items-center gap-1.5">
                                    <span
                                        class="inline-block w-4 h-0.5 rounded"
                                        style:background-color=color
                                    />
                                    {ei.kind}
                                </span>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
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
