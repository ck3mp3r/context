use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};

use super::build_url_with_params;

/// Return type for use_search hook
pub struct UseSearchReturn {
    pub search_input: ReadSignal<String>,
    pub search_query: ReadSignal<String>,
    pub on_immediate_change: Callback<String>,
    pub on_debounced_change: Callback<String>,
}

/// Hook for managing search state with URL persistence and debouncing
///
/// Reads initial search from URL query parameter (?q=term) and updates
/// the URL when search changes. Also resets pagination to page 0 when
/// search query changes.
///
/// # Example
/// ```rust
/// let search = use_search();
///
/// view! {
///     <SearchInput
///         value=search.search_input
///         on_change=search.on_debounced_change
///         on_immediate_change=search.on_immediate_change
///         placeholder="Search..."
///     />
/// }
/// ```
pub fn use_search() -> UseSearchReturn {
    let query = use_query_map();
    let navigate = use_navigate();
    let location = use_location();

    // Initialize from URL
    let initial_search = query.read().get("q").unwrap_or_default();
    let (search_input, set_search_input) = signal(initial_search.clone());
    let (search_query, set_search_query) = signal(initial_search);

    // Watch for URL changes (e.g., back/forward navigation)
    Effect::new(move |_| {
        let url_search = query.get().get("q").unwrap_or_default();
        set_search_query.set(url_search.clone());
        set_search_input.set(url_search);
    });

    let on_immediate_change = Callback::new(move |value: String| {
        set_search_input.set(value);
    });

    let on_debounced_change = Callback::new(move |value: String| {
        set_search_query.set(value.clone());

        // Update URL with search query and reset page to 0
        let pathname = location.pathname.get();
        let url = if value.trim().is_empty() {
            // Remove search param when empty, also reset page
            build_url_with_params(
                pathname,
                query.read().clone(),
                [("q".to_string(), None), ("page".to_string(), None)].into(),
            )
        } else {
            // Set search param and reset page to 0
            build_url_with_params(
                pathname,
                query.read().clone(),
                [
                    ("q".to_string(), Some(value)),
                    ("page".to_string(), None), // Remove page param (defaults to 0)
                ]
                .into(),
            )
        };
        navigate(&url, Default::default());
    });

    UseSearchReturn {
        search_input,
        search_query,
        on_immediate_change,
        on_debounced_change,
    }
}
