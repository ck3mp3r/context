# Test runner for nushell test suite
# This file demonstrates a main function that runs tests

export def "test basic math" [] {
    assert equal (1 + 1) 2
}

export def "test string ops" [] {
    assert equal ("hello" | str length) 5
}

export def "test list ops" [] {
    assert equal ([1 2 3] | length) 3
}

def main [] {
    # Get all test functions
    let tests = (
        scope commands
            | where ($it.name | str starts-with "test ")
    )

    print $"Running ($tests | length) tests..."

    # Run each test
    for test in $tests {
        print $"  - ($test.name)"
        do $test.name
    }

    print "All tests passed!"
}
