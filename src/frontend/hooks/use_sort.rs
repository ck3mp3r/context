use leptos::prelude::*;

/// Return type for use_sort hook
pub struct UseSortReturn {
    pub sort_field: ReadSignal<String>,
    pub sort_order: ReadSignal<String>,
    pub on_sort_change: Callback<String>,
    pub on_order_change: Callback<String>,
}

/// Hook for managing sort state
///
/// # Example
/// ```rust
/// let sort = use_sort("created_at", "desc", Callback::new(move |_| {
///     set_page.set(0); // Reset pagination on sort change
/// }));
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
    on_sort: Callback<()>,
) -> UseSortReturn {
    let (sort_field, set_sort_field) = signal(default_field.into());
    let (sort_order, set_sort_order) = signal(default_order.into());

    let on_sort_change = Callback::new(move |field: String| {
        set_sort_field.set(field);
        on_sort.run(());
    });

    let on_order_change = Callback::new(move |order: String| {
        set_sort_order.set(order);
        on_sort.run(());
    });

    UseSortReturn {
        sort_field,
        sort_order,
        on_sort_change,
        on_order_change,
    }
}
