use leptos::prelude::*;

use crate::api::repos;
use crate::models::Repo;

#[component]
pub fn Repos() -> impl IntoView {
    // Create a local resource that fetches repos (no Send requirement for WASM)
    let repos_resource =
        LocalResource::new(|| async move { repos::list(Some(20), None, None).await });

    view! {
        <div class="container mx-auto p-6">
            <h2 class="text-3xl font-bold text-ctp-text mb-6">"Repositories"</h2>

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading repositories..."</p> }
            }>
                {move || {
                    repos_resource
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(paginated) => {
                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            "No repositories found. Add one to get started!"
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="grid gap-4">
                                            {paginated
                                                .items
                                                .iter()
                                                .map(|repo| {
                                                    view! { <RepoCard repo=repo.clone()/> }
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
                                        <p class="text-ctp-red font-semibold">"Error loading repositories"</p>
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
fn RepoCard(repo: Repo) -> impl IntoView {
    view! {
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors">
            <div class="flex justify-between items-start mb-2">
                <h3 class="text-xl font-semibold text-ctp-text break-all">{repo.remote.clone()}</h3>
                <span class="text-xs text-ctp-overlay0 ml-2 flex-shrink-0">{repo.id}</span>
            </div>

            {repo
                .path
                .as_ref()
                .map(|p| {
                    view! {
                        <p class="text-ctp-subtext0 text-sm mb-3 font-mono">
                            <span class="text-ctp-overlay1">"Path: "</span>
                            {p.clone()}
                        </p>
                    }
                })}

            {(!repo.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-2">
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

            <div class="text-xs text-ctp-overlay0 mt-3">
                <span>"Created: " {repo.created_at}</span>
            </div>
        </div>
    }
}
