module test::unused_return_value_negative {
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};

    /// Correct: split return value is used
    public fun split_coin_used(
        coin: &mut Coin<SUI>,
        recipient: address,
        ctx: &mut TxContext
    ) {
        let split_coin = coin::split(coin, 100, ctx);
        transfer::public_transfer(split_coin, recipient);
    }

    /// Correct: take return value is bound
    public fun take_used(
        coin: &mut Coin<SUI>,
        ctx: &mut TxContext
    ) {
        let taken = coin::take(coin::balance_mut(coin), 50, ctx);
        coin::destroy_zero(taken);
    }

    /// Correct: split used inline
    public fun split_inline_transfer(
        coin: &mut Coin<SUI>,
        ctx: &mut TxContext
    ) {
        transfer::public_transfer(
            coin::split(coin, 100, ctx),
            tx_context::sender(ctx)
        );
    }
}
