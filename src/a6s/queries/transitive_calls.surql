-- Find all functions reachable from a given function through call chains
-- Traverses up to 3 levels deep (depth is fixed, not configurable)
-- Examples:
--   $name = "main" finds all functions reachable from main (3 levels)
--   $name = "handle_request" shows the call tree of a request handler
-- Parameter: $name (String) - Source function name
-- Returns: All transitively called functions with file locations
--
SELECT
    out.name AS name,
    out.kind AS kind,
    out.file_path AS file_path,
    out.start_line AS start_line
FROM calls
WHERE
    in.repo_id = $repo_id
    AND out.repo_id = $repo_id
    AND (
        -- Depth 1: in.name matches source
        in.name = $name
        OR
        -- Depth 2: in is a symbol called by source
        in IN (
            SELECT VALUE out FROM calls
            WHERE in.repo_id = $repo_id
              AND out.repo_id = $repo_id
              AND in.name = $name
        )
        OR
        -- Depth 3: in is a symbol at depth 2
        in IN (
            SELECT VALUE out FROM calls
            WHERE in.repo_id = $repo_id
              AND out.repo_id = $repo_id
              AND in IN (
                  SELECT VALUE out FROM calls
                  WHERE in.repo_id = $repo_id
                    AND out.repo_id = $repo_id
                    AND in.name = $name
              )
        )
    )
GROUP BY out.name, out.kind, out.file_path, out.start_line;
