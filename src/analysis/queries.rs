//! Bundled NanoGraph queries for code analysis
//!
//! Query definitions live in `src/analysis/queries/*.gq` as standalone files.
//! They are embedded at compile time and installed to every analyzed repo's
//! `queries/` directory so they're available via `c5t_code_query(query_name="...")`.

use std::path::Path;

const BUNDLED_QUERIES: &[(&str, &str)] = &[
    ("overview", include_str!("queries/overview.gq")),
    ("public_api", include_str!("queries/public_api.gq")),
    ("module_map", include_str!("queries/module_map.gq")),
    ("hub_symbols", include_str!("queries/hub_symbols.gq")),
    ("callers", include_str!("queries/callers.gq")),
    ("callees", include_str!("queries/callees.gq")),
    ("type_hierarchy", include_str!("queries/type_hierarchy.gq")),
    ("file_symbols", include_str!("queries/file_symbols.gq")),
    ("symbol_search", include_str!("queries/symbol_search.gq")),
    ("entry_points", include_str!("queries/entry_points.gq")),
    ("uses_type", include_str!("queries/uses_type.gq")),
    ("annotates_type", include_str!("queries/annotates_type.gq")),
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
