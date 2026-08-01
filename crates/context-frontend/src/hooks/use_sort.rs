use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};

use super::build_url_with_params;

/// Return type for use_sort hook
pub struct UseSortReturn {
    pub sort_field: ReadSignal<String>,
    pub sort_order: ReadSignal<String>,
    pub on_sort_change: Callback<String>,
    pub on_order_change: Callback<String>,
}

/// Hook for managing sort state with URL persistence
///
/// Reads initial sort from URL query parameters (?sort=field&order=asc) and updates
/// the URL when sort changes. Also resets pagination to page 0 when sort changes.
///
/// # Example
/// ```rust
/// let sort = use_sort("created_at", "desc");
///
/// view! {
///     <SortControls
///         sort_field=sort.sort_field
///         sort_order=sort.sort_order
///         on_sort_change=sort.on_sort_change
///         on_order_change=sort.on_order_change
///         fields=vec![...]
///     />
/// }
/// ```
pub fn use_sort(
    default_field: impl Into<String>,
    default_order: impl Into<String>,
) -> UseSortReturn {
    let query = use_query_map();
    let navigate = use_navigate();
    let location = use_location();

    let default_field = default_field.into();
    let default_order = default_order.into();

    // Initialize from URL or use defaults
    let initial_field = query.read().get("sort").unwrap_or(default_field.clone());
    let initial_order = query.read().get("order").unwrap_or(default_order.clone());

    let (sort_field, set_sort_field) = signal(initial_field);
    let (sort_order, set_sort_order) = signal(initial_order);

    // Watch for URL changes (e.g., back/forward navigation)
    Effect::new(move |_| {
        let url_field = query.get().get("sort").unwrap_or(default_field.clone());
        let url_order = query.get().get("order").unwrap_or(default_order.clone());
        set_sort_field.set(url_field);
        set_sort_order.set(url_order);
    });

    let query_for_field = query;
    let location_for_field = location.clone();
    let navigate_for_field = navigate.clone();

    let on_sort_change = Callback::new(move |field: String| {
        set_sort_field.set(field.clone());

        // Update URL with new sort field (keep current order) and reset page
        let pathname = location_for_field.pathname.get();
        let current_order = sort_order.get();
        let url = build_url_with_params(
            pathname,
            query_for_field.read().clone(),
            [
                ("sort".to_string(), Some(field)),
                ("order".to_string(), Some(current_order)),
                ("page".to_string(), None), // Reset to page 0
            ]
            .into(),
        );
        navigate_for_field(&url, Default::default());
    });

    let on_order_change = Callback::new(move |order: String| {
        set_sort_order.set(order.clone());

        // Update URL with new sort order (keep current field) and reset page
        let pathname = location.pathname.get();
        let current_field = sort_field.get();
        let url = build_url_with_params(
            pathname,
            query.read().clone(),
            [
                ("sort".to_string(), Some(current_field)),
                ("order".to_string(), Some(order)),
                ("page".to_string(), None), // Reset to page 0
            ]
            .into(),
        );
        navigate(&url, Default::default());
    });

    UseSortReturn {
        sort_field,
        sort_order,
        on_sort_change,
        on_order_change,
    }
}
