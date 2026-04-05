use leptos::prelude::*;
use leptos_router::components::A;
use wasm_bindgen::prelude::*;

use crate::components::CopyableId;
use crate::models::Repo;
use crate::utils::extract_repo_name;

#[wasm_bindgen(
    inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }"
)]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

#[component]
pub fn RepoCard(
    repo: Repo,
    #[prop(optional)] project_id: Option<String>,
    #[prop(optional)] current_query: Option<String>,
    #[prop(optional)] breadcrumb_name: Option<String>,
) -> impl IntoView {
    let display_name = extract_repo_name(&repo.remote);
    let remote_url = repo.remote.clone();
    let detail_href = if let Some(ref pid) = project_id {
        format!("/projects/{}/repos/{}", pid, repo.id)
    } else {
        format!("/repos/{}", repo.id)
    };

    let page_state = use_context::<crate::breadcrumb_state::BreadcrumbPageState>();

    view! {
        <A
            href=detail_href
            attr:class="block bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors cursor-pointer"
            on:click=move |_| {
                if let (Some(state), Some(query), Some(name)) =
                    (page_state.as_ref(), &current_query, &breadcrumb_name)
                {
                    state.set_query(name, query);
                }
            }
        >
            <div class="flex items-start gap-3">
                <CopyableId id=repo.id.clone()/>
                <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2 mb-2">
                        <h3 class="text-xl font-semibold text-ctp-text truncate" title=remote_url.clone()>
                            {display_name}
                        </h3>
                        <a
                            href=remote_url
                            target="_blank"
                            rel="noopener noreferrer"
                            class="flex-shrink-0 text-ctp-overlay0 hover:text-ctp-blue transition-colors"
                            title="Open remote"
                            on:click=|ev: leptos::ev::MouseEvent| ev.stop_propagation()
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
                                <path d="M11 3a1 1 0 100 2h2.586l-6.293 6.293a1 1 0 101.414 1.414L15 6.414V9a1 1 0 102 0V4a1 1 0 00-1-1h-5z"/>
                                <path d="M5 5a2 2 0 00-2 2v8a2 2 0 002 2h8a2 2 0 002-2v-3a1 1 0 10-2 0v3H5V7h3a1 1 0 000-2H5z"/>
                            </svg>
                        </a>
                    </div>

                    {repo.path.clone().map(|p| {
                        let (copied, set_copied) = signal(false);
                        let path_for_copy = p.clone();
                        let path_for_title = p.clone();
                        let path_for_display = p.clone();

                        let do_copy = move |ev: leptos::ev::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            copy_to_clipboard(&path_for_copy);
                            set_copied.set(true);
                            set_timeout(
                                move || set_copied.set(false),
                                std::time::Duration::from_secs(2),
                            );
                        };

                        let do_copy_btn = do_copy.clone();

                        view! {
                            <div class="flex items-center gap-2 mb-3">
                                <p
                                    class="text-ctp-subtext0 text-sm font-mono truncate cursor-pointer hover:text-ctp-blue transition-colors flex-1 min-w-0"
                                    title=path_for_title
                                    on:click=do_copy.clone()
                                >
                                    <span class="text-ctp-overlay1">"Path: "</span>
                                    {path_for_display}
                                </p>
                                <button
                                    on:click=do_copy_btn
                                    class="flex-shrink-0 text-ctp-overlay0 hover:text-ctp-blue transition-colors text-sm"
                                    title="Copy to clipboard"
                                >
                                    {move || if copied.get() { "\u{2713}" } else { "\u{1F4CB}" }}
                                </button>
                            </div>
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
        </A>
    }
}
