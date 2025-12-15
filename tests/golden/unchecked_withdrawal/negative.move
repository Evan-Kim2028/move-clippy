// Golden test: unchecked_withdrawal - NEGATIVE (should NOT trigger lint)
// Description: Withdrawal with proper balance validation

module 0x1::test;

const E_INSUFFICIENT_BALANCE: u64 = 1;

public struct UserBalance has key, store {
    id: UID,
    balance: u64,
}

// GOOD: withdraw with balance check
public fun good_withdraw(user: &mut UserBalance, amount: u64): u64 {
    assert!(user.balance >= amount, E_INSUFFICIENT_BALANCE);
    user.balance = user.balance - amount;
    amount
}

// GOOD: checked_sub pattern
public fun good_withdraw_checked(user: &mut UserBalance, amount: u64): u64 {
    assert!(amount <= user.balance, E_INSUFFICIENT_BALANCE);
    user.balance = user.balance - amount;
    amount
}
