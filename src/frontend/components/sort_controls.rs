use leptos::prelude::*;

/// A reusable component for sorting controls (field selection + order toggle).
///
/// Provides a dropdown for selecting sort field and a button for toggling asc/desc order.
/// Designed to be compact and inline.
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
    // Handle field selection change
    let on_field_change = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        on_sort_change.run(value);
    };

    // Toggle between asc and desc
    let on_order_toggle = move |_| {
        let current = sort_order.get();
        let new_order = if current == "asc" {
            "desc".to_string()
        } else {
            "asc".to_string()
        };
        on_order_change.run(new_order);
    };

    view! {
        <div class="flex items-center gap-3 text-sm">
            <span class="text-ctp-subtext0">"Sort by:"</span>

            // Sort field dropdown
            <select
                class="px-3 py-1.5 rounded-lg border-gray-600 bg-gray-700 text-white focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
                on:change=on_field_change
            >
                {fields
                    .into_iter()
                    .map(|(value, label)| {
                        let value_clone = value.clone();
                        let is_selected = move || sort_field.get() == value_clone;
                        view! {
                            <option value=value selected=is_selected>
                                {label}
                            </option>
                        }
                    })
                    .collect::<Vec<_>>()}
            </select>

            // Order toggle button
            <button
                on:click=on_order_toggle
                class="px-3 py-1.5 rounded-lg bg-gray-700 text-white hover:bg-gray-600 transition-colors border border-gray-600 flex items-center gap-1.5"
                title="Toggle sort order"
            >
                {move || {
                    if sort_order.get() == "asc" {
                        view! {
                            <>
                                <span class="text-lg leading-none">"↑"</span>
                                <span>"Asc"</span>
                            </>
                        }
                    } else {
                        view! {
                            <>
                                <span class="text-lg leading-none">"↓"</span>
                                <span>"Desc"</span>
                            </>
                        }
                    }
                }}
            </button>
        </div>
    }
}
