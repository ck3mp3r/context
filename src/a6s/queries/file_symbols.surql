-- All symbols defined in a specific file
-- Use to get an overview of a single file's contents
-- Parameter: $path (String) - file path
-- Examples:
--   $path = "src/api/v1/graph.rs" → all symbols in graph.rs
--   Use for: file exploration, understanding module contents
SELECT
    out.name AS name,
    out.kind AS kind,
    out.start_line AS start_line,
    out.end_line AS end_line,
    out.signature AS signature,
    out.entry_type AS entry_type
FROM file_contains
WHERE in.repo_id = $repo_id AND in.path = $path
FETCH out;
