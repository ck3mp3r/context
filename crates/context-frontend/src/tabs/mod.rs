//! The tab shell: the pure navigation model, its reactive registry, the keep-alive container, the
//! minimal open path, and the session a reload restores.
//!
//! Rendering of the strip belongs to `strip.rs`, keyboard handling to `keyboard.rs`, and the one
//! native call — the window title — to `title.rs`. URL wiring belongs to the router. This module
//! owns what the design settles as data (identity, close, cycle, strip rows), the wrappers that keep
//! every tab mounted, the one-tab open path, and the stored session.

mod boot;
mod container;
mod keyboard;
mod liveness;
mod open;
mod pure;
mod registry;
mod route;
mod session;
mod strip;
mod title;

pub use boot::{
    BootPending, finish_boot, open_from_card, resolve_titles, restore_or_boot,
    start_session_persistence,
};
pub use container::{TabContainer, TabScope, use_tab_scope};
pub use keyboard::start_keyboard;
pub use liveness::should_refetch;
pub use open::{focus_or_open, open_surface, surface_url, write_active_url};
pub use pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, TabRouteKind, Tabs};
pub use registry::TabRegistry;
pub use route::{RouteKind, route_kind};
pub use session::{
    BootPlan, BootSurface, SESSION_VERSION, StoredKey, StoredKind, StoredSession, StoredTab,
    TabTitles, adopt_owned_url, boot_surface, storage_key, write_owned_url,
};
pub use strip::TabStrip;
pub use title::start_window_title;
