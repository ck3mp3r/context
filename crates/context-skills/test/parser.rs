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
