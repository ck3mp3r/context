use super::analyser::Go;
use crate::analysis::types::*;

fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/golang/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

/// Extract the name component from a SymbolId string.
/// Format: "symbol:file_path:name:line"
fn extract_name(sid: &str) -> Option<&str> {
    let s = sid.strip_prefix("symbol:")?;
    let last_colon = s.rfind(':')?;
    let before_last = &s[..last_colon];
    let second_last_colon = before_last.rfind(':')?;
    Some(&before_last[second_last_colon + 1..])
}

/// Check if an edge exists with the given kind and from/to names.
fn has_edge(parsed: &ParsedFile, kind: EdgeKind, from_name: &str, to_name: &str) -> bool {
    parsed.edges.iter().any(|e| {
        e.kind == kind
            && extract_name(e.from.as_str()) == Some(from_name)
            && extract_name(e.to.as_str()) == Some(to_name)
    })
}

/// Get all edges of a given kind as (from_name, to_name) pairs.
fn edges_of_kind<'a>(parsed: &'a ParsedFile, kind: EdgeKind) -> Vec<(&'a str, &'a str)> {
    parsed
        .edges
        .iter()
        .filter(|e| e.kind == kind)
        .filter_map(|e| {
            let from = extract_name(e.from.as_str())?;
            let to = extract_name(e.to.as_str())?;
            Some((from, to))
        })
        .collect()
}

/// Get all edges from a specific symbol.
fn edges_from<'a>(parsed: &'a ParsedFile, kind: EdgeKind, from_name: &str) -> Vec<&'a str> {
    parsed
        .edges
        .iter()
        .filter(|e| e.kind == kind && extract_name(e.from.as_str()) == Some(from_name))
        .filter_map(|e| extract_name(e.to.as_str()))
        .collect()
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
fn test_cache_methods_accept_receiver_type() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // Methods with receiver (c *Cache) should have ParamType edge to Cache
    // Find all methods that have ParamType edge TO Cache
    let methods_accepting_cache: Vec<&str> = edges_of_kind(&parsed, EdgeKind::ParamType)
        .iter()
        .filter(|(_, to)| *to == "Cache")
        .map(|(from, _)| *from)
        .collect();

    assert!(
        methods_accepting_cache.contains(&"Get"),
        "Get should accept Cache, got: {:?}",
        methods_accepting_cache
    );
    assert!(
        methods_accepting_cache.contains(&"Set"),
        "Set should accept Cache, got: {:?}",
        methods_accepting_cache
    );
    assert!(
        methods_accepting_cache.contains(&"Delete"),
        "Delete should accept Cache, got: {:?}",
        methods_accepting_cache
    );
    assert!(
        methods_accepting_cache.contains(&"Cleanup"),
        "Cleanup should accept Cache, got: {:?}",
        methods_accepting_cache
    );
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

    // Calls edges should include time.Now() - check for edge to "Now"
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);

    assert!(
        calls.iter().any(|(_, to)| *to == "Now"),
        "should capture time.Now() selector call, got: {:?}",
        calls
    );
}

#[test]
fn test_cache_composite_literals() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // Composite literals like Cache{} and Item{} should appear as Calls edges
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);

    assert!(
        calls.iter().any(|(_, to)| *to == "Cache" || *to == "Item"),
        "should capture composite literals as constructor calls, got calls: {:?}",
        calls
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
    // Go imports store pkg name in imported_names AND set is_glob for type resolution
    assert!(imp.entry.imported_names.contains(&"log".to_string()));
    assert!(imp.entry.is_glob);
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

    // Calls edges should include fmt.Sprintf and http.ListenAndServe
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);

    assert!(
        calls.iter().any(|(_, to)| *to == "Sprintf"),
        "should capture fmt.Sprintf(), got: {:?}",
        calls
    );
    assert!(
        calls.iter().any(|(_, to)| *to == "ListenAndServe"),
        "should capture http.ListenAndServe(), got: {:?}",
        calls
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

    let extends_edges = edges_of_kind(&parsed, EdgeKind::Extends);

    assert!(
        extends_edges
            .iter()
            .any(|(from, to)| *from == "ReadWriteCache" && *to == "Cache"),
        "should extract ReadWriteCache embeds Cache as heritage, got: {:?}",
        extends_edges
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

#[test]
fn test_go_visibility() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let vis = |name: &str| -> Option<String> {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.visibility.clone())
    };

    // Exported (uppercase first letter)
    assert_eq!(
        vis("Cache"),
        Some("public".to_string()),
        "uppercase struct should be public"
    );
    assert_eq!(
        vis("Item"),
        Some("public".to_string()),
        "uppercase struct should be public"
    );
    assert_eq!(
        vis("Cacher"),
        Some("public".to_string()),
        "uppercase interface should be public"
    );
    assert_eq!(
        vis("New"),
        Some("public".to_string()),
        "uppercase function should be public"
    );
    assert_eq!(
        vis("DefaultTTL"),
        Some("public".to_string()),
        "uppercase const should be public"
    );

    // Unexported (lowercase first letter)
    assert_eq!(
        vis("mu"),
        Some("private".to_string()),
        "lowercase field should be private"
    );
    assert_eq!(
        vis("items"),
        Some("private".to_string()),
        "lowercase field should be private"
    );
}

#[test]
fn test_entry_type_init() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "pkg/cache/cache.go");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name && s.kind == "function")
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("init"),
        Some("init".to_string()),
        "func init() should have entry_type 'init'"
    );
}

#[test]
fn test_entry_type_test_function() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "pkg/cache/cache_test.go");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("TestCacheGet"),
        Some("test".to_string()),
        "func TestXxx should have entry_type 'test'"
    );
}

#[test]
fn test_entry_type_benchmark() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "pkg/cache/cache_test.go");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("BenchmarkCacheSet"),
        Some("benchmark".to_string()),
        "func BenchmarkXxx should have entry_type 'benchmark'"
    );
}

#[test]
fn test_entry_type_example() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "pkg/cache/cache_test.go");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("ExampleNew"),
        Some("example".to_string()),
        "func ExampleXxx should have entry_type 'example'"
    );
}

#[test]
fn test_entry_type_regular_function_is_none() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "pkg/cache/cache.go");

    let entry = |name: &str| {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name && s.kind == "function")
            .and_then(|s| s.entry_type.clone())
    };

    assert_eq!(
        entry("New"),
        None,
        "regular function should not have entry_type"
    );
}

#[test]
fn test_struct_field_containment() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let cache_fields: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains("Cache"))
        .filter(|e| !e.from.as_str().contains("ReadWriteCache"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        cache_fields.contains(&"mu"),
        "Cache should contain field 'mu', got: {:?}",
        cache_fields
    );
    assert!(
        cache_fields.contains(&"items"),
        "Cache should contain field 'items', got: {:?}",
        cache_fields
    );

    let item_fields: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains("Item"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        item_fields.contains(&"Value"),
        "Item should contain field 'Value', got: {:?}",
        item_fields
    );
    assert!(
        item_fields.contains(&"Expires"),
        "Item should contain field 'Expires', got: {:?}",
        item_fields
    );

    let rwc_fields: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains("ReadWriteCache"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        rwc_fields.contains(&"maxSize"),
        "ReadWriteCache should contain field 'maxSize', got: {:?}",
        rwc_fields
    );
    assert!(
        rwc_fields.contains(&"hits"),
        "ReadWriteCache should contain field 'hits', got: {:?}",
        rwc_fields
    );
}

#[test]
fn test_interface_method_containment() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    let method_syms: Vec<&RawSymbol> = parsed
        .symbols
        .iter()
        .filter(|s| s.kind == "function" && s.name == "Get" || s.name == "Set")
        .collect();

    assert!(
        method_syms
            .iter()
            .any(|s| s.name == "Get" && s.start_line >= 21 && s.start_line <= 22),
        "should extract Cacher.Get as a function symbol"
    );
    assert!(
        method_syms
            .iter()
            .any(|s| s.name == "Set" && s.start_line >= 22 && s.start_line <= 23),
        "should extract Cacher.Set as a function symbol"
    );

    let cacher_methods: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasMethod && e.from.as_str().contains("Cacher"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        cacher_methods.contains(&"Get"),
        "Cacher interface should contain method 'Get', got: {:?}",
        cacher_methods
    );
    assert!(
        cacher_methods.contains(&"Set"),
        "Cacher interface should contain method 'Set', got: {:?}",
        cacher_methods
    );
}

#[test]
fn test_server_struct_field_containment() {
    let code = load_testdata("server.go");
    let parsed = Go::extract(&code, "server/server.go");

    let server_fields: Vec<&str> = parsed
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::HasField && e.from.as_str().contains("Server"))
        .filter_map(|e| e.to.as_str().split(':').nth(2))
        .collect();

    assert!(
        server_fields.contains(&"host"),
        "Server should contain field 'host', got: {:?}",
        server_fields
    );
    assert!(
        server_fields.contains(&"port"),
        "Server should contain field 'port', got: {:?}",
        server_fields
    );
}

// ============================================================================
// Type Reference Tests (TDD RED phase)
// ============================================================================
// These tests define the expected behavior for type reference extraction.
// They will FAIL until the Go parser implements extract_type_refs().

// --- Parameter Type References ---

#[test]
fn test_go_param_type_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessItems(items []Item) should produce ParamType edge to Item
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessItems");

    assert!(
        param_types.contains(&"Item"),
        "ProcessItems should accept Item (from []Item), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessConfig(config *Config) should produce ParamType edge to Config
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessConfig");

    assert!(
        param_types.contains(&"Config"),
        "ProcessConfig should accept Config (from *Config), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessDirect(config Config) should produce ParamType edge to Config
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessDirect");

    assert!(
        param_types.contains(&"Config"),
        "ProcessDirect should accept Config, got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_multiple() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMultiple(items []Item, config *Config, cache Cache)
    // should produce ParamType edges to Item, Config, and Cache
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessMultiple");

    assert!(
        param_types.contains(&"Item"),
        "ProcessMultiple should accept Item, got: {:?}",
        param_types
    );
    assert!(
        param_types.contains(&"Config"),
        "ProcessMultiple should accept Config, got: {:?}",
        param_types
    );
    assert!(
        param_types.contains(&"Cache"),
        "ProcessMultiple should accept Cache, got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_map_value() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMap(data map[string]Config) should extract Config as param type
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessMap");

    assert!(
        param_types.contains(&"Config"),
        "ProcessMap should accept Config (map value), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_map_key() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMapKey(data map[Item]string) should extract Item as param type
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessMapKey");

    assert!(
        param_types.contains(&"Item"),
        "ProcessMapKey should accept Item (map key), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_ptr_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessPtrQualified(req *http.Request) should extract Request as param type
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessPtrQualified");

    assert!(
        param_types.contains(&"Request"),
        "ProcessPtrQualified should accept Request (*http.Request), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_param_type_filters_builtins() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Built-in types (string, int, error, bool, etc.) should NOT produce ParamType edges
    let builtin_edges: Vec<_> = edges_of_kind(&parsed, EdgeKind::ParamType)
        .into_iter()
        .filter(|(_, to)| {
            matches!(
                *to,
                "string" | "int" | "int64" | "bool" | "error" | "byte" | "rune" | "any"
            )
        })
        .collect();

    assert!(
        builtin_edges.is_empty(),
        "Built-in types should be filtered out, but found: {:?}",
        builtin_edges
    );
}

// --- Return Type References ---

#[test]
fn test_go_return_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // NewConfig() *Config should produce ReturnType edge to Config
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "NewConfig");

    assert!(
        return_types.contains(&"Config"),
        "NewConfig should return Config, got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetCache() Cache should produce ReturnType edge to Cache
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetCache");

    assert!(
        return_types.contains(&"Cache"),
        "GetCache should return Cache, got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_tuple() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // LoadConfigAndError(path string) (*Config, error)
    // should produce ReturnType edge to Config but NOT error
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "LoadConfigAndError");

    assert!(
        return_types.contains(&"Config"),
        "LoadConfigAndError should return Config, got: {:?}",
        return_types
    );
    assert!(
        !return_types.contains(&"error"),
        "LoadConfigAndError should NOT have error in return types (it's a builtin), got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetItems() []Item should produce ReturnType edge to Item
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetItems");

    assert!(
        return_types.contains(&"Item"),
        "GetItems should return Item (from []Item), got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_tuple_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetItemsAndError() ([]Item, error) should produce ReturnType edge to Item, NOT error
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetItemsAndError");

    assert!(
        return_types.contains(&"Item"),
        "GetItemsAndError should return Item (from []Item), got: {:?}",
        return_types
    );
    assert!(
        !return_types.contains(&"error"),
        "GetItemsAndError should NOT have error in return types (it's a builtin), got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_ptr_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetPtrQualified() *http.Handler should produce ReturnType edge to Handler
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetPtrQualified");

    assert!(
        return_types.contains(&"Handler"),
        "GetPtrQualified should return Handler (*http.Handler), got: {:?}",
        return_types
    );
}

#[test]
fn test_go_return_type_multiple_user_types() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetMultipleTypes() (Cache, Handler, error) should produce ReturnType edges to Cache AND Handler, NOT error
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetMultipleTypes");

    assert!(
        return_types.contains(&"Cache"),
        "GetMultipleTypes should return Cache, got: {:?}",
        return_types
    );
    assert!(
        return_types.contains(&"Handler"),
        "GetMultipleTypes should return Handler, got: {:?}",
        return_types
    );
    assert!(
        !return_types.contains(&"error"),
        "GetMultipleTypes should NOT have error in return types (it's a builtin), got: {:?}",
        return_types
    );
}

// --- Method Parameter and Return Type References ---

#[test]
fn test_go_method_param_and_return() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // func (c *Config) Process(req Request) Response
    // should produce ParamType edge to Request AND ReturnType edge to Response
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "Process");
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "Process");

    assert!(
        param_types.contains(&"Request"),
        "Process method should accept Request, got: {:?}",
        param_types
    );

    assert!(
        return_types.contains(&"Response"),
        "Process method should return Response, got: {:?}",
        return_types
    );
}

#[test]
fn test_go_method_return_interface() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // func (c *Config) GetHandler() Handler
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetHandler");

    assert!(
        return_types.contains(&"Handler"),
        "GetHandler should return Handler, got: {:?}",
        return_types
    );
}

// --- Field Type References ---

#[test]
fn test_go_field_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Handler Handler (direct type)
    // Note: The field is named "Handler" and has type Handler
    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "Handler", "Handler"),
        "Handler field should have FieldType edge to Handler, got: {:?}",
        edges_of_kind(&parsed, EdgeKind::FieldType)
    );
}

#[test]
fn test_go_field_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Cache *Cache (pointer type)
    // Note: The field is named "Cache" and has type *Cache
    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "Cache", "Cache"),
        "Cache field should have FieldType edge to Cache, got: {:?}",
        edges_of_kind(&parsed, EdgeKind::FieldType)
    );
}

#[test]
fn test_go_field_type_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Items []Item (slice type)
    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "Items", "Item"),
        "Items field should have FieldType edge to Item, got: {:?}",
        edges_of_kind(&parsed, EdgeKind::FieldType)
    );
}

#[test]
fn test_go_field_type_map_value() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Meta map[string]Value (map value type)
    assert!(
        has_edge(&parsed, EdgeKind::FieldType, "Meta", "Value"),
        "Meta field should have FieldType edge to Value, got: {:?}",
        edges_of_kind(&parsed, EdgeKind::FieldType)
    );
}

#[test]
fn test_go_field_type_filters_string() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Name string should NOT produce a FieldType edge to "string"
    // String is a builtin and should be filtered out
    let string_field_edges: Vec<_> = edges_of_kind(&parsed, EdgeKind::FieldType)
        .into_iter()
        .filter(|(_, to)| *to == "string")
        .collect();

    assert!(
        string_field_edges.is_empty(),
        "Name field should NOT have FieldType edge to string (builtin), got: {:?}",
        string_field_edges
    );
}

// --- Variadic Parameter Types ---

#[test]
fn test_go_variadic_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMany(items ...Item) should produce ParamType edge to Item
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessMany");

    assert!(
        param_types.contains(&"Item"),
        "ProcessMany should accept Item (from ...Item), got: {:?}",
        param_types
    );
}

// --- Interface Method Signatures ---

#[test]
fn test_go_interface_method_types() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Processor.Process(req Request) Response
    // Find the Process method inside Processor interface using HasMethod edges
    let process_method_edge = parsed.edges.iter().find(|e| {
        e.kind == EdgeKind::HasMethod
            && e.from.as_str().contains("Processor")
            && e.to.as_str().contains("Process")
    });

    // Ensure we actually found the method
    assert!(
        process_method_edge.is_some(),
        "Should find Process method in Processor interface"
    );

    // Check Process has ParamType edge to Request and ReturnType edge to Response
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "Process");
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "Process");

    assert!(
        param_types.contains(&"Request"),
        "Processor.Process should accept Request, got: {:?}",
        param_types
    );
    assert!(
        return_types.contains(&"Response"),
        "Processor.Process should return Response, got: {:?}",
        return_types
    );
}

// --- Generic Types (Go 1.18+) ---

#[test]
fn test_go_generic_param_inner_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessGeneric(c Container[Item]) should produce ParamType edge to Item
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ProcessGeneric");

    assert!(
        param_types.contains(&"Item"),
        "ProcessGeneric should accept Item (from Container[Item]), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_generic_return_inner_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetContainer() Container[Config] should produce ReturnType edge to Config
    let return_types = edges_from(&parsed, EdgeKind::ReturnType, "GetContainer");

    assert!(
        return_types.contains(&"Config"),
        "GetContainer should return Config (from Container[Config]), got: {:?}",
        return_types
    );
}

// --- Channel Types ---

#[test]
fn test_go_channel_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // SendItems(ch chan Item) should produce ParamType edge to Item
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "SendItems");

    assert!(
        param_types.contains(&"Item"),
        "SendItems should accept Item (from chan Item), got: {:?}",
        param_types
    );
}

#[test]
fn test_go_receive_channel_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ReceiveConfigs(ch <-chan Config) should produce ParamType edge to Config
    let param_types = edges_from(&parsed, EdgeKind::ParamType, "ReceiveConfigs");

    assert!(
        param_types.contains(&"Config"),
        "ReceiveConfigs should accept Config (from <-chan Config), got: {:?}",
        param_types
    );
}

// --- Summary count test for verification ---

#[test]
fn test_go_type_refs_summary() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    let param_count = edges_of_kind(&parsed, EdgeKind::ParamType).len();
    let return_count = edges_of_kind(&parsed, EdgeKind::ReturnType).len();
    let field_count = edges_of_kind(&parsed, EdgeKind::FieldType).len();

    // These are minimum counts based on types.go content
    assert!(
        param_count >= 15,
        "Expected at least 15 ParamType edges, got {}. All: {:?}",
        param_count,
        edges_of_kind(&parsed, EdgeKind::ParamType)
    );
    assert!(
        return_count >= 8,
        "Expected at least 8 ReturnType edges, got {}. All: {:?}",
        return_count,
        edges_of_kind(&parsed, EdgeKind::ReturnType)
    );
    assert!(
        field_count >= 5,
        "Expected at least 5 FieldType edges, got {}. All: {:?}",
        field_count,
        edges_of_kind(&parsed, EdgeKind::FieldType)
    );
}

// --- normalise_import_path tests ---

#[test]
fn test_normalise_import_path_pkg_prefix() {
    use crate::analysis::lang::LanguageAnalyser;

    let go = super::analyser::Go;

    assert_eq!(
        go.normalise_import_path("github.com/acme/myapp/pkg/common"),
        "pkg::common",
        "Should extract pkg/common suffix and convert to internal format"
    );

    assert_eq!(
        go.normalise_import_path("github.com/acme/myapp/pkg/service"),
        "pkg::service",
        "Should extract pkg/service suffix"
    );
}

#[test]
fn test_normalise_import_path_internal_prefix() {
    use crate::analysis::lang::LanguageAnalyser;

    let go = super::analyser::Go;

    assert_eq!(
        go.normalise_import_path("github.com/acme/myapp/internal/cache"),
        "internal::cache",
        "Should handle internal packages"
    );
}

#[test]
fn test_normalise_import_path_cmd_prefix() {
    use crate::analysis::lang::LanguageAnalyser;

    let go = super::analyser::Go;

    assert_eq!(
        go.normalise_import_path("github.com/acme/myapp/cmd/server"),
        "cmd::server",
        "Should extract cmd/server suffix"
    );
}

#[test]
fn test_normalise_import_path_nested() {
    use crate::analysis::lang::LanguageAnalyser;

    let go = super::analyser::Go;

    assert_eq!(
        go.normalise_import_path("github.com/acme/myapp/pkg/api/v1"),
        "pkg::api::v1",
        "Should handle nested packages"
    );
}

#[test]
fn test_normalise_import_path_stdlib() {
    use crate::analysis::lang::LanguageAnalyser;

    let go = super::analyser::Go;

    assert_eq!(
        go.normalise_import_path("fmt"),
        "fmt",
        "Standard library imports stay as-is"
    );

    assert_eq!(
        go.normalise_import_path("net/http"),
        "net::http",
        "Standard library with path converts slashes"
    );
}

// --- Type alias tests ---

#[test]
fn test_go_type_alias_primitive() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    let sym = parsed.symbols.iter().find(|s| s.name == "Duration");
    assert!(
        sym.is_some_and(|s| s.kind == "type"),
        "Duration type alias should be extracted with kind 'type'"
    );
}

#[test]
fn test_go_type_alias_function() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    let sym = parsed.symbols.iter().find(|s| s.name == "HandlerFunc");
    assert!(
        sym.is_some_and(|s| s.kind == "type"),
        "HandlerFunc function type alias should be extracted with kind 'type'"
    );
}

#[test]
fn test_go_type_alias_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    let sym = parsed.symbols.iter().find(|s| s.name == "ItemSlice");
    assert!(
        sym.is_some_and(|s| s.kind == "type"),
        "ItemSlice slice type alias should be extracted with kind 'type'"
    );
}

#[test]
fn test_go_type_alias_no_duplicate_struct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    // Config is a struct, NOT a type alias - should only appear once as struct
    let config_syms: Vec<_> = parsed
        .symbols
        .iter()
        .filter(|s| s.name == "Config")
        .collect();
    assert_eq!(
        config_syms.len(),
        1,
        "Config should appear exactly once (as struct, not also as type alias)"
    );
    assert_eq!(
        config_syms[0].kind, "struct",
        "Config should be kind 'struct', not 'type'"
    );
}

#[test]
fn test_go_type_alias_no_duplicate_interface() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    // Handler interface should only appear once as kind="interface", not also as kind="type"
    // (Note: there's also a field named Handler at line 11, but that's kind="field")
    let handler_interface_syms: Vec<_> = parsed
        .symbols
        .iter()
        .filter(|s| s.name == "Handler" && s.kind == "interface")
        .collect();

    assert_eq!(
        handler_interface_syms.len(),
        1,
        "Handler interface should appear exactly once"
    );

    // Make sure there's no type alias for Handler
    let handler_type_syms: Vec<_> = parsed
        .symbols
        .iter()
        .filter(|s| s.name == "Handler" && s.kind == "type")
        .collect();

    assert_eq!(
        handler_type_syms.len(),
        0,
        "Handler should NOT appear as kind='type' (type alias)"
    );
}

#[test]
fn test_go_type_alias_visibility() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "types/types.go");

    // Duration starts with uppercase - public
    let duration = parsed.symbols.iter().find(|s| s.name == "Duration");
    assert!(
        duration.is_some_and(|s| s.visibility == Some("public".to_string())),
        "Duration should be public (uppercase)"
    );
}

// --- Function reference / callback tests ---

#[test]
fn test_go_func_ref_as_argument() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // RegisterHook(myHookHandler) should create a Calls edge to myHookHandler
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    let has_hook_call = calls.iter().any(|(_, to)| *to == "myHookHandler");

    assert!(
        has_hook_call,
        "myHookHandler passed as argument should create a call edge, got: {:?}",
        calls
    );
}

#[test]
fn test_go_func_ref_multiple() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // OnInit(setupConfig) should create a Calls edge to setupConfig
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    let has_setup_call = calls.iter().any(|(_, to)| *to == "setupConfig");

    assert!(
        has_setup_call,
        "setupConfig passed as argument should create a call edge, got: {:?}",
        calls
    );
}

#[test]
fn test_go_func_ref_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // CallWithQualifiedFuncRef calls: http.HandleFunc("/notfound", http.NotFound)
    // This should create a Calls edge to NotFound
    let calls = edges_of_kind(&parsed, EdgeKind::Calls);
    let has_notfound_call = calls.iter().any(|(_, to)| *to == "NotFound");

    assert!(
        has_notfound_call,
        "http.NotFound passed as argument should create a call edge, got: {:?}",
        calls
    );
}

// --- Type Assertions (Usage edges) ---

#[test]
fn test_go_type_assertion_uses_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseTypeAssertion does: v, ok := x.(*Config)
    // This should create a TypeRef edge from UseTypeAssertion to Config
    let type_refs = edges_from(&parsed, EdgeKind::TypeRef, "UseTypeAssertion");

    assert!(
        type_refs.contains(&"Config"),
        "UseTypeAssertion should have TypeRef edge to Config from type assertion x.(*Config), got: {:?}",
        type_refs
    );
}

#[test]
fn test_go_composite_literal_uses_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseCompositeLiteral does: Config{}, &Config{}
    // These should create Calls edges (composite literal = constructor call)
    // Note: []Item{{}, {}} won't create Item calls because the inner {} don't have explicit type identifiers
    let calls = edges_from(&parsed, EdgeKind::Calls, "UseCompositeLiteral");

    assert!(
        calls.contains(&"Config"),
        "UseCompositeLiteral should have Calls edge to Config from Config{{}}, got: {:?}",
        calls
    );
    // The nested {} in []Item{{}, {}} don't have explicit type - Go infers from slice type
    // So we don't expect a call to Item from the implicit type literals
}

#[test]
fn test_go_var_declaration_type_annotation_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseVarDeclaration does: var cfg Config, var cfgPtr *Config, var items []Item
    // This should create TypeRef edges to Config and Item
    let type_refs = edges_from(&parsed, EdgeKind::TypeRef, "UseVarDeclaration");

    assert!(
        type_refs.contains(&"Config"),
        "UseVarDeclaration should have TypeRef edge to Config from var cfg Config, got: {:?}",
        type_refs
    );
    assert!(
        type_refs.contains(&"Item"),
        "UseVarDeclaration should have TypeRef edge to Item from var items []Item, got: {:?}",
        type_refs
    );
}

// --- Method Receiver Types (Accepts edges) ---

#[test]
fn test_go_method_receiver_accepts_edge() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // func (c *Cache) Get(key string) should create ParamType edge from Get to Cache
    // (receiver type is treated as a parameter type)
    let get_params = edges_from(&parsed, EdgeKind::ParamType, "Get");

    assert!(
        get_params.contains(&"Cache"),
        "Get method should have ParamType edge to Cache (receiver type), got: {:?}",
        get_params
    );
}

// --- Test File Detection ---

#[test]
fn test_go_test_file_all_symbols_tagged() {
    // All symbols in a _test.go file should have entry_type = "test"
    let code = r#"
package cache

type mockClient struct {
    callCount int
}

func (m *mockClient) Call() {
    m.callCount++
}

func setupTest() *mockClient {
    return &mockClient{}
}

func TestSomething(t *testing.T) {
    client := setupTest()
    client.Call()
}
"#;
    let parsed = Go::extract(code, "cache/cache_test.go");

    // All symbols should have entry_type = "test"
    let non_test_symbols: Vec<&str> = parsed
        .symbols
        .iter()
        .filter(|s| s.entry_type.is_none() && s.kind != "package" && s.kind != "field")
        .map(|s| s.name.as_str())
        .collect();

    assert!(
        non_test_symbols.is_empty(),
        "All symbols in _test.go should have entry_type, but these don't: {:?}",
        non_test_symbols
    );

    // Verify specific symbols have entry_type = "test"
    let mock_client = parsed
        .symbols
        .iter()
        .find(|s| s.name == "mockClient")
        .expect("mockClient struct should exist");
    assert_eq!(
        mock_client.entry_type,
        Some("test".to_string()),
        "mockClient should have entry_type 'test'"
    );

    let setup_test = parsed
        .symbols
        .iter()
        .find(|s| s.name == "setupTest")
        .expect("setupTest function should exist");
    assert_eq!(
        setup_test.entry_type,
        Some("test".to_string()),
        "setupTest should have entry_type 'test'"
    );
}

#[test]
fn test_go_non_test_file_not_tagged() {
    // Symbols in regular .go files should NOT get entry_type just from file name
    let code = r#"
package cache

type Client struct {
    url string
}

func NewClient(url string) *Client {
    return &Client{url: url}
}
"#;
    let parsed = Go::extract(code, "cache/cache.go");

    let client = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Client")
        .expect("Client struct should exist");
    assert_eq!(
        client.entry_type, None,
        "Client in non-test file should not have entry_type"
    );

    let new_client = parsed
        .symbols
        .iter()
        .find(|s| s.name == "NewClient")
        .expect("NewClient function should exist");
    assert_eq!(
        new_client.entry_type, None,
        "NewClient in non-test file should not have entry_type"
    );
}
