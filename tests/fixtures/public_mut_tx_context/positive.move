module txc::bad;

use sui::tx_context::TxContext;

public fun takes_shared(ctx: &TxContext) {
    let _ = ctx;
}

public fun fully_qualified(ctx: &sui::tx_context::TxContext) {
    let _ = ctx;
}
