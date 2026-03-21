// Tree-sitter parser wrapper
//
// Provides abstraction over tree-sitter for parsing source code.

use tree_sitter::{Language, Parser as TsParser, Tree};

/// Wrapper around tree-sitter Parser
pub struct Parser {
    parser: TsParser,
    language: Language,
    language_name: String,
}

impl Parser {
    /// Create a new parser for Rust
    pub fn new_rust() -> Result<Self, tree_sitter::LanguageError> {
        let mut parser = TsParser::new();
        let language = unsafe { tree_sitter_rust::LANGUAGE.into() };
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
    rust: Language,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            rust: unsafe { tree_sitter_rust::LANGUAGE.into() },
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
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
