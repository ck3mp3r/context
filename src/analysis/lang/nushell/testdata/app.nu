use std

const VERSION = "1.0.0"
const MAX_RETRIES = 5

module network {
    export def ping [host: string] -> string {
        $"Pinging ($host)..."
    }

    export def fetch [url: string] -> string {
        http get $url
    }
}

def process-items [items: list<string>] -> list<string> {
    $items | each { |it| $it | str trim | str upcase }
}

def main [] {
    let hosts = ["example.com" "google.com"]
    $hosts | each { |h| network ping $h }

    let data = process-items ["  hello  " "  world  "]
    print $data

    let files = ls | where size > 1mb
    $files | length
}

alias ll = ls -la
alias gst = git status
