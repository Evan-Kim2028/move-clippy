// Golden test: unchecked_coin_split - NEGATIVE (should NOT trigger)
// Description: Proper coin splitting with balance checks

module 0x1::test {
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;

    const E_INSUFFICIENT_BALANCE: u64 = 1;

    // GOOD: Split with balance check
    public fun good_split(coin: &mut Coin<SUI>, amount: u64): Coin<SUI> {
        assert!(coin::value(coin) >= amount, E_INSUFFICIENT_BALANCE);
        coin::split(coin, amount)
    }

    // GOOD: Check before split
    public fun checked_split(c: &mut Coin<SUI>, amt: u64): Coin<SUI> {
        let balance = coin::value(c);
        assert!(balance >= amt, E_INSUFFICIENT_BALANCE);
        coin::split(c, amt)
    }
}
