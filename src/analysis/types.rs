// Type definitions for code analysis
//
// These types represent symbols and relationships extracted from source code.
// Newtype wrappers enforce type safety: you can't mix up a SymbolId with a
// SymbolName or FileId at the type level.

// ============================================================================
// Newtype wrappers for type-safe identifiers
// ============================================================================

/// Opaque identifier for a symbol node in the graph.
/// Format: "symbol:{file_path}:{name}:{start_line}"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolId(String);

impl SymbolId {
    pub fn new(file_path: &str, name: &str, start_line: usize) -> Self {
        Self(format!("symbol:{}:{}:{}", file_path, name, start_line))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque identifier for a file node in the graph.
/// Format: "file:{path}"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(String);

impl FileId {
    pub fn new(path: &str) -> Self {
        Self(format!("file:{}", path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A symbol name used for cross-file resolution lookups.
/// Distinct from SymbolId to prevent mixing names and IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolName(String);

impl SymbolName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SymbolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// Symbol
// ============================================================================

/// Symbol from source code, generic over the kind type.
///
/// Each language defines its own `Kind` enum (e.g. `rust::Kind`, `nushell::Kind`).
/// The `K: AsRef<str>` bound ensures the kind can always be serialized to its
/// language-specific string (e.g. `"command"`, `"struct"`, `"alias"`).
#[derive(Debug, Clone)]
pub struct Symbol<K: AsRef<str> + std::fmt::Debug> {
    pub name: String,
    pub kind: K,
    pub language: String, // "rust", "nushell", etc.
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String, // Code snippet with context - empty during insertion
    pub signature: Option<String>, // For functions/methods: "fn foo(a: i32) -> String"
}

impl<K: AsRef<str> + std::fmt::Debug> Symbol<K> {
    /// Create a new symbol (content empty, filled during query)
    pub fn new(
        name: String,
        kind: K,
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

// ============================================================================
// Relationship enums (used in store methods and DeferredEdge)
// ============================================================================

/// Types of references between symbols — each maps to a distinct edge type in the graph
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceType {
    Import,
    TypeAnnotation,
    FieldType,
    Usage,
    ReturnType,
    ParamType,
}

impl ReferenceType {
    /// PascalCase edge name for the graph schema
    pub fn edge_name(&self) -> &str {
        match self {
            Self::Import => "Import",
            Self::TypeAnnotation => "TypeAnnotation",
            Self::FieldType => "FieldType",
            Self::Usage => "Uses",
            Self::ReturnType => "Returns",
            Self::ParamType => "Accepts",
        }
    }
}

impl std::str::FromStr for ReferenceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Import" => Ok(Self::Import),
            "TypeAnnotation" => Ok(Self::TypeAnnotation),
            "FieldType" => Ok(Self::FieldType),
            "Uses" => Ok(Self::Usage),
            "Returns" => Ok(Self::ReturnType),
            "Accepts" => Ok(Self::ParamType),
            _ => Err(format!("Unknown reference type: {}", s)),
        }
    }
}

/// Types of inheritance/implementation relationships
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritanceType {
    Extends,    // TypeScript, Java (class extends)
    Implements, // Rust (impl Trait for Type), Java/TypeScript (implements)
    TraitBound, // Rust (where T: Trait)
}

impl InheritanceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::TraitBound => "trait_bound",
        }
    }
}

impl std::str::FromStr for InheritanceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "extends" => Ok(Self::Extends),
            "implements" => Ok(Self::Implements),
            "trait_bound" => Ok(Self::TraitBound),
            _ => Err(format!("Unknown inheritance type: {}", s)),
        }
    }
}
