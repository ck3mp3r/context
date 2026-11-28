//! Anchor resolution for note content.
//!
//! An anchor names a part of a note by heading or by text. Resolution is pure:
//! it reads the note content string and returns a 1-indexed inclusive line
//! range, the same convention as `LineRange`.

use sha2::Digest;

use crate::mcp::tools::notes::hash_prefix;

/// Error returned when an anchor does not resolve to exactly one range.
#[derive(Debug)]
pub(crate) enum AnchorError {
    /// No line matches the anchor.
    NotFound { anchor: String },
    /// More than one line matches the anchor; carries the candidate lines.
    Ambiguous { anchor: String, lines: Vec<usize> },
}

/// Resolve an anchor to a 1-indexed inclusive line range.
///
/// A trimmed anchor of 1-6 `#` characters followed by a space is a heading
/// anchor; any other anchor matches a unique line by substring.
pub(crate) fn resolve_anchor(content: &str, anchor: &str) -> Result<(usize, usize), AnchorError> {
    let anchor = anchor.trim();

    if let Some((level, title)) = parse_heading(anchor) {
        return resolve_heading(content, level, title, anchor);
    }

    resolve_text(content, anchor)
}

/// Compute the etag of a section from its text alone.
///
/// Line numbers are excluded so the etag survives line shifts above the
/// section. Returns an 8-character lowercase hex string.
pub(crate) fn compute_section_etag(section_text: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, section_text.as_bytes());
    hash_prefix(&hasher.finalize())
}

/// Resolve a heading anchor to its section.
fn resolve_heading(
    content: &str,
    level: usize,
    title: &str,
    anchor: &str,
) -> Result<(usize, usize), AnchorError> {
    let lines: Vec<&str> = content.lines().collect();

    let matches: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| heading_matches(line, level, title))
        .map(|(idx, _)| idx + 1)
        .collect();

    let start = single_match(anchor, matches)?;

    let end = lines
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, line)| heading_level(line).is_some_and(|line_level| line_level <= level))
        .map_or(lines.len(), |(idx, _)| idx);

    Ok((start, end))
}

/// Resolve a text anchor to the single line that contains it.
fn resolve_text(content: &str, anchor: &str) -> Result<(usize, usize), AnchorError> {
    let matches: Vec<usize> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(anchor))
        .map(|(idx, _)| idx + 1)
        .collect();

    let line = single_match(anchor, matches)?;

    Ok((line, line))
}

/// Require exactly one candidate line.
fn single_match(anchor: &str, matches: Vec<usize>) -> Result<usize, AnchorError> {
    match matches.as_slice() {
        [] => Err(AnchorError::NotFound {
            anchor: anchor.to_string(),
        }),
        [only] => Ok(*only),
        _ => Err(AnchorError::Ambiguous {
            anchor: anchor.to_string(),
            lines: matches,
        }),
    }
}

/// Return the heading level of a line, when the line starts with 1-6 `#`
/// characters followed by a space.
fn heading_level(line: &str) -> Option<usize> {
    let level = line.chars().take_while(|c| *c == '#').count();

    if (1..=6).contains(&level) && line[level..].starts_with(' ') {
        Some(level)
    } else {
        None
    }
}

/// Split a heading line into its level and its trimmed title.
fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let level = heading_level(line)?;
    Some((level, line[level + 1..].trim()))
}

/// Check a line against a heading anchor: same level, same title
/// (case-insensitive).
fn heading_matches(line: &str, level: usize, title: &str) -> bool {
    let Some((line_level, line_title)) = parse_heading(line) else {
        return false;
    };

    line_level == level && line_title.to_lowercase() == title.to_lowercase()
}

#[cfg(test)]
#[path = "../../../test/mcp/tools/anchors.rs"]
mod anchors_test;
