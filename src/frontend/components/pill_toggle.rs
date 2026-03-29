use leptos::ev;
use leptos::prelude::*;

/// Catppuccin accent colors for pill toggles.
#[derive(Clone, Copy, PartialEq)]
pub enum PillColor {
    Blue,
    Green,
    Mauve,
}

impl PillColor {
    /// Returns (bg, border, hover_text, hover_border) class names for the active and inactive states.
    fn active_classes(self) -> (&'static str, &'static str, &'static str) {
        match self {
            PillColor::Blue => ("bg-ctp-blue", "border-ctp-blue", "text-ctp-base"),
            PillColor::Green => ("bg-ctp-green", "border-ctp-green", "text-ctp-base"),
            PillColor::Mauve => ("bg-ctp-mauve", "border-ctp-mauve", "text-ctp-base"),
        }
    }

    fn hover_classes(self) -> (&'static str, &'static str) {
        match self {
            PillColor::Blue => ("hover:text-ctp-blue", "hover:border-ctp-blue"),
            PillColor::Green => ("hover:text-ctp-green", "hover:border-ctp-green"),
            PillColor::Mauve => ("hover:text-ctp-mauve", "hover:border-ctp-mauve"),
        }
    }
}

/// A toggle pill/chip button with Catppuccin accent coloring.
///
/// # Example
/// ```rust
/// <PillToggle
///     label="Full"
///     active=Signal::derive(move || view.get() == "full")
///     color=PillColor::Blue
///     on_click=move |_| set_view.set("full".to_string())
/// />
/// ```
#[component]
pub fn PillToggle(
    /// Display label
    #[prop(into)]
    label: String,
    /// Whether this pill is currently active
    #[prop(into)]
    active: Signal<bool>,
    /// Accent color for the active state
    color: PillColor,
    /// Click handler
    on_click: impl Fn(ev::MouseEvent) + 'static,
) -> impl IntoView {
    let (active_bg, active_border, active_text) = color.active_classes();
    let (hover_text, hover_border) = color.hover_classes();

    view! {
        <button
            class="text-xs px-2.5 py-1 rounded-md border transition-colors"
            class=(active_bg, move || active.get())
            class=(active_text, move || active.get())
            class=(active_border, move || active.get())
            class=("bg-ctp-base", move || !active.get())
            class=("text-ctp-subtext0", move || !active.get())
            class=("border-ctp-surface2", move || !active.get())
            class=(hover_text, move || !active.get())
            class=(hover_border, move || !active.get())
            on:click=on_click
        >
            {label}
        </button>
    }
}
