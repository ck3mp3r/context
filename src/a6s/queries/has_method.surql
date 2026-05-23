-- Query: has_method
-- Description: Retrieve all method membership relationships
-- Returns: List of has_method edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM has_method
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
