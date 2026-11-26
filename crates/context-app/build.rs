use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rustc-env=CONTEXT_BUILD_ID={}", build_id());
    tauri_build::build()
}

/// Short git sha of the checkout, or an empty string when the build has no
/// checkout.
///
/// A Nix build or an unpacked archive has no `.git`, so a missing id is not
/// fatal: the id is embedded but no longer read, semver precedence decides the
/// offer.
fn build_id() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}
