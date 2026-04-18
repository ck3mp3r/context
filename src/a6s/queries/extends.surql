-- Query: extends
-- Description: Retrieve all type extension relationships
-- Returns: List of extends edges with child and parent details

SELECT
    in.symbol_id AS src_id,
    in.name AS child,
    in.kind AS child_kind,
    in.file_path AS child_file,
    out.symbol_id AS dst_id,
    out.name AS parent,
    out.kind AS parent_kind,
    out.file_path AS parent_file
FROM extends
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
