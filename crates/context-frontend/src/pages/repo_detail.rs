use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;

use crate::api::{ApiClientError, repos};
use crate::components::EntityHeader;
use crate::models::{Repo, UpdateMessage};
use crate::tabs::{TabScope, should_refetch};
use crate::utils::extract_repo_name;
use crate::websocket::use_websocket_updates;

#[wasm_bindgen(
    inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }"
)]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

#[component]
pub fn RepoDetail(scope: TabScope) -> impl IntoView {
    // The tab's entity id never changes for the tab's lifetime.
    let repo_id = StoredValue::new_local(scope.entity_id().to_string());

    let (repo_data, set_repo_data) = signal(None::<Result<Repo, ApiClientError>>);

    // WebSocket updates - refetch trigger
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);
    let ws_updates = use_websocket_updates();

    // Watch for WebSocket repo updates for THIS repo. A mounted surface runs its fetch once, so
    // loaded data is the gate: a hidden tab keeps its own repo live, and another repo's message
    // refetches nothing (decision 15).
    Effect::new(move || {
        let Some(msg) = ws_updates.get() else {
            return;
        };
        let current_id = repo_id.get_value();
        if current_id.is_empty() {
            return;
        }
        match &msg {
            UpdateMessage::RepoUpdated {
                repo_id: updated_id,
            } if should_refetch(
                repo_data.get_untracked().is_some(),
                false,
                *updated_id == current_id,
            ) =>
            {
                set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
            }
            UpdateMessage::RepoDeleted {
                repo_id: deleted_id,
            } if *deleted_id == current_id => {
                // No route exists for this surface any more, and closing the tab is the
                // deletion reaction's job: this handler writes nothing.
                web_sys::console::log_1(
                    &format!("Repo {} deleted via WebSocket, tab stays open", deleted_id).into(),
                );
            }
            _ => {}
        }
    });

    // Fetch repo when params change or refetch trigger fires
    Effect::new(move || {
        let id = repo_id.get_value();
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
                <div class="flex items-start justify-between mb-2">
                    <div class="flex items-center gap-3">
                        <EntityHeader id=repo.id.clone() title=display_name.clone()/>
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
    }
}
