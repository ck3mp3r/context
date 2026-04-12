-- Public API surface: all public structs, traits, interfaces, and enums
-- Use to understand what a codebase exposes
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line
FROM symbol
WHERE repo_id = $repo_id AND visibility = "public"
ORDER BY kind ASC;
