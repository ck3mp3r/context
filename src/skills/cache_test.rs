use super::*;
use crate::db::SkillAttachment;
use crate::db::utils::generate_entity_id;
use crate::sync::set_base_path;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use std::fs;
use std::sync::Once;

/// Global test setup - runs once across all tests in this module.
///
/// Sets a shared base path for all cache tests to avoid race conditions
/// when tests run in parallel. Each test uses unique skill names to
/// avoid conflicts.
static INIT: Once = Once::new();

fn setup_test_env() {
    INIT.call_once(|| {
        // Set a global test base path that all tests will share
        let test_base = std::env::temp_dir().join("c5t-cache-test-global");
        set_base_path(test_base.clone());

        // Clean up any previous test artifacts
        let _ = std::fs::remove_dir_all(&test_base);

        // Create fresh base directory
        std::fs::create_dir_all(&test_base).expect("Failed to create test base directory");
    });
}

#[test]
fn test_parse_skill_name_from_content() {
    // Valid skill content
    let content = r#"---
name: deploy-kubernetes
description: Deploy apps to K8s cluster
---

# Instructions
Deploy stuff"#;
    assert_eq!(
        parse_skill_name_from_content(content).unwrap(),
        "deploy-kubernetes"
    );

    // With extra frontmatter fields
    let content = r#"---
name: pdf-processing
description: Process PDFs
license: MIT
compatibility: Claude
---

# Instructions"#;
    assert_eq!(
        parse_skill_name_from_content(content).unwrap(),
        "pdf-processing"
    );

    // Missing name field should error
    let content = r#"---
description: No name field
---
"#;
    assert!(parse_skill_name_from_content(content).is_err());

    // Invalid frontmatter should error
    let content = "No frontmatter here";
    assert!(parse_skill_name_from_content(content).is_err());
}

#[test]
fn test_extract_attachments_creates_skill_md() {
    setup_test_env();

    // Use unique skill name to avoid conflicts with parallel tests
    let unique_id = generate_entity_id();
    let skill_name = format!("skill-md-test-{}", unique_id);

    // Skill content with frontmatter
    let skill_content = format!(
        r#"---
name: {}
description: A test skill
license: MIT
---

# Instructions

This is a test skill for cache validation."#,
        skill_name
    );

    // Create test attachments
    let script_content = "#!/bin/bash\necho 'test'";
    let attachments = vec![SkillAttachment {
        id: generate_entity_id(),
        skill_id: generate_entity_id(),
        type_: "script".to_string(),
        filename: "scripts/deploy.sh".to_string(),
        content: BASE64.encode(script_content),
        content_hash: "abc123".to_string(),
        mime_type: Some("text/x-shellscript".to_string()),
        created_at: None,
        updated_at: None,
    }];

    // Extract attachments - should use skill_name for directory
    let cache_dir = extract_attachments(&skill_name, &skill_content, &attachments).unwrap();

    // Verify cache directory uses skill name (not ID)
    assert!(cache_dir.to_string_lossy().ends_with(&skill_name));

    // Verify SKILL.md was created
    let skill_md_path = cache_dir.join("SKILL.md");
    assert!(skill_md_path.exists(), "SKILL.md should be created");

    // Verify SKILL.md content matches input
    let read_content = fs::read_to_string(&skill_md_path).unwrap();
    assert_eq!(read_content, skill_content);

    // Verify attachments were also extracted
    assert!(cache_dir.join("scripts/deploy.sh").exists());
    let script_read = fs::read_to_string(cache_dir.join("scripts/deploy.sh")).unwrap();
    assert_eq!(script_read, script_content);

    // Cleanup - remove this test's cache directory
    invalidate_cache(&skill_name).unwrap();
    assert!(!cache_dir.exists());
}

#[test]
fn test_get_skills_cache_dir() {
    let cache_dir = get_skills_cache_dir();
    // Should use XDG data dir + skills
    assert!(cache_dir.to_string_lossy().contains("skills"));
    assert!(cache_dir.to_string_lossy().contains("c5t"));
}

#[test]
fn test_get_skill_cache_dir() {
    let cache_dir = get_skill_cache_dir("my-skill");
    // Should use XDG data dir + skills/<name>
    assert!(cache_dir.to_string_lossy().contains("skills/my-skill"));
    assert!(cache_dir.to_string_lossy().contains("c5t"));
}

#[test]
fn test_extract_attachments() {
    setup_test_env();

    // Use unique skill name to avoid conflicts with parallel tests
    let unique_id = generate_entity_id();
    let skill_name = format!("attachments-test-{}", unique_id);

    // Skill content with matching skill name
    let skill_content = format!(
        r#"---
name: {}
description: Test skill for attachments
---

# Instructions
Test content"#,
        skill_name
    );

    // Create test attachments with relative paths (like real scanner output)
    let script_content = "#!/bin/bash\necho 'Hello'";
    let reference_content = "# Documentation\n\nThis is a reference.";
    let nested_content = "<xml/>";

    let skill_id = generate_entity_id();
    let attachments = vec![
        SkillAttachment {
            id: generate_entity_id(),
            skill_id: skill_id.clone(),
            type_: "script".to_string(),
            filename: "scripts/test.sh".to_string(), // Relative path
            content: BASE64.encode(script_content),
            content_hash: "abc123".to_string(),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at: None,
            updated_at: None,
        },
        SkillAttachment {
            id: generate_entity_id(),
            skill_id: skill_id.clone(),
            type_: "reference".to_string(),
            filename: "README.md".to_string(), // Root-level file
            content: BASE64.encode(reference_content),
            content_hash: "def456".to_string(),
            mime_type: Some("text/markdown".to_string()),
            created_at: None,
            updated_at: None,
        },
        SkillAttachment {
            id: generate_entity_id(),
            skill_id: skill_id.clone(),
            type_: "reference".to_string(),
            filename: "ooxml/document.xml".to_string(), // Nested path
            content: BASE64.encode(nested_content),
            content_hash: "ghi789".to_string(),
            mime_type: Some("application/xml".to_string()),
            created_at: None,
            updated_at: None,
        },
    ];

    // Extract attachments
    let cache_dir = extract_attachments(&skill_name, &skill_content, &attachments).unwrap();

    // Verify cache uses skill name
    assert!(cache_dir.to_string_lossy().ends_with(&skill_name));

    // Verify SKILL.md exists
    assert!(cache_dir.join("SKILL.md").exists());
    let skill_md_read = fs::read_to_string(cache_dir.join("SKILL.md")).unwrap();
    assert_eq!(skill_md_read, skill_content);

    // Verify files exist at correct paths
    assert!(cache_dir.join("scripts/test.sh").exists());
    assert!(cache_dir.join("README.md").exists());
    assert!(cache_dir.join("ooxml/document.xml").exists());

    // Verify content
    let script_read = fs::read_to_string(cache_dir.join("scripts/test.sh")).unwrap();
    assert_eq!(script_read, script_content);

    let reference_read = fs::read_to_string(cache_dir.join("README.md")).unwrap();
    assert_eq!(reference_read, reference_content);

    let nested_read = fs::read_to_string(cache_dir.join("ooxml/document.xml")).unwrap();
    assert_eq!(nested_read, nested_content);

    // Verify executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script_meta = fs::metadata(cache_dir.join("scripts/test.sh")).unwrap();
        let perms = script_meta.permissions();
        assert_eq!(perms.mode() & 0o111, 0o111); // Check executable bits
    }

    // Cleanup - remove this test's cache directory
    invalidate_cache(&skill_name).unwrap();
    assert!(!cache_dir.exists());
}

#[test]
fn test_invalidate_cache() {
    setup_test_env();

    // Use unique skill name to avoid conflicts with parallel tests
    let unique_id = generate_entity_id();
    let skill_name = format!("invalidate-test-{}", unique_id);

    let cache_dir = get_skill_cache_dir(&skill_name);

    // Create cache directory
    fs::create_dir_all(&cache_dir).unwrap();
    assert!(cache_dir.exists());

    // Invalidate
    invalidate_cache(&skill_name).unwrap();
    assert!(!cache_dir.exists());

    // Invalidating non-existent cache should not error
    invalidate_cache(&skill_name).unwrap();
}
