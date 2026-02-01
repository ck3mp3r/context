//! SKILL.md parsing (simplified)
//!
//! Parses SKILL.md files to extract ONLY name and description for DB indexing.
//! The full SKILL.md content is stored as-is - LLMs parse frontmatter themselves.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Failed to read file: {0}")]
    ReadError(String),

    #[error("Invalid YAML frontmatter: {0}")]
    YamlError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid frontmatter format")]
    InvalidFormat,
}

/// Parsed SKILL.md content - minimal extraction for DB indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMd {
    /// Skill name (required for DB indexing)
    pub name: String,
    /// Description (required for FTS5 search)
    pub description: String,
    /// Full SKILL.md content (YAML frontmatter + Markdown body)
    #[serde(skip)]
    pub content: String,
}

/// Minimal frontmatter structure for extracting only name + description
#[derive(Debug, Deserialize)]
struct MinimalFrontmatter {
    name: String,
    description: Option<String>,
}

/// Parse a SKILL.md file - extracts only name/description, returns full content
///
/// Expected format:
/// ```markdown
/// ---
/// name: skill-name
/// description: Short description
/// license: MIT (optional, stays in content)
/// compatibility: openai, anthropic (optional, stays in content)
/// allowed_tools: [tool1, tool2] (optional, stays in content)
/// ---
///
/// # Instructions
///
/// Markdown content here...
/// ```
pub fn parse_skill_md(path: &Path) -> Result<SkillMd, ParserError> {
    // 1. Read full file content
    let content = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ParserError::FileNotFound(path.display().to_string())
        } else {
            ParserError::ReadError(e.to_string())
        }
    })?;

    // 2. Extract YAML frontmatter (between --- delimiters)
    let (frontmatter, _body) = extract_frontmatter(&content)?;

    // 3. Parse ONLY name + description from YAML
    let minimal: MinimalFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(|e| ParserError::YamlError(e.to_string()))?;

    // 4. Validate required fields
    if minimal.name.is_empty() {
        return Err(ParserError::MissingField("name".to_string()));
    }

    let description = minimal
        .description
        .filter(|d| !d.is_empty())
        .ok_or_else(|| ParserError::MissingField("description".to_string()))?;

    // 5. Return name + description + full content
    Ok(SkillMd {
        name: minimal.name,
        description,
        content, // Full SKILL.md as-is!
    })
}

/// Extract YAML frontmatter and Markdown body from content
///
/// Expected format:
/// ```text
/// ---
/// key: value
/// ---
/// body content
/// ```
fn extract_frontmatter(content: &str) -> Result<(String, String), ParserError> {
    let lines: Vec<&str> = content.lines().collect();

    // Check for opening ---
    if lines.is_empty() || lines[0].trim() != "---" {
        return Err(ParserError::InvalidFormat);
    }

    // Find closing ---
    let closing_index = lines
        .iter()
        .skip(1)
        .position(|line| line.trim() == "---")
        .ok_or(ParserError::InvalidFormat)?
        + 1; // +1 because we skipped the first line

    // Extract frontmatter (between the --- markers)
    let frontmatter = lines[1..closing_index].join("\n");

    // Extract body (everything after closing ---)
    let body = if closing_index + 1 < lines.len() {
        lines[closing_index + 1..].join("\n")
    } else {
        String::new()
    };

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_minimal_skill_md() {
        // Create a temp file with minimal SKILL.md (name + description required)
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        let content = r#"---
name: test-skill
description: A minimal test skill
---

# Test Skill

This is a test skill.
"#;
        std::fs::write(&skill_path, content).unwrap();

        let result = parse_skill_md(&skill_path);
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A minimal test skill");
        assert!(skill.content.contains("# Test Skill"));
    }

    #[test]
    fn test_parse_full_skill_md() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        let content = r#"---
name: full-skill
description: A complete skill example
license: MIT
compatibility: openai, anthropic
allowed_tools: ["read", "write", "execute"]
---

# Full Skill

This skill demonstrates all available metadata fields.

## Usage

Use this skill for testing purposes.
"#;
        std::fs::write(&skill_path, content).unwrap();

        let result = parse_skill_md(&skill_path);
        assert!(result.is_ok());

        let skill = result.unwrap();
        assert_eq!(skill.name, "full-skill");
        assert_eq!(skill.description, "A complete skill example");
        // All other fields are in content, not parsed separately
        assert!(skill.content.contains("license: MIT"));
        assert!(skill.content.contains("compatibility: openai, anthropic"));
        assert!(skill.content.contains("# Full Skill"));
    }

    #[test]
    fn test_parse_missing_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        let content = r#"---
description: Missing name field
---

# Test
"#;
        std::fs::write(&skill_path, content).unwrap();

        let result = parse_skill_md(&skill_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParserError::YamlError(_)));
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        let content = "# No Frontmatter\n\nJust markdown content.";
        std::fs::write(&skill_path, content).unwrap();

        let result = parse_skill_md(&skill_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParserError::InvalidFormat));
    }

    #[test]
    fn test_parse_empty_instructions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        // Missing description should fail
        let content = r#"---
name: no-description
---
"#;
        std::fs::write(&skill_path, content).unwrap();

        let result = parse_skill_md(&skill_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParserError::MissingField(_)));
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let path = PathBuf::from("/this/file/does/not/exist.md");
        let result = parse_skill_md(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ParserError::FileNotFound(_)));
    }
}
