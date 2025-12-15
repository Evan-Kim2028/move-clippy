module example::test {
    use std::option::Option;

    // Simple pattern - should trigger
    public fun test_simple(opt: Option<u64>) {
        if (opt.is_some()) {
            let value = opt.destroy_some();
            process(value);
        }
    }

    // Multiple statements in body - should trigger
    public fun test_multiple_statements(opt: Option<u64>) {
        if (opt.is_some()) {
            let v = opt.destroy_some();
            let doubled = v * 2;
            store(doubled);
        }
    }

    // Different variable names - should trigger
    public fun test_diff_names(my_option: Option<u64>) {
        if (my_option.is_some()) {
            let extracted = my_option.destroy_some();
            use_value(extracted);
        }
    }

    fun process(x: u64) {}
    fun store(x: u64) {}
    fun use_value(x: u64) {}
}
