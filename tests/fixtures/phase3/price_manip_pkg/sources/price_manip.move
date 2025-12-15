// Test fixture for price_manipulation_window lint
// Tests detection of state changes between oracle price reads

module price_manip_pkg::trading {
    public struct Oracle has store {
        price: u64,
    }

    public struct Pool has store {
        reserve_a: u64,
        reserve_b: u64,
    }

    // Oracle price read functions
    public fun get_price(oracle: &Oracle): u64 {
        oracle.price
    }

    public fun get_oracle_price(oracle: &Oracle): u64 {
        oracle.price
    }

    // State mutation functions
    public fun update_reserves(pool: &mut Pool, new_a: u64, new_b: u64) {
        pool.reserve_a = new_a;
        pool.reserve_b = new_b;
    }

    public fun set_reserve_a(pool: &mut Pool, value: u64) {
        pool.reserve_a = value;
    }

    // SHOULD WARN: Price read, state modified, price read again
    public fun bad_sandwich_pattern(oracle: &Oracle, pool: &mut Pool): u64 {
        let price1 = get_price(oracle);           // First price read
        update_reserves(pool, 1000, 2000);        // State mutation
        let price2 = nested_oracle_read(oracle);  // Nested second read (tests recursive call extraction)
        // Attacker could manipulate price between reads
        price1 + price2
    }

    fun nested_oracle_read(oracle: &Oracle): u64 {
        get_oracle_price(oracle)
    }

    // SHOULD WARN: Multiple oracle reads with state change in between
    public fun bad_multi_read(oracle: &Oracle, pool: &mut Pool): u64 {
        let price_a = get_price(oracle);
        set_reserve_a(pool, 5000);  // State change!
        let price_b = get_price(oracle);
        price_a * price_b
    }

    // SHOULD NOT WARN: Single price read, no manipulation window
    public fun good_single_read(oracle: &Oracle, pool: &mut Pool): u64 {
        let price = get_price(oracle);
        update_reserves(pool, 1000, 2000);
        // Price is cached, no second read
        price * 2
    }

    // SHOULD NOT WARN: Multiple reads but no state change in between
    public fun good_no_mutation(oracle: &Oracle): u64 {
        let price1 = get_price(oracle);
        let price2 = get_oracle_price(oracle);
        // No state mutation between reads
        (price1 + price2) / 2
    }

    // SHOULD NOT WARN: State change after all reads
    public fun good_mutation_after_reads(oracle: &Oracle, pool: &mut Pool): u64 {
        let price1 = get_price(oracle);
        let price2 = get_oracle_price(oracle);
        // All reads done, now safe to mutate
        update_reserves(pool, price1, price2);
        price1 + price2
    }
}
