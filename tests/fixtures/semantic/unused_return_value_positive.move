module test::unused_return_value_positive {
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;
    use sui::tx_context::TxContext;

    /// Bug: coin::split return value is ignored
    public fun split_coin_ignored(
        coin: &mut Coin<SUI>,
        ctx: &mut TxContext
    ) {
        // Should fire: split returns a Coin but it's discarded
        coin::split(coin, 100, ctx);
        // The split coin is lost!
    }

    /// Bug: coin::take return value is ignored
    public fun take_ignored(
        coin: &mut Coin<SUI>,
        ctx: &mut TxContext
    ) {
        // Should fire: take returns extracted Coin
        coin::take(coin::balance_mut(coin), 50, ctx);
        // The taken coin is lost!
    }

    /// Bug: Multiple ignored returns
    public fun multiple_ignored(
        coin: &mut Coin<SUI>,
        ctx: &mut TxContext
    ) {
        // Both should fire
        coin::split(coin, 100, ctx);
        coin::split(coin, 200, ctx);
    }
}
