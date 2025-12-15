// Golden test: unnecessary_public_entry - POSITIVE (should trigger lint)
// Description: Functions with both public and entry modifiers

module 0x1::test {
    use sui::tx_context::TxContext;

    // BAD: Both public and entry
    public entry fun bad_both_modifiers(ctx: &mut TxContext) {
        let _ = ctx;
    }

    // BAD: Another example
    public entry fun bad_transfer(amount: u64, ctx: &TxContext) {
        let _ = amount;
        let _ = ctx;
    }
}
