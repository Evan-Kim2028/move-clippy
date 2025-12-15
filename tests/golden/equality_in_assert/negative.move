module example::test {
    const E_FAIL: u64 = 1;

    // Already using assert_eq! - should not trigger
    public fun test_already_assert_eq() {
        let x = 5;
        let y = 5;
        assert_eq!(x, y);
    }

    // Not equality (!=) - should not trigger
    public fun test_not_equal() {
        let x = 5;
        assert!(x != 0);
    }

    // Greater than - should not trigger
    public fun test_greater_than() {
        let value = 10;
        assert!(value > 5);
    }

    // Less than - should not trigger
    public fun test_less_than() {
        let value = 3;
        assert!(value < 10);
    }

    // Complex expression with function call - should not trigger
    public fun test_complex() {
        let vec = vector::empty();
        assert!(vector::length(&vec) == 0);
    }

    // Boolean condition - should not trigger
    public fun test_bool() {
        let flag = true;
        assert!(flag);
    }
}
