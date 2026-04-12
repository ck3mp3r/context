-- Query: has_member
-- Description: Retrieve all module/namespace membership relationships
-- Returns: List of has_member edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM has_member
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
