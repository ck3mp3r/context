-- Find test functions that call a specific function (test coverage discovery)
-- Use this to understand what tests cover a function you are changing
-- Examples:
--   $target_name = "parse_config" finds all tests calling parse_config
--   $target_name = "authenticate" finds auth-related tests
-- Parameter: $target_name (String) - Function name to find tests for
-- Returns: Test functions that call the target with file locations
-- Limit: 30 test results
SELECT
    in.name AS test_name,
    in.file_path AS test_file,
    in.start_line AS test_line,
    out.name AS target_name,
    out.file_path AS target_file
FROM calls
WHERE repo_id = $repo_id
  AND out.name = $target_name
  AND in.entry_type = 'test'
ORDER BY in.file_path, in.start_line
LIMIT 30;
