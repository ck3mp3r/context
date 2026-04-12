-- Find all functions called by a given function
-- Use to understand what a function depends on
-- Parameter: $name (String) - source function name
SELECT
    out.name AS name,
    out.kind AS kind,
    out.file_path AS file_path,
    out.start_line AS start_line
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id AND in.name = $name
FETCH out;
