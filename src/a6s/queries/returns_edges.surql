-- Query: returns_edges
-- Description: Get all return type edges (function → return type)
-- Returns: src_id, dst_id for visualization

SELECT
    in.symbol_id as src_id,
    out.symbol_id as dst_id
FROM returns
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id;
