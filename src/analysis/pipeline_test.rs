#[cfg(test)]
mod tests {
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
        let result = reg.resolve_with_imports("Config", "whatever", "any.rs", "rust");
        assert!(result.is_some(), "unique bare name should resolve");
        assert_eq!(result.unwrap().as_str(), "symbol:src/server.rs:Config:1");
    }

    #[test]
    fn test_bare_name_ambiguous_prefers_same_language() {
        let reg = build_registry();

        // "init" exists in Rust, Nushell, and Go
        // A Rust caller should get the Rust "init"
        let result = reg.resolve_with_imports("init", "caller", "src/caller.rs", "rust");
        assert!(result.is_some(), "should resolve init for Rust caller");
        assert_eq!(
            result.unwrap().as_str(),
            "symbol:src/server.rs:init:10",
            "Rust caller should get Rust init, not Nushell or Go"
        );

        // A Nushell caller should get the Nushell "init"
        let result = reg.resolve_with_imports("init", "caller", "src/caller.nu", "nushell");
        assert!(result.is_some(), "should resolve init for Nushell caller");
        assert_eq!(
            result.unwrap().as_str(),
            "symbol:src/commands/mod.nu:init:5",
            "Nushell caller should get Nushell init, not Rust or Go"
        );

        // A Go caller should get the Go "init"
        let result = reg.resolve_with_imports("init", "caller", "cmd/caller.go", "go");
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
        let result = reg.resolve_with_imports("init", "server", "src/server.rs", "rust");
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
        let result = reg.resolve_with_imports("init", "app", "src/app.rs", "rust");
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
        let result = reg.resolve_with_imports("Config", "caller", "src/caller.nu", "nushell");
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
        let result = reg.resolve_with_imports("process", "app", "src/app.rs", "rust");
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
        let result = reg.resolve_with_imports("init", "caller", "src/caller.nu", "nushell");
        assert!(
            result.is_none(),
            "ambiguous cross-language with no same-language match should return None, got: {:?}",
            result.map(|id| id.as_str())
        );
    }
}
