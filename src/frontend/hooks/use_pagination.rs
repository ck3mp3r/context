use leptos::prelude::*;

/// Return type for use_pagination hook
pub struct UsePaginationReturn {
    pub page: ReadSignal<usize>,
    pub set_page: WriteSignal<usize>,
    pub on_prev: Callback<()>,
    pub on_next: Callback<()>,
}

/// Hook for managing pagination state
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
    let (page, set_page) = signal(0usize);

    let on_prev = Callback::new(move |_| {
        let current = page.get();
        if current > 0 {
            set_page.set(current - 1);
        }
    });

    let on_next = Callback::new(move |_| {
        set_page.set(page.get() + 1);
    });

    UsePaginationReturn {
        page,
        set_page,
        on_prev,
        on_next,
    }
}
