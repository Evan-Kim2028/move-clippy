module test::oracle_zero_price_negative {
    use sui::object::{Self, UID};

    const E_ZERO_PRICE: u64 = 1;

    public struct PriceOracle has key {
        id: UID,
        last_price: u64,
    }

    /// Correct: Oracle price validated before use
    public fun calculate_value_with_check(
        oracle: &PriceOracle,
        amount: u64,
    ): u64 {
        let price = oracle.last_price;
        assert!(price > 0, E_ZERO_PRICE);  // Validation present
        amount * price / 1000
    }

    /// Correct: Oracle price validated with != 0
    public fun calculate_shares_with_check(
        oracle: &PriceOracle,
        total_value: u64,
    ): u64 {
        let price = oracle.last_price;
        assert!(price != 0, E_ZERO_PRICE);  // Validation present
        total_value / price
    }
}
