/// Test fixture for shared_with_balance lint.
/// 
/// This lint detects when objects containing Balance<T> fields are shared.
/// Shared objects are publicly accessible, so Balance-containing objects
/// need careful access control to prevent fund theft.
///
/// Tier 2 (Near-Zero FP): Detects Balance field + share_object call pattern.

module sui::object {
    public struct UID has store {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }

    public fun delete(id: UID) {
        let UID { id: _ } = id;
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module sui::transfer {
    public native fun transfer<T: key>(obj: T, recipient: address);
    public native fun share_object<T: key>(obj: T);
    public native fun public_share_object<T: key>(obj: T);
}

module sui::balance {
    public struct Balance<phantom T> has store {
        value: u64,
    }

    public fun zero<T>(): Balance<T> {
        Balance { value: 0 }
    }

    public fun value<T>(b: &Balance<T>): u64 {
        b.value
    }

    public fun join<T>(self: &mut Balance<T>, other: Balance<T>) {
        let Balance { value } = other;
        self.value = self.value + value;
    }

    public fun split<T>(self: &mut Balance<T>, amount: u64): Balance<T> {
        self.value = self.value - amount;
        Balance { value: amount }
    }
}

module sui::coin {
    use sui::balance::Balance;
    
    public struct Coin<phantom T> has key, store {
        id: sui::object::UID,
        balance: Balance<T>,
    }
}

module shared_with_balance_pkg::test_cases {
    use sui::object::UID;
    use sui::balance::Balance;
    use sui::transfer;
    use sui::tx_context::TxContext;

    public struct SUI has drop {}

    // ==========================================================================
    // POSITIVE CASES - Should trigger shared_with_balance
    // ==========================================================================

    /// Pool containing Balance - sharing exposes funds
    /// SHOULD FIRE when shared
    public struct Pool has key {
        id: UID,
        reserves: Balance<SUI>,
    }

    /// Treasury with Balance
    /// SHOULD FIRE when shared
    public struct Treasury has key {
        id: UID,
        funds: Balance<SUI>,
    }

    /// Vault with multiple Balances
    /// SHOULD FIRE when shared
    public struct Vault has key {
        id: UID,
        coin_a: Balance<SUI>,
        coin_b: Balance<SUI>,
    }

    /// Share a Pool - dangerous!
    public fun share_pool(ctx: &mut TxContext) {
        let pool = Pool {
            id: sui::object::new(ctx),
            reserves: sui::balance::zero<SUI>(),
        };
        // LINT: shared_with_balance should fire here
        transfer::share_object(pool);
    }

    /// Share a Treasury
    public fun share_treasury(ctx: &mut TxContext) {
        let treasury = Treasury {
            id: sui::object::new(ctx),
            funds: sui::balance::zero<SUI>(),
        };
        // LINT: shared_with_balance should fire here
        transfer::share_object(treasury);
    }

    /// Public share variant
    public fun public_share_vault(ctx: &mut TxContext) {
        let vault = Vault {
            id: sui::object::new(ctx),
            coin_a: sui::balance::zero<SUI>(),
            coin_b: sui::balance::zero<SUI>(),
        };
        // LINT: shared_with_balance should fire here
        transfer::public_share_object(vault);
    }

    // ==========================================================================
    // NEGATIVE CASES - Should NOT trigger shared_with_balance
    // ==========================================================================

    /// Object without Balance - fine to share
    public struct Config has key {
        id: UID,
        threshold: u64,
    }

    /// Object without Balance - sharing is safe
    public fun share_config(ctx: &mut TxContext) {
        let config = Config {
            id: sui::object::new(ctx),
            threshold: 100,
        };
        // No Balance field, safe to share
        transfer::share_object(config);
    }

    /// Pool transferred, not shared - owner controls it
    public struct OwnedPool has key {
        id: UID,
        reserves: Balance<SUI>,
    }

    /// Transfer (not share) Balance-containing object - fine
    public fun transfer_owned_pool(ctx: &mut TxContext, recipient: address) {
        let pool = OwnedPool {
            id: sui::object::new(ctx),
            reserves: sui::balance::zero<SUI>(),
        };
        // Transfer keeps it owned, not publicly accessible
        transfer::transfer(pool, recipient);
    }

    /// Registry without Balance
    public struct Registry has key {
        id: UID,
        entries: vector<address>,
    }

    /// Share registry - no Balance, safe
    public fun share_registry(ctx: &mut TxContext) {
        let registry = Registry {
            id: sui::object::new(ctx),
            entries: vector[],
        };
        transfer::share_object(registry);
    }
}
