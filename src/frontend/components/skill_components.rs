use leptos::prelude::*;
use pulldown_cmark::{Options, Parser, html};
use thaw::*;

use crate::api::skills;
use crate::components::CopyableId;
use crate::models::{Skill, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn SkillCard(
    skill: Skill,
    #[prop(optional)] on_click: Option<Callback<String>>,
) -> impl IntoView {
    // Create a preview of the description (first 200 chars)
    let preview_content = if skill.description.chars().count() > 200 {
        let truncated: String = skill.description.chars().take(200).collect();
        format!("{}...", truncated)
    } else {
        skill.description.clone()
    };

    let skill_id = skill.id.clone();
    let href = if on_click.is_some() {
        "#".to_string()
    } else {
        format!("/skills/{}", skill.id)
    };

    view! {
        <div class="relative w-full">
            <div class="relative bg-ctp-surface0 border border-ctp-surface1 rounded-lg p-4 hover:border-ctp-blue transition-colors flex flex-col"
                 style="z-index: 2; width: 100%; height: 100%;">
            <a
                href=href
                on:click=move |ev| {
                    if let Some(callback) = on_click {
                        ev.prevent_default();
                        callback.run(skill_id.clone());
                    }
                }

                class="flex flex-col h-full"
            >
                <div class="flex items-start gap-2 mb-2">
                    <div class="flex-shrink-0">
                        <CopyableId id=skill.id.clone()/>
                    </div>
                    <h3 class="flex-1 min-w-0 break-words text-xl font-semibold text-ctp-text">{skill.name.clone()}</h3>
                </div>

            <div class="relative flex-grow mb-4">
                <div class="text-ctp-subtext0 text-sm leading-relaxed overflow-hidden" style="max-height: 6rem;">
                    {preview_content}
                </div>
                <div class="absolute bottom-0 left-0 right-0 h-16 bg-gradient-to-t from-ctp-surface0 to-transparent pointer-events-none"></div>
            </div>

            <div class="mt-auto">
            {(!skill.tags.is_empty())
                .then(|| {
                    view! {
                        <div class="flex flex-wrap gap-2 mb-2">
                            {skill
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

            <div class="flex justify-between text-xs text-ctp-overlay0">
                <span>"Created: " {skill.created_at}</span>
                <span>"Updated: " {skill.updated_at}</span>
            </div>
            </div>
            </a>
            </div>
        </div>
    }
}

#[component]
pub fn SkillDetailModal(skill_id: ReadSignal<String>, open: RwSignal<bool>) -> impl IntoView {
    use leptos::task::spawn_local;

    let (skill_data, set_skill_data) = signal(None::<Skill>);
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);

    // WebSocket updates
    let ws_updates = use_websocket_updates();
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates for this specific skill
    Effect::new(move || {
        let current_skill_id = skill_id.get();
        if let Some(UpdateMessage::SkillUpdated {
            skill_id: updated_id,
        }) = ws_updates.get()
            && updated_id == current_skill_id
        {
            web_sys::console::log_1(
                &format!(
                    "Skill {} updated via WebSocket, refetching...",
                    current_skill_id
                )
                .into(),
            );
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
        if let Some(UpdateMessage::SkillDeleted {
            skill_id: deleted_id,
        }) = ws_updates.get()
            && deleted_id == current_skill_id
        {
            web_sys::console::log_1(
                &format!(
                    "Skill {} deleted via WebSocket, closing modal...",
                    current_skill_id
                )
                .into(),
            );
            open.set(false);
        }
    });

    // Fetch skill when modal opens or skill_id changes or WebSocket update
    Effect::new(move || {
        let current_skill_id = skill_id.get();
        let is_open = open.get();
        let trigger = refetch_trigger.get();

        if is_open && !current_skill_id.is_empty() {
            set_loading.set(true);
            set_error.set(None);

            web_sys::console::log_1(
                &format!(
                    "Fetching skill details: id={}, trigger={}",
                    current_skill_id, trigger
                )
                .into(),
            );

            spawn_local(async move {
                match skills::get(&current_skill_id).await {
                    Ok(skill) => {
                        web_sys::console::log_1(&format!("Loaded skill: {}", skill.name).into());
                        set_skill_data.set(Some(skill));
                        set_loading.set(false);
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error loading skill: {}", e).into());
                        set_error.set(Some(e.to_string()));
                        set_loading.set(false);
                    }
                }
            });
        }
    });

    view! {
        <OverlayDrawer
            open
            position=DrawerPosition::Right
            class="skill-detail-drawer"
        >
            <DrawerBody class="h-full overflow-hidden p-0">
                <div class="h-full">
                    {move || {
                        if loading.get() {
                            view! {
                                <div class="flex justify-center items-center p-8">
                                    <Spinner/>
                                </div>
                            }.into_any()
                        } else if let Some(err) = error.get() {
                            view! {
                                <div class="p-6">
                                    // Close button
                                    <button
                                        on:click=move |_| open.set(false)
                                        class="absolute top-4 right-4 text-ctp-overlay0 hover:text-ctp-text text-2xl leading-none px-2 z-10"
                                    >
                                        "✕"
                                    </button>
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading skill"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err}</p>
                                    </div>
                                </div>
                            }.into_any()
                        } else if let Some(skill) = skill_data.get() {
                            view! {
                                <div class="p-6">
                                    // Close button
                                    <button
                                        on:click=move |_| open.set(false)
                                        class="absolute top-4 right-4 text-ctp-overlay0 hover:text-ctp-text text-2xl leading-none px-2 z-10"
                                    >
                                        "✕"
                                    </button>

                                    // Title and ID
                                    <div class="mb-6">
                                        <div class="flex items-center gap-3 mb-2">
                                            <CopyableId id=skill.id.clone()/>
                                            <h2 class="text-2xl font-bold text-ctp-text">{skill.name.clone()}</h2>
                                        </div>
                                    </div>

                                    // Parse and display frontmatter as table, then markdown body
                                    {
                                        // Parse YAML frontmatter
                                        let (frontmatter_lines, markdown_content) = if skill.content.starts_with("---") {
                                            // Find the closing ---
                                            if let Some(end_idx) = skill.content[3..].find("\n---") {
                                                let frontmatter = &skill.content[4..3 + end_idx]; // Skip opening ---\n
                                                let body = &skill.content[3 + end_idx + 4..]; // Skip past closing ---
                                                (frontmatter.lines().collect::<Vec<_>>(), body)
                                            } else {
                                                (vec![], skill.content.as_str())
                                            }
                                        } else {
                                            (vec![], skill.content.as_str())
                                        };

                                        // Strip relative file links from markdown to prevent 404s
                                        // Convert [text](path/to/file.md) -> **text** (bold text instead of link)
                                        let markdown_without_links = markdown_content
                                            .lines()
                                            .map(|line| {
                                                // Match markdown links: [text](url)
                                                let mut result = line.to_string();
                                                // Regex-like replacement for [text](relative/path)
                                                while let Some(start) = result.find("](") {
                                                    if let Some(link_start) = result[..start].rfind('[') {
                                                        if let Some(end) = result[start + 2..].find(')') {
                                                            let link_text = &result[link_start + 1..start];
                                                            let link_url = &result[start + 2..start + 2 + end];

                                                            // Only strip relative file links (not http/https)
                                                            if !link_url.starts_with("http://") && !link_url.starts_with("https://") {
                                                                // Replace [text](relative/path) with **text**
                                                                let replacement = format!("**{}**", link_text);
                                                                result.replace_range(link_start..start + 2 + end + 1, &replacement);
                                                            } else {
                                                                break; // Keep external links, move on
                                                            }
                                                        } else {
                                                            break;
                                                        }
                                                    } else {
                                                        break;
                                                    }
                                                }
                                                result
                                            })
                                            .collect::<Vec<_>>()
                                            .join("\n");

                                        // Render markdown body
                                        let mut options = Options::empty();
                                        options.insert(Options::ENABLE_STRIKETHROUGH);
                                        options.insert(Options::ENABLE_TABLES);
                                        options.insert(Options::ENABLE_FOOTNOTES);
                                        options.insert(Options::ENABLE_TASKLISTS);

                                        let parser = Parser::new_ext(markdown_without_links.trim(), options);
                                        let mut html_output = String::new();
                                        html::push_html(&mut html_output, parser);

                                        view! {
                                            <div class="mb-6">
                                                // Frontmatter table
                                                {(!frontmatter_lines.is_empty()).then(|| {
                                                    view! {
                                                        <div class="mb-4 bg-ctp-surface0 border border-ctp-surface2 rounded-lg p-4">
                                                            <h4 class="text-sm font-semibold text-ctp-subtext1 mb-3">"Metadata"</h4>
                                                            <table class="w-full text-sm">
                                                                <tbody>
                                                                    {frontmatter_lines.iter().filter_map(|line| {
                                                                        let trimmed = line.trim();
                                                                        if trimmed.is_empty() || trimmed.starts_with("#") {
                                                                            None
                                                                        } else if let Some((key, value)) = trimmed.split_once(':') {
                                                                            Some(view! {
                                                                                <tr class="border-b border-ctp-surface1 last:border-0">
                                                                                    <td class="py-2 pr-4 text-ctp-subtext1 font-medium align-top">{key.trim()}</td>
                                                                                    <td class="py-2 text-ctp-text">{value.trim()}</td>
                                                                                </tr>
                                                                            })
                                                                        } else {
                                                                            None
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </tbody>
                                                            </table>
                                                        </div>
                                                    }
                                                })}

                                                // Markdown body
                                                <div class="bg-ctp-surface1 rounded-lg p-6 overflow-auto prose prose-invert max-w-none" inner_html=html_output></div>
                                            </div>
                                        }
                                    }

                                    // Tags
                                    {(!skill.tags.is_empty()).then(|| {
                                        view! {
                                            <div class="mb-6">
                                                <h4 class="text-sm font-semibold text-ctp-subtext1 mb-2">"Tags"</h4>
                                                <div class="flex flex-wrap gap-2">
                                                    {skill.tags.iter().map(|tag| {
                                                        view! {
                                                            <span class="bg-ctp-surface2 text-ctp-text text-sm px-3 py-1 rounded">
                                                                {tag.clone()}
                                                            </span>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </div>
                                            </div>
                                        }
                                    })}

                                    // Metadata
                                    <div class="border-t border-ctp-surface1 pt-4 mt-4">
                                        <div class="grid grid-cols-2 gap-4 text-sm">
                                            <div>
                                                <span class="text-ctp-subtext1">"Created:"</span>
                                                <p class="text-ctp-text">{skill.created_at.clone()}</p>
                                            </div>
                                            <div>
                                                <span class="text-ctp-subtext1">"Updated:"</span>
                                                <p class="text-ctp-text">{skill.updated_at.clone()}</p>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! { <p class="text-ctp-subtext0 p-6">"No skill data"</p> }.into_any()
                        }
                    }}
                </div>
            </DrawerBody>
        </OverlayDrawer>
    }
}
