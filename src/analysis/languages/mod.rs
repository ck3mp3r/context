// Language-specific extractors
//
// Each language has its own module with:
// - extractor.rs: Symbol and relationship extraction
// - queries/*.scm: Tree-sitter query files (optional)
// - extractor_test.rs: Tests

pub mod rust;

// Re-export extractors for convenience
pub use rust::RustExtractor;
