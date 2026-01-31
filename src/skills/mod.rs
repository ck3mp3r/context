//! Skill management logic
//!
//! This module contains skill-specific business logic including:
//! - Cache extraction (extracting attachments from DB to filesystem)
//! - Skill import from various sources (git, archives, local paths)
//! - SKILL.md parsing (YAML frontmatter + Markdown)
//! - Attachment scanning and encoding

mod cache;
mod import;
mod parser;
mod scanner;
mod source;

// Re-export cache functions
pub use cache::{
    clear_all_caches, extract_attachments, get_skill_cache_dir, get_skills_cache_dir,
    invalidate_cache,
};

// Re-export import functions
pub use import::{ImportError, import_skill};
