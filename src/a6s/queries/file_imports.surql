-- Query: file_imports
-- Description: Retrieve all file import relationships
-- Returns: List of file_imports edges with file ID and symbol details

SELECT
    in.file_id AS src_id,
    out.symbol_id AS dst_id,
    out.name AS dst_name,
    out.kind AS dst_kind,
    out.language AS dst_language,
    out.file_path AS dst_file_path,
    out.start_line AS dst_start_line,
    out.entry_type AS dst_entry_type
FROM file_imports
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
