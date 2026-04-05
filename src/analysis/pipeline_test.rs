use crate::analysis::pipeline::SymbolRegistry;
use crate::analysis::types::*;

fn build_registry() -> SymbolRegistry {
    let mut reg = SymbolRegistry::new();

    // Rust symbols
    reg.register(
        QualifiedName::new("server", "init"),
        SymbolId::new("src/server.rs", "init", 10),
        "function",
        "rust",
    );
    reg.register(
        QualifiedName::new("server", "Config"),
        SymbolId::new("src/server.rs", "Config", 1),
        "struct",
        "rust",
    );
    reg.register(
        QualifiedName::new("server", "run"),
        SymbolId::new("src/server.rs", "run", 20),
        "function",
        "rust",
    );

    // Nushell symbols with overlapping names
    reg.register(
        QualifiedName::new("commands", "init"),
        SymbolId::new("src/commands/mod.nu", "init", 5),
        "command",
        "nushell",
    );
    reg.register(
        QualifiedName::new("commands", "run"),
        SymbolId::new("src/commands/mod.nu", "run", 15),
        "command",
        "nushell",
    );

    // Go symbols with overlapping names
    reg.register(
        QualifiedName::new("main", "init"),
        SymbolId::new("main.go", "init", 3),
        "function",
        "go",
    );

    reg
}

#[test]
fn test_bare_name_unique_resolves_without_language() {
    let reg = build_registry();

    // "Config" only exists in Rust — should resolve regardless of caller language
    let result = reg.resolve_with_imports_and_kind("Config", "whatever", "any.rs", "rust", None);
    assert!(result.is_some(), "unique bare name should resolve");
    assert_eq!(result.unwrap().as_str(), "symbol:src/server.rs:Config:1");
}

#[test]
fn test_bare_name_ambiguous_prefers_same_language() {
    let reg = build_registry();

    // "init" exists in Rust, Nushell, and Go
    // A Rust caller should get the Rust "init"
    let result = reg.resolve_with_imports_and_kind("init", "caller", "src/caller.rs", "rust", None);
    assert!(result.is_some(), "should resolve init for Rust caller");
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:src/server.rs:init:10",
        "Rust caller should get Rust init, not Nushell or Go"
    );

    // A Nushell caller should get the Nushell "init"
    let result =
        reg.resolve_with_imports_and_kind("init", "caller", "src/caller.nu", "nushell", None);
    assert!(result.is_some(), "should resolve init for Nushell caller");
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:src/commands/mod.nu:init:5",
        "Nushell caller should get Nushell init, not Rust or Go"
    );

    // A Go caller should get the Go "init"
    let result = reg.resolve_with_imports_and_kind("init", "caller", "cmd/caller.go", "go", None);
    assert!(result.is_some(), "should resolve init for Go caller");
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:main.go:init:3",
        "Go caller should get Go init, not Rust or Nushell"
    );
}

#[test]
fn test_same_module_resolution_unaffected_by_language() {
    let reg = build_registry();

    // Same-module lookup (step 1) should still work — it's exact qualified match
    let result = reg.resolve_with_imports_and_kind("init", "server", "src/server.rs", "rust", None);
    assert!(result.is_some());
    assert_eq!(result.unwrap().as_str(), "symbol:src/server.rs:init:10");
}

#[test]
fn test_import_resolution_unaffected_by_language() {
    let mut reg = build_registry();

    // Add import table: src/app.rs imports "init" from "server"
    let mut table = crate::analysis::pipeline::ImportTable::default();
    table
        .name_to_module
        .insert("init".to_string(), "server".to_string());
    reg.import_tables.insert("src/app.rs".to_string(), table);

    // Should resolve via import table (step 2), not bare name (step 3)
    let result = reg.resolve_with_imports_and_kind("init", "app", "src/app.rs", "rust", None);
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:src/server.rs:init:10",
        "import resolution should still work"
    );
}

#[test]
fn test_no_same_language_match_returns_none() {
    let reg = build_registry();

    // "Config" only exists in Rust — a Nushell caller should NOT resolve it
    // Cross-language bare name fallback is never valid
    let result =
        reg.resolve_with_imports_and_kind("Config", "caller", "src/caller.nu", "nushell", None);
    assert!(
        result.is_none(),
        "cross-language bare name should not resolve, got: {:?}",
        result.map(|id| id.as_str())
    );
}

#[test]
fn test_ambiguous_same_language_returns_none() {
    let mut reg = SymbolRegistry::new();

    // Two Rust functions with the same bare name in different modules
    reg.register(
        QualifiedName::new("server", "process"),
        SymbolId::new("src/server.rs", "process", 10),
        "function",
        "rust",
    );
    reg.register(
        QualifiedName::new("handlers", "process"),
        SymbolId::new("src/handlers.rs", "process", 5),
        "function",
        "rust",
    );

    // Bare name lookup from an unrelated module should return None
    // because there are 2 same-language candidates
    let result = reg.resolve_with_imports_and_kind("process", "app", "src/app.rs", "rust", None);
    assert!(
        result.is_none(),
        "ambiguous same-language bare name should return None, got: {:?}",
        result.map(|id| id.as_str())
    );
}

#[test]
fn test_ambiguous_cross_language_no_same_lang_returns_none() {
    let mut reg = SymbolRegistry::new();

    // "init" exists in Rust and Go but NOT Nushell
    reg.register(
        QualifiedName::new("server", "init"),
        SymbolId::new("src/server.rs", "init", 10),
        "function",
        "rust",
    );
    reg.register(
        QualifiedName::new("main", "init"),
        SymbolId::new("main.go", "init", 3),
        "function",
        "go",
    );

    // Nushell caller: no same-language match, and 2 cross-language candidates
    // Should return None — not pick an arbitrary cross-language match
    let result =
        reg.resolve_with_imports_and_kind("init", "caller", "src/caller.nu", "nushell", None);
    assert!(
        result.is_none(),
        "ambiguous cross-language with no same-language match should return None, got: {:?}",
        result.map(|id| id.as_str())
    );
}

#[test]
fn test_go_import_resolution_with_internal_module_path() {
    // Simulates: pkg/analyzer/hpa.go imports "github.com/k8sgpt-ai/k8sgpt/pkg/common"
    // and uses type "Context" from that package
    let mut reg = SymbolRegistry::new();

    // Register Go symbols with internal module paths (as derive_go_module_path produces)
    // pkg/common/types.go has Context type
    reg.register(
        QualifiedName::new("pkg::common", "Context"),
        SymbolId::new("pkg/common/types.go", "Context", 10),
        "struct",
        "go",
    );
    reg.register(
        QualifiedName::new("pkg::common", "Analyzer"),
        SymbolId::new("pkg/common/types.go", "Analyzer", 20),
        "struct",
        "go",
    );

    // pkg/analyzer/hpa.go has function that uses Context
    reg.register(
        QualifiedName::new("pkg::analyzer", "Analyze"),
        SymbolId::new("pkg/analyzer/hpa.go", "Analyze", 15),
        "function",
        "go",
    );

    // Add import table for pkg/analyzer/hpa.go
    // The import "github.com/k8sgpt-ai/k8sgpt/pkg/common" should map to internal "pkg::common"
    let mut table = crate::analysis::pipeline::ImportTable::default();
    // Key insight: the import table should use INTERNAL module format, not raw Go import path
    table
        .name_to_module
        .insert("common".to_string(), "pkg::common".to_string());
    reg.import_tables
        .insert("pkg/analyzer/hpa.go".to_string(), table);

    // Now resolve "Context" from pkg/analyzer/hpa.go
    // Step 1: Same module lookup fails (Context not in pkg::analyzer)
    // Step 2: Import table lookup should succeed (common -> pkg::common -> pkg::common::Context)
    let result = reg.resolve_with_imports_and_kind(
        "Context",
        "pkg::analyzer",
        "pkg/analyzer/hpa.go",
        "go",
        None,
    );

    assert!(
        result.is_some(),
        "Go import resolution should find Context via import table"
    );
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:pkg/common/types.go:Context:10",
        "Should resolve to Context in pkg/common"
    );
}

#[test]
fn test_go_import_resolution_bare_name_fallback() {
    // When import table doesn't have the mapping, bare name fallback should still work
    // for unique symbols
    let mut reg = SymbolRegistry::new();

    // Only one "UniqueType" exists in the whole codebase
    reg.register(
        QualifiedName::new("pkg::special", "UniqueType"),
        SymbolId::new("pkg/special/types.go", "UniqueType", 5),
        "struct",
        "go",
    );

    // Caller in different package with no import table entry for UniqueType
    let result = reg.resolve_with_imports_and_kind(
        "UniqueType",
        "pkg::caller",
        "pkg/caller/main.go",
        "go",
        None,
    );

    assert!(
        result.is_some(),
        "Unique bare name should resolve via fallback"
    );
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:pkg/special/types.go:UniqueType:5"
    );
}

#[test]
fn test_go_import_resolution_ambiguous_without_import_fails() {
    // Multiple types with same name in different packages
    // Without proper import table, should NOT resolve (ambiguous)
    let mut reg = SymbolRegistry::new();

    reg.register(
        QualifiedName::new("pkg::common", "Config"),
        SymbolId::new("pkg/common/types.go", "Config", 10),
        "struct",
        "go",
    );
    reg.register(
        QualifiedName::new("pkg::cache", "Config"),
        SymbolId::new("pkg/cache/types.go", "Config", 5),
        "struct",
        "go",
    );

    // Caller has no import table - ambiguous Config should not resolve
    let result = reg.resolve_with_imports_and_kind(
        "Config",
        "pkg::analyzer",
        "pkg/analyzer/main.go",
        "go",
        None,
    );

    assert!(
        result.is_none(),
        "Ambiguous bare name without import should return None, got: {:?}",
        result.map(|id| id.as_str())
    );
}

#[test]
fn test_go_glob_import_resolution() {
    // Simulates: pkg/analyzer/hpa.go imports "github.com/k8sgpt-ai/k8sgpt/pkg/common"
    // and uses type "Result" from that package via glob import
    let mut reg = SymbolRegistry::new();

    // Register Go symbols with internal module paths
    reg.register(
        QualifiedName::new("pkg::common", "Result"),
        SymbolId::new("pkg/common/types.go", "Result", 76),
        "struct",
        "go",
    );
    reg.register(
        QualifiedName::new("pkg::common", "Analyzer"),
        SymbolId::new("pkg/common/types.go", "Analyzer", 39),
        "struct",
        "go",
    );

    // pkg/analyzer/hpa.go
    reg.register(
        QualifiedName::new("pkg::analyzer", "Analyze"),
        SymbolId::new("pkg/analyzer/hpa.go", "Analyze", 31),
        "function",
        "go",
    );

    // Add glob import table for pkg/analyzer/hpa.go
    // Go imports are wildcards - normalise_import_path converts the path
    let mut table = crate::analysis::pipeline::ImportTable::default();
    // Glob modules contain the NORMALISED internal path, not the raw Go import
    table.glob_modules.push("pkg::common".to_string());
    reg.import_tables
        .insert("pkg/analyzer/hpa.go".to_string(), table);

    // Resolve "Result" from pkg/analyzer/hpa.go via glob import
    let result = reg.resolve_with_imports_and_kind(
        "Result",
        "pkg::analyzer",
        "pkg/analyzer/hpa.go",
        "go",
        None,
    );

    assert!(
        result.is_some(),
        "Go glob import resolution should find Result via glob_modules"
    );
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:pkg/common/types.go:Result:76",
        "Should resolve to Result in pkg/common"
    );

    // Also verify Analyzer resolves
    let result = reg.resolve_with_imports_and_kind(
        "Analyzer",
        "pkg::analyzer",
        "pkg/analyzer/hpa.go",
        "go",
        None,
    );
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:pkg/common/types.go:Analyzer:39"
    );
}

#[test]
fn test_go_glob_import_does_not_resolve_wrong_package() {
    // Verify glob import only resolves symbols from the imported package
    let mut reg = SymbolRegistry::new();

    // Result in pkg::common
    reg.register(
        QualifiedName::new("pkg::common", "Result"),
        SymbolId::new("pkg/common/types.go", "Result", 76),
        "struct",
        "go",
    );
    // Result in pkg::other (different package)
    reg.register(
        QualifiedName::new("pkg::other", "Result"),
        SymbolId::new("pkg/other/types.go", "Result", 10),
        "struct",
        "go",
    );

    // Only import pkg::common, not pkg::other
    let mut table = crate::analysis::pipeline::ImportTable::default();
    table.glob_modules.push("pkg::common".to_string());
    reg.import_tables
        .insert("pkg/analyzer/hpa.go".to_string(), table);

    // Should resolve to pkg::common::Result, not pkg::other::Result
    let result = reg.resolve_with_imports_and_kind(
        "Result",
        "pkg::analyzer",
        "pkg/analyzer/hpa.go",
        "go",
        None,
    );

    assert!(result.is_some());
    assert_eq!(
        result.unwrap().as_str(),
        "symbol:pkg/common/types.go:Result:76",
        "Should resolve to Result from imported package (pkg::common), not pkg::other"
    );
}

/// Integration tests that run the full pipeline on actual Go files
mod integration_tests {
    use crate::analysis::lang::LanguageAnalyser;
    use crate::analysis::lang::golang::Go;
    use crate::analysis::pipeline::{ImportTable, SymbolRegistry};
    use crate::analysis::types::*;

    /// Extract the name component from a SymbolId string.
    /// Format: "symbol:file_path:name:line"
    fn extract_name(sid: &str) -> Option<&str> {
        let s = sid.strip_prefix("symbol:")?;
        let last_colon = s.rfind(':')?;
        let before_last = &s[..last_colon];
        let second_last_colon = before_last.rfind(':')?;
        Some(&before_last[second_last_colon + 1..])
    }

    /// Simulates Phase 2 (register) + Phase 2b (resolve edges) of the pipeline
    /// to verify that type edges are correctly extracted AND resolved.
    fn simulate_pipeline_type_refs(
        files: &[(ParsedFile, &str)], // (parsed_file, normalised_import_path_for_glob)
    ) -> Vec<(String, String, String)> {
        // Vec of (from_symbol_name, edge_kind, to_type_name)
        let mut registry = SymbolRegistry::new();
        let mut resolved_refs = Vec::new();

        // Phase 2: Register all symbols
        for (pf, _) in files {
            let module_path = Go.derive_module_path(&pf.file_path);
            for sym in &pf.symbols {
                let sid = sym.symbol_id();
                let qn = QualifiedName::new(&module_path, &sym.name);
                registry.register(qn, sid, &sym.kind, &sym.language);
            }
        }

        // Build import tables (Phase 2 continued)
        for (pf, glob_import) in files {
            let mut table = ImportTable::default();
            if !glob_import.is_empty() {
                table.glob_modules.push(glob_import.to_string());
            }
            registry.import_tables.insert(pf.file_path.clone(), table);
        }

        // Phase 2b: Resolve type ref edges
        for (pf, _) in files {
            let module_path = Go.derive_module_path(&pf.file_path);
            for edge in &pf.edges {
                // Only process type-reference edges (not Calls, HasField, etc.)
                let is_type_ref = matches!(
                    edge.kind,
                    EdgeKind::ParamType
                        | EdgeKind::ReturnType
                        | EdgeKind::FieldType
                        | EdgeKind::TypeRef
                );
                if !is_type_ref {
                    continue;
                }

                let from_name = extract_name(edge.from.as_str()).unwrap_or("unknown");
                let to_name_raw = extract_name(edge.to.as_str()).unwrap_or("unknown");

                // Try to resolve the target through the registry
                if let Some(resolved_id) = registry.resolve_with_imports_and_kind(
                    to_name_raw,
                    &module_path,
                    &pf.file_path,
                    &pf.language,
                    None,
                ) {
                    let resolved_name = extract_name(resolved_id.as_str())
                        .unwrap_or(to_name_raw)
                        .to_string();
                    resolved_refs.push((
                        from_name.to_string(),
                        format!("{:?}", edge.kind),
                        resolved_name,
                    ));
                }
            }
        }

        resolved_refs
    }

    #[test]
    fn test_pipeline_resolves_same_package_type_refs() {
        // Single file with types and functions referencing them
        let code = r#"
package types

type Config struct {
    Name string
}

type Handler struct {
    Cfg *Config
}

func NewConfig() *Config {
    return &Config{}
}

func ProcessConfig(c Config) {
}
"#;

        let parsed = Go::extract(code, "pkg/types/types.go");

        // Simulate pipeline with no external imports
        let files = vec![(parsed, "")];
        let refs = simulate_pipeline_type_refs(&files);

        // Should have:
        // - Handler.Cfg field -> Config (FieldType)
        // - NewConfig returns -> Config (ReturnType)
        // - ProcessConfig accepts -> Config (ParamType)
        assert!(
            refs.iter().any(|(from, kind, to)| from == "Cfg"
                && kind.contains("FieldType")
                && to == "Config"),
            "Handler.Cfg should have FieldType -> Config, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|(from, kind, to)| from == "NewConfig"
                && kind.contains("ReturnType")
                && to == "Config"),
            "NewConfig should have ReturnType -> Config, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|(from, kind, to)| from == "ProcessConfig"
                && kind.contains("ParamType")
                && to == "Config"),
            "ProcessConfig should have ParamType -> Config, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_pipeline_resolves_cross_package_type_refs_via_glob_import() {
        // File 1: pkg/common/types.go - defines types
        let common_code = r#"
package common

type Result struct {
    Status int
}

type Analyzer struct {
    Name string
}
"#;
        let common_parsed = Go::extract(common_code, "pkg/common/types.go");

        // File 2: pkg/analyzer/hpa.go - imports and uses types from common
        let analyzer_code = r#"
package analyzer

func Analyze(a Analyzer) ([]Result, error) {
    return nil, nil
}
"#;
        let analyzer_parsed = Go::extract(analyzer_code, "pkg/analyzer/hpa.go");

        // Simulate pipeline - analyzer imports common via glob
        let files = vec![
            (common_parsed, ""),              // common has no imports
            (analyzer_parsed, "pkg::common"), // analyzer imports pkg::common as glob
        ];
        let refs = simulate_pipeline_type_refs(&files);

        // Should have:
        // - Analyze accepts -> Analyzer (ParamType) resolved via glob import
        // - Analyze returns -> Result (ReturnType) resolved via glob import
        assert!(
            refs.iter().any(|(from, kind, to)| from == "Analyze"
                && kind.contains("ParamType")
                && to == "Analyzer"),
            "Analyze should have ParamType -> Analyzer via glob import, got: {:?}",
            refs
        );
        assert!(
            refs.iter().any(|(from, kind, to)| from == "Analyze"
                && kind.contains("ReturnType")
                && to == "Result"),
            "Analyze should have ReturnType -> Result via glob import, got: {:?}",
            refs
        );
    }

    #[test]
    fn test_pipeline_extracts_tuple_slice_return_types() {
        // Test the specific pattern: func Foo() ([]Type, error)
        let code = r#"
package analyzer

type Result struct {
    Data string
}

func GetResults() ([]Result, error) {
    return nil, nil
}

func GetSingleResult() (*Result, error) {
    return nil, nil
}
"#;
        let parsed = Go::extract(code, "pkg/analyzer/analyzer.go");

        // Simulate pipeline with same package (no external imports needed)
        let files = vec![(parsed, "")];
        let refs = simulate_pipeline_type_refs(&files);

        // Should extract Result from both ([]Result, error) and (*Result, error)
        let result_returns: Vec<_> = refs
            .iter()
            .filter(|(_, kind, to)| kind.contains("ReturnType") && to == "Result")
            .collect();

        assert_eq!(
            result_returns.len(),
            2,
            "Should have 2 ReturnType -> Result refs (from GetResults and GetSingleResult), got: {:?}",
            refs
        );
    }
}
