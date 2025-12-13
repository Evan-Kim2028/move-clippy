module unneeded_return::good;

use sui::tx_context::TxContext;

public fun value(_ctx: &mut TxContext): u64 {
    let inner = 42;
    inner
}

public fun early_return(_ctx: &mut TxContext) {
    if (false) {
        return;
    };
    loop {
        break;
    };
}
