//! The minimal open path: open one tab and write its URL once.
//!
//! `open_surface` is the single-tab case. Key dedup beyond the registry's own identity rule,
//! parent-first for children, and the remaining card clicks arrive with the open/URL task.

use leptos_router::{NavigateOptions, hooks::use_navigate};

use super::pure::{TabId, TabKey};
use super::registry::TabRegistry;

/// Open one surface and focus it.
///
/// The registry already refuses a duplicate surface (decision 6), so opening a key that is open
/// focuses that tab and returns its id.
pub fn open_surface(registry: TabRegistry, key: TabKey) -> TabId {
    registry.open(key)
}

/// The only URL writer: one `replace` navigation, so no history entry is created
/// (decisions 11 and 21).
pub fn write_active_url(writer: &impl Fn(&str, NavigateOptions), url: &str) {
    writer(
        url,
        NavigateOptions {
            replace: true,
            ..Default::default()
        },
    );
}

/// The router's navigate function, bound once during component setup for [`write_active_url`].
pub fn use_url_writer() -> impl Fn(&str, NavigateOptions) + Clone {
    use_navigate()
}
