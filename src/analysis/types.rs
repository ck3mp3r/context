// Type definitions for code analysis
//
// These types represent symbols and relationships extracted from source code.

/// Symbol from source code
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: Kind,
    pub language: String, // "rust", "typescript", etc.
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String, // Code snippet with context (±2 lines) - empty during insertion
    pub signature: Option<String>, // For functions/methods: "fn foo(a: i32) -> String"
}

impl Symbol {
    /// Create a new symbol (content empty, filled during query)
    pub fn new(
        name: String,
        kind: Kind,
        language: String,
        file_path: String,
        start_line: usize,
        end_line: usize,
        signature: Option<String>,
    ) -> Self {
        Self {
            name,
            kind,
            language,
            file_path,
            start_line,
            end_line,
            content: String::new(),
            signature,
        }
    }
}

/// Symbol types - language-agnostic categories
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Kind {
    Function,
    Class,
    Interface,
    Struct,
    Trait,
    Enum,
    Constant,
    Variable,
    Static,
    Module,
    TypeAlias,
    Impl { target_type: String },
}

impl std::str::FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "class" => Ok(Self::Class),
            "interface" => Ok(Self::Interface),
            "struct" => Ok(Self::Struct),
            "trait" => Ok(Self::Trait),
            "enum" => Ok(Self::Enum),
            "constant" | "const" => Ok(Self::Constant),
            "variable" => Ok(Self::Variable),
            "static" => Ok(Self::Static),
            "module" | "mod" => Ok(Self::Module),
            "type" | "type_alias" => Ok(Self::TypeAlias),
            "impl" => Ok(Self::Impl {
                target_type: String::new(),
            }),
            _ => Err(format!("Unknown symbol kind: {}", s)),
        }
    }
}

impl Kind {
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
            Self::Static => "static",
            Self::Module => "module",
            Self::TypeAlias => "type_alias",
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
