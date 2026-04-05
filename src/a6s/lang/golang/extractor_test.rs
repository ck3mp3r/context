use super::extractor::GolangExtractor;
use crate::a6s::extract::LanguageExtractor;

#[test]
fn test_golang_extractor_language() {
    let extractor = GolangExtractor;
    assert_eq!(extractor.language(), "go");
}

#[test]
fn test_golang_extractor_extensions() {
    let extractor = GolangExtractor;
    assert_eq!(extractor.extensions(), &["go"]);
}

#[test]
fn test_golang_extractor_extract_returns_empty() {
    let extractor = GolangExtractor;
    let code = "package main\nfunc main() {}";
    let parsed = extractor.extract(code, "main.go");

    assert_eq!(parsed.file_path, "main.go");
    assert_eq!(parsed.language, "go");
    assert_eq!(parsed.symbols.len(), 0);
    assert_eq!(parsed.edges.len(), 0);
    assert_eq!(parsed.imports.len(), 0);
}

#[test]
fn test_golang_extractor_queries_nonempty() {
    let extractor = GolangExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    assert!(!extractor.type_ref_queries().is_empty());
}
