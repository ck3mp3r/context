//! Skill management logic
//!
//! This module contains skill-specific business logic including:
//! - Cache extraction (extracting attachments from DB to filesystem)
//! - Skill import from various sources (git, archives, local paths)
//! - SKILL.md parsing (YAML frontmatter + Markdown)
//! - Attachment scanning and encoding

mod cache;
#[cfg(test)]
mod cache_test;
mod import;
mod parser;
mod scanner;
mod source;

// Re-export cache functions
pub use cache::{
    clear_all_caches, extract_attachments, get_skill_cache_dir, get_skills_cache_dir,
    invalidate_cache, parse_skill_name_from_content,
};

// Re-export import functions
pub use import::{ImportError, import_skill};

/// Generate deterministic skill ID from skill name.
/// Uses SHA256 hash of name, truncated to 8-char hex (first 4 bytes).
/// Same name = same ID.
pub fn generate_skill_id(name: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let hash = hasher.finalize();
    format!(
        "{:08x}",
        &hash[..4].iter().fold(0u32, |acc, &b| (acc << 8) | b as u32)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_skill_id_length_constraint() {
        // Test various skill names to ensure all IDs are exactly 8 characters
        let test_names = vec![
            "rust-async-patterns",
            "nushell-testing",
            "a",
            "test",
            "very-long-skill-name-with-many-characters",
            "🦀 Rust Skill",
            "",
            "UPPERCASE",
            "lowercase",
            "123-numbers",
        ];

        for name in test_names {
            let id = generate_skill_id(name);
            assert_eq!(
                id.len(),
                8,
                "Skill ID for '{}' should be exactly 8 characters, got '{}' with length {}",
                name,
                id,
                id.len()
            );

            // Also verify it's valid hexadecimal
            assert!(
                id.chars().all(|c| c.is_ascii_hexdigit()),
                "Skill ID for '{}' should contain only hex characters, got '{}'",
                name,
                id
            );
        }
    }

    #[test]
    fn test_generate_skill_id_deterministic() {
        // Same name should always produce the same ID
        let name = "rust-async-patterns";
        let id1 = generate_skill_id(name);
        let id2 = generate_skill_id(name);

        assert_eq!(id1, id2, "Same skill name should generate the same ID");
        assert_eq!(id1.len(), 8, "ID should be 8 characters");
    }

    #[test]
    fn test_generate_skill_id_uniqueness() {
        // Different names should produce different IDs
        let name1 = "skill-one";
        let name2 = "skill-two";

        let id1 = generate_skill_id(name1);
        let id2 = generate_skill_id(name2);

        assert_ne!(
            id1, id2,
            "Different skill names should generate different IDs"
        );
        assert_eq!(id1.len(), 8, "ID1 should be 8 characters");
        assert_eq!(id2.len(), 8, "ID2 should be 8 characters");
    }
}
