#!/usr/bin/env python3

import os
import re


def fix_repo_constructors(file_path):
    with open(file_path, "r") as f:
        content = f.read()

    # Pattern to match Repo { ... } struct constructors that are missing project_ids
    pattern = r"(Repo \{[^}]*tags: [^,\n]*,?\s*)(created_at: [^,\n]*,)"

    def replace_func(match):
        prefix = match.group(1)
        created_at_line = match.group(2)
        # Add project_ids before created_at
        return f"{prefix}project_ids: vec![], // Empty by default - relationships managed separately\n        {created_at_line}"

    new_content = re.sub(pattern, replace_func, content, flags=re.DOTALL)

    if new_content != content:
        with open(file_path, "w") as f:
            f.write(new_content)
        print(f"Fixed {file_path}")
    else:
        print(f"No changes needed for {file_path}")


# Fix all files that contain Repo constructors
files_to_fix = ["src/db/sqlite/repo_test.rs", "src/db/sqlite/critical_tests.rs"]

for file_path in files_to_fix:
    if os.path.exists(file_path):
        fix_repo_constructors(file_path)
    else:
        print(f"File not found: {file_path}")
