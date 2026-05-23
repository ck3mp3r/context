-- File-level dependency graph: aggregated cross-file relationships
-- Shows which files depend on which, with edge count as weight
SELECT
    in.file_path AS src_file,
    out.file_path AS dst_file,
    count() AS weight
FROM calls
WHERE in.repo_id = $repo_id
    AND out.repo_id = $repo_id
    AND in.file_path != out.file_path
GROUP BY in.file_path, out.file_path
ORDER BY weight DESC;