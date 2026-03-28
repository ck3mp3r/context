/// Extract a display-friendly `org/repo` name from a git remote URL.
///
/// Handles HTTPS, SSH, and various URL formats:
/// - `https://github.com/org/repo.git` -> `org/repo`
/// - `https://github.com/org/repo` -> `org/repo`
/// - `git@github.com:org/repo.git` -> `org/repo`
/// - `ssh://git@github.com/org/repo.git` -> `org/repo`
///
/// Falls back to the original URL if parsing fails.
pub fn extract_repo_name(remote: &str) -> String {
    let stripped = remote.trim_end_matches('/').trim_end_matches(".git");

    // SSH shorthand: git@host:org/repo
    if let Some(path) = stripped.strip_prefix("git@") {
        if let Some((_host, path)) = path.split_once(':') {
            return path.to_string();
        }
    }

    // URL-style: https://host/org/repo or ssh://git@host/org/repo
    if let Some(after_scheme) = stripped
        .strip_prefix("https://")
        .or_else(|| stripped.strip_prefix("http://"))
        .or_else(|| stripped.strip_prefix("ssh://"))
    {
        // Skip host (and optional user@)
        let path = after_scheme.split_once('/').map(|(_, p)| p);
        if let Some(path) = path {
            // Take last two segments: org/repo
            let segments: Vec<&str> = path.split('/').collect();
            if segments.len() >= 2 {
                let org = segments[segments.len() - 2];
                let repo = segments[segments.len() - 1];
                return format!("{}/{}", org, repo);
            }
        }
    }

    // Fallback: return original
    remote.to_string()
}
