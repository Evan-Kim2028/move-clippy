// Golden test: unchecked_withdrawal - POSITIVE (should trigger lint)
// Description: Withdrawal without balance validation

module 0x1::test;

use sui::coin::{Self, Coin};
use sui::sui::SUI;

public struct UserBalance has key, store {
    id: UID,
    balance: u64,
}

// BAD: withdraw without checking balance
public fun bad_withdraw(user: &mut UserBalance, amount: u64): u64 {
    user.balance = user.balance - amount;
    amount
}

// BAD: withdraw_all without validation
public fun bad_withdraw_all(user: &mut UserBalance): u64 {
    let amt = user.balance;
    user.balance = 0;
    amt
}
