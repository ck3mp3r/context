//! GitHub release lookup for the updater.
//!
//! The updater plugin has no channel logic: it fetches a manifest from whatever
//! endpoint it is handed. So the app picks the release itself — filter the
//! release list by channel, order what remains by version core and publication
//! time, and hand the caller the winning tag's `latest.json` asset URL.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::Deserialize;

/// GitHub repository the updater reads releases from.
pub(crate) const GITHUB_REPO: &str = "ck3mp3r/context";

/// User agent GitHub requires on API requests.
const USER_AGENT: &str = "context-updater";

/// Release list page size; this repository has far fewer releases than this.
const RELEASES_PER_PAGE: usize = 100;

/// Channel value that accepts stable releases only.
const STABLE_CHANNEL: &str = "stable";

/// Environment variable carrying the token the release fetch authenticates with.
const TOKEN_ENV: &str = "C5T_GITHUB_TOKEN";

/// How much of a non-JSON response body the failure quotes.
const BODY_HEAD_CHARS: usize = 120;

/// A release the updater can install.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Version parsed from the release tag.
    pub version: Version,
    /// Release tag, as GitHub spells it.
    pub tag: String,
    /// Updater manifest asset of this release.
    pub manifest_url: String,
    /// Whether the release is flagged as a pre-release.
    pub is_prerelease: bool,
    /// Release notes: the release body.
    pub notes: Option<String>,
}

/// One release of the GitHub release list, reduced to the fields selection reads.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitHubRelease {
    /// Tag of the release; the version lives here.
    pub(crate) tag_name: String,
    /// Whether GitHub flags the release as a pre-release.
    pub(crate) prerelease: bool,
    /// Whether the release is still a draft.
    pub(crate) draft: bool,
    /// Publication time, as GitHub spells it; drafts carry none.
    pub(crate) published_at: Option<String>,
    /// Release body, carried into the candidate as notes.
    pub(crate) body: Option<String>,
}

/// Fetch the release list, in the order GitHub reports it.
///
/// The fetch authenticates with the token in `C5T_GITHUB_TOKEN` when one is set,
/// and names the cause of a failure that the JSON decoder cannot report: an
/// exhausted rate limit, or a body that is not JSON because the network path is
/// intercepted.
pub(crate) async fn fetch_releases(client: &reqwest::Client) -> Result<Vec<GitHubRelease>, String> {
    let url =
        format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page={RELEASES_PER_PAGE}");

    let mut request = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");

    if let Some(authorization) = auth_header(std::env::var(TOKEN_ENV).ok().as_deref()) {
        request = request.header(reqwest::header::AUTHORIZATION, authorization);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to reach the GitHub API: {error}"))?;

    // Every header is read before the body: reading the body consumes the response.
    let remaining = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok());
    let reset = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok());
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    // GitHub reports an exhausted budget with 403, so this check runs before the
    // status check and on every status.
    if let Some(error) = rate_limit_error(remaining, reset) {
        return Err(error);
    }

    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub API returned {status}"));
    }

    let body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read the GitHub release list body: {error}"))?;

    // An intercepting middlebox answers 2xx with its own page, so the body itself
    // is checked before the decoder reports it as broken JSON.
    if let Some(error) = content_type_error(content_type.as_deref(), &body) {
        return Err(error);
    }

    serde_json::from_str::<Vec<GitHubRelease>>(&body)
        .map_err(|error| format!("Failed to parse the GitHub release list: {error}"))
}

/// Pick the release `channel` should install, given the running identity.
///
/// Drafts, releases the channel excludes, and tags that are not a version are
/// ignored. What remains is ordered by numeric core, then by publication time.
/// The tag's build id never orders anything: it is a short sha, and shas compare
/// as text, not as time.
///
/// The winner is offered only when it is newer than the running build:
///
/// - a lower core is never offered, so a channel cannot downgrade the app
/// - an equal core with a different build id is a newer build of the same version
/// - an equal core with the same build id is this build, so nothing is offered
/// - an equal core whose tag repeats a pre-release running version is this release,
///   so nothing is offered: the tag carries the source-branch sha while the build
///   id is the release-branch head, so the two never match
/// - that version rule reads only a running pre-release version, so a bare-version
///   dev build stays eligible for the stable release of its own core
/// - an empty `current_build_id` cannot be told apart from another build of the
///   same core, so an equal core is treated as this build; offering the same
///   release on every check would loop forever
pub(crate) fn select_candidate(
    releases: &[GitHubRelease],
    channel: &str,
    current_version: &str,
    current_build_id: &str,
) -> Option<Candidate> {
    let current = parse_version(current_version)?;
    let current_core = core(&current);
    let mut winner: Option<(Version, &GitHubRelease, Option<DateTime<Utc>>)> = None;

    for release in releases {
        if release.draft || (channel == STABLE_CHANNEL && release.prerelease) {
            continue;
        }
        let Some(version) = parse_version(&release.tag_name) else {
            continue;
        };
        let at = publication_time(release);

        // A tie keeps the release seen first, so the result never depends on how
        // the comparison breaks ties.
        let better = match &winner {
            None => true,
            Some((best, _, best_at)) => (core(&version), at) > (core(best), *best_at),
        };
        if better {
            winner = Some((version, release, at));
        }
    }

    let (version, release, _) = winner?;
    let winner_core = core(&version);

    if winner_core < current_core {
        return None;
    }
    if winner_core == current_core
        && (current_build_id.is_empty()
            || version.pre.as_str() == current_build_id
            || (!current.pre.is_empty() && current == version))
    {
        return None;
    }

    Some(Candidate {
        version,
        tag: release.tag_name.clone(),
        manifest_url: manifest_url(&release.tag_name),
        is_prerelease: release.prerelease,
        notes: release.body.clone(),
    })
}

/// Updater manifest asset of the release tagged `tag`.
///
/// Release assets live under the tag, so the URL is formed from the tag the
/// selection picked.
pub fn manifest_url(tag: &str) -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/download/{tag}/latest.json")
}

/// Parse a tag or version, ignoring a single leading `v`.
fn parse_version(raw: &str) -> Option<Version> {
    Version::from_str(raw.strip_prefix('v').unwrap_or(raw)).ok()
}

/// Numeric core of `version`, the only part of it that orders one version above
/// another.
fn core(version: &Version) -> (u64, u64, u64) {
    (version.major, version.minor, version.patch)
}

/// Publication time of `release`, or None when the field is absent or is not an
/// RFC 3339 timestamp; an undated release sorts below every dated one.
fn publication_time(release: &GitHubRelease) -> Option<DateTime<Utc>> {
    let raw = release.published_at.as_deref()?;

    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// Why `content_type` rules the body out as a release list, or None when the body
/// is JSON.
///
/// A proxy, a captive portal, or an intercepting middlebox answers with its own
/// page and a content type of its own, so the header names the cause the decoder
/// cannot.
pub(crate) fn content_type_error(content_type: Option<&str>, body: &str) -> Option<String> {
    let content_type = content_type.map(str::trim).unwrap_or_default();
    if content_type.starts_with("application/json") {
        return None;
    }

    let named = if content_type.is_empty() {
        "none"
    } else {
        content_type
    };
    let head: String = body.chars().take(BODY_HEAD_CHARS).collect();

    Some(format!(
        "GitHub API answered with {named}; the network path to api.github.com is \
         intercepted or blocked; first response bytes: {head}"
    ))
}

/// Why the request budget is spent, or None when requests remain.
///
/// GitHub answers an exhausted budget with 403 and `x-ratelimit-remaining: 0`, so
/// the remaining count decides and the reset timestamp is reported as UTC when it
/// parses.
pub(crate) fn rate_limit_error(remaining: Option<&str>, reset: Option<&str>) -> Option<String> {
    if remaining?.trim() != "0" {
        return None;
    }

    let resets_at = reset
        .and_then(|raw| raw.trim().parse::<i64>().ok())
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .map_or_else(|| "unreadable".to_string(), |at| at.to_rfc3339());

    Some(format!(
        "GitHub API rate limit exhausted; the budget refills at {resets_at}. \
         Set {TOKEN_ENV} to a personal access token to raise it."
    ))
}

/// Authorization header value for `token`, or None when the token is absent or
/// blank so the request stays anonymous.
pub(crate) fn auth_header(token: Option<&str>) -> Option<String> {
    let token = token?.trim();
    if token.is_empty() {
        return None;
    }

    Some(format!("Bearer {token}"))
}
