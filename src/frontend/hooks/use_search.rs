use leptos::prelude::*;

/// Return type for use_search hook
pub struct UseSearchReturn {
    pub search_input: ReadSignal<String>,
    pub search_query: ReadSignal<String>,
    pub on_immediate_change: Callback<String>,
    pub on_debounced_change: Callback<String>,
}

/// Hook for managing search state with debouncing
///
/// # Example
/// ```rust
/// let search = use_search(Callback::new(move |_| {
///     set_page.set(0); // Reset pagination on search
/// }));
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
pub fn use_search(on_search: Callback<String>) -> UseSearchReturn {
    let (search_input, set_search_input) = signal(String::new());
    let (search_query, set_search_query) = signal(String::new());

    let on_immediate_change = Callback::new(move |value: String| {
        set_search_input.set(value);
    });

    let on_debounced_change = Callback::new(move |value: String| {
        set_search_query.set(value.clone());
        on_search.run(value);
    });

    UseSearchReturn {
        search_input,
        search_query,
        on_immediate_change,
        on_debounced_change,
    }
}
