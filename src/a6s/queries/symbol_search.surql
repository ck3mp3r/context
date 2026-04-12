-- Find symbols by name across the entire codebase
-- Use for quick lookup when you know a symbol name but not its location
-- Parameter: $name (String) - symbol name to search for
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line,
    signature
FROM symbol
WHERE repo_id = $repo_id AND name = $name;
