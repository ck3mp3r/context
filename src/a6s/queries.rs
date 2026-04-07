//! Bundled NanoGraph queries for a6s analysis
//!
//! Query definitions live in `src/a6s/queries/*.gq` as standalone files.
//! They are embedded at compile time and installed to every analyzed repo's
//! `queries/` directory.

use std::path::Path;

pub const CALLS_EDGES: &str = include_str!("queries/calls_edges.gq");
pub const FILE_IMPORTS: &str = include_str!("queries/file_imports.gq");
pub const ALL_SYMBOLS: &str = include_str!("queries/all_symbols.gq");

const BUNDLED_QUERIES: &[(&str, &str)] = &[
    ("calls_edges", CALLS_EDGES),
    ("file_imports", FILE_IMPORTS),
    ("all_symbols", ALL_SYMBOLS),
];

pub fn install_bundled_queries(repo_path: &Path) -> Result<(), std::io::Error> {
    let queries_dir = repo_path.join("queries");
    std::fs::create_dir_all(&queries_dir)?;

    for (name, content) in BUNDLED_QUERIES {
        std::fs::write(queries_dir.join(format!("{}.gq", name)), content)?;
    }

    tracing::info!("Installed {} bundled queries", BUNDLED_QUERIES.len());
    Ok(())
}
