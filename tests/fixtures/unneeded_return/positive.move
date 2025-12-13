module unneeded_return::bad;

use sui::tx_context::TxContext;

public fun value(_ctx: &mut TxContext): u64 {
    let inner = 42;
    return inner;
}

public fun nested(_ctx: &mut TxContext) {
    if (true) {
        return;
    } else {
        return;
    };
}
