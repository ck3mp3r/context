-- Query: all_symbols
-- Description: Retrieve all symbols from the code graph
-- Returns: List of symbols with their basic properties

SELECT
    symbol_id,
    name,
    kind,
    language,
    file_path,
    start_line,
    entry_type,
    module_path
FROM symbol
WHERE repo_id = $repo_id;
