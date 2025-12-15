// Golden test: stale_oracle_price - POSITIVE (should trigger lint)
// Description: Using get_price_unsafe without timestamp validation

module 0x1::test {
    use sui::clock::Clock;

    public struct Oracle has key {
        id: UID,
        price: u64,
        updated_at: u64,
    }

    // BAD: Using get_price_unsafe (stale price risk)
    public fun bad_get_price(oracle: &Oracle): u64 {
        get_price_unsafe(oracle)
    }

    fun get_price_unsafe(oracle: &Oracle): u64 {
        oracle.price
    }
}
