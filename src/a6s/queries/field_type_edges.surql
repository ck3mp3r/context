-- Query: field_type_edges
-- Description: Get all field type edges (field → type)
-- Returns: src_id, dst_id for visualization

SELECT
    in.symbol_id as src_id,
    out.symbol_id as dst_id
FROM field_type
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id;
