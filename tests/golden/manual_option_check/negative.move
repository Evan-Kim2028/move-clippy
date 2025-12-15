module example::test {
    use std::option::Option;

    // Already using do! - should not trigger
    public fun test_already_do(opt: Option<u64>) {
        opt.do!(|v| {
            process(v);
        });
    }

    // No destroy_some - should not trigger
    public fun test_no_destroy(opt: Option<u64>) {
        if (opt.is_some()) {
            do_something();
        }
    }

    // Different option variable - should not trigger
    public fun test_different_var(opt1: Option<u64>, opt2: Option<u64>) {
        if (opt1.is_some()) {
            let value = opt2.destroy_some(); // Different variable!
            process(value);
        }
    }

    fun process(x: u64) {}
    fun do_something() {}
}
