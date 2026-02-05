#[cfg(feature = "backend")]
pub mod api;

#[cfg(feature = "backend")]
pub mod cli;

#[cfg(feature = "backend")]
pub mod db;

#[cfg(feature = "backend")]
pub mod mcp;

#[cfg(feature = "backend")]
pub mod skills;

#[cfg(feature = "backend")]
pub mod sync;

#[cfg(feature = "backend")]
pub mod serde_utils;

#[cfg(feature = "backend")]
pub fn init() {
    // Install ring as the default crypto provider for rustls
    // This must be called before any reqwest Client is created
    let _ = rustls::crypto::ring::default_provider().install_default();
}
