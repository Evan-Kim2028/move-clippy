// Golden test: while_true_to_loop - POSITIVE (should trigger lint)
// Description: Using while(true) instead of loop

module 0x1::test;

public fun bad_infinite_loop() {
    while (true) {
        break;
    }
}

public fun bad_with_condition() {
    let mut i = 0;
    while (true) {
        i = i + 1;
        if (i > 10) break;
    }
}
