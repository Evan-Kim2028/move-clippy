module my_package::my_module {
    const E_INVALID_VALUE: u64 = 1;
    const E_OUT_OF_RANGE: u64 = 2;
    const E_DIVISION_BY_ZERO: u64 = 100;

    public fun good_named_errors(value: u64) {
        assert!(value > 0, E_INVALID_VALUE);
        assert!(value < 100, E_OUT_OF_RANGE);
    }

    public fun good_error_constants(x: u64, y: u64) {
        assert!(x != 0, E_DIVISION_BY_ZERO);
        assert!(y != 0, E_DIVISION_BY_ZERO);
    }
}
