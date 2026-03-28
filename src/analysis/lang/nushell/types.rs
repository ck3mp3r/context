// Nushell symbol types

/// Nushell-specific symbol types.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Kind {
    /// `def`, `def --env`, `def --wrapped`, `export def`
    Command,
    /// `module`
    Module,
    /// `alias`
    Alias,
    /// `extern`
    Extern,
    /// `const`
    Const,
}

impl AsRef<str> for Kind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Kind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Command => "command",
            Self::Module => "module",
            Self::Alias => "alias",
            Self::Extern => "extern",
            Self::Const => "const",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "command" => Ok(Self::Command),
            "module" => Ok(Self::Module),
            "alias" => Ok(Self::Alias),
            "extern" => Ok(Self::Extern),
            "const" => Ok(Self::Const),
            _ => Err(format!("Unknown Nushell symbol type: {}", s)),
        }
    }
}
