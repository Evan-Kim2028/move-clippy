// Golden test: unchecked_withdrawal - NEGATIVE (should NOT trigger)
// Description: Withdrawal with proper balance validation

module 0x1::test {
    const E_INSUFFICIENT_BALANCE: u64 = 1;

    public struct Account has key {
        id: UID,
        balance: u64,
    }

    // GOOD: Withdraw with balance check
    public fun good_withdraw(account: &mut Account, amount: u64): u64 {
        assert!(account.balance >= amount, E_INSUFFICIENT_BALANCE);
        account.balance = account.balance - amount;
        amount
    }

    // GOOD: withdraw_all with explicit zero check
    public fun safe_withdraw_all(account: &mut Account): u64 {
        let amount = account.balance;
        assert!(amount > 0, E_INSUFFICIENT_BALANCE);
        account.balance = 0;
        amount
    }
}
