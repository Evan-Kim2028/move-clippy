// Test fixture for oracle_price_taint lint
// Tests taint tracking for untrusted oracle prices

module oracle_taint_pkg::oracle_taint {
    use sui::object::UID;

    const E_INVALID_PRICE: u64 = 1;
    const E_STALE_PRICE: u64 = 2;

    // Mock oracle struct
    public struct Oracle has key {
        id: UID,
        price: u64,
        timestamp: u64,
    }

    // Mock functions that would be in an oracle module
    public fun get_price(_oracle: &Oracle): u64 {
        // This would fetch from the oracle
        1000
    }

    public fun get_oracle_price(_oracle: &Oracle): u64 {
        // Alternative oracle price getter
        1000
    }

    // SHOULD WARN: Unvalidated oracle price used in calculation
    public fun bad_calculate_value(oracle: &Oracle, amount: u64): u64 {
        let price = get_price(oracle);
        // Price is tainted and used directly in calculation!
        amount * price
    }

    // SHOULD WARN: Tainted price flows through variable
    public fun bad_price_flow(oracle: &Oracle, amount: u64): u64 {
        let price = get_oracle_price(oracle);
        let adjusted_price = price;  // Taint propagates
        amount * adjusted_price
    }

    // SHOULD NOT WARN: Price is validated before use
    public fun good_calculate_value(oracle: &Oracle, amount: u64): u64 {
        let price = get_price(oracle);
        assert!(price > 0, E_INVALID_PRICE);  // Validation!
        amount * price
    }

    // SHOULD NOT WARN: Price validated with bounds check
    public fun good_bounds_check(oracle: &Oracle, amount: u64, max_price: u64): u64 {
        let price = get_price(oracle);
        assert!(price > 0 && price <= max_price, E_INVALID_PRICE);
        amount * price
    }

    // SHOULD WARN: Only partial validation (timestamp but not price)
    public fun partial_validation(oracle: &Oracle, amount: u64, current_time: u64): u64 {
        let price = get_price(oracle);
        // Validates timestamp but not price value!
        assert!(oracle.timestamp > current_time - 3600, E_STALE_PRICE);
        amount * price
    }

    // SHOULD NOT WARN: Price used only for comparison, not arithmetic
    public fun price_comparison_only(oracle: &Oracle, threshold: u64): bool {
        let price = get_price(oracle);
        price > threshold  // Comparison is safe
    }

    // SHOULD WARN: Taint propagates through struct unpacking  
    public struct PriceData has drop {
        value: u64,
        confidence: u64,
    }

    public fun get_price_data(_oracle: &Oracle): PriceData {
        PriceData { value: 1000, confidence: 100 }
    }

    public fun bad_unpack_price(oracle: &Oracle, amount: u64): u64 {
        let PriceData { value: price, confidence: _ } = get_price_data(oracle);
        // price is tainted from oracle
        amount * price
    }
}
