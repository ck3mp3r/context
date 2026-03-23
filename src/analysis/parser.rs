// Tree-sitter parser wrapper
//
// Provides abstraction over tree-sitter for parsing source code.

use crate::analysis::extractor::SymbolExtractor;
use crate::analysis::languages::rust::RustExtractor;
use tree_sitter::{Language, Parser as TsParser, Tree};

/// Wrapper around tree-sitter Parser
pub struct Parser {
    parser: TsParser,
    #[allow(dead_code)]
    language: Language,
    language_name: String,
}

impl Parser {
    /// Create a new parser for Rust
    pub fn new_rust() -> Result<Self, tree_sitter::LanguageError> {
        let mut parser = TsParser::new();
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&language)?;

        Ok(Self {
            parser,
            language,
            language_name: "rust".to_string(),
        })
    }

    /// Parse source code and return the AST
    pub fn parse(&mut self, content: &str) -> Option<Tree> {
        self.parser.parse(content, None)
    }

    /// Get the language name
    pub fn language_name(&self) -> &str {
        &self.language_name
    }
}

/// Registry of supported languages
pub struct LanguageRegistry {
    // Rust only for MVP
    #[allow(dead_code)]
    rust: Language,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            rust: tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// Detect language from file content using Tree-sitter parsers
    ///
    /// Tries to parse with each supported language and returns the one that succeeds
    pub fn detect_language(&self, content: &str, file_path: &str) -> Option<DetectedLanguage> {
        // Try extension hint first for performance
        if let Some(ext) = std::path::Path::new(file_path).extension()
            && let Some(ext_str) = ext.to_str()
            && let Some(lang) = self.detect_from_extension(ext_str)
        {
            // Verify with parser
            if self.can_parse(content, &lang) {
                return Some(lang);
            }
        }

        // Fall back to trying all parsers
        self.detect_from_content(content)
    }

    /// Detect language from extension (fast path)
    fn detect_from_extension(&self, ext: &str) -> Option<DetectedLanguage> {
        match ext {
            "rs" => Some(DetectedLanguage::Rust),
            _ => None,
        }
    }

    /// Detect language by trying to parse with each parser
    fn detect_from_content(&self, content: &str) -> Option<DetectedLanguage> {
        // Try Rust parser
        if self.can_parse(content, &DetectedLanguage::Rust) {
            return Some(DetectedLanguage::Rust);
        }

        // Add more languages here as we support them

        None
    }

    /// Check if content can be successfully parsed with a language
    fn can_parse(&self, content: &str, language: &DetectedLanguage) -> bool {
        match language {
            DetectedLanguage::Rust => {
                if let Ok(mut parser) = Parser::new_rust()
                    && let Some(tree) = parser.parse(content)
                {
                    // Check if parse was successful (not just error recovery)
                    let root = tree.root_node();
                    return !root.has_error();
                }
                false
            }
        }
    }

    /// Check if a file extension is supported
    pub fn supports_extension(&self, ext: &str) -> bool {
        matches!(ext, "rs")
    }

    /// Get a parser for a file path
    pub fn get_parser_for_file(&self, path: &str) -> Option<Parser> {
        let ext = std::path::Path::new(path).extension()?.to_str()?;

        if self.supports_extension(ext) {
            Parser::new_rust().ok()
        } else {
            None
        }
    }

    /// Get an extractor for detected language
    pub fn get_extractor(&self, language: &DetectedLanguage) -> ExtractorInstance {
        match language {
            DetectedLanguage::Rust => ExtractorInstance::Rust(RustExtractor),
        }
    }

    /// Get an extractor for a file by detecting its language
    pub fn get_extractor_for_file(&self, path: &str, content: &str) -> Option<ExtractorInstance> {
        let language = self.detect_language(content, path)?;
        Some(self.get_extractor(&language))
    }
}

/// Detected language enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedLanguage {
    Rust,
    // Add more languages here
}

/// Extractor instance enum - holds concrete extractor types
///
/// NO dyn - pure static dispatch via enum
pub enum ExtractorInstance {
    Rust(RustExtractor),
    // Add more languages here
}

impl ExtractorInstance {
    /// Extract symbols - dispatches to concrete type
    pub fn extract_symbols(
        &self,
        code: &str,
        file_path: &str,
    ) -> Vec<crate::analysis::types::ExtractedSymbol> {
        match self {
            ExtractorInstance::Rust(extractor) => extractor.extract_symbols(code, file_path),
        }
    }

    /// Extract relationships - dispatches to concrete type
    pub fn extract_relationships(
        &self,
        code: &str,
        file_path: &str,
    ) -> Vec<crate::analysis::types::ExtractedRelationship> {
        match self {
            ExtractorInstance::Rust(extractor) => extractor.extract_relationships(code, file_path),
        }
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
