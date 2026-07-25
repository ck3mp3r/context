-- Find functions that use a specific type (composite literals, type assertions)
-- Use to find all functions that instantiate or assert a type
-- Parameter: $name (String) - type name to search for
-- Examples:
--   $name = "Request" → functions using Request type
--   Use for: finding type consumers, usage patterns
SELECT
    in.name AS name,
    in.kind AS kind,
    in.file_path AS file_path,
    in.start_line AS start_line,
    out.name AS type_name
FROM uses
WHERE repo_id = $repo_id AND out.name = $name;
