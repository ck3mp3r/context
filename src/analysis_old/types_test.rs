use crate::analysis::types::*;

// ---- RawSymbol ----

#[test]
fn test_raw_symbol_id_generation() {
    let sym = RawSymbol {
        name: "foo".to_string(),
        kind: "function".to_string(),
        file_path: "src/main.rs".to_string(),
        start_line: 10,
        end_line: 20,
        signature: Some("fn foo() -> i32".to_string()),
        language: "rust".to_string(),
    };
    assert_eq!(sym.symbol_id().as_str(), "symbol:src/main.rs:foo:10");
}

// ---- CallForm ----

#[test]
fn test_call_form_variants() {
    assert_eq!(CallForm::Free, CallForm::Free);
    assert_ne!(CallForm::Free, CallForm::Method);
    assert_ne!(CallForm::Method, CallForm::Scoped);
}

// ---- RawCall ----

#[test]
fn test_raw_call_free() {
    let call = RawCall {
        file_path: "src/main.rs".to_string(),
        call_site_line: 5,
        callee_name: "println".to_string(),
        call_form: CallForm::Free,
        receiver: None,
        qualifier: None,
        enclosing_symbol_idx: Some(0),
    };
    assert_eq!(call.callee_name, "println");
    assert_eq!(call.call_form, CallForm::Free);
    assert!(call.receiver.is_none());
}

#[test]
fn test_raw_call_method() {
    let call = RawCall {
        file_path: "src/main.rs".to_string(),
        call_site_line: 10,
        callee_name: "push".to_string(),
        call_form: CallForm::Method,
        receiver: Some("self.items".to_string()),
        qualifier: None,
        enclosing_symbol_idx: Some(1),
    };
    assert_eq!(call.call_form, CallForm::Method);
    assert_eq!(call.receiver.as_deref(), Some("self.items"));
}

#[test]
fn test_raw_call_scoped() {
    let call = RawCall {
        file_path: "src/main.rs".to_string(),
        call_site_line: 15,
        callee_name: "new".to_string(),
        call_form: CallForm::Scoped,
        receiver: None,
        qualifier: Some("HashMap".to_string()),
        enclosing_symbol_idx: Some(0),
    };
    assert_eq!(call.call_form, CallForm::Scoped);
    assert_eq!(call.qualifier.as_deref(), Some("HashMap"));
}

// ---- ParsedFile ----

#[test]
fn test_parsed_file_new() {
    let pf = ParsedFile::new("src/main.rs", "rust");
    assert_eq!(pf.file_path, "src/main.rs");
    assert_eq!(pf.language, "rust");
    assert!(pf.symbols.is_empty());
    assert!(pf.calls.is_empty());
    assert!(pf.imports.is_empty());
    assert!(pf.heritage.is_empty());
    assert!(pf.containments.is_empty());
    assert!(pf.type_refs.is_empty());
}

#[test]
fn test_parsed_file_collects_symbols_and_calls() {
    let mut pf = ParsedFile::new("src/lib.rs", "rust");
    pf.symbols.push(RawSymbol {
        name: "process".to_string(),
        kind: "function".to_string(),
        file_path: "src/lib.rs".to_string(),
        start_line: 1,
        end_line: 10,
        signature: None,
        language: "rust".to_string(),
    });
    pf.calls.push(RawCall {
        file_path: "src/lib.rs".to_string(),
        call_site_line: 5,
        callee_name: "helper".to_string(),
        call_form: CallForm::Free,
        receiver: None,
        qualifier: None,
        enclosing_symbol_idx: Some(0),
    });
    assert_eq!(pf.symbols.len(), 1);
    assert_eq!(pf.calls.len(), 1);
    assert_eq!(pf.calls[0].enclosing_symbol_idx, Some(0));
}

// ---- RawHeritage ----

#[test]
fn test_raw_heritage() {
    let h = RawHeritage {
        file_path: "src/lib.rs".to_string(),
        type_name: "MyStruct".to_string(),
        parent_name: "Display".to_string(),
        kind: InheritanceType::Implements,
    };
    assert_eq!(h.type_name, "MyStruct");
    assert_eq!(h.parent_name, "Display");
    assert_eq!(h.kind, InheritanceType::Implements);
}

// ---- RawContainment ----

#[test]
fn test_raw_containment() {
    let c = RawContainment {
        file_path: "src/lib.rs".to_string(),
        parent_name: "MyStruct".to_string(),
        child_symbol_idx: 2,
    };
    assert_eq!(c.parent_name, "MyStruct");
    assert_eq!(c.child_symbol_idx, 2);
}

// ---- RawTypeRef ----

#[test]
fn test_raw_type_ref() {
    let tr = RawTypeRef {
        file_path: "src/lib.rs".to_string(),
        from_symbol_idx: 0,
        type_name: "Config".to_string(),
        ref_kind: ReferenceType::FieldType,
    };
    assert_eq!(tr.type_name, "Config");
    assert_eq!(tr.ref_kind, ReferenceType::FieldType);
}

// ---- RawImport ----

#[test]
fn test_raw_import() {
    let imp = RawImport {
        file_path: "src/lib.rs".to_string(),
        entry: ImportEntry::named_import("std::collections", vec!["HashMap".to_string()]),
    };
    assert_eq!(imp.entry.module_path, "std::collections");
    assert_eq!(imp.entry.imported_names, vec!["HashMap"]);
}
