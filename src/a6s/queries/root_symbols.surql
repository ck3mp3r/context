-- Query: root_symbols
-- Description: Find root symbols (not targeted by any containment edge)
-- Returns: List of symbols that are not children of any has_member, has_method, or has_field edge
--          Includes child_count (number of outgoing containment edges)
--
-- Containment edge direction: parent->edge->child
-- - has_member: module/namespace has members
-- - has_method: type has methods
-- - has_field: struct/class has fields
--
-- Root symbols are those with no incoming containment edges (no parent container)

SELECT
    symbol_id,
    name,
    kind,
    language,
    file_path,
    start_line,
    entry_type,
    module_path,
    0 AS child_count
FROM symbol
WHERE repo_id = $repo_id
    AND symbol_id NOT IN (SELECT out.symbol_id FROM has_member WHERE in.repo_id = $repo_id)
    AND symbol_id NOT IN (SELECT out.symbol_id FROM has_method WHERE in.repo_id = $repo_id)
    AND symbol_id NOT IN (SELECT out.symbol_id FROM has_field WHERE in.repo_id = $repo_id);
