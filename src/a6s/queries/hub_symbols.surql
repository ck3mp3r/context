-- Most-called symbols: functions and methods with the highest incoming call count
-- Hub symbols are the most important code in the codebase. Start here to understand core logic
SELECT
    out,
    count() AS incoming_calls
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
GROUP BY out
ORDER BY incoming_calls DESC
LIMIT 30;
