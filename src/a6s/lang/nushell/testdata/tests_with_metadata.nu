# Test file with metadata annotations

# unit
export def "test basic addition" [] {
    use std assert
    assert equal (1 + 1) 2
}

# integration
export def "test api integration" [] {
    use std assert
    assert true
}

# ignore
# unit
export def "test broken feature" [] {
    # This test is skipped
    assert false
}

# Just a regular function
export def helper-function [] {
    42
}

# ignore
export def "test slow operation" [] {
    # This test is ignored
    sleep 10sec
}

# integration
# This test doesn't have ignore
export def "test database connection" [] {
    use std assert
    assert true
}
