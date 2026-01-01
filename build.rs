//! Build script for embedding frontend assets.
//!
//! In release mode, this automatically builds the frontend with Trunk
//! and prepares the dist/ directory for rust-embed to include in the binary.
//!
//! In debug mode, this script does nothing (frontend assets are read from
//! filesystem at runtime by rust-embed).

fn main() {
    // Only build frontend assets in release mode
    // In debug mode, rust-embed reads from filesystem (no build needed)
    #[cfg(not(debug_assertions))]
    {
        use std::process::Command;

        println!("cargo:rerun-if-changed=src/frontend");
        println!("cargo:rerun-if-changed=Trunk.toml");
        println!("cargo:rerun-if-changed=src/frontend/assets");

        println!("cargo:warning=Building frontend with Trunk...");

        let status = Command::new("trunk")
            .args(["build", "--release", "--dist", "dist"])
            .env("CARGO_TARGET_DIR", "target/trunk")
            .status()
            .expect("Failed to execute trunk command. Is trunk installed?");

        if !status.success() {
            panic!(
                "Trunk build failed with exit code: {:?}. \
                 Ensure trunk is installed and the frontend builds successfully.",
                status.code()
            );
        }

        println!("cargo:warning=Frontend build completed successfully");
    }

    // Debug builds - no frontend build needed
    #[cfg(debug_assertions)]
    {
        println!(
            "cargo:warning=Debug build: Skipping frontend build (rust-embed will read from dist/ at runtime)"
        );
    }
}
