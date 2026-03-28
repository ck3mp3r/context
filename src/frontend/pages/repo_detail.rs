use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::api::{ApiClientError, projects, repos};
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

            // Graph placeholder
            <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-6">
                <h3 class="text-lg font-semibold text-ctp-text mb-4">"Code Graph"</h3>
                <div class="flex items-center justify-center h-64 border border-dashed border-ctp-surface2 rounded-lg">
                    <p class="text-ctp-overlay0 text-sm">"Graph visualization coming soon"</p>
                </div>
            </div>
        </div>
    }
}
