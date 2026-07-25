-- Public API surface: all public structs, traits, interfaces, and enums
-- Use to understand what a codebase exposes
-- Works across languages: Rust (pub), Go (pub for exported), Kotlin (pub for public/default)
-- Examples:
--   code_query(query_name="public_api") → all public/exported symbols
--   Use to understand: library API surface, exposed interfaces
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line
FROM symbol
WHERE repo_id = $repo_id AND visibility IN ["public", "pub"]
ORDER BY kind ASC;
