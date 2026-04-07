use std

const VERSION = "1.0.0"
const MAX_RETRIES = 5

module network {
    export def ping [host: string]: nothing -> string {
        $"Pinging ($host)..."
    }

    export def fetch [url: string]: nothing -> string {
        http get $url
    }
}

def process-items [items: list]: nothing -> list {
    $items | each { |it| $it | str trim | str upcase }
}

def main [] {
    let data = process-items ["hello" "world"]
    print $data
}

alias ll = ls -la
alias gst = git status
