// Golden test: unchecked_withdrawal - POSITIVE (should trigger with --experimental)
// Description: Withdrawal without balance validation (Thala hack pattern)

module 0x1::test {
    public struct Account has key {
        id: UID,
        balance: u64,
    }

    // BAD: Withdraw without checking balance
    public fun bad_withdraw(account: &mut Account, amount: u64): u64 {
        account.balance = account.balance - amount;
        amount
    }

    // BAD: withdraw_all without validation
    public fun withdraw_all(account: &mut Account): u64 {
        let amount = account.balance;
        account.balance = 0;
        amount
    }
}
