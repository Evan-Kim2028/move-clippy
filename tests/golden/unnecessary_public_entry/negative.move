// Golden test: unnecessary_public_entry - NEGATIVE (should NOT trigger lint)
// Description: Functions with only public or entry, not both

module 0x1::test {
    use sui::tx_context::TxContext;

    // GOOD: Only public
    public fun good_public_only(amount: u64) {
        let _ = amount;
    }

    // GOOD: Only entry
    entry fun good_entry_only(ctx: &TxContext) {
        let _ = ctx;
    }

    // GOOD: Public with package visibility
    public(package) fun good_package_visibility() {}

    // GOOD: No modifiers (private)
    fun good_private() {}
}
