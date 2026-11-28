//! Install the Chromium build the bundled Playwright driver expects.
//!
//! One-time prerequisite for the browser harness:
//!
//! ```text
//! cargo run --manifest-path crates/context-frontend/Cargo.toml \
//!   --features e2e --example install-browsers
//! ```
//!
//! Chromium is the default selection. Name other browsers to override it
//! (`... --example install-browsers -- firefox`). Pass `--with-deps` on Linux
//! to also install the system libraries the browsers need.

use playwright_rs::{install_browsers, install_browsers_with_deps};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let with_deps = args.iter().any(|arg| arg == "--with-deps");
    let browsers: Vec<&str> = args
        .iter()
        .filter(|arg| arg.as_str() != "--with-deps")
        .map(String::as_str)
        .collect();

    let selection: Vec<&str> = if browsers.is_empty() {
        vec!["chromium"]
    } else {
        browsers
    };

    if with_deps {
        install_browsers_with_deps(Some(&selection)).await?;
    } else {
        install_browsers(Some(&selection)).await?;
    }

    println!("installed: {selection:?}");
    Ok(())
}
