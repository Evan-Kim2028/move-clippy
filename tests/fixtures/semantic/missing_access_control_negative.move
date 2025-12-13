module test::missing_access_control_negative {
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    public struct Vault has key {
        id: UID,
        balance: u64,
        paused: bool,
    }

    public struct AdminCap has key, store {
        id: UID,
    }

    /// Correct: Has capability parameter
    public fun set_pause_status(
        _cap: &AdminCap,
        vault: &mut Vault,
        paused: bool
    ) {
        vault.paused = paused;
    }

    /// Correct: Has admin cap
    public fun withdraw_all(
        _admin_cap: &AdminCap,
        vault: &mut Vault
    ): u64 {
        let amount = vault.balance;
        vault.balance = 0;
        amount
    }

    /// Correct: Getter function (starts with get_)
    public fun get_balance(vault: &mut Vault): u64 {
        // Even though it has &mut, it's a getter so no warning
        vault.balance
    }

    /// Correct: View function (starts with is_)
    public fun is_paused(vault: &mut Vault): bool {
        vault.paused
    }

    /// Correct: Package-only visibility
    public(package) fun internal_update(vault: &mut Vault, amount: u64) {
        vault.balance = amount;
    }
}
