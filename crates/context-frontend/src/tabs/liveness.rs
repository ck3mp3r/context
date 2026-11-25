//! Scoped liveness: the rule that decides whether a view refetches on a websocket message.
//!
//! Decision 15: a hidden tab refetches only the data it has already loaded, and only on its own
//! triggers. A panel the user never opened holds no data, so it fetches nothing.

/// Whether a view should refetch in response to one websocket message.
///
/// `has_loaded` is the view's data being present: its data signal is `Some(..)`, or its fetch
/// effect has already run. `is_active` is the view being on screen. `msg_matches_entity` is the
/// message naming this view's data.
///
/// A view that shows and has not loaded yet still refetches, so the first message never races the
/// first fetch. A view that is neither loaded nor on screen is a panel the user never opened: it
/// fetches nothing.
pub fn should_refetch(has_loaded: bool, is_active: bool, msg_matches_entity: bool) -> bool {
    msg_matches_entity && (has_loaded || is_active)
}
