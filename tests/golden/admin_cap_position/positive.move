// Golden test: admin_cap_position - POSITIVE (should trigger lint)
// Description: Capability parameters should be first (or second after TxContext)

module 0x1::test {
    use sui::tx_context::TxContext;

    public struct AdminCap has key, store {
        id: UID,
    }

    // BAD: AdminCap should be first parameter
    public fun bad_cap_last(amount: u64, cap: &AdminCap) {
        let _ = amount;
        let _ = cap;
    }

    // BAD: AdminCap should be first or second (after TxContext)
    public fun bad_cap_third(ctx: &TxContext, amount: u64, cap: &AdminCap) {
        let _ = ctx;
        let _ = amount;
        let _ = cap;
    }

    // BAD: AdminCap in middle position
    public fun bad_cap_middle(x: u64, cap: &AdminCap, y: u64) {
        let _ = x;
        let _ = cap;
        let _ = y;
    }
}
