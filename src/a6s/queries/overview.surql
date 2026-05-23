-- High-level codebase overview: symbol counts by kind
-- Run this first when exploring a new codebase to understand its structure and scale
SELECT
    kind,
    count() AS total
FROM symbol
WHERE repo_id = $repo_id
GROUP BY kind;
