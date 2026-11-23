//! Start-at-login hook.
//!
//! The service reads the stored `start_at_login` preference and hands it to
//! [`apply_start_at_login`]. The LaunchAgent writer lands in a later task; until
//! then the preference is stored only, and this module keeps the call site in
//! place so the writer plugs in without touching the service binary.

use context_core::Settings;

/// Apply the stored start-at-login preference.
///
/// The LaunchAgent writer fills this in. Until it exists the setting changes
/// nothing at runtime: the call site is what the service depends on.
pub fn apply_start_at_login(_settings: &Settings) {}
