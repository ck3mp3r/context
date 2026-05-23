-- Find all functions that call a given function
-- Use to trace who depends on a function. Helps assess impact of changes
-- Parameter: $name (String) - target function name
SELECT
    in.name AS name,
    in.kind AS kind,
    in.file_path AS file_path,
    in.start_line AS start_line
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id AND out.name = $name
FETCH in;
