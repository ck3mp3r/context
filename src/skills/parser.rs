//! SKILL.md parsing
//!
//! Parses SKILL.md files according to the Agent Skills specification:
//! - YAML frontmatter (metadata)
//! - Markdown body (instructions)

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

/// Parsed SKILL.md content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMd {
    // Required fields from YAML frontmatter
    pub name: String,

    // Optional fields from YAML frontmatter
    pub description: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Option<Vec<String>>,

    // Additional metadata (flexible JSON)
    #[serde(flatten)]
    pub metadata: Option<serde_json::Value>,

    // Markdown body (everything after frontmatter)
    pub instructions: String,
}

/// Parse a SKILL.md file
///
/// Expected format:
/// ```markdown
/// ---
/// name: skill-name
/// description: Optional description
/// license: MIT
/// compatibility: openai, anthropic
/// allowed_tools: [tool1, tool2]
/// ---
///
/// # Instructions
///
/// Markdown content here...
/// ```
pub fn parse_skill_md(_path: &Path) -> Result<SkillMd, ParserError> {
    // TODO: Implement SKILL.md parsing
    // 1. Read file
    // 2. Extract YAML frontmatter (between --- delimiters)
    // 3. Parse YAML to struct
    // 4. Extract Markdown body (everything after frontmatter)
    // 5. Validate required fields

    Err(ParserError::InvalidFormat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_minimal_skill_md() {
        let path = PathBuf::from("test_skill.md");
        let result = parse_skill_md(&path);
        assert!(result.is_err()); // Placeholder until implemented
    }
}
