//! Shared utilities for CLI commands

use tabled::{Table, settings::Style};

/// Truncate a string with ellipsis if it exceeds max length
pub fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    }
}

/// Format optional tags vector for display
pub fn format_tags(tags: Option<&Vec<String>>) -> String {
    match tags {
        Some(t) if !t.is_empty() => t.join(", "),
        _ => "-".to_string(),
    }
}

/// Parse comma-separated tags string into vector
pub fn parse_tags(tags: Option<&str>) -> Option<Vec<String>> {
    tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
}

/// Apply consistent table styling
pub fn apply_table_style(table: &mut Table) {
    table.with(Style::rounded());
}
