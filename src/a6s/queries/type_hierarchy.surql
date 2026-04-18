-- Inheritance and implementation relationships between types
-- Shows which types extend or implement other types. Key for understanding polymorphism
-- Uses extends and implements tables for reliable performance
SELECT
    in.name AS type_name,
    in.kind AS type_kind,
    out.name AS parent_name,
    out.kind AS parent_kind,
    in.file_path AS file_path,
    'extends' AS relationship
FROM extends
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
FETCH in, out;
