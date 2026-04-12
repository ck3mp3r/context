-- Inheritance and implementation relationships between types
-- Shows which types extend or implement other types. Key for understanding polymorphism
SELECT
    in.name AS type_name,
    in.kind AS type_kind,
    out.name AS parent_name,
    out.kind AS parent_kind,
    in.file_path AS file_path
FROM inherits
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
