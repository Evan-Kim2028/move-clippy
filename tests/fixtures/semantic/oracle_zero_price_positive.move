module test::oracle_zero_price_positive {
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    // Hypothetical oracle module
    public struct PriceOracle has key {
        id: UID,
        last_price: u64,
    }

    /// Bug: Oracle price used in calculation without zero check
    public fun calculate_value_no_check(
        oracle: &PriceOracle,
        amount: u64,
    ): u64 {
        let price = oracle.last_price;
        // Should fire: price used in multiplication without zero validation
        amount * price / 1000
    }

    /// Bug: Oracle price used in division without zero check
    public fun calculate_shares_no_check(
        oracle: &PriceOracle,
        total_value: u64,
    ): u64 {
        let price = oracle.last_price;
        // Should fire: price used as divisor without zero validation
        total_value / price
    }
}
