use leptos::prelude::*;
use std::collections::HashMap;

/// # Breadcrumb Query State System
///
/// Event-driven state management for breadcrumb navigation, preserving ALL query parameters.
///
/// ## Overview
///
/// This system allows cards to store their current query state (page, search, sort, filters)
/// when clicked, and breadcrumbs to restore that state when navigating back.
///
/// **Key Concepts:**
/// - Each list/page has a unique **name identifier** (e.g., "projects", project_id)
/// - Cards **emit** the current query string on click using `set_query(name, query_string)`
/// - Breadcrumbs **read** the stored query string using `get_query(name)`
/// - State persists until page refresh (no automatic cleanup)
#[derive(Clone)]
pub struct BreadcrumbPageState {
    queries: RwSignal<HashMap<String, String>>,
}

impl BreadcrumbPageState {
    pub fn new() -> Self {
        Self {
            queries: RwSignal::new(HashMap::new()),
        }
    }

    /// Store a query string for a specific breadcrumb identifier.
    ///
    /// # Arguments
    /// * `name` - Unique identifier for the breadcrumb (e.g., "projects", project_id)
    /// * `query` - Query string to store (e.g., "?page=2&q=search&sort=title")
    pub fn set_query(&self, name: &str, query: &str) {
        self.queries.update(|map| {
            map.insert(name.to_string(), query.to_string());
        });
    }

    /// Retrieve the stored query string for a breadcrumb identifier.
    ///
    /// # Arguments
    /// * `name` - Unique identifier for the breadcrumb
    ///
    /// # Returns
    /// `Some(query)` if a query was previously stored, `None` otherwise
    pub fn get_query(&self, name: &str) -> Option<String> {
        self.queries.with(|map| map.get(name).cloned())
    }
}

impl Default for BreadcrumbPageState {
    fn default() -> Self {
        Self::new()
    }
}
