-- Query: symbol_children
-- Description: Get direct children of a given symbol via containment edges
-- Parameters:
--   $root_symbol_id: The parent symbol ID to get children for
-- Returns: List of child symbols with their properties and child_count
--
-- Containment edge direction: parent->edge->child (parent is 'in', child is 'out')
-- Examples:
--   $root_symbol_id = "symbol:abc123" → children of a class/module
--   Use for: exploring class members, module contents

SELECT
    out.symbol_id AS symbol_id,
    out.name AS name,
    out.kind AS kind,
    out.language AS language,
    out.file_path AS file_path,
    out.start_line AS start_line,
    out.entry_type AS entry_type,
    out.module_path AS module_path,
    id AS parent_id,
    (
        (SELECT count() FROM has_member WHERE in.symbol_id = $parent.out.symbol_id GROUP ALL)[0].count ?? 0 +
        (SELECT count() FROM has_method WHERE in.symbol_id = $parent.out.symbol_id GROUP ALL)[0].count ?? 0 +
        (SELECT count() FROM has_field WHERE in.symbol_id = $parent.out.symbol_id GROUP ALL)[0].count ?? 0
    ) AS child_count
FROM (
    SELECT out, in.symbol_id AS id FROM has_member WHERE in.symbol_id = $root_symbol_id AND in.repo_id = $repo_id
    UNION
    SELECT out, in.symbol_id AS id FROM has_method WHERE in.symbol_id = $root_symbol_id AND in.repo_id = $repo_id
    UNION
    SELECT out, in.symbol_id AS id FROM has_field WHERE in.symbol_id = $root_symbol_id AND in.repo_id = $repo_id
);
