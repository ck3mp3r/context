//! Go language analyser.
//!
//! This module provides code analysis for Go source files, extracting symbols,
//! calls, imports, and type references.

use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::types::{ParsedFile, QualifiedName, RawSymbol, SymbolId};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use super::helpers::{go_entry_type, go_visibility};
use super::symbols;
use super::type_refs;

pub struct Go;

const QUERIES: &str = include_str!("queries/symbols.scm");

/// Known Go project directory prefixes that indicate local code.
const GO_LOCAL_DIRS: &[&str] = &["pkg", "internal", "cmd", "api", "app", "lib", "src"];

impl Go {
    pub fn name() -> &'static str {
        "go"
    }

    pub fn extensions() -> &'static [&'static str] {
        &["go"]
    }

    pub fn grammar() -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    pub fn queries() -> &'static str {
        QUERIES
    }

    pub fn extract(code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "go");
        let language = Self::grammar();

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).expect("grammar error");
        let tree = match parser.parse(code, None) {
            Some(t) => t,
            None => return parsed,
        };

        let query = match Query::new(&language, QUERIES) {
            Ok(q) => q,
            Err(_) => return parsed,
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            symbols::process_match(&query, m, code, file_path, &mut parsed);
        }

        // Check if this is a test file
        let is_test_file = file_path.ends_with("_test.go");

        for sym in &mut parsed.symbols {
            sym.visibility = go_visibility(&sym.name);
            if sym.kind == "function" {
                sym.entry_type = go_entry_type(&sym.name);
            }
            // All symbols in _test.go files get entry_type = "test" if not already set
            // (preserves more specific types like "benchmark", "example", "fuzz")
            if is_test_file && sym.entry_type.is_none() && sym.kind != "package" {
                sym.entry_type = Some("test".to_string());
            }
        }

        // Second pass: extract type references
        type_refs::extract_type_refs(&tree, code, file_path, &mut parsed);

        parsed
    }
}

// ============================================================================
// LanguageAnalyser trait implementation
// ============================================================================

impl LanguageAnalyser for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn queries(&self) -> &'static str {
        QUERIES
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        // Delegate to static method for backwards compatibility
        Go::extract(code, file_path)
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        use std::path::Path;

        let path = Path::new(file_path);
        // Go module path is the directory containing the file
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

        // Convert path separators to ::
        parent.replace(['/', '\\'], "::")
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        // Convert Go import path to internal module format.
        // "github.com/acme/myapp/pkg/common" → "pkg::common"
        let parts: Vec<&str> = import_path.split('/').collect();

        for (i, part) in parts.iter().enumerate() {
            if GO_LOCAL_DIRS.contains(part) {
                return parts[i..].join("::");
            }
        }

        // No known local dir - just convert slashes to ::
        import_path.replace('/', "::")
    }

    fn find_import_source(
        &self,
        symbols: &[RawSymbol],
        _file_path: &str,
        _module_path: &str,
        _registry: &std::collections::HashMap<QualifiedName, SymbolId>,
    ) -> Option<SymbolId> {
        // For Go, the import source is the package symbol
        symbols
            .iter()
            .find(|s| s.kind == "package")
            .map(|s| s.symbol_id())
    }

    fn resolve_import_targets(
        &self,
        import_path: &str,
        _imported_names: &[String],
        registry: &std::collections::HashMap<QualifiedName, SymbolId>,
        symbol_languages: &std::collections::HashMap<SymbolId, String>,
        symbol_kinds: &std::collections::HashMap<SymbolId, String>,
    ) -> Vec<SymbolId> {
        // For Go: find target package by matching import path suffix
        // Import path like "github.com/foo/bar/pkg/analyzer" should match
        // a package symbol in a directory like "pkg/analyzer"
        let import_suffix = import_path.replace('/', "::");
        let pkg_name = import_path.rsplit('/').next().unwrap_or(import_path);

        for (qn, target_id) in registry {
            // Skip if not Go
            if symbol_languages.get(target_id).is_some_and(|l| l != "go") {
                continue;
            }
            // Skip if not a package
            if symbol_kinds.get(target_id).is_some_and(|k| k != "package") {
                continue;
            }

            let qn_str = qn.as_str();
            // Check if the qualified name ends with the import suffix
            if qn_str.ends_with(&import_suffix)
                || qn_str.ends_with(&format!("::{}", pkg_name))
                || qn_str == pkg_name
            {
                return vec![target_id.clone()];
            }
        }

        Vec::new()
    }
}
