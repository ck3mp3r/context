use leptos::prelude::*;
use std::collections::HashMap;

/// # Breadcrumb Page State System
///
/// Event-driven pagination state management for breadcrumb navigation.
///
/// ## Overview
///
/// This system allows cards (Projects, TaskLists, Notes, etc.) to store their current
/// pagination state when clicked, and breadcrumbs to restore that state when navigating back.
///
/// **Key Concepts:**
/// - Each list/page has a unique **name identifier** (e.g., "projects", project_id)
/// - Cards **emit** the current page number on click using `set_page(name, page)`
/// - Breadcrumbs **read** the stored page number using `get_page(name)`
/// - State persists until page refresh (no automatic cleanup)
///
/// ## Architecture
///
/// ```text
/// ┌─────────────┐           ┌──────────────────┐           ┌─────────────┐
/// │   Card      │  click    │ BreadcrumbPage   │  read     │ Breadcrumb  │
/// │  Component  │─────────> │     State        │ <─────────│  Component  │
/// │             │ set_page()│  (RwSignal)      │ get_page()│             │
/// └─────────────┘           └──────────────────┘           └─────────────┘
/// ```
///
/// ## Usage Guide
///
/// ### 1. Setup (app.rs)
///
/// Provide the context in your app root:
///
/// ```rust
/// use crate::breadcrumb_state::BreadcrumbPageState;
///
/// let page_state = BreadcrumbPageState::new();
/// provide_context(page_state);
/// ```
///
/// ### 2. Card Component: Store Page on Click
///
/// **CRITICAL:** Pass a `ReadSignal<usize>` (not `usize`) to capture page at click time,
/// not render time!
///
/// ```rust
/// #[component]
/// pub fn MyCard(
///     item: MyItem,
///     #[prop(optional)] current_page: Option<ReadSignal<usize>>,  // Signal!
///     #[prop(optional)] breadcrumb_name: Option<String>,
/// ) -> impl IntoView {
///     let page_state = use_context::<BreadcrumbPageState>();
///     
///     view! {
///         <a
///             href=format!("/items/{}", item.id)
///             on:click=move |_| {
///                 // Store current page at click time
///                 if let (Some(state), Some(page_sig), Some(name)) =
///                     (page_state.as_ref(), current_page, &breadcrumb_name) {
///                     state.set_page(name, page_sig.get());  // .get() here!
///                 }
///             }
///         >
///             // ... card content
///         </a>
///     }
/// }
/// ```
///
/// ### 3. Parent Page: Pass Signal to Card
///
/// ```rust
/// let pagination = use_pagination();
///
/// view! {
///     <div>
///         {items.into_iter().map(|item| {
///             view! {
///                 <MyCard
///                     item=item
///                     current_page=pagination.page  // Pass signal, not value!
///                     breadcrumb_name="my-list"
///                 />
///             }
///         }).collect::<Vec<_>>()}
///     </div>
/// }
/// ```
///
/// ### 4. Breadcrumb: Add Name and Read State
///
/// ```rust
/// let items = vec![
///     BreadcrumbItem::new("My List")
///         .with_href("/my-list")
///         .with_name("my-list"),  // Name identifier
/// ];
/// ```
///
/// The `Breadcrumb` component automatically reads stored pages for items with names
/// and appends `?page=X` to the href.
///
/// ## Naming Convention
///
/// Use consistent, unique identifiers:
///
/// - **Top-level lists:** Simple string literals
///   - `"projects"` - Projects list
///   - `"notes"` - Notes list
///   - `"skills"` - Skills list
///
/// - **Nested lists (tabs/detail pages):** Use parent entity ID
///   - `project.id` - Task lists tab in project detail
///   - `note.id` - Subnotes list in note detail
///
/// ## State Lifecycle
///
/// - **Created:** When context is provided in app root
/// - **Updated:** When cards are clicked (`set_page`)
/// - **Read:** When breadcrumbs render (`get_page`)
/// - **Cleared:** Only on page refresh (no automatic cleanup)
///
/// ## Common Patterns
///
/// ### Pattern 1: Two-Level Navigation (Projects → Project Detail)
///
/// ```rust
/// // 1. Projects page stores its page
/// on:click=move |_| {
///     state.set_page("projects", pagination.page.get());
/// }
///
/// // 2. Project detail breadcrumb reads it
/// BreadcrumbItem::new("Projects")
///     .with_href("/")
///     .with_name("projects")
/// ```
///
/// ### Pattern 2: Three-Level Navigation (Projects → Project → TaskList)
///
/// ```rust
/// // 1. Projects page stores its page
/// state.set_page("projects", projects_page.get());
///
/// // 2. Project detail (task lists tab) stores its page
/// state.set_page(project_id, task_lists_page.get());
///
/// // 3. TaskList detail has two breadcrumbs
/// vec![
///     BreadcrumbItem::new("Projects")
///         .with_href("/")
///         .with_name("projects"),
///     BreadcrumbItem::new(project.title)
///         .with_href(format!("/projects/{}", project.id))
///         .with_name(project.id),  // Restores task lists page
/// ]
/// ```
///
/// ## Troubleshooting
///
/// ### Issue: Page captured at render time, not click time
///
/// **Wrong:**
/// ```rust
/// let current_page = pagination.page.get();  // ❌ Gets page at render
/// on:click=move |_| { state.set_page("list", current_page); }
/// ```
///
/// **Correct:**
/// ```rust
/// let page_signal = pagination.page;  // ✅ Store signal
/// on:click=move |_| { state.set_page("list", page_signal.get()); }
/// ```
///
/// ### Issue: Pagination reset on initial load
///
/// Effects that reset pagination must skip the first run:
///
/// ```rust
/// let is_first_load = std::cell::Cell::new(true);
/// Effect::new(move || {
///     some_trigger.get();
///     if is_first_load.get() {
///         is_first_load.set(false);
///     } else {
///         pagination.set_page.set(0);
///     }
/// });
/// ```
///
/// ### Issue: Breadcrumb not restoring page
///
/// Check that:
/// 1. Card passes `breadcrumb_name` that matches breadcrumb's `.with_name()`
/// 2. Card receives `ReadSignal<usize>`, not `usize`
/// 3. Breadcrumb item has `.with_name()` called
///
/// ## Example: Complete Implementation
///
/// See `src/frontend/pages/projects.rs` and `src/frontend/components/task_components.rs`
/// for a complete working example of the breadcrumb pagination system.
#[derive(Clone)]
pub struct BreadcrumbPageState {
    pages: RwSignal<HashMap<String, usize>>,
}

impl BreadcrumbPageState {
    pub fn new() -> Self {
        Self {
            pages: RwSignal::new(HashMap::new()),
        }
    }

    /// Store a page number for a specific breadcrumb identifier.
    ///
    /// # Arguments
    /// * `name` - Unique identifier for the breadcrumb (e.g., "projects", project_id)
    /// * `page` - Page number to store (0-indexed)
    pub fn set_page(&self, name: &str, page: usize) {
        self.pages.update(|map| {
            map.insert(name.to_string(), page);
        });
    }

    /// Retrieve the stored page number for a breadcrumb identifier.
    ///
    /// # Arguments
    /// * `name` - Unique identifier for the breadcrumb
    ///
    /// # Returns
    /// `Some(page)` if a page was previously stored, `None` otherwise
    pub fn get_page(&self, name: &str) -> Option<usize> {
        self.pages.with(|map| map.get(name).copied())
    }
}

impl Default for BreadcrumbPageState {
    fn default() -> Self {
        Self::new()
    }
}
