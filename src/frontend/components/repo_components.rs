use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::components::CopyableId;
use crate::models::Repo;

#[wasm_bindgen(
    inline_js = "export function copy_to_clipboard(text) { navigator.clipboard.writeText(text); }"
)]
extern "C" {
    fn copy_to_clipboard(text: &str);
}

#[component]
pub fn RepoCard(repo: Repo) -> impl IntoView {
    view! {
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors">
            <div class="flex items-start gap-3">
                <CopyableId id=repo.id.clone()/>
                <div class="flex-1 min-w-0">
                    <h3 class="text-xl font-semibold text-ctp-text break-all mb-2">{repo.remote.clone()}</h3>

                    {repo.path.clone().map(|p| {
                        let (copied, set_copied) = signal(false);
                        let path_clone = p.clone();
                        let path_clone2 = p.clone();
                        let path_for_display = p.clone();

                        let do_copy_text = move |ev: leptos::ev::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            copy_to_clipboard(&path_clone);
                            set_copied.set(true);
                            set_timeout(
                                move || {
                                    set_copied.set(false);
                                },
                                std::time::Duration::from_secs(2),
                            );
                        };

                        let do_copy_button = move |ev: leptos::ev::MouseEvent| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            copy_to_clipboard(&path_clone2);
                            set_copied.set(true);
                            set_timeout(
                                move || {
                                    set_copied.set(false);
                                },
                                std::time::Duration::from_secs(2),
                            );
                        };

                        view! {
                            <div class="flex items-center gap-2 mb-3">
                                <p
                                    class="text-ctp-subtext0 text-sm font-mono truncate cursor-pointer hover:text-ctp-blue transition-colors flex-1 min-w-0"
                                    title=p.clone()
                                    on:click=do_copy_text
                                >
                                    <span class="text-ctp-overlay1">"Path: "</span>
                                    {path_for_display.clone()}
                                </p>
                                <button
                                    on:click=do_copy_button
                                    class="flex-shrink-0 text-ctp-overlay0 hover:text-ctp-blue transition-colors text-sm"
                                    title="Copy to clipboard"
                                >
                                    {move || {
                                        if copied.get() {
                                            "✓"
                                        } else {
                                            "📋"
                                        }
                                    }}

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
        </div>
    }
}
