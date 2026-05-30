-- @internal
-- Query: declares_mod
-- Description: Retrieve all module declaration relationships (mod foo;)
-- Returns: List of declares_mod edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM declares_mod
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
