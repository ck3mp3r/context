//! Human phrases for the update status the pill shows.
//!
//! The pill is the only surface that reports an update, so the wording lives here once: a status
//! string in, one sentence out. Nothing here imports the app or the view layer, which is what lets
//! the host-compiled test binary include this file directly instead of linking the wasm lib.

/// Phrase for a status the pill reports, or `None` when it reports nothing.
///
/// `idle` and every unrecognised status render nothing: the pill is invisible between updates.
/// An `available` status names the candidate version when it has one.
pub fn update_phrase(status: &str, candidate_version: Option<&str>) -> Option<String> {
    let phrase = match status {
        "checking" => "Checking for updates…".to_string(),
        "downloading" => "Downloading update…".to_string(),
        "installing" => "Finishing the update…".to_string(),
        "ready" => "Update installed — restart to apply".to_string(),
        "available" => {
            match candidate_version
                .map(str::trim)
                .filter(|version| !version.is_empty())
            {
                Some(version) => format!("Update {version} available"),
                None => "Update available".to_string(),
            }
        }
        _ => return None,
    };

    Some(phrase)
}

/// Message for a failed update: the detail the flow recorded, or a plain fallback.
///
/// A message that is only whitespace counts as absent, so a half-written failure never renders an
/// empty line where the reason belongs.
pub fn update_failure(message: Option<&str>) -> String {
    match message.map(str::trim) {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => "The update failed.".to_string(),
    }
}

/// Whether a dismissal hides the pill for this candidate version.
///
/// The dismissal names one candidate version, so a different candidate — which is how a newer
/// announcement arrives — brings the pill back. A payload that names no candidate is its own key,
/// so a bare notice stays dismissed until the GUI restarts.
pub fn update_dismissed(dismissed: Option<&str>, candidate_version: Option<&str>) -> bool {
    match dismissed {
        Some(version) => version == candidate_version.unwrap_or_default(),
        None => false,
    }
}
