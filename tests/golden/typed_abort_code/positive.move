module my_package::my_module {
    public fun bad_numeric_abort(value: u64) {
        assert!(value > 0, 1);
        assert!(value < 100, 2);
    }

    public fun bad_multiple_errors(x: u64, y: u64) {
        assert!(x != 0, 100);
        assert!(y != 0, 101);
        assert!(x + y > 0, 102);
    }

    public fun bad_abort_statement(value: u64) {
        if (value == 0) {
            abort 42
        };
    }
}
