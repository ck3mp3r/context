// Symbol resolution for relationship extraction
//
// Maps symbol names to their IDs to enable creating relationship edges.
// Handles ambiguity when multiple symbols share the same name.

use crate::analysis::types::{ExtractedSymbol, SymbolKind};
use std::collections::HashMap;

/// Symbol resolution index: name → Vec<SymbolEntry>
///
/// Multiple symbols can have the same name (different files/scopes),
/// so we store all candidates and use context to resolve.
pub struct SymbolIndex {
    /// name → Vec<SymbolEntry>
    by_name: HashMap<String, Vec<SymbolEntry>>,
}

#[derive(Debug, Clone)]
struct SymbolEntry {
    symbol_id: String,
    file_path: String,
    kind: SymbolKind,
}

impl SymbolIndex {
    /// Build index from extracted symbols with their generated IDs
    pub fn build(symbols: &[(ExtractedSymbol, String)]) -> Self {
        let mut by_name: HashMap<String, Vec<SymbolEntry>> = HashMap::new();

        for (symbol, symbol_id) in symbols {
            let entry = SymbolEntry {
                symbol_id: symbol_id.clone(),
                file_path: symbol.file_path.clone(),
                kind: symbol.kind.clone(),
            };

            by_name.entry(symbol.name.clone()).or_default().push(entry);
        }

        Self { by_name }
    }

    /// Resolve a symbol name to possible candidates
    ///
    /// Returns all symbols with matching name, with confidence scores.
    /// Prefers symbols in the same file as the context.
    pub fn resolve(&self, name: &str, context_file: Option<&str>) -> Vec<ResolutionCandidate> {
        let Some(entries) = self.by_name.get(name) else {
            return vec![];
        };

        let mut candidates: Vec<_> = entries
            .iter()
            .map(|entry| {
                let confidence = if let Some(ctx_file) = context_file {
                    if entry.file_path == ctx_file {
                        1.0 // Same file = high confidence
                    } else {
                        0.7 // Different file = lower confidence
                    }
                } else {
                    0.5 // No context = ambiguous
                };

                ResolutionCandidate {
                    symbol_id: entry.symbol_id.clone(),
                    file_path: entry.file_path.clone(),
                    kind: entry.kind.clone(),
                    confidence,
                }
            })
            .collect();

        // Sort by confidence (highest first)
        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }

    /// Get the best resolution for a symbol (highest confidence)
    pub fn resolve_best(&self, name: &str, context_file: Option<&str>) -> Option<String> {
        self.resolve(name, context_file)
            .first()
            .map(|c| c.symbol_id.clone())
    }
}

/// A candidate symbol resolution with confidence score
#[derive(Debug, Clone)]
pub struct ResolutionCandidate {
    pub symbol_id: String,
    pub file_path: String,
    pub kind: SymbolKind,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbol(name: &str, file_path: &str, kind: SymbolKind) -> (ExtractedSymbol, String) {
        let symbol = ExtractedSymbol {
            name: name.to_string(),
            kind,
            file_path: file_path.to_string(),
            start_line: 1,
            end_line: 10,
            content: String::new(),
            signature: None,
        };
        let symbol_id = format!("symbol:{}:{}:1", file_path, name);
        (symbol, symbol_id)
    }

    #[test]
    fn test_resolve_same_file_preferred() {
        let symbols = vec![
            make_symbol("foo", "src/a.rs", SymbolKind::Function),
            make_symbol("foo", "src/b.rs", SymbolKind::Function),
        ];

        let index = SymbolIndex::build(&symbols);
        let candidates = index.resolve("foo", Some("src/a.rs"));

        assert_eq!(candidates.len(), 2);
        // Same file should have highest confidence
        assert_eq!(candidates[0].confidence, 1.0);
        assert!(candidates[0].symbol_id.contains("src/a.rs"));
    }

    #[test]
    fn test_resolve_ambiguous_no_context() {
        let symbols = vec![
            make_symbol("parse", "src/parser.rs", SymbolKind::Function),
            make_symbol("parse", "src/lexer.rs", SymbolKind::Function),
            make_symbol("parse", "src/compiler.rs", SymbolKind::Function),
        ];

        let index = SymbolIndex::build(&symbols);
        let candidates = index.resolve("parse", None);

        assert_eq!(candidates.len(), 3);
        // All should have equal confidence without context
        for candidate in &candidates {
            assert_eq!(candidate.confidence, 0.5);
        }
    }

    #[test]
    fn test_resolve_best() {
        let symbols = vec![
            make_symbol("foo", "src/a.rs", SymbolKind::Function),
            make_symbol("foo", "src/b.rs", SymbolKind::Function),
        ];

        let index = SymbolIndex::build(&symbols);
        let best = index.resolve_best("foo", Some("src/a.rs"));

        assert!(best.is_some());
        assert!(best.unwrap().contains("src/a.rs"));
    }

    #[test]
    fn test_resolve_not_found() {
        let symbols = vec![make_symbol("foo", "src/a.rs", SymbolKind::Function)];

        let index = SymbolIndex::build(&symbols);
        let candidates = index.resolve("bar", Some("src/a.rs"));

        assert_eq!(candidates.len(), 0);
    }
}
