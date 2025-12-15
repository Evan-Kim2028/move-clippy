// Golden test: unchecked_coin_split - POSITIVE (should trigger with --experimental)
// Description: Using coin::split without balance validation

module 0x1::test {
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;

    // BAD: Split without checking balance first
    public fun bad_split(coin: &mut Coin<SUI>, amount: u64): Coin<SUI> {
        coin::split(coin, amount)
    }

    // BAD: Another split without validation
    public fun split_unchecked(c: &mut Coin<SUI>, amt: u64): Coin<SUI> {
        let split_coin = coin::split(c, amt);
        split_coin
    }
}
