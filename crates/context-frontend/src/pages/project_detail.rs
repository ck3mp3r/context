use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::*;

use crate::api::{ApiClientError, QueryBuilder, projects};
use crate::components::{
    ExternalRefLink, NoteCard, Pagination, RepoCard, SearchInput, SkillCard, SkillDetailModal,
    SortControls, TaskListCard,
};
use crate::hooks::{use_pagination_in_memory, use_search_in_memory, use_sort_in_memory};
use crate::models::{Note, Paginated, Project, Repo, Skill, TaskList, UpdateMessage};
use crate::tabs::{TabScope, should_refetch};
use crate::websocket::use_websocket_updates;

#[component]
pub fn ProjectDetail(scope: TabScope) -> impl IntoView {
    // The tab's entity id never changes for the tab's lifetime, so one local store serves every
    // reactive closure below without a clone per closure.
    let project_id = StoredValue::new_local(scope.entity_id().to_string());

    let (project_data, set_project_data) = signal(None::<Result<Project, ApiClientError>>);

    // The project dashboard owns the section pills as internal tab state (decision 3): a panel
    // click changes this signal and never rewrites the URL.
    let active_panel = RwSignal::new("task-lists");
    let active_tab = move || active_panel.get();
    let (task_lists_data, set_task_lists_data) =
        signal(None::<Result<Paginated<TaskList>, ApiClientError>>);
    let (notes_data, set_notes_data) = signal(None::<Result<Paginated<Note>, ApiClientError>>);
    let (repos_data, set_repos_data) = signal(None::<Result<Paginated<Repo>, ApiClientError>>);
    let (skills_data, set_skills_data) = signal(None::<Result<Paginated<Skill>, ApiClientError>>);

    const TASK_LIST_PAGE_SIZE: usize = 12;
    const NOTE_PAGE_SIZE: usize = 12;
    const REPO_PAGE_SIZE: usize = 12;
    const SKILL_PAGE_SIZE: usize = 12;

    // Hooks for task lists tab
    let task_list_pagination = use_pagination_in_memory();
    let task_list_search = use_search_in_memory();
    let task_list_sort = use_sort_in_memory("updated_at", "desc");

    // Hooks for notes tab
    let note_pagination = use_pagination_in_memory();
    let note_search = use_search_in_memory();
    let note_sort = use_sort_in_memory("last_activity_at", "desc");

    // Hooks for repos tab
    let repo_pagination = use_pagination_in_memory();
    let repo_search = use_search_in_memory();
    let repo_sort = use_sort_in_memory("created_at", "desc");

    // Hooks for skills tab
    let skill_pagination = use_pagination_in_memory();
    let skill_search = use_search_in_memory();
    let skill_sort = use_sort_in_memory("created_at", "desc");

    // Show archived is per-tab list state: it never enters the URL (decision 20).
    let show_archived_task_lists = RwSignal::new(false);

    // Skill detail modal state
    let skill_modal_open = RwSignal::new(false);
    let selected_skill_id = RwSignal::new(String::new());

    // WebSocket updates
    let ws_updates = use_websocket_updates();

    // Triggers to force refetch
    let (note_refetch_trigger, set_note_refetch_trigger) = signal(0u32);
    let (task_list_refetch_trigger, set_task_list_refetch_trigger) = signal(0u32);
    let (repo_refetch_trigger, set_repo_refetch_trigger) = signal(0u32);
    let (skill_refetch_trigger, set_skill_refetch_trigger) = signal(0u32);
    let (project_refetch_trigger, set_project_refetch_trigger) = signal(0u32);

    // A panel opens for good on its first pill click; the default panel is open at mount. Until then
    // the panel holds no data, so it has nothing to load and nothing to keep live (decision 15).
    let task_lists_opened = RwSignal::new(true);
    let notes_opened = RwSignal::new(false);
    let repos_opened = RwSignal::new(false);
    let skills_opened = RwSignal::new(false);

    // The tab's own visibility, read without subscribing: the dispatch below reacts to a new
    // message, never to a tab switch.
    let tab_active = {
        let scope = scope.clone();
        move || untrack(|| scope.is_active())
    };

    // One websocket message drives at most one refetch, and only for a view that is on screen or has
    // already loaded (decision 15). A message naming another view's data refetches nothing, and a
    // panel the user never opened fetches nothing. Every read but the message is untracked, so a
    // refetch never re-triggers on the data it just wrote.
    //
    // A child message names the child's id, not its project, so a panel matches by the kind of data
    // it lists: it refreshes its own list, which it has loaded.
    Effect::new(move || {
        let Some(update) = ws_updates.get() else {
            return;
        };
        let panel = active_panel.get_untracked();
        match update {
            UpdateMessage::NoteCreated { .. }
            | UpdateMessage::NoteUpdated { .. }
            | UpdateMessage::NoteDeleted { .. } => {
                if should_refetch(
                    notes_opened.get_untracked(),
                    panel == "notes",
                    true,
                ) {
                    web_sys::console::log_1(
                        &"Note updated via WebSocket, refetching project notes...".into(),
                    );
                    set_note_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
            }
            UpdateMessage::TaskListCreated { .. }
            | UpdateMessage::TaskListUpdated { .. }
            | UpdateMessage::TaskListDeleted { .. }
            | UpdateMessage::TaskCreated { .. }
            | UpdateMessage::TaskUpdated { .. }
            | UpdateMessage::TaskDeleted { .. } => {
                if should_refetch(
                    task_lists_opened.get_untracked(),
                    panel == "task-lists",
                    true,
                ) {
                    web_sys::console::log_1(
                        &"TaskList/Task updated via WebSocket, refetching project task lists..."
                            .into(),
                    );
                    set_task_list_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
            }
            UpdateMessage::RepoCreated { .. }
            | UpdateMessage::RepoUpdated { .. }
            | UpdateMessage::RepoDeleted { .. } => {
                if should_refetch(repos_opened.get_untracked(), panel == "repos", true) {
                    web_sys::console::log_1(
                        &"Repo updated via WebSocket, refetching project repos...".into(),
                    );
                    set_repo_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
            }
            UpdateMessage::SkillCreated { .. }
            | UpdateMessage::SkillUpdated { .. }
            | UpdateMessage::SkillDeleted { .. } => {
                if should_refetch(skills_opened.get_untracked(), panel == "skills", true) {
                    web_sys::console::log_1(
                        &"Skill updated via WebSocket, refetching project skills...".into(),
                    );
                    set_skill_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
            }
            UpdateMessage::ProjectUpdated {
                project_id: updated_id,
            }
            | UpdateMessage::ProjectDeleted {
                project_id: updated_id,
            } => {
                // The header is the tab's own entity, so its message is the one this tab answers to.
                if should_refetch(
                    project_data.get_untracked().is_some(),
                    tab_active(),
                    updated_id == project_id.get_value(),
                ) {
                    web_sys::console::log_1(
                        &"Project updated via WebSocket, refetching project...".into(),
                    );
                    set_project_refetch_trigger.update(|n| *n = n.wrapping_add(1));
                }
            }
            _ => {}
        }
    });

    // Fetch task lists for this project (with archived toggle, search, and pagination)
    Effect::new(move || {
        let id = project_id.get_value();
        let _ = project_refetch_trigger.get(); // Track refetch trigger from WebSocket updates
        if !id.is_empty() {
            spawn_local(async move {
                let result = projects::get(&id).await;
                set_project_data.set(Some(result));
            });
        }
    });

    // Fetch task lists for this project (with archived toggle, search, and pagination). The default
    // panel is open at mount, so this is the one panel that loads without a click.
    Effect::new(move || {
        let id = project_id.get_value();
        let opened = task_lists_opened.get();
        let show_archived = show_archived_task_lists.get();
        let search_query = task_list_search.search_query.get();
        let current_page = task_list_pagination.page.get();
        let current_sort = task_list_sort.sort_field.get();
        let current_order = task_list_sort.sort_order.get();
        let _ = task_list_refetch_trigger.get();
        if id.is_empty() || !opened {
            return;
        }
        spawn_local(async move {
            let status = if show_archived { None } else { Some("active") };
            let offset = current_page * TASK_LIST_PAGE_SIZE;
            let search_opt = if search_query.trim().is_empty() {
                None
            } else {
                Some(search_query)
            };
            let mut builder = QueryBuilder::<TaskList>::new()
                .limit(TASK_LIST_PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order)
                .param("project_id", id);

            if let Some(stat) = status {
                builder = builder.param("status", stat);
            }

            if let Some(search) = search_opt {
                builder = builder.search(search);
            }

            let result = builder.fetch().await;
            set_task_lists_data.set(Some(result));
        });
    });

    // Fetch notes for this project (with FTS5 search support and pagination)
    Effect::new(move || {
        let id = project_id.get_value();
        let search = note_search.search_query.get();
        let current_page = note_pagination.page.get();
        let current_sort = note_sort.sort_field.get();
        let current_order = note_sort.sort_order.get();
        let opened = notes_opened.get();
        let _ = note_refetch_trigger.get();
        if id.is_empty() || !opened {
            return;
        }
        spawn_local(async move {
            let search_query = if search.trim().is_empty() {
                None
            } else {
                Some(search)
            };
            let offset = current_page * NOTE_PAGE_SIZE;

            let mut builder = QueryBuilder::<Note>::new()
                .limit(NOTE_PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order)
                .param("project_id", id)
                .param("type", "note")
                .param("include_content", "true");

            if let Some(search) = search_query {
                builder = builder.search(search);
            }

            let result = builder.fetch().await;
            set_notes_data.set(Some(result));
        });
    });

    // Fetch repos for this project (with pagination)
    Effect::new(move || {
        let id = project_id.get_value();
        let current_page = repo_pagination.page.get();
        let current_sort = repo_sort.sort_field.get();
        let current_order = repo_sort.sort_order.get();
        let current_search = repo_search.search_query.get();
        let opened = repos_opened.get();
        let _ = repo_refetch_trigger.get();
        if id.is_empty() || !opened {
            return;
        }
        spawn_local(async move {
            let offset = current_page * REPO_PAGE_SIZE;

            let mut builder = QueryBuilder::<Repo>::new()
                .limit(REPO_PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order)
                .param("project_id", id);

            if !current_search.trim().is_empty() {
                builder = builder.search(current_search);
            }

            let result = builder.fetch().await;
            set_repos_data.set(Some(result));
        });
    });

    // Fetch skills for this project (with pagination)
    Effect::new(move || {
        let id = project_id.get_value();
        let current_page = skill_pagination.page.get();
        let current_sort = skill_sort.sort_field.get();
        let current_order = skill_sort.sort_order.get();
        let current_search = skill_search.search_query.get();
        let opened = skills_opened.get();
        let _ = skill_refetch_trigger.get();
        if id.is_empty() || !opened {
            return;
        }
        spawn_local(async move {
            let offset = current_page * SKILL_PAGE_SIZE;

            let mut builder = QueryBuilder::<Skill>::new()
                .limit(SKILL_PAGE_SIZE)
                .offset(offset)
                .sort(current_sort)
                .order(current_order)
                .param("project_id", id);

            if !current_search.trim().is_empty() {
                builder = builder.search(current_search);
            }

            let result = builder.fetch().await;
            set_skills_data.set(Some(result));
        });
    });

    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]" data-page="project-detail">
            <div class="container mx-auto p-6 flex-1">
            {move || match project_data.get() {
                None => {
                    view! {
                        <div class="text-center py-12">
                            <p class="text-ctp-subtext0">"Loading project..."</p>
                        </div>
                    }
                        .into_any()
                }
                Some(Ok(project)) => {
                    view! {
                        <div>
                            // Project metadata (description, tags, external refs if present)
                            {(project.description.is_some() || !project.tags.is_empty() || !project.external_refs.is_empty()).then(|| {
                                view! {
                                    <div class="mb-6">
                                        {project
                                            .description
                                            .as_ref()
                                            .map(|desc| {
                                                view! { <p class="text-ctp-subtext0 text-sm mb-3">{desc.clone()}</p> }
                                            })}

                                        {(!project.tags.is_empty())
                                            .then(|| {
                                                view! {
                                                    <div class="flex flex-wrap gap-2 mb-3">
                                                        {project
                                                            .tags
                                                            .iter()
                                                            .map(|tag| {
                                                                view! {
                                                                    <span class="text-xs bg-ctp-surface1 text-ctp-subtext1 px-3 py-1 rounded">
                                                                        {tag.clone()}
                                                                    </span>
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </div>
                                                }
                                            })}

                                        {(!project.external_refs.is_empty())
                                            .then(|| {
                                                view! {
                                                    <div class="flex flex-wrap gap-1">
                                                        {project
                                                            .external_refs
                                                            .iter()
                                                            .map(|ext_ref| {
                                                                view! {
                                                                    <ExternalRefLink external_ref=ext_ref.clone()/>
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()}
                                                    </div>
                                                }
                                            })}
                                    </div>
                                }
                            })}

                            // Section pills: internal tab state, so a click never rewrites the URL
                            // (decision 3). A click also opens the panel for good: a panel the user
                            // never opened fetches nothing (decision 15).
                            <div class="border-b border-ctp-surface1 mb-6">
                                <div class="flex gap-6">
                                    <button
                                        type="button"
                                        data-panel="task-lists"
                                        on:click=move |_| active_panel.set("task-lists")
                                        class=move || {
                                            if active_tab() == "task-lists" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >
                                        "Task Lists"
                                    </button>
                                    <button
                                        type="button"
                                        data-panel="notes"
                                        on:click=move |_| {
                                            notes_opened.set(true);
                                            active_panel.set("notes");
                                        }
                                        class=move || {
                                            if active_tab() == "notes" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >
                                        "Notes"
                                    </button>
                                    <button
                                        type="button"
                                        data-panel="skills"
                                        on:click=move |_| {
                                            skills_opened.set(true);
                                            active_panel.set("skills");
                                        }
                                        class=move || {
                                            if active_tab() == "skills" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >
                                        "Skills"
                                    </button>
                                    <button
                                        type="button"
                                        data-panel="repos"
                                        on:click=move |_| {
                                            repos_opened.set(true);
                                            active_panel.set("repos");
                                        }
                                        class=move || {
                                            if active_tab() == "repos" {
                                                "pb-3 border-b-2 border-ctp-blue text-ctp-blue font-medium"
                                            } else {
                                                "pb-3 border-b-2 border-transparent text-ctp-subtext0 hover:text-ctp-text"
                                            }
                                        }
                                    >
                                        "Repos"
                                    </button>
                                </div>
                            </div>

                            // Tab Content
                            <div data-panel-content=active_tab>
                                {move || {
                                    let _proj_id = project.id.clone();
                                    match active_tab() {
                                        "task-lists" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=task_list_search.search_input
                                                                on_change=task_list_search.on_debounced_change
                                                                on_immediate_change=task_list_search.on_immediate_change
                                                                placeholder="Search task lists..."
                                                            >
                                                                <div class="flex items-center gap-2 whitespace-nowrap">
                                                                    <Switch checked=show_archived_task_lists/>
                                                                    <span class="text-sm text-ctp-text">"Show archived"</span>
                                                                </div>
                                                            </SearchInput>
                                                        </div>
                                                        <SortControls
                                                            sort_field=task_list_sort.sort_field
                                                            sort_order=task_list_sort.sort_order
                                                            on_sort_change=task_list_sort.on_sort_change
                                                            on_order_change=task_list_sort.on_order_change
                                                            fields=vec![
                                                                ("title".to_string(), "Title".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                                ("updated_at".to_string(), "Updated".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match task_lists_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading task lists..."</p> }
                                                                .into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            let total_pages = paginated.total.div_ceil(TASK_LIST_PAGE_SIZE);

                                                            if paginated.items.is_empty() {
                                                                view! {
                                                                <p class="text-ctp-subtext0">
                                                                    {if task_list_search.search_query.get().trim().is_empty() {
                                                                        "No task lists for this project yet"
                                                                    } else {
                                                                        "No task lists found matching your search"
                                                                    }}
                                                                </p>
                                                            }
                                                                .into_any()
                                                            } else {
                                                                  view! {
                                                                    <div>
                                                                         <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                              {paginated.items
                                                                                 .into_iter()
                                                                                 .map(|task_list| {
                                                                                     let proj_id = project_id.get_value();
                                                                                     view! {
                                                                                         <TaskListCard
                                                                                             task_list=task_list
                                                                                             project_id=proj_id.clone()
                                                                                         />
                                                                                     }
                                                                                 })
                                                                                 .collect::<Vec<_>>()}
                                                                         </div>

                                                                        // Pagination
                                                                        <Pagination
                                                                            current_page=task_list_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=task_list_pagination.on_prev
                                                                            on_next=task_list_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=TASK_LIST_PAGE_SIZE
                                                                            item_name="task lists".to_string()
                                                                        />
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }
                                                        }
                                                        Some(Err(err)) => {
                                                            view! {
                                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                                    <p class="text-ctp-red">{err.to_string()}</p>
                                                                </div>
                                                            }
                                                                .into_any()
                                                        }
                                                    }}

                                                </div>
                                            }
                                                .into_any()
                                        }
                                        "notes" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=note_search.search_input
                                                                on_change=note_search.on_debounced_change
                                                                on_immediate_change=note_search.on_immediate_change
                                                                placeholder="Search notes..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=note_sort.sort_field
                                                            sort_order=note_sort.sort_order
                                                            on_sort_change=note_sort.on_sort_change
                                                            on_order_change=note_sort.on_order_change
                                                            fields=vec![
                                                                ("title".to_string(), "Title".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                                ("last_activity_at".to_string(), "Updated".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match notes_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading notes..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            let total_pages = paginated.total.div_ceil(NOTE_PAGE_SIZE);

                                                            // Backend already filtered with FTS5, just display results
                                                            if paginated.items.is_empty() {
                                                                view! { <p class="text-ctp-subtext0">"No notes found"</p> }
                                                                    .into_any()
                                                            } else {
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                            {paginated
                                                                .items
                                                                .iter()
                                                                .map(|note| {
                                                                    let proj_id = project_id.get_value();
                                                                    view! {
                                                                         <NoteCard
                                                                            note=note.clone()
                                                                            project_id=proj_id.clone()
                                                                        />
                                                                    }
                                                                })
                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        // Pagination
                                                                        <Pagination
                                                                            current_page=note_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=note_pagination.on_prev
                                                                            on_next=note_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=NOTE_PAGE_SIZE
                                                                            item_name="notes".to_string()
                                                                        />
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }
                                                        }
                                                        Some(Err(err)) => {
                                                            view! {
                                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                                    <p class="text-ctp-red">{err.to_string()}</p>
                                                                </div>
                                                            }
                                                                .into_any()
                                                        }
                                                    }}

                                                </div>
                                            }
                                                .into_any()
                                        }
                                        "repos" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=repo_search.search_input
                                                                on_change=repo_search.on_debounced_change
                                                                on_immediate_change=repo_search.on_immediate_change
                                                                placeholder="Search repositories by remote URL or tags..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=repo_sort.sort_field
                                                            sort_order=repo_sort.sort_order
                                                            on_sort_change=repo_sort.on_sort_change
                                                            on_order_change=repo_sort.on_order_change
                                                            fields=vec![
                                                                ("remote".to_string(), "Remote".to_string()),
                                                                ("path".to_string(), "Path".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match repos_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading repos..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            if paginated.items.is_empty() {
                                                                view! {
                                                                    <p class="text-ctp-subtext0">
                                                                        {if repo_search.search_query.get().trim().is_empty() {
                                                                            "No repositories found. Add one to get started!"
                                                                        } else {
                                                                            "No repositories found matching your search."
                                                                        }}
                                                                    </p>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                let total_pages = paginated.total.div_ceil(REPO_PAGE_SIZE);
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                            {paginated
                                                                                .items
                                                                                .iter()
                                                                                .map(|repo| {
                                                                                    let proj_id = project_id.get_value();
                                                                                    view! {
                                                                                        <RepoCard
                                                                                            repo=repo.clone()
                                                                                            project_id=proj_id.clone()
                                                                                        />
                                                                                    }
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        <Pagination
                                                                            current_page=repo_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=repo_pagination.on_prev
                                                                            on_next=repo_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=REPO_PAGE_SIZE
                                                                            item_name="repositories".to_string()
                                                                        />
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }
                                                        }
                                                        Some(Err(err)) => {
                                                            view! {
                                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                                    <p class="text-ctp-red">{err.to_string()}</p>
                                                                </div>
                                                            }
                                                                .into_any()
                                                        }
                                                    }}

                                            </div>
                                        }
                                            .into_any()
                                        }
                                        "skills" => {
                                            view! {
                                                <div>
                                                    // Search input and sort controls
                                                    <div class="mb-4 flex gap-4 items-center">
                                                        <div class="flex-1">
                                                            <SearchInput
                                                                value=skill_search.search_input
                                                                on_change=skill_search.on_debounced_change
                                                                on_immediate_change=skill_search.on_immediate_change
                                                                placeholder="Search skills by name or tags..."
                                                            />
                                                        </div>
                                                        <SortControls
                                                            sort_field=skill_sort.sort_field
                                                            sort_order=skill_sort.sort_order
                                                            on_sort_change=skill_sort.on_sort_change
                                                            on_order_change=skill_sort.on_order_change
                                                            fields=vec![
                                                                ("name".to_string(), "Name".to_string()),
                                                                ("created_at".to_string(), "Created".to_string()),
                                                            ]
                                                        />
                                                    </div>

                                                    {move || match skills_data.get() {
                                                        None => {
                                                            view! { <p class="text-ctp-subtext0">"Loading skills..."</p> }.into_any()
                                                        }
                                                        Some(Ok(paginated)) => {
                                                            if paginated.items.is_empty() {
                                                                view! {
                                                                    <p class="text-ctp-subtext0">
                                                                        {if skill_search.search_query.get().trim().is_empty() {
                                                                            "No skills found. Add one to get started!"
                                                                        } else {
                                                                            "No skills found matching your search."
                                                                        }}
                                                                    </p>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                let total_pages = paginated.total.div_ceil(SKILL_PAGE_SIZE);
                                                                view! {
                                                                    <div>
                                                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6 auto-rows-fr">
                                                                            {paginated
                                                                                .items
                                                                                .iter()
                                                                                .map(|skill| {
                                                                                    view! {
                                                                                        <SkillCard
                                                                                            skill=skill.clone()
                                                                                            on_click=Callback::new(move |id: String| {
                                                                                                selected_skill_id.set(id);
                                                                                                skill_modal_open.set(true);
                                                                                            })
                                                                                        />
                                                                                    }
                                                                                })
                                                                                .collect::<Vec<_>>()}
                                                                        </div>

                                                                        <Pagination
                                                                            current_page=skill_pagination.page
                                                                            total_pages=total_pages
                                                                            on_prev=skill_pagination.on_prev
                                                                            on_next=skill_pagination.on_next
                                                                            show_summary=true
                                                                            total_items=paginated.total
                                                                            page_size=SKILL_PAGE_SIZE
                                                                            item_name="skills".to_string()
                                                                        />
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            }
                                                        }
                                                        Some(Err(err)) => {
                                                            view! {
                                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                                                                    <p class="text-ctp-red">{err.to_string()}</p>
                                                                </div>
                                                            }
                                                                .into_any()
                                                        }
                                                    }}

                                                </div>
                                            }
                                                .into_any()
                                        }
                                        _ => view! { <div></div> }.into_any(),
                                    }
                                }}

                            </div>
                        </div>
                    }
                        .into_any()
                }
                Some(Err(err)) => {
                    view! {
                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                            <p class="text-ctp-red font-semibold">"Error loading project"</p>
                            <p class="text-ctp-subtext0 text-sm mt-2">{err.to_string()}</p>
                        </div>
                    }
                        .into_any()
                }
            }}

            </div>
            // End container div (from line 329)

            // Skill detail modal
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
        // End flex container
    }
}
