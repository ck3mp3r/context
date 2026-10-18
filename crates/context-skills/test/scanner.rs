use super::*;

#[test]
fn test_scan_skill_with_root_level_files() {
    // Create temp skill directory with files at root level (like docx skill)
    let temp_dir = std::env::temp_dir().join(format!("test-skill-root-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create SKILL.md (should be skipped)
    std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();

    // Create root-level reference files (like docx/docx-js.md)
    std::fs::write(temp_dir.join("reference.md"), "# Reference").unwrap();
    std::fs::write(temp_dir.join("forms.json"), r#"{"test": true}"#).unwrap();

    // Create LICENSE (should be skipped)
    std::fs::write(temp_dir.join("LICENSE.txt"), "Apache").unwrap();

    let result = scan_attachments(&temp_dir).unwrap();

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    // Should find 2 files (reference.md, forms.json), skip SKILL.md and LICENSE.txt
    assert_eq!(result.len(), 2);

    let filenames: Vec<&str> = result.iter().map(|a| a.filename.as_str()).collect();
    assert!(filenames.contains(&"reference.md"));
    assert!(filenames.contains(&"forms.json"));

    // Both should be classified as references
    assert!(result.iter().all(|a| a.type_ == "reference"));
}

#[test]
fn test_scan_skill_with_subdirectories() {
    // Create temp skill directory with subdirectories (like pdf skill)
    let temp_dir = std::env::temp_dir().join(format!("test-skill-subdir-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create SKILL.md
    std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();

    // Create scripts directory
    std::fs::create_dir_all(temp_dir.join("scripts")).unwrap();
    std::fs::write(temp_dir.join("scripts/__init__.py"), "# Init").unwrap();
    std::fs::write(temp_dir.join("scripts/helper.py"), "# Helper").unwrap();

    // Create custom directory (like ooxml/)
    std::fs::create_dir_all(temp_dir.join("ooxml")).unwrap();
    std::fs::write(temp_dir.join("ooxml/document.xml"), "<xml/>").unwrap();

    let result = scan_attachments(&temp_dir).unwrap();

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    // Should find 3 files with preserved paths
    assert_eq!(result.len(), 3);

    let filenames: Vec<&str> = result.iter().map(|a| a.filename.as_str()).collect();
    assert!(filenames.contains(&"scripts/__init__.py"));
    assert!(filenames.contains(&"scripts/helper.py"));
    assert!(filenames.contains(&"ooxml/document.xml"));

    // Check types
    let scripts: Vec<_> = result.iter().filter(|a| a.type_ == "script").collect();
    assert_eq!(scripts.len(), 2);

    let references: Vec<_> = result.iter().filter(|a| a.type_ == "reference").collect();
    assert_eq!(references.len(), 1);
}

#[test]
fn test_scan_skill_no_attachments() {
    // Create temp skill directory with only SKILL.md (like brand-guidelines)
    let temp_dir = std::env::temp_dir().join(format!("test-skill-empty-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();
    std::fs::write(temp_dir.join("LICENSE.txt"), "Apache").unwrap();

    let result = scan_attachments(&temp_dir).unwrap();

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();

    // Should find nothing (SKILL.md and LICENSE skipped)
    assert_eq!(result.len(), 0);
}

#[test]
fn test_classify_file() {
    assert_eq!(classify_file("test.py"), "script");
    assert_eq!(classify_file("test.sh"), "script");
    assert_eq!(classify_file("test.js"), "script");

    assert_eq!(classify_file("test.md"), "reference");
    assert_eq!(classify_file("test.json"), "reference");
    assert_eq!(classify_file("test.yaml"), "reference");

    assert_eq!(classify_file("test.png"), "asset");
    assert_eq!(classify_file("test.jpg"), "asset");
    assert_eq!(classify_file("test.svg"), "asset");
}

#[test]
fn test_should_skip() {
    assert!(should_skip("SKILL.md"));
    assert!(should_skip("LICENSE"));
    assert!(should_skip("LICENSE.txt"));
    assert!(should_skip("README.md"));
    assert!(should_skip(".git"));
    assert!(should_skip(".gitignore"));
    assert!(should_skip(".DS_Store"));

    assert!(!should_skip("reference.md"));
    assert!(!should_skip("test.py"));
}
