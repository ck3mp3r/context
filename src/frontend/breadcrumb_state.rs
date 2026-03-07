use leptos::prelude::*;
use std::collections::HashMap;

/// Global state for tracking page numbers across breadcrumb navigation.
///
/// Each breadcrumb can store and retrieve a page number using a unique name identifier.
/// This enables navigation back to the correct page of a list after visiting a detail page.
///
/// # Example
/// ```rust
/// // Card click: store page before navigating
/// let page_state = use_context::<BreadcrumbPageState>().unwrap();
/// page_state.set_page("projects", 3);
///
/// // Breadcrumb: read page and append to href
/// let page = page_state.get_page("projects"); // Some(3)
/// let href = format!("/?page={}", page.unwrap_or(0));
/// ```
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
