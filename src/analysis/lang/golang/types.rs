/// Go-specific symbol types.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Kind {
    /// `func` top-level function
    Function,
    /// `func (receiver) name()` method with receiver
    Method,
    /// `type X struct { ... }`
    Struct,
    /// `type X interface { ... }`
    Interface,
    /// `type X = ...` or `type X underlying`
    TypeAlias,
    /// `const` declaration
    Const,
    /// `var` declaration
    Var,
    /// `package` declaration
    Package,
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
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::TypeAlias => "type_alias",
            Self::Const => "const",
            Self::Var => "var",
            Self::Package => "package",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            "struct" => Ok(Self::Struct),
            "interface" => Ok(Self::Interface),
            "type_alias" => Ok(Self::TypeAlias),
            "const" => Ok(Self::Const),
            "var" => Ok(Self::Var),
            "package" => Ok(Self::Package),
            _ => Err(format!("Unknown Go symbol type: {}", s)),
        }
    }
}
