use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use wasm_bindgen::prelude::*;

use crate::api::{ApiClientError, graph, projects, repos};
use crate::components::{Breadcrumb, BreadcrumbItem, CopyableId};
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
}

// =============================================================================
// GraphViewer component
// =============================================================================

#[component]
fn GraphViewer(repo_id: String) -> impl IntoView {
    let container_id = "graph-canvas";
    let (graph_state, set_graph_state) = signal(GraphState::Loading);
    let (view, set_view) = signal("full".to_string());
    let (include_tests, set_include_tests) = signal(false);
    let (active_kinds, set_active_kinds) = signal(std::collections::HashSet::<String>::new());

    let repo_id_for_fetch = repo_id.clone();

    // Fetch graph data and initialize Sigma.js
    Effect::new(move || {
        let current_view = view.get();
        let tests = include_tests.get();
        let repo_id = repo_id_for_fetch.clone();

        set_graph_state.set(GraphState::Loading);

        spawn_local(async move {
            match graph::get_repo_graph(&repo_id, Some(&current_view), tests).await {
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
                        set_graph_state.set(GraphState::Loaded);
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
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold text-ctp-text">"Code Graph"</h3>
                <div class="flex items-center gap-3">
                    <label class="flex items-center gap-1.5 text-ctp-subtext0 text-sm cursor-pointer">
                        <input
                            type="checkbox"
                            class="accent-ctp-blue"
                            prop:checked=move || include_tests.get()
                            on:change=move |ev| {
                                let checked = event_target::<web_sys::HtmlInputElement>(&ev)
                                    .checked();
                                set_include_tests.set(checked);
                            }
                        />
                        "Tests"
                    </label>
                    <select
                        class="bg-ctp-base border border-ctp-surface2 text-ctp-text text-sm rounded px-2 py-1"
                        on:change=move |ev| {
                            set_view.set(event_target_value(&ev));
                        }
                    >
                        <option value="full" selected=true>"Full"</option>
                        <option value="calls">"Calls"</option>
                        <option value="inherits">"Inherits"</option>
                        <option value="references">"References"</option>
                        <option value="contains">"Contains"</option>
                    </select>
                    <button
                        class="bg-ctp-base border border-ctp-surface2 text-ctp-subtext0 text-sm rounded px-2 py-1 hover:text-ctp-blue hover:border-ctp-blue transition-colors"
                        on:click=move |_| zoom_to_fit(container_id)
                        title="Fit to screen"
                    >
                        "Fit"
                    </button>
                </div>
            </div>

            // Graph canvas container (always in DOM so initGraph can find it)
            <div
                id=container_id
                class="w-full bg-ctp-mantle rounded-lg relative"
                style="height: 500px;"
            >
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

            // Legend
            <div class="flex flex-wrap gap-x-4 gap-y-1 mt-3 text-xs text-ctp-subtext0">
                <LegendItem color="#89b4fa" label="function" kind="function" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#a6e3a1" label="struct" kind="struct" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#f9e2af" label="enum" kind="enum" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#cba6f7" label="trait" kind="trait" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#fab387" label="module" kind="mod" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#f2cdcd" label="const" kind="constant" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#f38ba8" label="static" kind="static" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#94e2d5" label="type alias" kind="type_alias" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
                <LegendItem color="#f5c2e7" label="macro" kind="macro" active_kinds=active_kinds set_active_kinds=set_active_kinds/>
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

#[component]
fn LegendItem(
    color: &'static str,
    label: &'static str,
    kind: &'static str,
    active_kinds: ReadSignal<std::collections::HashSet<String>>,
    set_active_kinds: WriteSignal<std::collections::HashSet<String>>,
) -> impl IntoView {
    let kind_str = kind.to_string();
    let kind_for_check = kind_str.clone();

    let is_active = move || {
        let kinds = active_kinds.get();
        kinds.is_empty() || kinds.contains(&kind_for_check)
    };

    view! {
        <button
            class="flex items-center gap-1.5 cursor-pointer transition-opacity"
            style:opacity=move || if is_active() { "1" } else { "0.3" }
            on:click=move |_| {
                let mut kinds = active_kinds.get();
                if kinds.contains(&kind_str) {
                    kinds.remove(&kind_str);
                } else {
                    kinds.insert(kind_str.clone());
                }
                // If all are selected, clear to mean "show all"
                if kinds.len() == 9 {
                    kinds.clear();
                }
                set_active_kinds.set(kinds);
            }
        >
            <span
                class="inline-block w-2.5 h-2.5 rounded-full"
                style:background-color=color
            />
            {label}
        </button>
    }
}
