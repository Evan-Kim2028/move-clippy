module example::test {
    const E_FAIL: u64 = 1;
    const E_MISMATCH: u64 = 2;

    // Simple equality check - should trigger
    public fun test_simple() {
        let x = 5;
        let y = 5;
        assert!(x == y);
    }

    // With error code - should trigger
    public fun test_with_error_code() {
        let balance = 100;
        assert!(balance == 100, E_FAIL);
    }

    // With error code and message - should trigger
    public fun test_with_error_and_message() {
        let value = 42;
        assert!(value == 42, E_MISMATCH, "values don't match");
    }

    // Field access - should trigger
    public fun test_field_access() {
        let obj = SomeStruct { amount: 10 };
        assert!(obj.amount == 10);
    }

    // Module-qualified types - should trigger
    public fun test_qualified() {
        let addr = @0x123;
        assert!(addr == @0x123, 0);
    }

    struct SomeStruct has drop {
        amount: u64
    }
}
