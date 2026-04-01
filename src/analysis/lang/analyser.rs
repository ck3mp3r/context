//! Language analyser trait for SOLID pipeline architecture.

use crate::analysis::types::{ParsedFile, QualifiedName, RawSymbol, SymbolId};
use std::collections::HashMap;

/// Trait for language-specific code analysis.
pub trait LanguageAnalyser {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn grammar(&self) -> tree_sitter::Language;
    fn queries(&self) -> &'static str;
    fn extract(&self, code: &str, file_path: &str) -> ParsedFile;
    fn derive_module_path(&self, file_path: &str) -> String;

    fn normalise_import_path(&self, import_path: &str) -> String {
        import_path.to_string()
    }

    fn find_import_source(
        &self,
        _symbols: &[RawSymbol],
        _file_path: &str,
        _module_path: &str,
        _registry: &HashMap<QualifiedName, SymbolId>,
    ) -> Option<SymbolId> {
        None
    }

    /// Resolve import targets given an import path.
    /// Returns symbol IDs that this import path should link to.
    fn resolve_import_targets(
        &self,
        import_path: &str,
        imported_names: &[String],
        registry: &HashMap<QualifiedName, SymbolId>,
        _symbol_languages: &HashMap<SymbolId, String>,
        _symbol_kinds: &HashMap<SymbolId, String>,
    ) -> Vec<SymbolId> {
        // Default: direct qualified name lookup for each imported name
        imported_names
            .iter()
            .filter_map(|name| {
                let qn = QualifiedName::new(import_path, name);
                registry.get(&qn).cloned()
            })
            .collect()
    }
}

/// Single source of truth for all supported languages.
macro_rules! define_languages {
    ($($mod:ident::$type:ident),* $(,)?) => {
        pub enum Analyser {
            $($type(super::$mod::$type),)*
        }

        impl Analyser {
            pub fn for_language(lang: &str) -> Option<Self> {
                match lang {
                    $(l if l == super::$mod::$type.name() => Some(Self::$type(super::$mod::$type)),)*
                    _ => None,
                }
            }

            pub fn for_extension(ext: &str) -> Option<Self> {
                $(if super::$mod::$type.extensions().contains(&ext) {
                    return Some(Self::$type(super::$mod::$type));
                })*
                None
            }
        }

        impl LanguageAnalyser for Analyser {
            fn name(&self) -> &'static str {
                match self { $(Self::$type(a) => a.name(),)* }
            }

            fn extensions(&self) -> &'static [&'static str] {
                match self { $(Self::$type(a) => a.extensions(),)* }
            }

            fn grammar(&self) -> tree_sitter::Language {
                match self { $(Self::$type(a) => a.grammar(),)* }
            }

            fn queries(&self) -> &'static str {
                match self { $(Self::$type(a) => a.queries(),)* }
            }

            fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
                match self { $(Self::$type(a) => a.extract(code, file_path),)* }
            }

            fn derive_module_path(&self, file_path: &str) -> String {
                match self { $(Self::$type(a) => a.derive_module_path(file_path),)* }
            }

            fn normalise_import_path(&self, import_path: &str) -> String {
                match self { $(Self::$type(a) => a.normalise_import_path(import_path),)* }
            }

            fn find_import_source(
                &self,
                symbols: &[RawSymbol],
                file_path: &str,
                module_path: &str,
                registry: &HashMap<QualifiedName, SymbolId>,
            ) -> Option<SymbolId> {
                match self { $(Self::$type(a) => a.find_import_source(symbols, file_path, module_path, registry),)* }
            }

            fn resolve_import_targets(
                &self,
                import_path: &str,
                imported_names: &[String],
                registry: &HashMap<QualifiedName, SymbolId>,
                symbol_languages: &HashMap<SymbolId, String>,
                symbol_kinds: &HashMap<SymbolId, String>,
            ) -> Vec<SymbolId> {
                match self { $(Self::$type(a) => a.resolve_import_targets(import_path, imported_names, registry, symbol_languages, symbol_kinds),)* }
            }
        }

        /// Returns all file extensions supported by any language analyser.
        pub fn supported_extensions() -> Vec<&'static str> {
            let mut exts = Vec::new();
            $(exts.extend(super::$mod::$type.extensions());)*
            exts
        }
    };
}

define_languages!(golang::Go, rust::Rust, nushell::Nushell,);
