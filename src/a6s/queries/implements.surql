-- Query: implements
-- Description: Retrieve all trait/interface implementation relationships
-- Returns: List of implements edges with implementor and interface details

SELECT
    in.symbol_id AS src_id,
    in.name AS implementor,
    in.kind AS implementor_kind,
    in.file_path AS implementor_file,
    out.symbol_id AS dst_id,
    out.name AS interface,
    out.kind AS interface_kind,
    out.file_path AS interface_file
FROM implements
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
