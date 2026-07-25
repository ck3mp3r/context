-- High-level codebase overview: symbol counts by kind
-- Run this first when exploring a new codebase to understand its structure and scale
-- Examples:
--   code_query(query_name="overview") → symbol counts by type (functions, classes, etc)
--   Use first when exploring unfamiliar codebase
SELECT
    kind,
    count() AS total
FROM symbol
WHERE repo_id = $repo_id
GROUP BY kind;
