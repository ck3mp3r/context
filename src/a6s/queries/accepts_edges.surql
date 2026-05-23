-- Query: accepts_edges
-- Description: Get all parameter type edges (function → type parameter)
-- Returns: src_id, dst_id for visualization

SELECT
    in.symbol_id as src_id,
    out.symbol_id as dst_id
FROM accepts
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id;
