-- @internal
-- Query: subgraph_edges
-- Description: Get non-containment edges where both endpoints are in a given set of symbol IDs
-- Parameters:
--   $symbol_ids: Array of symbol IDs to filter edges by
-- Returns: List of edges with src_id, dst_id, and edge_type
--
-- Covers all non-containment edge types:
-- - calls: function/method calls
-- - implements: trait/interface implementation
-- - extends: class/type extension
-- - accepts: function parameter types
-- - returns: function return types
-- - field_type: field type references
-- - type_annotation: type annotations
-- - uses: variable/constant references
--
-- NOTE: file_imports is excluded as it's IN file OUT symbol (not symbol->symbol)

SELECT src_id, dst_id, 'Calls' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM calls
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'Implements' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM implements
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'Extends' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM extends
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'Accepts' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM accepts
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'Returns' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM returns
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'FieldType' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM field_type
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'TypeAnnotation' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM type_annotation
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
)
UNION
SELECT src_id, dst_id, 'Uses' AS edge_type FROM (
    SELECT in.symbol_id AS src_id, out.symbol_id AS dst_id
    FROM uses
    WHERE in.repo_id = $repo_id
        AND out.repo_id = $repo_id
        AND in.symbol_id IN $symbol_ids
        AND out.symbol_id IN $symbol_ids
);
