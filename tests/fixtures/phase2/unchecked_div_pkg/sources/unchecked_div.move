// Test fixture for unchecked_division_v2 lint
// Tests CFG-aware division validation tracking

module unchecked_div_pkg::unchecked_div {
    const E_DIVISION_BY_ZERO: u64 = 1;

    // SHOULD WARN: Division without zero check
    public fun bad_divide(a: u64, b: u64): u64 {
        a / b  // No validation!
    }

    // SHOULD WARN: Modulo without zero check
    public fun bad_modulo(a: u64, b: u64): u64 {
        a % b  // No validation!
    }

    // SHOULD NOT WARN: Division with proper zero check
    public fun good_divide(a: u64, b: u64): u64 {
        assert!(b != 0, E_DIVISION_BY_ZERO);
        a / b
    }

    // SHOULD NOT WARN: Division with greater-than check
    public fun good_divide_gt(a: u64, b: u64): u64 {
        assert!(b > 0, E_DIVISION_BY_ZERO);
        a / b
    }

    // SHOULD NOT WARN: Division by constant
    public fun divide_by_constant(a: u64): u64 {
        a / 100  // Constant divisor is always safe
    }

    // SHOULD WARN: Conditional check doesn't cover all paths
    public fun conditional_divide(a: u64, b: u64, check: bool): u64 {
        if (check) {
            assert!(b != 0, E_DIVISION_BY_ZERO);
        };
        // Division happens outside the check!
        a / b
    }

    // SHOULD NOT WARN: Check happens in called function
    public fun checked_via_call(a: u64, b: u64): u64 {
        validate_divisor(b);
        a / b
    }

    fun validate_divisor(d: u64) {
        assert!(d != 0, E_DIVISION_BY_ZERO);
    }

    // SHOULD NOT WARN: Complex expression with inline check
    public fun inline_check_divide(a: u64, b: u64): u64 {
        if (b == 0) {
            0
        } else {
            a / b
        }
    }
}
