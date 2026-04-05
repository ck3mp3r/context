//! Language-specific code analysis implementations.
//!
//! Each language module implements the `LanguageAnalyser` trait.
//! Use `Analyser::for_language()` or `Analyser::for_extension()` to get an analyser.

#[cfg(feature = "backend")]
pub mod rust;

#[cfg(feature = "backend")]
pub mod golang;

#[cfg(feature = "backend")]
pub mod nushell;

#[cfg(feature = "backend")]
mod analyser;

#[cfg(feature = "backend")]
pub use analyser::{Analyser, LanguageAnalyser, supported_extensions};
