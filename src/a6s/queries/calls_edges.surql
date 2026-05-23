-- Query: calls_edges
-- Description: Retrieve all function call relationships with full source and target details
-- Returns: List of call edges with complete symbol information for both caller and callee

SELECT
    in.symbol_id AS src_id,
    in.name AS src_name,
    in.kind AS src_kind,
    in.language AS src_language,
    in.file_path AS src_file_path,
    in.start_line AS src_start_line,
    in.entry_type AS src_entry_type,
    out.symbol_id AS dst_id,
    out.name AS dst_name,
    out.kind AS dst_kind,
    out.language AS dst_language,
    out.file_path AS dst_file_path,
    out.start_line AS dst_start_line,
    out.entry_type AS dst_entry_type
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
