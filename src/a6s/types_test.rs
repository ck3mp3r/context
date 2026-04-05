// Tests for a6s/types.rs

use super::types::*;

#[test]
fn test_symbol_ref_resolved() {
    let id = SymbolId::new("src/main.rs", "main", 1);
    let sym_ref = SymbolRef::resolved(id.clone());

    assert!(sym_ref.is_resolved());
    assert_eq!(sym_ref, SymbolRef::Resolved(id));
}

#[test]
fn test_symbol_ref_unresolved() {
    let sym_ref = SymbolRef::unresolved("Foo", "src/main.rs");

    assert!(!sym_ref.is_resolved());
    match sym_ref {
        SymbolRef::Unresolved { name, file_path } => {
            assert_eq!(name, "Foo");
            assert_eq!(file_path, "src/main.rs");
        }
        _ => panic!("Expected Unresolved variant"),
    }
}

#[test]
fn test_raw_edge_with_symbol_ref() {
    let from_id = SymbolId::new("src/main.rs", "main", 1);
    let to_unresolved = SymbolRef::unresolved("println", "src/main.rs");

    let edge = RawEdge {
        from: SymbolRef::Resolved(from_id),
        to: to_unresolved.clone(),
        kind: EdgeKind::Calls,
        line: Some(5),
    };

    assert!(edge.from.is_resolved());
    assert!(!edge.to.is_resolved());
    assert_eq!(edge.line, Some(5));
}

#[test]
fn test_resolved_edge_construction() {
    let from = SymbolId::new("src/main.rs", "main", 1);
    let to = SymbolId::new("src/lib.rs", "helper", 10);

    let edge = ResolvedEdge {
        from: from.clone(),
        to: to.clone(),
        kind: EdgeKind::Calls,
        line: Some(5),
    };

    assert_eq!(edge.from, from);
    assert_eq!(edge.to, to);
    assert_eq!(edge.line, Some(5));
}

#[test]
fn test_pipeline_progress_variants() {
    let scanned = PipelineProgress::Scanned(10);
    let extracted = PipelineProgress::Extracted(10);
    let resolved = PipelineProgress::Resolved(ResolveStats {
        symbols_registered: 100,
        edges_resolved: 50,
        edges_dropped: 5,
        imports_resolved: 20,
    });
    let loaded = PipelineProgress::Loaded;

    // Just verify they all construct
    match scanned {
        PipelineProgress::Scanned(n) => assert_eq!(n, 10),
        _ => panic!("Wrong variant"),
    }
    match extracted {
        PipelineProgress::Extracted(n) => assert_eq!(n, 10),
        _ => panic!("Wrong variant"),
    }
    match resolved {
        PipelineProgress::Resolved(stats) => assert_eq!(stats.symbols_registered, 100),
        _ => panic!("Wrong variant"),
    }
    match loaded {
        PipelineProgress::Loaded => {}
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_resolve_stats_default() {
    let stats = ResolveStats {
        symbols_registered: 0,
        edges_resolved: 0,
        edges_dropped: 0,
        imports_resolved: 0,
    };

    assert_eq!(stats.symbols_registered, 0);
    assert_eq!(stats.edges_resolved, 0);
    assert_eq!(stats.edges_dropped, 0);
    assert_eq!(stats.imports_resolved, 0);
}
