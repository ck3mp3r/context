//! The tab shell: the pure navigation model, its reactive registry, the keep-alive container, and
//! the minimal open path.
//!
//! Rendering of the strip belongs to its own module, URL wiring to the router, and keyboard handling
//! to the strip. This module owns what the design settles as data (identity, close, cycle, strip
//! rows), the wrappers that keep every tab mounted, and the one-tab open path.

mod container;
mod open;
mod pure;
mod registry;

pub use container::{TabContainer, TabScope, use_tab_scope};
pub use open::{open_surface, use_url_writer, write_active_url};
pub use pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, TabRouteKind, Tabs};
pub use registry::TabRegistry;
