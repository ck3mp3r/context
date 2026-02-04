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
        "{:x}",
        &hash[..4].iter().fold(0u32, |acc, &b| (acc << 8) | b as u32)
    )
}
