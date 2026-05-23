-- Public API surface: all public structs, traits, interfaces, and enums
-- Use to understand what a codebase exposes
-- Works across languages: Rust (pub), Go (pub for exported), Kotlin (pub for public/default)
SELECT
    name,
    kind,
    visibility,
    file_path,
    start_line
FROM symbol
WHERE repo_id = $repo_id AND visibility IN ["public", "pub"]
ORDER BY kind ASC;
