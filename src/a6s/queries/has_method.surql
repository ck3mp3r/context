-- @internal
-- Query: has_method
-- Description: Retrieve all method membership relationships
-- Returns: List of has_method edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM has_method
WHERE repo_id = $repo_id;
