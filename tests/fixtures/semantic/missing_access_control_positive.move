module test::missing_access_control_positive {
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    public struct Vault has key {
        id: UID,
        balance: u64,
        paused: bool,
    }

    /// Bug: Public function modifies state without access control
    public fun set_pause_status(vault: &mut Vault, paused: bool) {
        // Should fire: no capability parameter
        vault.paused = paused;
    }

    /// Bug: Public function modifies balance without cap
    public fun withdraw_all(vault: &mut Vault): u64 {
        // Should fire: dangerous state modification
        let amount = vault.balance;
        vault.balance = 0;
        amount
    }

    /// Bug: Public function with multiple mut params, no cap
    public fun transfer_funds(
        from: &mut Vault,
        to: &mut Vault,
        amount: u64
    ) {
        // Should fire: modifies multiple objects without authorization
        from.balance = from.balance - amount;
        to.balance = to.balance + amount;
    }
}
