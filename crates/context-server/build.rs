//! Creates a stub `dist/index.html` when the `embed-frontend` feature is on.
//!
//! The `Embed` derive panics when the folder is absent, so a fresh clone
//! cannot compile `context-server` with `embed-frontend` enabled. Cargo
//! feature unification enables the feature for every workspace command that
//! includes `context-app`. The stub keeps those builds working; a real Trunk
//! build replaces it, and an existing `index.html` is never touched.

use std::env;
use std::fs;
use std::io;
use std::path::Path;

const STUB_INDEX_HTML: &str = concat!(
    "<!doctype html>\n",
    "<html lang=\"en\">\n",
    "<head><meta charset=\"utf-8\"><title>Context</title></head>\n",
    "<body>\n",
    "<p>The frontend assets are not embedded in this build.</p>\n",
    "<p>Run <code>trunk build --release</code> to generate them.</p>\n",
    "</body>\n",
    "</html>\n",
);

fn main() -> io::Result<()> {
    if env::var_os("CARGO_FEATURE_EMBED_FRONTEND").is_none() {
        return Ok(());
    }

    // `dist/` is gitignored, so cargo's default package scan never sees it and a
    // warm-tree deletion would not rerun this script. The explicit path is
    // watched directly; a missing watched path counts as changed.
    println!("cargo:rerun-if-changed=dist");

    ensure_stub_index(Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn ensure_stub_index(manifest_dir: &Path) -> io::Result<()> {
    let dist_dir = manifest_dir.join("dist");
    let index_html = dist_dir.join("index.html");

    if index_html.exists() {
        return Ok(());
    }

    fs::create_dir_all(&dist_dir)?;
    fs::write(index_html, STUB_INDEX_HTML)
}
