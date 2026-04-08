use crate::a6s::extract::LanguageExtractor;
use crate::a6s::registry::SymbolRegistry;
use crate::a6s::types::{ParsedFile, RawImport, ResolvedImport};

/// Go language extractor (stub implementation).
pub struct GolangExtractor;

impl LanguageExtractor for GolangExtractor {
    fn language(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/golang/queries/symbols.scm")
    }

    fn type_ref_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/golang/queries/type_refs.scm")
    }

    fn extract(&self, _code: &str, file_path: &str) -> ParsedFile {
        ParsedFile {
            file_path: file_path.to_string(),
            language: "go".to_string(),
            symbols: Vec::new(),
            edges: Vec::new(),
            imports: Vec::new(),
            file_category: None,
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
