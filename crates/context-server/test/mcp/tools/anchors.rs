//! Tests for anchor resolution and section etags.

use super::{AnchorError, compute_section_etag, resolve_anchor};

const DOC: &str = "# Title\nIntro line\n\n## Foo\nFoo body one\nFoo body two\n\n### Sub\nSub body\n\n## Bar\nBar body";

fn resolved(content: &str, anchor: &str) -> (usize, usize) {
    resolve_anchor(content, anchor).expect("anchor should resolve")
}

#[test]
fn heading_anchor_resolves_to_its_section() {
    assert_eq!(
        resolved(DOC, "## Foo"),
        (4, 10),
        "section must end before the next heading of the same or higher level"
    );
    assert_eq!(
        resolved(DOC, "### Sub"),
        (8, 10),
        "a deeper heading keeps its own lines until the next same-or-higher heading"
    );
    assert_eq!(
        resolved(DOC, "## Bar"),
        (11, 12),
        "a trailing section runs to the last line"
    );
}

#[test]
fn heading_anchor_matches_the_heading_level_only() {
    let content = "## Foo\na\n### Foo\nb";

    assert_eq!(
        resolved(content, "## Foo"),
        (1, 4),
        "a level-2 anchor must ignore a deeper '### Foo' heading"
    );
    assert_eq!(
        resolved(content, "### Foo"),
        (3, 4),
        "a level-3 anchor must match only the '### Foo' heading"
    );
    assert!(
        matches!(
            resolve_anchor(content, "# Foo"),
            Err(AnchorError::NotFound { .. })
        ),
        "level equality is part of the match"
    );
}

#[test]
fn duplicate_headings_are_ambiguous() {
    let content = "## Foo\na\n## Foo\nb";

    match resolve_anchor(content, "## Foo") {
        Err(AnchorError::Ambiguous { anchor, lines }) => {
            assert_eq!(anchor, "## Foo");
            assert_eq!(
                lines,
                vec![1, 3],
                "the error must carry every candidate line"
            );
        }
        other => panic!("expected an ambiguous error, got: {other:?}"),
    }
}

#[test]
fn heading_titles_match_case_insensitively_and_ignore_surrounding_space() {
    assert_eq!(resolved(DOC, "## FOO"), (4, 10));
    assert_eq!(
        resolved(DOC, "  ## Foo  "),
        (4, 10),
        "the anchor is trimmed"
    );
}

#[test]
fn missing_heading_title_is_not_found() {
    match resolve_anchor(DOC, "## Missing") {
        Err(AnchorError::NotFound { anchor }) => assert_eq!(anchor, "## Missing"),
        other => panic!("expected a not-found error, got: {other:?}"),
    }
}

#[test]
fn hashes_without_a_space_are_text_anchors() {
    let content = "##NotAHeading\nbody";

    assert_eq!(
        resolved(content, "##NotAHeading"),
        (1, 1),
        "a heading needs 1-6 hashes followed by a space"
    );

    let content = "####### Seven\nbody";
    assert_eq!(
        resolved(content, "####### Seven"),
        (1, 1),
        "seven hashes are not a heading"
    );
}

#[test]
fn text_anchor_resolves_to_the_single_matching_line() {
    assert_eq!(
        resolved(DOC, "Foo body two"),
        (6, 6),
        "a text anchor covers only its own line"
    );
}

#[test]
fn text_anchor_is_case_sensitive_and_reports_candidates() {
    assert!(
        matches!(
            resolve_anchor(DOC, "foo body two"),
            Err(AnchorError::NotFound { .. })
        ),
        "text matching is case-sensitive"
    );

    match resolve_anchor(DOC, "Foo body") {
        Err(AnchorError::Ambiguous { anchor, lines }) => {
            assert_eq!(anchor, "Foo body");
            assert_eq!(
                lines,
                vec![5, 6],
                "the error must carry every candidate line"
            );
        }
        other => panic!("expected an ambiguous error, got: {other:?}"),
    }
}

#[test]
fn text_anchor_without_a_match_is_not_found() {
    assert!(
        matches!(
            resolve_anchor(DOC, "nothing here"),
            Err(AnchorError::NotFound { .. })
        ),
        "an unmatched text anchor must report not-found"
    );
}

#[test]
fn compute_section_etag_is_first_four_bytes_of_sha256() {
    use sha2::Digest;

    let section = "## Foo\nFoo body one";
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, section.as_bytes());
    let digest = hasher.finalize();
    let expected: String = digest
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect();

    let actual = compute_section_etag(section);

    assert_eq!(actual, expected, "section etag must hash the section text");
    assert_eq!(actual.len(), 8, "section etag must be 8 hex characters");
    assert_eq!(actual, actual.to_ascii_lowercase());
    assert_ne!(
        actual,
        compute_section_etag("## Foo\nFoo body two"),
        "changing the section text must change the etag"
    );
}

#[test]
fn section_etag_ignores_line_shift() {
    let section = "## Foo\nFoo body one\nFoo body two";
    let shifted = format!("A\nB\nC\nD\nE\n{section}");

    let (start, end) = resolved(&shifted, "## Foo");
    let lines: Vec<&str> = shifted.lines().collect();
    let resolved_text = lines[start - 1..end].join("\n");

    assert_eq!(resolved_text, section, "the section text must not change");
    assert_eq!(
        compute_section_etag(&resolved_text),
        compute_section_etag(section),
        "the section etag must survive line shifts above the section"
    );
}
