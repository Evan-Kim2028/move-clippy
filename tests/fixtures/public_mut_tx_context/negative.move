module txc::good;

use sui::tx_context::TxContext;

public fun takes_mut(ctx: &mut TxContext) {
    ctx;
}

public fun unrelated_ref(value: &u64) {
    value;
}
