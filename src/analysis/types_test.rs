// Tests for types module

use super::*;

#[test]
fn test_symbol_kind_as_str() {
    assert_eq!(SymbolKind::Function.as_str(), "function");
    assert_eq!(SymbolKind::Class.as_str(), "class");
    assert_eq!(SymbolKind::Struct.as_str(), "struct");
    assert_eq!(SymbolKind::Trait.as_str(), "trait");
    assert_eq!(SymbolKind::Enum.as_str(), "enum");
    assert_eq!(SymbolKind::Constant.as_str(), "constant");
    assert_eq!(SymbolKind::Variable.as_str(), "variable");
}

#[test]
fn test_symbol_kind_impl() {
    let impl_kind = SymbolKind::Impl {
        target_type: "MyStruct".to_string(),
    };
    assert_eq!(impl_kind.as_str(), "impl");
}
