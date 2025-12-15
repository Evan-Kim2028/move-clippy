module example::test {
    use std::vector;

    // Classic manual iteration - should trigger
    public fun test_simple(vec: &vector<u64>) {
        let mut i = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            process(*elem);
            i = i + 1;
        };
    }

    // Different spacing in increment - should trigger
    public fun test_spacing(vec: &vector<u64>) {
        let mut idx = 0;
        while (idx < vec.length()) {
            let value = vec.borrow(idx);
            use_value(*value);
            idx=idx+1;
        };
    }

    // Reverse increment (1 + i) - should trigger
    public fun test_reverse_increment(items: &vector<u64>) {
        let mut i = 0;
        while (i < items.length()) {
            let item = items.borrow(i);
            handle(*item);
            i = 1 + i;
        };
    }

    fun process(x: u64) {}
    fun use_value(x: u64) {}
    fun handle(x: u64) {}
}
