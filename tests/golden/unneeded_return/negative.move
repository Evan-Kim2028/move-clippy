// Golden test: unneeded_return - NEGATIVE (should NOT trigger lint)
// Description: Implicit returns (correct form)

module 0x1::test;

public fun good_implicit_return(): u64 {
    let x = 42;
    x
}

public fun good_literal(): bool {
    true
}

public fun good_expression(): u64 {
    1 + 2 + 3
}

// Early return in middle of function is OK
public fun good_early_return(x: u64): u64 {
    if (x == 0) {
        return 0
    };
    x * 2
}
