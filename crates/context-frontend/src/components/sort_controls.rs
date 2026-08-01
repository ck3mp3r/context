use leptos::prelude::*;

/// A reusable component for sorting controls with integrated direction toggle.
///
/// Click a field once to sort by that field (ascending). Click again to reverse order (descending).
/// Visual indicator shows current sort field and direction with arrow icons.
///
/// # Props
/// - `sort_field`: ReadSignal<String> - The current sort field value
/// - `sort_order`: ReadSignal<String> - The current order ("asc" or "desc")
/// - `on_sort_change`: Callback<String> - Called when sort field changes
/// - `on_order_change`: Callback<String> - Called when order changes
/// - `fields`: Vec<(String, String)> - Available sort fields as (value, label) pairs
#[component]
pub fn SortControls(
    /// Current sort field
    sort_field: ReadSignal<String>,
    /// Current sort order ("asc" or "desc")
    sort_order: ReadSignal<String>,
    /// Callback when sort field changes
    on_sort_change: Callback<String>,
    /// Callback when order changes
    on_order_change: Callback<String>,
    /// Available fields as (value, label) pairs
    fields: Vec<(String, String)>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3 text-sm">
            <span class="text-ctp-subtext0">"Sort by:"</span>

            // Radio toggle buttons with integrated direction
            <div class="flex gap-1 bg-ctp-surface0 p-1 rounded-lg border border-ctp-surface1">
                {fields
                    .into_iter()
                    .map(|(value, label)| {
                        let value_for_click = value.clone();
                        let value_for_class = value.clone();
                        let value_for_title = value.clone();
                        let value_for_content = value.clone();

                        let on_click = move |_| {
                            let current_field = sort_field.get();
                            let current_order = sort_order.get();

                            if current_field == value_for_click {
                                // Same field clicked - toggle order
                                let new_order = if current_order == "asc" {
                                    "desc".to_string()
                                } else {
                                    "asc".to_string()
                                };
                                on_order_change.run(new_order);
                            } else {
                                // Different field clicked - change field (reset to asc)
                                on_sort_change.run(value_for_click.clone());
                                on_order_change.run("asc".to_string());
                            }
                        };

                        view! {
                            <button
                                on:click=on_click
                                class=move || {
                                    if sort_field.get() == value_for_class {
                                        "px-3 py-1.5 rounded bg-ctp-blue text-ctp-base font-medium transition-colors flex items-center gap-1"
                                    } else {
                                        "px-3 py-1.5 rounded text-ctp-text hover:bg-ctp-surface1 transition-colors"
                                    }
                                }
                                title=move || {
                                    if sort_field.get() == value_for_title {
                                        if sort_order.get() == "asc" {
                                            "Click to sort descending"
                                        } else {
                                            "Click to sort ascending"
                                        }
                                    } else {
                                        "Click to sort by this field"
                                    }
                                }
                            >
                                {move || {
                                    if sort_field.get() == value_for_content {
                                        let arrow = if sort_order.get() == "asc" { "↑" } else { "↓" };
                                        view! {
                                            <>
                                                <span>{label.clone()}</span>
                                                <span class="text-base leading-none">{arrow}</span>
                                            </>
                                        }.into_any()
                                    } else {
                                        view! { <span>{label.clone()}</span> }.into_any()
                                    }
                                }}
                            </button>
                        }
                    })
                    .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
