// Golden test: modern_method_syntax - NEGATIVE (should NOT trigger lint)
// Description: Using modern method syntax (receiver.function())

module 0x1::test {
    use sui::tx_context::TxContext;
    use sui::object::UID;

    public fun good_sender_call(ctx: &TxContext): address {
        // GOOD: using method syntax
        ctx.sender()
    }

    public fun good_delete(id: UID) {
        // GOOD: using method syntax
        id.delete();
    }

    public fun good_uid_to_inner(id: &UID): address {
        // GOOD: using method syntax
        id.uid_to_inner()
    }

    // GOOD: Complex expressions as receiver don't trigger
    public fun complex_receiver_ok() {
        let result = some_function().process();
    }
}
