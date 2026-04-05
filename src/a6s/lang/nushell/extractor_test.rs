use super::extractor::NushellExtractor;
use crate::a6s::extract::LanguageExtractor;

#[test]
fn test_nushell_extractor_language() {
    let extractor = NushellExtractor;
    assert_eq!(extractor.language(), "nushell");
}

#[test]
fn test_nushell_extractor_extensions() {
    let extractor = NushellExtractor;
    assert_eq!(extractor.extensions(), &["nu"]);
}

#[test]
fn test_nushell_extractor_extract_returns_empty() {
    let extractor = NushellExtractor;
    let code = "def main [] { print \"hello\" }";
    let parsed = extractor.extract(code, "main.nu");

    assert_eq!(parsed.file_path, "main.nu");
    assert_eq!(parsed.language, "nushell");
    assert_eq!(parsed.symbols.len(), 0);
    assert_eq!(parsed.edges.len(), 0);
    assert_eq!(parsed.imports.len(), 0);
}

#[test]
fn test_nushell_extractor_queries() {
    let extractor = NushellExtractor;
    assert!(!extractor.symbol_queries().is_empty());
    // Nushell has no type_refs query
    assert_eq!(extractor.type_ref_queries(), "");
}
