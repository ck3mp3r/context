-- All entry points: main, init, test, benchmark, fuzz, example, and exported functions
-- Shows where execution starts and test/benchmark functions
SELECT
    name,
    entry_type,
    kind,
    file_path,
    start_line,
    language
FROM symbol
WHERE repo_id = $repo_id AND entry_type != NONE AND entry_type != ""
ORDER BY entry_type ASC;
