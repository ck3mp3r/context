use super::extractor::RustExtractor;
use crate::a6s::extract::LanguageExtractor;

#[test]
fn test_rust_extractor_language() {
    let extractor = RustExtractor;
    assert_eq!(extractor.language(), "rust");
}

#[test]
fn test_rust_extractor_extensions() {
    let extractor = RustExtractor;
    assert_eq!(extractor.extensions(), &["rs"]);
}

#[test]
fn test_rust_extractor_extract_returns_empty() {
    let extractor = RustExtractor;
    let code = "fn main() {}";
    let parsed = extractor.extract(code, "src/main.rs");

    assert_eq!(parsed.file_path, "src/main.rs");
    assert_eq!(parsed.language, "rust");
    assert_eq!(parsed.symbols.len(), 0);
    assert_eq!(parsed.edges.len(), 0);
    assert_eq!(parsed.imports.len(), 0);
}

#[test]
fn test_rust_extractor_queries_nonempty() {
    let extractor = RustExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    assert!(!extractor.type_ref_queries().is_empty());
}
