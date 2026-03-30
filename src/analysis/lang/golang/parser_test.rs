use super::parser::Go;
use crate::analysis::types::*;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/golang/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

#[test]
fn test_query_compiles() {
    let language = Go::grammar();
    assert!(tree_sitter::Query::new(&language, Go::queries()).is_ok());
}

// --- cache.go ---

#[test]
fn test_cache_symbols() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let sym = |name: &str| parsed.symbols.iter().find(|s| s.name == name);

    assert!(sym("cache").is_some_and(|s| s.kind == "package"));
    assert!(sym("Cache").is_some_and(|s| s.kind == "struct"));
    assert!(sym("Item").is_some_and(|s| s.kind == "struct"));
    assert!(sym("Cacher").is_some_and(|s| s.kind == "interface"));
    assert!(sym("New").is_some_and(|s| s.kind == "function"));
    assert!(sym("DefaultTTL").is_some_and(|s| s.kind == "const"));
}

#[test]
fn test_cache_methods_containment() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let cache_methods: Vec<&str> = parsed
        .containments
        .iter()
        .filter(|c| c.parent_name == "Cache")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
        .collect();

    assert!(cache_methods.contains(&"Get"));
    assert!(cache_methods.contains(&"Set"));
    assert!(cache_methods.contains(&"Delete"));
    assert!(cache_methods.contains(&"Cleanup"));
}

#[test]
fn test_cache_imports() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    assert!(
        parsed.imports.iter().any(|i| i.entry.module_path == "sync"),
        "should extract 'sync' import from grouped import block, got: {:?}",
        parsed
            .imports
            .iter()
            .map(|i| &i.entry.module_path)
            .collect::<Vec<_>>()
    );
    assert!(
        parsed.imports.iter().any(|i| i.entry.module_path == "time"),
        "should extract 'time' import from grouped import block"
    );
}

#[test]
fn test_cache_selector_calls() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let scoped: Vec<(&str, Option<&str>)> = parsed
        .calls
        .iter()
        .filter(|c| c.call_form == CallForm::Scoped)
        .map(|c| (c.callee_name.as_str(), c.qualifier.as_deref()))
        .collect();

    assert!(
        scoped.iter().any(|(name, _)| *name == "Now"),
        "should capture time.Now() selector call, got: {:?}",
        scoped
    );
}

#[test]
fn test_cache_composite_literals() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    assert!(
        parsed
            .calls
            .iter()
            .any(|c| c.callee_name == "Cache" || c.callee_name == "Item"),
        "should capture composite literals as constructor calls, got calls: {:?}",
        parsed
            .calls
            .iter()
            .map(|c| &c.callee_name)
            .collect::<Vec<_>>()
    );
}

// --- server.go ---

#[test]
fn test_server_aliased_import() {
    let code = load_testdata("server.go");
    let parsed = Go::extract(&code, "server/server.go");

    let aliased = parsed.imports.iter().find(|i| i.entry.alias.is_some());

    assert!(
        aliased.is_some(),
        "should extract aliased import, got: {:?}",
        parsed
            .imports
            .iter()
            .map(|i| (&i.entry.module_path, &i.entry.alias))
            .collect::<Vec<_>>()
    );

    let imp = aliased.unwrap();
    assert_eq!(imp.entry.module_path, "github.com/sirupsen/logrus");
    assert_eq!(imp.entry.alias.as_deref(), Some("log"));
    assert!(imp.entry.imported_names.contains(&"log".to_string()));
}

#[test]
fn test_server_all_imports() {
    let code = load_testdata("server.go");
    let parsed = Go::extract(&code, "server/server.go");

    assert!(
        parsed.imports.iter().any(|i| i.entry.module_path == "fmt"),
        "should extract 'fmt' from grouped imports"
    );
    assert!(
        parsed
            .imports
            .iter()
            .any(|i| i.entry.module_path == "net/http"),
        "should extract 'net/http' from grouped imports"
    );
}

#[test]
fn test_server_selector_calls() {
    let code = load_testdata("server.go");
    let parsed = Go::extract(&code, "server/server.go");

    let scoped: Vec<(&str, Option<&str>)> = parsed
        .calls
        .iter()
        .filter(|c| c.call_form == CallForm::Scoped)
        .map(|c| (c.callee_name.as_str(), c.qualifier.as_deref()))
        .collect();

    assert!(
        scoped
            .iter()
            .any(|(name, qual)| *name == "Sprintf" && *qual == Some("fmt")),
        "should capture fmt.Sprintf(), got: {:?}",
        scoped
    );
    assert!(
        scoped
            .iter()
            .any(|(name, qual)| *name == "ListenAndServe" && *qual == Some("http")),
        "should capture http.ListenAndServe(), got: {:?}",
        scoped
    );
}

#[test]
fn test_cache_struct_fields() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let fields: Vec<&str> = parsed
        .symbols
        .iter()
        .filter(|s| s.kind == "field")
        .map(|s| s.name.as_str())
        .collect();

    assert!(
        fields.contains(&"mu"),
        "should extract Cache.mu field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"items"),
        "should extract Cache.items field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"Value"),
        "should extract Item.Value field, got: {:?}",
        fields
    );
    assert!(
        fields.contains(&"Expires"),
        "should extract Item.Expires field, got: {:?}",
        fields
    );
}

#[test]
fn test_cache_struct_embedding_heritage() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    assert!(
        parsed
            .heritage
            .iter()
            .any(|h| h.type_name == "ReadWriteCache" && h.parent_name == "Cache"),
        "should extract ReadWriteCache embeds Cache as heritage, got: {:?}",
        parsed.heritage
    );
}

#[test]
fn test_cache_write_access_assignment() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    assert!(
        parsed
            .write_accesses
            .iter()
            .any(|w| w.property == "maxSize" && w.receiver == "rwc"),
        "should capture rwc.maxSize = size write access, got: {:?}",
        parsed.write_accesses
    );
}

#[test]
fn test_cache_write_access_increment() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    assert!(
        parsed
            .write_accesses
            .iter()
            .any(|w| w.property == "hits" && w.receiver == "rwc"),
        "should capture rwc.hits++ write access, got: {:?}",
        parsed.write_accesses
    );
}
