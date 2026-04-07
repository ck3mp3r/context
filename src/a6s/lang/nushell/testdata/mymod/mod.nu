# mod.nu defines module "mymod"
# It can export its own commands and also contain inline modules

export def greet [name: string]: nothing -> string {
    $"Hello, ($name)!"
}

export const VERSION = "2.0.0"

module inner {
    export def helper []: nothing -> string {
        "I am inner helper"
    }

    export const INNER_CONST = 42
}

export alias hi = greet
