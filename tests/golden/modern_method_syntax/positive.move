// Golden test: modern_method_syntax - POSITIVE (should trigger lint)
// Description: Using module::function(receiver) instead of receiver.function()

module 0x1::test {
    use sui::tx_context::{Self, TxContext};
    use sui::object::{Self, UID};

    public fun bad_sender_call(ctx: &TxContext): address {
        // BAD: should use ctx.sender()
        tx_context::sender(ctx)
    }

    public fun bad_delete(id: UID) {
        // BAD: should use id.delete()
        object::delete(id);
    }

    public fun bad_uid_to_inner(id: &UID): address {
        // BAD: should use id.uid_to_inner()
        object::uid_to_inner(id)
    }
}
