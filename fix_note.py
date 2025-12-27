#!/usr/bin/env python3

import re

# Read the file
with open("src/db/sqlite/note.rs", "r") as f:
    content = f.read()

# Pattern to match Note { ... } struct constructors
pattern = r'(Note \{[^}]*note_type,\s*)(created_at: row\.get\("created_at"\),)'


# Replace with the pattern that adds project_ids field
def replace_func(match):
    prefix = match.group(1)
    created_at_line = match.group(2)
    return f"{prefix}project_ids: vec![], // Empty by default - relationships managed separately\n                    {created_at_line}"


# Apply the replacement
new_content = re.sub(pattern, replace_func, content, flags=re.DOTALL)

# Write back
with open("src/db/sqlite/note.rs", "w") as f:
    f.write(new_content)

print("Fixed Note constructors")
