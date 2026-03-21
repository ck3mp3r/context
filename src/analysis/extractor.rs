// Symbol extraction from parsed AST
//
// Generic trait for extracting symbols from language-specific ASTs.

use crate::analysis::types::{ExtractedRelationship, ExtractedSymbol};

/// Trait for language-specific symbol extraction
///
/// Implementing this trait allows adding support for new programming languages
/// without modifying existing code (Open/Closed Principle).
pub trait SymbolExtractor {
    /// Extract symbols from source code
    ///
    /// # Arguments
    /// * `code` - Source code to parse
    /// * `file_path` - Path to the file being analyzed (for symbol metadata)
    ///
    /// # Returns
    /// Vector of extracted symbols with their metadata
    fn extract_symbols(&self, code: &str, file_path: &str) -> Vec<ExtractedSymbol>;

    /// Extract relationships between symbols (calls, references, etc.)
    ///
    /// # Arguments
    /// * `code` - Source code to parse
    /// * `file_path` - Path to the file being analyzed
    ///
    /// # Returns
    /// Vector of relationships found in the code
    fn extract_relationships(&self, code: &str, file_path: &str) -> Vec<ExtractedRelationship> {
        // Default implementation returns empty (Phase 2 feature)
        let _ = (code, file_path);
        vec![]
    }
}
