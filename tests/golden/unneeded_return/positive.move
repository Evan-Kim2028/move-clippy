// Golden test: unneeded_return - POSITIVE (should trigger lint)
// Description: Using explicit return at end of function

module 0x1::test;

public fun bad_explicit_return(): u64 {
    let x = 42;
    return x
}

public fun bad_return_literal(): bool {
    return true
}

public fun bad_return_expression(): u64 {
    return 1 + 2 + 3
}
