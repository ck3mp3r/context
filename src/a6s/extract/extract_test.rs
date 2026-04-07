//! Tests for LanguageExtractor trait and dispatch

use super::*;

#[test]
fn test_get_extractor_for_known_extensions() {
    // Extractors are now registered
    assert!(Extractor::for_extension("rs").is_some());
    assert!(Extractor::for_extension("go").is_some());
    assert!(Extractor::for_extension("nu").is_some());
}

#[test]
fn test_get_extractor_for_unknown_extension() {
    assert!(Extractor::for_extension("xyz").is_none());
    assert!(Extractor::for_extension("py").is_none());
}

#[test]
fn test_supported_extensions() {
    let exts = supported_extensions();
    assert_eq!(exts.len(), 3);
    assert!(exts.contains(&"rs"));
    assert!(exts.contains(&"go"));
    assert!(exts.contains(&"nu"));
}

// Compile-time trait checks
fn _assert_trait_is_send_sync() {
    fn _check<T: Send + Sync>() {}
    _check::<Extractor>();
}
