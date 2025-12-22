/// Test fixture for stale_oracle_price_v3 lint (CFG-aware dataflow).
///
/// This lint tracks oracle price values from `get_price_unsafe` through the CFG
/// and reports when they are used without freshness validation.
///
/// KEY DIFFERENCE FROM V2:
/// - V2 flags ALL calls to get_price_unsafe
/// - V3 only flags when the price is USED without validation
///
/// Detection is DATAFLOW-BASED:
/// - Sources: pyth::get_price_unsafe, price_info::get_price_unsafe, etc.
/// - Validation sinks: check_price_is_fresh, check_freshness, get_price_no_older_than
/// - Use sinks: passing price to other functions, field access, return

// =============================================================================
// Mock Oracle Modules
// =============================================================================

module pyth::pyth {
    public struct Price has drop, copy {
        price: u64,
        timestamp: u64,
    }

    /// Unsafe function - may return stale prices
    public fun get_price_unsafe(_feed_id: u64): Price {
        Price { price: 100, timestamp: 0 }
    }

    /// Validate price freshness - marks price as validated
    public fun check_price_is_fresh(_price: &Price, _max_age: u64): bool {
        true
    }

    /// Safe function - returns validated price
    public fun get_price_no_older_than(_feed_id: u64, _max_age: u64): Price {
        Price { price: 100, timestamp: 0 }
    }
}

module pyth::price_info {
    public struct PriceInfoObject has drop {
        price: u64,
    }

    /// Unsafe function in price_info module
    public fun get_price_unsafe(_info: &PriceInfoObject): u64 {
        _info.price
    }
}

module switchboard::switchboard {
    public struct Price has drop, copy {
        value: u64,
    }

    /// Unsafe function
    public fun get_price_unsafe(_feed_id: u64): Price {
        Price { value: 100 }
    }

    /// Validate freshness
    public fun check_freshness(_price: &Price, _max_staleness: u64) {
        // aborts if stale
    }
}

module supra::supra {
    public struct Price has drop, copy {
        price: u64,
    }

    /// Unsafe function
    public fun get_price_unsafe(_feed_id: u64): Price {
        Price { price: 100 }
    }
}

// =============================================================================
// Helper module to receive prices (simulates a lending protocol)
// =============================================================================

module stale_oracle_price_v3_pkg::lending {
    use pyth::pyth;

    public fun borrow_with_price(_price: &pyth::Price, _amount: u64) {
        // Lending logic that uses the price
    }

    public fun calculate_collateral(_price: &pyth::Price, _amount: u64): u64 {
        100
    }
}

// =============================================================================
// POSITIVE CASES - Should trigger stale_oracle_price_v3
// =============================================================================

module stale_oracle_price_v3_pkg::positive_cases {
    use pyth::pyth;
    use switchboard::switchboard;
    use supra::supra;
    use stale_oracle_price_v3_pkg::lending;

    /// Get unsafe price and pass to MODULE CALL without validation - SHOULD FIRE V3
    public fun unvalidated_price_to_lending(feed_id: u64) {
        let price = pyth::get_price_unsafe(feed_id);
        // Passing unvalidated price to another module (lending protocol)
        lending::borrow_with_price(&price, 1000);
    }

    /// Get unsafe price and pass to local function without validation - V1/V2 only
    public fun unvalidated_price_used(feed_id: u64): u64 {
        let price = pyth::get_price_unsafe(feed_id);
        // Passing unvalidated price to local function (not detected by v3)
        use_price(&price)
    }

    /// Get unsafe price and return without validation - SHOULD FIRE
    public fun unvalidated_price_returned(feed_id: u64): pyth::Price {
        pyth::get_price_unsafe(feed_id)
    }

    /// Multiple unsafe prices without validation - SHOULD FIRE MULTIPLE TIMES
    public fun multiple_unvalidated(feed_id: u64): u64 {
        let p1 = pyth::get_price_unsafe(feed_id);
        let p2 = switchboard::get_price_unsafe(feed_id);
        use_price(&p1) + use_switchboard_price(&p2)
    }

    /// Unsafe price used in complex expression - SHOULD FIRE
    public fun unvalidated_in_expression(feed_id: u64): u64 {
        let price = pyth::get_price_unsafe(feed_id);
        // Price used without validation
        calculate_collateral(&price, 1000)
    }

    /// Helper functions that receive unvalidated prices
    fun use_price(_price: &pyth::Price): u64 {
        100
    }

    fun use_switchboard_price(_price: &switchboard::Price): u64 {
        100
    }

    fun calculate_collateral(_price: &pyth::Price, _amount: u64): u64 {
        100
    }
}

// =============================================================================
// NEGATIVE CASES - Should NOT trigger stale_oracle_price_v3
// =============================================================================

module stale_oracle_price_v3_pkg::negative_cases {
    use pyth::pyth;
    use switchboard::switchboard;

    /// Price validated before use - NO LINT
    public fun validated_before_use(feed_id: u64): u64 {
        let price = pyth::get_price_unsafe(feed_id);
        // Validate freshness first
        assert!(pyth::check_price_is_fresh(&price, 60), 0);
        // Now safe to use
        use_price(&price)
    }

    /// Using safe function - NO LINT
    public fun use_safe_function(feed_id: u64): pyth::Price {
        pyth::get_price_no_older_than(feed_id, 60)
    }

    /// Price validated via check_freshness - NO LINT
    public fun switchboard_validated(feed_id: u64): u64 {
        let price = switchboard::get_price_unsafe(feed_id);
        // Validation function
        switchboard::check_freshness(&price, 60);
        use_switchboard_price(&price)
    }

    /// Price validated with custom pattern - NO LINT
    /// (function name contains validation keywords)
    public fun custom_validation(feed_id: u64): u64 {
        let price = pyth::get_price_unsafe(feed_id);
        validate_freshness(&price);
        use_price(&price)
    }

    /// User-defined function with similar name (not from oracle module) - NO LINT
    public fun local_get_price_unsafe(): u64 {
        42
    }

    /// Helper functions
    fun use_price(_price: &pyth::Price): u64 {
        100
    }

    fun use_switchboard_price(_price: &switchboard::Price): u64 {
        100
    }

    fun validate_freshness(_price: &pyth::Price) {
        // custom validation
    }
}

// =============================================================================
// EDGE CASES
// =============================================================================

module stale_oracle_price_v3_pkg::edge_cases {
    use pyth::pyth;

    /// Price stored in variable but never used - NO LINT
    /// (no actual use of the unvalidated price)
    public fun unused_price(feed_id: u64) {
        let _price = pyth::get_price_unsafe(feed_id);
        // Price is never used, so no vulnerability
    }

    /// Price validated in one branch, used in both - SHOULD BE OK
    /// (optimistic: if any path validates, we trust it)
    public fun conditional_validation(feed_id: u64, validate: bool): u64 {
        let price = pyth::get_price_unsafe(feed_id);
        if (validate) {
            assert!(pyth::check_price_is_fresh(&price, 60), 0);
        };
        use_price(&price)
    }

    fun use_price(_price: &pyth::Price): u64 {
        100
    }
}
