-- Find symbols matching a name pattern (substring search)
-- Use this when you know part of a name but not the exact spelling
-- Examples:
--   $pattern = "auth" finds authenticate, AuthService, authorization
--   $pattern = "config" finds getConfig, ConfigLoader, parse_config
--   $pattern = "handle" finds handleRequest, handle_http_request
-- Parameter: $pattern (String) - Substring to match against symbol names
-- Returns: Symbols matching pattern with file location and signature
-- Limit: 50 results sorted by file_path for grouping
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line,
    signature,
    module_path
FROM symbol
WHERE repo_id = $repo_id
  AND name CONTAINS $pattern
ORDER BY file_path, start_line
LIMIT 50;
