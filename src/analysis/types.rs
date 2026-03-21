// Type definitions for code analysis
//
// These types represent symbols and relationships extracted from source code.

/// Extracted from Tree-sitter AST, to be inserted into NanoGraph
#[derive(Debug, Clone)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,           // Code snippet with context (±2 lines)
    pub signature: Option<String>, // For functions/methods: "fn foo(a: i32) -> String"
}

/// Symbol kinds - language-agnostic categories
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    Struct,
    Trait,
    Enum,
    Constant,
    Variable,
    Impl { target_type: String },
}

impl SymbolKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Impl { .. } => "impl",
        }
    }
}

/// Extracted relationship between symbols
#[derive(Debug, Clone)]
pub struct ExtractedRelationship {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
    pub relation_type: RelationType,
    pub confidence: f64,
}

/// Relationship types between symbols
#[derive(Debug, Clone)]
pub enum RelationType {
    Contains,
    Calls { call_site_line: usize },
    References { reference_type: ReferenceType },
    Inherits { inheritance_type: InheritanceType },
}

/// Types of references
#[derive(Debug, Clone)]
pub enum ReferenceType {
    Import,
    TypeAnnotation,
    Usage,
}

/// Types of inheritance
#[derive(Debug, Clone)]
pub enum InheritanceType {
    Extends,
    Implements,
    TraitBound,
}
