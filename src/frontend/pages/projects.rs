use leptos::prelude::*;

use crate::api::projects;
use crate::models::Project;

#[component]
pub fn Projects() -> impl IntoView {
    // Create a local resource that fetches projects (no Send requirement for WASM)
    let projects_resource =
        LocalResource::new(|| async move { projects::list(Some(20), None).await });

    view! {
        <div class="container mx-auto p-6">
            <h2 class="text-3xl font-bold text-ctp-text mb-6">"Projects"</h2>

            <Suspense fallback=move || {
                view! { <p class="text-ctp-subtext0">"Loading projects..."</p> }
            }>
                {move || {
                    projects_resource
                        .get()
                        .map(|result| match result.as_ref() {
                            Ok(paginated) => {
                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            "No projects found. Create one to get started!"
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="grid gap-4">
                                            {paginated
                                                .items
                                                .iter()
                                                .map(|project| {
                                                    view! { <ProjectCard project=project.clone()/> }
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
                                        <p class="text-ctp-red font-semibold">"Error loading projects"</p>
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
fn ProjectCard(project: Project) -> impl IntoView {
    view! {
        <div class="bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors">
            <div class="flex justify-between items-start mb-2">
                <h3 class="text-xl font-semibold text-ctp-text">{project.title}</h3>
                <span class="text-xs text-ctp-overlay0">{project.id}</span>
            </div>

            {project
                .description
                .as_ref()
                .map(|desc| {
                    view! { <p class="text-ctp-subtext0 text-sm mb-3">{desc.clone()}</p> }
                })}

            {(!project.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-2">
                            {project
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

            <div class="flex justify-between text-xs text-ctp-overlay0 mt-3">
                <span>"Created: " {project.created_at}</span>
                <span>"Updated: " {project.updated_at}</span>
            </div>
        </div>
    }
}
