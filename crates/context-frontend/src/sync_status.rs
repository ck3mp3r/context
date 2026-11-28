//! The sync state the indicator shows, and the sentence it renders for it.
//!
//! The indicator has one surface and one vocabulary, so the wording lives here once: a state in,
//! one sentence out. The dirty rule is a heuristic the indicator owns — the backend reports git
//! cleanliness and row counts, not whether the two sides differ. Nothing here imports the app or
//! the view layer, which is what lets the host-compiled test binary include this file directly
//! instead of linking the wasm lib.

/// What the indicator is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
    /// No cycle is running; the indicator reports whether local work is waiting.
    Idle,
    /// A cycle is running: import, then export.
    Syncing,
    /// The last cycle was rejected; `message` may carry the server's reason.
    Failed,
}

/// The state the indicator renders: the phase, whether local work waits, and any failure detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncIndicator {
    pub phase: SyncPhase,
    pub dirty: bool,
    pub message: Option<String>,
}

/// Sentence for an indicator, used as the icon's accessible title.
///
/// The phase decides the sentence, and the dirty flag only shapes the settled one: a running cycle
/// and a failure both say their own thing regardless of local work. A failure message that is only
/// whitespace counts as absent, so a half-written reason never renders an empty tail.
pub fn sync_title(indicator: &SyncIndicator) -> String {
    match indicator.phase {
        SyncPhase::Syncing => "Syncing…".to_string(),
        SyncPhase::Failed => match indicator.message.as_deref().map(str::trim) {
            Some(detail) if !detail.is_empty() => format!("Sync failed: {detail}"),
            _ => "Sync failed".to_string(),
        },
        SyncPhase::Idle if indicator.dirty => "Pending changes — click to sync".to_string(),
        SyncPhase::Idle => "Synced".to_string(),
    }
}

/// Whether the two sides of sync disagree.
///
/// Dirty means either the sync directory holds uncommitted changes, or the database and the sync
/// files hold a different number of rows. The sync files include tombstoned rows by design, because
/// deletion propagation needs them, so the like-for-like comparison is live rows plus tombstones
/// against file lines. Counts stay a heuristic: an edit that keeps the totals equal reads as clean.
pub fn dirty_from_status(
    db_total: u64,
    tombstones_total: u64,
    files_total: u64,
    git_clean: bool,
) -> bool {
    !git_clean || db_total + tombstones_total != files_total
}
