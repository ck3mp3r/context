//! Language-specific extractors

#[cfg(feature = "backend")]
pub mod rust;

#[cfg(feature = "backend")]
pub mod golang;

#[cfg(feature = "backend")]
pub mod nushell;
