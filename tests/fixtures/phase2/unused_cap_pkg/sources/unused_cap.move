// Test fixture for unused_capability_param_v2 lint
// Tests CFG-aware capability usage tracking

module unused_cap_pkg::unused_cap {
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    // A capability struct with key+store abilities
    public struct AdminCap has key, store {
        id: UID,
    }

    // A pool struct for testing
    public struct Pool has key {
        id: UID,
        value: u64,
    }

    const E_WRONG_CAP: u64 = 1;

    // SHOULD WARN: Capability parameter never used - missing access control
    public fun bad_withdraw(_cap: &AdminCap, pool: &mut Pool, amount: u64): u64 {
        // cap is not validated or used at all!
        let withdrawn = pool.value;
        pool.value = pool.value - amount;
        withdrawn
    }

    // SHOULD NOT WARN: Capability is properly validated
    public fun good_withdraw(cap: &AdminCap, pool: &mut Pool, amount: u64): u64 {
        // Capability is used in assertion - proper access control
        assert!(object::id(cap) == object::uid_to_inner(&pool.id), E_WRONG_CAP);
        let withdrawn = pool.value;
        pool.value = pool.value - amount;
        withdrawn
    }

    // SHOULD NOT WARN: Capability is passed to another function
    public fun delegate_withdraw(cap: &AdminCap, pool: &mut Pool, amount: u64): u64 {
        verify_cap(cap);
        let withdrawn = pool.value;
        pool.value = pool.value - amount;
        withdrawn
    }

    fun verify_cap(_cap: &AdminCap) {
        // Internal verification
    }

    // SHOULD WARN: Capability in conditional but not all paths use it
    public fun conditional_bad(cap: &AdminCap, pool: &mut Pool, use_cap: bool): u64 {
        if (use_cap) {
            // cap used here
            let _ = object::id(cap);
        };
        // But this path doesn't use cap
        pool.value
    }
}
