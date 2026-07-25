-- List all symbols within a specific module or package
-- Use this to explore code organization and see what's in a module
-- Examples:
--   $module_path = "api::v1" → all symbols in api/v1 module
--   $module_path = "db::sqlite" → all database symbols
--   $module_path = "core" → top-level core module symbols
-- Parameter: $module_path (String) - Module path to explore
--   Format depends on language: :: for Rust, . for others
-- Returns: All symbols in the module, grouped by file
-- Limit: 100 results per module
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line,
    signature,
    module_path
FROM symbol
WHERE repo_id = $repo_id
  AND module_path = $module_path
ORDER BY file_path, start_line
LIMIT 100;
