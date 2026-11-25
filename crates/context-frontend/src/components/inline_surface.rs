//! One inline overlay surface: a backdrop and a panel, rendered where the surface lives.
//!
//! Decision 12 forbids a surface under `document.body`. thaw's `Dialog` and `OverlayDrawer` render
//! through a `Teleport` to the body and do not forward its `mount` option, so the three surfaces
//! that used them render through this component instead: a normal descendant of the tab wrapper,
//! positioned against the viewport with `position: fixed`.
//!
//! A hidden tab stays mounted under `display: none`, so an open surface inside it paints nothing
//! and takes no pointer or key events. No extra hide logic exists.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::tabs::use_tab_scope;

/// The elements a Tab press may land on inside an open surface.
const FOCUSABLE: &str =
    "button, [href], input, select, textarea, [tabindex]:not([tabindex=\"-1\"])";

/// The attribute that marks the scroll-lock stylesheet a surface owns.
const SCROLL_LOCK_ATTRIBUTE: &str = "data-c5t-scroll-lock";

// region:    --- Types

/// Where a surface's panel sits inside the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceLayout {
    /// A centred dialog, 90% of the viewport tall.
    Modal,
    /// A full-height panel pinned to the right edge.
    RightDrawer,
}

impl SurfaceLayout {
    /// The root's classes: they place the panel and nothing else.
    fn root_class(self) -> &'static str {
        match self {
            Self::Modal => "inline-surface inline-surface--modal",
            Self::RightDrawer => "inline-surface inline-surface--drawer",
        }
    }

    /// The panel's classes: they size and animate the panel.
    fn panel_class(self) -> &'static str {
        match self {
            Self::Modal => "inline-surface-panel inline-surface-panel--modal",
            Self::RightDrawer => "inline-surface-panel inline-surface-panel--drawer",
        }
    }
}

// endregion: --- Types

// region:    --- Components

/// An overlay surface that renders inside the tab wrapper that owns it.
///
/// The surface stays mounted while closed and hides with `display: none`, so opening it restarts
/// the fade and the slide, and the surface keeps its state across a tab switch (decision 13).
#[component]
pub fn InlineSurface(
    /// The DOM token that names this surface in `data-surface`.
    name: &'static str,
    /// The surface's accessible name.
    label: &'static str,
    /// Where the panel sits in the viewport.
    layout: SurfaceLayout,
    /// The surface's own panel classes, which carry a size the layout does not.
    #[prop(optional)] panel_class: Option<&'static str>,
    /// Open state. Escape and a backdrop click set it to `false`.
    open: RwSignal<bool>,
    children: Children,
) -> impl IntoView {
    let panel_ref = NodeRef::<leptos::html::Div>::new();
    let scroll_lock = StoredValue::new_local(None::<web_sys::Element>);
    let tab = use_tab_scope();

    // The page scrolls under a closed surface, and under an open surface in a hidden tab: a hidden
    // wrapper is `display: none`, so its surface must not hold the page.
    Effect::new(move |_| {
        let active = tab.as_ref().is_none_or(|tab| tab.is_active());
        set_scroll_lock(scroll_lock, open.get() && active);
    });
    on_cleanup(move || set_scroll_lock(scroll_lock, false));

    // The panel takes focus on open, so a Tab press cycles inside it.
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        if let Some(panel) = panel_ref.get() {
            let _ = panel.focus();
        }
    });

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() != "Tab" {
            return;
        }
        if let Some(panel) = panel_ref.get() {
            ev.prevent_default();
            trap_tab(&panel, ev.shift_key());
        }
    };

    // Escape reaches the open surface wherever focus sits, which is how a dialog closes after a
    // click moved focus onto the card that opened it.
    let escape_listener = window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.key() == "Escape" && open.get_untracked() {
            ev.prevent_default();
            open.set(false);
        }
    });
    on_cleanup(move || escape_listener.remove());

    let on_backdrop_click = move |_| open.set(false);

    view! {
        <div
            class=layout.root_class()
            style=move || if open.get() { "" } else { "display:none" }
            data-surface=name
            role="dialog"
            aria-modal="true"
            aria-label=label
            on:keydown=on_keydown
        >
            <div
                class="inline-surface-backdrop"
                data-surface-backdrop=""
                aria-hidden="true"
                on:click=on_backdrop_click
            ></div>
            <div
                node_ref=panel_ref
                class=move || match panel_class {
                    Some(extra) => format!("{} {extra}", layout.panel_class()),
                    None => layout.panel_class().to_string(),
                }
                data-surface-panel=""
                tabindex="-1"
            >
                {children()}
            </div>
        </div>
    }
}

// endregion: --- Components

// region:    --- Support

/// Hold or release this surface's scroll lock. Each surface owns its own stylesheet, so two open
/// surfaces never release each other's lock.
fn set_scroll_lock(slot: StoredValue<Option<web_sys::Element>, LocalStorage>, lock: bool) {
    slot.update_value(|held| match (lock, held.take()) {
        (true, None) => *held = insert_scroll_lock_style(),
        (false, Some(style)) => style.remove(),
        (true, Some(style)) => *held = Some(style),
        (false, None) => {}
    });
}

/// Append the stylesheet that stops the document from scrolling.
fn insert_scroll_lock_style() -> Option<web_sys::Element> {
    let document = document();
    let style = document.create_element("style").ok()?;
    style.set_attribute(SCROLL_LOCK_ATTRIBUTE, "").ok()?;
    style.set_text_content(Some("html { overflow: hidden; }"));
    document.document_element()?.append_child(&style).ok()?;
    Some(style)
}

/// Keep a Tab press inside the panel: step to the next element, and wrap at either end.
fn trap_tab(panel: &web_sys::HtmlElement, backwards: bool) {
    let elements = focusable_elements(panel);
    let Some(last) = elements.len().checked_sub(1) else {
        let _ = panel.focus();
        return;
    };

    let active = document().active_element();
    let position = active.as_ref().and_then(|active| {
        elements
            .iter()
            .position(|element| element.is_same_node(Some(active)))
    });

    let target = match (position, backwards) {
        (Some(0), true) => elements.get(last),
        (Some(index), true) => elements.get(index - 1),
        (Some(index), false) if index == last => elements.first(),
        (Some(index), false) => elements.get(index + 1),
        (None, _) => elements.first(),
    };

    if let Some(target) = target {
        let _ = target.focus();
    }
}

/// The elements inside the panel that a Tab press can reach, in document order.
fn focusable_elements(panel: &web_sys::HtmlElement) -> Vec<web_sys::HtmlElement> {
    let Ok(nodes) = panel.query_selector_all(FOCUSABLE) else {
        return Vec::new();
    };

    (0..nodes.length())
        .filter_map(|index| nodes.get(index))
        .filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

// endregion: --- Support
