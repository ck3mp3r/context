# A sibling file in the mymod directory
# This file's commands also belong to module "mymod"

export def process [input: string]: nothing -> string {
    $input | str upcase
}

module nested {
    export def deep-func []: nothing -> string {
        "I am deeply nested"
    }
}
