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
// Module path derivation
// ============================================================================

pub fn derive_module_path(file_path: &str, language: &str) -> String {
    match language {
        "rust" => derive_rust_module_path(file_path),
        "nushell" => derive_nushell_module_path(file_path),
        _ => String::new(),
    }
}

fn derive_rust_module_path(file_path: &str) -> String {
    use std::path::Path;

    let path = Path::new(file_path);
    let path = path
        .strip_prefix("src/")
        .or_else(|_| path.strip_prefix("src"))
        .unwrap_or(path);

    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    let module_part = match file_name {
        "lib.rs" | "main.rs" => parent.to_string(),
        "mod.rs" => parent.to_string(),
        _ => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if parent.is_empty() {
                stem.to_string()
            } else {
                format!("{}/{}", parent, stem)
            }
        }
    };

    module_part.replace('/', "::")
}

fn derive_nushell_module_path(file_path: &str) -> String {
    use std::path::Path;

    let path = Path::new(file_path);
    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    let module_part = match file_name {
        "mod.nu" => parent.to_string(),
        _ => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if parent.is_empty() {
                stem.to_string()
            } else {
                format!("{}/{}", parent, stem)
            }
        }
    };

    module_part.replace('/', "::")
}

// ============================================================================
// Relationship enums
// ============================================================================

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
}

impl<K: AsRef<str> + std::fmt::Debug> Symbol<K> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallForm {
    Free,
    Method,
    Scoped,
}

#[derive(Debug, Clone)]
pub struct RawSymbol {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
    pub language: String,
}

impl RawSymbol {
    pub fn symbol_id(&self) -> SymbolId {
        SymbolId::new(&self.file_path, &self.name, self.start_line)
    }
}

#[derive(Debug, Clone)]
pub struct RawCall {
    pub file_path: String,
    pub call_site_line: usize,
    pub callee_name: String,
    pub call_form: CallForm,
    pub receiver: Option<String>,
    pub qualifier: Option<String>,
    pub enclosing_symbol_idx: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RawImport {
    pub file_path: String,
    pub entry: ImportEntry,
}

#[derive(Debug, Clone)]
pub struct RawHeritage {
    pub file_path: String,
    pub type_name: String,
    pub parent_name: String,
    pub kind: InheritanceType,
}

#[derive(Debug, Clone)]
pub struct RawContainment {
    pub file_path: String,
    pub parent_name: String,
    pub child_symbol_idx: usize,
}

#[derive(Debug, Clone)]
pub struct RawTypeRef {
    pub file_path: String,
    pub from_symbol_idx: usize,
    pub type_name: String,
    pub ref_kind: ReferenceType,
}

#[derive(Debug)]
pub struct ParsedFile {
    pub file_path: String,
    pub language: String,
    pub symbols: Vec<RawSymbol>,
    pub calls: Vec<RawCall>,
    pub imports: Vec<RawImport>,
    pub heritage: Vec<RawHeritage>,
    pub containments: Vec<RawContainment>,
    pub type_refs: Vec<RawTypeRef>,
}

impl ParsedFile {
    pub fn new(file_path: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            language: language.into(),
            symbols: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
            heritage: Vec::new(),
            containments: Vec::new(),
            type_refs: Vec::new(),
        }
    }
}
