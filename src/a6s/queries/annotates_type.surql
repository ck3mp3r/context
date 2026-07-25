-- Find functions that declare variables with a specific type annotation
-- Use to find all functions that declare variables of a type via var x Type
-- Parameter: $name (String) - type name to search for
-- Examples:
--   $name = "Error" → functions with Error annotations
--   Use for: error handling discovery, type usage
SELECT
    in.name AS name,
    in.kind AS kind,
    in.file_path AS file_path,
    in.start_line AS start_line,
    out.name AS type_name
FROM type_annotation
WHERE repo_id = $repo_id AND out.name = $name;
