// Symbol extraction from parsed AST
//
// Generic trait for extracting symbols from language-specific ASTs.

use crate::analysis::types::{ExtractedRelationship, ExtractedSymbol};

/// Trait for language-specific symbol extraction
pub trait SymbolExtractor {
    /// Extract symbols from source code
    fn extract(&self, code: &str) -> Vec<ExtractedSymbol>;

    /// Extract relationships between symbols
    fn extract_relationships(&self, code: &str) -> Vec<ExtractedRelationship>;
}
