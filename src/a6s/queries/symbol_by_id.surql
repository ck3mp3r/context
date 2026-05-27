-- Query: symbol_by_id
-- Description: Fetch a single symbol by its symbol_id and repo_id
-- Parameters:
--   $symbol_id: The symbol ID to fetch
--   $repo_id: The repository ID
-- Returns: Single symbol record with all properties

SELECT
    symbol_id,
    name,
    kind,
    language,
    file_path,
    start_line,
    entry_type,
    module_path,
    (
        count(->has_member->symbol) +
        count(->has_method->symbol) +
        count(->has_field->symbol)
    ) AS child_count
FROM symbol
WHERE symbol_id = $symbol_id
    AND repo_id = $repo_id
LIMIT 1;
