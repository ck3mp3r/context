-- Module and package hierarchy
-- Use to understand codebase organization. Modules with many symbols are key areas
-- Supports Rust (module), Go (package), and Kotlin (package)
-- Examples:
--   code_query(query_name="module_map") → package/module hierarchy
--   Use for: understanding code organization
SELECT
    out.name AS module_name,
    out.file_path AS file_path,
    out.visibility AS visibility
FROM file_contains
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id AND out.kind IN ["module", "package"]
FETCH out;
