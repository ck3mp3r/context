-- Query: extends
-- Description: Retrieve all type extension relationships
-- Returns: List of extends edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM extends
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
