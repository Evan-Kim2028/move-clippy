// Golden test: while_true_to_loop - NEGATIVE (should NOT trigger lint)
// Description: Using loop keyword (correct form)

module 0x1::test;

public fun good_loop() {
    loop {
        break;
    }
}

public fun good_while_condition() {
    let mut i = 0;
    while (i < 10) {
        i = i + 1;
    }
}
