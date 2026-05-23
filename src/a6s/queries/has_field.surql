-- Query: has_field
-- Description: Retrieve all field membership relationships
-- Returns: List of has_field edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM has_field
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
