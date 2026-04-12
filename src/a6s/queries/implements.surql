-- Query: implements
-- Description: Retrieve all trait/interface implementation relationships
-- Returns: List of implements edges with source and target symbol IDs

SELECT
    in.symbol_id AS src_id,
    out.symbol_id AS dst_id
FROM implements
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
