module test::ctx_test {
    use sui::tx_context::TxContext;

    public fun good_mutable_ctx(ctx: &mut TxContext) {
        let _ = ctx;
    }

    public fun good_entry_mutable(amount: u64, ctx: &mut TxContext) {
        let _ = amount;
        let _ = ctx;
    }

    public fun good_sender(ctx: &mut TxContext): address {
        @0x1
    }

    fun private_immutable(ctx: &TxContext) {
        let _ = ctx;
    }
}
