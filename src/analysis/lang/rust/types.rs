// Rust symbol types

use crate::analysis::types::Kind as GenericKind;

/// Rust-specific symbol types
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Kind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
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
            Self::Impl => "impl",
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
            "impl" => Ok(Self::Impl),
            "mod" => Ok(Self::Mod),
            "const" => Ok(Self::Const),
            "static" => Ok(Self::Static),
            "type" => Ok(Self::Type),
            _ => Err(format!("Unknown Rust symbol type: {}", s)),
        }
    }
}

impl From<Kind> for GenericKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Function => GenericKind::Function,
            Kind::Struct => GenericKind::Struct,
            Kind::Enum => GenericKind::Enum,
            Kind::Trait => GenericKind::Trait,
            Kind::Impl => GenericKind::Impl {
                target_type: String::new(),
            },
            Kind::Mod => GenericKind::Variable, // Map to closest generic type
            Kind::Const => GenericKind::Constant,
            Kind::Static => GenericKind::Variable,
            Kind::Type => GenericKind::Struct, // Type alias maps to struct
        }
    }
}
