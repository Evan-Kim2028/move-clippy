module test::ctx_test {
    use sui::tx_context::TxContext;

    public fun bad_immutable_ctx(ctx: &TxContext) {
        let _ = ctx;
    }

    public fun bad_entry_immutable(amount: u64, ctx: &TxContext) {
        let _ = amount;
        let _ = ctx;
    }

    public fun bad_sender(ctx: &TxContext): address {
        @0x1
    }
}
