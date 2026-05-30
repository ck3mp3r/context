-- Show types that flow through a function (parameters and return types)
-- Use this to understand data dependencies and type contracts
-- Examples:
--   $function_name = "process_request" shows Request param, Response return
--   $function_name = "validate_user" shows User param, Result return
-- Parameter: $function_name (String) - Function to analyze
-- Returns: Parameter types and return type information
-- Limit: 20 type relationships
SELECT
    in.name AS function_name,
    in.kind AS function_kind,
    'accepts' AS relationship,
    out.name AS type_name,
    out.kind AS type_kind,
    out.file_path AS type_file
FROM accepts
WHERE in.repo_id = $repo_id
  AND out.repo_id = $repo_id
  AND in.name = $function_name
FETCH in, out

UNION

SELECT
    in.name AS function_name,
    in.kind AS function_kind,
    'returns' AS relationship,
    out.name AS type_name,
    out.kind AS type_kind,
    out.file_path AS type_file
FROM returns
WHERE in.repo_id = $repo_id
  AND out.repo_id = $repo_id
  AND in.name = $function_name
FETCH in, out
LIMIT 20;
