module my_pkg::m;

fun f(ctx: u8) {
    let _a = tx_context::sender(ctx, 1);
}
