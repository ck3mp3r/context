-- Most-called symbols: functions and methods with the highest incoming call count
-- Hub symbols are the most important code in the codebase. Start here to understand core logic
SELECT
    out.name AS name,
    out.kind AS kind,
    out.file_path AS file_path,
    out.start_line AS start_line,
    count() AS incoming_calls
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
GROUP BY out.name, out.kind, out.file_path, out.start_line
ORDER BY incoming_calls DESC
LIMIT 30;
