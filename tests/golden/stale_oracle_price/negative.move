// Golden test: stale_oracle_price - NEGATIVE (should NOT trigger lint)
// Description: Using safe price fetching with timestamp validation

module 0x1::test {
    use sui::clock::{Self, Clock};

    const E_STALE_PRICE: u64 = 1;
    const MAX_STALENESS: u64 = 300; // 5 minutes

    public struct Oracle has key {
        id: UID,
        price: u64,
        updated_at: u64,
    }

    // GOOD: Safe price getter with staleness check
    public fun get_price(oracle: &Oracle, clock: &Clock): u64 {
        let current_time = clock.timestamp_ms();
        assert!(current_time - oracle.updated_at < MAX_STALENESS, E_STALE_PRICE);
        oracle.price
    }

    // GOOD: Another safe getter
    public fun get_validated_price(oracle: &Oracle, clock: &Clock): u64 {
        validate_freshness(oracle, clock);
        oracle.price
    }

    fun validate_freshness(oracle: &Oracle, clock: &Clock) {
        let age = clock.timestamp_ms() - oracle.updated_at;
        assert!(age < MAX_STALENESS, E_STALE_PRICE);
    }
}
