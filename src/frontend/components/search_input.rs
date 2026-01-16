use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

/// A reusable debounced search input component.
///
/// This component encapsulates the debouncing logic and provides a clean
/// search input that can be embedded anywhere in the application.
///
/// # Props
/// - `value`: ReadSignal<String> - The current input value to display
/// - `on_change`: Callback<String> - Called with the debounced search value
/// - `on_immediate_change`: Callback<String> - Called immediately on input (for updating the signal)
/// - `placeholder`: &'static str - Placeholder text (default: "Search...")
/// - `debounce_ms`: u32 - Debounce delay in milliseconds (default: 500)
#[component]
pub fn SearchInput(
    /// The current value of the search input
    value: ReadSignal<String>,
    /// Callback when the debounced search value changes
    on_change: Callback<String>,
    /// Callback when the input value changes immediately (before debounce)
    on_immediate_change: Callback<String>,
    /// Placeholder text for the input
    #[prop(optional, default = "Search...")]
    placeholder: &'static str,
    /// Debounce delay in milliseconds
    #[prop(optional, default = 500)]
    debounce_ms: u32,
) -> impl IntoView {
    // Store the timeout ID so we can cancel it
    let debounce_timeout = RwSignal::new(None::<i32>);

    // Handle search input change with proper debouncing
    let on_input = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);

        // Immediately update the input signal (invoke callback directly)
        on_immediate_change.run(value.clone());

        // Cancel the previous timeout if it exists
        if let Some(timeout_id) = debounce_timeout.get() {
            web_sys::window()
                .unwrap()
                .clear_timeout_with_handle(timeout_id);
        }

        // Set new timeout for debounced callback
        let callback = Closure::once(move || {
            on_change.run(value);
            debounce_timeout.set(None); // Clear timeout ID after it fires
        });

        let timeout_id = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                debounce_ms as i32,
            )
            .unwrap();

        debounce_timeout.set(Some(timeout_id));
        callback.forget();
    };

    view! {
        <input
            type="text"
            placeholder=placeholder
            prop:value=move || value.get()
            on:input=on_input
            class="w-full rounded-lg border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none"
        />
    }
}
