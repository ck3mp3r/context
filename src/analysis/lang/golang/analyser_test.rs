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

    // Methods with receiver (c *Cache) should have Accepts edge to Cache
    // Find all methods that accept Cache as a parameter (receiver)
    let methods_accepting_cache: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.type_name == "Cache" && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| parsed.symbols[tr.from_symbol_idx].name.as_str())
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "Cache")
        .filter(|c| parsed.symbols[c.child_symbol_idx].kind == "field")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "Item")
        .filter(|c| parsed.symbols[c.child_symbol_idx].kind == "field")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "ReadWriteCache")
        .filter(|c| parsed.symbols[c.child_symbol_idx].kind == "field")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "Cacher")
        .filter(|c| parsed.symbols[c.child_symbol_idx].kind == "function")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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
        .containments
        .iter()
        .filter(|c| c.parent_name == "Server")
        .filter(|c| parsed.symbols[c.child_symbol_idx].kind == "field")
        .map(|c| parsed.symbols[c.child_symbol_idx].name.as_str())
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

    // ProcessItems(items []Item) should produce ParamType ref to Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessItems" && s.kind == "function")
        .expect("ProcessItems should exist");

    let has_item_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_item_param,
        "ProcessItems should accept Item (from []Item), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_param_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessConfig(config *Config) should produce ParamType ref to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessConfig" && s.kind == "function")
        .expect("ProcessConfig should exist");

    let has_config_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config_param,
        "ProcessConfig should accept Config (from *Config), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_param_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessDirect(config Config) should produce ParamType ref to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessDirect" && s.kind == "function")
        .expect("ProcessDirect should exist");

    let has_config_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config_param,
        "ProcessDirect should accept Config, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_param_type_multiple() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMultiple(items []Item, config *Config, cache Cache)
    // should produce ParamType refs to Item, Config, and Cache
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessMultiple" && s.kind == "function")
        .expect("ProcessMultiple should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Item"),
        "ProcessMultiple should accept Item, got: {:?}",
        param_refs
    );
    assert!(
        param_refs.contains(&"Config"),
        "ProcessMultiple should accept Config, got: {:?}",
        param_refs
    );
    assert!(
        param_refs.contains(&"Cache"),
        "ProcessMultiple should accept Cache, got: {:?}",
        param_refs
    );
}

#[test]
fn test_go_param_type_map_value() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMap(data map[string]Config) should extract Config as param type
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessMap" && s.kind == "function")
        .expect("ProcessMap should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Config"),
        "ProcessMap should accept Config (map value), got: {:?}",
        param_refs
    );
}

#[test]
fn test_go_param_type_map_key() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMapKey(data map[Item]string) should extract Item as param type
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessMapKey" && s.kind == "function")
        .expect("ProcessMapKey should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Item"),
        "ProcessMapKey should accept Item (map key), got: {:?}",
        param_refs
    );
}

#[test]
fn test_go_param_type_ptr_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessPtrQualified(req *http.Request) should extract Request as param type
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessPtrQualified" && s.kind == "function")
        .expect("ProcessPtrQualified should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Request"),
        "ProcessPtrQualified should accept Request (*http.Request), got: {:?}",
        param_refs
    );
}

#[test]
fn test_go_param_type_filters_builtins() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Built-in types (string, int, error, bool, etc.) should NOT produce type refs
    let builtin_refs: Vec<_> = parsed
        .type_refs
        .iter()
        .filter(|tr| {
            tr.ref_kind == ReferenceType::ParamType
                && matches!(
                    tr.type_name.as_str(),
                    "string" | "int" | "int64" | "bool" | "error" | "byte" | "rune" | "any"
                )
        })
        .collect();

    assert!(
        builtin_refs.is_empty(),
        "Built-in types should be filtered out, but found: {:?}",
        builtin_refs
            .iter()
            .map(|tr| &tr.type_name)
            .collect::<Vec<_>>()
    );
}

// --- Return Type References ---

#[test]
fn test_go_return_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // NewConfig() *Config should produce ReturnType ref to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "NewConfig" && s.kind == "function")
        .expect("NewConfig should exist");

    let has_config_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_config_return,
        "NewConfig should return Config, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_return_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetCache() Cache should produce ReturnType ref to Cache
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetCache" && s.kind == "function")
        .expect("GetCache should exist");

    let has_cache_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Cache"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_cache_return,
        "GetCache should return Cache, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_return_type_tuple() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // LoadConfigAndError(path string) (*Config, error)
    // should produce ReturnType ref to Config but NOT error
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "LoadConfigAndError" && s.kind == "function")
        .expect("LoadConfigAndError should exist");

    let return_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ReturnType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        return_refs.contains(&"Config"),
        "LoadConfigAndError should return Config, got: {:?}",
        return_refs
    );
    assert!(
        !return_refs.contains(&"error"),
        "LoadConfigAndError should NOT have error in return refs (it's a builtin), got: {:?}",
        return_refs
    );
}

#[test]
fn test_go_return_type_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetItems() []Item should produce ReturnType ref to Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetItems" && s.kind == "function")
        .expect("GetItems should exist");

    let has_item_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_item_return,
        "GetItems should return Item (from []Item), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_return_type_tuple_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetItemsAndError() ([]Item, error) should produce ReturnType ref to Item, NOT error
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetItemsAndError" && s.kind == "function")
        .expect("GetItemsAndError should exist");

    let return_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ReturnType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        return_refs.contains(&"Item"),
        "GetItemsAndError should return Item (from []Item), got: {:?}",
        return_refs
    );
    assert!(
        !return_refs.contains(&"error"),
        "GetItemsAndError should NOT have error in return refs (it's a builtin), got: {:?}",
        return_refs
    );
}

#[test]
fn test_go_return_type_ptr_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetPtrQualified() *http.Handler should produce ReturnType ref to Handler
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetPtrQualified" && s.kind == "function")
        .expect("GetPtrQualified should exist");

    let return_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ReturnType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        return_refs.contains(&"Handler"),
        "GetPtrQualified should return Handler (*http.Handler), got: {:?}",
        return_refs
    );
}

#[test]
fn test_go_return_type_multiple_user_types() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetMultipleTypes() (Cache, Handler, error) should produce ReturnType refs to Cache AND Handler, NOT error
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetMultipleTypes" && s.kind == "function")
        .expect("GetMultipleTypes should exist");

    let return_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ReturnType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        return_refs.contains(&"Cache"),
        "GetMultipleTypes should return Cache, got: {:?}",
        return_refs
    );
    assert!(
        return_refs.contains(&"Handler"),
        "GetMultipleTypes should return Handler, got: {:?}",
        return_refs
    );
    assert!(
        !return_refs.contains(&"error"),
        "GetMultipleTypes should NOT have error in return refs (it's a builtin), got: {:?}",
        return_refs
    );
}

// --- Method Parameter and Return Type References ---

#[test]
fn test_go_method_param_and_return() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // func (c *Config) Process(req Request) Response
    // should produce ParamType ref to Request AND ReturnType ref to Response
    let method_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Process" && s.kind == "function")
        .expect("Process method should exist");

    let has_request_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Request"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_request_param,
        "Process method should accept Request, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .collect::<Vec<_>>()
    );

    let has_response_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Response"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_response_return,
        "Process method should return Response, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_method_return_interface() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // func (c *Config) GetHandler() Handler
    let method_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetHandler" && s.kind == "function")
        .expect("GetHandler method should exist");

    let has_handler_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == method_idx
            && tr.type_name == "Handler"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(
        has_handler_return,
        "GetHandler should return Handler, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == method_idx)
            .collect::<Vec<_>>()
    );
}

// --- Field Type References ---

#[test]
fn test_go_field_type_direct() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Handler Handler (direct type)
    let field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Handler" && s.kind == "field")
        .expect("Handler field should exist");

    let has_handler_ref = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == field_idx
            && tr.type_name == "Handler"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_handler_ref,
        "Handler field should have FieldType ref to Handler, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == field_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_field_type_pointer() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Cache *Cache (pointer type)
    // Find the Cache field (not the Cache struct)
    let field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Cache" && s.kind == "field")
        .expect("Cache field should exist");

    let has_cache_ref = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == field_idx
            && tr.type_name == "Cache"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_cache_ref,
        "Cache field should have FieldType ref to Cache, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == field_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_field_type_slice() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Items []Item (slice type)
    let field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Items" && s.kind == "field")
        .expect("Items field should exist");

    let has_item_ref = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == field_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_item_ref,
        "Items field should have FieldType ref to Item, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == field_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_field_type_map_value() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Meta map[string]Value (map value type)
    let field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Meta" && s.kind == "field")
        .expect("Meta field should exist");

    let has_value_ref = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == field_idx
            && tr.type_name == "Value"
            && tr.ref_kind == ReferenceType::FieldType
    });
    assert!(
        has_value_ref,
        "Meta field should have FieldType ref to Value, got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == field_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_field_type_filters_string() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Config.Name string should NOT produce a FieldType ref
    let name_field_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "Name" && s.kind == "field");

    if let Some(idx) = name_field_idx {
        let has_string_ref = parsed.type_refs.iter().any(|tr| {
            tr.from_symbol_idx == idx
                && tr.type_name == "string"
                && tr.ref_kind == ReferenceType::FieldType
        });
        assert!(
            !has_string_ref,
            "Name field should NOT have FieldType ref to string (builtin)"
        );
    }
}

// --- Variadic Parameter Types ---

#[test]
fn test_go_variadic_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessMany(items ...Item) should produce ParamType ref to Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessMany" && s.kind == "function")
        .expect("ProcessMany should exist");

    let has_item_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_item_param,
        "ProcessMany should accept Item (from ...Item), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

// --- Interface Method Signatures ---

#[test]
fn test_go_interface_method_types() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // Processor.Process(req Request) Response
    // Find the Process method inside Processor interface
    let process_idx = parsed.symbols.iter().position(|s| {
        s.name == "Process"
            && s.kind == "function"
            && parsed.containments.iter().any(|c| {
                c.parent_name == "Processor"
                    && c.child_symbol_idx
                        == parsed
                            .symbols
                            .iter()
                            .position(|x| std::ptr::eq(x, s))
                            .unwrap_or(usize::MAX)
            })
    });

    // Ensure we actually found the method
    assert!(
        process_idx.is_some(),
        "Should find Process method in Processor interface"
    );

    let idx = process_idx.unwrap();
    let has_request_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == idx
            && tr.type_name == "Request"
            && tr.ref_kind == ReferenceType::ParamType
    });
    let has_response_return = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == idx
            && tr.type_name == "Response"
            && tr.ref_kind == ReferenceType::ReturnType
    });
    assert!(has_request_param, "Processor.Process should accept Request");
    assert!(
        has_response_return,
        "Processor.Process should return Response"
    );
}

// --- Generic Types (Go 1.18+) ---

#[test]
fn test_go_generic_param_inner_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ProcessGeneric(c Container[Item]) should produce ParamType refs to:
    // - Container (the outer type)
    // - Item (the inner type argument)
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ProcessGeneric" && s.kind == "function")
        .expect("ProcessGeneric should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Item"),
        "ProcessGeneric should accept Item (from Container[Item]), got: {:?}",
        param_refs
    );
}

#[test]
fn test_go_generic_return_inner_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // GetContainer() Container[Config] should produce ReturnType ref to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "GetContainer" && s.kind == "function")
        .expect("GetContainer should exist");

    let return_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::ReturnType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        return_refs.contains(&"Config"),
        "GetContainer should return Config (from Container[Config]), got: {:?}",
        return_refs
    );
}

// --- Channel Types ---

#[test]
fn test_go_channel_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // SendItems(ch chan Item) should produce ParamType ref to Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "SendItems" && s.kind == "function")
        .expect("SendItems should exist");

    let has_item_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Item"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_item_param,
        "SendItems should accept Item (from chan Item), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_go_receive_channel_param_type() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // ReceiveConfigs(ch <-chan Config) should produce ParamType ref to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "ReceiveConfigs" && s.kind == "function")
        .expect("ReceiveConfigs should exist");

    let has_config_param = parsed.type_refs.iter().any(|tr| {
        tr.from_symbol_idx == fn_idx
            && tr.type_name == "Config"
            && tr.ref_kind == ReferenceType::ParamType
    });
    assert!(
        has_config_param,
        "ReceiveConfigs should accept Config (from <-chan Config), got type_refs: {:?}",
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.from_symbol_idx == fn_idx)
            .collect::<Vec<_>>()
    );
}

// --- Summary count test for verification ---

#[test]
fn test_go_type_refs_summary() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    let param_count = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.ref_kind == ReferenceType::ParamType)
        .count();
    let return_count = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.ref_kind == ReferenceType::ReturnType)
        .count();
    let field_count = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.ref_kind == ReferenceType::FieldType)
        .count();

    // These are minimum counts based on types.go content
    // Exact counts will be verified once implementation is complete
    assert!(
        param_count >= 15,
        "Expected at least 15 ParamType refs, got {}. All type_refs: {:?}",
        param_count,
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.ref_kind == ReferenceType::ParamType)
            .map(|tr| &tr.type_name)
            .collect::<Vec<_>>()
    );
    assert!(
        return_count >= 8,
        "Expected at least 8 ReturnType refs, got {}. All type_refs: {:?}",
        return_count,
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.ref_kind == ReferenceType::ReturnType)
            .map(|tr| &tr.type_name)
            .collect::<Vec<_>>()
    );
    assert!(
        field_count >= 5,
        "Expected at least 5 FieldType refs, got {}. All type_refs: {:?}",
        field_count,
        parsed
            .type_refs
            .iter()
            .filter(|tr| tr.ref_kind == ReferenceType::FieldType)
            .map(|tr| &tr.type_name)
            .collect::<Vec<_>>()
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

    // RegisterHook(myHookHandler) should create a call to myHookHandler
    let call_to_hook = parsed
        .calls
        .iter()
        .find(|c| c.callee_name == "myHookHandler");

    assert!(
        call_to_hook.is_some(),
        "myHookHandler passed as argument should create a call edge"
    );
}

#[test]
fn test_go_func_ref_multiple() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // OnInit(setupConfig) should create a call to setupConfig
    let call_to_setup = parsed.calls.iter().find(|c| c.callee_name == "setupConfig");

    assert!(
        call_to_setup.is_some(),
        "setupConfig passed as argument should create a call edge"
    );
}

#[test]
fn test_go_func_ref_qualified() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // CallWithQualifiedFuncRef calls: http.HandleFunc("/notfound", http.NotFound)
    // This should create a call edge to NotFound with qualifier "http"
    let call_to_notfound = parsed
        .calls
        .iter()
        .find(|c| c.callee_name == "NotFound" && c.qualifier.as_deref() == Some("http"));

    assert!(
        call_to_notfound.is_some(),
        "http.NotFound passed as argument should create a scoped call edge, got: {:?}",
        parsed
            .calls
            .iter()
            .map(|c| (&c.callee_name, &c.qualifier))
            .collect::<Vec<_>>()
    );

    assert_eq!(
        call_to_notfound.unwrap().call_form,
        CallForm::Scoped,
        "Qualified func ref should have Scoped call form"
    );
}

// --- Type Assertions (Usage edges) ---

#[test]
fn test_go_type_assertion_uses_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseTypeAssertion does: v, ok := x.(*Config)
    // This should create a Usage edge from UseTypeAssertion to Config
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "UseTypeAssertion" && s.kind == "function")
        .expect("UseTypeAssertion should exist");

    let usage_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::Usage)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        usage_refs.contains(&"Config"),
        "UseTypeAssertion should have Usage edge to Config from type assertion x.(*Config), got: {:?}",
        usage_refs
    );
}

#[test]
fn test_go_composite_literal_uses_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseCompositeLiteral does: Config{}, &Config{}, []Item{}
    // This should create Usage edges to Config and Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "UseCompositeLiteral" && s.kind == "function")
        .expect("UseCompositeLiteral should exist");

    let usage_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::Usage)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        usage_refs.contains(&"Config"),
        "UseCompositeLiteral should have Usage edge to Config from Config{{}}, got: {:?}",
        usage_refs
    );
    assert!(
        usage_refs.contains(&"Item"),
        "UseCompositeLiteral should have Usage edge to Item from []Item{{}}, got: {:?}",
        usage_refs
    );
}

#[test]
fn test_go_var_declaration_type_annotation_edge() {
    let code = load_testdata("types.go");
    let parsed = Go::extract(&code, "testdata/types.go");

    // UseVarDeclaration does: var cfg Config, var cfgPtr *Config, var items []Item
    // This should create TypeAnnotation edges to Config and Item
    let fn_idx = parsed
        .symbols
        .iter()
        .position(|s| s.name == "UseVarDeclaration" && s.kind == "function")
        .expect("UseVarDeclaration should exist");

    let type_annot_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == fn_idx && tr.ref_kind == ReferenceType::TypeAnnotation)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        type_annot_refs.contains(&"Config"),
        "UseVarDeclaration should have TypeAnnotation edge to Config from var cfg Config, got: {:?}",
        type_annot_refs
    );
    assert!(
        type_annot_refs.contains(&"Item"),
        "UseVarDeclaration should have TypeAnnotation edge to Item from var items []Item, got: {:?}",
        type_annot_refs
    );
}

// --- Method Receiver Types (Accepts edges) ---

#[test]
fn test_go_method_receiver_accepts_edge() {
    let code = load_testdata("cache.go");
    let parsed = Go::extract(&code, "cache/cache.go");

    // func (c *Cache) Get(key string) should create Accepts edge from Get to Cache
    // Note: There's also a Get in the Cacher interface, so we need to find the one
    // that has an Accepts edge to Cache (the method implementation, not the interface method)
    let get_idx = parsed
        .symbols
        .iter()
        .enumerate()
        .find(|(idx, s)| {
            s.name == "Get"
                && s.kind == "function"
                && parsed.type_refs.iter().any(|tr| {
                    tr.from_symbol_idx == *idx
                        && tr.type_name == "Cache"
                        && tr.ref_kind == ReferenceType::ParamType
                })
        })
        .map(|(idx, _)| idx)
        .expect("Get method with Cache receiver should exist");

    let param_refs: Vec<&str> = parsed
        .type_refs
        .iter()
        .filter(|tr| tr.from_symbol_idx == get_idx && tr.ref_kind == ReferenceType::ParamType)
        .map(|tr| tr.type_name.as_str())
        .collect();

    assert!(
        param_refs.contains(&"Cache"),
        "Get method should have Accepts edge to Cache (receiver type), got: {:?}",
        param_refs
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
