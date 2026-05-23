// Type definitions for code analysis
//
// Newtype wrappers enforce type safety and the raw extraction types
// form the intermediate representation between tree-sitter parsing
// and graph loading.

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

    pub fn file_path(&self) -> Option<&str> {
        let s = self.0.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&s[..second_last_colon])
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

/// A module-qualified symbol name for unambiguous cross-file resolution.
///
/// Format: `"module_path::symbol_name"` (e.g., `"analysis::types::SymbolId"`).
/// Separator is always `::` regardless of language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName(String);

impl QualifiedName {
    pub fn new(module_path: &str, symbol_name: &str) -> Self {
        if module_path.is_empty() {
            Self(symbol_name.to_string())
        } else {
            Self(format!("{}::{}", module_path, symbol_name))
        }
    }

    pub fn from_qualified(qualified: impl Into<String>) -> Self {
        Self(qualified.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn bare_name(&self) -> &str {
        self.0.rsplit("::").next().unwrap_or(&self.0)
    }

    pub fn module_path(&self) -> &str {
        match self.0.rsplit_once("::") {
            Some((path, _)) => path,
            None => "",
        }
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// Relationship enums
// ============================================================================

/// Semantic edge kinds for the code graph.
/// These represent typed relationships between symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeKind {
    // Membership (parent contains child)
    HasField,  // struct → field
    HasMethod, // impl/trait → method
    HasMember, // module → symbol

    // Heritage
    Implements, // type → trait
    Extends,    // type → parent type

    // References
    Calls,       // function → function
    FileImports, // file → imported symbol (file-level imports)
    Import,      // symbol → imported symbol (scoped imports)
    TypeRef,     // symbol → type it references
    FieldType,   // field → its type
    ParamType,   // function → parameter type
    ReturnType,  // function → return type
    Usage,       // symbol → identifier it references (const, var, etc.)
}

impl EdgeKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::HasField => "HasField",
            Self::HasMethod => "HasMethod",
            Self::HasMember => "HasMember",
            Self::Implements => "Implements",
            Self::Extends => "Extends",
            Self::Calls => "Calls",
            Self::FileImports => "FileImports",
            Self::Import => "Import",
            Self::TypeRef => "TypeRef",
            Self::FieldType => "FieldType",
            Self::ParamType => "ParamType",
            Self::ReturnType => "ReturnType",
            Self::Usage => "Usage",
        }
    }

    /// Get the edge name used in the graph store.
    /// Some EdgeKinds map to different names in the graph schema.
    pub fn graph_edge_name(&self) -> &str {
        match self {
            Self::HasField => "SymbolContains",
            Self::HasMethod => "SymbolContains",
            Self::HasMember => "SymbolContains",
            Self::Implements => "Inherits",
            Self::Extends => "Inherits",
            Self::Calls => "Calls",
            Self::FileImports => "FileImports",
            Self::Import => "Import",
            Self::TypeRef => "TypeAnnotation",
            Self::FieldType => "FieldType",
            Self::ParamType => "Accepts",
            Self::ReturnType => "Returns",
            Self::Usage => "Uses",
        }
    }
}

/// A raw edge between two symbols, emitted directly by analysers.
/// No resolution needed — both ends are SymbolIds.
#[derive(Debug, Clone)]
pub struct RawEdge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InheritanceType {
    Extends,
    Implements,
    TraitBound,
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

// ============================================================================
// Symbol (used by store for graph queries)
// ============================================================================

#[derive(Debug, Clone)]
pub struct Symbol<K: AsRef<str> + std::fmt::Debug> {
    pub name: String,
    pub kind: K,
    pub language: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub entry_type: Option<String>,
}

// ============================================================================
// Import entry (shared between old and new pipeline)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub module_path: String,
    pub imported_names: Vec<String>,
    pub alias: Option<String>,
    pub is_glob: bool,
}

impl ImportEntry {
    pub fn module_import(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            imported_names: Vec::new(),
            alias: None,
            is_glob: false,
        }
    }

    pub fn named_import(module_path: impl Into<String>, names: Vec<String>) -> Self {
        Self {
            module_path: module_path.into(),
            imported_names: names,
            alias: None,
            is_glob: false,
        }
    }

    pub fn glob_import(module_path: impl Into<String>) -> Self {
        Self {
            module_path: module_path.into(),
            imported_names: Vec::new(),
            alias: None,
            is_glob: true,
        }
    }
}

// ============================================================================
// Raw extraction types (query-based pipeline)
// ============================================================================

#[derive(Debug, Clone)]
pub struct RawSymbol {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
    pub language: String,
    pub visibility: Option<String>,
    pub entry_type: Option<String>,
}

impl RawSymbol {
    pub fn symbol_id(&self) -> SymbolId {
        SymbolId::new(&self.file_path, &self.name, self.start_line)
    }
}

#[derive(Debug, Clone)]
pub struct RawImport {
    pub file_path: String,
    pub entry: ImportEntry,
}

#[derive(Debug)]
pub struct ParsedFile {
    pub file_path: String,
    pub language: String,
    pub symbols: Vec<RawSymbol>,
    pub edges: Vec<RawEdge>,
    pub imports: Vec<RawImport>,
}

impl ParsedFile {
    pub fn new(file_path: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            language: language.into(),
            symbols: Vec::new(),
            edges: Vec::new(),
            imports: Vec::new(),
        }
    }
}
