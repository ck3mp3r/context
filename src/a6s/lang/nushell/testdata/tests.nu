# Test functions demonstrating various test patterns

# Basic test with space separator
export def "test fibonacci base cases" [] {
    use std assert
    assert equal (fib 0) 0
    assert equal (fib 1) 1
}

# Kebab-case test
export def test-addition [] {
    use std assert
    assert equal (1 + 1) 2
}

# Snake_case test
export def test_subtraction [] {
    use std assert
    assert equal (5 - 3) 2
}

# Private test (no export)
def "test internal helper" [] {
    use std assert
    assert true
}

# NOT a test - has parameters
export def "test runner" [name: string] {
    print $"Running test: ($name)"
}

# NOT a test - doesn't start with test
export def calculate-sum [] {
    42
}

# NOT a test - starts with "testing" not "test"
export def testing-utils [] {
    "helper"
}

# Main function (also not a test)
def main [] {
    let tests = (scope commands | where name starts-with "test ")
    print $"Found ($tests | length) tests"
}
