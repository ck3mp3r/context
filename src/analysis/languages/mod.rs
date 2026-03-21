// Language-specific extractors

#[cfg(feature = "backend")]
pub mod rust;

// Tests
#[cfg(all(test, feature = "backend"))]
mod rust_test;
