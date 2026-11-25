//! The tab shell: the pure navigation model, its reactive registry, the keep-alive container, the
//! minimal open path, and the session a reload restores.
//!
//! Rendering of the strip belongs to its own module, URL wiring to the router, and keyboard handling
//! to the strip. This module owns what the design settles as data (identity, close, cycle, strip
//! rows), the wrappers that keep every tab mounted, the one-tab open path, and the stored session.

mod boot;
mod container;
mod liveness;
mod open;
mod pure;
mod registry;
mod session;

pub use boot::{resolve_titles, restore_or_boot, start_session_persistence};
pub use container::{TabContainer, TabScope, use_tab_scope};
pub use liveness::should_refetch;
pub use open::{focus_or_open, open_surface, surface_url, write_active_url};
pub use pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, TabRouteKind, Tabs};
pub use registry::TabRegistry;
pub use session::{
    BootPlan, BootSurface, SESSION_VERSION, StoredKey, StoredKind, StoredSession, StoredTab, TabTitles,
    boot_surface, storage_key, write_owned_url,
};
