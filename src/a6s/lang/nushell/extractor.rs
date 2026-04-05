use crate::a6s::extract::LanguageExtractor;
use crate::a6s::registry::SymbolRegistry;
use crate::a6s::types::{ParsedFile, RawImport, ResolvedImport};

/// Nushell language extractor (stub implementation).
pub struct NushellExtractor;

impl LanguageExtractor for NushellExtractor {
    fn language(&self) -> &'static str {
        "nushell"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["nu"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_nu::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/nushell/queries/symbols.scm")
    }

    fn type_ref_queries(&self) -> &'static str {
        "" // Nushell has no type_refs query
    }

    fn extract(&self, _code: &str, file_path: &str) -> ParsedFile {
        ParsedFile {
            file_path: file_path.to_string(),
            language: "nushell".to_string(),
            symbols: Vec::new(),
            edges: Vec::new(),
            imports: Vec::new(),
        }
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        file_path.to_string()
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        import_path.to_string()
    }

    fn resolve_imports(
        &self,
        _imports: &[RawImport],
        _registry: &SymbolRegistry,
    ) -> Vec<ResolvedImport> {
        Vec::new()
    }
}
