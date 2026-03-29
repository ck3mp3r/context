// Rust symbol types

/// Rust-specific symbol types.
/// Only kinds that actually get inserted as symbol nodes.
/// Impl blocks are NOT symbols - they produce edges (SymbolContains, Inherits)
/// and are handled via ImplInfo + parse_impl().
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Kind {
    Function,
    Struct,
    Enum,
    Trait,
    Mod,
    Const,
    Static,
    Type,
}

impl AsRef<str> for Kind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Kind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Mod => "mod",
            Self::Const => "const",
            Self::Static => "static",
            Self::Type => "type",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "struct" => Ok(Self::Struct),
            "enum" => Ok(Self::Enum),
            "trait" => Ok(Self::Trait),
            "mod" => Ok(Self::Mod),
            "const" => Ok(Self::Const),
            "static" => Ok(Self::Static),
            "type" => Ok(Self::Type),
            _ => Err(format!("Unknown Rust symbol type: {}", s)),
        }
    }
}
