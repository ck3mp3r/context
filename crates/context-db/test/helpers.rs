//! Tests for helpers.rs.

use crate::helpers::*;

#[test]
fn test_sanitize_fts5_query_simple() {
    assert_eq!(sanitize_fts5_query("rust"), Some("rust*".to_string()));
}

#[test]
fn test_sanitize_fts5_query_multiple_terms() {
    assert_eq!(
        sanitize_fts5_query("rust async"),
        Some("rust* async*".to_string())
    );
}

#[test]
fn test_sanitize_fts5_query_boolean_operators() {
    assert_eq!(
        sanitize_fts5_query("rust AND async"),
        Some("rust AND async".to_string())
    );
    assert_eq!(
        sanitize_fts5_query("backend OR frontend"),
        Some("backend OR frontend".to_string())
    );
    assert_eq!(
        sanitize_fts5_query("code NOT deprecated"),
        Some("code NOT deprecated".to_string())
    );
}

#[test]
fn test_sanitize_fts5_query_phrase() {
    assert_eq!(
        sanitize_fts5_query("\"exact match\""),
        Some("\"exact match\"".to_string())
    );
}

#[test]
fn test_sanitize_fts5_query_empty() {
    assert_eq!(sanitize_fts5_query(""), None);
    assert_eq!(sanitize_fts5_query("   "), None);
    assert_eq!(sanitize_fts5_query("\t\n"), None);
}

#[test]
fn test_sanitize_fts5_query_special_chars() {
    // Special FTS5 chars should be stripped
    assert_eq!(
        sanitize_fts5_query("hello@world#test"),
        Some("hello* world* test*".to_string())
    );
}

#[test]
fn test_sanitize_fts5_query_unbalanced_quotes() {
    // Odd number of quotes - should remove all quotes
    assert_eq!(
        sanitize_fts5_query("hello \"world"),
        Some("hello* world*".to_string())
    );
}

#[test]
fn test_sanitize_fts5_query_unicode() {
    // Unicode characters should be preserved
    assert_eq!(
        sanitize_fts5_query("Rust über"),
        Some("Rust* über*".to_string())
    );
}
