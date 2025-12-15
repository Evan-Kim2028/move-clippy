// Golden test: admin_cap_position - NEGATIVE (should NOT trigger lint)
// Description: Capability parameters in correct position

module 0x1::test {
    use sui::tx_context::TxContext;

    public struct AdminCap has key, store {
        id: UID,
    }

    // GOOD: AdminCap is first parameter
    public fun good_cap_first(cap: &AdminCap, amount: u64) {
        let _ = cap;
        let _ = amount;
    }

    // GOOD: AdminCap is second after TxContext
    public fun good_cap_second(ctx: &TxContext, cap: &AdminCap, amount: u64) {
        let _ = ctx;
        let _ = cap;
        let _ = amount;
    }

    // GOOD: TxContext first, AdminCap second
    public fun good_order(ctx: &mut TxContext, cap: &AdminCap) {
        let _ = ctx;
        let _ = cap;
    }

    // GOOD: No capability parameter
    public fun no_cap(amount: u64) {
        let _ = amount;
    }
}
