-- Most-called symbols: functions and methods with the highest incoming call count
-- Hub symbols are the most important code in the codebase. Start here to understand core logic
-- Examples:
--   code_query(query_name="hub_symbols") → 30 most-called functions
--   Use to find: core utilities, heavily-used abstractions, critical code
SELECT
    out.name AS name,
    out.kind AS kind,
    out.file_path AS file_path,
    out.start_line AS start_line,
    count() AS incoming_calls
FROM calls
WHERE repo_id = $repo_id
GROUP BY out.name, out.kind, out.file_path, out.start_line
ORDER BY incoming_calls DESC
LIMIT 30;
