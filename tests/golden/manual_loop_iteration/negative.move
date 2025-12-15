// Golden test: manual_loop_iteration - NEGATIVE (should NOT trigger lint)
// Description: Using loop macros or different loop patterns

module 0x1::test {
    public fun good_use_do_ref() {
        let v = vector[1, 2, 3];
        // GOOD: Using do_ref! macro
        v.do_ref!(|elem| {
            let _ = elem;
        });
    }

    public fun good_use_fold() {
        let v = vector[1, 2, 3];
        // GOOD: Using fold! macro
        let sum = v.fold!(0, |acc, elem| acc + elem);
        let _ = sum;
    }

    public fun good_different_loop() {
        let mut i = 0;
        // GOOD: Not iterating over vector
        while (i < 10) {
            let _ = i * 2;
            i = i + 1;
        };
    }
}
