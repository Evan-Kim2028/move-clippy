module my_pkg::m;

fun f(ctx: u8, id: u8, paid: u8) {
    let _a = tx_context::sender(ctx);
    object::delete(id);
    let _b = coin::into_balance(paid);
}
