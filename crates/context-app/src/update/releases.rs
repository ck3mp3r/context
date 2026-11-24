//! GitHub release lookup for the updater.
//!
//! The updater plugin has no channel logic: it fetches a manifest from whatever
//! endpoint it is handed. So the app picks the release itself — filter the
//! release list by channel, take the highest semver tag, and hand the caller that
//! tag's `latest.json` asset URL.

use std::str::FromStr;

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
    /// Release body, carried into the candidate as notes.
    pub(crate) body: Option<String>,
}

/// Fetch the release list, in the order GitHub reports it.
pub(crate) async fn fetch_releases(client: &reqwest::Client) -> Result<Vec<GitHubRelease>, String> {
    let url =
        format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page={RELEASES_PER_PAGE}");

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Failed to reach the GitHub API: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub API returned {status}"));
    }

    response
        .json::<Vec<GitHubRelease>>()
        .await
        .map_err(|error| format!("Failed to parse the GitHub release list: {error}"))
}

/// Pick the release `channel` should install, given the running `current_version`.
///
/// Drafts, releases the channel excludes, and tags that are not a version are
/// ignored. The highest version wins; a winner that is not newer than the running
/// version yields no candidate, so the updater is never offered a downgrade or a
/// reinstall.
pub(crate) fn select_candidate(
    releases: &[GitHubRelease],
    channel: &str,
    current_version: &str,
) -> Option<Candidate> {
    let current = parse_version(current_version)?;

    releases
        .iter()
        .filter(|release| !release.draft)
        .filter(|release| channel != STABLE_CHANNEL || !release.prerelease)
        .filter_map(|release| Some((parse_version(&release.tag_name)?, release)))
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .filter(|(version, _)| version > &current)
        .map(|(version, release)| Candidate {
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
