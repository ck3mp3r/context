pub mod cli;

pub fn init() {
    // Install ring as the default crypto provider for rustls
    // This must be called before any reqwest Client is created
    let _ = rustls::crypto::ring::default_provider().install_default();
}
