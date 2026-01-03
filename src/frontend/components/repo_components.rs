use leptos::prelude::*;

use crate::components::CopyableId;
use crate::models::Repo;

#[component]
pub fn RepoCard(repo: Repo) -> impl IntoView {
    view! {
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors">
            <div class="flex items-start gap-3">
                <CopyableId id=repo.id.clone()/>
                <div class="flex-1 min-w-0">
                    <h3 class="text-xl font-semibold text-ctp-text break-all mb-2">{repo.remote.clone()}</h3>

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
            </div>
        </div>
    }
}
