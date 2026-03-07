use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};

/// Return type for use_pagination hook
pub struct UsePaginationReturn {
    pub page: ReadSignal<usize>,
    pub set_page: WriteSignal<usize>,
    pub on_prev: Callback<()>,
    pub on_next: Callback<()>,
}

/// Hook for managing pagination state with URL persistence
///
/// Reads initial page from URL query parameter (?page=X) and updates
/// the URL when pagination changes, ensuring state persists across
/// navigation and browser back/forward actions.
///
/// # Example
/// ```rust
/// let pagination = use_pagination();
///
/// view! {
///     <Pagination
///         current_page=pagination.page
///         total_pages=total_pages
///         on_prev=pagination.on_prev
///         on_next=pagination.on_next
///         ...
///     />
/// }
/// ```
pub fn use_pagination() -> UsePaginationReturn {
    let query = use_query_map();
    let navigate = use_navigate();
    let location = use_location();

    // Create signal, will be synced with URL via Effect
    let (page, set_page) = signal(0usize);

    // Sync page signal with URL changes
    Effect::new(move |_| {
        let current_page = query
            .get()
            .get("page")
            .and_then(|p| p.parse::<usize>().ok())
            .unwrap_or(0);
        set_page.set(current_page);
    });

    // Clone navigate and location for use in closures
    let navigate_prev = navigate.clone();
    let location_prev = location.clone();

    let on_prev = Callback::new(move |_| {
        let current = page.get();
        if current > 0 {
            let new_page = current - 1;
            let pathname = location_prev.pathname.get();
            let url = if new_page == 0 {
                // Omit page param for page 0 (cleaner URLs)
                pathname
            } else {
                format!("{}?page={}", pathname, new_page)
            };
            navigate_prev(&url, Default::default());
        }
    });

    let on_next = Callback::new(move |_| {
        let new_page = page.get() + 1;
        let pathname = location.pathname.get();
        navigate(
            &format!("{}?page={}", pathname, new_page),
            Default::default(),
        );
    });

    UsePaginationReturn {
        page,
        set_page,
        on_prev,
        on_next,
    }
}
