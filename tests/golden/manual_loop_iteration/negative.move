module example::test {
    use std::vector;

    // Already using do_ref! - should not trigger
    public fun test_already_do_ref(vec: &vector<u64>) {
        vec.do_ref!(|elem| {
            process(*elem);
        });
    }

    // No increment - should not trigger
    public fun test_no_increment(vec: &vector<u64>) {
        let mut i = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            process(*elem);
            // Missing increment (not incrementing the loop variable)
        };
    }

    // Different iteration variable - should not trigger
    public fun test_different_iter_var(vec: &vector<u64>) {
        let mut i = 0;
        let mut j = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            process(*elem);
            j = j + 1; // Wrong variable!
        };
    }

    fun process(x: u64) {}
}
