use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClientError, QueryBuilder};
use crate::components::{Pagination, SearchInput, SkillCard, SkillDetailModal, SortControls};
use crate::hooks::{use_pagination, use_search, use_sort};
use crate::models::{Paginated, Skill, UpdateMessage};
use crate::websocket::use_websocket_updates;

#[component]
pub fn Skills() -> impl IntoView {
    view! {
        <SkillsList/>
    }
}

#[component]
fn SkillsList() -> impl IntoView {
    const PAGE_SIZE: usize = 12;

    // Hooks for search, sort, and pagination
    let pagination = use_pagination();
    let search = use_search(Callback::new(move |_| {
        pagination.set_page.set(0);
    }));
    let sort = use_sort(
        "updated_at",
        "desc",
        Callback::new(move |_| {
            pagination.set_page.set(0);
        }),
    );

    let (skills_data, set_skills_data) = signal(None::<Result<Paginated<Skill>, ApiClientError>>);

    // Skill detail modal state
    let skill_modal_open = RwSignal::new(false);
    let selected_skill_id = RwSignal::new(String::new());

    // WebSocket updates
    let ws_updates = use_websocket_updates();
    let (refetch_trigger, set_refetch_trigger) = signal(0u32);

    // Watch for WebSocket updates and trigger refetch when skills change
    Effect::new(move || {
        if let Some(
            UpdateMessage::SkillCreated { .. }
            | UpdateMessage::SkillUpdated { .. }
            | UpdateMessage::SkillDeleted { .. },
        ) = ws_updates.get()
        {
            web_sys::console::log_1(&"Skill updated via WebSocket, refetching...".into());
            set_refetch_trigger.update(|n| *n = n.wrapping_add(1));
        }
    });

    // Use Effect to fetch when dependencies change (including WebSocket updates)
    Effect::new(move || {
        let current_page = pagination.page.get();
        let current_query = search.search_query.get();
        let current_sort = sort.sort_field.get();
        let current_order = sort.sort_order.get();
        let trigger = refetch_trigger.get();

        // Log for debugging with all dependency values
        web_sys::console::log_1(
            &format!(
                "FETCH TRIGGERED: page={}, query='{}', sort={}, order={}, trigger={}",
                current_page, current_query, current_sort, current_order, trigger
            )
            .into(),
        );

        // Reset to loading state immediately
        set_skills_data.set(None);

        spawn_local(async move {
            let offset = current_page * PAGE_SIZE;

            web_sys::console::log_1(
                &format!("API call: offset={}, limit={}", offset, PAGE_SIZE).into(),
            );

            let mut builder = QueryBuilder::<Skill>::new()
                .limit(PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order);

            if !current_query.trim().is_empty() {
                builder = builder.search(current_query);
            }

            let result = builder.fetch().await;

            match &result {
                Ok(data) => web_sys::console::log_1(
                    &format!("Got {} skills, total {}", data.items.len(), data.total).into(),
                ),
                Err(e) => web_sys::console::log_1(&format!("Error: {}", e).into()),
            }

            set_skills_data.set(Some(result));
        });
    });

    view! {
        <div class="container mx-auto p-6">
            <div class="flex justify-between items-center mb-6">
                <h2 class="text-3xl font-bold text-ctp-text">"Skills"</h2>
            </div>

            // Search bar and sort controls
            <div class="mb-6 flex gap-4 items-center">
                <div class="flex-1">
                    <SearchInput
                        value=search.search_input
                        on_change=search.on_debounced_change
                        on_immediate_change=search.on_immediate_change
                        placeholder="Search skills..."
                    />
                </div>
                <SortControls
                    sort_field=sort.sort_field
                    sort_order=sort.sort_order
                    on_sort_change=sort.on_sort_change
                    on_order_change=sort.on_order_change
                    fields=vec![
                        ("name".to_string(), "Name".to_string()),
                        ("created_at".to_string(), "Created".to_string()),
                        ("updated_at".to_string(), "Updated".to_string()),
                    ]
                />
            </div>

            {move || {
                match skills_data.get() {
                    None => view! { <p class="text-ctp-subtext0">"Loading skills..."</p> }.into_any(),
                    Some(result) => {
                                        match result {
                            Ok(paginated) => {
                                let total_pages = paginated.total.div_ceil(PAGE_SIZE);

                                if paginated.items.is_empty() {
                                    view! {
                                        <p class="text-ctp-subtext0">
                                            {if search.search_query.get().trim().is_empty() {
                                                "No skills found. Create one to get started!"
                                            } else {
                                                "No skills found matching your search."
                                            }}
                                        </p>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div>
                                            // Skills grid
                                            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
                                                {paginated
                                                    .items
                                                    .iter()
                                                    .map(|skill| {
                                                        view! {
                                                            <SkillCard
                                                                skill=skill.clone()
                                                                on_click=Callback::new({
                                                                    move |skill_id: String| {
                                                                        selected_skill_id.set(skill_id);
                                                                        skill_modal_open.set(true);
                                                                    }
                                                                })
                                                            />
                                                        }
                                                    })
                                                    .collect::<Vec<_>>()}
                                            </div>

                                            // Pagination
                                            <Pagination
                                                current_page=pagination.page
                                                total_pages=total_pages
                                                on_prev=pagination.on_prev
                                                on_next=pagination.on_next
                                                show_summary=true
                                                total_items=paginated.total
                                                page_size=PAGE_SIZE
                                                item_name="skills".to_string()
                                            />
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(err) => {
                                view! {
                                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                        <p class="text-ctp-red font-semibold">"Error loading skills"</p>
                                        <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    }
                }
            }}

            // Skill detail modal - only render when open
            {move || {
                if skill_modal_open.get() {
                    Some(view! {
                        <SkillDetailModal
                            skill_id=selected_skill_id.read_only()
                            open=skill_modal_open
                        />
                    })
                } else {
                    None
                }
            }}

        </div>
    }
}
