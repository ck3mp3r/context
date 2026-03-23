// Rust language support for code analysis

mod extractor;

#[cfg(test)]
mod extractor_test;

pub use extractor::RustExtractor;
