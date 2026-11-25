//! The tab shell: the pure navigation model and its reactive registry.
//!
//! Rendering belongs to the container, URL wiring to the router, and keyboard handling to the
//! strip. This module owns only what the design settles as data: identity, close, cycle, and strip
//! rows.

mod pure;
mod registry;

pub use pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, TabRouteKind, Tabs};
pub use registry::TabRegistry;
