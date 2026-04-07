//! Language extraction trait and dispatch
//!
//! Uses the same macro pattern as analysis/lang/analyser.rs for static dispatch.

use crate::a6s::registry::SymbolRegistry;
use crate::a6s::types::{ParsedFile, RawImport, ResolvedImport};

#[cfg(test)]
mod extract_test;

/// Language-specific code extractor.
///
/// Implementations must be Send + Sync — they are shared across
/// spawn_blocking tasks. Typically unit structs with no mutable state.
pub trait LanguageExtractor: Send + Sync {
    fn language(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn grammar(&self) -> tree_sitter::Language;
    fn symbol_queries(&self) -> &'static str;
    fn type_ref_queries(&self) -> &'static str;

    /// Extract symbols, edges, and imports from a single file.
    fn extract(&self, code: &str, file_path: &str) -> ParsedFile;

    fn derive_module_path(&self, file_path: &str) -> String;
    fn normalise_import_path(&self, import_path: &str) -> String;
    fn resolve_imports(
        &self,
        imports: &[RawImport],
        registry: &SymbolRegistry,
    ) -> Vec<ResolvedImport>;

    /// Post-extraction multi-file fixups (default: no-op).
    fn resolve_file_modules(&self, _parsed_files: &mut [ParsedFile]) {}
}

/// Single source of truth for all supported languages.
/// Static dispatch via enum - zero runtime overhead.
#[allow(unused_macros)]
macro_rules! define_extractors {
    ($($mod:ident::$type:ident),* $(,)?) => {
        pub enum Extractor {
            $($type(super::lang::$mod::extractor::$type),)*
        }

        impl Extractor {
            pub fn for_language(lang: &str) -> Option<Self> {
                match lang {
                    $(l if l == super::lang::$mod::extractor::$type.language() => {
                        Some(Self::$type(super::lang::$mod::extractor::$type))
                    },)*
                    _ => None,
                }
            }

            pub fn for_extension(ext: &str) -> Option<Self> {
                $(if super::lang::$mod::extractor::$type.extensions().contains(&ext) {
                    return Some(Self::$type(super::lang::$mod::extractor::$type));
                })*
                None
            }
        }

        impl LanguageExtractor for Extractor {
            fn language(&self) -> &'static str {
                match self { $(Self::$type(e) => e.language(),)* }
            }

            fn extensions(&self) -> &'static [&'static str] {
                match self { $(Self::$type(e) => e.extensions(),)* }
            }

            fn grammar(&self) -> tree_sitter::Language {
                match self { $(Self::$type(e) => e.grammar(),)* }
            }

            fn symbol_queries(&self) -> &'static str {
                match self { $(Self::$type(e) => e.symbol_queries(),)* }
            }

            fn type_ref_queries(&self) -> &'static str {
                match self { $(Self::$type(e) => e.type_ref_queries(),)* }
            }

            fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
                match self { $(Self::$type(e) => e.extract(code, file_path),)* }
            }

            fn derive_module_path(&self, file_path: &str) -> String {
                match self { $(Self::$type(e) => e.derive_module_path(file_path),)* }
            }

            fn normalise_import_path(&self, import_path: &str) -> String {
                match self { $(Self::$type(e) => e.normalise_import_path(import_path),)* }
            }

            fn resolve_imports(
                &self,
                imports: &[RawImport],
                registry: &SymbolRegistry,
            ) -> Vec<ResolvedImport> {
                match self { $(Self::$type(e) => e.resolve_imports(imports, registry),)* }
            }

            fn resolve_file_modules(&self, parsed_files: &mut [ParsedFile]) {
                match self { $(Self::$type(e) => e.resolve_file_modules(parsed_files),)* }
            }
        }

        /// Returns all file extensions supported by any extractor.
        pub fn supported_extensions() -> Vec<&'static str> {
            let mut exts = Vec::new();
            $(exts.extend(super::lang::$mod::extractor::$type.extensions());)*
            exts
        }
    };
}

// Use the macro to define the Extractor enum with all languages
define_extractors!(
    rust::RustExtractor,
    golang::GolangExtractor,
    nushell::NushellExtractor,
);
