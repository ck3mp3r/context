-- Ego-network: all symbols directly connected via calls (both directions)
-- Returns both incoming callers and outgoing callees in a single result set
--
-- Parameters:
--   $repo_id (String) - Repository ID
--   $name (String) - Source symbol name
--
-- LIMITATION: Only covers 'calls' edges. For a complete ego-network including
-- all relationship types (has_method, has_field, extends, implements, etc.),
-- use the specific edge queries (callers, callees, has_method, etc.) separately.
--
SELECT
    IF in.name = $name THEN out.name ELSE in.name END AS name,
    IF in.name = $name THEN out.kind ELSE in.kind END AS kind,
    IF in.name = $name THEN out.file_path ELSE in.file_path END AS file_path,
    IF in.name = $name THEN out.start_line ELSE in.start_line END AS start_line,
    IF in.name = $name THEN 'outgoing' ELSE 'incoming' END AS direction,
    'calls' AS edge_type
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
    AND (in.name = $name OR out.name = $name)
FETCH in, out;