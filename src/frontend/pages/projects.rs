use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, projects};
use crate::components::CopyableId;
use crate::models::{Paginated, Project};

#[component]
pub fn Projects() -> impl IntoView {
    let (projects_data, set_projects_data) =
        signal(None::<Result<Paginated<Project>, ApiClientError>>);

    // Fetch projects on mount
    Effect::new(move || {
        spawn_local(async move {
            let result = projects::list(Some(100), None).await;
            set_projects_data.set(Some(result));
        });
    });

    view! {
        <div class="container mx-auto p-6">
            <div class="mb-8">
                <h2 class="text-3xl font-bold text-ctp-text mb-2">"Projects"</h2>
                <p class="text-ctp-subtext0">"Select a project to view task lists, notes, and repos"</p>
            </div>

            {move || match projects_data.get() {
                None => {
                    view! {
                        <div class="text-center py-12">
                            <p class="text-ctp-subtext0">"Loading projects..."</p>
                        </div>
                    }
                        .into_any()
                }
                Some(Ok(paginated)) => {
                    if paginated.items.is_empty() {
                        view! {
                            <div class="text-center py-12">
                                <p class="text-ctp-subtext0">"No projects found. Create one to get started!"</p>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 auto-rows-fr">
                                {paginated
                                    .items
                                    .iter()
                                    .map(|project| {
                                        let project_id = project.id.clone();
                                        let project_title = project.title.clone();
                                        let project_description = project.description.clone();
                                        let project_tags = project.tags.clone();
                                        view! {
                                            <div class="relative bg-ctp-surface0 rounded-lg p-6 border border-ctp-surface1 hover:border-ctp-blue transition-colors flex flex-col h-full min-h-[280px]">
                                                <div class="absolute top-2 right-2">
                                                    <CopyableId id=project_id.clone()/>
                                                </div>
                                                <a
                                                    href=format!("/projects/{}", project_id)
                                                    class="flex flex-col h-full"
                                                >
                                                    <h3 class="text-xl font-semibold text-ctp-text mb-2 pr-20">
                                                        {project_title}
                                                    </h3>
                                                {project_description
                                                    .as_ref()
                                                    .map(|desc| {
                                                        view! {
                                                            <p class="text-sm text-ctp-subtext0 mb-4">
                                                                {desc.clone()}
                                                            </p>
                                                        }
                                                    })}

                                                <div class="flex-grow"></div>

                                                {(!project_tags.is_empty())
                                                    .then(|| {
                                                        view! {
                                                            <div class="flex flex-wrap gap-2 mt-auto">
                                                                {project_tags
                                                                    .iter()
                                                                    .map(|tag| {
                                                                        view! {
                                                                            <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-2 py-1 rounded">
                                                                                {tag.clone()}
                                                                            </span>
                                                                        }
                                                                    })
                                                                    .collect::<Vec<_>>()}
                                                            </div>
                                                        }
                                                    })}
                                                </a>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </div>
                        }
                            .into_any()
                    }
                }
                Some(Err(err)) => {
                    view! {
                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                            <p class="text-ctp-red font-semibold">"Error loading projects"</p>
                            <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                        </div>
                    }
                        .into_any()
                }
            }}

        </div>
    }
}
