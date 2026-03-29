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

    /// Extract the file path from the SymbolId.
    /// Format: "symbol:{file_path}:{name}:{start_line}"
    pub fn file_path(&self) -> Option<&str> {
        let s = self.0.strip_prefix("symbol:")?;
        // Find the last two colons to separate file_path from name:line
        // We need to find name:line at the end, which is {name}:{line}
        // Since name can't contain ':', we find from the right
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

/// A module-qualified symbol name for unambiguous cross-file resolution.
///
/// Format: `"module_path::symbol_name"` (e.g., `"analysis::types::SymbolId"`).
/// For root-level symbols (no module), the bare name is used (e.g., `"main"`).
///
/// The separator is always `::` regardless of language, providing a
/// uniform namespace across Rust, Go, and Nushell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName(String);

impl QualifiedName {
    /// Create a qualified name from module path and symbol name.
    /// If module_path is empty, returns the bare symbol name.
    pub fn new(module_path: &str, symbol_name: &str) -> Self {
        if module_path.is_empty() {
            Self(symbol_name.to_string())
        } else {
            Self(format!("{}::{}", module_path, symbol_name))
        }
    }

    /// Create a qualified name from an already-qualified string.
    pub fn from_qualified(qualified: impl Into<String>) -> Self {
        Self(qualified.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the bare symbol name (last segment after `::`)
    pub fn bare_name(&self) -> &str {
        self.0.rsplit("::").next().unwrap_or(&self.0)
    }

    /// Get the module path (everything before the last `::`)
    /// Returns empty string for unqualified names.
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

/// Derives the module path from a source file path.
///
/// Language-specific rules:
/// - **Rust**: Strips `src/` prefix, removes file extension and `mod.rs`/`lib.rs`/`main.rs`
///   suffixes. E.g., `src/analysis/types.rs` → `analysis::types`,
///   `src/analysis/mod.rs` → `analysis`, `src/lib.rs` → `""` (root).
/// - **Go**: Uses the package name directly (not file path), since Go's
///   package identity is declared, not inferred from paths. Returns empty
///   for now — Go callers should use the package name from `package_clause`.
/// - **Nushell**: Strips `.nu` extension, converts path separators to `::`.
pub fn derive_module_path(file_path: &str, language: &str) -> String {
    match language {
        "rust" => derive_rust_module_path(file_path),
        "nushell" => derive_nushell_module_path(file_path),
        // Go uses package names, not file paths — caller provides module path directly
        _ => String::new(),
    }
}

/// Derive module path from a Rust file path.
///
/// Rules:
/// - Strip `src/` prefix if present
/// - `lib.rs`, `main.rs` at root → empty (crate root)
/// - `mod.rs` → parent directory path
/// - Other `.rs` files → directory path + stem
/// - Path separators become `::`
fn derive_rust_module_path(file_path: &str) -> String {
    use std::path::Path;

    let path = Path::new(file_path);

    // Strip src/ prefix
    let path = path
        .strip_prefix("src/")
        .or_else(|_| path.strip_prefix("src"))
        .unwrap_or(path);

    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    let module_part = match file_name {
        "lib.rs" | "main.rs" => {
            // Crate root — module path is just the parent directory
            parent.to_string()
        }
        "mod.rs" => {
            // Module root — module path is the parent directory
            parent.to_string()
        }
        _ => {
            // Regular file — module path is parent + stem
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if parent.is_empty() {
                stem.to_string()
            } else {
                format!("{}/{}", parent, stem)
            }
        }
    };

    // Convert path separators to ::
    module_part.replace('/', "::")
}

/// Derive module path from a Nushell file path.
///
/// Rules:
/// - Strip `.nu` extension
/// - `mod.nu` → parent directory path
/// - Convert path separators to `::`
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- SymbolId::file_path() ----

    #[test]
    fn test_symbol_id_file_path() {
        let id = SymbolId::new("src/analysis/types.rs", "SymbolId", 14);
        assert_eq!(id.file_path(), Some("src/analysis/types.rs"));
    }

    #[test]
    fn test_symbol_id_file_path_deep() {
        let id = SymbolId::new("src/analysis/lang/rust/parser.rs", "extract_callee", 32);
        assert_eq!(id.file_path(), Some("src/analysis/lang/rust/parser.rs"));
    }

    // ---- QualifiedName construction ----

    #[test]
    fn test_qualified_name_with_module_path() {
        let qn = QualifiedName::new("analysis::types", "SymbolId");
        assert_eq!(qn.as_str(), "analysis::types::SymbolId");
    }

    #[test]
    fn test_qualified_name_bare_name() {
        let qn = QualifiedName::new("", "main");
        assert_eq!(qn.as_str(), "main");
    }

    #[test]
    fn test_qualified_name_from_qualified() {
        let qn = QualifiedName::from_qualified("analysis::types::SymbolId");
        assert_eq!(qn.as_str(), "analysis::types::SymbolId");
    }

    #[test]
    fn test_qualified_name_bare_name_extraction() {
        let qn = QualifiedName::new("analysis::types", "SymbolId");
        assert_eq!(qn.bare_name(), "SymbolId");
    }

    #[test]
    fn test_qualified_name_bare_name_no_module() {
        let qn = QualifiedName::new("", "main");
        assert_eq!(qn.bare_name(), "main");
    }

    #[test]
    fn test_qualified_name_module_path_extraction() {
        let qn = QualifiedName::new("analysis::types", "SymbolId");
        assert_eq!(qn.module_path(), "analysis::types");
    }

    #[test]
    fn test_qualified_name_module_path_empty() {
        let qn = QualifiedName::new("", "main");
        assert_eq!(qn.module_path(), "");
    }

    // ---- Rust module path derivation ----

    #[test]
    fn test_rust_module_path_lib_rs() {
        assert_eq!(derive_module_path("src/lib.rs", "rust"), "");
    }

    #[test]
    fn test_rust_module_path_main_rs() {
        assert_eq!(derive_module_path("src/main.rs", "rust"), "");
    }

    #[test]
    fn test_rust_module_path_mod_rs() {
        assert_eq!(
            derive_module_path("src/analysis/mod.rs", "rust"),
            "analysis"
        );
    }

    #[test]
    fn test_rust_module_path_regular_file() {
        assert_eq!(
            derive_module_path("src/analysis/types.rs", "rust"),
            "analysis::types"
        );
    }

    #[test]
    fn test_rust_module_path_deep_nested() {
        assert_eq!(
            derive_module_path("src/analysis/lang/rust/parser.rs", "rust"),
            "analysis::lang::rust::parser"
        );
    }

    #[test]
    fn test_rust_module_path_nested_mod_rs() {
        assert_eq!(
            derive_module_path("src/analysis/lang/rust/mod.rs", "rust"),
            "analysis::lang::rust"
        );
    }

    #[test]
    fn test_rust_module_path_no_src_prefix() {
        // Some repos don't have src/ prefix
        assert_eq!(derive_module_path("lib.rs", "rust"), "");
    }

    // ---- Nushell module path derivation ----

    #[test]
    fn test_nushell_module_path_simple() {
        assert_eq!(derive_module_path("utils.nu", "nushell"), "utils");
    }

    #[test]
    fn test_nushell_module_path_nested() {
        assert_eq!(
            derive_module_path("modules/network/client.nu", "nushell"),
            "modules::network::client"
        );
    }

    #[test]
    fn test_nushell_module_path_mod_nu() {
        assert_eq!(
            derive_module_path("modules/network/mod.nu", "nushell"),
            "modules::network"
        );
    }

    // ---- Go module path (uses package name, not file path) ----

    #[test]
    fn test_go_module_path_returns_empty() {
        // Go doesn't derive from file path — uses package_clause
        assert_eq!(derive_module_path("pkg/server/handler.go", "go"), "");
    }

    // ---- QualifiedName equality for HashMap usage ----

    #[test]
    fn test_qualified_names_same_name_different_modules_are_distinct() {
        let qn1 = QualifiedName::new("api", "Config");
        let qn2 = QualifiedName::new("frontend::api", "Config");
        assert_ne!(qn1, qn2);
    }

    #[test]
    fn test_qualified_names_same_are_equal() {
        let qn1 = QualifiedName::new("analysis::types", "SymbolId");
        let qn2 = QualifiedName::new("analysis::types", "SymbolId");
        assert_eq!(qn1, qn2);
    }

    #[test]
    fn test_qualified_name_hashmap_lookup() {
        use std::collections::HashMap;
        let mut map: HashMap<QualifiedName, &str> = HashMap::new();
        map.insert(QualifiedName::new("api", "Config"), "api_config");
        map.insert(
            QualifiedName::new("frontend::api", "Config"),
            "frontend_config",
        );

        assert_eq!(
            map.get(&QualifiedName::new("api", "Config")),
            Some(&"api_config")
        );
        assert_eq!(
            map.get(&QualifiedName::new("frontend::api", "Config")),
            Some(&"frontend_config")
        );
        // Two Configs coexist without collision
        assert_eq!(map.len(), 2);
    }
}
